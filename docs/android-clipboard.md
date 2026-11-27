# Android clipboard

GPUI's `cx.read_from_clipboard()` and `cx.write_to_clipboard(item)` reach Android's `ClipboardManager` through
Flutter's own clipboard code, ported into [PlatformPlugin.java](../crates/android/java/io/github/xuckless/parallax/PlatformPlugin.java)
(`getClipboardData`, `setClipboardData`; ledger: [PORTED.md](../third_party/flutter/PORTED.md)). Rules:
[AGENTS.md](../AGENTS.md) (port before write).

## How it works

- **The bridge.** `ParallaxActivity.onCreate` builds the Activity's `PlatformPlugin` and passes it to
  `nativeActivityCreated`; Rust keeps a JNI global reference in that Activity's window
  (`ActivityWindow::platform_plugin`, [activity_window.rs](../crates/android/src/activity_window.rs)), freed when
  the Activity is destroyed.
- **The calls.** `Platform::read_from_clipboard` and `write_to_clipboard`
  ([platform.rs](../crates/android/src/platform.rs)) call the newest live Activity's plugin synchronously, on the
  main thread, as GPUI expects: the clipboard is the system's, so any Activity serves. A Java exception is logged and
  cleared; reading then gives `None`.
- **Plain text only**, as Flutter on Android: a read is the primary clip's first item as text (a `content:` URI is
  coerced to text, Flutter's rule); a write is `ClipData.newPlainText` of `ClipboardItem::text()`. GPUI's metadata
  and image entries do not reach Android (an image-only item is logged and dropped).

Not ported: Flutter's `clipboardHasStrings` (GPUI has no "can paste" query) and `ClipboardContentFormat.fromValue`
(Rust passes no format, the channel's default). Android 12+ shows its "pasted from" toast on a read; Android 10+
refuses reads from an app without focus (the read is `None`).

## Check it

In hello on an emulator (`cargo parallax run android`): type into the field, long-press a word, tap **Copy** in the
toolbar, move the caret and tap **Paste**: the word is inserted. `adb logcat -s parallax PlatformPlugin` shows any
read or write error.
