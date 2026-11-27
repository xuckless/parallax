# Android touch

How a finger on the screen becomes a GPUI event in [`parallax-android`](../crates/android/src/lib.rs). The backend
around it: [android.md](android.md). Why the Java is Flutter's: [AGENTS.md: Flutter is the second source of
truth](../AGENTS.md#flutter-is-the-second-source-of-truth); the ledger rows are in
[PORTED.md](../third_party/flutter/PORTED.md).

## The path

1. **Java, ported.** [`ParallaxSurfaceView.onTouchEvent`](../crates/android/java/io/github/xuckless/parallax/ParallaxSurfaceView.java)
   calls `requestUnbufferedDispatch(event)` (as Flutter's `FlutterView` does, for every event once it has an
   engine; this view always has one) so moves arrive as the OS reads them instead of batched to vsync, then hands
   each `MotionEvent` to [`AndroidTouchProcessor`](../crates/android/java/io/github/xuckless/parallax/AndroidTouchProcessor.java),
   Flutter's file nearly verbatim. It packs one record per pointer into a little-endian direct `ByteBuffer`, exactly as
   for Flutter's engine: 36 fields of 8 bytes, a synthesized remove after every touch up, batched moves for the other
   pointers on `ACTION_POINTER_UP`. Only the sink changed: the constructor takes the view's Activity id in place of the renderer, and
   `renderer.dispatchPointerDataPacket` became the static native `ParallaxSurfaceView.nativeDispatchPointerDataPacket(activity, buffer, position)`; touches from a view whose
   Activity does not own the window are dropped ([android-lifecycle.md](android-lifecycle.md#several-activities)).
2. **JNI.** The native is a row in `natives` in [host.rs](../crates/android/src/host.rs). It borrows the buffer's
   first `position` bytes for the length of the call (main thread; the buffer is fresh per event) and hands them to
   `Shell::pointer`.
3. **Decoder.** [pointer.rs](../crates/android/src/pointer.rs) reads the packet with Flutter's own layout: the
   `Field` enum lists `PointerData` from Flutter's `pointer_data.h` in order, so a field's offset is its position.
   `PointerChange` and `PointerDeviceKind` values come from two tables indexed by Flutter's numbers
   (`CHANGES`, `DEVICE_KINDS`). A packet that is not whole records, or carries an unknown change or kind, is
   rejected whole with a typed `Error` and logged. The signal kind (`SIGNAL_KINDS`) is read too: a wheel turn.
4. **TouchEvent.** Each `DEVICE_KINDS` row names a route: finger and unknown → `TouchEvent { device: Touch }`,
   stylus and eraser → `Pen`, mouse and trackpad → mouse and scroll events ([android-mouse.md](android-mouse.md)).
   `Pointers` maps Flutter's `device` (Android pointer id and tool type) to a fresh upstream `TouchId` per touch,
   since GPUI ids must never be reused and Android's are. The touch column of `CHANGES` says what each change becomes:
   down → `Started`, move → `Moved`, up → `Ended`, cancel → `Cancelled`; add, remove and a pen's hover send nothing.
   Positions are divided by the display density (logical pixels); pressure becomes `force` when the device reports a
   range. A move or up for a pointer that never went down is dropped.
5. **GPUI.** Each event goes to the window as `PlatformInput::Touch` through `Callbacks::input`, in packet order.
   Upstream's [gestures.rs](../upstream/gpui-pre/src/gestures.rs) recognizes taps (synthesized mouse down/up, so
   `on_click` fires), pans (scroll events with touch phases) and long presses, unless an `on_touch` element claims
   the touch (deepest handler wins, [core-patches.md](core-patches.md#0004-touch-opt-out)). Every finger of a
   multi-touch arrives as its own touch, so two or more on an `on_pinch` element pinch it ([pinch.md](pinch.md)).
   The platform's `mouse_position` reports the last touch.

## Not yet

- **Platform views.** `trackMotionEvents` is false, as in Flutter's `FlutterView`, so the ported
  [`MotionEventTracker`](../crates/android/java/io/github/xuckless/parallax/MotionEventTracker.java) is unused.

## Changing it

- **Flutter changes the packet** (`scripts/flutter-diff.sh` shows a diff for `pointer_data.h` or
  `AndroidTouchProcessor.java`): port the Java change, then edit `Field` or the value tables in `pointer.rs` in
  Flutter's order. The decoder test rows build records from the same `Field` enum.
- **Route a change elsewhere** (e.g. a pen's hover → mouse move): change its row in `CHANGES` and add a test row.
- **Check on the emulator:** `adb shell input tap` and `adb shell input swipe`; `Shell::pointer` logs each
  event with `log::debug!`, below the `Info` level `on_load` sets, so raise that locally to see them.
- Tests: `cargo test -p parallax-android` (one named row per packet case in `pointer::tests::touches`) and
  `scripts/java-test.sh` (Flutter's `AndroidTouchProcessorTest` on Robolectric, [android-tests.md](android-tests.md)).
