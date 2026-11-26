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
  being dispatched, and `None` during key events, render, and after the event returns. Read it inside a listener.
- **How core sets it:** in `Window::dispatch_event`, from the `pointer_source` table that the patch adds to
  `window.rs`, with one arm per `PlatformInput` variant:
  - backend mouse, scroll, pinch and file-drop input: `Mouse`, not synthesized;
  - `Touch`: its own `device`, not synthesized;
  - long press, touch drag and back gesture: the latest touch's device, synthesized.
  Taps, pans and long presses that the core recognizer builds are the latest touch's device, synthesized, including
  ones fired later by timers (long press, momentum). The value is saved and restored around each dispatch.
- **How backends set it:** they set only `TouchEvent::device`. Mouse input needs nothing.
  - iOS maps `UITouch.type` (`touch_device`, added to `gpui_ios`'s `events.rs`): Pencil is `Pen`, an iPad pointer
    is `Mouse`.
  - Web touches are `Touch`.
  - Android maps Flutter's device kind through the `DEVICES` table in
    [pointer.rs](../crates/android/src/pointer.rs) ([android-touch.md](android-touch.md)): stylus and inverted stylus
    are `Pen`, mouse and trackpad are `Mouse`.
- **Limit:** web pens arrive on the mouse path ([events.rs](../upstream/gpui-pre-web/src/events.rs) treats only
  `pointerType == "touch"` as a touch), so they read as `Mouse`. Fixing that needs a device field on mouse events,
  which means ~95 struct literals across every backend. That is why 0003 is a per-dispatch query, not a field.

## 0004: touch opt-out

[0004-touch-opt-out.patch](../patches/0004-touch-opt-out.patch). It needs 0003.

- **API:** `.on_touch(|event: &TouchEvent, window, cx| …)` on any interactive element (`InteractiveElement` /
  `Interactivity`). Touches that start over the element reach this listener in every phase. They never become mouse,
  click, scroll or long-press events, even after the finger moves off the element.
- **Mechanism:** `TouchEvent` is now a `MouseEvent`, so `Window::on_mouse_event::<TouchEvent>` works for custom
  elements too. Each started touch goes to those listeners first. One that calls `window.prevent_default()` claims
  the touch, the same rule touch drag and long press use (`on_touch` does it for you). The window keeps one
  `TouchRoute` per touch id:
  - `Gestures`: the upstream recognizer handles it;
  - `Listeners { start }`: every later event is hit-tested at `start`.
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
`UITextContentType`. Android reads the fields in the text-input port. iOS and web don't read them yet.

## Tests

Each patch carries table-driven tests on GPUI's test platform:
- 0003: `pointer_source_names_the_device_and_synthesis_of_each_event` (window.rs);
- 0004: `touch_listeners_opt_an_element_out_of_touch_to_mouse` (div.rs);
- 0005: the existing `text_input_configuration_and_focus_state_are_forwarded_on_change` (input.rs).

The packaged crate can't run its own lib tests as shipped: `svg_renderer.rs` includes two fonts from Zed's
`assets/`, and `style.rs` tests use `util_macros`, which is not vendored. To run them, copy `build/parallax-gpui/`,
add any two `.ttf` files at the font paths, drop the `#[perf]` lines, then run
`cargo test -p parallax-gpui --features test-support --lib` (the three `svg_renderer` font tests fail on stand-in
fonts).
