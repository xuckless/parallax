# Velocity tracking

How fast a finger was moving when it lifted decides whether a touch pan flings, and how far. GPUI's touch recognizer
in [gestures.rs](../upstream/gpui-pre/src/gestures.rs) estimated that with its own least-squares fit. Patch
[0011](../patches/0011-flutter-velocity-tracker.patch) replaces it with Flutter's velocity trackers, ported to Rust
(rules: [AGENTS.md, Flutter](../AGENTS.md#flutter-is-the-second-source-of-truth); ledger rows in
[PORTED.md](../third_party/flutter/PORTED.md)).

## What came from where

| Parallax (in `gpui-pre/src/gestures/`, via the patch) | Flutter 3.50.1 |
|---|---|
| `velocity_tracker.rs`: `VelocityTracker`, `VelocityTrackerKind` | [velocity_tracker.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/lib/src/gestures/velocity_tracker.dart) |
| `lsq_solver.rs`: `LeastSquaresSolver`, `PolynomialFit` | [lsq_solver.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/lib/src/gestures/lsq_solver.dart) |
| `velocity_tracker_test.rs` + `velocity_tracker_data.rs` | [velocity_tracker_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/velocity_tracker_test.dart) + [velocity_tracker_data.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/velocity_tracker_data.dart) |
| `lsq_solver_test.rs` | [lsq_solver_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/gestures/lsq_solver_test.dart) |

Flutter's three tracker classes are one `VelocityTracker` with a kind. Their names, constants and comments are
Flutter's, so a later Flutter fix diffs straight in.

## The kinds

| `VelocityTrackerKind` | Flutter class | Estimate |
|---|---|---|
| `LeastSquares` | `VelocityTracker` | quadratic least-squares fit over the last 100 ms of continuous motion (≥ 3 samples), slope at the newest sample |
| `IOSScrollViewFling` | `IOSScrollViewFlingVelocityTracker` | the last three sample-to-sample velocities, weighted 0.6 / 0.35 / 0.05 (oldest first), like `UIScrollView` |
| `MacOSScrollViewFling` | `MacOSScrollViewFlingVelocityTracker` | the same, weighted 0.15 / 0.65 / 0.2 |

All three report zero when no sample came in for over 40 ms (the finger stopped before lifting). The weights are one
table in `velocity_tracker.rs`; a new kind of that shape is a row there plus an arm in `get_velocity`.

## Which platform gets which

`GestureTuning::velocity_tracker` holds the kind, next to `scroll_physics`. Flutter's
[`ScrollBehavior.velocityTrackerBuilder`](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/lib/src/widgets/scroll_configuration.dart)
gives iOS the iOS tracker, macOS the macOS one, and Android, Linux and Windows `LeastSquares`: each target's row of
the [platform behaviour table](behaviour.md), which every Parallax entry point applies (the web takes its host's).
`GestureTuning::default()` (Flutter's iOS values) has `IOSScrollViewFling`. An app overrides the kind like any other
`GestureTuning` field. The scale recognizer ([scale-and-overscroll.md](scale-and-overscroll.md)) uses
`LeastSquares`, as Flutter's does.

`get_velocity_estimate` also returns Flutter's `VelocityEstimate.offset`, the distance between the oldest and newest
samples the estimate used: the recognizer flings only when it exceeds the physics' minimum fling distance
([gesture-recognizers.md](gesture-recognizers.md)).

## Tests

The ported Flutter tests are table-driven, one row per Flutter test, named after it. They sit inside GPUI because the
tracker is private and GPUI's test platform can't reach it. Run them with
`scripts/patch-test.sh velocity_tracker` and `scripts/patch-test.sh lsq_solver` ([patch-tests.md](patch-tests.md)).

Before the port, upstream's tracker passed the two recorded traces and its quadratic fit matched the two quadratic
lsq rows. It failed "Makes consistent velocity estimates with consistent velocity" and had no iOS or macOS tracker
for the assertion rows. It also reported a velocity from two samples, where Flutter reports zero.

## When Flutter's tag moves

[flutter-diff.sh](../scripts/flutter-diff.sh) lists changes to each ledger row ([flutter.md](flutter.md)). Carry a
change into the patch, keeping Flutter's names, and add a test row for any new Flutter test case.
