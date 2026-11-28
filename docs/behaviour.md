# Behaviour table

Platform-dependent defaults live in one visible table ([AGENTS.md](../AGENTS.md): "Platform feel", "No hidden
behaviour"): `TABLE` in [behaviour.rs](../crates/platform/src/behaviour.rs), in `parallax-platform`. Every value is
Flutter 3.50.1's `TargetPlatform` default, ported (ledger: [PORTED.md](../third_party/flutter/PORTED.md)); the code
comment next to each names its Flutter source. Flutter's rules themselves (constants, slops, scroll physics) are
GPUI's, ported once by patches 0012-0013; a row holds only what Flutter picks per platform.

## The table

| Target | `touch_slop` (hit / pan / scale) | `scroll_physics` | Overscroll | `velocity_tracker` | Haptics | Touch target |
|---|---|---|---|---|---|---|
| Android | `Device { fallback: 8 }`: the device's (8 / 16 / 18 at 8) | `Clamping` | `Stretch` | `LeastSquares` | long press `Vibrate`, selection handle `SelectionClick` | 48 |
| iOS | `Unset`: 18 / 36 / 18 | `Bouncing { Normal }` | `Bounce` | `IOSScrollViewFling` | long press `HeavyImpact`, picker item `SelectionClick` | 44 |
| macOS | `Unset`: 18 / 36 / 18 | `Bouncing { Fast }` | `Bounce` | `MacOSScrollViewFling` | none | 36 (Apple, `APPLE_MACOS_MIN_TOUCH_TARGET`) |
| Windows | `Unset`: 18 / 36 / 18 | `Clamping` | `None` | `LeastSquares` | none | 48 |
| Linux | `Unset`: 18 / 36 / 18 | `Clamping` | `None` | `LeastSquares` | none | 48 |

`text_selection`: Android, macOS, Windows and Linux run Flutter's selection rules (`TextSelectionControls::Flutter`
with a `SelectionRules` row: Android's mobile arms, macOS the Apple ones, Windows and Linux the desktop ones, Material's
22 px handle everywhere); iOS is `Native` (UIKit). What each rule does: [text-selection.md](text-selection.md).

`scroll_physics` is GPUI's `ScrollPhysics` ([scroll-physics.md](scroll-physics.md)), which also gives the fling
limits: min / max fling 100 / 8000 px/s for `Bouncing { Normal }`, 100 / 64000 for `Fast` (with its 1400 constant
deceleration), 50 / 8000 for `Clamping`; drag start threshold 3.5 for bouncing, none for clamping.

- **Slops:** `touch_slop` is the embedder's `DeviceGestureSettings.touchSlop`: `Unset` (it sends none) or
  `Device { fallback }` (the platform reports one at runtime). `TouchSlop::resolve(reported)` gives the value, and
  `gpui::slops(kind, value)` (Flutter's `computeHitSlop` / `PanSlop` / `ScaleSlop`) gives touch and pen these, a
  mouse 1 / 2 / 1.
- **Android's slop** is the device's, as Flutter's embedding sends it: `ParallaxSurfaceView` reads
  `ViewConfiguration.getScaledTouchSlop()` (physical pixels) with every density it sends Rust (surface created and
  changed, insets: FlutterView's `physicalTouchSlop` beside `devicePixelRatio`), and parallax-android divides it by
  the density (`DeviceGestureSettings.fromView`): `parallax_android::touch_slop()`. AOSP's 8 dp is the documented
  fallback, used until the first surface reports.
- **Stretch:** Flutter's `MaterialScrollBehavior` picks it on Android whenever `useMaterial3` (the default) is on;
  `Glow` is the Material 2 option. **Touch target:** the set Flutter's `.adaptive` widgets pick per platform.
- **The web** has no row. As in Flutter web, it takes its host OS's row (`detect_operating_system` ports
  `BrowserDetection.detectOperatingSystem`; unknown → Android), but with Flutter's slops (`web_behaviour`): Flutter
  web never sets `physicalTouchSlop`, so an Android browser keeps 18 / 36.
- **Desktop touch:** touch slops, the row's physics, drags scroll: [desktop-touch-behaviour.md](desktop-touch-behaviour.md).
- **Not in the table:** reduce motion and reduce transparency are the user's OS settings, read at runtime on every
  target, not per-target defaults: [os-settings.md](os-settings.md).
- The same everywhere (Flutter's `constants.dart`, `gpui::constants`): long press 500 ms, double tap within 300 ms,
  at least 40 ms apart, 100 px apart at most; taps count per target ([gesture-recognizers.md](gesture-recognizers.md)).

How the row reaches GPUI (automatically, before app code), density changes, overriding it and what GPUI reads:
[behaviour-tuning.md](behaviour-tuning.md).

## Extending

- A target: a `Target` variant ([platform.md](platform.md)), its row in `TABLE`, a row in the `behaviour_table` test
  whose name cites the Flutter sources, and this page's table.
- A field: add it to `Behaviour` with a comment naming its Flutter function, fill every row, assert it in the test.
- A Flutter bump: diff the sources listed in PORTED.md and update the values.

Never pick a value by `cfg` outside the table, and never change one without its Flutter source.
