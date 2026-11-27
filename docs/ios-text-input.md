# iOS text input: Flutter's oracle

Flutter's [`FlutterTextInputPluginTest.mm`](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/darwin/ios/framework/Source/FlutterTextInputPluginTest.mm)
cases run against upstream gpui_ios's `UITextInput` view ([text_input.rs](../upstream/gpui-pre-ios/src/ios/text_input.rs)).
Patch [0028](../patches/0028-ios-text-input-flutter-oracle.patch) adds the table; [0029](../patches/0029-ios-text-input-flutter-editing.patch),
[0030](../patches/0030-ios-keyboard-type-secure-entry.patch), [0036](../patches/0036-ios-edit-actions.patch) and
[0037](../patches/0037-ios-text-content-type.patch) port Flutter's fixes for the rows that failed. Selection UI:
[ios-text-selection.md](ios-text-selection.md).
Which cases are in scope: [oracle-inventory-text.md](audits/oracle-inventory-text.md#ios-phase-4).

## How the table runs

gpui_ios's text-input decisions are not plain functions: they live in the Objective-C methods of `GPUITextInputView`
and need UIKit. So the table runs as a UIKit binary on the iOS simulator, inside upstream's own harness
(`tests/text_input.rs`, `harness = false`, main thread):

- `text_input_flutter_oracle.rs` (included by `text_input.rs` under `cfg(test)`) holds the rows. A row is one Flutter
  test: its `UITextInput` calls and checks as `Step`s, in Flutter's order. To add a case, add a row; a new kind of
  call or check is a `Step` variant and one arm in `run_step`.
- Flutter's view holds the text; in GPUI the app does. A fake app (`Field`: upstream's `examples/input.rs` editing
  rules on UTF-16) answers the view's `PlatformInputHandler`, built on gpui's `TestAppContext` with 0028's
  test-only `PlatformInputHandler::for_focused_field`. Its `edits` count is Flutter's `updateEditingClient` count.
- Flutter's `setSelectionRects:` input becomes the app's geometry answers (`Answer`): in GPUI the app lays text out
  (`bounds_for_range` is `firstRect`), so the geometry rows check that the view passes them on unchanged.
- `Open` rows run and report without failing the run, and fail it once they pass (then mark them `Pass`).
- The binary has no app bundle, so iOS would ask the user before each read of the general pasteboard and the row
  would hang: under `cfg(test)` the edit actions use a named pasteboard (`pasteboard()` in `text_input.rs`).

Run it ([ios-text-oracle.sh](../scripts/ios-text-oracle.sh)) after [`apply-patches.sh`](../scripts/apply-patches.sh):

```sh
scripts/ios-text-oracle.sh                       # every row; non-zero exit on a failure
scripts/ios-text-oracle.sh testDeletingBackward  # rows whose name contains the argument
```

On macOS it builds and runs locally; elsewhere it copies `build/parallax-gpui/` to the owner's Mac (`MAC`, default
`pixl-mac`) and runs there. It boots `IOS_SIMULATOR` (default iPhone 17e) only if it is not booted, and shuts down
only a simulator it booted. **CI:** a `macos-latest` job can run `scripts/apply-patches.sh` then this script (Xcode's
simulator runtime is on the image; the Rust toolchain needs the `aarch64-apple-ios-sim` target, as the iOS row has).

## Results (simulator, iOS 26.5)

46 rows pass: Flutter's 45 and one GPUI row (every `AutofillHint` against UIKit's content type). Before the fixes,
15 failed, one of them by aborting the run:

| Flutter test | Upstream did | Fix |
|---|---|---|
| testDeletingBackward | deleted a whole Thai grapheme (`ดี`) | 0029: one code point unless an emoji (Flutter deletes one UTF-16 unit) |
| testSystemOnlyAddingPartialComposedCharacter | aborted: `to_string()` of half a surrogate pair reads a NULL `UTF8String` | 0029: UTF-16 → lossy string; Flutter's deleted-emoji cache |
| testCachedComposedCharacterClearedAtKeyboardInteraction | (same abort) | 0029: `shouldChangeTextInRange:replacementText:` clears the cache |
| testTextInRange | nil for a range past the end | 0029: clamp to the document |
| testUITextInputAvoidUnnecessaryUndateEditingClientCalls | `unmark_text` with nothing marked | 0029: early return |
| testFlutterTokenizerCanParseLines | system tokenizer's lines wrong (15 → 14..17) | 0029: Flutter's `FlutterTokenizer` line granularity |
| testSecureInput, testUpdateSecureTextEntry | no `isSecureTextEntry` | 0030 |
| testKeyboardType, testKeyboardTypeWebSearch | always the default keyboard | 0030: Flutter's `ToUIKeyboardType` |
| testStandardEditActions, testCanPerformActionForSelectActions, testCanPerformActionCaptureTextFromCamera, testPastingNonTextDisallowed | no `cut:`, `copy:`, `paste:`, `delete:`, `selectAll:` | 0036: Flutter's actions and `canPerformAction:withSender:`; copy and cut also off in secure fields (Flutter's `copyEnabled`) |
| testDisablingAutofillOnInputClient | no `textContentType` | 0037: Flutter's `ToUITextContentType`; no hint is "" (autofill off) |

## Cases left out

Of the inventory's 39 + 15: the 6 delta cases (GPUI has no delta model); testUpdateFirstRectForRange (Flutter's
editable transform and marked rect); the scribble, floating-cursor (`testFloatingCursor`) and placeholder-rect cases
(Flutter's own bookkeeping); testKeyboardTypeTwitter, testVisiblePasswordUseAlphanumeric,
testSettingKeyboardTypeNoneDisablesSystemKeyboard, testInputActionContinueAction and
testIgnoresSelectionChangeIfSelectionIsDisabled (GPUI's `TextInputConfiguration` has no such keyboard, action or
switch); the other autofill cases (Flutter's autofill groups and contexts). Affinity checks are dropped: GPUI positions have none. Ledger: [PORTED.md](../third_party/flutter/PORTED.md).

## Size limit

AGENTS.md caps iOS text-input fixes at about 800 lines before they move to a crate. Changed lines today (added +
removed, tables and the oracle file not counted): 0005's iOS part 1, 0027 126, 0029 170, 0030 17, 0036 90, 0037 27,
0038 122 (553). 0028 is the test table (about 1,020 lines, 971 of them the oracle file).
