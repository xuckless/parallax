//! One Activity's GPUI window as Android sees it: its surface state machine, renderer, metrics, input and text
//! input, and GPUI's callbacks for it. Each live `ParallaxActivity` has one; all run on the Android main thread, as
//! desktop GPUI's windows share one thread. The app-wide side (Activities, GPU, launch) is `host.rs`.
//! How it fits together: docs/android-multi-window.md.

use std::{
    cell::{Cell, RefCell},
    sync::Arc,
    time::Instant,
};

use gpui::{
    AnyWindowHandle, DevicePixels, DispatchEventResult, Edges, Haptic, Pixels, PlatformAtlas,
    PlatformInput, PlatformInputHandler, RequestFrameOptions, Size, WindowInsets, WindowVisibility,
    point, px,
};
use gpui_wgpu::{GpuContext, WgpuRenderer};
use serde_json::Value;

use crate::{
    Error,
    backend::Device,
    haptics, keys,
    lifecycle::ActivityId,
    metrics::Metrics,
    pointer,
    renderer::{self, NativeWindow},
    surface::{Action, Surface, SurfaceEvent},
    text_input::{self, Edit, EditingState},
};

/// A GPUI callback the window registered, taken out while it runs so it can re-enter the shell.
pub(crate) struct Slot<F: ?Sized>(RefCell<Option<Box<F>>>);

impl<F: ?Sized> Slot<F> {
    pub(crate) fn set(&self, callback: Box<F>) {
        *self.0.borrow_mut() = Some(callback);
    }

    /// Runs `call` on the callback; `None` when GPUI has not registered one.
    pub(crate) fn with<R>(&self, call: impl FnOnce(&mut F) -> R) -> Option<R> {
        let mut callback = self.0.borrow_mut().take()?;
        let result = call(&mut callback);
        self.0.borrow_mut().get_or_insert(callback);
        Some(result)
    }
}

impl<F: ?Sized> Default for Slot<F> {
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}

/// GPUI's callbacks for one window.
#[derive(Default)]
pub(crate) struct Callbacks {
    pub(crate) request_frame: Slot<dyn FnMut(RequestFrameOptions)>,
    /// Pointers arrive here as touch, mouse and scroll input ([`ActivityWindow::pointer`]), keys as key, modifier and
    /// back events ([`ActivityWindow::key`]).
    pub(crate) input: Slot<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    pub(crate) resize: Slot<dyn FnMut(Size<Pixels>, f32)>,
    pub(crate) insets_changed: Slot<dyn FnMut(WindowInsets)>,
    pub(crate) active_status: Slot<dyn FnMut(bool)>,
    pub(crate) visibility: Slot<dyn FnMut(WindowVisibility)>,
    /// Run once, when the Activity is destroyed: GPUI removes the window.
    pub(crate) close: RefCell<Option<Box<dyn FnOnce()>>>,
}

/// Whether the renderer's current surface has shown a frame. Each new surface (attach, device recovery) starts
/// `Pending`; the window's first successful draw on it logs "parallax: first frame drawn on a WxH … surface" and
/// moves it to `Drawn`. scripts/android-smoke.sh greps that line to know a launch, rotation or resume really drew.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FirstFrame {
    Pending,
    Drawn,
}

/// Where the Activity's GPUI window is. One per Activity; the renderer lives only in `Open`.
pub(crate) enum Gpui {
    /// No GPUI window yet: the app's `launch` runs on the Activity's first surface.
    Waiting,
    /// `launch` ran for this Activity; the window it opens (`Platform::open_window`) binds here.
    Launched,
    /// GPUI's window for this Activity, drawn by its renderer.
    Open {
        handle: AnyWindowHandle,
        /// Boxed: a renderer is large, and the other states hold nothing.
        renderer: Box<WgpuRenderer>,
    },
}

/// One Activity's window. Main thread only.
pub(crate) struct ActivityWindow {
    pub(crate) activity: ActivityId,
    surface: RefCell<Surface<NativeWindow>>,
    pub(crate) metrics: Cell<Metrics>,
    /// The app's shared GPU context: every window's renderer draws with it.
    gpu: GpuContext,
    /// Picks the renderer's backend order ([`crate::backend`]); `ro.hardware` never changes, so it is read once.
    device: Device,
    pub(crate) gpui: RefCell<Gpui>,
    pub(crate) callbacks: Callbacks,
    pub(crate) pointers: RefCell<pointer::Pointers>,
    pub(crate) keyboard: RefCell<keys::Keyboard>,
    /// The focused field's handler: GPUI installs it every frame ([`crate::window`]), the IME queries it.
    pub(crate) input_handler: RefCell<Option<PlatformInputHandler>>,
    pub(crate) text_input: RefCell<text_input::Session>,
    /// Haptics GPUI played since Java last asked ([`ActivityWindow::framework_calls`]), oldest first; none while
    /// frames stop.
    pub(crate) haptics: RefCell<Vec<Haptic>>,
    /// A plain `Cell`: only the main thread draws and attaches surfaces.
    pub(crate) first_frame: Cell<FirstFrame>,
}

impl ActivityWindow {
    pub(crate) fn new(activity: ActivityId, gpu: GpuContext, device: Device) -> Self {
        Self {
            activity,
            surface: RefCell::new(Surface::None),
            metrics: Cell::new(Metrics::BEFORE_FIRST_SURFACE),
            gpu,
            device,
            gpui: RefCell::new(Gpui::Waiting),
            callbacks: Callbacks::default(),
            pointers: RefCell::new(pointer::Pointers::default()),
            keyboard: RefCell::new(keys::Keyboard::default()),
            input_handler: RefCell::new(None),
            text_input: RefCell::new(text_input::Session::default()),
            haptics: RefCell::new(Vec::new()),
            first_frame: Cell::new(FirstFrame::Pending),
        }
    }

    /// GPUI's handle for this Activity's window, once open.
    pub(crate) fn gpui_handle(&self) -> Option<AnyWindowHandle> {
        match &*self.gpui.borrow() {
            Gpui::Open { handle, .. } => Some(*handle),
            Gpui::Waiting | Gpui::Launched => None,
        }
    }

    /// Runs `call` on the renderer; `None` before GPUI opens the window.
    pub(crate) fn with_renderer<R>(&self, call: impl FnOnce(&mut WgpuRenderer) -> R) -> Option<R> {
        match &mut *self.gpui.borrow_mut() {
            Gpui::Open { renderer, .. } => Some(call(renderer)),
            Gpui::Waiting | Gpui::Launched => None,
        }
    }

    pub(crate) fn set_display(&self, size: Size<DevicePixels>, refresh_rate: f32) {
        self.metrics.set(Metrics {
            size,
            refresh_rate,
            ..self.metrics.get()
        });
    }

    pub(crate) fn set_insets(&self, safe_area: Edges<DevicePixels>, ime: Edges<DevicePixels>) {
        let metrics = Metrics {
            safe_area,
            ime,
            ..self.metrics.get()
        };
        self.metrics.set(metrics);
        log::info!(
            "parallax: Activity {}: insets in pixels: safe area {safe_area:?}, ime {ime:?}",
            self.activity.0
        );
        self.callbacks
            .insets_changed
            .with(|callback| callback(metrics.insets()));
    }

    /// The current native window: ready, or waiting for the renderer to be rebuilt on it.
    fn current_window(&self) -> Option<NativeWindow> {
        match &*self.surface.borrow() {
            Surface::Ready { window, .. } | Surface::Recovering { window, .. } => {
                Some(window.clone())
            }
            Surface::None | Surface::Lost { .. } => None,
        }
    }

    /// Whether the app's `launch` waits for this Activity: it ran for it, and GPUI has not opened its window.
    pub(crate) fn is_launched(&self) -> bool {
        match &*self.gpui.borrow() {
            Gpui::Launched => true,
            Gpui::Waiting | Gpui::Open { .. } => false,
        }
    }

    /// Opens this Activity's GPUI window: creates the renderer on the ready surface and returns its atlas.
    pub(crate) fn open(&self, handle: AnyWindowHandle) -> Result<Arc<dyn PlatformAtlas>, Error> {
        let window = self.current_window().ok_or(Error::NoSurface)?;
        let renderer = renderer::create(&self.gpu, self.device, &window, self.metrics.get().size)?;
        let atlas: Arc<dyn PlatformAtlas> = renderer.sprite_atlas().clone();
        *self.gpui.borrow_mut() = Gpui::Open {
            handle,
            renderer: Box::new(renderer),
        };
        Ok(atlas)
    }

    /// The Activity was destroyed: GPUI closes its window (on its next turn) and drops the renderer with it.
    pub(crate) fn close(&self) {
        let close = self.callbacks.close.borrow_mut().take();
        if let Some(close) = close {
            close();
        }
    }

    /// Feeds one event through the surface state machine and carries out its action on the renderer.
    pub(crate) fn handle(&self, event: SurfaceEvent<NativeWindow>) {
        let state = self.surface.replace(Surface::None);
        let (next, action) = state.next(event);
        // Not every vsync: Draw and Wait are the quiet steady states.
        if !matches!(action, Action::Draw | Action::Wait) {
            log::info!(
                "parallax: Activity {}: surface {next:?}, {action:?}",
                self.activity.0
            );
        }
        *self.surface.borrow_mut() = next;
        let size = self.metrics.get().size;
        match action {
            Action::Attach { previous } => {
                let window = self.current_window();
                let attached = self
                    .with_renderer(|renderer| {
                        let Some(window) = window else {
                            return Ok(());
                        };
                        // The old surface lets go of its window first: Vulkan and EGL connect to an ANativeWindow
                        // once, and the window may be the same one (a resize).
                        renderer.unconfigure_surface();
                        let instance = self
                            .gpu
                            .borrow()
                            .as_ref()
                            .map(|context| context.instance.clone());
                        instance.ok_or(Error::NoBackend).and_then(|instance| {
                            renderer
                                .replace_surface(&window, renderer::surface_config(size), &instance)
                                .map_err(|error| Error::Renderer(format!("{error:#}")))
                        })
                    })
                    // Before GPUI opens the window there is no renderer: it is created on this window then.
                    .unwrap_or(Ok(()));
                // Released only now: the renderer has dropped its surface on the previous window.
                drop(previous);
                match attached {
                    Ok(()) => {
                        self.first_frame.set(FirstFrame::Pending);
                        self.resized();
                    }
                    Err(error) => {
                        // The state machine rebuilds the renderer after the backoff. scripts/android-smoke.sh fails
                        // on this line: keep its wording.
                        log::error!("parallax: cannot attach the new surface: {error}");
                        self.handle(SurfaceEvent::AttachFailed { at: Instant::now() });
                    }
                }
            }
            Action::Detach { window } => {
                self.with_renderer(WgpuRenderer::unconfigure_surface);
                // Android requires the window released before surfaceDestroyed returns.
                drop(window);
            }
            Action::RecoverDevice => {
                let window = self.current_window();
                let recovered = self
                    .with_renderer(|renderer| match window {
                        Some(window) => {
                            renderer::recover(&self.gpu, self.device, renderer, &window, size)
                        }
                        None => Err(Error::NoSurface),
                    })
                    .unwrap_or(Err(Error::NoSurface));
                match recovered {
                    Ok(()) => self.handle(SurfaceEvent::Recovered),
                    Err(error) => {
                        // The state machine waits longer before the next try. scripts/android-smoke.sh fails on
                        // this line: keep its wording.
                        log::error!("parallax: GPU device recovery failed: {error}");
                        self.handle(SurfaceEvent::RecoveryFailed { at: Instant::now() });
                    }
                }
            }
            Action::Redraw => {
                self.first_frame.set(FirstFrame::Pending);
                self.resized();
            }
            Action::Draw => {
                self.callbacks
                    .request_frame
                    .with(|callback| callback(RequestFrameOptions::default()));
            }
            Action::Wait => {}
            Action::Reject { why } => log::warn!("parallax: surface event ignored: {why}"),
        }
    }

    /// Tells GPUI the window size or scale may have changed, then redraws the new surface.
    pub(crate) fn resized(&self) {
        let metrics = self.metrics.get();
        self.callbacks
            .resize
            .with(|callback| callback(metrics.bounds().size, metrics.density));
        self.redraw();
    }

    /// A full frame: after a new surface, size or device the old content is gone.
    fn redraw(&self) {
        self.callbacks.request_frame.with(|callback| {
            callback(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            })
        });
    }

    /// One pointer data packet → GPUI touch, mouse and scroll events, dispatched in packet order. Upstream turns a
    /// tap into a click; a mouse clicks itself. The keyboard is read before GPUI runs (it reads modifiers too).
    pub(crate) fn pointer(&self, packet: &[u8]) {
        let modifiers = self.keyboard.borrow().modifiers();
        let mut uptime = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: `uptime` is a valid, writable timespec and CLOCK_MONOTONIC always exists on Android; on the
        // impossible failure it stays zero, which only makes touches read as dispatched now.
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut uptime) };
        #[allow(
            clippy::useless_conversion,
            reason = "`time_t` and `c_long` are 32 bits on armv7 and x86: widen before multiplying"
        )]
        let clock = pointer::Clock {
            now: Instant::now(),
            uptime_micros: i64::from(uptime.tv_sec) * 1_000_000 + i64::from(uptime.tv_nsec) / 1_000,
        };
        let events = match pointer::decode(packet) {
            Ok(data) => self.pointers.borrow_mut().events(
                &data,
                self.metrics.get().density,
                modifiers,
                clock,
            ),
            Err(error) => {
                log::error!("parallax: pointer packet dropped: {error}");
                return;
            }
        };
        for event in events {
            log::debug!("parallax: {event:?}");
            self.callbacks.input.with(|callback| {
                callback(event);
            });
        }
    }

    /// One key data packet → GPUI; returns whether GPUI handled it. The keyboard is not borrowed while GPUI runs,
    /// so the window can read modifiers during dispatch.
    pub(crate) fn key(&self, packet: &[u8], extras: keys::KeyEventExtras) -> bool {
        let data = match keys::decode(packet) {
            Ok(data) => data,
            Err(error) => {
                log::error!("parallax: key dropped: {error}");
                return false;
            }
        };
        // GPUI hit-tests back gestures; Android's back key has none, so it starts at the left edge, mid-height,
        // where an edge swipe would (iOS's back gesture comes from the leading edge).
        let back_at = point(px(0.), self.metrics.get().bounds().size.height / 2.);
        let step = self.keyboard.borrow_mut().input(&data, extras, back_at);
        log::debug!("parallax: {data:?} → {step:?}");
        match step {
            keys::Step::Answer { input, handled } => {
                if let Some(input) = input {
                    self.callbacks.input.with(|callback| callback(input));
                }
                handled
            }
            keys::Step::Ask(input) => {
                // No window yet: nothing handled it, so Android gets the key.
                let result = self
                    .callbacks
                    .input
                    .with(|callback| callback(input))
                    .unwrap_or(DispatchEventResult {
                        propagate: true,
                        default_prevented: false,
                    });
                self.keyboard.borrow_mut().reply(result)
            }
        }
    }

    /// Runs `call` on the focused field's handler, taken out so GPUI can re-enter the window; `None` without one.
    /// Re-entrancy: if GPUI installs a new handler during `call` (`set_input_handler`), the newer one is kept and
    /// this one dropped.
    fn with_input_handler<R>(
        &self,
        call: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Option<R> {
        let mut handler = self.input_handler.borrow_mut().take()?;
        let result = call(&mut handler);
        self.input_handler.borrow_mut().get_or_insert(handler);
        Some(result)
    }

    /// The pending text-input calls for Java, as Flutter's JSON; empties them. Never logs the text (password
    /// fields included): the calls carry it.
    fn text_input_calls(&self) -> Option<Value> {
        let pending = self.text_input.borrow_mut().take()?;
        let state = if pending.sends_state() {
            self.with_input_handler(|handler| {
                let state = read_state(handler)?;
                match pending.commit(&state) {
                    Some(edit) => {
                        log::debug!("parallax: composition committed for a new client");
                        apply(handler, edit);
                        read_state(handler)
                    }
                    None => Some(state),
                }
            })
            .flatten()
        } else {
            None
        };
        if pending.sets_client() && state.is_none() {
            // Once per focus: each focus sets a new client.
            log::error!(
                "parallax: the focused field's text is unreadable (no text_length_utf16 or text_for_range): \
                 the IME sees it empty"
            );
        }
        // The field's text bounds, for autofill's rect: the caret when the field is empty.
        let reads_bounds = self.text_input.borrow().reads_bounds(&pending);
        let bounds = if reads_bounds {
            self.with_input_handler(|handler| {
                let length = match &state {
                    Some(state) => state.text.encode_utf16().count(),
                    None => handler.text_length_utf16().unwrap_or(0),
                };
                handler.bounds_for_range(0..length)
            })
            .flatten()
        } else {
            None
        };
        let described = state.as_ref().map_or("none".to_string(), describe);
        let calls = self.text_input.borrow_mut().send(pending, state, bounds)?;
        log::debug!("parallax: text input {pending:?}, state: {described}");
        Some(calls)
    }

    /// GPUI's pending calls for Java, emptied: a JSON object of Flutter channel → its calls, with only the channels
    /// that have some (`flutter/textinput`: [`Self::text_input_calls`]; `flutter/platform`: the haptics,
    /// [`haptics::platform_calls`]); `None` when none has.
    pub(crate) fn framework_calls(&self) -> Option<Value> {
        let channels = [
            ("flutter/textinput", self.text_input_calls()),
            (
                "flutter/platform",
                haptics::platform_calls(&self.haptics.take()),
            ),
        ];
        let calls: serde_json::Map<String, Value> = channels
            .into_iter()
            .filter_map(|(channel, calls)| Some((channel.to_string(), calls?)))
            .collect();
        (!calls.is_empty()).then_some(Value::Object(calls))
    }

    /// Java's editing state after an IME edit → GPUI, synchronously: merged onto GPUI's state when GPUI changed
    /// since Java's mirror was last current ([`text_input::Session::java_edit`]).
    pub(crate) fn update_editing_state(&self, java: EditingState) {
        self.with_input_handler(|handler| {
            let Some(gpui) = read_state(handler) else {
                log::debug!("parallax: IME edit on an unreadable field dropped");
                return;
            };
            let Some(target) = self.text_input.borrow_mut().java_edit(&java, &gpui) else {
                log::debug!("parallax: IME edit for a client GPUI is replacing dropped");
                return;
            };
            let edits = text_input::edits(&gpui, &target);
            log::debug!(
                "parallax: IME {} over {} → {} calls",
                describe(&java),
                describe(&gpui),
                edits.len()
            );
            for edit in edits {
                apply(handler, edit);
            }
            // The app may refuse or change the edit (a filter, a length limit): the session decides whether Java
            // must take GPUI's state.
            let after = read_state(handler).unwrap_or(target);
            self.text_input.borrow_mut().java_edited(java, &after);
        });
    }

    /// The IME's editor action: GPUI gets Enter, as a hardware key (see [`keys::Keyboard::press`]).
    pub(crate) fn perform_action(&self) {
        let inputs = self.keyboard.borrow().press("enter");
        for input in inputs {
            self.callbacks.input.with(|callback| callback(input));
        }
    }

    /// GPUI's bounds of a UTF-16 range as `[left, top, right, bottom]` in view pixels.
    pub(crate) fn bounds_for_range(&self, range: std::ops::Range<usize>) -> Option<[f32; 4]> {
        let bounds = self
            .with_input_handler(|handler| handler.bounds_for_range(range))
            .flatten()?;
        let density = self.metrics.get().density;
        Some([
            f32::from(bounds.left()) * density,
            f32::from(bounds.top()) * density,
            f32::from(bounds.right()) * density,
            f32::from(bounds.bottom()) * density,
        ])
    }

    /// Every vsync (Choreographer): GPUI draws if anything changed, or device recovery moves on.
    pub(crate) fn frame(&self) {
        let device_lost = self
            .with_renderer(|renderer| renderer.device_lost())
            .unwrap_or(false);
        self.handle(SurfaceEvent::Frame {
            at: Instant::now(),
            device_lost,
        });
    }
}

/// One [`Edit`] as GPUI's `InputHandler` call.
fn apply(handler: &mut PlatformInputHandler, edit: Edit) {
    match edit {
        Edit::Replace { range, text } => handler.replace_text_in_range(Some(range), &text),
        Edit::Mark {
            range,
            text,
            selected,
        } => handler.replace_and_mark_text_in_range(Some(range), &text, selected),
        Edit::Unmark => handler.unmark_text(),
        Edit::Select(range) => handler.set_selected_text_range(range),
    }
}

/// The focused field's whole editing state, in UTF-16, clamped to its text ([`EditingState::clamped`]). The
/// editable range is not honoured yet: Java mirrors the whole document. A handler without `text_length_utf16`
/// (GPUI's default is `None`) is asked for everything; the range it adjusts to must start at 0.
fn read_state(handler: &mut PlatformInputHandler) -> Option<EditingState> {
    let mut adjusted = None;
    let text = match handler.text_length_utf16() {
        Some(length) => handler.text_for_range(0..length, &mut adjusted),
        None => handler.text_for_range(0..usize::MAX, &mut adjusted),
    }
    .filter(|_| adjusted.as_ref().is_none_or(|range| range.start == 0))?;
    let selection = handler
        .selected_text_range(false)
        .map(|selection| match selection.reversed {
            true => (selection.range.end, selection.range.start),
            false => (selection.range.start, selection.range.end),
        });
    let composing = handler.marked_text_range();
    Some(EditingState::clamped(text, selection, composing))
}

/// An editing state for the log: its length and ranges, never its text (it may be a password).
fn describe(state: &EditingState) -> String {
    format!(
        "{} UTF-16 units, selection {:?}, composing {:?}",
        state.text.encode_utf16().count(),
        state.selection,
        state.composing
    )
}
