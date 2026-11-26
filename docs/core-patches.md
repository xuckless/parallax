# Core patches

Patches to GPUI core: pointer device tags (0003, and 0008 for back gestures), opting an element out of touch →
mouse, and keyboard data for text input. Rules: [AGENTS.md](../AGENTS.md#upstream-and-patch-rules) ("Touch → mouse"
under Decisions); how to write a patch: [patches.md](patches.md); apps get them via [fork crates](gpui-fork-crates.md).

## 0003: pointer source

`Window::pointer_source()` names the device of the pointer event being dispatched (mouse, touch, pen) and whether it
was synthesized: [pointer-source.md](pointer-source.md).

## 0004: touch opt-out

[0004-touch-opt-out.patch](../patches/0004-touch-opt-out.patch). It needs 0003.

- **API:** `.on_touch(|event: &TouchEvent, window, cx| …)` on an element with an id (`StatefulInteractiveElement` /
  `Interactivity`). A touch it claims reaches this listener in every phase, even after the finger or the element
  moves, and never becomes mouse, click, scroll or long-press events.
- **Deepest wins** (Flutter's gesture arena, simplified): the frontmost element under a starting touch that handles
  touches, taps, presses, drags or scrolls decides. `on_touch` claims it; the others leave it to the upstream
  recognizer. A button or scroll container inside an `on_touch` canvas still works; nested `on_touch`: inner wins.
- **Mechanism:** `TouchEvent` is a `MouseEvent` now. One `TouchRoute` per touch id: `Deciding` during `Started`
  (first decision wins; bubble order visits the frontmost element first), then `Gestures` or
  `Claimed(GlobalElementId)`. Later phases of a claimed touch reach the owner by `Window::touch_owner`, not by hit
  test. Custom elements decide from a `TouchEvent` listener: `Window::claim_touch` to own the touch, or
  `Window::leave_touch_to_gestures` when they handle taps or scrolls themselves (as divs do), so an ancestor's
  `on_touch` cannot take it. The window's mouse position
  does not move. After a claimed touch ends the window draws if dirty, as after a tap, so the web keyboard opens for
  a field focused in an `Ended` handler.
- **Taps click as touches:** div's `on_click` now receives the declared `ClickEvent::Touch(TouchClickEvent)` when the
  press and release were synthesized from a tap. `ClickEvent::Mouse` is only for real mouse clicks.

## 0005: text input configuration

[0005-text-input-keyboard-type.patch](../patches/0005-text-input-keyboard-type.patch). Data only: backends read these
fields from `TextInputConfiguration` in `set_text_input_configuration`.

| Field | Type | Values |
|---|---|---|
| `keyboard_type` | `KeyboardType` | `Text` (default), `Number`, `Decimal`, `Phone`, `Email`, `Url`, `Search`: the HTML `inputmode` set |
| `secure` | `bool` | password entry: hidden, no suggestions |
| `autofill` | `Option<AutofillHint>` | `Username`, `Password`, `NewPassword`, `OneTimeCode`, `Email`, `Phone`, `Name`, `PostalAddress`, `PostalCode`, `CreditCardNumber` |

Each `AutofillHint` maps to an HTML `autocomplete` token, an Android `AUTOFILL_HINT_*` and an iOS
`UITextContentType`. Android reads the fields in the text-input port ([android-text-input.md](android-text-input.md#configuration)),
autofill included ([android-autofill.md](android-autofill.md)). iOS and web don't read them yet.

## 0010: composition ends on focus loss

[0010-end-composition-on-focus-loss.patch](../patches/0010-end-composition-on-focus-loss.patch). No app API. When a
draw finds that focus left the field whose input handler it holds (blur, or another element), it calls the field's
`unmark_text` if it has marked text, before the handler is dropped: the text stays, the underline goes, and no stale
composing range reaches the next IME session ([android-text-input.md](android-text-input.md)). It runs in the draw
because a tap's focus change is drawn inside the touch dispatch, before any platform call; core's own deferred
`end_composition_before_focusing_native_view` (native views only) would come too late there.

When it unmarked something it calls `PlatformWindow::end_composition` (no-op by default), so the native IME drops its
composition too and doesn't type it again: macOS `discardMarkedText`; web blurs and refocuses the hidden IME mirror if
the browser is composing. iOS and Android keep the no-op: a field change or blur already restarts their IME session.

## Tests

Table-driven tests on GPUI's test platform: 0003 `pointer_source_names_the_device_and_synthesis_of_each_event`
(window.rs), 0004 `touch_listeners_opt_an_element_out_of_touch_to_mouse` (div.rs, one row per element tree and
gesture), 0005 the existing `text_input_configuration_and_focus_state_are_forwarded_on_change` (input.rs), 0010
`focus_moves_end_the_left_fields_composition` (input.rs, one row per focus move, a tap among them; it counts the
test platform's `end_composition` calls).

Run them with [`scripts/patch-test.sh`](../scripts/patch-test.sh) ([patch-tests.md](patch-tests.md)); CI does too.
