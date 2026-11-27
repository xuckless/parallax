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
| `phase` | the platform's | `Started` / `Moved` / `Ended`, matching `touch` |
| `touch` | `None` | `TouchPinch::Start(ScaleStartDetails)`, `Update(ScaleUpdateDetails)`, `End(ScaleEndDetails)` |

The details are Flutter's (`gpui::scale`): focal point and `local_focal_point` (relative to the element's bounds),
`scale`, `horizontal_scale`, `vertical_scale`, `rotation` (radians, not normalised, as in Flutter), `pointer_count`,
`focal_point_delta`; `End` has the last finger's `velocity` (clamped to Flutter's fling limits) and
`scale_velocity`. A finger joining or leaving ends the gesture and the next move starts it again, as in Flutter.

**Touch pinches need an element id** (like `on_click`): later fingers and phases find the element by it. Without
one, `on_pinch` gets trackpad pinches only. Custom elements use `Window::join_pinch(touch, owner)` from a
`TouchEvent` listener at `Started`, and read `Window::pinch_owner()` in their `PinchEvent` listener.

## Who gets the fingers: Flutter's arena as claims

The window keeps one `PinchArena` (`gestures/pinch.rs` in the patch), one explicit state:

| State | Meaning | Next |
|---|---|---|
| `Idle` | no finger on a pinch element | a finger starts on one → `Single` |
| `Single` | one finger on the element: the touch recognizer's (tap, pan, long press) | a second finger on it → `Contested`; it lifts → `Idle` |
| `Contested` | two or more: the scale recognizer has them, so does the touch recognizer | the scale passes its slop → `Won`; back to one → `Single` |
| `Won` | the fingers reach only the pinch; the touch recognizer got `Cancelled` for them | last finger lifts → `Idle` |

- **Which element:** the deepest element with `on_pinch` under a starting finger (`TouchRoute::Gestures { pinch }`),
  unless a deeper `on_touch` claimed it ([core-patches.md](core-patches.md#0004-touch-opt-out)). A button inside
  the element keeps its taps and the pinch still joins. One element pinches at a time per window.
- **One finger pans, as today:** the touch recognizer's pan slop is below the scale's, so in Flutter's arena the
  pan wins a one-finger drag too. A second finger turns that pan into a pinch: the pan ends `Cancelled`.
- **Slops:** `gpui::slops` with the window's `GestureTuning::touch_slop` ([behaviour.md](behaviour.md)): span change
  over kScaleSlop (18) or focal point travel over the pan slop (36; 16 on Android).
- **Elsewhere:** an element without `on_pinch` keeps today's behaviour: the first finger taps or pans, the rest are
  ignored. A long press or touch drag in progress is cancelled when the pinch wins.

## Platforms: every finger arrives

| Platform | Multi-touch path |
|---|---|
| Android | `AndroidTouchProcessor` packs every pointer of each `MotionEvent`; `Pointers` gives each a fresh `TouchId` ([android-touch.md](android-touch.md)) |
| iOS | `gpui_ios` enables `multipleTouchEnabled` and sends one `TouchEvent` per `UITouch` |
| Web | `gpui_web` maps each touch `pointerId` to its own `TouchId`; the canvas has `touch-action: none`, so the browser does not zoom the page |

## Tests

`gestures::pinch::tests::touch_pinches_reach_the_deepest_on_pinch_element` replays recorded traces
([trace format](gesture-recognizers.md#trace-format)) into nested `on_pinch` divs: pinch out and in, rotate, three
fingers, a finger lifted mid-pinch, pan turned pinch, two-finger tap, one finger left before the slop, and an
element without `on_pinch`. Run `scripts/patch-test.sh pinch` ([patch-tests.md](patch-tests.md)). The hello box
([examples.md](examples.md)) zooms with a pinch and shows its rotation.
