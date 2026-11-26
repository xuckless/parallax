# Android autofill

How a field's `TextInputConfiguration::autofill` hint (patch 0005, [core-patches.md](core-patches.md#0005-text-input-configuration))
reaches Android's autofill service, and how a filled value gets back into GPUI. It rides on the text-input session:
[android-text-input.md](android-text-input.md). The Java is Flutter's autofill code, ported with the rest of
`TextInputPlugin` and `TextInputChannel` (ledger rows in [PORTED.md](../third_party/flutter/PORTED.md)).

## The path

1. **Rust → Java, with the new client.** [text_input.rs](../crates/android/src/text_input.rs) adds Flutter's
   `Configuration.Autofill` to `setClient` when the field has a hint and GPUI has named it (`autofill_json`):
   `uniqueIdentifier` is `parallax.field.<FocusId>`, so it is the same for the field's whole life and differs per
   field; `hints` is one Flutter `AutofillHints` name (`autofill_hint`, an exhaustive match, below); `editingValue`
   is the state sent with it. The calls then follow Flutter's `EditableText` attach: `setClient`,
   `setEditableSizeAndTransform` (the field's text bounds from `bounds_for_range`, read by `Shell` only for a field
   with a hint; at least 1 logical pixel wide, since an empty field's bounds are its 0-wide caret and Android takes
   a 0-wide `Rect` as empty), `setEditingState`, `show`, `requestAutofill` last. While that client lives, `Shell`
   reads the bounds every frame and `Session` sends `setEditableSizeAndTransform` again whenever they moved from
   the rect Java has (the field scrolled or resized under the keyboard), as `EditableText` does on layout.
2. **Java → AutofillManager, ported.** `ParallaxSurfaceView` is `IMPORTANT_FOR_AUTOFILL_YES`, as `FlutterView`.
   `TextInputPlugin` keeps the client's configuration per autofill id (`updateAutofillConfigurationIfNeeded`), saves
   the rect, calls `notifyViewEntered` on `requestAutofill`, `notifyViewExited` on hide, a new client and clear, and
   `notifyValueChanged` after each text change. `onProvideAutofillVirtualStructure` reports the field as a virtual
   child with its hints and rect.
3. **Filled values → GPUI, merged.** The service calls `ParallaxSurfaceView.autofill(values)`. For the focused field,
   `TextInputPlugin.autofill` sets the value on the IME's mirror, exactly like a keyboard edit, so it reaches GPUI
   through `nativeUpdateEditingState` and `Session::java_edit`'s merge (the sync protocol in
   [android-text-input.md](android-text-input.md#the-sync-protocol)), never past it.
4. **A late id.** GPUI installs a focused field's handler a frame after `FocusGained` (so on the API 35 emulator,
   every focus). The first `setClient` carries no autofill; when the id arrives, `Session::installed` announces the
   field again (a new client, with autofill) only if it has a hint, so plain fields keep one restart per focus.

## Hints

| GPUI `AutofillHint` | Flutter `AutofillHints` | Android hint (`Configuration.Autofill`) |
|---|---|---|
| `Username` / `Password` | `username` / `password` | `AUTOFILL_HINT_USERNAME` / `_PASSWORD` |
| `NewPassword` / `OneTimeCode` | `newPassword` / `oneTimeCode` | `newPassword` / `smsOTPCode` |
| `Email` / `Phone` | `email` / `telephoneNumber` | `AUTOFILL_HINT_EMAIL_ADDRESS` / `phoneNumber` |
| `Name` / `PostalAddress` | `name` / `fullStreetAddress` | `personName` / `streetAddress` |
| `PostalCode` / `CreditCardNumber` | `postalCode` / `creditCardNumber` | `AUTOFILL_HINT_POSTAL_CODE` / `_CREDIT_CARD_NUMBER` |

A new GPUI hint needs an arm in `autofill_hint` and a row in `text_input::tests::autofill_hints`.

## Not yet

- **Groups.** GPUI tells the platform about the focused field only, so `setClient` has no `fields` and the service sees
  one field at a time; `updateEditingStateWithTag` (other fields' values) only logs.
- **Save prompts.** Flutter commits the context from `TextInput.finishAutofillContext`, which GPUI has no API for; the
  ported handler is there, Rust never sends it.
- **Rect accuracy.** The rect is the field's text bounds, not its box (the input handler knows no more).

## Check it

`examples/hello`'s second field has `AutofillHint::Email` ([examples.md](examples.md)). On the emulator: set an
autofill service (`adb shell settings put secure autofill_service <component>`; Google's offers nothing without an
account, so a small test service that returns one dataset for `emailAddress` is the quickest check), tap the field:
the service's suggestion appears under it, and picking it fills the field. `adb shell dumpsys autofill` shows the session and the virtual child's hints. The calls and IME edits
log lengths and ranges, never text. Tests: `cargo test -p parallax-android text_input`.
