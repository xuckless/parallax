# Android mouse and trackpad

How a mouse, a trackpad or a wheel becomes GPUI mouse and scroll events in [`parallax-android`](../crates/android/src/lib.rs).
The packet path (Java processor → JNI → decoder) is shared with touch: [android-touch.md](android-touch.md).

## The path

1. **Java, ported.** Button presses, drags and releases arrive in `ParallaxSurfaceView.onTouchEvent`; hover moves and
   wheel turns in `onGenericMotionEvent`, which hands them to Flutter's `AndroidTouchProcessor.onGenericMotionEvent`
   as `FlutterView` does ([ParallaxSurfaceView.java](../crates/android/java/io/github/xuckless/parallax/ParallaxSurfaceView.java)).
   The processor takes pointer-class `ACTION_HOVER_MOVE` and `ACTION_SCROLL` only: a hover is a `HOVER` record, a wheel
   is a `HOVER` with the `SCROLL` signal and Flutter's scroll deltas (Android's `ViewConfiguration` scroll factor, in
   pixels). Flutter's trackpad rule stays: a `SOURCE_MOUSE` press with no button is a pan, packed as
   `PAN_ZOOM_START/UPDATE/END` records of kind `TRACKPAD` with the offset from the pan's start.
2. **Routing.** [pointer.rs](../crates/android/src/pointer.rs) reads each record's kind through `DEVICE_KINDS`, which
   names a `Route`: `Touch(device)` (finger, pen, unknown) or `Mouse` (mouse, trackpad). A signal with an action
   (`SIGNAL_KINDS`: scroll → wheel) wins over the route, so a wheel turn scrolls whatever tool reports it.
3. **Mouse actions.** `MOUSE_ACTIONS` maps each change: down → `MouseDown`, move and hover → `MouseMove`, up →
   `MouseUp`, cancel → `MouseExited` (GPUI has no mouse cancel, and an up would click), pan/zoom → `ScrollEvent` in
   the matching `TouchPhase`, add and remove → nothing.
4. **Buttons.** `BUTTONS` maps Flutter's bits (Android's `BUTTON_*` masked to five) to GPUI's: primary → `Left`,
   secondary → `Right`, tertiary → `Middle`, back and forward → `Navigate`. A press's button is the bit that is new
   since the last record (none new: `Left`); a release's is the bit that went. A move reports the first held bit.
5. **Clicks.** `Pointers` keeps the last press. The next press of the same button within 300 ms (Android's double-tap
   timeout) and 5 logical pixels (gpui_web's radius; Android's double-tap slop is sized for fingers) counts one more
   click; packet time stamps (microseconds) are the clock, so it is testable. Down and up carry the count, so
   `on_click` gets `ClickEvent::Mouse` with `click_count` as on desktop (patch 0004: only taps are
   `ClickEvent::Touch`), and patch 0003's `pointer_source()` says `Mouse`, not synthesized.
6. **Scrolling.** A wheel is `ScrollDelta::Pixels`, Flutter's delta negated (Flutter's is how far to scroll, GPUI's how
   far the content moves, as gpui_web negates the DOM's), phase `Moved`. A trackpad pan sends the change in pan offset
   since the last record, at the pan's start position, phases `Started` / `Moved` / `Ended`. Every position is
   divided by the display density; the last one is the window's `mouse_position`.
7. **Modifiers** come from the keyboard ([android-keys.md](android-keys.md)), read before GPUI runs.

## Not yet

- **Hover enter and exit.** Flutter's processor ignores `ACTION_HOVER_ENTER/EXIT`, so `is_hovered` stays false and no
  `MouseExited` is sent when the cursor leaves the view.
- **A second button during a drag** (Android sends `ACTION_BUTTON_PRESS`, which the processor ignores) only changes the
  held button of the moves; it sends no down or up of its own.
- **Pointer icons** (`set_cursor_style`), pinch (Android reports no trackpad scale), and the right button as Back on
  devices whose firmware maps it.

## Changing it

- Reroute a device kind: its `DEVICE_KINDS` row. Remap a change: its `MOUSE_ACTIONS` row. A new button: a `BUTTONS`
  row. Each table has named test rows in `pointer::tests::mouse` and `devices`.
- **Check on the emulator:** `adb shell input mouse tap X Y` (a click), `adb shell input mouse swipe X1 Y1 X2 Y2` (a
  drag), `adb shell input mouse scroll X Y --axis VSCROLL,-1` (a wheel notch down). `Shell::pointer` logs each event
  at debug level, below `on_load`'s info level.
