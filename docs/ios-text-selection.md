# iOS text selection: UIKit's own

On iOS a focused GPUI field selects text the way a `UITextField` does: UIKit draws the caret, highlight, handles and
loupe and shows its edit menu. Patch [0038](../patches/0038-ios-text-interaction.patch) adds a `UITextInteraction`
to gpui_ios's text input view ([text_input.rs](../upstream/gpui-pre-ios/src/ios/text_input.rs) plus the patch);
the edit menu calls patch [0036](../patches/0036-ios-edit-actions.patch)'s actions. The behaviour table marks iOS
`TextSelectionControls::Native` ([behaviour.md](behaviour.md)), so Parallax's own engine
([text-selection.md](text-selection.md)) stays off there (AGENTS.md, "iOS text selection").

## What the user gets

| Gesture | UIKit does | The app sees |
|---|---|---|
| Tap an unfocused field | nothing (no interaction yet) | the tap: the field focuses and asks for the keyboard |
| Tap the focused field's text | places the caret; a tap on the caret shows the edit menu | `set_selected_text_range` |
| Double tap | selects the word (`tokenizer`), shows handles and the menu | `set_selected_text_range` |
| Drag a handle | extends the selection | `set_selected_text_range` per move |
| Long press | the loupe over the text, the caret following | `set_selected_text_range` |
| Edit menu | Cut, Copy, Paste, Delete, Select All, AutoFill, … | `replace_text_in_range` / selection changes |
| Anything outside the text | nothing | the touch, as before |

Hardware shortcuts (Cmd+X/C/V/A) that GPUI leaves unhandled go to UIKit's text system
([ios-keyboard.md](ios-keyboard.md)), which sends the same actions; idb sends no chords, so the owner's iPad check
confirms it.

## How it works

- **Interaction lifetime.** `sync_text_interaction` adds a `UITextInteraction` (editable mode, as Flutter's
  `-[FlutterTextInputView textInteraction]` makes it) when the view becomes first responder and removes it when the
  view resigns. One `Option` holds it; `reconcileTextInput` and `resignFirstResponder` call the sync.
- **Where touches go.** UIKit puts the interaction's gestures on the text input's own view and only works when that
  view is the `textInputView`. So the view now covers GPUI's view (the window sizes it) and is its own
  `textInputView`; `hitTest:withEvent:` claims a touch only while editing and only on the field's text (inside the
  app's `selectionRectsForRange:` rects for the whole document) or within 22 pt (half of Apple's 44 pt target) of a
  selection end, where the handle knobs are. Not `closestPositionToPoint:`: an app may clamp
  `character_index_for_point` to the nearest character for any point (upstream's examples/input.rs does), which
  would claim every touch on the screen. Every
  other touch falls through to GPUI's view. Touches it claims but the gestures leave are forwarded to GPUI's view
  (Flutter's `touchesBegan:` … forwarding); a recognized gesture cancels them there.
- **Geometry** comes from the app's input handler, as for the IME: `caretRectForPosition:`, `firstRectForRange:`,
  `closestPositionToPoint:`, and `selectionRectsForRange:` from `selection_bounds_for_range`, falling back to
  `bounds_for_range` when the app gives none (fine for one-line fields; multi-line fields should give per-line rects).
- **Edit actions** tell the input delegate afterwards (`action_changed`), so UIKit's handles follow a paste or cut.

## For app authors

UIKit draws its caret and highlight over GPUI's; an app may draw its own as well (hello does) or leave them to
UIKit on iOS. Give `selection_bounds_for_range` for multi-line text. Read-only text has no interaction (the field
must take text input).

## Checks

The protocol smoke test (patch 0028's harness) checks the interaction comes and goes with the responder; the oracle
table covers the actions ([ios-text-input.md](ios-text-input.md)). The gestures were checked in hello on the iPhone
17 Pro and iPad Pro 11-inch simulators (iOS 26.5) with idb: double tap, handle drag, Copy then Paste, the loupe, a
tap on a button outside the field. A real iPad run is the owner's ([ROADMAP](../ROADMAP.md), Phase 4 follow-ups).

## Not done

- Read-only (non-editable) selection: GPUI has no read-only text input handler.
- Flutter's framework-driven `UIEditMenuInteraction` (custom menu items, `showEditMenu`): GPUI has no API to ask for
  a menu, so UIKit's own menu is the only one.
- Scribble (`UIScribbleInteraction`) and the floating cursor.
