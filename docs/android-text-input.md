# Android text input

How the soft keyboard (IME) edits a GPUI text field in [`parallax-android`](../crates/android/src/lib.rs). The backend
around it: [android.md](android.md); the key path it joins: [android-keys.md](android-keys.md). The Java is Flutter's,
ported ([AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth); ledger rows in
[PORTED.md](../third_party/flutter/PORTED.md)), so Flutter's IME fixes keep applying.

## The pieces

- **Java, ported.** [`TextInputPlugin`](../crates/android/java/io/github/xuckless/parallax/TextInputPlugin.java) owns the
  session: client, configuration → `EditorInfo`, show and hide, Flutter's restart rules.
  [`InputConnectionAdaptor`](../crates/android/java/io/github/xuckless/parallax/InputConnectionAdaptor.java) is the
  IME's `InputConnection`; it edits [`ListenableEditingState`](../crates/android/java/io/github/xuckless/parallax/ListenableEditingState.java),
  the Java mirror whose batch edits Flutter's logic needs. [`TextInputChannel`](../crates/android/java/io/github/xuckless/parallax/TextInputChannel.java)
  is Flutter's channel with JNI in place of the `MethodChannel`.
  [`ImeSyncDeferringInsetsCallback`](../crates/android/java/io/github/xuckless/parallax/ImeSyncDeferringInsetsCallback.java)
  animates the `ime` inset (API 30+), which reaches GPUI through `Window::insets()` ([android.md](android.md)).
- **Rust.** [text_input.rs](../crates/android/src/text_input.rs) decides (pure, host-tested); `Shell` in
  [host.rs](../crates/android/src/host.rs) calls GPUI's `PlatformInputHandler`, which the window keeps in the shell
  so the IME's JNI calls reach it ([window.rs](../crates/android/src/window.rs)). Offsets are UTF-16 on both sides.

## The sync protocol

GPUI is the truth; Java's `ListenableEditingState` is a mirror. All on the main thread:

1. **Java → GPUI, synchronous.** After each IME batch edit, `TextInputPlugin.didChangeEditingState` sends the full
   state (`nativeUpdateEditingState`: text, selection, composing). `text_input::edits` diffs it against GPUI's state
   and makes the calls: one `replace_text_in_range`, or `replace_and_mark_text_in_range` when the new text is the
   composing range (`setComposingText`), then `unmark_text` / a mark / `set_selected_text_range` only where still
   different. Notices GPUI sends meanwhile are the edit's echo (`Origin::Java`) and dropped. If GPUI then differs from
   Java (the app filtered the edit), Java gets GPUI's state at the frame's end.
2. **GPUI → Java, at the frame's end.** `text_input_state_changed`, `set_text_input_configuration`, a new focused
   field (`FocusId`) and `show`/`hide_soft_keyboard` fold into one `Pending` (`Pending::next`, a row per notice).
   `nativeFrame` returns it as Flutter's `flutter/textinput` calls in JSON (`clearClient`, `setClient` with
   `Configuration.fromJson`'s keys, `setEditingState`, `show`/`hide`); Java runs them after GPUI's draw, so
   `restartInput` → `onCreateInputConnection` re-enters Rust with GPUI free.
3. **Conflict.** While a `Pending` holds a change Java has not seen, an IME edit is based on a stale state: it is
   dropped and the frame's `setEditingState` (with Flutter's composing-restart rule) brings the IME up to date.

Focus shows the keyboard, blur hides it (as `gpui_ios`). Editor actions (`performEditorAction`) reach GPUI as an
Enter press; GPUI's `TextInputAction` only labels the key. `CursorAnchorInfo` gets GPUI's caret
(`nativeBoundsForRange`); `update_ime_position` is not used.

## Keys

Hardware keys go to GPUI first ([android-keys.md](android-keys.md)); keys GPUI leaves unhandled go to
`TextInputPlugin.handleKeyEvent` → `InputConnectionAdaptor.handleKeyEvent` (characters, arrows, Enter as the editor
action), as in Flutter. The IME's `sendKeyEvent` goes back through `KeyboardManager` (`KeyboardDelegate`).

## Configuration

`configuration_json` maps GPUI's `TextInputConfiguration` to `Configuration.fromJson`'s keys: `CAPITALIZATION` and
`ACTIONS` (row tables), `input_type` (`KeyboardType` → `TextInputType`; Search is `webSearch`) and `autofill_hint`
(`AutofillHint` → Flutter's `AutofillHints`), exhaustive matches so a new variant of patch 0005 fails to compile until
it gets an arm. `secure` is `obscureText` with personalized learning off. `autofill` is sent in Flutter's shape; the
ported Java drops it until autofill is ported.

## Limits

- Java mirrors the whole document (`text_length_utf16` required, as on iOS); `text_input_editable_range` is not used.
- No autofill, deltas, private commands or `commitContent` (images) yet; see the ledger rows.

## Check it

`examples/hello` has a field ([examples.md](examples.md)). On the emulator: tap it, `adb shell input text hello`,
`adb shell input keyevent 67` (backspace); `adb logcat -s parallax` shows each IME state, the
calls it became and the frame's calls. Tests: `cargo test -p parallax-android text_input`.
