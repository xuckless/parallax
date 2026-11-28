//! An OS-native view under a GPUI window, on every target (docs/examples-native-view.md): the view draws its own
//! frames, GPUI shows it through a hole, and a GPUI div covers part of it. One GPUI side (demo.rs) for every target;
//! per target only its look (data) and its `make_view` rows, the ways it makes its views:
//!
//! | Target | Rows (`view` option) | The view |
//! |---|---|---|
//! | Wayland | `wayland` | the app's `wl_surface`, a new shm buffer about 60 times a second from its own thread |
//! | Windows | `swap-chain`, `webview2` | a composition swap chain presented every vsync; a WebView2 page |
//! | macOS, iOS | `apple` | an `NSView` / `UIView` recoloured on a 60 Hz timer (iOS: and a `UITextField`) |
//! | Android | `control`, `view`, `hosted` | a `SurfaceControl`, `SurfaceView` or `in_surface` one, filled from its own thread, and a WebView (or, `stack`, two tiles) |
//! | Web | `dom` | two iframes, an animated canvas and a form, made by index.html |
//!
//! Once a second it logs `native-view: gpui frames/s N, view frames/s M` (N is 0 while only the views change), and
//! every pointer event over a view as `native-view: <view> pointer <kind> at (x, y)`, relative to the view.

pub mod demo;

#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_family = "wasm")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

use demo::{Demo, Options, Target};
use parallax::gpui::{
    App, Bounds, WindowBackgroundAppearance, WindowBounds, WindowOptions, prelude::*, px, size,
};

/// This target's look and `make_view` rows; `None` where native views don't exist.
#[cfg(any(target_os = "macos", target_os = "ios"))]
const TARGET: Option<&Target> = Some(&apple::TARGET);
#[cfg(target_os = "android")]
const TARGET: Option<&Target> = Some(&android::TARGET);
#[cfg(target_os = "linux")]
const TARGET: Option<&Target> = Some(&wayland::TARGET);
#[cfg(target_family = "wasm")]
const TARGET: Option<&Target> = Some(&web::TARGET);
#[cfg(target_os = "windows")]
const TARGET: Option<&Target> = Some(&windows::TARGET);
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "linux",
    target_family = "wasm",
    target_os = "windows"
)))]
const TARGET: Option<&Target> = None;

/// Desktop and iOS options: `--name [value]` arguments (docs/examples-native-view.md).
fn argument(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let at = args.iter().position(|arg| *arg == format!("--{name}"))?;
    let value = args.get(at + 1).filter(|value| !value.starts_with("--"));
    Some(value.cloned().unwrap_or_default())
}

pub fn launch(_: &mut App) {}

pub fn open_window(cx: &mut App) {
    let Some(target) = TARGET else {
        log::error!("native-view: no native views on this target");
        return cx.quit();
    };
    // Read before the window opens: whether it is see-through. Android and the web read theirs below.
    let arguments = Options::parse(argument);
    let options = WindowOptions {
        window_bounds: target.look.window.map(|[width, height]| {
            WindowBounds::Windowed(Bounds::centered(None, size(px(width), px(height)), cx))
        }),
        window_background: if arguments.clipped {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        },
        ..Default::default()
    };
    let opened = cx.open_window(options, move |window, cx| {
        #[cfg(target_os = "android")]
        let arguments = android::options(window).unwrap_or(arguments);
        #[cfg(target_family = "wasm")]
        let arguments = web::options();
        cx.new(|cx| Demo::new(target, arguments, window, cx))
    });
    match opened {
        Ok(_) => log::info!("native-view: window open"),
        Err(error) => {
            log::error!("native-view: cannot open the window: {error:#}");
            cx.quit();
        }
    }
    cx.activate(true);
}

/// Desktop and iOS entry point (main.rs).
#[cfg(not(any(target_os = "android", target_family = "wasm")))]
pub fn main() -> Result<(), parallax::parallax_platform::Error> {
    // The renderers' lines: the alpha mode each chose (wgpu, gpui_windows, gpui_apple), and on Windows one line per
    // GPUI frame copied into its composition surface, which the checks count.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(
        "warn,native_view=info,hello=info,gpui_wgpu::wgpu_renderer=info,gpui_windows=info,\
         gpui_windows::directx_renderer::native_view_surface=debug,gpui_apple=info",
    ))
    .init();
    parallax::run(|cx| {
        launch(cx);
        open_window(cx);
    })
}

#[cfg(target_os = "android")]
parallax::android_main!(launch, open_window);
