# Native views on Android

`NativeView` on Android ([native-view.md](native-view.md) is the shared design): the app's `View` or `SurfaceControl`
sits under GPUI's surface and shows through the hole the wgpu renderer cuts (patch
[0047](../patches/0047-wgpu-native-view-hole.patch)). No fork patch: it is all in parallax-android —
[native_view.rs](../crates/android/src/native_view.rs) decides, Java's
[NativeViews](../crates/android/java/io/github/xuckless/parallax/NativeViews.java) acts. How each kind of view is
hosted, and what each costs: [native-view-android-hosting.md](native-view-android-hosting.md). The checks:
[native-view-android-checks.md](native-view-android-checks.md).

## Using it

1. Build the view with the Activity: `parallax_android::with_activity(window, |env, activity| …)`.
2. `NativeView::new(ptr, window, cx)`, `ptr` a JNI reference to one of
   - a **`SurfaceControl`** with a buffer (API 29+), rendered through `new Surface(control)`: video, a browser
     engine's frames, camera. Its buffer size follows the element (in pixels);
   - any parentless **`View`**: a `WebView`, a `MapView`, an ordinary `SurfaceView` (API 28's surface);
   - a `View` wrapped by **`parallax_android::in_surface(env, &view)`** (API 34+; an ordinary `View` below): drawn in
     a surface of its own that moves in step with GPUI's frame. For views without text input (video, a map).
3. Touches: GPUI takes them all (`NativeViewInput::Window`): the view's element hears them, positions relative to
   `view.bounds(window)`. `parallax_android::forward_touches(window, &view, mode)` sends each gesture (and mouse wheel
   turn) that *starts* on the view, no GPUI element over it there, to the `View` too, offset into its coordinates, so
   a WebView scrolls and takes taps. `TouchForwarding::On` gives it to both, so a WebView inside a GPUI scroll
   container scrolls both; `TouchForwarding::Exclusive` gives it to the `View` alone.
4. Focus follows GPUI's: Android focus (keys, the IME) moves into the `View` while the view's handle is focused —
   GPUI focuses it when the view's element is tapped, or `window.focus(&view.focus_handle(cx), cx)` — and back to
   GPUI's view when anything else is. The `View` never takes focus by itself (a WebView does on touch). Back always
   goes to GPUI first (`on_back_gesture`); unclaimed, the Activity's back follows.
5. `view.set_background(colour)` paints the underlay: what shows where a view has not caught up with its hole.

[examples/native-view](../examples/native-view/) ([android.rs](../examples/native-view/src/android.rs)) does all of
this (`--es view control|view|hosted` picks the surface's kind, `--ez animate true` moves its slot every frame,
`--ez stack true` shows two overlapping views in place of the WebView; [examples-native-view.md](examples-native-view.md)).

## What happens

| Event | Java does |
|---|---|
| first `NativeView` attached | GPUI's `SurfaceView` goes `setZOrderOnTop(true)` + `TRANSLUCENT` (posted: the format change recreates GPUI's surface, and the renderer re-attaches as after Home); back to opaque and below the window when the last one goes |
| first frame showing a view on GL (API 30+) | the renderer is rebuilt on Vulkan, keeping its atlas: GL surfaces have no alpha ([hosting](native-view-android-hosting.md#gl-and-vulkan)) |
| `View` attached | added `GONE` to the underlay, a `FrameLayout` under GPUI's view in the Activity's content, which blocks focus to its children |
| `SurfaceControl` attached | reparented under GPUI's surface, under GPUI's buffer; again on every new GPUI surface |
| a frame moves / resizes / restacks / shows / hides views | one JNI call before the renderer draws: `View`s get `FrameLayout` params, Z and clip bounds (laid out in this vsync's traversal) or `GONE`; `SurfaceControl`s get position, layer, crop (API 33+) and visibility in a transaction applied with GPUI's next buffer (`applyTransactionToFrame`, API 34+; at once on 29–33) and their buffer size at once |
| GPUI focuses a view's handle, or something else | the underlay lets focus in and the `View` takes it; or blocks it again and GPUI's view takes it back |
| Back while a `View` has focus | the underlay hands it to GPUI's view first (`KeyboardManager`, as without views) |
| touch down, wheel turn | `NativeViewTouchDispatcher` (React Native's `JSTouchDispatcher`, ported) asks Rust's hit test (`on_hit_test_native_view`) and the view's touch mode; the gesture goes to that view until up or cancel, besides GPUI or instead of it |

Frames that change no placement call nothing, so the view's own frames never wake GPUI.

## Limits

- A plain `View` moves with the window's frame, not GPUI's: while it moves every frame the hole may lead by a frame
  (`in_surface` hosting moves in step, at the cost of focus and the IME).
- A `SurfaceControl` clipped by a content mask shows whole on API 29–32 (no crop before 33).
- While a `View` has focus, keys reach it directly: GPUI's key bindings don't see them (Back excepted).
- Accessibility of embedded views: 0.2.x.

## Tests

[native_view_placement.rs](../crates/android/src/native_view_placement.rs): placements (with layer and clip) reach
Java only when they change, and the background as a colour int (the rules are the core's,
[native-view-placement.md](native-view-placement.md)); [backend.rs](../crates/android/src/backend.rs): when a GL
renderer moves to Vulkan (host `cargo test -p parallax-android`). Robolectric ([android-tests.md](android-tests.md)):
`NativeViewTouchDispatcherTest`, one row per gesture and wheel rule per touch mode; `NativeViewsTest`, focus,
stacking and clip rows.
