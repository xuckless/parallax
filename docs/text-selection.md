# Text selection (Android and touch desktops)

Flutter's text selection behaviour, ported as a pure state machine in `parallax-ext`
([text_selection/mod.rs](../crates/ext/src/text_selection/mod.rs), geometry in
[geometry.rs](../crates/ext/src/text_selection/geometry.rs)). Pointer input in; the new selection and the overlay
(handles, toolbar anchors, magnifier) out, as plain values. The app draws the overlay itself; there is no Material or
Cupertino look. iOS keeps UIKit's `UITextInteraction` (AGENTS.md, "iOS text selection",
[ios-text-selection.md](ios-text-selection.md)): its behaviour row says `TextSelectionControls::Native`, so
`TextSelection::for_behaviour` returns `None` there. Sources and dropped parts:
[PORTED.md](../third_party/flutter/PORTED.md).

## How to use it

1. Make one `TextSelection` per field: `TextSelection::for_behaviour(&current_behaviour()?)`. `None`: native
   selection, keep your own tap handling.
2. Implement `TextLayout` for your laid-out text: offset ↔ point, word, line and paragraph ranges, caret rect, line
   height, bounds, all in window coordinates and in your own offset unit.
3. Feed every `Input` with a `Field` (selection, focus, single or multi line, screen): GPUI's mouse down (`Down`,
   click count as `tap_count`, `window.pointer_source()` as the device), pressed moves and ups, right click
   (`SecondaryTap`), the `LongPressEvent` phases (claim `Started` with `prevent_default`), a claimed
   `TouchDragEvent` that starts in a handle's `hit` rect (`HandleDrag*`), and `Typed` for keyboard or IME changes.
   For finger drags on the text, compete for the touch-down offer with
   `window.compete_for_touch_drag(selection.touch_drag_kind())` ([touch-drag.md](touch-drag.md)); when the drag
   wins (`TouchDragStart::PastSlop`), send `Down` at its `start_position` (touch, one tap), then `Move`s and `Up`.
4. Apply the `Effects`: the selection, `request_keyboard` (focus and show the keyboard), the haptic.
5. Draw `overlay()`: `handles` (`rect`, `kind`: which quadrant points at the text, `hit`: at least 48 square),
   `toolbar` anchors (place it with `toolbar_origin`), `magnifier` (`rect`; animate a `jumped_line` move over
   `jumpBetweenLinesAnimationDuration`).

[hello's text field](../examples/hello/src/text_field.rs) is the worked example: handles as a circle plus a square,
the loupe as a bordered pill (GPUI can't magnify what's behind it), a Copy / Paste / Select all toolbar.

## The engine

One `Gesture` enum (`Idle`, `Pressed`, `Dragging`, `LongPress`, `HandleDrag`), each holding its own data, and one
`match (gesture, input)` in `TextSelection::handle` with every pair, rejected ones last. Each step calls Flutter's
handlers, named after them (`on_tap_down`, `tap_up`, `drag_update`, `handle_drag_update`, …), and
`RenderEditable`'s selection helpers (`select_words_in_range`, `word_at_offset`). Overlay visibility follows
`TextField._shouldShowSelectionHandles`: handles only for a finger or stylus, never a mouse.

GPUI differences: a touch tap reaches the field as mouse down then up, both at release (GPUI recognizes taps on
release), so a touch's `onTapDown` runs late; Android sets the selection on tap up anyway. A long press runs
`onTapDown` first, as Flutter's recognizer does. The drag slop is the row's recognizer's (`gpui::slops`) at the device's touch slop, `Input::Down::touch_slop` (the
window's `GestureTuning::touch_slop`), so a drag GPUI hands over past the slop starts as a drag here too.

## Per-platform rules

`SelectionRules` in the behaviour table ([behaviour.md](behaviour.md)): tap edge, shift select, long press, tap count
folding, finger drag, drag recognizer, triple tap boundary, secondary tap, handle drag, magnifier, handle size, desktop
drag hiding the toolbar. macOS takes the Apple arms. Flutter's desktop controls draw no handles; the desktop rows
take Material's 22 px handle so a touch can grab one. The handle-drag click is `haptics.selection_handle_drag`.

## Tests

[tests.rs](../crates/ext/src/text_selection/tests.rs): 48 gesture rows (Flutter test names from
`text_selection_test.dart` and `text_field_test.dart`, on a 10 px monospace layout), handle geometry, `_getHandleDy`,
toolbar anchors and placement, the magnifier's positioning tests and `shiftWithinBounds`. `text_selection_table` in
[behaviour.rs](../crates/platform/src/behaviour.rs) checks the rows. hello's `mouse_selects_by_the_desktop_row` drives
the field through GPUI's test platform with mouse events, `touch_selects_by_the_android_row` with touches on
Android's row (tap, long press, a horizontal finger drag moving the caret, a vertical drag left to the pan).

## Live checks

[weston-selection-checks.sh](../scripts/weston-selection-checks.sh) runs hello under Weston's headless backend
([traces-desktop.md](traces-desktop.md#linux-weston-headless), CI's `linux-touch` job), types `abc def ghi` into the
first field, finds its glyphs on screen (`locate-colour.py ffffff ink`), then sends real mouse and `wl_touch` input
through Weston's test plugin: click, click left of the text, double and triple click, a drag backwards, a double-click
drag (word by word), a shift-click, a touch long press, and a long press then an end-handle drag. Each row checks
hello's last `text field selection: base..extent` log line (offsets only, never the text). Run it with
`WESTON_RUN="podman exec <container>"` where Weston's build dependencies live in a container; add a check as a row.

## Not ported (yet)

- A finger drag right after a double tap (word by word): GPUI's won drag carries no tap count.
- Scrolling fields (drag offsets, edge scroll), read-only fields, obscured text, right-to-left handles, spell check,
  force press, the floating cursor, handle taps, the toolbar's own buttons and styles (Material's).
- Web on an iOS host takes the iOS row, so it has no selection gestures.

## Other implementations

longbridge/gpui-component (`crates/base/src/input/base/touch.rs`, `crates/component/src/touch_selection/`, read at
`0d9850db`) has touch selection for its own `InputState`: long press selects a word, handles, an edit menu. It is
tied to its input widget, has no magnifier or per-platform rules, and lets handles cross (Flutter's Android forbids
that), so nothing was taken from it.
