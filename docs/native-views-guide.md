# Native views: a guide

A guide for app code. A `NativeView` puts an OS-native view (a browser page, a video, a map, a camera preview) inside
a GPUI window. GPUI paints a hole where it goes and the OS compositor shows the view through it, so the view runs at
its own frame rate and GPUI draws no frame while only the view changes. How it works underneath:
[native-view.md](native-view.md); what each target was measured to do: [native-view-gate.md](native-view-gate.md).

## What you hand it, per target

| Target | The view | Its page |
|---|---|---|
| macOS / iOS | an `NSView` / `UIView` | [native-view-apple.md](native-view-apple.md) |
| Windows | DirectComposition content (a swap chain, a visual); a WebView2 page | [native-view-windows.md](native-view-windows.md), [WebView2](native-view-webview2.md) |
| Linux (Wayland) | your own `wl_surface`; commit its buffers through `gpui_linux::NativeViewBuffers` | [native-view-wayland.md](native-view-wayland.md) |
| Android | a JNI reference to a `View` or a `SurfaceControl` | [native-view-android.md](native-view-android.md) |
| Web | a DOM element (`iframe`, `video`, `canvas`, …) | [native-view-web.md](native-view-web.md) |

X11 has no native views.

## Using one

```rust
// SAFETY: `view` points to a live view of the kind the table names, in no other view hierarchy.
let view = unsafe { NativeView::new(view, window, cx) }?;
view.set_background(rgb(0x202020), cx);   // what shows until the view draws its first frame
```

The handle is an element: paint it where the view goes (`view.clone().into_element()`, sized like any div), and
whatever you paint after it covers it, so GPUI buttons can sit on top of a video. A clip (a scroll container) cuts
it on every target. `view.bounds(window)` says where it showed in the last frame. GPUI owns the view's frame, visibility and order until the last clone of the handle drops, which removes it (the
safety rules per platform are on `NativeView::new`).

## Who gets the pointer

| Targets | Over the view |
|---|---|
| macOS, web | the view itself (a page scrolls and clicks as in a browser) |
| Windows, Wayland, iOS, Android | GPUI: add listeners to the view's element and forward the input to the view; subtract `view.bounds(window)`'s origin for view-relative positions |

Keys follow focus: while the view's element is focused, its key listeners see keys first; forward them to the view.
Details: [native-view.md](native-view.md#input).

## The example

[examples/native-view](../examples/native-view/README.md) runs on every target: a native view in a slot, a GPUI div
over one corner, buttons that move, resize, hide and drop the views, and a frame counter that shows GPUI stays idle
while the view animates. What it shows per target: its README; options and log lines:
[examples-native-view.md](examples-native-view.md).
