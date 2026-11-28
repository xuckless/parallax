//! A native view under a GPUI window: an `NSView` on macOS, a `UIView` on iOS (docs/native-view-apple.md).
//!
//! The view recolours itself on a 60 Hz `NSTimer`, from the app's own main thread, with no GPUI frame; a GPUI div
//! covers part of it. Once a second the log says how many frames each side drew:
//! `native-view-apple: gpui frames/s N, native view frames/s M`. While only the view changes, N is 0.
//!
//! Input: on macOS the view takes the mouse over it (AppKit hit-tests it) and logs it in its own coordinates; on iOS
//! GPUI takes every touch, and the slot's listeners log it relative to the view. Either way the div over the view
//! takes its own clicks. "Move" moves and resizes the view's slot every frame; the gap colour (magenta, set with
//! `NativeView::set_background`) is what would show if hole and view fell out of step.

use objc2::{
    class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, ClassBuilder, Sel},
    sel,
};
use parallax::gpui::{
    App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, NativeView, Pixels,
    ScrollEvent, Window, WindowOptions, canvas, div, prelude::*, px, rgb, rgba,
};
use std::{cell::Cell, ffi::CStr, ptr::NonNull, rc::Rc, sync::OnceLock, time::Duration};

#[cfg(target_os = "ios")]
mod typing;

/// The view's colour turns through these hues (degrees), never near the magenta gap colour.
const HUES: std::ops::Range<f32> = 90.0..210.0;
/// The native view's own frame rate: one recolour per timer tick.
const NATIVE_FPS: f64 = 60.;
/// Shown wherever hole and view are out of step: nothing else on screen is this colour.
const GAP_COLOUR: u32 = 0xff00ff;
const TICKS: &CStr = c"ticks";

#[cfg(target_os = "macos")]
const SUPERCLASS: &CStr = c"NSView";
#[cfg(target_os = "ios")]
const SUPERCLASS: &CStr = c"UIView";

/// Where the view's slot is: still, or moving and resizing every frame since the given time.
enum Motion {
    Still,
    Moving { since: std::time::Instant },
}

struct Example {
    native_view: NativeView,
    motion: Motion,
    shown: bool,
    /// GPUI frames since the last report: one per `render`.
    frames: u32,
    /// The slot's bounds as last laid out, for view-relative positions.
    slot: Rc<Cell<Bounds<Pixels>>>,
    overlay_clicks: u32,
    /// iOS: keyboard focus between a GPUI field and a native `UITextField` (typing.rs).
    #[cfg(target_os = "ios")]
    typing: typing::Typing,
    _report: parallax::gpui::Task<()>,
}

pub fn open_window(cx: &mut App) {
    cx.open_window(WindowOptions::default(), |window, cx| {
        cx.new(|cx| Example::new(window, cx))
    })
    .map(|_| ())
    .unwrap_or_else(|error| log::error!("native-view-apple: cannot open the window: {error:#}"));
    cx.activate(true);
}

impl Example {
    #[expect(
        clippy::expect_used,
        reason = "the example has nothing to show without its view"
    )]
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = new_view();
        // SAFETY: `view` is a live, parentless NSView / UIView, on the main thread.
        let native_view = unsafe { NativeView::new(NonNull::from(&*view).cast(), window, cx) }
            .expect("macOS and iOS windows show native views");
        native_view.set_background(rgb(GAP_COLOUR), cx);
        let report_view = view.clone();
        let report = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let native_frames = take_ticks(&report_view);
                // No notify: reporting must not cause a frame of its own.
                let reported = this.update(cx, |this, _| {
                    log::info!(
                        "native-view-apple: gpui frames/s {}, native view frames/s {native_frames}",
                        this.frames
                    );
                    this.frames = 0;
                });
                if reported.is_err() {
                    break;
                }
            }
        });
        Self {
            native_view,
            motion: Motion::Still,
            shown: true,
            frames: 0,
            slot: Rc::default(),
            overlay_clicks: 0,
            #[cfg(target_os = "ios")]
            typing: typing::Typing::new(window, cx),
            _report: report,
        }
    }

    fn button(
        id: &'static str,
        label: String,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(id)
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0xd9e2ec))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                on_click(this, cx);
                cx.notify();
            }))
    }
}

impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frames += 1;
        let (left, top, width, height) = match self.motion {
            Motion::Still => (40., 20., 320., 240.),
            Motion::Moving { since } => {
                window.request_animation_frame();
                let t = since.elapsed().as_secs_f32();
                (
                    40. + 60. * (t * 2.).sin(),
                    20. + 30. * (t * 1.7).cos(),
                    320. + 80. * (t * 1.3).sin(),
                    240. + 60. * (t * 1.1).cos(),
                )
            }
        };
        let slot = self.slot.clone();
        let relative = {
            let slot = self.slot.clone();
            move |position: parallax::gpui::Point<Pixels>| position - slot.get().origin
        };
        let (down, moved, scrolled) = (relative.clone(), relative.clone(), relative);
        let moving = matches!(self.motion, Motion::Moving { .. });
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .pt(px(60.))
            .px_4()
            .bg(rgb(0xf0f4f8))
            .text_color(rgb(0x1f2933))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(Self::button(
                        "move",
                        if moving { "Stop" } else { "Move" }.into(),
                        |this, _| {
                            this.motion = match this.motion {
                                Motion::Still => Motion::Moving {
                                    since: std::time::Instant::now(),
                                },
                                Motion::Moving { .. } => Motion::Still,
                            };
                        },
                        cx,
                    ))
                    .child(Self::button(
                        "show",
                        if self.shown { "Hide" } else { "Show" }.into(),
                        |this, _| this.shown = !this.shown,
                        cx,
                    ))
                    .child(format!("div clicks: {}", self.overlay_clicks)),
            )
            .children({
                #[cfg(target_os = "ios")]
                let typing = Some(self.typing.render(cx));
                #[cfg(not(target_os = "ios"))]
                let typing: Option<parallax::gpui::Div> = None;
                typing
            })
            .child(
                div().flex_1().relative().child(
                    div()
                        .id("slot")
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(width))
                        .h(px(height))
                        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, _| {
                            log::info!(
                                "native-view-apple: slot: mouse down at {:?}",
                                down(event.position)
                            );
                        })
                        .on_mouse_move(move |event: &MouseMoveEvent, _, _| {
                            if event.dragging() {
                                log::info!(
                                    "native-view-apple: slot: drag at {:?}",
                                    moved(event.position)
                                );
                            }
                        })
                        .on_scroll(move |event: &ScrollEvent, _, _| {
                            log::info!(
                                "native-view-apple: slot: scroll {:?} at {:?}",
                                event.delta,
                                scrolled(event.position)
                            );
                        })
                        .child(
                            canvas(move |bounds, _, _| slot.set(bounds), |_, _, _, _| {})
                                .absolute()
                                .size_full(),
                        )
                        .when(self.shown, |this| this.child(self.native_view.clone()))
                        .child(
                            div()
                                .id("overlay")
                                .absolute()
                                .top(px(16.))
                                .left(px(16.))
                                .w(px(150.))
                                .h(px(70.))
                                .p_2()
                                .rounded_md()
                                .bg(rgba(0xffffffe6))
                                .occlude()
                                .child("A GPUI div")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.overlay_clicks += 1;
                                        log::info!(
                                            "native-view-apple: div over the view: click {}",
                                            this.overlay_clicks
                                        );
                                        cx.notify();
                                    }),
                                ),
                        ),
                ),
            )
    }
}

/// Makes the native view and starts its 60 Hz recolour timer.
fn new_view() -> Retained<AnyObject> {
    let class = view_class();
    // SAFETY: `class` is an NSView / UIView subclass built below; `NSTimer` and
    // `NSRunLoop` are Foundation's, on the main thread.
    unsafe {
        let view: Retained<AnyObject> = msg_send![class, new];
        #[cfg(target_os = "macos")]
        let _: () = msg_send![&*view, setWantsLayer: true];
        let timer: Retained<AnyObject> = msg_send![
            class!(NSTimer),
            timerWithTimeInterval: 1. / NATIVE_FPS,
            target: &*view,
            selector: sel!(tick:),
            userInfo: std::ptr::null::<AnyObject>(),
            repeats: true
        ];
        // Common modes: the view keeps changing during a macOS live resize too.
        let run_loop: Retained<AnyObject> = msg_send![class!(NSRunLoop), mainRunLoop];
        let common_modes: Retained<AnyObject> = msg_send![
            class!(NSString),
            stringWithUTF8String: c"kCFRunLoopCommonModes".as_ptr()
        ];
        let _: () = msg_send![&*run_loop, addTimer: &*timer, forMode: &*common_modes];
        view
    }
}

/// Native view frames since the last call.
fn take_ticks(view: &AnyObject) -> u64 {
    let Some(ivar) = view.class().instance_variable(TICKS) else {
        return 0;
    };
    // SAFETY: The ivar is the `u64` added in `view_class`, on the main thread.
    unsafe { std::mem::take(&mut *ivar.load_ptr::<u64>(view)) }
}

#[expect(
    clippy::expect_used,
    reason = "AppKit / UIKit is linked, and only this registers the class"
)]
fn view_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let superclass = AnyClass::get(SUPERCLASS).expect("AppKit / UIKit is linked");
        let mut builder = ClassBuilder::new(c"ParallaxNativeViewExample", superclass)
            .expect("the class is only registered here");
        builder.add_ivar::<u64>(TICKS);
        // SAFETY: Each function matches its selector's signature.
        unsafe {
            builder.add_method(sel!(tick:), tick as extern "C" fn(_, _, _));
            #[cfg(target_os = "macos")]
            {
                builder.add_method(sel!(isFlipped), is_flipped as extern "C" fn(_, _) -> _);
                builder.add_method(
                    sel!(acceptsFirstMouse:),
                    accepts_first_mouse as extern "C" fn(_, _, _) -> _,
                );
                builder.add_method(sel!(mouseDown:), mac_input as extern "C" fn(_, _, _));
                builder.add_method(sel!(mouseDragged:), mac_input as extern "C" fn(_, _, _));
                builder.add_method(sel!(scrollWheel:), mac_input as extern "C" fn(_, _, _));
            }
            #[cfg(target_os = "ios")]
            builder.add_method(
                sel!(touchesBegan:withEvent:),
                ios_touch as extern "C" fn(_, _, _, _),
            );
        }
        builder.register()
    })
}

/// One native frame: the next colour.
extern "C" fn tick(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    let Some(ivar) = this.class().instance_variable(TICKS) else {
        return;
    };
    // SAFETY: The ivar is the view's `u64` tick count; colours are made and set on the main thread.
    unsafe {
        let ticks = &mut *ivar.load_ptr::<u64>(this);
        *ticks += 1;
        let phase = (*ticks as f32 / (NATIVE_FPS as f32 * 2.)).fract();
        let triangle = 1. - (2. * phase - 1.).abs();
        let [r, g, b] = hsv_to_rgb(HUES.start + (HUES.end - HUES.start) * triangle);
        #[cfg(target_os = "macos")]
        {
            let color: *mut AnyObject =
                msg_send![class!(NSColor), colorWithSRGBRed: r, green: g, blue: b, alpha: 1.0f64];
            let cg_color: *mut std::ffi::c_void = msg_send![color, CGColor];
            let layer: *mut AnyObject = msg_send![this, layer];
            let _: () = msg_send![layer, setBackgroundColor: cg_color];
        }
        #[cfg(target_os = "ios")]
        {
            let color: *mut AnyObject =
                msg_send![class!(UIColor), colorWithRed: r, green: g, blue: b, alpha: 1.0f64];
            let _: () = msg_send![this, setBackgroundColor: color];
        }
    }
}

/// Full saturation and value.
fn hsv_to_rgb(hue: f32) -> [f64; 3] {
    let h = hue / 60.;
    let x = 1. - (h % 2. - 1.).abs();
    let [r, g, b] = match h as u32 {
        0 => [1., x, 0.],
        1 => [x, 1., 0.],
        2 => [0., 1., x],
        3 => [0., x, 1.],
        4 => [x, 0., 1.],
        _ => [1., 0., x],
    };
    [r as f64, g as f64, b as f64]
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
// SAFETY: Matches CoreGraphics' `CGPoint`, two doubles.
unsafe impl objc2::Encode for CGPoint {
    const ENCODING: objc2::Encoding =
        objc2::Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}

#[cfg(target_os = "macos")]
extern "C" fn is_flipped(_: &AnyObject, _: Sel) -> objc2::runtime::Bool {
    objc2::runtime::Bool::YES
}

#[cfg(target_os = "macos")]
extern "C" fn accepts_first_mouse(
    _: &AnyObject,
    _: Sel,
    _: *mut AnyObject,
) -> objc2::runtime::Bool {
    objc2::runtime::Bool::YES
}

/// AppKit gave the view a mouse press, drag or scroll: log it in the view's own (flipped) coordinates.
#[cfg(target_os = "macos")]
extern "C" fn mac_input(this: &AnyObject, selector: Sel, event: *mut AnyObject) {
    // SAFETY: AppKit passes a live NSEvent to its view, on the main thread.
    unsafe {
        let in_window: CGPoint = msg_send![event, locationInWindow];
        let in_view: CGPoint =
            msg_send![this, convertPoint: in_window, fromView: std::ptr::null::<AnyObject>()];
        let delta_y: f64 = msg_send![event, scrollingDeltaY];
        log::info!(
            "native-view-apple: NSView: {} at ({:.1}, {:.1}), scroll dy {delta_y:.1}",
            selector.name().to_string_lossy(),
            in_view.x,
            in_view.y
        );
    }
}

/// GPUI's view is on top and takes every touch, so this should never log unless the app forwards touches.
#[cfg(target_os = "ios")]
extern "C" fn ios_touch(_: &AnyObject, _: Sel, _: *mut AnyObject, _: *mut AnyObject) {
    log::warn!("native-view-apple: UIView got a touch; GPUI's view should have taken it");
}
