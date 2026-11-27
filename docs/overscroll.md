# Overscroll in scroll containers

What a GPUI scroll container does when a touch drags or flings it past an edge. [Patch
0017](../patches/0017-overscroll-in-scroll-containers.patch) wires the physics of [0012](scroll-physics.md) and the
indicators of [0015](scale-and-overscroll.md) into `div`'s scroll offset; the glue is a port of Flutter's
`ScrollPosition` (ledger rows: [PORTED.md](../third_party/flutter/PORTED.md)). Why port: [flutter.md](flutter.md).

## Choosing it

`gpui::overscroll::Overscroll` is one value, read from one place: the window's `GestureTuning::overscroll`, which
the [behaviour table](behaviour.md) fills per target (`Behaviour::gesture_tuning`). A container may set its own.

| Target row | `overscroll` | Physics (`scroll_physics`) | Past an edge |
|---|---|---|---|
| iOS, macOS | `Bounce` | `Bouncing` | the content follows the finger with Flutter's friction and springs back; a fling bounces |
| Android | `Stretch` | `Clamping` | the content holds the edge and stretches along the axis, then recedes |
| Windows, Linux | `None` | `Clamping` | a hard stop, as GPUI always did |
| Web | its host OS's row | | |

The physics decides whether the offset may leave its range (bouncing does, clamping refuses); `overscroll` picks the
indicator the refused overscroll drives (`Stretch`, `Glow`: Material 2's glow, no row uses it) or none (`Bounce`).
`None` skips all of it: today's clamp, with the recognizer's own momentum. `GestureTuning::default()` is `None`, so
GPUI without Parallax is unchanged.

```rust
// Per container: opt out, or pick another indicator.
div().id("list").overflow_y_scroll().overscroll(Overscroll::None)
// Per app: override the row, after the entry point applied it (docs/behaviour.md, "Override").
cx.set_gesture_tuning(GestureTuning { overscroll: Overscroll::Glow, ..current_behaviour()?.gesture_tuning() });
```

## How it works

| Piece | Where | Flutter |
|---|---|---|
| `AxisPosition`: one `Activity` (`Idle`, `Drag`, `Ballistic`) and one `Indicator` (`Off`, `Stretch`, `Glow`) per axis | `overscroll/scroll_position.rs` | `ScrollPosition`, its activities |
| `handle_drag`: touch scroll phase × activity → next; drag steps through `apply_physics_to_user_offset` and `set_pixels` | same | `applyUserOffset`, `setPixels`, `ScrollDragController.end` |
| `tick`: the ballistic simulation per frame; a move the boundary refuses ends it and hits the indicator | same | `BallisticScrollActivity._tick`, `applyMoveTo` |
| stretch: each child moves as a `1 + abs(strength)` scale about the stretched edge would move its centre | `div.rs` prepaint | `StretchEffect`'s `Transform` fallback |
| glow: `GlowController::paint` per edge, mirrored or transposed from the top edge | `div.rs` paint, `glow_shapes` | `_GlowingOverscrollIndicatorPainter` |

- **The release:** the pan's last scroll step reads the recognizer's release velocity (`Window::touch_fling_velocity`,
  zero unless Flutter's `isFlingGesture` passed) and starts `create_ballistic_simulation` with the container's own
  extents. The recognizer's momentum steps that follow are ignored by an overscrolling container.
- **Frames:** prepaint ticks the position, leaves an axis it moves unclamped, and requests the next frame while a
  simulation or an indicator animates.
- **Wheels and trackpads** (`ScrollSource::Pointer`) stop a drag or fling and never overscroll, as in Flutter.

## Limits

- GPUI has no scale transform for every primitive, so the stretch moves children but does not resize them; a
  container with one child stretches only by moving it.
- `uniform_list` and `list` stop hard: their visible range assumes an in-range offset.
- Nested containers each overscroll on their own axes; a pan reaches every container under its start (upstream
  routing), so same-axis nesting moves both. `OverscrollIndicatorNotification` has no receiver (every indicator
  shows). The glow tracks the pan's start across the axis (scroll events carry the start position).

## Tests

`overscroll::scroll_position::tests::touch_overscroll_follows_flutter`: one row per behaviour, replaying a touch trace
([gesture-recognizers.md](gesture-recognizers.md#trace-format)) frame by frame through the test platform and checking
the offsets, the stretch (a row's rendered position) and the painted glows. Run `scripts/patch-test.sh
touch_overscroll` ([patch-tests.md](patch-tests.md)). To add a behaviour, add a row.
