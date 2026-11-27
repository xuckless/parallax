# Scale and overscroll ports

[Patch 0015](../patches/0015-scale-and-overscroll-ports.patch) adds Flutter's pinch/rotate recognizer and its
overscroll maths to GPUI as new files: pure state machines and functions of time, called by nothing yet. Why
port rather than write: [AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth). What each file leaves out:
the rows for 0015 in [PORTED.md](../third_party/flutter/PORTED.md). Their tests are Flutter's, run by
[patch-test.sh](patch-tests.md) (`scripts/patch-test.sh scale`, `scripts/patch-test.sh overscroll`).

## The modules

| Module | Flutter source | What it is |
|---|---|---|
| `gpui::scale` (`gestures/scale.rs`) | `gestures/scale.dart` | `ScaleGestureRecognizer<V>`: touches in, `ScaleOutput`s out (Resolve, Start, Update, End) |
| `gpui::overscroll` `Simulation`, `Tolerance` | `physics/simulation.dart`, `tolerance.dart` | the simulation trait every physics type below implements |
| `SpringSimulation`, `ScrollSpringSimulation` | `physics/spring_simulation.dart` | closed-form springs (f64; GPUI's `SpringConfig` steps f32 state instead) |
| `BouncingScrollSimulation<F>` | `widgets/scroll_simulation.dart` | iOS fling: friction `F`, then a spring once it would leave the range |
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
starts it again, as in Flutter. Slops: Flutter's (`TOUCH_SLOP` 18, pan 36), or `touch_slop` for the platform's.

## Overscroll: how to drive it

- **Drag in overscroll:** pass the drag delta through `BouncingScrollPhysics::apply_physics_to_user_offset`.
- **Release:** `create_ballistic_simulation::<F>(metrics, velocity)`, then read `x(t)` / `is_done(t)` per frame
  (seconds since release).
- **Clamping platforms:** `ClampingScrollPhysics::apply_boundary_conditions` gives the overscroll a move refuses;
  feed it to the indicators.
- **Indicators:** send each `ScrollNotification` (start, update, overscroll with velocity or drag, end) with the
  `ScrollMetrics` to `GlowingOverscrollIndicator` or `StretchingOverscrollIndicator`; `dispatch` is where ancestors
  may disallow the indicator or set a glow's paint offset. Call `tick(now)` every frame with the frame time; draw
  from `GlowController::paint(width, height)` (leading edge at the top; rotate per edge) or `stretch_strength()`.

## Plug points for the merge and Wave 12

| Trait or type | Implemented by | Notes |
|---|---|---|
| `scale::PointerVelocityTracker` | gestures.rs's `VelocityTracker` (in scale.rs) | Wave 11's port of Flutter's `VelocityTracker` (agent A) replaces that impl |
| `overscroll::FrictionSimulation` | nothing yet (tests use a constant-velocity stand-in) | the `FrictionSimulation` port (agent B, patch 0012) implements it; its rows then check Flutter's friction |
| `overscroll::Simulation`, `Tolerance` | the springs and `BouncingScrollSimulation` | merge with patch 0012's simulation trait, keeping one |
| scale constants (`TOUCH_SLOP`, `MIN_FLING_VELOCITY`, ...) | scale.rs | merge with other ports of `constants.dart` |

Wave 12 wires them: the scale recognizer into touch dispatch next to the upstream `TouchGestureRecognizer`
(claims as the arena; scale results to `on_pinch` or a new listener), bouncing physics and the indicators into
GPUI's scroll containers, chosen per target from the platform behaviour table
([AGENTS.md](../AGENTS.md#decisions-settled-do-not-reopen-without-the-owner), "Platform feel").
