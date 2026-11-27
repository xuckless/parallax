# Scale and overscroll ports

[Patch 0015](../patches/0015-scale-and-overscroll-ports.patch) adds Flutter's pinch/rotate recognizer and its
overscroll maths to GPUI as new files: pure state machines and functions of time, called by nothing yet. Why
port rather than write: [AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth). What each file leaves out:
the rows for 0015 in [PORTED.md](../third_party/flutter/PORTED.md). Their tests are Flutter's, run by
[patch-test.sh](patch-tests.md) (`scripts/patch-test.sh scale`, `scripts/patch-test.sh overscroll`).

## The modules

| Module | Flutter source | What it is |
|---|---|---|
| `gpui::scale` (`gestures/scale.rs`) | `gestures/scale.dart` | `ScaleGestureRecognizer`: touches in, `ScaleOutput`s out (Resolve, Start, Update, End) |
| `gpui::overscroll` `near_equal`, `near_zero` | `physics/utils.dart` | float comparisons the springs and indicators use |
| `SpringSimulation`, `ScrollSpringSimulation` | `physics/spring_simulation.dart` | closed-form springs (f64; GPUI's `SpringConfig` steps f32 state instead) |
| `BouncingScrollSimulation` | `widgets/scroll_simulation.dart` | iOS fling: 0012's `FrictionSimulation`, then a spring once it would leave the range |
| `BouncingScrollPhysics`, `ClampingScrollPhysics` | `widgets/scroll_physics.dart` | overscroll friction on drags, boundary conditions, the ballistic simulation on release |
| `GlowController`, `StretchController` and the two indicators | `widgets/overscroll_indicator.dart` | Android's glow (opacity, size, displacement over time) and stretch amount |

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
starts it again, as in Flutter. Slops: Flutter's (`kTouchSlop` 18, `kPanSlop` 36), or `touch_slop` for the platform's.

## Overscroll: how to drive it

- **Drag in overscroll:** pass the drag delta through `BouncingScrollPhysics::apply_physics_to_user_offset`.
- **Release:** `create_ballistic_simulation(metrics, velocity)`, then read `x(t)` / `is_done(t)` per frame
  (seconds since release).
- **Clamping platforms:** `ClampingScrollPhysics::apply_boundary_conditions` gives the overscroll a move refuses;
  feed it to the indicators.
- **Indicators:** send each `ScrollNotification` (start, update, overscroll with velocity or drag, end) with the
  `ScrollMetrics` to `GlowingOverscrollIndicator` or `StretchingOverscrollIndicator`; `dispatch` is where ancestors
  may disallow the indicator or set a glow's paint offset. Call `tick(now)` every frame with the frame time; draw
  from `GlowController::paint(width, height)` (leading edge at the top; rotate per edge) or `stretch_strength()`.

## What it shares with the other ports

One copy of each Flutter piece: the scale recognizer tracks velocity with [0011](velocity-tracking.md)'s
`VelocityTracker` (least squares, as Flutter's `VelocityTracker.withKind`) and reads slops and fling limits from
[0013](gesture-recognizers.md)'s `gpui::constants`; the overscroll simulations implement
[0012](scroll-physics.md)'s `Simulation` and `Tolerance`, and `BouncingScrollSimulation`'s friction phase is 0012's
`FrictionSimulation` (its time-0 test row runs it with Flutter's values).

Wave 12 wires them: the scale recognizer into touch dispatch next to the upstream `TouchGestureRecognizer`
(claims as the arena; scale results to `on_pinch` or a new listener), bouncing physics and the indicators into
GPUI's scroll containers, chosen per target from the platform behaviour table
([AGENTS.md](../AGENTS.md#decisions-settled-do-not-reopen-without-the-owner), "Platform feel").
