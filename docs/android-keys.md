# Android keys

How a key press becomes a GPUI event in [`parallax-android`](../crates/android/src/lib.rs). The backend around it:
[android.md](android.md); the touch path it mirrors: [android-touch.md](android-touch.md). Why the Java is Flutter's:
[AGENTS.md: Flutter is the second source of truth](../AGENTS.md#flutter-is-the-second-source-of-truth); ledger rows in
[PORTED.md](../third_party/flutter/PORTED.md).

## The path

1. **View.** [`ParallaxSurfaceView`](../crates/android/java/io/github/xuckless/parallax/ParallaxSurfaceView.java) is
   focusable and takes focus when attached (without focus keys stop at the DecorView). `dispatchKeyEvent` is
   `FlutterView`'s: start or end key tracking, then hand the event to the `KeyboardManager`.
2. **Java, ported.** [`KeyboardManager`](../crates/android/java/io/github/xuckless/parallax/KeyboardManager.java) offers
   each event to its one responder, [`KeyEmbedderResponder`](../crates/android/java/io/github/xuckless/parallax/KeyEmbedderResponder.java),
   which maps scan code and key code to Flutter's physical and logical keys with
   [`KeyboardMap`](../crates/android/java/io/github/xuckless/parallax/KeyboardMap.java), synthesizes the events that bring
   Shift, Ctrl, Alt and Caps Lock in line with the event's meta state, and packs each event with
   [`KeyData.toBytes`](../crates/android/java/io/github/xuckless/parallax/KeyData.java). The changes: the packet goes
   to the static native `nativeDispatchKeyData(activity, buffer, canceled)` instead of the `flutter/keydata` channel, and its return value
   is the framework's reply, synchronously (GPUI runs on the main thread).
3. **Unhandled keys.** As in Flutter, `KeyboardManager` then offers a key GPUI did not handle to the text input
   (`ViewDelegate.onTextInputKeyEvent` → `TextInputPlugin.handleKeyEvent`, [android-text-input.md](android-text-input.md)), and otherwise redispatches it to the root
   view, which hands it on to Android: volume to the system, an unclaimed back to the Activity.
4. **JNI.** A row in `natives` in [host.rs](../crates/android/src/host.rs) borrows the buffer for the call and hands
   it to `Shell::key`, which returns GPUI's answer. Keys from a view whose Activity does not own the window are
   dropped unhandled ([android-lifecycle.md](android-lifecycle.md#several-activities)).
5. **Decoder and table.** [keys.rs](../crates/android/src/keys.rs) reads the packet with Flutter's layout (the `Field`
   enum: `charSize`, then `key_data.h`'s `KeyData` in order, then the UTF-8 character). The `KEYS` table maps a Flutter
   logical key to GPUI: a key name (`left`, `enter`, `f5`), a modifier, Caps Lock or Back; printable keys in Flutter's
   Unicode plane are their own character. Logical keys without a row send nothing and go back to Android.
6. **GPUI.** `Keyboard::input` decides, `Keyboard::reply` reads GPUI's answer; the keyboard is not borrowed while GPUI
   runs. Keys become `KeyDown` (repeat: `is_held`) and `KeyUp` with the held modifiers; `key_char` is Flutter's character
   unless it is a control character. Modifier keys become `ModifiersChanged` only, as on GPUI's other platforms; Caps
   Lock toggles on its down. `PlatformWindow::modifiers` and `capslock` read the same state.

## Back

`KEYCODE_BACK` (Flutter's `goBack`) becomes upstream's
[`BackGestureEvent`](../upstream/gpui-pre/src/gestures.rs): `Started` on the press, at the window's left edge,
mid-height (GPUI hit-tests back gestures; a key has no position), with `source: BackGestureSource::Key`, so
`Window::pointer_source()` is `None` during it ([patch 0008](../patches/0008-back-gesture-source.patch), [0003](pointer-source.md)). An element with `on_back_gesture` claims it
(`default_prevented`); then repeats are swallowed and the release sends `Committed`, or `Cancelled` when Android
cancelled it (`FLAG_CANCELED`, passed next to the packet because key data has no field for it). Unclaimed, the press and release
go to Android, whose default finishes the Activity. One state enum, `Back::{Idle, Offered, Claimed}`, and one
`Back::next(state, BackEvent) → (state, Outcome)` that both `Keyboard::input` (back key events) and `Keyboard::reply`
(GPUI's answers) call; `keys::tests::back_transitions` has a row per (state, event).

Claiming works after typing because of [patch 0006](../patches/0006-claim-back-gesture-after-key-input.patch):
upstream's `on_back_gesture` tested `hitbox.is_hovered`, which is false while the last input was a key (keyboard
modality hides hover), and a back gesture does not change the modality, so no element could claim a back press until
the next touch. The patch hit-tests the gesture's own position regardless of modality; its window test has a row per
preceding input (key press, touch).

## Not yet

- **Keyboard layout.** `keyboard_layout` and `keyboard_mapper` are still stubs ([android.md](android-stubs.md)).
- **Predictive back** (API 33+ `OnBackInvokedCallback`, progress while swiping) and the device type field.

## Changing it

- **Flutter changes a file** (`scripts/flutter-diff.sh`): port the Java change; for `key_data.h` edit `Field` or
  `KEY_EVENT_TYPES` in Flutter's order. `KeyboardMap.java` is generated: copy it again whole.
- **Map another key:** add a row to `KEYS` with its value and name from `KeyboardMap.keyCodeToLogical`, and a test row.
- **Check on the emulator:** `adb shell input keyevent 29` (a), `21` (left), `4` (back); `input keycombination 59 29`
  (shift+a). `Shell::key` logs each packet and its step with `log::debug!`. Injected events have meta state 0 on the
  virtual keyboard device, so the responder (as in Flutter) synthesizes a Shift release right after the press; a hardware
  keyboard keeps it held.
- Tests: `cargo test -p parallax-android` (`keys::tests`: one row per key sequence and per packet case) and
  `scripts/java-test.sh` (Flutter's `KeyboardManagerTest` on Robolectric, [android-tests.md](android-tests.md)).
