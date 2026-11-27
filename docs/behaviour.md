# Behaviour table

Platform-dependent defaults live in one visible table ([AGENTS.md](../AGENTS.md): "Platform feel", "No hidden
behaviour"): `TABLE` in [behaviour.rs](../crates/platform/src/behaviour.rs), in `parallax-platform`. Every value is
Flutter 3.50.1's `TargetPlatform` default, ported (ledger: [PORTED.md](../third_party/flutter/PORTED.md)); the code
comment next to each names its Flutter source. Flutter's tuning wins over GPUI's until upstream matures.

## The table

| Target | Touch slop (hit / pan / scale) | Scroll physics | Overscroll | Velocity tracker | Haptics | Touch target |
|---|---|---|---|---|---|---|
| Android | 8 / 16 / 18 | `CLAMPING` | `Stretch` | least squares | long press `Vibrate`, selection handle `SelectionClick` | 48 |
| iOS | 18 / 36 / 18 | `BOUNCING` | `Bounce` | iOS scroll view | long press `HeavyImpact`, picker item `SelectionClick` | 44 |
| macOS | 18 / 36 / 18 | `BOUNCING_DESKTOP` | `Bounce` | macOS scroll view | none | 44 |
| Windows | 18 / 36 / 18 | `CLAMPING` | `None` | least squares | none | 48 |
| Linux | 18 / 36 / 18 | `CLAMPING` | `None` | least squares | none | 48 |

| Scroll physics | GPUI fling model | Min / max fling (px/s) | Drag start threshold |
|---|---|---|---|
| `BOUNCING` (`BouncingScrollPhysics`) | `Exponential { 0.998 }` | 100 / 8000 | 3.5 |
| `BOUNCING_DESKTOP` (`decelerationRate: fast`) | `Exponential { 0.998 }` | 100 / 64000 | 3.5 |
| `CLAMPING` (`ClampingScrollPhysics`) | `FrictionSpline { 0.015, 160 }` | 50 / 8000 | none |

The same on every row (Flutter's `gestures/constants.dart`, kept under Flutter's names): long press 500 ms, double tap
within 300 ms (`kDoubleTapTimeout`), at least 40 ms apart, 100 px apart at most; a mouse's slops are 1 / 2 / 1
(`PRECISE_POINTER_SLOPS`); `touch_to_mouse` is `true` (GPUI's tap → click; opt out per element with `on_touch`,
[core-patches.md](core-patches.md)).

- **Pointer kind:** `Behaviour::slops(device)` is Flutter's `computeHitSlop`/`computePanSlop`/`computeScaleSlop`:
  touch and pen get the row's slops, a mouse the precise ones.
- **Android's slop** is what Flutter's embedding sends: `ViewConfiguration.getScaledTouchSlop()` (8 dp on AOSP). The
  row holds that default; reading the device's own value is a ROADMAP item.
- **Stretch:** Flutter's `MaterialScrollBehavior` picks it on Android whenever `useMaterial3` (the default) is on,
  whatever the Android version; `Glow` is the Material 2 option.
- **Touch target:** Flutter picks 48 or 44 by widget set; the table follows its `.adaptive` widgets (Cupertino on iOS
  and macOS).
- **The web** has no row. As in Flutter web, it takes its host OS's row: `detect_operating_system` ports
  `BrowserDetection.detectOperatingSystem` (`navigator.platform`, `userAgent`, `maxTouchPoints`; unknown → Android).

## How GPUI gets it

GPUI's touch recognizer reads a `GestureTuning` when a window opens. Upstream takes it only from the platform
(`Platform::gestures()`); [patch 0014](../patches/0014-app-gesture-tuning-override.patch) adds the app's override:
`App::set_gesture_tuning`. `Behaviour::gesture_tuning()` maps a row onto GPUI's fields (`touch_slop` ← hit slop,
`multi_tap_interval` ← `kDoubleTapTimeout`, `multi_tap_slop` ← `kDoubleTapSlop`, `long_press_duration` ←
`kLongPressTimeout`, `scroll_physics` and `min_fling_velocity` ← the scroll physics row). An app sets it first in
its launch function, as [hello](../examples/hello/src/lib.rs) does:

```rust
if let Ok(behaviour) = parallax::parallax_platform::behaviour::current_behaviour() {
    cx.set_gesture_tuning(behaviour.gesture_tuning());
}
```

Windows opened later use it; open windows keep theirs. Without the call GPUI keeps the platform's own tuning.

**Override:** pass a changed copy, e.g. `GestureTuning { touch_slop: px(12.), ..behaviour.gesture_tuning() }`.
Row values GPUI doesn't take yet (pan and scale slops, max fling, haptics, overscroll, touch target) are data for
the widgets and patches that will read them: see ROADMAP Phase 2.

## Extending

- A target: a `Target` variant ([platform.md](platform.md)), its row in `TABLE`, a row in the `behaviour_table` test
  whose name cites the Flutter sources, and this page's table.
- A field: add it to `Behaviour` with a comment naming the Flutter function it comes from, fill every row, assert it
  in the test.
- A Flutter bump: diff the sources listed in PORTED.md and update the values.

Never pick a value by `cfg` outside the table, and never change one without its Flutter source.
