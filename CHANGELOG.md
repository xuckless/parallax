# Changelog

All notable changes to the published crates (`parallax`, `parallax-platform`, `parallax-android`,
`cargo-parallax`). Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/) (pre-1.0: a minor bump may break). Release lines: [ROADMAP.md](ROADMAP.md#releases).

## [Unreleased]

### Fixed

- macOS draws text: `parallax-platform` now turns on `gpui_platform`'s `font-kit` feature. 0.1.1 to 0.1.4 built
  without it, so `gpui_macos` used GPUI's `NoopTextSystem` and every macOS app drew no text (boxes only).
- The `hello` example's desktop binary logs to stderr (`env_logger`, `RUST_LOG` to change the level).

## [0.1.4] - 2026-11-26 — Gate 1

Wave 10: **Gate 1 passes**: Android text input, mouse, autofill and locales, checked with Gboard on API 28, 33 and 35
emulators ([docs/gate-1.md](docs/gate-1.md); no real-device testing yet). Built on `gpui-pre` 0.3.14 plus Parallax's
patches, published as the GPUI fork `parallax-gpui*` 0.3.14-parallax.7. Checked by unit, JVM and patch tests and an
API 35 emulator smoke test in CI.

### Added

- `BackGestureEvent::source` (`BackGestureSource::EdgeSwipe` or `Key`): Android's Back key and navigation-bar button
  are `Key`, with no pointer behind them. **Breaking:** every `BackGestureEvent` literal names its source; neither
  type has a `Default` any more, so a literal can't pick one silently.
- `TextSystem::invalidate_fonts`: drops cached font ids and line layouts without adding fonts, for a platform text
  system whose font matching changed (Android's locale order uses it).
- `PlatformWindow::end_composition` (patch 0010, no-op by default): GPUI calls it after ending a field's composition
  on focus loss, so a platform's native IME ends its side too (macOS `discardMarkedText`, the web IME mirror's blur
  and refocus).
- `Window::leave_touch_to_gestures` is public: a custom element that handles taps or scrolls itself keeps an ancestor's
  `on_touch` from taking the touch, as a div with `on_click` or `on_scroll` already does.
- `parallax-android`: a mouse or trackpad is a real mouse: clicks with click count, right and middle click, hover
  moves, wheel and two-finger trackpad scrolling (no longer synthesized taps).
- `parallax-android`: autofill. `TextInputConfiguration`'s autofill hint reaches Android's autofill service, which can
  offer and fill the focused field.
- `parallax-android`: runtime locale changes. Changing the app's or system language reorders the font fallback (for
  example Chinese vs Japanese glyphs for the same Han characters) without a restart.
- CI runs an Android emulator smoke test (API 35): `examples/hello` launches, takes taps and typing, rotates,
  backgrounds and resumes, waiting for the first frame on each new surface.

### Changed

- `parallax-android`: emulators try GL before Vulkan (the API 33 emulator's Vulkan drew black, then garbled glyphs);
  devices still try Vulkan first.
- A gesture recognized from touches (tap, pan, long press) reports the device of its own touch in
  `Window::pointer_source()`, not of the latest touch, so a pen and a finger down together no longer mislabel each
  other. A fling's momentum keeps the device that flung it, its closing scroll included when another touch catches
  it.

### Fixed

- `parallax-android`: mouse buttons pressed or released out of order (right up before left, say) send a `MouseDown`
  or `MouseUp` for the button that changed, not the wrong one.
- `parallax-android`: autofill's anchor rect follows the field when it scrolls or resizes under the keyboard.
- `parallax-android`: an email, URL or search field keeps its keyboard layout when suggestions are off (GPUI's
  default); a secure one is a plain password field.
- `parallax-android`: after a rotation on API 28 the app redraws at the new size (it kept the old frame until the
  next input); a resize now makes a new surface.
- `parallax-android`: colours are no longer dimmed: Android's default focus highlight greyed the whole view.
- `parallax-android`: surface lifecycle: a failed surface attach retries through device recovery instead of leaving a
  blank window, each start or recovery builds one surface, and the first frame is logged on every new surface.
- `examples/hello` on the web: it panicked at the first draw (a browser gives GPUI no system fonts); it now embeds
  Noto Sans and Noto Sans Arabic. An app on the web must load its own fonts with `TextSystem::add_fonts`.
- A field's composition (an underlined IME range) ends when focus leaves it (patch 0010), so the next IME session
  doesn't get a stale range back and replace typed text. The native IME is told too, on macOS and the web, so moving
  focus from code mid-composition no longer types the composed text a second time.

### Known limits

- Android mouse: no hover enter/exit (`is_hovered` stays false), no pointer icons. Android autofill sees one field at
  a time and never offers to save (GPUI has no API to end an autofill context).
- Gboard's select mode (Shift + arrows) moves the caret instead of extending the selection.
- macOS: patch 0010's `discardMarkedText` is not yet checked with a real input method (web is checked in headless
  Chromium).
- Emulators on GL get a 16-bit (RGB565) surface, so dark colours are slightly quantized there; devices are not
  affected.
- API 28: the first show of many new emoji at once stalls a few frames (100 new emoji ≈ 92 ms, once).
- `examples/hello` on the web embeds no CJK font, so its CJK text shows missing glyphs.

## [0.1.3] - 2026-11-26

Wave 9: our GPUI fork, core patches and Android text input. Built on `gpui-pre` 0.3.14 plus Parallax's patches.
Verified on the Android emulator (no real-device testing yet).

### Added

- Our patched GPUI fork, published as `parallax-gpui*` crates (one per `gpui-pre-*` crate); every Parallax crate
  depends on it, so apps get each patch from crates.io.
- Pointer device on every pointer event: `TouchEvent::device` (mouse, touch, pen) and `Window::pointer_source()`,
  which says the device and whether the event was synthesized from touch. Taps reach `on_click` as
  `ClickEvent::Touch`.
- `.on_touch` on `StatefulInteractiveElement`: raw touches without touch → mouse emulation; the deepest handler under
  the touch wins, and buttons or scroll containers inside still click and scroll.
- `TextInputConfiguration`: `keyboard_type`, `secure` and an autofill hint (Android sends keyboard type and secure;
  autofill is not sent yet).
- `parallax-android`: soft-keyboard text input through Flutter's ported `InputConnectionAdaptor` and `TextInputPlugin`
  (checked with Gboard on the API 35 emulator, including autocorrect and suggestion picks).
- `parallax-android`: colour emoji drawn by Android itself, so every Android version draws its own (COLRv1 included).

### Fixed

- The back button can be claimed again right after typing (it was ignored until the next touch or mouse move).

### Known limits

- Android mouse and trackpad clicks arrive as synthesized taps; no hover or wheel yet. No Android autofill yet.

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
