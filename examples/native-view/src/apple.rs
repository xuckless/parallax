//! macOS and iOS (docs/native-view-apple.md): an `NSView` / `UIView` that recolours itself on a 60 Hz `NSTimer`, from
//! the app's own main thread, with no GPUI frame. On macOS the view takes the mouse over it (AppKit hit-tests it) and
//! logs it in its own coordinates; on iOS GPUI takes every touch and the view's element logs it. "Move" moves and
//! resizes the slot every frame, "Hide" hides the view, "Drop views" drops every native view handle, which detaches
//! the views for good (iOS: a GPUI field must then still get the keyboard, apple/typing.rs).
//!
//! scripts/apple-native-view-frames.py checks recordings by these colours: the view's hues, the div's orange, the
//! gap's magenta.

use objc2::{
    class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, ClassBuilder, Sel},
    sel,
};
use parallax::gpui::{Context, NativeView, Window};
use std::{ffi::CStr, ptr::NonNull, sync::OnceLock};

use crate::demo::{
    Action, Cover, Demo, Edge, Layout, Look, Made, Motion, Options, Place, Shape, Shown, Target,
    Term,
};

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

pub const TARGET: Target = Target {
    look: Look {
        window: None,
        background: 0xf0f4f8,
        text: 0x1f2933,
        underlay: GAP_COLOUR,
        fill: None,
        // At the right: a rotation's first frames show only the right of the new layout. Opaque, and no hue the
        // view turns through.
        cover: Cover {
            edge: Edge::Right,
            at: [16., 16.],
            size: [150., 70.],
            colour: 0xe07020,
        },
        layout: Layout::Bar(&[Action::Move, Action::Hide, Action::Drop]),
        places: &[Place {
            name: "view",
            half: 0,
            rect: [40., 20., 320., 240.],
            cover: true,
            moves: true,
        }],
        buttons: 0xd9e2ec,
        motion: Motion::Wave([
            Term::new(0., 60., 2., Shape::Sin),
            Term::new(0., 30., 1.7, Shape::Cos),
            Term::new(0., 80., 1.3, Shape::Sin),
            Term::new(0., 60., 1.1, Shape::Cos),
        ]),
        grow: [0., 0.],
    },
    variants: &[("apple", make_view)],
    on_gpui_frame: None,
};

fn make_view(_: &Options, window: &mut Window, cx: &mut Context<Demo>) -> Result<Made, String> {
    let view = new_view();
    // SAFETY: `view` is a live, parentless NSView / UIView, on the main thread.
    let native_view = unsafe { NativeView::new(NonNull::from(&*view).cast(), window, cx) }
        .map_err(|error| format!("{error:#}"))?;
    let ticking = view.clone();
    #[cfg_attr(
        not(target_os = "ios"),
        expect(unused_mut, reason = "iOS adds its text fields")
    )]
    let mut made = Made::new(vec![Shown::new("view", native_view, view)], move || {
        ticks(&ticking)
    });
    #[cfg(target_os = "ios")]
    {
        made.extra = Some(Box::new(typing::Typing::new(window, cx)?));
    }
    Ok(made)
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

/// Native view frames so far.
fn ticks(view: &AnyObject) -> u64 {
    let Some(ivar) = view.class().instance_variable(TICKS) else {
        return 0;
    };
    // SAFETY: The ivar is the `u64` added in `view_class`, on the main thread.
    unsafe { *ivar.load_ptr::<u64>(view) }
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
            "native-view: NSView: {} at ({:.1}, {:.1}), scroll dy {delta_y:.1}",
            selector.name().to_string_lossy(),
            in_view.x,
            in_view.y
        );
    }
}

/// GPUI's view is on top and takes every touch, so this should never log unless the app forwards touches.
#[cfg(target_os = "ios")]
extern "C" fn ios_touch(_: &AnyObject, _: Sel, _: *mut AnyObject, _: *mut AnyObject) {
    log::warn!("native-view: UIView got a touch; GPUI's view should have taken it");
}
