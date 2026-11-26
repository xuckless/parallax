# Android backend

[`parallax-android`](../crates/android/src/lib.rs) runs GPUI on Android: the 0.1.1 skeleton of
[ROADMAP.md](../ROADMAP.md) Phase 1. Rules: [AGENTS.md](../AGENTS.md) (Android glue, Android threading, Renderer). Background: [android-spike.md](android-spike.md).

## How it fits together

- **Activity → JNI → Platform.** [`ParallaxActivity`](../crates/android/java/io/github/xuckless/parallax/ParallaxActivity.java)
  reads `parallax.lib_name` from `<application>` meta-data, `System.loadLibrary`s the app, sets edge to edge and shows
  [`ParallaxSurfaceView`](../crates/android/java/io/github/xuckless/parallax/ParallaxSurfaceView.java). The app's
  `JNI_OnLoad` binds the view's static natives from one table (`natives` in [host.rs](../crates/android/src/host.rs)).
  On the first surface, `host.rs` builds the platform ([platform.rs](../crates/android/src/platform.rs)) and starts
  GPUI with `Application::run_embedded`; the app's launch function opens the one window ([window.rs](../crates/android/src/window.rs)).
- **Main thread only.** All JNI calls, GPUI and the renderer run on the Android main thread. They find their state in
  one thread-local, `MainThreadHost` in `host.rs` (why: static JNI methods carry no user pointer).
- **Looper dispatcher.** [dispatcher.rs](../crates/android/src/dispatcher.rs): main-thread runnables go into GPUI's
  priority queue (`PriorityQueueSender`/`Receiver`, exported on Android by
  [patch 0001](../patches/0001-export-priority-queue-android.patch)); an eventfd registered with `ALooper_addFd` wakes the Looper, which drains it. Workers and one timer thread as on Linux.
- **Frames.** `Choreographer` calls `nativeFrame` every vsync between `onResume` and `onPause`; GPUI draws if dirty,
  then Java runs GPUI's text-input calls (`nativeTextInputCalls`, [android-text-input.md](android-text-input.md)).
- **Surface state machine.** [surface.rs](../crates/android/src/surface.rs): `Surface::{None, Ready(window), Recovering, Lost}`,
  one `next(state, event) → (state, action)` for the `SurfaceHolder` callbacks, vsync frames and recovery outcomes.
  The shell applies the action: `replace_surface` on a new window (Home → back), `unconfigure_surface` before Android
  destroys it, device recovery after device loss. Tested row by row on the host (`cargo test -p parallax-android`).
- **Renderer table.** `Device::backends` ([backend.rs](../crates/android/src/backend.rs)): Vulkan then GL; GL first on
  the emulator (`ro.hardware` `ranchu`, `goldfish`: its Vulkan drew black on API 33; its GL is 16-bit, wgpu's EGL
  config). The first that configures the surface seeds the shared `GpuContext`, at startup and after device loss
  ([android-surface.md](android-surface.md#device-loss)). Made on the main thread ([renderer.rs](../crates/android/src/renderer.rs)).
- **Lifecycle.** Activity callbacks, focus and trim-memory go through one state machine
  ([lifecycle.rs](../crates/android/src/lifecycle.rs)) to upstream's `on_app_lifecycle`, `on_memory_warning`, window
  activity and visibility; `onDestroy` is `Disconnected`. Details: [android-lifecycle.md](android-lifecycle.md).
- **Insets.** The view sends `systemBars | displayCutout` and `ime` insets (API 28–29: system-window insets and the
  cutout) in pixels; GPUI reads them through `Window::insets()` and gets `on_insets_changed`.
- **Rotation** keeps the Activity (manifest `configChanges`, as Flutter); [a new surface](android-surface.md#resize).
- **No focus highlight.** Android's default one greyed the whole frame out of touch mode (white read 224 of 255).
- **Touch.** `onTouchEvent` → Flutter's `AndroidTouchProcessor` (ported Java) packs a pointer data packet →
  `nativeDispatchPointerDataPacket` → [pointer.rs](../crates/android/src/pointer.rs) decodes it into upstream
  `TouchEvent`s for `Callbacks::input`; upstream turns a tap into a click. Details: [android-touch.md](android-touch.md).
- **Fonts.** At startup the platform reads `/system/etc/fonts.xml`, memory-maps the system fonts into upstream's
  `CosmicTextSystem` and orders CJK by the app's current locales. Details: [android-fonts.md](android-fonts.md).
- **Keys.** `dispatchKeyEvent` → Flutter's `KeyboardManager` and `KeyEmbedderResponder` (ported Java) →
  `nativeDispatchKeyData` → [keys.rs](../crates/android/src/keys.rs): key, modifier and back events. Details: [android-keys.md](android-keys.md).
- **Text input.** Flutter's `TextInputPlugin` and `InputConnectionAdaptor` (ported Java) serve the IME; GPUI's `InputHandler` answers on the same thread
  ([text_input.rs](../crates/android/src/text_input.rs)), IME edits synchronously, GPUI's changes after each frame. Details: [android-text-input.md](android-text-input.md).

## The app entry point

An app is a `cdylib` crate depending on `parallax`. It writes its launch function and one macro line:

```rust
fn launch(cx: &mut parallax::gpui::App) { /* cx.open_window(...) */ }
parallax::android_main!(launch);
```

`android_main!` defines `JNI_OnLoad` on Android (logging to logcat, tag `parallax`; panics logged there too) and
nothing elsewhere. `launch` runs when the first surface exists, because GPUI needs one to open its window.
Apps that pick a platform themselves call `parallax_platform::current_platform`, which routes Android to
`parallax_android::current_platform` ([platform.md](platform.md)).

`Error` is public and exhaustive on 0.1.x, so variants are only added or deprecated (`AddFonts`, unused since
patch 0002); it becomes `#[non_exhaustive]`, and deprecated variants go, on the next breaking line.

## Stubs (not yet on Android)

Each is marked `Not yet on Android` in the source with where it will come from: a no-op, a fixed value, or a typed
`Error::NotYet`. 44 in all:

| Area | Stubs | Comes with |
|---|---|---|
| Lifecycle | `quit`, `on_reopen`, `on_system_sleep/wake`, `on_should_close`, `on_close` | `Activity.finish()`, back (keys), screen off/on |
| Input | `is_hovered`, `keyboard_layout`, `keyboard_mapper`, `on_keyboard_layout_change` | hover enter and exit, keyboard layout |
| Appearance | `appearance`, `window_appearance`, `on_appearance_changed`, `set_background_appearance`, `toggle_fullscreen`, corner radii | dark mode, immersive mode |
| System services | clipboard (2), `open_url`, `on_open_urls`, `register_url_scheme`, `open_with_system`, `reveal_path`, `hide`, file prompts (3), credentials (3), `prevent_idle_sleep`, thermal (2), cursor (3), `app_path`, `path_for_auxiliary_executable` | JNI services, later |
| Display | `on_displays_changed`, `on_display_changed`, display `uuid`, window raw handle | multi-display |
| Other | `spawn_realtime` without realtime priority; `prompt` (GPUI draws its own) | later |

Methods with no Android meaning (`restart`, `minimize`, `zoom`, `set_title`, menus, dock, `on_moved`, window
controls, `on_quit`: Android announces no process death) are no-ops with a one-line reason, not stubs. Density changes without a resize are not tracked yet.

## Run it on the emulator

`cargo parallax run android --device <serial>` ([cli.md](cli.md)) on an emulator started with `-gpu host`; then `adb logcat -s parallax` shows the font and renderer load times, the renderer backend, surface events and insets.
