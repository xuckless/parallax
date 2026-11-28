//! A `NativeView` on Windows (docs/native-view-windows.md): the app's own composition swap chain, on the app's own
//! D3D11 device and thread, presented every vsync with a new colour, shown under GPUI's content through
//! DirectComposition, with an orange div painted over its top-left corner. GPUI never renders for the view's frames.
//!
//! It logs what scripts/windows-native-view-checks.sh asserts: the view's placement in device pixels, the input its
//! element hears (positions in the view), GPUI frames and view presents per second, and the alpha mode GPUI chose
//! (gpui_windows logs that). The "move" button slides the view for two seconds, "resize" toggles its size.

#[cfg(target_os = "windows")]
mod app;
#[cfg(target_os = "windows")]
mod content;

#[cfg(target_os = "windows")]
fn main() -> Result<(), parallax::parallax_platform::Error> {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("warn,native_view_windows=info,gpui_windows=info"),
    )
    .init();
    parallax::run(app::open_window)
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("native-view-windows: Windows only");
}
