# Oracle inventory: text input and keys

Part of the [oracle inventory](oracle-inventory.md). Flutter tag `3.50.1`; upstream `gpui-pre` 0.3.14.
Cases = `@Test` (Java), `- (void)test…` (Obj-C), `test(`/`testWidgets(` (Dart).

## Android (Phase 1: tests of the Java port, not of upstream)

These check Parallax's port in `crates/android/java/`. The Rust side behind it answers from GPUI's
[`InputHandler`](../../upstream/gpui-pre/src/input.rs). Shared mapping: Flutter's mocked `TextInputChannel` /
`FlutterRenderer` / `KeyChannelResponder` become a mocked JNI-facing interface; assertions on channel calls become
assertions on that interface. Base path: `engine/src/flutter/shell/platform/android/test/io/flutter/`.

| Flutter test | Size / cases | Tests | Checks | Verdict |
|---|---|---|---|---|
| [ListenableEditingStateTest.java](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test/io/flutter/plugin/editing/ListenableEditingStateTest.java) | 15 KB / 13 | batch edits, listeners, composing region | ported `ListenableEditingState` | EASY: 13/13, runs as-is |
| [TextEditingDeltaTest.java](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test/io/flutter/plugin/editing/TextEditingDeltaTest.java) | 2.5 KB / 2 | delta construction | ported `TextEditingDelta` | EASY: 2/2 |
| [InputConnectionAdaptorTest.java](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test/io/flutter/plugin/editing/InputConnectionAdaptorTest.java) | 56 KB / 36 | context-menu actions, arrow keys over emoji, extracted text, cursor anchor, batch end on close | ported `InputConnectionAdaptor` | MAPPED: ~27/36; 9 `performPrivateCommand` cases drop if private commands are dropped |
| [TextInputPluginTest.java](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test/io/flutter/plugin/editing/TextInputPluginTest.java) | 140 KB / 53 | restart rules, input types, deltas, IME insets, autofill, platform views | input parts of `TextInputPlugin`, `ImeSyncDeferringInsetsCallback` | HARD: ~25/53 (input types 8, restart 5, deltas 5, insets 4, `EditorInfo` 3); autofill 13 and platform views drop |
| [KeyboardManagerTest.java](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test/io/flutter/embedding/android/KeyboardManagerTest.java) | 71 KB / 29 | responder order, redispatch, modifier/caps-lock sync, minted codes | ported `KeyboardManager` (+ `KeyEmbedderResponder`) | MAPPED: ~21/29; the 21 embedder cases need `KeyEmbedderResponder` ported too (not in AGENTS.md's list) |
| [AndroidTouchProcessorTest.java](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test/io/flutter/embedding/android/AndroidTouchProcessorTest.java) | 32 KB / 15 | pointer packet fields: tool type, pressure, size, scroll wheel, synthesized | ported `AndroidTouchProcessor` | MAPPED: 15/15 if the port keeps Flutter's `ByteBuffer` packet over JNI; otherwise rewrite against the Rust decoder |

## iOS (Phase 4)

| Flutter test | Size / cases | Tests | Checks upstream | Verdict |
|---|---|---|---|---|
| [FlutterTextInputPluginTest.mm](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/darwin/ios/framework/Source/FlutterTextInputPluginTest.mm) | 207 KB / 135 | see [risk (b)](oracle-inventory.md#b-ios-fluttertextinputplugintestmm) | [ios/text_input.rs](../../upstream/gpui-pre-ios/src/ios/text_input.rs) `UITextInput` impl | MAPPED: 39 behaviour + 15 mapped; 81 not portable |
| [FlutterKeyboardManagerTest.mm](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/darwin/ios/framework/Source/FlutterKeyboardManagerTest.mm) | 12 KB / 5 | responder chain of key presses | [ios/events.rs](../../upstream/gpui-pre-ios/src/ios/events.rs) | HARD: ~1/5 |

Mapping for the 39: each case's initial text, selection and marked range → a fake `PlatformInputHandler`; the
`UITextInput` call → the row's event; Flutter's `setSelectionRects` input → the fake's `bounds_for_range`.
The [existing fixture](../../upstream/gpui-pre-ios/tests/text_input.rs) has no input handler, so the fake is new work.
`keyboardType` is hard-coded `Default` upstream, and `TextInputConfiguration` has no keyboard-type or secure field:
the 11 trait cases find a gap, not a bug.

## Web (Phase 4)

| Flutter test | Size / cases | Tests | Checks upstream | Verdict |
|---|---|---|---|---|
| [text_editing_test.dart](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/lib/web_ui/test/engine/text_editing_test.dart) | 193 KB / 133 | per-browser strategies (30), `HybridTextEditing` (51), `EditingState` (11), autofill (19), deltas (8), embedded scroll (10), styles (4) | [ime_mirror.rs](../../upstream/gpui-pre-web/src/ime_mirror.rs), [events.rs](../../upstream/gpui-pre-web/src/events.rs) | MAPPED: ~50/133. One mirror `<textarea>` vs Flutter's element per field: config, focus/blur, iOS refocus and `EditingState` cases map to `apply_configuration`, `focus`/`blur`, `stored_text`/`stored_selection`. Autofill, deltas, embedding drop |

## Framework (Dart; no platform code)

| Flutter test | Size / cases | Verdict |
|---|---|---|
| [services/text_input_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/services/text_input_test.dart), [delta_text_input_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/services/delta_text_input_test.dart), [text_editing_delta_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/services/text_editing_delta_test.dart), [hardware_keyboard_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/services/hardware_keyboard_test.dart) | 139 KB / 102 | HARD: 0. Channel protocol and Flutter's key model |
| [widgets/editable_text_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/widgets/editable_text_test.dart), [text_selection_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/widgets/text_selection_test.dart), [magnifier_test.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/test/widgets/magnifier_test.dart) | 772 KB / 396 | HARD: widget tests. Behaviour reference only for Phase 4 selection handles, magnifier, toolbar |

Area total: 11 rows / 16 files, 919 cases; EASY 2, MAPPED 5, HARD 4 (rows); ~208 cases salvageable (54 iOS, 50 web, 103 Android port, 1 iOS keys).
