//! The WebView2 page: its environment and composition controller, made asynchronously on the main thread, rendering
//! into a visual on GPUI's DirectComposition device. WebView2's completion handlers run from GPUI's own message loop,
//! so the page's state is shared with GPUI's listeners through an `Rc`.

use std::{cell::RefCell, rc::Rc};

use webview2_com::{
    CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
        COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, CreateCoreWebView2EnvironmentWithOptions,
        ICoreWebView2CompositionController, ICoreWebView2Controller, ICoreWebView2Environment3,
        ICoreWebView2EnvironmentOptions,
    },
    WebMessageReceivedEventHandler, take_pwstr,
};
use windows::{
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::DirectComposition::IDCompositionVisual,
    },
    core::{Error, HSTRING, Interface, PCWSTR, PWSTR},
};

/// The page: a local HTML file, loaded as a string. It posts its input events and frame rate back.
const PAGE: &str = include_str!("page.html");

/// Where the page is in its life.
enum State {
    /// The environment and controller are on their way; the latest size, in device px, waits for them.
    Starting { size: [i32; 2] },
    Ready {
        composition: ICoreWebView2CompositionController,
        controller: ICoreWebView2Controller,
    },
}

#[derive(Clone)]
pub struct Page(Rc<RefCell<State>>);

impl Page {
    /// Starts WebView2 for `window`, rendering into `visual` (made on GPUI's DirectComposition device).
    pub fn start(window: HWND, visual: IDCompositionVisual) -> windows::core::Result<Self> {
        let page = Self(Rc::new(RefCell::new(State::Starting { size: [1, 1] })));
        let state = page.0.clone();
        let created = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |result, environment| {
                let environment: ICoreWebView2Environment3 = result
                    .and_then(|()| environment.ok_or_else(Error::empty)?.cast())
                    .inspect_err(|error| log::error!("webview: no environment: {error}"))?;
                let made = CreateCoreWebView2CompositionControllerCompletedHandler::create(
                    Box::new(move |result, composition| {
                        // WebView2 drops what a handler returns, so its errors are logged here.
                        let composition = result
                            .and_then(|()| composition.ok_or_else(Error::empty))
                            .and_then(|composition| {
                                show(&composition, &visual).map(|()| composition)
                            })
                            .inspect_err(|error| log::error!("webview: no page: {error}"))?;
                        let controller: ICoreWebView2Controller = composition.cast()?;
                        let [width, height] = match &*state.borrow() {
                            State::Starting { size } => *size,
                            State::Ready { .. } => return Ok(()),
                        };
                        // SAFETY: plain call on a live controller.
                        unsafe { controller.SetBounds(rect(width, height)) }?;
                        *state.borrow_mut() = State::Ready {
                            composition,
                            controller,
                        };
                        log::info!("webview: ready");
                        Ok(())
                    }),
                );
                // SAFETY: `window` is GPUI's live window, on this thread.
                unsafe { environment.CreateCoreWebView2CompositionController(window, &made) }
                    .inspect_err(|error| log::error!("webview: no controller: {error}"))
            },
        ));
        // SAFETY: null folders pick WebView2's defaults (the installed runtime, a data folder by the exe).
        unsafe {
            CreateCoreWebView2EnvironmentWithOptions(
                PCWSTR::null(),
                PCWSTR::null(),
                None::<&ICoreWebView2EnvironmentOptions>,
                &created,
            )
        }?;
        Ok(page)
    }

    /// Sizes the page to its element's laid-out size, in device px.
    pub fn resize(&self, width: i32, height: i32) {
        match &mut *self.0.borrow_mut() {
            State::Starting { size } => *size = [width, height],
            // SAFETY: plain call on a live controller.
            State::Ready { controller, .. } => {
                if let Err(error) = unsafe { controller.SetBounds(rect(width, height)) } {
                    log::error!("webview: cannot resize: {error}");
                }
            }
        }
    }

    /// Sends a mouse event at `at`, device px in the page, as the page's own window would have had it.
    pub fn mouse(
        &self,
        kind: COREWEBVIEW2_MOUSE_EVENT_KIND,
        keys: COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
        data: i32,
        [x, y]: [i32; 2],
    ) {
        let State::Ready { composition, .. } = &*self.0.borrow() else {
            return;
        };
        // SAFETY: plain call on a live controller; mouseData is a DWORD holding a signed wheel delta.
        if let Err(error) =
            unsafe { composition.SendMouseInput(kind, keys, data as u32, POINT { x, y }) }
        {
            log::error!("webview: cannot send {kind:?}: {error}");
        }
    }

    /// Gives the page the keyboard: WebView2 takes the window's focus and reads the keys itself.
    pub fn focus(&self) {
        let State::Ready { controller, .. } = &*self.0.borrow() else {
            return;
        };
        // SAFETY: plain call on a live controller.
        if let Err(error) =
            unsafe { controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) }
        {
            log::error!("webview: cannot focus: {error}");
        }
    }
}

/// Renders the controller's page into `visual`, logs what it posts, and loads it.
fn show(
    composition: &ICoreWebView2CompositionController,
    visual: &IDCompositionVisual,
) -> windows::core::Result<()> {
    let controller: ICoreWebView2Controller = composition.cast()?;
    let messages = WebMessageReceivedEventHandler::create(Box::new(|_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        let mut message = PWSTR::null();
        // SAFETY: `message` outlives the call; take_pwstr frees what it gets.
        unsafe { args.TryGetWebMessageAsString(&mut message) }?;
        log::info!("page: {}", take_pwstr(message));
        Ok(())
    }));
    let mut token = 0;
    // SAFETY: plain calls on a live controller and its webview; the visual is on the device of GPUI's tree.
    unsafe {
        composition.SetRootVisualTarget(visual)?;
        controller.SetIsVisible(true)?;
        let webview = controller.CoreWebView2()?;
        webview.add_WebMessageReceived(&messages, &mut token)?;
        webview.NavigateToString(&HSTRING::from(PAGE))?;
    }
    Ok(())
}

fn rect(width: i32, height: i32) -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    }
}
