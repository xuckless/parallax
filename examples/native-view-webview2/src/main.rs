//! A WebView2 page as a `NativeView` on Windows (docs/native-view-windows.md): WebView2's visual hosting
//! (`CreateCoreWebView2CompositionController`) renders the page into a visual the app makes on GPUI's own
//! DirectComposition device (`gpui_windows::composition_device`), shown through GPUI's hole with an orange div over its
//! corner. The view element's listeners send the mouse and wheel to the page (`SendMouseInput`); a click on the page
//! gives it the keyboard focus (`MoveFocus`), a click elsewhere takes it back. GPUI never renders for the page's frames.
//!
//! It logs what scripts/windows-native-view-checks.sh asserts: the view's placement in device pixels, what the page
//! posts back (`WebMessageReceived`: its input events and its animation frames per second) and GPUI frames per second.

#[cfg(target_os = "windows")]
mod app;
#[cfg(target_os = "windows")]
mod page;

#[cfg(target_os = "windows")]
fn main() -> Result<(), parallax::parallax_platform::Error> {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("warn,native_view_webview2=info,gpui_windows=info"),
    )
    .init();
    parallax::run(app::open_window)
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("native-view-webview2: Windows only");
}
