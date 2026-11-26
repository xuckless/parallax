# Oracle inventory: gestures, velocity, scroll physics

Part of the [oracle inventory](oracle-inventory.md). Flutter tag `3.50.1`; upstream `gpui-pre` 0.3.14.
Cases = `test(`/`testWidgets(`/`testGesture(` calls. "Salvage" = cases whose input and expected output survive the mapping.

Everything checked here lives in one file, [gestures.rs](../../upstream/gpui-pre/src/gestures.rs) (its own 40 tests at the bottom).
`VelocityTracker`, `ScrollPhysics::fling_*` and `TouchGestureRecognizer` are private or `pub(crate)`, so every row below
needs in-crate test access (see [risk 1](oracle-inventory.md#risks)).

## Velocity and fitting

| Flutter test | Size / cases | Tests | Checks upstream | Verdict |
|---|---|---|---|---|
| [velocity_tracker_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/velocity_tracker_test.dart) + [velocity_tracker_data.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/velocity_tracker_data.dart) | 7.6 KB + 48 KB trace / 11 | recorded pointer trace → expected velocities; 40 ms stopped gap; iOS/macOS fling trackers | [`VelocityTracker::velocity`](../../upstream/gpui-pre/src/gestures.rs) | EASY: (time, position) samples → velocity. Same constants (100 ms, 40 ms, 20 samples, lsq2). 7/11; iOS/macOS fling trackers have no GPUI twin |
| [lsq_solver_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/lsq_solver_test.dart) | 3 KB / 5 | polynomial least-squares fits | [`quadratic_velocity_at_newest`](../../upstream/gpui-pre/src/gestures.rs) | EASY for the 2 quadratic cases; GPUI returns only the linear term, so 2/5 (linear-degree and `toString` cases drop) |

## Physics

| Flutter test | Size / cases | Tests | Checks upstream | Verdict |
|---|---|---|---|---|
| [friction_simulation_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/physics/friction_simulation_test.dart) | 2.9 KB / 3 | `FrictionSimulation` x(t), dx(t), time to stop | `ScrollPhysics::Exponential` `fling_distance`/`fling_duration` | MAPPED: `decay_per_ms = drag^(1/1000)` (iOS 0.998/ms ≡ drag 0.135). 2/3; constant-deceleration case has no twin |
| [scroll_simulation_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/widgets/scroll_simulation_test.dart) | 6.7 KB / 5 | `ClampingScrollSimulation`: stable start, only decelerates, ballistic, physical formula | `ScrollPhysics::FrictionSpline` | MAPPED: `(position, velocity)` → `fling_distance(speed, t)`. Both are AOSP `OverScroller`; 4/5 (property checks, not exact values) |
| [newton_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/physics/newton_test.dart) | 10.6 KB / 11 | friction, gravity, springs, kinetic scroll, edge springs | `ScrollPhysics::Exponential` | HARD: no springs, gravity or bounds in GPUI. 3/11 (friction cases) |
| [spring_simulation_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/physics/spring_simulation_test.dart), [clamped_simulation_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/physics/clamped_simulation_test.dart) | 5.8 KB / 9 | springs, clamping | none | HARD: 0/9. Becomes relevant only if a container adds bounce |
| [scroll_physics_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/widgets/scroll_physics_test.dart) | 25.6 KB / 25 | `Bouncing`/`Clamping` overscroll resistance, `applyTo`, `shouldUpdate` | none: GPUI leaves boundaries to the container | HARD: ~6/25 (overscroll resistance math) once a container owns bounce; the rest is widget API |
| [range_maintaining_scroll_physics_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/widgets/range_maintaining_scroll_physics_test.dart) | 16 KB / 6 | content resize while scrolled | none | HARD: 0/6 (widget layout) |

## Recognizers

Mapping for every MAPPED row: `TestPointer.down/move/up/cancel` → `TouchEvent { phase: Started/Moved/Ended/Cancelled }`
into `TouchGestureRecognizer::handle_event_at(now)`; `tester.async.elapse` → advancing `now`; callbacks → the
`RecognizedTouchGesture` stream. Pass Flutter's constants as an explicit `GestureTuning` (GPUI defaults differ:
slop 8 vs `kTouchSlop` 18, multi-tap 400 ms/16 px vs 300 ms/100 px). `arena.close/sweep` boilerplate drops out
(one recognizer, claims instead of an arena). Mouse-button, multi-pointer and device-kind cases do not carry over.

| Flutter test | Size / cases | Tests | Checks upstream | Verdict |
|---|---|---|---|---|
| [tap_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/tap_test.dart) | 32 KB / 33 | tap, slop cancel, timeout, cancel event, buttons | tap path of `TouchGestureRecognizer` | MAPPED: ~11/33 |
| [double_tap_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/double_tap_test.dart), [serial_tap_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/serial_tap_test.dart), [multitap_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/multitap_test.dart) | 43 KB / 41 | inter/intra-tap delay and distance, over-rapid taps, tap count | `CompletedTap` tap count | MAPPED: ~12/41 |
| [long_press_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/long_press_test.dart) | 27 KB / 26 | duration, move before/after accept, up cancels | `LongPressEvent`, long-press offer/claim | MAPPED: ~9/26 |
| [drag_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/drag_test.dart), [monodrag_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/monodrag_test.dart) | 98 KB / 50 | pan/drag start, axis, max-velocity clamp, synthesized events skip velocity, multi-pointer | pan, axis lock, `MAX_FLING_VELOCITY`, `min_fling_velocity` | MAPPED: ~12/50 (35 are multi-pointer) |
| [tap_and_drag_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/tap_and_drag_test.dart) | 33 KB / 31 | tap-then-drag (text selection) | `TouchDragEvent` claim | MAPPED: ~5/31; most need a selection widget |
| [scale_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/scale_test.dart) | 63 KB / 15 | pinch/rotate | none: "Pinch recognition is not implemented yet" | HARD: 0/15 now; all 15 become MAPPED when upstream adds pinch |
| [arena_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/arena_test.dart), [team_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/team_test.dart), [recognizer_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/recognizer_test.dart) | 22 KB / 23 | arena accept/sweep/hold, teams | claim model in `TouchGestureRecognizer` | HARD: ~2/23 ("first claim wins") |
| [resampler_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/resampler_test.dart) | 34 KB / 10 | pointer resampling to vsync | none (GPUI predicts instead) | HARD: 0/10 |

Not listed (no GPUI equivalent, 0 salvage): `debug`, `events`, `hit_test`, `pointer_router`, `pointer_signal_resolver`,
`gesture_binding*`, `force_press`, `transformed_*` tests in [packages/flutter/test/gestures](https://github.com/flutter/flutter/tree/3.50.1/packages/flutter/test/gestures).

Area total: 22 files, 304 cases; EASY 2 files, MAPPED 10, HARD 10; ~75 cases salvageable.
