# Scroll physics presets

A touch pan released fast enough keeps scrolling: GPUI's recognizer in `gestures.rs` turns the release into momentum
scroll events. How that momentum decelerates is `ScrollPhysics`, chosen per platform through
`GestureTuning::scroll_physics`. Patch [0012](../patches/0012-flutter-scroll-simulations.patch) makes each preset run
a fling simulation ported from Flutter 3.50.1 ([why Flutter](flutter.md)). The ledger rows are in
[PORTED.md](../third_party/flutter/PORTED.md).

## The presets

| Preset | Flutter physics | Simulation (`gpui-pre/src/gestures/physics.rs`) | Constants |
|---|---|---|---|
| `ScrollPhysics::ios()` = `Exponential { decay_per_ms: 0.998 }` | `BouncingScrollPhysics`, in range | [`FrictionSimulation`](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/lib/src/physics/friction_simulation.dart) | drag `0.998^1000` (Flutter's "~0.135", `UIScrollView.decelerationRate.normal`) |
| `ScrollPhysics::android()` = `FrictionSpline { friction: 0.015, pixels_per_inch: 160 }` | `ClampingScrollPhysics`, in range | [`ClampingScrollSimulation`](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/lib/src/widgets/scroll_simulation.dart) | AOSP `OverScroller`'s friction and deceleration rate; `pixels_per_inch` replaces Flutter's fixed 160 |

`ScrollPhysics::simulation(speed)` builds the preset's simulation, as Flutter's `createBallisticSimulation` does for
a position inside the content. The public `ScrollPhysics` enum and its constructors are unchanged from upstream.
`Tolerance`, `Simulation`, `FrictionSimulation` and `ClampingScrollSimulation` are exported from the crate root, so
an app or a scroll container can build its own.

Android's fling is Flutter's ballistic form of `OverScroller`: the same total distance as Android, but a deceleration
that depends only on the current velocity, so it starts at the release velocity and comes to a smooth stop. Upstream
GPUI ran AOSP's spline itself, which fails Flutter's clamping tests (see the patch header).

## How momentum uses it

- The recognizer starts momentum only if the simulation is not done at time 0 (Flutter's "stationary" check).
- Each frame emits the step from the previous distance to `x(elapsed)`; the fling ends on the first frame where
  `is_done(elapsed)`, as Flutter's ballistic scroll activity does.
- The stop tolerance is Flutter's `ScrollPhysics.toleranceFor` at a device pixel ratio of 2: velocity
  `1 / (0.050 * 2)` = 10 px/s (upstream's `MOMENTUM_STOP_VELOCITY`), distance 0.5 px. The recognizer doesn't know the
  window's scale factor, so it can't use the real ratio yet.

## Boundaries

Presets model deceleration only. Bounce, glow and clamping at the content's edges belong to the scroll container,
and the overscroll maths (`BouncingScrollSimulation`'s springs, `frictionFactor`) is separate work. `FrictionSimulation`
keeps Flutter's `final_x`, `time_at_x` and constant deceleration because those parts build on them.

## Not ported yet

| Flutter behaviour | Where it would go |
|---|---|
| `BouncingScrollPhysics.minFlingVelocity` = 100 (GPUI's `min_fling_velocity` is 50 on every preset) | platform behaviour table (ROADMAP Phase 2) |
| macOS `ScrollDecelerationRate.fast`: constant deceleration 1400, max fling velocity x8 | a desktop preset |
| `carriedMomentum`: repeated iOS flings speed up | the recognizer's fling catch |
| `toleranceFor` at the window's real device pixel ratio | passing the scale factor to the recognizer |

## Tests

Flutter's tests are the spec ([AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth), rule 6). They are
tables at the bottom of `physics.rs`, one row per Flutter test, with the Flutter name in the row:
`FRICTION_CASES` (`friction_simulation_test.dart`, two `newton_test.dart` cases), `CLAMPING_CASES`
(`scroll_simulation_test.dart`) and `presets_keep_the_release_velocity_at_time_0` (`scroll_physics_test.dart`).
They run with GPUI's lib tests: `scripts/patch-test.sh physics` ([patch-tests.md](patch-tests.md)). To port another
Flutter case, add a row and a ledger note; the dropped cases are listed in the ledger.
