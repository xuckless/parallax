# Core patches

Three patches to GPUI core: pointer device tags, opting an element out of touch → mouse, and keyboard data for text
input. The rules they follow are in [AGENTS.md](../AGENTS.md#upstream-and-patch-rules) (the "Touch → mouse" row under
Decisions); how to write and translate a patch is in [patches.md](patches.md). Apps get these patches through the
`parallax-gpui*` fork crates ([gpui-fork-crates.md](gpui-fork-crates.md)).

## 0003: pointer source

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
  - back gesture by its `BackGestureSource`: `EdgeSwipe` is `Touch`, synthesized; `Key` is `None`, like a key;
  - the platform's long press and touch drag (no backend sends them; they name no touch): `Touch`, synthesized.
  Taps, pans and long presses that the core recognizer builds carry the device of the touch they come from,
  synthesized, including ones fired later by timers (long press, momentum): the recognizer's per-touch state records
  it, so a pen and a finger down together each label their own. The value is saved and restored around each dispatch.
- **How backends set it:** they set `TouchEvent::device` and `BackGestureEvent::source`. Mouse input needs nothing.
  - iOS maps `UITouch.type` (`touch_device`, added to `gpui_ios`'s `events.rs`): Pencil is `Pen`, an iPad pointer
    is `Mouse`. Its edge swipe is `EdgeSwipe`; Android's Back key is `Key` ([android-keys.md](android-keys.md#back)).
  - Web touches are `Touch`.
  - Android maps Flutter's device kind through the `DEVICE_KINDS` table in
    [pointer.rs](../crates/android/src/pointer.rs) ([android-touch.md](android-touch.md)): stylus and inverted stylus
    are `Pen`, mouse and trackpad are `Mouse`.
  - Later patches that build a `TouchEvent` or `BackGestureEvent` literal must set the new field (0006's test does).
- **Limit:** web pens arrive on the mouse path ([events.rs](../upstream/gpui-pre-web/src/events.rs) treats only
  `pointerType == "touch"` as a touch), so they read as `Mouse`. Fixing that needs a device field on mouse events,
  which means ~95 struct literals across every backend. That is why 0003 is a per-dispatch query, not a field.

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
`UITextContentType`. Android reads the fields in the text-input port ([android-text-input.md](android-text-input.md#configuration)). iOS and web don't read them yet.

## Tests

Table-driven tests on GPUI's test platform: 0003 `pointer_source_names_the_device_and_synthesis_of_each_event`
(window.rs), 0004 `touch_listeners_opt_an_element_out_of_touch_to_mouse` (div.rs, one row per element tree and
gesture), 0005 the existing `text_input_configuration_and_focus_state_are_forwarded_on_change` (input.rs).

The packaged crate can't run its own lib tests as shipped: `svg_renderer.rs` includes two fonts from Zed's
`assets/`, and `style.rs` tests use `util_macros`, which is not vendored. To run them, copy `build/parallax-gpui/`,
add any two `.ttf` files at the font paths, drop the `#[perf]` lines, then run
`cargo test -p parallax-gpui --features test-support --lib` (the three `svg_renderer` font tests fail on stand-in
fonts).
