# Text selection (Android and touch desktops)

Flutter's text selection behaviour, ported as a pure state machine in `parallax-ext` (published, and re-exported by
the facade as `parallax::parallax_ext`)
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
2. Implement `TextLayout` for your laid-out text: offset ↔ point, word, line and paragraph ranges, caret rect (with
   its `Affinity` at a soft wrap: `Upstream` is the line before's end), line height, bounds, all in window
   coordinates and in your own offset unit.
3. Feed every `Input` with a `Field` (selection, focus, single or multi line, `TextVisibility::Obscured` for a
   password, screen): GPUI's mouse down (`Down`, click count as `tap_count`, `window.pointer_source()` as the
   device), pressed moves and ups, right click (`SecondaryTap`), the `LongPressEvent` phases (claim `Started` with
   `prevent_default`), a touch-down `TouchDragEvent` in a handle's `hit` rect (claim it, send `HandleDown`, then
   `HandleMove` / `HandleUp`: the engine tells a handle tap from a drag), and `Typed` for keyboard or IME changes.
   For finger drags on the text, compete for the touch-down offer with
   `window.compete_for_touch_drag(selection.touch_drag_kind())` ([touch-drag.md](touch-drag.md)); when the drag
   wins (`TouchDragStart::PastSlop`), send `DragWon` (where it went down and where it won), then `Move`s and `Up`.
   A `Cancelled` touch is `Cancel`: it never places the caret.
4. Apply the `Effects`: the selection, `request_keyboard` (focus and show the keyboard), the haptic.
5. Draw `overlay()`: `handles` (`rect`, `kind`: which quadrant points at the text, `hit`: at least 48 square),
   `toolbar` anchors (place it with `toolbar_origin`, its buttons from `toolbar_buttons`), `magnifier` (`rect`;
   animate a `jumped_line` move over `jumpBetweenLinesAnimationDuration`).

[hello's text field](../examples/hello/src/text_field.rs) is the worked example: handles as a circle plus a square,
the loupe as a bordered pill (GPUI can't magnify what's behind it), a Cut / Copy / Paste / Select all toolbar.

## The engine

One `Gesture` enum (`Idle`, `Pressed`, `Dragging`, `LongPress`, `HandlePress`, `HandleDrag`), each holding its own
data, and one
`match (gesture, input)` in `TextSelection::handle` with every pair, rejected ones last. Each step calls Flutter's
handlers, named after them (`on_tap_down`, `tap_up`, `drag_update`, `handle_drag_update`, …), and
`RenderEditable`'s selection helpers (`select_words_in_range`, `word_at_offset`). Overlay visibility follows
`TextField._shouldShowSelectionHandles`: handles only for a finger or stylus, never a mouse. A handle touch is
Flutter's `_SelectionHandleOverlay` arena: released within the touch slop it taps the handle (a caret's toolbar
toggles, Material's `_handleSelectionHandleTapped`); past it the pan wins and the drag and magnifier start there.
Obscured text is one word (`getWordAtOffset`), and `toolbar_buttons` never offers Cut or Copy for it.

GPUI differences: a touch tap reaches the field as mouse down then up, both at release (GPUI recognizes taps on
release), so a touch's `onTapDown` runs late; Android sets the selection on tap up anyway. A long press runs
`onTapDown` first, as Flutter's recognizer does. The drag slop is the row's recognizer's (`gpui::slops`) at the device's touch slop, `Input::Down::touch_slop` (the
window's `GestureTuning::touch_slop`) for a mouse drag; a touch drag GPUI's recognizer already won arrives as
`DragWon` and starts at once, whatever this slop says.

## Per-platform rules

`SelectionRules` in the behaviour table ([behaviour.md](behaviour.md)): tap edge, shift select, long press, tap count
folding, finger drag, drag recognizer, triple tap boundary, secondary tap, handle drag, magnifier, handle size, desktop
drag hiding the toolbar (`DragToolbar`), Select all (`SelectAllButton`). macOS takes the Apple arms. Flutter's desktop controls draw no handles; the desktop rows
take Material's 22 px handle so a touch can grab one. The handle-drag click is `haptics.selection_handle_drag`.

Tests, live checks under Weston, what is not ported and other implementations:
[text-selection-tests.md](text-selection-tests.md).
