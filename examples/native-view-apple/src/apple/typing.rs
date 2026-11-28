//! iOS keyboard focus between GPUI and a native view (docs/native-view-apple.md#keyboard): a GPUI text field next
//! to a `UITextField` shown as a native view.
//!
//! A tap on the native field focuses its handle and makes the `UITextField` first responder (GPUI's view takes every
//! touch, so the app does both); GPUI's own text input resigns. A tap on the GPUI field moves focus back, and GPUI's
//! text input takes first responder from the `UITextField`. Log: `UITextField: first responder yes/no`,
//! `UITextField: text "…"`. Dropping the native field while it has the keyboard must hand the keyboard back: a tap on
//! the GPUI field then types there (Return logs hello's `text field submitted: N bytes`).

use objc2::{
    class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
    sel,
};
use parallax::gpui::{
    App, Context, Entity, FocusHandle, Focusable, MouseButton, NativeView, Subscription,
    TextInputConfiguration, Window, div, prelude::*, px, rgb,
};
use std::{ffi::CStr, ptr::NonNull, sync::OnceLock};

use hello::text_field::{self, TextField};

/// `UIControlEventEditingChanged`.
const EDITING_CHANGED: usize = 1 << 17;

pub struct Typing {
    field: Entity<TextField>,
    /// `None` once "Drop views" dropped it.
    native: Option<NativeField>,
}

/// The `UITextField`, shown as a native view.
struct NativeField {
    native_view: NativeView,
    text_field: Retained<AnyObject>,
    _blur: Subscription,
}

impl Typing {
    #[expect(
        clippy::expect_used,
        reason = "the example has nothing to show without its view"
    )]
    pub fn new<V: 'static>(window: &mut Window, cx: &mut Context<V>) -> Self {
        text_field::bind_keys(cx);
        let field =
            cx.new(|cx| TextField::new("GPUI field", TextInputConfiguration::default(), cx));
        let text_field = new_text_field();
        // SAFETY: `text_field` is a live, parentless UITextField, on the main thread.
        let native_view =
            unsafe { NativeView::new(NonNull::from(&*text_field).cast(), window, cx) }
                .expect("iOS windows show native views");
        native_view.set_background(rgb(super::GAP_COLOUR), cx);
        // Focus leaving the native field for something that is not a text field leaves the UITextField first
        // responder (GPUI takes it back only for a field of its own), so the app resigns it.
        let resign = text_field.clone();
        let blur = cx.on_blur(&native_view.focus_handle(cx), window, move |_, _, _| {
            // SAFETY: A live UITextField, on the main thread.
            let _: Bool = unsafe { msg_send![&*resign, resignFirstResponder] };
        });
        Self {
            field,
            native: Some(NativeField {
                native_view,
                text_field,
                _blur: blur,
            }),
        }
    }

    /// Drops the native field's handle, which detaches the `UITextField` (and resigns it if it has the keyboard).
    pub fn drop_native_field(&mut self) {
        self.native = None;
    }

    pub fn render(&self, cx: &App) -> impl IntoElement {
        div()
            .flex()
            .gap_2()
            .h(px(44.))
            .child(div().flex_1().child(self.field.clone()))
            .children(self.native.as_ref().map(|native| {
                let focus: FocusHandle = native.native_view.focus_handle(cx);
                let text_field = native.text_field.clone();
                div()
                    .id("native-field")
                    .w(px(160.))
                    .h_full()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        // The contract: focus the view's handle, then make the view first responder.
                        window.focus(&focus, cx);
                        // SAFETY: A live UITextField, on the main thread.
                        let _: Bool = unsafe { msg_send![&*text_field, becomeFirstResponder] };
                    })
                    .child(native.native_view.clone())
            }))
    }
}

/// A bordered `UITextField` that logs its text and first-responder changes.
fn new_text_field() -> Retained<AnyObject> {
    let class = text_field_class();
    // SAFETY: `class` is a UITextField subclass built below; UIKit setters on the main thread.
    unsafe {
        let field: Retained<AnyObject> = msg_send![class, new];
        // A square, opaque field: a native view is treated as opaque, and rounded corners would show the gap colour.
        let _: () = msg_send![&*field, setBorderStyle: 1isize]; // UITextBorderStyleLine
        let white: *mut AnyObject = msg_send![class!(UIColor), whiteColor];
        let _: () = msg_send![&*field, setBackgroundColor: white];
        let placeholder: Retained<AnyObject> = msg_send![
            class!(NSString),
            stringWithUTF8String: c"UITextField".as_ptr()
        ];
        let _: () = msg_send![&*field, setPlaceholder: &*placeholder];
        let _: () = msg_send![
            &*field,
            addTarget: &*field,
            action: sel!(editingChanged:),
            forControlEvents: EDITING_CHANGED
        ];
        field
    }
}

#[expect(
    clippy::expect_used,
    reason = "UIKit is linked, and only this registers the class"
)]
fn text_field_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let superclass = AnyClass::get(c"UITextField").expect("UIKit is linked");
        let mut builder = ClassBuilder::new(c"ParallaxExampleTextField", superclass)
            .expect("the class is only registered here");
        // SAFETY: Each function matches its selector's signature.
        unsafe {
            builder.add_method(
                sel!(editingChanged:),
                editing_changed as extern "C" fn(_, _, _),
            );
            builder.add_method(
                sel!(becomeFirstResponder),
                become_first_responder as extern "C" fn(_, _) -> _,
            );
            builder.add_method(
                sel!(resignFirstResponder),
                resign_first_responder as extern "C" fn(_, _) -> _,
            );
        }
        builder.register()
    })
}

extern "C" fn editing_changed(this: &AnyObject, _: Sel, _: *mut AnyObject) {
    // SAFETY: `text` is a UITextField's NSString, read on the main thread.
    unsafe {
        let text: *mut AnyObject = msg_send![this, text];
        let utf8: *const std::ffi::c_char = if text.is_null() {
            c"".as_ptr()
        } else {
            msg_send![text, UTF8String]
        };
        log::info!(
            "native-view-apple: UITextField: text {:?}",
            CStr::from_ptr(utf8).to_string_lossy()
        );
    }
}

/// UITextField's own `becomeFirstResponder` or `resignFirstResponder`.
fn call_super(this: &AnyObject, selector: Sel) -> Bool {
    let Some(superclass) = text_field_class().superclass() else {
        return Bool::NO;
    };
    // SAFETY: Both selectors are UIResponder's, take nothing and return a BOOL; on the main thread.
    unsafe { objc2::runtime::MessageReceiver::send_super_message(this, superclass, selector, ()) }
}

extern "C" fn become_first_responder(this: &AnyObject, selector: Sel) -> Bool {
    let became = call_super(this, selector);
    log::info!(
        "native-view-apple: UITextField: first responder {}",
        if became.as_bool() {
            "yes"
        } else {
            "no (refused)"
        }
    );
    became
}

extern "C" fn resign_first_responder(this: &AnyObject, selector: Sel) -> Bool {
    let resigned = call_super(this, selector);
    if resigned.as_bool() {
        log::info!("native-view-apple: UITextField: first responder no");
    }
    resigned
}
