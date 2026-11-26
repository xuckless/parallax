# Android touch

How a finger on the screen becomes a GPUI event in [`parallax-android`](../crates/android/src/lib.rs). The backend
around it: [android.md](android.md). Why the Java is Flutter's: [AGENTS.md: Flutter is the second source of
truth](../AGENTS.md#flutter-is-the-second-source-of-truth); the ledger rows are in
[PORTED.md](../third_party/flutter/PORTED.md).

## The path

1. **Java, ported.** [`ParallaxSurfaceView.onTouchEvent`](../crates/android/java/io/github/xuckless/parallax/ParallaxSurfaceView.java)
   hands each `MotionEvent` to [`AndroidTouchProcessor`](../crates/android/java/io/github/xuckless/parallax/AndroidTouchProcessor.java),
   Flutter's file nearly verbatim. It packs one record per pointer into a little-endian direct `ByteBuffer`, exactly as
   for Flutter's engine: 36 fields of 8 bytes, a synthesized remove after every touch up, batched moves for the other
   pointers on `ACTION_POINTER_UP`. Only the last call changed: `renderer.dispatchPointerDataPacket` became the static
   native `ParallaxSurfaceView.nativeDispatchPointerDataPacket(buffer, position)`.
2. **JNI.** The native is a row in `natives` in [host.rs](../crates/android/src/host.rs). It borrows the buffer's
   first `position` bytes for the length of the call (main thread; the buffer is fresh per event) and hands them to
   `Shell::touch`.
3. **Decoder.** [pointer.rs](../crates/android/src/pointer.rs) reads the packet with Flutter's own layout: the
   `Field` enum lists `PointerData` from Flutter's `pointer_data.h` in order, so a field's offset is its position.
   `PointerChange` and `PointerDeviceKind` values come from two tables indexed by Flutter's numbers. A packet that is
   not whole records, or carries an unknown change or kind, is rejected whole with a typed `Error` and logged.
4. **TouchEvent.** `Touches` maps Flutter's `device` (Android pointer id and tool type) to a fresh upstream `TouchId`
   per touch, since GPUI ids must never be reused and Android's are. The `TOUCH_PHASES` table says what each change
   becomes: down → `Started`, move → `Moved`, up → `Ended`, cancel → `Cancelled`; add, remove, hover and trackpad
   pan/zoom send nothing yet. Positions are divided by the display density (logical pixels); pressure becomes
   `force` when the device reports a range. A move or up for a pointer that never went down is dropped.
5. **GPUI.** Each event goes to the window as `PlatformInput::Touch` through `Callbacks::input`, in packet order.
   Upstream's [gestures.rs](../upstream/gpui-pre/src/gestures.rs) recognizes taps (synthesized mouse down/up, so
   `on_click` fires), pans (scroll events with touch phases) and long presses. `mouse_position` reports the last
   touch.

## Not yet

- **Device kind and synthesized flag.** Every kind (finger, stylus, eraser, mouse button) becomes a `TouchEvent`;
  upstream has no field for the device. The core patch in [ROADMAP.md](../ROADMAP.md) Phase 1 plugs in where
  `Touches::events` builds the event.
- **Mouse hover, wheel and trackpad.** `AndroidTouchProcessor.onGenericMotionEvent` is ported but the view does not
  call it yet; hover and pan/zoom rows in `TOUCH_PHASES` send nothing.
- **Platform views.** `trackMotionEvents` is false, as in Flutter's `FlutterView`, so the ported
  [`MotionEventTracker`](../crates/android/java/io/github/xuckless/parallax/MotionEventTracker.java) is unused.
- **Java tests.** Flutter's `AndroidTouchProcessorTest` needs Robolectric (a later Phase 1 task). It mocks the
  renderer; here the seam is the static native, which a Robolectric shadow of `ParallaxSurfaceView` can capture.

## Changing it

- **Flutter changes the packet** (`scripts/flutter-diff.sh` shows a diff for `pointer_data.h` or
  `AndroidTouchProcessor.java`): port the Java change, then edit `Field` or the value tables in `pointer.rs` in
  Flutter's order. The decoder test rows build records from the same `Field` enum.
- **Route a change elsewhere** (e.g. hover → mouse move): change its row in `TOUCH_PHASES` and add a decoder row.
- **Check on the emulator:** `adb shell input tap` and `adb shell input swipe`; `Shell::touch` logs each
  `TouchEvent` with `log::debug!`, below the `Info` level `on_load` sets, so raise that locally to see them.
- Tests: `cargo test -p parallax-android` (one named row per packet case in `pointer::tests`).
