# Touch and mouse: the rule and its opt-out

A guide for app code. The rule ("Touch → mouse" under Decisions in [AGENTS.md](../AGENTS.md)) is the same on every
target, phones and touchscreen desktops alike: **touch is turned into mouse input by default, and an element opts
out with `.on_touch`; the deepest element that handles a touch decides.** So a GPUI app written for the mouse works
on a phone unchanged, and an element that wants raw fingers asks for them.

## By default: touch acts as a mouse

GPUI's touch recognizer (upstream's, tuned with Flutter's gesture rules: [gesture-recognizers.md](gesture-recognizers.md))
turns each touch into what a mouse would have sent:

| The finger does | The app gets | Notes |
|---|---|---|
| Tap | mouse down/up, then `on_click` with `ClickEvent::Touch` | double taps count as `click_count` 2 |
| Pan | `on_scroll_wheel` with touch phases; scroll containers scroll with the platform's physics and overscroll | [scroll-physics.md](scroll-physics.md), [overscroll.md](overscroll.md) |
| Long press | a long press (tooltips show on it) and the platform's haptic when an element claims it | [haptics.md](haptics.md) |
| Two fingers | `on_pinch` on the element under them | [pinch.md](pinch.md) |
| Drag a handle or a slider | the element claims or competes for the drag | [touch-drag.md](touch-drag.md) |

Pens behave like a finger (with `force`); a real mouse is untouched. On Windows a hovering pen moves the mouse.

## Asking where an event came from

Inside any pointer listener, `window.pointer_source()` says which device sent it and whether GPUI synthesized it:

```rust
.on_click(|event, window, _| match event {
    ClickEvent::Touch(_) => log::info!("tapped"),           // a finger or pen tap
    ClickEvent::Mouse(_) => log::info!("clicked: {:?}", window.pointer_source()),
    ClickEvent::Keyboard(_) => log::info!("pressed with a key"),
})
```

`PointerSource { device: Mouse | Touch | Pen, synthesized }` is `Some` only while a pointer event is dispatched.
Per-platform details and limits (web pens read as `Mouse`): [pointer-source.md](pointer-source.md).

## Opting out: `.on_touch`

```rust
div().id("canvas").on_touch(|event: &TouchEvent, window, cx| {
    // every phase of every finger that started here: Started, Moved, Ended, Cancelled
    log::info!("{:?} {:?} at {:?}", event.id, event.phase, event.position);
})
```

A touch the element claims reaches only this listener, in every phase, even after the finger leaves the element,
and is never turned into clicks, scrolls or long presses. The element needs an `id`.

**Deepest wins:** when a touch starts, the frontmost element under it that handles touches, taps, presses, drags or
scrolls decides. So:

- a button or a scroll container inside an `on_touch` canvas still clicks and scrolls;
- nested `on_touch` elements: the inner one gets the touch;
- a custom element that handles taps itself calls `window.leave_touch_to_gestures()`, so an ancestor's `on_touch`
  cannot take its touches.

hello's trace pad is a full-window `on_touch` element ([lib.rs](../examples/hello/src/lib.rs), `trace_pad`). The
mechanism (touch routes, `Window::claim_touch`): [core-patches.md](core-patches.md#0004-touch-opt-out).

## Feel per platform

How far a finger moves before a tap becomes a pan, how a fling decays, which overscroll shows: one row per target,
applied for you. [platform-feel.md](platform-feel.md).
