# Changelog

All notable changes to the published crates (`parallax`, `parallax-platform`, `parallax-android`,
`cargo-parallax`). Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/) (pre-1.0: a minor bump may break). Release lines: [ROADMAP.md](ROADMAP.md#releases).

## [0.1.2] - 2026-11-26

Wave 8: Android keys, fonts and lifecycle. Built on `gpui-pre` 0.3.14. Verified on the Android emulator (API 28 and 35
images; no real-device testing yet).

### Added

- `parallax-android`: hardware keys through Flutter's ported `KeyboardManager` and `KeyEmbedderResponder` (KeyDown,
  KeyUp, modifiers, caps lock), Android key codes → GPUI keys as one table, the back button as upstream's back gesture
  (cancelled presses cancel).
- `parallax-android`: Android system fonts from `fonts.xml`, with per-language fallback (regional Chinese, Japanese and
  Korean glyphs) and colour emoji where the system ships the CBDT emoji font.
- `parallax-android`: Activity lifecycle mapped to GPUI's lifecycle, active, visibility and memory-warning hooks; the
  window follows the most recently started Activity; GPU device-loss recovery with backoff through the backend table.
- `cargo-parallax`: `[app] icon` renders an SVG to Android launcher icons (densities + adaptive icon); generated
  Activities use `singleTop`.
- JVM tests for the ported Java (Robolectric), starting with Flutter's `AndroidTouchProcessorTest`.

### Known limits

- Emoji are blank on Android images that ship only the COLRv1 emoji font (Android 15 emulator); Android-drawn emoji
  are planned for 0.1.3.
- No text input yet (keys don't insert text). One GPUI window per process: in split-screen only the most recently
  started Activity draws. Locale changes after start aren't followed yet.

## [0.1.1] - 2026-11-25

The Phase 1 skeleton: one GPUI app on desktop, web and Android. Built on `gpui-pre` 0.3.14 (unpatched).
Verified on the Android emulator (no real-device testing yet).

### Added

- `parallax-android`: GPUI on the Android main Looper (`ALooper_addFd` + eventfd dispatcher, worker pool, timer
  thread); `gpui_wgpu` on Vulkan with a GL fallback, from one backend table; surface lifecycle as one state machine
  (none, ready, lost) with surface and device-loss recovery; system bar, cutout and IME insets; touch through
  Flutter's ported `AndroidTouchProcessor` and `MotionEventTracker` into upstream `TouchEvent`s; `android_main!`.
- `parallax-platform`: one target → platform table (desktop, web and iOS upstream; Android → `parallax-android`).
- `parallax`: facade re-exporting `gpui`, `parallax-platform` and `parallax-android`.
- `cargo-parallax`: `cargo parallax run android` builds the app, generates a Gradle project from `parallax.toml`,
  installs and launches the APK.
- `hello` example (in the repository): a box that changes colour on tap, on desktop, web and Android.

### Known limits

- 53 Android platform methods are stubs (lifecycle, keys, clipboard, appearance, system services); see
  [docs/android.md](docs/android.md#stubs-not-yet-on-android).
- No text input or hardware keys on Android yet; no system fonts there yet.
- The desktop tap → click in `hello` is checked by hand, not by a script.
- `cargo parallax run android --release` still builds a debug-signed APK.
- iOS builds in CI only; not run.

[0.1.1]: https://github.com/xuckless/parallax/releases/tag/v0.1.1
