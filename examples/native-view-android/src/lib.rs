//! Native views on Android (docs/native-view-android.md): a surface the app fills with a new colour every frame from
//! its own thread, with a GPUI div over part of it, and a WebView showing a local page, scrolled and tapped through
//! forwarded touches. Both sit under GPUI's surface and show through the holes GPUI cuts.
//!
//! The surface is a `SurfaceControl` child of GPUI's surface on API 29+, an ordinary `SurfaceView` on API 28 or
//! with `adb shell am start … --es surface view`. `--ez animate true` moves and resizes the surface's slot every GPUI
//! frame. Once a second logcat (tag `native-view-android`, through `parallax`) says
//! `gpui frames/s N, surface frames/s M`: N is 0 while only the views change. Pointer events over either view log
//! their position relative to the view; the div over the surface logs its own taps; the page logs its scrolls and
//! taps to the `chromium` tag. The underlay is magenta: any magenta on screen is a hole the view has not caught up
//! with.

#[cfg(target_os = "android")]
mod demo;
#[cfg(target_os = "android")]
mod java;

#[cfg(target_os = "android")]
parallax::android_main!(demo::launch, demo::open_window);
