//! The window: the page's native view in an element that sends the mouse to the page, an orange cover over its corner,
//! and a once-a-second log of GPUI frames.

use std::{cell::Cell, ptr::NonNull, rc::Rc, time::Duration};

use anyhow::Context as _;
use parallax::gpui::{
    App, Bounds, Context, DispatchPhase, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NativeView, Pixels, Point, Render, ScrollDelta, ScrollEvent, Window,
    WindowBounds, WindowOptions, canvas, div, prelude::*, px, rgb, size,
};
use raw_window_handle::{HasWindowHandle as _, RawWindowHandle};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE,
    COREWEBVIEW2_MOUSE_EVENT_KIND_LEFT_BUTTON_DOWN, COREWEBVIEW2_MOUSE_EVENT_KIND_LEFT_BUTTON_UP,
    COREWEBVIEW2_MOUSE_EVENT_KIND_MIDDLE_BUTTON_DOWN,
    COREWEBVIEW2_MOUSE_EVENT_KIND_MIDDLE_BUTTON_UP, COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE,
    COREWEBVIEW2_MOUSE_EVENT_KIND_RIGHT_BUTTON_DOWN, COREWEBVIEW2_MOUSE_EVENT_KIND_RIGHT_BUTTON_UP,
    COREWEBVIEW2_MOUSE_EVENT_KIND_WHEEL, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
    COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_CONTROL,
    COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_LEFT_BUTTON,
    COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_MIDDLE_BUTTON,
    COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_NONE, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_RIGHT_BUTTON,
    COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_SHIFT,
};
use windows::{
    Win32::{Foundation::HWND, UI::Input::KeyboardAndMouse::SetFocus},
    core::Interface as _,
};

use crate::page::Page;

/// Where the view sits in the window and its size, in logical px, as in examples/native-view-windows.
const VIEW_ORIGIN: [f32; 2] = [41., 41.];
const VIEW_SIZE: [f32; 2] = [322., 242.];
/// The colour shown until the page draws. Magenta, so a missing page is easy to find in a screenshot.
const GAP_COLOUR: u32 = 0xff00ff;
const COVER_COLOUR: u32 = 0xff8000;
const WINDOW_COLOUR: u32 = 0x303030;
/// A wheel notch in SendMouseInput's mouseData (WHEEL_DELTA), and the lines GPUI turns it into (Windows' default).
const WHEEL_DELTA: f32 = 120.;
const LINES_PER_NOTCH: f32 = 3.;
/// What a notch scrolls in Chromium on Windows: 3 lines of 100 / 3 px. A precise touchpad's pixel deltas use it.
const PIXELS_PER_NOTCH: f32 = 100.;

/// GPUI button → WebView2's down and up events and its virtual key while held. Extend with a row per button.
const BUTTONS: [(
    MouseButton,
    COREWEBVIEW2_MOUSE_EVENT_KIND,
    COREWEBVIEW2_MOUSE_EVENT_KIND,
    COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
); 3] = [
    (
        MouseButton::Left,
        COREWEBVIEW2_MOUSE_EVENT_KIND_LEFT_BUTTON_DOWN,
        COREWEBVIEW2_MOUSE_EVENT_KIND_LEFT_BUTTON_UP,
        COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_LEFT_BUTTON,
    ),
    (
        MouseButton::Right,
        COREWEBVIEW2_MOUSE_EVENT_KIND_RIGHT_BUTTON_DOWN,
        COREWEBVIEW2_MOUSE_EVENT_KIND_RIGHT_BUTTON_UP,
        COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_RIGHT_BUTTON,
    ),
    (
        MouseButton::Middle,
        COREWEBVIEW2_MOUSE_EVENT_KIND_MIDDLE_BUTTON_DOWN,
        COREWEBVIEW2_MOUSE_EVENT_KIND_MIDDLE_BUTTON_UP,
        COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_MIDDLE_BUTTON,
    ),
];

struct Example {
    view: NativeView,
    page: Page,
    hwnd: HWND,
    /// GPUI frames drawn, counted as the window paints.
    frames: Rc<Cell<u64>>,
    /// The button pressed on the page, while it is held: the page then gets every move and the release, wherever
    /// the pointer goes, as its own window's mouse capture would give it.
    held: Option<MouseButton>,
}

pub fn open_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(640.), px(480.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        cx.new(|cx| {
            Example::new(window, cx).unwrap_or_else(|error| {
                log::error!("webview: cannot attach: {error:#}");
                std::process::exit(1)
            })
        })
    });
    if let Err(error) = opened {
        log::error!("cannot open the window: {error}");
        cx.quit();
    }
}

impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
            anyhow::bail!("not a Win32 window");
        };
        let hwnd = HWND(handle.hwnd.get() as _);
        let (view, page) = Self::attach(hwnd, window, cx)?;
        // A lost GPU device gives GPUI a new DirectComposition device, which the page's visual can't join: the page
        // is made again there. The callback runs inside GPUI's recovery, so the work waits for the app's next turn.
        let (this, async_cx, gpui_window) = (
            cx.entity().downgrade(),
            cx.to_async(),
            Window::window_handle(window),
        );
        let watched = gpui_windows::on_composition_device_changed(window, move || {
            let this = this.clone();
            async_cx
                .spawn(async move |cx| {
                    gpui_window.update(cx, |_, window, cx| {
                        this.update(cx, |this, cx| this.reattach(window, cx))
                    })
                })
                .detach();
        });
        anyhow::ensure!(watched, "GPUI shows no DirectComposition tree");
        let frames = Rc::new(Cell::new(0));
        let counted = frames.clone();
        cx.spawn(async move |_, cx| {
            let mut last = 0;
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                log::info!("per second: gpui frames {}", counted.get() - last);
                last = counted.get();
            }
        })
        .detach();
        Ok(Self {
            view,
            page,
            hwnd,
            frames,
            held: None,
        })
    }

    /// Makes a visual on GPUI's DirectComposition device, the page rendering into it, and its native view.
    fn attach(
        hwnd: HWND,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<(NativeView, Page)> {
        // The page's visual must be on GPUI's own device, the one its window's visual tree is on.
        let device = gpui_windows::composition_device(window)
            .context("GPUI shows no DirectComposition tree")?;
        // SAFETY: plain creation on a live device.
        let visual = unsafe { device.CreateVisual() }?;
        let raw = NonNull::new(visual.as_raw()).context("null visual")?;
        // SAFETY: `raw` is a live IDCompositionVisual on GPUI's device; GPUI takes a reference of its own.
        let view = unsafe { NativeView::new(raw, window, cx) }?;
        view.set_background(rgb(GAP_COLOUR), cx);
        let page = Page::start(hwnd, device, visual)?;
        log::info!("native view: attached");
        Ok((view, page))
    }

    /// The window's DirectComposition device changed: the page and its view are made again on the new one.
    fn reattach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        log::info!("native view: the DirectComposition device changed, attaching the page again");
        match Self::attach(self.hwnd, window, cx) {
            Ok((view, page)) => {
                (self.view, self.page, self.held) = (view, page, None);
                cx.notify();
            }
            Err(error) => log::error!("native view: cannot attach again: {error:#}"),
        }
    }

    /// A move or the release of the held button, wherever the pointer is (the window's capture listeners).
    fn drag(
        &mut self,
        kind: COREWEBVIEW2_MOUSE_EVENT_KIND,
        modifiers: Modifiers,
        position: Point<Pixels>,
        window: &Window,
    ) {
        let held = if kind == COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE {
            self.held
        } else {
            None
        };
        self.page.mouse(
            kind,
            virtual_keys(modifiers, held),
            0,
            self.at(position, window),
        );
    }

    /// The held button was released: the page gets the release, and a leave if the pointer is off it.
    fn release(
        &mut self,
        event: &MouseUpEvent,
        up: COREWEBVIEW2_MOUSE_EVENT_KIND,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.held.is_none() {
            return;
        }
        self.drag(up, event.modifiers, event.position, window);
        self.held = None;
        let over = self
            .view
            .bounds(window)
            .is_some_and(|bounds| bounds.contains(&event.position));
        if !over {
            let keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_NONE;
            self.page
                .mouse(COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE, keys, 0, [0, 0]);
        }
        cx.notify();
    }

    /// A window point in the page, in device px: WebView2's bounds are in raw pixels from the page's top left.
    fn at(&self, position: Point<Pixels>, window: &Window) -> [i32; 2] {
        let origin = self
            .view
            .bounds(window)
            .map_or(Point::default(), |bounds| bounds.origin);
        let at = (position - origin).scale(window.scale_factor());
        [at.x.0.round() as i32, at.y.0.round() as i32]
    }
}

/// WebView2's virtual keys for the modifiers and the button held.
fn virtual_keys(
    modifiers: Modifiers,
    held: Option<MouseButton>,
) -> COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS {
    let button = BUTTONS
        .iter()
        .find(|(button, ..)| Some(*button) == held)
        .map_or(COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_NONE, |(.., keys)| {
            *keys
        });
    // (held, key) pairs; extend with a row per modifier WebView2 takes.
    [
        (modifiers.shift, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_SHIFT),
        (
            modifiers.control,
            COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_CONTROL,
        ),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .fold(button, |keys, (_, key)| {
        COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(keys.0 | key.0)
    })
}

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let [x, y] = VIEW_ORIGIN;
        let [width, height] = VIEW_SIZE;
        let frames = self.frames.clone();
        let page = self.page.clone();
        // Fills the view's element, so its prepaint gets this frame's layout: the page is sized from it, and it is
        // logged for the checks, in device pixels as GPUI snaps the hole.
        let layout = canvas(
            move |bounds: Bounds<Pixels>, window, _| {
                frames.set(frames.get() + 1);
                let scale = window.scale_factor();
                let edge = |logical: Pixels| (f32::from(logical) * scale).round();
                let [left, top, right, bottom] = [
                    edge(bounds.left()),
                    edge(bounds.top()),
                    edge(bounds.right()),
                    edge(bounds.bottom()),
                ];
                log::info!("view placement: device {left} {top} {right} {bottom}");
                page.place([left, top, right, bottom].map(|edge| edge as i32));
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        // The view's own element hears the mouse over it (patch 0048) and sends it on to the page.
        let mut element = self
            .view
            .clone()
            .into_element()
            .id("page")
            // While a button is held, the window's capture listeners below send the moves.
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, _| {
                if this.held.is_none() {
                    let kind = COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE;
                    this.drag(kind, event.modifiers, event.position, window);
                }
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, _| {
                // Mid-drag the page keeps the pointer until the release.
                if !hovered && this.held.is_none() {
                    let keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_NONE;
                    this.page
                        .mouse(COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE, keys, 0, [0, 0]);
                }
            }))
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, _| {
                // Positive away from the user in both.
                let notches = match event.delta {
                    ScrollDelta::Lines(lines) => lines.y / LINES_PER_NOTCH,
                    ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / PIXELS_PER_NOTCH,
                };
                let data = (notches * WHEEL_DELTA).round() as i32;
                log::info!("view input: wheel {data}");
                let keys = virtual_keys(event.modifiers, None);
                this.page.mouse(
                    COREWEBVIEW2_MOUSE_EVENT_KIND_WHEEL,
                    keys,
                    data,
                    this.at(event.position, window),
                );
            }));
        for (button, down, up, _) in BUTTONS {
            // A quick click's release, before `capture` was painted; capture's listener runs first otherwise.
            element = element.on_mouse_up(
                button,
                cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                    this.release(event, up, window, cx)
                }),
            );
            element = element.on_mouse_down(
                button,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    let at = this.at(event.position, window);
                    log::info!("view input: {button:?} down at {} {}", at[0], at[1]);
                    this.page
                        .mouse(down, virtual_keys(event.modifiers, Some(button)), 0, at);
                    this.held = Some(button);
                    // The page takes the keys; the window's own listener below must not take them back.
                    this.page.focus();
                    cx.stop_propagation();
                    cx.notify();
                }),
            );
        }
        // While a button pressed on the page is held: every move and its release, wherever the pointer is.
        let capture = self.held.map(|held| {
            let this = cx.entity();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moving = this.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            let kind = COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE;
                            moving.update(cx, |this, _| {
                                this.drag(kind, event.modifiers, event.position, window)
                            });
                        }
                    });
                    let Some((.., up, _)) =
                        BUTTONS.iter().find(|(button, ..)| *button == held).copied()
                    else {
                        return;
                    };
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && event.button == held {
                            this.update(cx, |this, cx| this.release(event, up, window, cx));
                        }
                    });
                },
            )
            .size_0()
        });
        div()
            .size_full()
            .bg(rgb(WINDOW_COLOUR))
            .children(capture)
            // A click anywhere but the page gives GPUI's window the keys back.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    // SAFETY: GPUI's own live window, on its thread.
                    if let Err(error) = unsafe { SetFocus(Some(this.hwnd)) } {
                        log::error!("cannot take the keyboard back: {error}");
                    }
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height))
                    .child(element)
                    .child(layout)
                    .child(
                        div()
                            .id("cover")
                            .absolute()
                            .top_0()
                            .left_0()
                            .w(px(100.))
                            .h(px(60.))
                            .bg(rgb(COVER_COLOUR))
                            .occlude()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                log::info!("cover: down");
                                cx.stop_propagation();
                            }),
                    ),
            )
    }
}
