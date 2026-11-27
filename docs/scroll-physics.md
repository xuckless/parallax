# Scroll physics

How a scrollable decelerates after a fling and behaves past its edges is GPUI's `ScrollPhysics`: one Rust type
for Flutter's `ScrollPhysics` class, a variant per subclass, used by the touch recognizer's flings, the overscroll
maths and the [behaviour table](behaviour.md). Patch [0012](../patches/0012-flutter-scroll-simulations.patch) ports
it from Flutter 3.50.1 ([why Flutter](flutter.md)); ledger rows in [PORTED.md](../third_party/flutter/PORTED.md).

## The type

| `ScrollPhysics` | Flutter class | Simulation on release | Fling limits (min / max px/s) |
|---|---|---|---|
| `Bouncing { deceleration_rate: Normal }` = `ios()` | `BouncingScrollPhysics` | `BouncingScrollSimulation`: friction drag 0.135 (UIScrollView's 0.998 per ms, written once), a spring past an edge | 100 / 8000 |
| `Bouncing { deceleration_rate: Fast }` | `BouncingScrollPhysics(decelerationRate: fast)` (macOS) | the same, plus a constant deceleration of 1400 and a stiffer spring | 100 / 64000 |
| `Clamping` = `android()` | `ClampingScrollPhysics` | `ClampingScrollSimulation` (Android's `OverScroller`, made ballistic); a spring back when out of range | 50 / 8000 |

Its methods are Flutter's: `create_ballistic_simulation(metrics, velocity)`, `apply_physics_to_user_offset`
(overscroll friction while dragging), `apply_boundary_conditions` (the overscroll a clamping scrollable refuses),
`spring`, `min_fling_distance` / `min_fling_velocity` / `max_fling_velocity`, `drag_start_distance_motion_threshold`,
with `ScrollMetrics`, `tolerance_for(device_pixel_ratio)` and `ScrollDecelerationRate::friction_factor`.

## Where the code is (`gpui-pre/src/gestures/`)

| File | Flutter |
|---|---|
| `physics.rs` (`gpui::physics`) | physics/*.dart: `Tolerance`, `Simulation`, `FrictionSimulation`, `SpringSimulation`, `ScrollSpringSimulation`, `near_equal` / `near_zero`; foundation's `precisionErrorTolerance` |
| `scroll_simulation.rs` | widgets/scroll_simulation.dart: `BouncingScrollSimulation`, `ClampingScrollSimulation` |
| `scroll_physics.rs` | widgets/scroll_physics.dart: `ScrollPhysics`, `ScrollDecelerationRate`, `ScrollMetrics`, `tolerance_for` |

**Springs are upstream's.** `SpringDescription` is GPUI's `SpringConfig` (plus `with_damping_ratio`), and
`SpringSimulation` is glue over `SpringConfig::step` with Flutter's `isDone`: upstream's closed form passes every
spring case of Flutter's newton_test.dart and the rubber-band rows, so Flutter's `_SpringSolution` is not ported.

## How a touch fling uses it

The recognizer has no extents (the scroll container clamps), so it calls `create_ballistic_simulation` with
unbounded metrics: Bouncing runs its friction phase, Clamping its `ClampingScrollSimulation`. The release velocity is
clamped to the tuning's `max_fling_velocity`, and a release flings only as Flutter's `isFlingGesture` allows
([gesture-recognizers.md](gesture-recognizers.md)). Each frame emits the step to `x(elapsed)`; the fling ends on the
first frame `is_done(elapsed)`. The stop tolerance is `tolerance_for(2.0)` (10 px/s, 0.5 px): the recognizer doesn't
know the window's scale factor yet.

## Not ported yet

| Flutter behaviour | Where it would go |
|---|---|
| `carriedMomentum`: repeated iOS flings speed up | the recognizer's fling catch |
| `toleranceFor` at the window's real device pixel ratio | passing the scale factor to the recognizer |
| `PageScrollPhysics` and the other physics classes | a variant each, when a widget needs one |

## Tests

Flutter's tests are the spec ([AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth), rule 6), one row
per Flutter test with its name: `FRICTION_CASES` and `SPRING_CASES` in physics.rs (friction_simulation_test.dart,
newton_test.dart), `CLAMPING_CASES` and `BOUNCING_CASES` in scroll_simulation.rs (scroll_simulation_test.dart,
newton_test.dart), and scroll_physics_tests.rs (scroll_physics_test.dart, one time-0 row over both physics). Run
them with `scripts/patch-test.sh physics` and `scripts/patch-test.sh scroll_` ([patch-tests.md](patch-tests.md)).
To port another Flutter case, add a row and a ledger note; the dropped cases are listed in the ledger.
