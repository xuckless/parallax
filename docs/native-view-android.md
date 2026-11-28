# Native views on Android

`NativeView` on Android ([native-view.md](native-view.md) is the shared design): the app's `View` or `SurfaceControl`
sits under GPUI's surface and shows through the hole the wgpu renderer cuts (patch
[0047](../patches/0047-wgpu-native-view-hole.patch)). No fork patch: it is all in parallax-android —
[native_view.rs](../crates/android/src/native_view.rs) decides, Java's
[NativeViews](../crates/android/java/io/github/xuckless/parallax/NativeViews.java) acts.

## Using it

1. Build the view with the Activity: `parallax_android::with_activity(window, |env, activity| …)`.
2. `NativeView::new(ptr, window, cx)`, `ptr` a JNI reference to either
   - a **`SurfaceControl`** with a buffer (API 29+, `SurfaceControl.Builder().setBufferSize(…)`), rendered through
     `new Surface(control)`: video, a browser engine's frames, camera. Its buffer size follows the element (in pixels);
   - any parentless **`View`**: a `WebView`, a `MapView`, an ordinary `SurfaceView` (API 28's surface).
3. Touches: GPUI takes them all (`NativeViewInput::Window`): the view's element hears them, positions relative to
   `view.bounds(window)`. `parallax_android::forward_touches(window, &view, TouchForwarding::On)` also sends each
   gesture that *starts* on the view (no GPUI element over it there) to the `View`, offset into its coordinates, so a
   WebView scrolls and takes taps.
4. `view.set_background(colour)` paints the underlay: what shows where a view has not caught up with its hole.

[examples/native-view-android](../examples/native-view-android/) does all of this
(`--es surface view` for the SurfaceView path, `--ez animate true` to move the slot every frame).

## What happens

| Event | Java does |
|---|---|
| first `NativeView` attached | GPUI's `SurfaceView` goes `setZOrderOnTop(true)` + `TRANSLUCENT` (posted: the format change recreates GPUI's surface, and the renderer re-attaches as after Home); back to opaque and below the window when the last one goes |
| `View` attached | added `GONE` to the underlay, a `FrameLayout` under GPUI's view in the Activity's content |
| `SurfaceControl` attached | reparented under GPUI's surface at layer −1 (under GPUI's buffer); again on every new GPUI surface |
| a frame moves / resizes / shows / hides views | one JNI call before the renderer draws: `View`s get `FrameLayout` params (laid out in this vsync's traversal) or `GONE`; `SurfaceControl`s get position and visibility in a transaction applied with GPUI's next buffer (`applyTransactionToFrame`, API 34+; at once on 29–33) and their buffer size at once |
| touch down | `NativeViewTouchDispatcher` (React Native's `JSTouchDispatcher`, ported) asks Rust's hit test (`on_hit_test_native_view`); the gesture goes to that view until up or cancel, besides GPUI |

Frames that change no placement call nothing, so the view's own frames never wake GPUI.

## Checked (2026-11-28, API 35 and API 28 emulators)

| Check | Result |
|---|---|
| GPUI frames/s while the surface draws ~55/s and the WebView idles | 0 |
| `SurfaceControl` slot moving and resizing every frame, 40 shots | position never off; a growing edge shows the background for up to one producer frame (the old-size buffer is not scaled) |
| `SurfaceView` slot moving every frame, 20 shots | 14 exact, 6 one frame behind (the window's frame is not GPUI's) |
| input | surface `down`/`up` at view-relative (238.9, 217.1); the div over it hears its own tap, the view nothing; WebView tap runs the page's `onclick`, a swipe scrolls it (`scrollY 209`) |
| rotation, Home and back | views re-laid out, `SurfaceControl` re-parented, no gap |
| `place` JNI call (debug build) | median 237 µs, p90 491 µs per placing frame |

The emulator's GL is a 16-bit EGL config (no alpha): the renderer logs `only Opaque, so native views show black`, and
holes are black (API 28: GPUI's layer at z +1, the app's `SurfaceView` at z −2 placed at its hole, both in
`dumpsys SurfaceFlinger`; input works). The API 35 checks ran with Vulkan first on the emulator, a local change: real
devices take Vulkan first already ([android.md](android.md)). Holes need Vulkan, or GL with an alpha config.

## Limits

- A `View` moves with the window's frame, not GPUI's: while it moves every frame the hole may lead by a frame.
- Accessibility of embedded views: 0.2.x.

## Tests

[native_view_placement.rs](../crates/android/src/native_view_placement.rs): placements reach Java only when they
change, and the background rule (host `cargo test -p parallax-android`). `NativeViewTouchDispatcherTest` (Robolectric,
[android-tests.md](android-tests.md)): one row per gesture rule.
