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
| macOS | `Unset`: 18 / 36 / 18 | `Bouncing { Fast }` | `Bounce` | `MacOSScrollViewFling` | none | 44 |
| Windows | `Unset`: 18 / 36 / 18 | `Clamping` | `None` | `LeastSquares` | none | 48 |
| Linux | `Unset`: 18 / 36 / 18 | `Clamping` | `None` | `LeastSquares` | none | 48 |

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
- The same everywhere (Flutter's `constants.dart`, `gpui::constants`): long press 500 ms, double tap within 300 ms,
  at least 40 ms apart, 100 px apart at most; taps count per target ([gesture-recognizers.md](gesture-recognizers.md)).

## How GPUI gets it: automatically

Every Parallax entry point applies the current target's row before app code runs:
[`parallax::run(launch)`](../crates/parallax/src/lib.rs) on desktop, iOS and the web (hello's
[main.rs](../examples/hello/src/main.rs) and web `start`), the facade's `android_main!` on Android. Both call
`parallax::set_platform_gesture_tuning` (`Result<(), parallax_platform::Error>`; they log an error and keep the
platform's tuning). `Behaviour::gesture_tuning(reported)` fills GPUI's `GestureTuning` from the row (`touch_slop` ←
the hit slop of the resolved touch slop: Android passes `parallax_android::touch_slop()`, the others `None`;
`scroll_physics`, `min_fling_velocity` / `max_fling_velocity` ← its fling limits, `velocity_tracker`, `overscroll`,
`long_press_haptic` ← `haptics.long_press`, played on a claimed long press: [haptics.md](haptics.md)), the rest from
`GestureTuning::default()` (Flutter's constants), through `App::set_gesture_tuning` (patch 0014).
`parallax_android::android_main!` alone does not apply it (the table builds on that crate): use the facade's.

**Density changes (Android):** when the device's logical slop changes while the app runs, parallax-android calls the
hook the facade's `android_main!` passes (`TouchSlopChanged`), and `Behaviour::retune` sets the new hit slop on the
app's tuning and its open window (`Window::set_gesture_tuning`, patch 0019), unless the app set its own slop. Logcat:
`parallax: touch slop <px> px at density <d>` from parallax-android whenever the density or slop moves (the density
reaches it through `onApplyWindowInsets`, the surface keeps its size), then `gesture tuning's touch slop now …` only
when the logical slop moved: Android scales the slop with the density, so most changes keep it (8 at 420 and 560 dpi;
`wm density 450` makes it 23 px / 2.8125 = 8.18).

**Override:** call `cx.set_gesture_tuning` in `launch`, after the entry point's call, with a changed copy, e.g.
`GestureTuning { touch_slop: px(12.), ..cx.gesture_tuning() }`. Windows opened later use it; an overridden slop
survives density changes.

**Read by GPUI:** `overscroll` is `GestureTuning::overscroll`, the default every scroll container follows (a
container may set its own): [overscroll.md](overscroll.md). `haptics.long_press` plays on a claimed long press
(patch 0018, [haptics.md](haptics.md)). The touch pinch's scale recognizer takes its pan and scale slops from
`touch_slop` ([pinch.md](pinch.md)). The other `haptics` rows (`tap`, `selection_handle_drag`, `picker_item`: no
GPUI widget plays them yet), `min_touch_target`, the pan and scale slops and `drag_start_distance_motion_threshold`
are data GPUI doesn't take yet; the widgets that later waves wire read them (ROADMAP Phase 2).

## Extending

- A target: a `Target` variant ([platform.md](platform.md)), its row in `TABLE`, a row in the `behaviour_table` test
  whose name cites the Flutter sources, and this page's table.
- A field: add it to `Behaviour` with a comment naming the Flutter function it comes from, fill every row, assert it
  in the test.
- A Flutter bump: diff the sources listed in PORTED.md and update the values.

Never pick a value by `cfg` outside the table, and never change one without its Flutter source.
