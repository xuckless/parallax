# Scale and overscroll ports

[Patch 0015](../patches/0015-scale-and-overscroll-ports.patch) adds Flutter's pinch/rotate recognizer and its
overscroll indicators to GPUI as new files: pure state machines and functions of time (the indicators are driven by
`div`'s scroll containers since [0017](overscroll.md)). The
overscroll physics (bounce, springs, clamping boundaries) is GPUI's `ScrollPhysics` ([scroll-physics.md](scroll-physics.md)). Why
port rather than write: [AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth). What each file leaves out:
the rows for 0015 in [PORTED.md](../third_party/flutter/PORTED.md). Their tests are Flutter's, run by
[patch-test.sh](patch-tests.md) (`scripts/patch-test.sh scale`, `scripts/patch-test.sh overscroll`).

## The modules

| Module | Flutter source | What it is |
|---|---|---|
| `gpui::scale` (`gestures/scale.rs`) | `gestures/scale.dart` | `ScaleGestureRecognizer`: touches in, `ScaleOutput`s out (Resolve, Start, Update, End) |
| `gpui::overscroll`: `GlowController`, `StretchController` and the two indicators | `widgets/overscroll_indicator.dart` | Android's glow (opacity, size, displacement over time) and stretch amount |
| `gpui::overscroll::AxisDirection` | `painting/basic_types.dart` | which way a scrollable's offset grows; up and left reverse the stretch |

## Scale recognizer: how to drive it

Per touch: `add_pointer(event, time)` at `Started`, then `handle_event(event, time)` for that event and each
later one. Flutter's gesture arena is the caller's:

- the touch enters the arena at `add_pointer`; a touch whose device is not in `supported_devices` is ignored;
- `ScaleOutput::Resolve { Accepted, pointers }`: the recognizer has already taken them (as Flutter's closed arena
  does) and started; claim them so rivals stop;
- `Resolve { Rejected, .. }`: it gave up; drop it from the arena;
- `accept_gesture(pointer)` when the arena hands it a touch (its only member), `reject_gesture(pointer)` when a
  rival won it. Both are ignored for a pointer whose arena already resolved.

`Start` / `Update` / `End` carry Flutter's details (focal point and delta in window pixels, scale, horizontal and
vertical scale, rotation in radians, pointer count, timestamps; `End` the last pointer's velocity clamped to
Flutter's fling limits and the scale velocity). A pointer joining or leaving ends the gesture and the next move
starts it again, as in Flutter. Slops: `gpui::slops` with `touch_slop` (`None`: Flutter's kScaleSlop 18 / kPanSlop
36; a device setting doubles into the pan slop).

## Overscroll: how to drive it

- **Drag in overscroll:** pass the drag delta through `ScrollPhysics::apply_physics_to_user_offset`.
- **Release:** `ScrollPhysics::create_ballistic_simulation(metrics, velocity)`, then read `x(t)` / `is_done(t)` per
  frame (seconds since release).
- **Clamping platforms:** `ScrollPhysics::Clamping.apply_boundary_conditions` gives the overscroll a move refuses;
  feed it to the indicators.
- **Indicators:** send each `ScrollNotification` (start, update, overscroll with velocity or drag, end) with the
  `ScrollMetrics` to `GlowingOverscrollIndicator` or `StretchingOverscrollIndicator`; `dispatch` is where ancestors
  may disallow the indicator or set a glow's paint offset. Call `tick(now)` every frame with the frame time; draw
  from `GlowController::paint(width, height)` (leading edge at the top; rotate per edge) or `stretch_strength()`.

## What it shares with the other ports

One copy of each Flutter piece: the scale recognizer tracks velocity with [0011](velocity-tracking.md)'s
`VelocityTracker` (least squares, as Flutter's `VelocityTracker.withKind`) and reads slops and fling limits from
[0013](gesture-recognizers.md)'s `gpui::slops` and `gpui::constants`; the stretch spring is
[0012](scroll-physics.md)'s `SpringSimulation` (GPUI's `SpringConfig`), the metrics its `ScrollMetrics`.

Wave 12 wires them: the scale recognizer into touch dispatch next to the upstream `TouchGestureRecognizer`
(claims as the arena; scale results to `on_pinch` or a new listener); patch 0017 drives the bouncing physics and the
indicators from `div`'s scroll containers, chosen per target from the platform behaviour table
([overscroll.md](overscroll.md)).
