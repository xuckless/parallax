# Touch pinch

[Patch 0016](../patches/0016-touch-pinch.patch) feeds multi-touch `TouchEvent`s to Flutter's scale recognizer
([scale-and-overscroll.md](scale-and-overscroll.md), patch 0015) and delivers what it recognizes to `on_pinch`. Only
the glue is new; the maths is Flutter's ([AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth)).

## The API

Upstream's `on_pinch` and `PinchEvent`, so one listener handles trackpad and finger pinches:

```rust
div().id("photo").on_pinch(cx.listener(|this, event: &PinchEvent, _, cx| {
    this.zoom *= 1. + event.delta; // the scale's change since the last event: trackpad or fingers
    if let Some(TouchPinch::Update(update)) = &event.touch {
        this.angle = update.rotation; // radians since the gesture started
    }
    cx.notify();
}))
```

| `PinchEvent` field | Trackpad pinch | Touch pinch |
|---|---|---|
| `position` | under the pointer | the focal point (window pixels) |
| `delta` | platform zoom delta | `scale / previous scale - 1` (0 at start and end) |
| `phase` | the platform's | `TouchPinch::phase()`: `Started` / `Moved` / `Ended` |
| `touch` | `None` | `TouchPinch::Start(ScaleStartDetails)`, `Update(ScaleUpdateDetails)`, `End(ScaleEndDetails)` |
| `touch_owner()` | `None` | the element it is for |

The details are Flutter's (`gpui::scale`): focal point and `local_focal_point` (relative to the element's bounds),
`scale`, `horizontal_scale`, `vertical_scale`, `rotation` (radians, not normalised, as in Flutter), `pointer_count`,
`focal_point_delta`; `End` has the last finger's `velocity` (clamped to Flutter's fling limits) and
`scale_velocity`. A finger joining or leaving ends the gesture and the next move starts it again, as in Flutter.

**Touch pinches need an element id** (like `on_click`). Custom elements call `Window::join_pinch(touch, owner)`
at a touch's `Started` and check `touch_owner()`; backends build trackpad pinches with `PinchEvent::new(..)`.

## Who gets the fingers: Flutter's arena as claims

The window keeps one `PinchArena` (`gestures/pinch.rs` in the patch), one explicit state, fed the touch
recognizer's claim on its touch (`TouchClaim`: none, possible, pan, press):

| State | Meaning | Next |
|---|---|---|
| `Idle` | no finger on a pinch element, or the touch recognizer won them | a finger on one → `Single` (`Contested` alone while the recognizer is busy) |
| `Single` | one finger: the touch recognizer's (tap, pan, long press) | a second on it → `Contested`; it pans, or a long press is claimed → `Idle` |
| `Contested` | the scale has them, the touch recognizer still has its one | the scale passes its slop → `Won`; the drag or a claimed long press wins → `Idle`; back to the recognizer's one → `Single` |
| `Won` | the fingers reach only the pinch; the touch recognizer got `Cancelled` for them | last finger lifts → `Idle` |

- **Which element:** a touch joins every `on_pinch` element under it, deepest first (`TouchRoute::Gestures { pinch
  }`), unless a deeper `on_touch` claimed it ([core-patches.md](core-patches.md#0004-touch-opt-out)). The deepest
  pinches, unless an earlier finger pinches an ancestor: a finger on each of two nested pinch elements pinches the
  outer one. A button inside keeps its taps. One element pinches at a time per window.
- **A running pan never becomes a pinch:** one finger pans before the scale (hit slop 18 under its pan slop 36),
  and a finger that joins a pan is the pan's (`monodrag.dart` accepts a joining pointer).
- **Two fingers:** the scale wins past a span change of kScaleSlop (18) or focal travel of the pan slop (36; 16 on
  Android, `gpui::slops` from `GestureTuning::touch_slop`). The drag wins if the fingers' focal point travels the
  hit slop first: Flutter's drag adds up every pointer's movement, so fingers dragged together pan and fingers
  spread apart pinch.
- **Long press:** held past the deadline with an element claiming it, the long press wins every finger
  (`didExceedDeadline` accepts in each arena): no pinch until they lift. A finger that lands during a claimed long
  press is the scale's alone, a one-finger pinch, and the long press goes on.
- **Elsewhere:** without `on_pinch`, the first finger taps or pans and the rest are ignored, as before.

## Platforms: every finger arrives

| Platform | Multi-touch path |
|---|---|
| Android | `AndroidTouchProcessor` packs every pointer of each `MotionEvent`; `Pointers` gives each a fresh `TouchId` ([android-touch.md](android-touch.md)) |
| iOS | `gpui_ios` enables `multipleTouchEnabled` and sends one `TouchEvent` per `UITouch` |
| Web | `gpui_web` maps each touch `pointerId` to its own `TouchId`; the canvas has `touch-action: none`, so the browser does not zoom the page |

## Tests

`gestures::pinch::tests::touch_pinches_follow_flutters_arena` replays traces ([trace
format](gesture-recognizers.md#trace-format), `gpui::play_trace`) into nested `on_pinch` divs, one row per arena
outcome: pinch out and in, rotate, three fingers, a finger lifted mid-pinch, a running pan, fingers dragged together,
fingers spread apart, a held long press, a finger joining a long press, nested pinch elements, two-finger tap, one
finger left before the slop, an element without `on_pinch`. Run `scripts/patch-test.sh pinch`
([patch-tests.md](patch-tests.md)). The hello box ([examples.md](examples.md)) zooms with a pinch and shows its
rotation.
