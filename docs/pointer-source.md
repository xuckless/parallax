# Patch 0003: pointer source

One of the [core patches](core-patches.md): every pointer event names its device and whether it was synthesized
("Touch → mouse" under Decisions in [AGENTS.md](../AGENTS.md)).

[0003-pointer-source.patch](../patches/0003-pointer-source.patch). Upstream turns taps into mouse presses and pans
into scroll events, and the only clue it leaves is the window-wide `last_input_was_touch()`.

- **API:** `Window::pointer_source() -> Option<PointerSource>`, where `PointerSource { device: PointerDevice,
  synthesized: bool }` and `PointerDevice` is `Mouse`, `Touch` or `Pen`. It is `Some` only while a pointer event is
  being dispatched, and `None` during key events, render (even the draw a tap runs before its dispatch returns), and
  after the event returns. Read it inside a listener.
- **How core sets it:** in `Window::dispatch_event`, from the `pointer_source` table that the patch adds to
  `window.rs`, with one arm per `PlatformInput` variant:
  - backend mouse, scroll, pinch and file-drop input: `Mouse`, not synthesized;
  - `Touch`: its own `device`, not synthesized;
  - back gesture by its `BackGestureSource` ([patch 0008](../patches/0008-back-gesture-source.patch), no default):
    `EdgeSwipe` is `Touch`, synthesized; `Key` is `None`, like a key;
  - the platform's long press and touch drag (no backend sends them; they name no touch): `Touch`, synthesized.
  Taps, pans and long presses that the core recognizer builds carry the device of the touch they come from,
  synthesized, including ones fired later by timers (long press, momentum): the recognizer's per-touch state records
  it, so a pen and a finger down together each label their own. A fling keeps its device up to its closing scroll,
  even when another touch catches it. The value is saved and restored around each dispatch.
- **How backends set it:** they set `TouchEvent::device` and `BackGestureEvent::source`. Mouse input needs nothing.
  - iOS maps `UITouch.type` (`touch_device`, added to `gpui_ios`'s `events.rs`): Pencil is `Pen`, an iPad pointer
    is `Mouse`. Its edge swipe is `EdgeSwipe`; Android's Back key is `Key` ([android-keys.md](android-keys.md#back)).
  - macOS 27 direct touches (a touchscreen Mac, a Sidecar iPad) are `Touch` ([desktop-touch-macos.md](desktop-touch-macos.md)).
  - Web touches are `Touch`. Android maps Flutter's device kind through the `DEVICE_KINDS` table in
    [pointer.rs](../crates/android/src/pointer.rs) ([android-touch.md](android-touch.md)): stylus and inverted stylus
    are `Pen`; a mouse and trackpad send real mouse events instead ([android-mouse.md](android-mouse.md)).
  - Later patches that build a `TouchEvent` or `BackGestureEvent` literal set the new field (0008 does in 0006's test).
- **Limit:** web pens arrive on the mouse path ([events.rs](../upstream/gpui-pre-web/src/events.rs) treats only
  `pointerType == "touch"` as a touch), so they read as `Mouse`. Fixing that needs a device field on mouse events,
  which means ~95 struct literals across every backend. That is why 0003 is a per-dispatch query, not a field.
