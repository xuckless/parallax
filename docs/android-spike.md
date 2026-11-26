# Android spike

Phase 1 spike ([ROADMAP.md](../ROADMAP.md), "Spike"): throwaway code, kept only as this write-up. Result, on the
Android emulator (API 35, x86_64): **GPUI drew a box from the Android main thread through `gpui_wgpu`, and a tap
changed its colour**, on both Vulkan and GL. Rules it was checked against: [AGENTS.md](../AGENTS.md) (Android glue,
Android threading, Renderer). Pinned upstream: `gpui-pre` 0.3.14, wgpu 30.0.2.

## What was tried

- **Java (47 lines):** one `Activity` with a `SurfaceView`. JNI calls: `nativeInit` (from `onCreate`),
  `nativeSurfaceChanged(Surface, w, h, density)`, `nativeFrame` (from a `Choreographer` frame callback, every vsync),
  `nativeTouch(action, pointerId, x, y)` (from `OnTouchListener`). No Gradle: `javac` + `d8` + `aapt2` + `apksigner`.
- **Rust (~650 lines, one `cdylib`):** `PlatformDispatcher`, `Platform`, `PlatformWindow`, `PlatformDisplay`, all on
  `gpui` + `gpui_wgpu` with `default-features = false`. `gpui` and `gpui_wgpu` build for `x86_64-linux-android`
  unchanged; only the NDK linker is needed in `.cargo/config.toml`.
- The app is `Application::run_embedded` ([app.rs](../upstream/gpui-pre/src/app.rs#L340)) with one view: a 200 px
  `div` with `.on_click` cycling three colours. Logged with `android_logger`, checked with `adb logcat` and `screencap`.

## Answers

1. **Main thread: ALooper fd callback works; no Java `Handler` needed.** `nativeInit` (main thread) creates an
   `eventfd` and registers it with [`ALooper_addFd`](https://developer.android.com/ndk/reference/group/looper)
   on `ALooper_forThread()` (returned 1). `dispatch_on_main_thread` pushes to a queue and writes the eventfd; the
   callback drains the queue. Log: foreground task, `background.spawn(..).await` and a 250 ms `timer` all resumed on
   `ThreadId(1)`. [`PlatformDispatcher`](../upstream/gpui-pre/src/platform.rs#L1584) needs only `is_main_thread`,
   `dispatch` (background pool), `dispatch_on_main_thread`, `dispatch_after`, `spawn_realtime`; the rest have defaults.
   `Platform::run` just calls the launch callback and returns: the Looper owns the loop (`run_embedded`).
   Snag: `PriorityQueueSender/Receiver` (used by [the Linux dispatcher](../upstream/gpui-pre-linux/src/linux/dispatcher.rs))
   are exported only on windows/linux/wasm ([gpui.rs:112](../upstream/gpui-pre/src/gpui.rs#L112)); the spike used `mpsc`.
2. **Surface needed.** [`Platform`](../upstream/gpui-pre/src/platform.rs#L394): essential are the executors,
   `text_system` (`CosmicTextSystem::new_without_system_fonts`), `run`, `open_window`, `primary_display`/`displays`,
   `keyboard_layout`/`keyboard_mapper`; ~45 other required methods were one-line stubs. [`PlatformWindow`](../upstream/gpui-pre/src/platform.rs#L1283):
   essential are `bounds`/`content_size`/`scale_factor`, `on_request_frame`, `on_input`, `draw`, `sprite_atlas`,
   the input-handler pair, raw window/display handles; ~30 others were stubs. Estimate for the real backend, excluding
   the ported Java and text input: dispatcher ~250 lines, platform ~500, window ~700, display ~80.
3. **Rendering: works, no glue patch.** `WgpuRenderer::new` ([wgpu_renderer.rs:297](../upstream/gpui-pre-wgpu/src/wgpu_renderer.rs#L297))
   takes a `raw-window-handle` 0.6 `AndroidNdkWindowHandle` from `ANativeWindow_fromSurface`; display handle `android()`.
   [`WgpuContext::instance`](../upstream/gpui-pre-wgpu/src/wgpu_context.rs#L492) asks for `VULKAN | GL` and found
   two adapters: **Vulkan picked** (host GPU through the emulator's Vulkan passthrough). With a GL-only instance seeding
   the shared `GpuContext`, **GLES 3.1 also drew** (no dual-source blending, so no subpixel text). Emulator
   `-gpu host` worked; `-gpu swiftshader_indirect` crashed the emulator itself (exit 139) during boot.
   Renderer creation took ~0.5 s on the main thread (Choreographer "Skipped 39 frames").
4. **Touch: yes.** `PlatformInput::Touch(TouchEvent)` from `MotionEvent` (pixels ÷ density) became a
   `ClickEvent::Mouse` through upstream's tap→mouse emulation, and `on_click` changed the colour (before/after
   screenshots). No Parallax gesture code needed for a tap.
5. **Upstream Android: not active.** The `target_os = "android"` block in [gpui's Cargo.toml](../upstream/gpui-pre/Cargo.toml)
   only adds `libc`, used by test/profiler code. The one Android PR, zed#61057 (community, July 2026), was closed
   unreviewed in Aug 2026 as stale and too large; a maintainer pointed to `gpui-ce`. `gh search issues "gpui android"`
   finds nothing open. Zed merged a smaller "build gpui against ohos" PR (#64579), so small cfg patches are accepted.
   `gpui_wgpu` already has `unconfigure_surface`/`replace_surface`, documented for Android's window teardown.
6. **Plan changes:** see below.

Not done: surface loss. After Home → reopen, Android gave a new `ANativeWindow`; the spike kept the old one and
EGL failed `swap_buffers` ("not a valid surface"). [`replace_surface`](../upstream/gpui-pre-wgpu/src/wgpu_renderer.rs#L2097) is the fix.

## Recommended Phase 1 changes

- Close the open question "ALooper fd vs Java `Handler`": **ALooper fd + eventfd**, no JNI per task.
- Skeleton: start GPUI with `Application::run_embedded`; the Activity owns the loop and `Choreographer` drives frames.
- Move "surface lifecycle as one enum" from Full Android into the Skeleton: Home → reopen breaks without it.
- Add a small patch: export `PriorityQueueSender/Receiver` on Android (cfg only), so the dispatcher can reuse them.
- Renderer creation (~0.5 s) blocks the main thread; decide where it runs before writing the window (lead's call).
- `targetSdk 35` forces edge-to-edge: the surface was the full 1080×2400 under the action bar, so insets matter in 0.1.1.
- Close "Is upstream adding Android?": no; keep the plan, file only small cfg patches.
- CI/emulator: use `-gpu host` (or `-gpu auto` headless); SwiftShader crashed here.

## Reproduce the emulator setup

```sh
~/Android/Sdk/emulator/emulator -list-avds          # Pixel_7_API35 (system-images;android-35;google_apis;x86_64)
ls -l /dev/kvm                                       # must be rw for the user
~/Android/Sdk/emulator/emulator -avd Pixel_7_API35 -no-window -no-audio -no-snapshot -no-boot-anim -gpu host &
adb wait-for-device; adb shell getprop sys.boot_completed   # 1 when booted
adb install -r spike.apk && adb shell am start -n dev.parallax.spike/.SpikeActivity
adb shell input tap 540 1200; adb exec-out screencap -p > shot.png; adb emu kill
```

Toolchain: NDK 27.1, build-tools 35.0.1, `android-35` platform, a JDK with `javac` (the system one is a JRE only).
