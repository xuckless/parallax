# Oracle inventory

Flutter's tests that can check upstream GPUI (or Parallax's Java port), at Flutter stable `3.50.1` against
`gpui-pre` 0.3.14 in [upstream/](../../upstream/). Sets the order of ROADMAP Phases 2 and 4
([ROADMAP.md](../../ROADMAP.md)); why Flutter is the oracle: [docs/flutter.md](../flutter.md).
Nothing here is ported yet, so nothing goes in the ledger.

Verdicts: **EASY** pure algorithm, inputs and outputs map directly. **MAPPED** needs a written mapping to GPUI's API
(stated in the row). **HARD** tests Flutter internals; the row says how many cases survive.

| Area | Detail | Files | Cases | EASY / MAPPED / HARD (files or rows) | Salvageable cases |
|---|---|---|---|---|---|
| Gestures, velocity, physics | [oracle-inventory-gestures.md](oracle-inventory-gestures.md) | 22 | 304 | 2 / 10 / 10 | ~75 |
| Text input and keys | [oracle-inventory-text.md](oracle-inventory-text.md) | 16 | 919 | 2 / 5 / 4 rows | ~208 |
| Accessibility | [oracle-inventory-a11y.md](oracle-inventory-a11y.md) | 7 | 523 | 0 / 3 / 3 rows | ~183 |

Counts are `@Test`, `- (void)test…` and top-level-or-grouped `test(`/`testWidgets(` calls in the file at the tag.
Salvage figures are estimates from test names and bodies, not runs.

## Risks

1. **Test access to upstream.** Every gesture oracle targets private code in
   [gestures.rs](../../upstream/gpui-pre/src/gestures.rs) (`VelocityTracker`, `ScrollPhysics::fling_*`,
   `pub(crate) TouchGestureRecognizer`). Tests from outside need either a patch adding an in-crate test module or
   a public seam. Not decided here; it is a patch-policy question for the owner.

### (a) Android: Robolectric and Mockito

Yes, all four need both. Every file runs `@RunWith(AndroidJUnit4.class)` on Robolectric (real `Editable`, `KeyEvent`,
`InputMethodManager`, `View` classes; the stub `android.jar` throws). `InputConnectionAdaptorTest` and
`TextInputPluginTest` add Robolectric shadows (`ShadowInputMethodManager`, `ShadowAutofillManager`, `ShadowBuild`,
custom `TestImm`) and `@Config(sdk = …)` per case. Mockito: `mock`/`spy`/`verify` throughout (InputConnectionAdaptor 47
uses, TextInputPlugin 396, KeyboardManager 32, AndroidTouchProcessor 34 with `ArgumentCaptor`, `InOrder`).
Even `ListenableEditingStateTest` and `TextEditingDeltaTest` use `AndroidJUnit4` and Mockito.

Infrastructure Parallax needs, from Flutter's [test_runner/build.gradle](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test_runner/build.gradle):
a Gradle Android library module (AGP 9.3.1, `compileSdk 37`) with JVM unit tests (`includeAndroidResources = true`),
JDK 17 (source/target 17, the `--add-opens` set Robolectric 4.17 asks for), `org.robolectric:robolectric:4.17`,
`org.mockito:mockito-core:5.20.0` loaded as a `-javaagent`, `junit:4.13.2`, `androidx.test:core:1.6.1`,
`androidx.test.ext:junit:1.2.1`, ICU4J 75.1. No device needed. Flutter's `io.flutter.Build.API_LEVELS` constants
come along; 8 cases pin `sdk` 24/25, below Parallax's API 28 minimum, and get raised or dropped.

### (b) iOS: FlutterTextInputPluginTest.mm

135 cases ([detail](oracle-inventory-text.md#ios-phase-4)), bucketed by name and body:

| Bucket | Cases | Portable? |
|---|---|---|
| `UITextInput` behaviour: text/range/marked text, delete, composed characters, `firstRectForRange` (12), `closestPosition`/`selectionRects` (6), tokenizer (4), edit actions | 39 | yes, as behaviour tables against [text_input.rs](../../upstream/gpui-pre-ios/src/ios/text_input.rs) |
| Text traits (keyboard type, secure, autocorrect, spellcheck, inline prediction, return key) | 11 | mapped via `TextInputConfiguration`; finds gaps (no keyboard-type field) |
| Update-once / no-echo of framework changes | 4 | mapped to `InputHandler` call counts |
| Delta generation | 6 | no (GPUI has no delta model) |
| Plugin internals: autofill 16, interactive keyboard 14, focus restore 10, edit menu 10, scribble 6, lifecycle/crash/view reuse 19 | 75 | no |

So about 39 portable as-is, 15 with a mapping, 81 not. They use OCMock heavily (164 `OCM…` uses); the tables replace it.

## Recommended order

Cheapest and highest value first. Phase 2:

1. Velocity tracker trace + lsq (EASY, ~9 cases, one recorded trace, checks the code every fling uses).
2. Friction and clamping simulations against both `ScrollPhysics` arms (~6 cases).
3. Tap, double-tap, long-press, drag recognizer rows with Flutter's tuning passed explicitly (~49 cases); this
   harness is also the Phase 2 trace-replay engine.
4. Scale when upstream ships pinch; arena, resampler, springs, overscroll not at all for now.

Phase 4 (Android port tests run in Phase 1, in this order: `ListenableEditingState`, `TextEditingDelta`,
`InputConnectionAdaptor`, `AndroidTouchProcessor`, `KeyboardManager`, `TextInputPlugin`):

1. iOS `UITextInput` behaviour table (39 cases, one fake input handler) — highest bug-finding value, upstream code is new.
2. iOS trait and update-count rows (15), filed as gaps where GPUI lacks a field.
3. Web `text_editing_test.dart` strategy and `EditingState` rows (~50) against `ime_mirror.rs`.
4. Selection handles, magnifier, toolbar from the widget tests as behaviour reference only.
