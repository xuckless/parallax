//! macOS and iOS entry point. Other targets have their own native view examples.

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn main() -> Result<(), parallax::parallax_platform::Error> {
    // `gpui_apple=info` logs the Metal layer's alpha mode each time a native view shows or goes.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,native_view_apple=info,gpui_apple=info"),
    )
    .init();
    parallax::run(apple::open_window)
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn main() {
    eprintln!("native-view-apple runs on macOS and iOS.");
}
