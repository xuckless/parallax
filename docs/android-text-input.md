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

GPUI is the truth; Java's `ListenableEditingState` is a mirror. All on the main thread. The `Session` keeps the
mirror as Rust last knew it: the state Java was sent, or the state Java sent.

1. **Java → GPUI, synchronous.** After each IME batch edit, `TextInputPlugin.didChangeEditingState` sends the full
   state (`nativeUpdateEditingState`: text, selection, composing). `Session::java_edit` carries it onto GPUI's state
   (`merge`, below); `text_input::edits` diffs the result against GPUI's state and makes the calls: one
   `replace_text_in_range`, or `replace_and_mark_text_in_range` when the new text is the composing range
   (`setComposingText`), then `unmark_text` / a mark / `set_selected_text_range` only where still different. Notices
   GPUI sends meanwhile are the edit's echo (`Origin::Java`) and dropped. Then `differs` decides whether Java must
   take GPUI's state: when the app changed the edit (a filter, a moved caret) or a GPUI change was merged in.
   What GPUI does not report (a reversed selection kept forward, no selection, no composing range) is not news.
2. **GPUI → Java, at the frame's end and after each key.** `text_input_state_changed`, `set_text_input_configuration`,
   a new focused field (`FocusId`) and `show`/`hide_soft_keyboard` fold into one `Pending` (`Pending::next`, a row per
   notice). `nativeFrameworkCalls` returns it as Flutter's `flutter/textinput` calls in JSON (`clearClient`,
   `setClient` with `Configuration.fromJson`'s keys, `setEditingState`, `show`/`hide`). Java asks after
   `nativeFrame` and right after `KeyboardManager` handled a key event (hardware or the IME's `sendKeyEvent`): Gboard
   sends DEL as a key, then may `commitText` before vsync, and the mirror must already hold GPUI's deletion. Java
   runs the calls outside GPUI's draw, so `restartInput` → `onCreateInputConnection` re-enters Rust with GPUI free. A
   new client always gets a `setEditingState`, empty if the field is unreadable (logged once per focus), never with
   a composing range. Patch 0010 ends composition when focus leaves a field; `Pending::commit` is the backstop for a
   new configuration on the same field mid-composition and for handlers that ignore `unmark_text`.
3. **Race: merge, don't drop.** An IME edit made while a GPUI change is on its way is based on the old mirror.
   `merge(mirror, java, gpui)` takes Java's change as one span of the mirror and lands it where that span is in
   GPUI's state; an offset inside the span GPUI changed moves to its end (GPUI's change stays whole, Java's lands
   next to it). Selection and composing move the same way. Java then gets the merged state at the next delivery. An
   edit for a client GPUI is replacing (a new field or none pending) is dropped: it belongs to the field GPUI left.

Focus shows the keyboard, blur hides it (as `gpui_ios`). Only a field that takes text input counts (between GPUI's
`FocusGained` and `FocusLost`). When another Activity takes the window, its view's `TextInputPlugin` has no client, so
a focused field is announced again (`Session::owner_changed`). Editor actions (`performEditorAction`) reach GPUI as an
Enter press, every action alike until GPUI has an action table; GPUI's `TextInputAction` only labels the key.
`CursorAnchorInfo` gets GPUI's caret (`nativeBoundsForRange`); `update_ime_position` is not used.

**Keys.** Hardware keys go to GPUI first ([android-keys.md](android-keys.md)); keys GPUI leaves unhandled go to
`TextInputPlugin.handleKeyEvent` → `InputConnectionAdaptor.handleKeyEvent` (characters, arrows, Enter as the editor
action), as in Flutter. The IME's `sendKeyEvent` goes back through `KeyboardManager` (`KeyboardDelegate`).

**Configuration.** `configuration_json` maps GPUI's `TextInputConfiguration` to `Configuration.fromJson`'s keys
`capitalization`, `action`, `input_type` (`KeyboardType` and `secure` → `TextInputType`; Search is `webSearch`) by
exhaustive matches: a new patch 0005 variant needs an arm. `secure` is `obscureText`, personalized learning off.
Flutter's number branch returns before `obscureText`, so a secure Number or Decimal sends `"password": true`; a secure
Phone is a number password (Android has none for phones). Email, Url and Search carry their own Android variation,
which Flutter's password and no-suggestions flags would overwrite (email without suggestions became `VARIATION_FILTER`,
no email keyboard): `has_own_variation` sends them with suggestions on, and as text when secure. `autofill` adds
`Configuration.Autofill` and two calls around the new client ([android-autofill.md](android-autofill.md)).

## Limits

- Java mirrors the whole document (`text_input_editable_range` unused; no `text_length_utf16`: `0..usize::MAX`).
- No deltas, private commands or `commitContent` (images) yet (ledger rows); autofill's: [android-autofill.md](android-autofill.md#not-yet).

## Check it

`examples/hello` has a field ([examples.md](examples.md)). On the emulator: tap it, `adb shell input text hello`,
`adb shell input keyevent 67` (backspace). IME edits and call deliveries log at debug level as lengths and ranges,
never text (a field may be a password); raise `on_load`'s info-level logger in [host.rs](../crates/android/src/host.rs)
to see them. Tests: `cargo test -p parallax-android text_input`, `scripts/java-test.sh` ([android-tests.md](android-tests.md)).
