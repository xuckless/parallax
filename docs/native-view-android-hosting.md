# How Android native views are hosted

Each attached view is one `Hosted` in Java's
[NativeViews](../crates/android/java/io/github/xuckless/parallax/NativeViews.java), picked by what the app passed to
`NativeView::new` ([native-view-android.md](native-view-android.md) has the API). To host another kind, add an
implementation and its row in `NativeViews.attach`.

| Kind | Hosted as | Moves with | Input, focus, IME, accessibility |
|---|---|---|---|
| `View` | `ViewHosted`: a child of the underlay, a `FrameLayout` under GPUI's view | the window's next traversal: up to a frame behind GPUI's hole | forwarded touches and wheel; focus when GPUI focuses its handle; IME and accessibility as any View |
| `SurfaceControl` (API 29+) | `ControlHosted`: a child of GPUI's surface, under its buffer | GPUI's buffer (API 34+, `applyTransactionToFrame`); at once on 29–33 | none: GPUI's element hears the touches |
| `in_surface(view)` (API 34+) | `PackageHosted`: the View in a `SurfaceControlViewHost`, its `SurfacePackage`'s control placed as a `SurfaceControl` | GPUI's buffer; a new size is laid out in the host's next traversal | its own window: none from GPUI (no forwarding, no focus, no IME) |

`in_surface` is opt-in because of that last column: a WebView or anything with text keeps the `View` path. Below
API 34 `SurfacePackage.getSurfaceControl` does not exist, so `in_surface` hosts the View as an ordinary one (logged).

## Stacking

GPUI's paint order decides which view is on top, never attach order: each placement carries the view's rank among the
frame's `n` views as a layer, `-n` (painted first) to `-1` (last). `SurfaceControl`s take it as their layer under
GPUI's buffer (layer 0); `View`s take it as Z, which the underlay draws and hit-tests by (negative, so no shadow).
`SurfaceControl`s are children of GPUI's surface, above the window, so they are always above every `View`. A
`SurfaceView` passed as a `View` draws in a surface behind the window that Android stacks by its own rules (attach
order), whatever its Z: overlapping surfaces want `SurfaceControl`s.

## Clip

A placement carries the part GPUI's content mask leaves (a scroll container clipping the view), from the core's
`visible()` ([native-view-placement.md](native-view-placement.md)), as a clip rect in the view's own pixels:
`View.setClipBounds`, `SurfaceControl` crop (API 33+; 29–32 show the view whole). A view the mask leaves nothing of
is hidden. Touches forwarded to a clipped `View` are not clipped: GPUI's hit test already stops them outside.

## The growing edge

A `SurfaceControl` gets its new buffer size at once, but the producer's next buffer has it: while the slot grows,
the new strip shows the underlay for up to one producer frame. Moves and shrinks are exact. The answer is
`NativeView::set_background`: a colour close to the content (a video's black) hides the strip. A producer that keeps
its queue full shows the strip longer: buffers queued at the old size reach the screen before the new size. Paced to
exactly 60 frames/s by a deadline, the example's producer gave strips up to 34 px in 120 emulator screenshots, against
7 px with its sleep of 16 ms after each frame (37–44 frames/s there; [checks](native-view-android-checks.md)).

## GL and Vulkan

The wgpu renderer cuts holes with a surface that has alpha. wgpu-hal's GL surfaces offer only `Opaque` (30.0.2,
ROADMAP Upstream notes), and the emulator's GL config is 16-bit RGB 565, so on GL a hole is black. When a window first
shows a native view on GL, [activity_window.rs](../crates/android/src/activity_window.rs) rebuilds the renderer on
Vulkan through `renderer::recover` (the atlas stays; the rule is `FOR_NATIVE_VIEWS` in
[backend.rs](../crates/android/src/backend.rs)), once per renderer. API 30+ only: on API 28–29 the emulator's GL
stays, and holes show black there (the views are placed and take input; real devices draw with Vulkan already). GL
stays first on emulators otherwise ([android.md](android.md)).
