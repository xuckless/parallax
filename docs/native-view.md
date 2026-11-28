# Native views: the shared design

A `NativeView` shows an OS-native view (a browser page, video, map) under GPUI's content, through a hole GPUI
paints. The view's own frames reach the screen through the OS compositor and draw no GPUI frame
([ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview), issue #1). Upstream GPUI has it on macOS only; this
page is what every target shares. Per target: [native-view-wayland.md](native-view-wayland.md),
[native-view-windows.md](native-view-windows.md) (and [WebView2](native-view-webview2.md)), [native-view-apple.md](native-view-apple.md),
[native-view-android.md](native-view-android.md), [native-view-web.md](native-view-web.md). Runtime checks per target:
[Wayland](native-view-wayland-checks.md), [Apple](native-view-apple-checks.md), [Android](native-view-android-checks.md),
[web](native-view-web-checks.md); Windows' are in [native-view-windows.md](native-view-windows.md).

## One API

`NativeView::new(view, window, cx)` takes the platform's own view as a pointer, as upstream's macOS one takes an
`NSView`: a `UIView` on iOS, DirectComposition content (or a visual on GPUI's device, as WebView2 wants) on Windows, the app's `wl_surface` on Wayland (owner
decision, 2026-11-27), a JNI reference to a `View` or `SurfaceControl` on Android, a DOM element (`web_sys::HtmlElement`)
on the web. The handle is an element: paint it
where the view goes, and elements painted after it cover it. `NativeView::bounds(window)` gives where it showed in the
last frame: the whole view, so pointer positions stay view-relative when a clip cuts it. A view its element's clip
cuts (scrolled partly out of a container) shows only the part the clip leaves, at its full size and position, on
every target (a clipping container on Apple, a clip visual on Windows, a crop on Wayland, `setClipBounds` or a
`SurfaceControl` crop on Android, API 33+ for the crop; the slot's clip on the web); clipped away entirely, it
hides. On Wayland the app commits every buffer through `gpui_linux::NativeViewBuffers`, which sets that crop for
each buffer's size ([native-view-wayland.md](native-view-wayland.md#clipped-views)).

## Placement

What shows of a painted view (its visible part, rounding for a coarser grid, the hole inset, the background colour) is
decided once, in gpui's `native_view_placement`: [native-view-placement.md](native-view-placement.md).

## The hole

The renderer turns `PrimitiveBatch::NativeViews` into a hole: the view's bounds, clipped to its content mask, replace
what was painted beneath with transparent black, in paint order (Chromium's `ReplaceExistingQuadWithHolePunch`,
Metal's `native_view_fragment`).

- **Metal** (macOS, iOS): upstream's; patch [0051](../patches/0051-native-view-in-step.patch) presents in step.
- **Direct3D 11** (Windows): patch [0050](../patches/0050-windows-native-view.patch)'s `native_view` shader.
- **wgpu** (Linux, Android, web): patch [0047](../patches/0047-wgpu-native-view-hole.patch), one hole for every wgpu
  target, in gpui_wgpu's new `wgpu_renderer/native_view_hole.rs` and `shaders_native_view.wgsl`. Holes are `Quad`s,
  so both instance transports (storage buffers, WebGL2) decode them; the pipeline doesn't blend, and its depth test
  keeps opaque quads painted after the view on top.
- **Alpha:** while the window has a native view attached (`Scene::native_views_attached`, shown or hidden), the wgpu
  surface uses its premultiplied alpha mode and has its hole pipeline; once the last view is dropped both go, so a
  window without views keeps its opaque fast path and never builds the pipeline, and hiding and showing a view
  rebuilds nothing. The mode is logged when the first view is attached: `native views: window surface alpha mode
  PreMultiplied` (on a WebGL2 canvas, which wgpu reports as `Opaque`: `native views: WebGL2 canvas, premultiplied
  alpha (the context's default)`, `NativeViewSupport::WebGl2Premultiplied`), or a warning. A surface that can't blend fails `WgpuRenderer::native_view_support()`
  (`NativeViewsUnsupported`), so Wayland's `NativeView::new` fails and the app can fall back.
- **Hole inset:** `WgpuRenderer::set_native_view_hole_inset` cuts holes inside the views' bounds, for a platform that
  rounds views to whole logical pixels (Wayland at a fractional scale).

## Input

`PlatformWindow::native_view_input()` says who gets pointer input over a shown view (`NativeViewInput`, one switch for
every backend, patch [0048](../patches/0048-native-view-window-input.patch)):

| `NativeViewInput` | Targets | Pointer input over the view |
|---|---|---|
| `View` | macOS, web | the view (AppKit hit-tests it; the browser does, with the canvas's `pointer-events` toggled by GPUI's hit test, patch [0054](../patches/0054-web-native-view.patch)); the view's element blocks the mouse from everything behind |
| `Window` | Wayland, Windows, iOS, Android | GPUI: the view's element occludes like a solid element (`HitboxBehavior::BlockMouseExceptAncestors`), so it and its ancestors hear the input, elements painted before it don't, elements painted after it keep theirs |

With `Window`, add listeners to the view's element (`view.clone().into_element().on_mouse_move(…)`) and subtract
`view.bounds(window)`'s origin for view-relative positions. The hit test skips every hitbox behind the view whose
element is not on the view's dispatch path (`Occlusion` in gpui's `window/native_view_input.rs`). Keys while the
view's `FocusHandle` is focused follow macOS's rule for its views: `reserve_action` bindings and the rest of a pending
binding first, then the view's element's key listeners (forward the key to the view and stop its propagation), then
key bindings with what the element leaves.

## Placement in step

A frame that moves, resizes, shows or hides a view must place the view in the same compositor transaction as the hole
(`places_views()`): Core Animation's transaction on Apple, `wl_subsurface.set_sync` on Wayland, GPUI's own
DirectComposition `Commit` on Windows, which also shows GPUI's frame while a view shows (a DComp surface, as a swap
chain's present is not tied to a commit), the transaction applied with GPUI's next buffer on Android (a `SurfaceControl`,
API 34+; a `View` moves with the window's own frame), the animation frame that draws the canvas on the web. Every other frame leaves the view alone, so its own frames never wait for
GPUI.

## Tests

- `wgpu_renderer::tests::native_view_hole::native_view_hole_is_cut_in_paint_order` (patch 0047): a table of pixels on
  the headless renderer (outside, under a quad painted after, erased opaque and translucent quads, the clip, the inset).
- `window::native_view::tests::window_input::window_input_over_a_native_view_reaches_its_element` (patch 0048): one
  row per input, with an element painted under the view that must hear none of it; `window::native_view_input::tests`:
  the hitbox behaviour per mode. A focused native view that is dropped hands focus back (0048's
  `focused_native_view_sent`); iOS also hands the keyboard back itself when the view goes (0052).
- Both run in [scripts/patch-test.sh](../scripts/patch-test.sh) ([patch-tests.md](patch-tests.md)).
