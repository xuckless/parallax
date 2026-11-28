//! `--view webview2` (docs/native-view-webview2.md): a WebView2 page as the native view. WebView2's visual hosting
//! (`CreateCoreWebView2CompositionController`) renders it into a visual the app makes on GPUI's own DirectComposition
//! device (`gpui_windows::composition_device`). The view's element sends the mouse and wheel on to the page
//! (`SendMouseInput`); a press on the page gives it the keyboard (`MoveFocus`), a press elsewhere takes it back. The
//! page posts its input events back (logged as `page: …`) and its frame count (the view frames of the log line).

use std::{cell::Cell, ptr::NonNull, rc::Rc};

use parallax::gpui::{App, Context, Modifiers, MouseButton, NativeView, ScrollDelta, Window};
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

use super::page::Page;
use crate::demo::{Demo, Forwarded, Made, Options, Pointer, Shown};

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

pub fn make_view(_: &Options, window: &mut Window, cx: &mut Context<Demo>) -> Result<Made, String> {
    let handle = window.window_handle().map_err(|error| error.to_string())?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("not a Win32 window".into());
    };
    let hwnd = HWND(handle.hwnd.get() as _);
    // The page's frames so far: it posts them, its view is made again after a device loss.
    let frames = Rc::new(Cell::new(0));
    let shown = attach(hwnd, &frames, window, cx)?;
    // A lost GPU device gives GPUI a new DirectComposition device, which the page's visual can't join: the page
    // is made again there. The callback runs inside GPUI's recovery, so the work waits for the app's next turn.
    let (this, async_cx, gpui_window) = (
        cx.entity().downgrade(),
        cx.to_async(),
        Window::window_handle(window),
    );
    let reattached = frames.clone();
    let watched = gpui_windows::on_composition_device_changed(window, move || {
        let (this, frames) = (this.clone(), reattached.clone());
        async_cx
            .spawn(async move |cx| {
                gpui_window.update(cx, |_, window, cx| {
                    log::info!("native-view: the DirectComposition device changed, attaching the page again");
                    match attach(hwnd, &frames, window, cx) {
                        Ok(shown) => {
                            this.update(cx, |demo, cx| demo.replace(shown, cx)).ok();
                        }
                        Err(error) => log::error!("native-view: cannot attach the page again: {error}"),
                    }
                })
            })
            .detach();
    });
    if !watched {
        return Err("GPUI shows no DirectComposition tree".into());
    }
    let mut made = Made::new(vec![shown], move || frames.get());
    made.on_press_elsewhere = Some(Rc::new(move || {
        // SAFETY: GPUI's own live window, on its thread.
        if let Err(error) = unsafe { SetFocus(Some(hwnd)) } {
            log::error!("native-view: cannot take the keyboard back: {error}");
        }
    }));
    Ok(made)
}

/// Makes a visual on GPUI's DirectComposition device, the page rendering into it, and its native view.
fn attach(
    hwnd: HWND,
    frames: &Rc<Cell<u64>>,
    window: &mut Window,
    cx: &mut App,
) -> Result<Shown, String> {
    // The page's visual must be on GPUI's own device, the one its window's visual tree is on.
    let device =
        gpui_windows::composition_device(window).ok_or("GPUI shows no DirectComposition tree")?;
    // SAFETY: plain creation on a live device.
    let visual = unsafe { device.CreateVisual() }.map_err(|error| error.to_string())?;
    let raw = NonNull::new(visual.as_raw()).ok_or("null visual")?;
    // SAFETY: `raw` is a live IDCompositionVisual on GPUI's device; GPUI takes a reference of its own.
    let view = unsafe { NativeView::new(raw, window, cx) }.map_err(|error| format!("{error:#}"))?;
    let page =
        Page::start(hwnd, device, visual, frames.clone()).map_err(|error| error.to_string())?;
    let mut shown = Shown::new("view", view, ());
    let placed = page.clone();
    shown.place = Some(Rc::new(move |edges| placed.place(edges)));
    shown.forward = Some(Rc::new(move |forwarded: Forwarded| {
        let Forwarded {
            pointer,
            at,
            scale,
            modifiers,
            held,
        } = forwarded;
        // WebView2's positions are device px from the page's top left.
        let at = [at.x, at.y].map(|logical| (f32::from(logical) * scale).round() as i32);
        let row = |button| BUTTONS.iter().find(|(row, ..)| *row == button);
        match pointer {
            Pointer::Down(button) => {
                if let Some((_, down, ..)) = row(button) {
                    page.mouse(*down, virtual_keys(modifiers, Some(button)), 0, at);
                }
                // The page takes the keys.
                page.focus();
            }
            Pointer::Up(button) => {
                if let Some((_, _, up, _)) = row(button) {
                    page.mouse(*up, virtual_keys(modifiers, None), 0, at);
                }
            }
            Pointer::Move => {
                let keys = virtual_keys(modifiers, held);
                page.mouse(COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE, keys, 0, at)
            }
            Pointer::Scroll(delta) => {
                // Positive away from the user in both.
                let notches = match delta {
                    ScrollDelta::Lines(lines) => lines.y / LINES_PER_NOTCH,
                    ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / PIXELS_PER_NOTCH,
                };
                let data = (notches * WHEEL_DELTA).round() as i32;
                let keys = virtual_keys(modifiers, None);
                page.mouse(COREWEBVIEW2_MOUSE_EVENT_KIND_WHEEL, keys, data, at);
            }
            Pointer::Leave => {
                let keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_NONE;
                page.mouse(COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE, keys, 0, [0, 0]);
            }
        }
    }));
    Ok(shown)
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
