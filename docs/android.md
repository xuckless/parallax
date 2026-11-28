# Android backend

[`parallax-android`](../crates/android/src/lib.rs) runs GPUI on Android: the 0.1.1 skeleton of [ROADMAP.md](../ROADMAP.md) Phase 1. Rules: [AGENTS.md](../AGENTS.md) (Android glue, Android threading, Renderer). Background: [android-spike.md](android-spike.md).

## How it fits together

- **Activity → JNI → Platform.** [`ParallaxActivity`](../crates/android/java/io/github/xuckless/parallax/ParallaxActivity.java)
  reads `parallax.lib_name` from `<application>` meta-data, `System.loadLibrary`s the app, sets edge to edge and shows
  [`ParallaxSurfaceView`](../crates/android/java/io/github/xuckless/parallax/ParallaxSurfaceView.java). The app's
  `JNI_OnLoad` binds the view's static natives from one table (`natives` in [host.rs](../crates/android/src/host.rs)).
  On the first surface, `host.rs` builds the platform ([platform.rs](../crates/android/src/platform.rs)) and starts
  GPUI with `Application::run_embedded`; the app's `launch` runs, then its `open_window` opens that Activity's window
  ([window.rs](../crates/android/src/window.rs)). Each Activity has its own window: `launch` runs once per process,
  `open_window` once per Activity ([android-multi-window.md](android-multi-window.md)).
- **Main thread only.** All JNI calls, GPUI and the renderer run on the Android main thread. They find their state in
  one thread-local, `MainThreadHost` in `host.rs` (why: static JNI methods carry no user pointer), and each
  Activity's window by the id Java tags every call with.
- **Looper dispatcher.** [dispatcher.rs](../crates/android/src/dispatcher.rs): main-thread runnables go into GPUI's
  priority queue (`PriorityQueueSender`/`Receiver`, exported on Android by
  [patch 0001](../patches/0001-export-priority-queue-android.patch)); an eventfd registered with `ALooper_addFd` wakes the Looper, which drains it. Workers and one timer thread as on Linux.
- **Frames.** `Choreographer` calls `nativeFrame` every vsync between `onResume` and `onPause`; GPUI draws if dirty,
  then Java pulls GPUI's framework calls (`nativeFrameworkCalls`: text input, [android-text-input.md](android-text-input.md), and haptics, [haptics.md](haptics.md)) and runs them.
- **Surface state machine** (one per Activity's window). [surface.rs](../crates/android/src/surface.rs): `Surface::{None, Ready(window), Recovering, Lost}`,
  one `next(state, event) → (state, action)` for the `SurfaceHolder` callbacks, vsync frames and recovery outcomes.
  The shell applies the action: `replace_surface` on a new window (Home → back), `unconfigure_surface` before Android
  destroys it, device recovery after device loss. Tested row by row on the host (`cargo test -p parallax-android`).
- **Renderer table.** `Device::backends` ([backend.rs](../crates/android/src/backend.rs)): Vulkan then GL; GL first on
  virtual devices (`ro.hardware` `ranchu`, `goldfish`: the emulator, whose Vulkan drew black on API 33 and whose GL is
  16-bit, wgpu's EGL config; `cutf_cvm`, `vsoc`: Cuttlefish; `vbox86`: Genymotion), read once at startup.
  The first that configures the surface seeds the shared `GpuContext`, at startup and after device loss
  ([android-surface.md](android-surface.md#device-loss)). Made on the main thread ([renderer.rs](../crates/android/src/renderer.rs)).
- **Lifecycle.** Activity callbacks, focus and trim-memory go through one state machine
  ([lifecycle.rs](../crates/android/src/lifecycle.rs)) to upstream's `on_app_lifecycle`, `on_memory_warning`, window
  activity and visibility; `onDestroy` is `Disconnected`. Details: [android-lifecycle.md](android-lifecycle.md).
- **Insets.** The view sends `systemBars | displayCutout` and `ime` insets in pixels; GPUI reads them through
  `Window::insets()` and gets `on_insets_changed`. API 28–29 has no `ime` type: the view sends the system-window
  insets and the cutout, and a keyboard-sized bottom inset (Flutter's `guessBottomKeyboardInset`) as the `ime` one.
  The keyboard reaches those insets only with the manifest's `adjustResize` (Flutter's app template's).
- **Clipboard.** Plain text through the ported `PlatformPlugin` over JNI: [android-clipboard.md](android-clipboard.md).
- **Rotation** keeps the Activity (manifest `configChanges`, as Flutter); [a new surface](android-surface.md#resize). Android's default focus highlight is off: it greyed the whole frame out of touch mode.
- **Touch.** `onTouchEvent` → Flutter's `AndroidTouchProcessor` (ported Java) packs a pointer data packet →
  `nativeDispatchPointerDataPacket` → [pointer.rs](../crates/android/src/pointer.rs) decodes it into upstream
  `TouchEvent`s for `Callbacks::input`; upstream turns a tap into a click. Details: [android-touch.md](android-touch.md).
- **Native views.** The app's `View` or `SurfaceControl` under GPUI's surface, through the renderer's hole:
  [native-view-android.md](native-view-android.md).
- **Fonts.** At startup the platform reads `/system/etc/fonts.xml`, memory-maps the system fonts into upstream's
  `CosmicTextSystem` and orders CJK by the app's current locales. Details: [android-fonts.md](android-fonts.md).
- **Keys.** `dispatchKeyEvent` → Flutter's `KeyboardManager` and `KeyEmbedderResponder` (ported Java) →
  `nativeDispatchKeyData` → [keys.rs](../crates/android/src/keys.rs): key, modifier and back events. Details: [android-keys.md](android-keys.md).
- **OS settings.** Java's `AccessibilityInfoModule` (React Native's, ported) sends the animation scales;
  "Remove animations" becomes `App::reduce_motion` ([os-settings.md](os-settings.md)).
- **Text input.** Flutter's `TextInputPlugin` and `InputConnectionAdaptor` (ported Java) serve the IME; GPUI's `InputHandler` answers on the same thread
  ([text_input.rs](../crates/android/src/text_input.rs)), IME edits synchronously, GPUI's changes after each frame. Details: [android-text-input.md](android-text-input.md).

## The app entry point

An app is a `cdylib` crate depending on `parallax`. It writes two functions and one macro line:

```rust
fn launch(cx: &mut parallax::gpui::App) { /* bind keys, set globals */ }
fn open_window(cx: &mut parallax::gpui::App) { /* cx.open_window(...) */ }
parallax::android_main!(launch, open_window);
```

`android_main!` defines `JNI_OnLoad` on Android (logging to logcat, tag `parallax`; panics logged there too) and
nothing elsewhere. Both functions are required, so no Activity is left without a window by default. `launch` runs
once per process, when the first Activity's first surface exists (GPUI needs one to open a window); `open_window`
runs on every Activity's first surface, the first one's right after `launch`
([android-multi-window.md](android-multi-window.md)).
Apps that pick a platform themselves call `parallax_platform::current_platform`, which routes Android to
`parallax_android::current_platform` ([platform.md](platform.md)).

`Error` is public and exhaustive on 0.1.x, so variants are only added or deprecated (`AddFonts`, unused since patch 0002); it becomes `#[non_exhaustive]`, and deprecated variants go, on the next breaking line.

What GPUI asks of a platform that Android does not do yet: [android-stubs.md](android-stubs.md).

## Run it on the emulator

`cargo parallax run android --device <serial>` ([cli-android.md](cli-android.md)) on an emulator started with `-gpu host`; then `adb logcat -s parallax` shows the font and renderer load times, the renderer backend, surface events, insets and `parallax: first frame drawn on a WxH portrait|landscape surface` (once per surface, when its first scene reaches the screen).
