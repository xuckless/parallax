# Changelog

All notable changes to the published crates (`parallax`, `parallax-platform`, `parallax-android`,
`cargo-parallax`). Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/) (pre-1.0: a minor bump may break). Release lines: [ROADMAP.md](ROADMAP.md#releases).

## [Unreleased] (0.1.9)

Wave 14 (Phase 4: text input and selection), fork `parallax-gpui*` 0.3.14-parallax.11 (patches 0028–0035).

Breaking changes (details under Changed): `parallax_android::TouchSlopChanged` takes the reporting Activity's
window; `android_main!` takes a required `open_window` next to `launch`.

Migration: move the `cx.open_window(...)` call out of `launch` into `fn open_window(cx: &mut App)`, then write
`parallax::android_main!(launch, open_window)` and, on desktop, iOS and the web,
`parallax::run(|cx| { launch(cx); open_window(cx) })`.

### Added

- `parallax-ext::text_selection`: Flutter's text selection behaviour for Android and touch desktops as a pure state
  machine (taps, double and triple taps, long press, shift-tap, mouse drags, handle drags, toolbar anchors, the
  Material magnifier's position), with the per-platform rules as a new `text_selection` column of the behaviour
  table (`TextSelectionControls`; iOS is `Native`). hello's text field uses it and draws plain handles, a loupe and
  a Copy / Paste / Select all toolbar ([docs/text-selection.md](docs/text-selection.md)).
- iOS hardware keyboard: presses reach GPUI as `KeyDown`, `KeyUp` and `ModifiersChanged` (shortcuts, arrows,
  Escape, Tab); keys GPUI leaves unhandled still type into the focused field, once; Caps Lock state is reported
  (patch 0031, ported from Flutter's iOS key responder).
- iOS keyboard inset: `WindowInsets::ime` counts only the docked keyboard's part over the window (floating and
  undocked iPad keyboards, Split View, Slide Over, Stage Manager) and follows the keyboard's animation instead of
  jumping, frame-exact on the keyboard's own spring curve (patch 0032, ported from Flutter's keyboard inset
  manager and its React Native spring).
- iOS: `App::on_keyboard_layout_change` fires when the user switches keyboard layout or language (patch 0033).
- iOS text input: Flutter's `FlutterTextInputPluginTest.mm` cases run as a table against gpui_ios on the iOS simulator
  (`scripts/ios-text-oracle.sh`, patch 0028, [docs/ios-text-input.md](docs/ios-text-input.md)): 40 of 44 rows pass,
  4 standard edit actions stay open for the edit-menu work.
- Web app lifecycle (patch 0035): `App::on_app_lifecycle` now hears the page's focus, visibility, freeze and
  resume, the back/forward cache and unloading, as Chrome's Page Lifecycle states mapped to Flutter web's phases
  (docs/web-lifecycle.md).
- Web keyboard inset (patch 0035): `WindowInsets::ime.bottom` is the on-screen keyboard's height from the visual
  viewport, Flutter's rule, and `on_insets_changed` fires when it changes.
- Web IME placement (patch 0034): the hidden IME element follows the caret on desktop browsers, so CJK candidate
  windows open at the text; apps call `Window::invalidate_character_coordinates` when the caret moves.

### Changed

- `parallax-gpui-ios`'s licence is now `Apache-2.0 AND BSD-3-Clause AND MIT`: it ships React Native's keyboard
  spring (MIT) as Flutter does, with `LICENSE-REACT-NATIVE`.
- Android: one GPUI window per `ParallaxActivity` (was one per process, drawn only by the most recently started
  Activity). Two Activities in split screen both draw and take touch and text; each gets its window from the app's
  `open_window` (below). `onDestroy` closes the Activity's window, and an app closing its window finishes its
  Activity (Flutter's `SystemNavigator.pop`). The app's lifecycle phase follows its most foreground Activity
  (`parallax-android`, docs/android-multi-window.md).
- Breaking: `parallax_android::TouchSlopChanged` takes the reporting Activity's window
  (`fn(&mut App, Option<AnyWindowHandle>, Pixels)`), and the facade retunes that window instead of every window
  (`parallax`). Apps using `parallax::android_main!` need no change for it.
- Breaking: `parallax::android_main!(launch, open_window)` and `parallax_android::android_main!(launch, open_window
  [, touch_slop_changed])` take the window opener as a required second argument (`parallax_android::on_load` too).
  `launch` sets the app up once per process; `open_window` opens one window and runs for every `ParallaxActivity`,
  the first one's right after `launch`, so a later Activity never silently re-runs `launch` (re-binding keys and
  globals). Migration: see the top of this section. hello's `launch` and `open_window` are shared by every target.

### Deprecated

- `parallax_android::Error::OneWindow`, no longer constructed; opening a window with no Activity waiting for one is
  the new `Error::NoActivityForWindow`.

### Fixed

- Android: Gboard's select mode (text-editing panel, Select on) extends the selection: a key event's meta state now
  reaches GPUI's keystroke, since Flutter's responder synthesizes no Shift for a virtual keyboard
  (`parallax-android`; Gate 1's known limit 5).
- iOS: backspace deletes a Thai vowel, not its whole grapheme, and whole emoji; half an emoji from iOS no longer
  aborts or corrupts the text; `textInRange:` clamps; line granularity of the tokenizer follows the text's line
  breaks (Flutter's fixes, patch 0029).
- iOS: fields show the keyboard their `TextInputConfiguration::keyboard_type` asks for, and `secure` fields are
  secure text entry (patch 0030).
- Web text input (patch 0034): `TextInputConfiguration::keyboard_type` now picks the software keyboard
  (`inputmode`), `autofill` sets `autocomplete` (else `off`), and the action key (`enterkeyhint`) is set only on iOS
  and Android, Flutter's rules; tested by rows from Flutter's text_editing_test.dart (docs/web-text-input.md).
- macOS direct touch (patches 0025, 0026), from the owner's Sidecar check: each finger keeps one id for its
  whole life (a finger landing or lifting while another moved was handled twice: started, cancelled and started
  again, or moved twice); a touch the system cancels is cancelled where it last was, not at (0, -32). `hello` logs
  `trackpad pinch end`.

## [0.1.8] - 2026-11-27 — Gate 3: desktop touch

Wave 13: desktop touch on Windows, Wayland, X11 and macOS 27 (patches 0021–0027, fork `parallax-gpui*`
0.3.14-parallax.10), pen on Windows, the iOS touch id fix, the Windows and Linux injection harnesses and Gate 3's
desktop trace replay. Gate 3 is synthetic-verified (injected touches; no touchscreen yet); macOS waits on a live
Sidecar check.

Breaking changes (details under Changed): touchscreens and pens on Windows, Linux and macOS 27 now reach apps as
`TouchEvent`s through GPUI's recognizers instead of the OS's emulated mouse events (a tap still clicks, once; a long
press no longer right-clicks); macOS's `min_touch_target` is 36 (was 44); `parallax-gpui-windows`,
`parallax-gpui-apple` and `parallax-gpui-macos` change licence to `Apache-2.0 AND BSD-3-Clause`.

### Added

- Desktop touch in the behaviour table: the macOS, Windows and Linux rows give a touch or pen Flutter's touch slops
  (18 / 36 / 18; the desktop embedders send none) and a mouse the precise-pointer ones, checked by
  `desktop_pointer_slops_table`; documented with Flutter's drag devices and desktop tap-target rules
  (docs/desktop-touch-behaviour.md).
- Gate 3 replay: `trace_replay` reads the `windows`, `wayland` and `x11` trace folders and compares them with the
  phone and browser traces under every row; the fixture contract for desktop harnesses is docs/traces-desktop.md.
- Windows touch and pen (patch 0021, fork `parallax-gpui*` 0.3.14-parallax.10): touchscreen and pen input on Windows
  reaches GPUI as `TouchEvent`s (`device` Touch or Pen, pen pressure as `force`, the pointer's own time stamp), so
  taps, pans, flings, long presses and pinches work as on a phone. Windows' `WM_POINTER` handling is ported from
  winit v0.30.13. Cancelled pointers and lost captures cancel the touch, and a hovering pen moves the mouse. The
  mouse messages Windows makes from touch and pen are dropped (Flutter's `MI_WP_SIGNATURE` check), so a tap clicks
  once, through GPUI's recognizer. Mouse and touchpad input are unchanged
  ([docs/desktop-touch-windows.md](docs/desktop-touch-windows.md)). Each `WM_POINTER` message reads its own pointer's
  samples, as Flutter's embedder does, not winit's whole frame: skipping a frame's other messages stopped Windows
  sending a pinch's later frames. Each sample carries its own pressure; a pointer whose up never arrived is
  cancelled before its next down; a hovering pen leaving the window ends the mouse hover; and a touch's end that
  comes while GPUI is busy waits for it, so no touch stays open.
- Windows touch checked with real input (Gate 3): CI's `windows-touch` job injects touch, pen and mouse into hello
  through a synthetic pointer device (dev-only `touch-inject`), records the Windows gesture traces the replay
  compares with Android, web and iOS, and runs runtime checks of patch 0021 (taps, drags, pinch, cancel, pen, pen
  hover, title bar, mouse, 150 % scale); hello's trace pad also logs pen force and mouse input
  ([docs/windows-touch-harness.md](docs/windows-touch-harness.md)).
- Wayland touch (patch 0022, fork `parallax-gpui*` 0.3.14-parallax.10): touchscreens on Wayland reach GPUI as
  `TouchEvent`s (taps, pans, flings and pinches through GPUI's recognizer, as on mobile), with the compositor's event
  time as `time_stamp`. Ported from winit's Wayland touch handling ([docs/desktop-touch-linux.md](docs/desktop-touch-linux.md)).
- X11 touchscreens (patch 0023, ported from winit 0.30.13): gpui_linux's X11 windows select XInput 2 touch
  events (XInput 2.2 and later) and send them as `TouchEvent`s (`device: Touch`, the server's event time as
  `time_stamp`), so taps, pans, pinches and overscroll work on X11 as on phones. The pointer events the X server
  emulates from a touch are dropped, so a tap clicks once ([docs/desktop-touch-linux.md](docs/desktop-touch-linux.md#x11)).
- Linux touch checked live: `scripts/record-traces.sh wayland` and `x11` record the Gate 2 gestures from hello in
  Weston's headless backend (real `wl_touch` from Weston's test plugin; X11 through Xwayland's XInput 2 touch), at
  output scale 1 and 2. They replay to the same taps, long press, pans, flings and pinch as Android, the web and
  iOS, a tap clicks once, and every touch keeps its own id in logical px. CI runs it (`linux-touch`)
  ([docs/traces-desktop.md](docs/traces-desktop.md#linux-weston-headless)).
- Direct touch on macOS 27 (patches 0024–0026): touchscreen Macs and iPads used as Sidecar displays send GPUI
  `TouchEvent`s (ids, phases including `Cancelled`, device `Touch`, the event's own time stamp) through a gesture
  recognizer on GPUI's view, so taps, pans, flings, long presses and pinches go through GPUI's touch recognizers. No
  change before macOS 27. Add `NSIsTouchNative` (true) to your app bundle's Info.plist to turn off AppKit's
  mouse/scroll emulation of touches; without it the mouse, scroll and magnify copies AppKit marks as touch's are
  dropped while touches are down, and a `mouseCancelled:` ends the press ([docs/desktop-touch-macos.md](docs/desktop-touch-macos.md)).
  Not yet checked on touch hardware.

### Fixed

- iOS touches keep their own `TouchId` (patch 0027): gpui_ios took the id from the `UITouch` address, which UIKit
  reuses, so a double tap's second touch had the first's id. Ids now count per touch, as Flutter's pointer
  identifiers do.
- Wayland and X11 windows blocked by a modal child still get a touch's end or cancel, so no touch stays open.

### Changed

- Desktop touchscreens and pens (Windows, Wayland, X11, macOS 27) send `TouchEvent`s, and the mouse events the OS
  emulates from them are dropped: touch → mouse now comes from GPUI's own recognizer (on by default, opt out per
  element with `.on_touch`), as on phones, so a tap clicks once and the OS's press-and-hold right click is gone.
  Mouse, trackpad and touchpad input are unchanged.
- Trace lines take an optional sixth field, the device (`Touch` or `Pen`); lines without it read as `Touch`. hello's
  trace pad logs it.
- hello's desktop window is 400 × 700 logical px (was 400 × 400), so the recorded gestures fit on desktop.
- `parallax-gpui-windows`, `parallax-gpui-apple` and `parallax-gpui-macos` are `Apache-2.0 AND BSD-3-Clause` and
  ship `LICENSE-FLUTTER` (patches 0021 and 0024 port from Flutter's Windows and Apple embedders). `parallax-gpui-windows` and
  `parallax-gpui-linux` credit winit in their READMEs (patches 0021–0023 port from it; the licence stays Apache-2.0).
- macOS's behaviour row asks for a 36 pt touch target (`APPLE_MACOS_MIN_TOUCH_TARGET`, Apple's macOS 27 touch
  guidance; was Cupertino's 44: Flutter has no macOS touch value).
- CI's `linux-touch` and `windows-touch` jobs run on pull requests, `main` and manual runs, like `android-smoke`.

## [0.1.7] - 2026-11-27 — Gate 2: pinch, overscroll, haptics

Wave 12: Phase 2's gestures wired into GPUI (patches 0016–0020, fork `parallax-gpui*` 0.3.14-parallax.9), Android's
device touch slop, touch time stamps and Gate 2's trace replay.

Breaking changes (details under Changed): `TouchEvent::time_stamp`, `PinchEvent` built with `PinchEvent::new`, two
new `GestureTuning` fields, `Behaviour::gesture_tuning(reported)` and `TouchSlop`, and `parallax_android::on_load`'s
touch slop hook.

### Added

- Touch pinch (patch 0016): two or more fingers on an element with an id and `on_pinch` pinch it, through
  upstream's `PinchEvent`, which gains `touch: Option<TouchPinch>` (`Start` / `Update` / `End` with Flutter's scale
  details: focal and local focal point, scale, horizontal and vertical scale, rotation, pointer count, focal point
  delta, end velocity and scale velocity). A touch pinch fills `position` and `delta` like a trackpad pinch, so one
  listener zooms for both. The deepest `on_pinch` element under the fingers gets it; who wins the fingers follows
  Flutter's gesture arena: one finger still taps and pans, a running pan never becomes a pinch (a joining finger is
  the pan's), fingers dragged together pan, and a claimed long press beats the pinch. `Window::join_pinch` and
  `PinchEvent::touch_owner()` let custom elements take part ([docs/pinch.md](docs/pinch.md)).
- Overscroll in scroll containers (patch 0017): a touch or trackpad pan dragged or flung past a `div` scroll
  container's edge bounces (iOS, macOS: Flutter's friction and spring), stretches (Android: Material 3's stretch,
  recedes on release) or stops hard (Windows, Linux), per the behaviour table; `Glow` (Material 2) on request.
  Trackpad pans drag and fling like a finger (Flutter's `PointerPanZoom`), with the platform's own momentum skipped;
  mouse wheels stop a fling and stay clamped, as in Flutter. A container follows Flutter's scroll activities: a
  touch on a moving container holds it and taps nothing, an offset set from outside stops a fling there, new extents
  restart it, and a horizontal container with nothing to scroll takes no drag. `gpui::overscroll::Overscroll`, `GestureTuning::overscroll` (default `None`: GPUI
  unchanged) and a per-container `.overscroll(..)`; `Overscroll::None` opts a container out
  ([docs/overscroll.md](docs/overscroll.md)). `uniform_list` and `list` don't overscroll yet.
- Haptics (patch 0018, [docs/haptics.md](docs/haptics.md)): `gpui::Haptic` (Flutter's eight `HapticFeedback` calls),
  `Window::perform_haptic(kind)` and `PlatformWindow::perform_haptic` (no-op by default, as on desktops), and
  `GestureTuning::long_press_haptic`, which the window plays once when an element claims a long press. The behaviour
  table's `haptics.long_press` fills it (Android vibrate, iOS heavy impact), so a long press that shows a tooltip, or
  any element that claims one, plays the platform's haptic with no app code. Android plays every kind with Flutter's
  ported `PlatformPlugin.vibrateHapticFeedback` (`View.performHapticFeedback`); iOS (gpui_ios) with
  `UIImpactFeedbackGenerator`, `UISelectionFeedbackGenerator`, `UINotificationFeedbackGenerator` and the vibrate
  system sound, as Flutter's `FlutterPlatformPlugin.mm`; the web (gpui_web) with `navigator.vibrate` and Flutter
  web's durations, where the browser has it.
- `Window::set_gesture_tuning` and `Window::gesture_tuning` (patch 0019): read and retune an open window's tuning in
  place (touches in progress keep their state); `PlatformWindow::set_gesture_tuning` (no-op by default) passes it
  to the backend, so gpui_web's keyboard tap shadow clears at the new touch slop.
- `gpui::play_trace` and `gpui::TraceStep` (patch 0013, `test-support`): one player for recorded input traces on the
  test platform's clock, with optional frames; the fork's gesture, pinch and overscroll tables and Parallax's
  `trace_replay` all use it.
- Android reads its device touch slop at runtime: `ParallaxSurfaceView` sends `ViewConfiguration.getScaledTouchSlop()`
  with every density (as Flutter's FlutterView sends `physicalTouchSlop`), and `parallax_android::touch_slop()` gives
  it in logical pixels. The behaviour table's Android row uses it, with 8 dp as the fallback, and the facade's
  `android_main!` applies it again, on the next looper turn, to the app's tuning and each open window's own when a
  density change moves it, while the platform owns the tuning.
- `parallax::set_gesture_tuning(cx, tuning)`: the app's own gesture tuning, recorded as the app's
  (`parallax_platform::behaviour::TuningSource`), so a density change never overrides it, even when it equals the
  platform's. Use it in place of `cx.set_gesture_tuning`.
- Touch time stamps (patch 0020, [docs/touch-time-stamps.md](docs/touch-time-stamps.md)): `TouchEvent::time_stamp`,
  when the platform says the touch happened (Flutter's `PointerEvent.timeStamp`), on GPUI's executor clock. Android
  fills it from `MotionEvent.getEventTime()`, iOS (gpui_ios) from `UITouch.timestamp`, the web (gpui_web) from
  `PointerEvent.timeStamp`. Velocity tracking (flings, pinch velocity) samples at the time stamp, so touches a
  platform delivers late or in bunches fling as fast as the finger moved; `None` is timed at dispatch, as before.
- Trace replay for Gate 2 ([docs/traces.md](docs/traces.md)): hello's trace pad ("Record touches") logs every touch
  in the trace format, timed by its time stamp; `scripts/record-traces.sh` records it on the web (headless Chromium
  over CDP, `scripts/cdp-touch.mjs`), the iOS simulator (idb on the owner's Mac) and the Android emulator (multi-touch events on its touchscreen); the
  `trace_replay` test replays every trace under every behaviour row and checks the platforms agree.
  `scripts/locate-colour.py` finds a colour on a screenshot (PNG too) for it and `android-smoke.sh`, and
  `scripts/hello-android.sh` is the one hello launch both scripts share.
- hello: the box zooms with a two-finger or trackpad pinch and shows the zoom and rotation, shows its colour as a
  tooltip on hover or long press (the long press plays the haptic); the emoji grid scrolls and shows the platform's
  overscroll.

### Changed

- **Breaking:** `TouchEvent` has a new public field, `time_stamp` (struct literals need `time_stamp: None`), and
  `PinchEvent` a public `touch` (`None` for trackpad pinches) and a private owner: build it with
  `PinchEvent::new(position, delta, modifiers, phase)` in place of a struct literal. `GestureTuning` has two,
  `overscroll` and `long_press_haptic`: literals without `..GestureTuning::default()` need them.
- **Breaking** (`parallax-platform`): `Behaviour::gesture_tuning` takes the reported touch slop
  (`gesture_tuning(None)` keeps the table's values) and now fills `overscroll` and `long_press_haptic` from the row;
  `Behaviour::touch_slop` is a `TouchSlop` (`Unset`, or `Device { fallback }`) in place of `Option<Pixels>`.
  `behaviour::Overscroll` is GPUI's `gpui::overscroll::Overscroll` (same variants) and
  `behaviour::Haptic` is `gpui::Haptic` (eight kinds, not three), both re-exported; new
  `Behaviour::retune(source, current, reported)` with `TuningSource`.
- **Breaking** (`parallax-android`): `on_load` takes a `TouchSlopChanged` hook (`fn(&mut App, Pixels)`, run on its
  own looper turn), and `android_main!` takes one as an optional second argument. Java pulls text-input calls and
  haptics in one `nativeFrameworkCalls` per frame; haptics are dropped while the Activity is paused.
- An element with `on_pinch` and an id now decides its touches like one with `on_click`: an ancestor's `on_touch`
  no longer takes them.

### Fixed

- Flings from the web no longer fall short: GPUI timed touches when it received them, and least squares read the
  browser's jittered frame-rate delivery as a slowing finger (the recorded web fling replayed ~40 % shorter than
  iOS's; with time stamps the two agree within the replay's tolerance under every behaviour row).
- Android: a density change (`wm density`, display size setting) now reaches GPUI's scale. The surface keeps its pixel
  size, so no `surfaceChanged` came; the density now also comes with the insets report the configuration change
  triggers.
- macOS: a trackpad scroll cancelled before it moved (`NSEventPhaseCancelled`) is `TouchPhase::Cancelled`, not
  `Moved`, so the pan it started ends.
- `scripts/sync-fork.sh` takes a hand-written Zed version of a patch's manifest hunk (`patches/zed/*.manifest.patch`,
  `patch-paths.sh --drop-manifests`), so 0018 rebuilds on our Zed fork.
- `scripts/android-smoke.sh`: closes another app's ANR or crash dialog that is already up before each step
  (`hide_error_dialogs` only stops new ones), and retries a rotation the emulator ignored, failing with a clear
  message after the retry.

## [0.1.6] - 2026-11-26 — Phase 2 gestures and physics

Wave 11: Phase 2's gestures and physics, ported from Flutter 3.50.1 into our GPUI fork (patches 0011–0015, fork
`parallax-gpui*` 0.3.14-parallax.8) with Flutter's tests as the oracle.

### Added

- Flutter's velocity trackers (patch 0011): `VelocityTrackerKind` (`LeastSquares`, `IOSScrollViewFling`,
  `MacOSScrollViewFling`) and `GestureTuning::velocity_tracker`; a fling starts from the estimate each platform's
  native scroll views use.
- Flutter's physics (patch 0012): `gpui::physics` (`Tolerance`, `Simulation`, `FrictionSimulation`,
  `SpringSimulation`, `ScrollSpringSimulation`, `near_equal`, `near_zero`, `precisionErrorTolerance`), with springs on
  GPUI's own `SpringConfig` (new `SpringConfig::with_damping_ratio`); `BouncingScrollSimulation`,
  `ClampingScrollSimulation`, `ScrollMetrics`, `ScrollDecelerationRate` and `tolerance_for` at the crate root.
- `gpui::constants`: Flutter's `constants.dart` under its own names (`kTouchSlop`, `kDoubleTapTimeout`, ...), and
  `gpui::slops(kind, touch_slop)` / `gpui::Slops`: Flutter's hit, pan and scale slops per pointer kind.
- `App::set_gesture_tuning` / `App::gesture_tuning` (patch 0014): the app's tuning for windows opened afterwards;
  `GestureTuning::max_fling_velocity`, the fling ceiling (macOS's is 64000 px/s).
- `parallax-platform`: the target → behaviour table (`behaviour::TABLE`, `current_behaviour()`, `web_behaviour`):
  Flutter's `TargetPlatform` defaults for touch slop, scroll physics, velocity tracker, overscroll, haptics and touch
  target ([docs/behaviour.md](docs/behaviour.md)).
- `parallax::run(launch)`: the desktop, iOS and web entry point. It and the facade's `android_main!` apply the
  current target's behaviour row before `launch`, so every app gets its platform's feel and may override it in
  `launch`. `parallax::set_platform_gesture_tuning` returns `Result<(), parallax_platform::Error>`.
- Flutter's scale (pinch/rotate) recognizer (`gpui::scale`) and overscroll indicator state (`gpui::overscroll`: glow,
  stretch, `AxisDirection`), patch 0015. Ported and tested, **not wired** into touch dispatch or scroll containers
  yet (Wave 12).

### Changed

- **Breaking:** `ScrollPhysics` is Flutter's: `Bouncing { deceleration_rate: ScrollDecelerationRate }` and
  `Clamping` replace `Exponential { decay_per_ms }` and `FrictionSpline { friction, pixels_per_inch }`
  (`ScrollPhysics::ios()` and `::android()` remain, now `const fn`). It carries Flutter's fling limits, spring and
  overscroll maths (`create_ballistic_simulation`, `apply_physics_to_user_offset`, `apply_boundary_conditions`).
  **Breaking:** `GestureTuning` has a new field, `max_fling_velocity`.

Behaviour apps will feel:

- Touch slop is Flutter's `kTouchSlop`, 18 px (was 8): a touch may wander further and still tap. Android uses its
  `ViewConfiguration` default of 8 dp; the web keeps 18 on every host, Android included, as Flutter web does.
- Double tap is Flutter's: the second tap must start within 300 ms of the first's release (was 400 ms, timed up to
  up), within 100 px of where the first started (was 16 px), at least 40 ms after the first started
  (`GestureTuning::multi_tap_min_interval`, new), and on the same target: a tap on another button nearby starts a new
  count instead of double-clicking it.
- A release flings only if it is fast enough and moved more than 18 px over the samples its velocity came from
  (Flutter's `isFlingGesture`): a slow, short drag no longer flings.
- Android flings run Flutter's `ClampingScrollSimulation`: they start at the release velocity and stop smoothly
  instead of with a jump (AOSP's spline fails Flutter's tests). iOS flings run `BouncingScrollSimulation`'s friction;
  macOS flings decelerate faster (Flutter's `fast` rate) and may start up to 64000 px/s.
- iOS and macOS rows, and `GestureTuning::default()`, need a 100 px/s release to fling (Flutter's
  `BouncingScrollPhysics`); the others 50 px/s.
- The touch recognizer reads the executor's clock (`background_executor().now()`), so GPUI's test platform can drive
  tap, long-press and fling timing.
- **Licence:** `parallax-gpui` is now `Apache-2.0 AND BSD-3-Clause` and ships Flutter's licence as `LICENSE-FLUTTER`;
  `parallax-platform` likewise (its behaviour table ports Flutter's defaults).

## [0.1.5] - 2026-11-26 — macOS text hotfix

Every macOS app built on Parallax 0.1.1 to 0.1.4 drew no text. Upgrading to 0.1.5 fixes it with no app changes.
Still built on the GPUI fork `parallax-gpui*` 0.3.14-parallax.7 (no patch changes).

### Fixed

- macOS draws text: `parallax-platform` now turns on `gpui_platform`'s `font-kit` feature. 0.1.1 to 0.1.4 built
  without it, so `gpui_macos` used GPUI's `NoopTextSystem` and every macOS app drew no text (boxes only).
- The `hello` example's desktop binary logs to stderr (`env_logger`, `warn,hello=info` by default, `RUST_LOG` to
  change the level).

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
