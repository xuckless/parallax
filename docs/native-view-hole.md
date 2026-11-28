# Native views: the hole

Part of [native-view.md](native-view.md): how each renderer cuts the hole a native view shows through, and when the
window's surface blends. Plan: [ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview).

The renderer turns `PrimitiveBatch::NativeViews` into a hole: the view's bounds, clipped to its content mask, replace
what was painted beneath with transparent black, in paint order (Chromium's `ReplaceExistingQuadWithHolePunch`,
Metal's `native_view_fragment`).

- **Metal** (macOS, iOS): upstream's; patch [0060](../patches/0060-apple-native-view-in-step-present.patch) presents in step.
- **Direct3D 11** (Windows): patch [0056](../patches/0056-windows-native-view-hole.patch)'s `native_view` shader.
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

## Tests

- `wgpu_renderer::tests::native_view_hole::native_view_hole_is_cut_in_paint_order` (patch 0047): a table of pixels on
  the headless renderer (outside, under a quad painted after, erased opaque and translucent quads, the clip, the inset).
  It runs in [scripts/patch-test.sh](../scripts/patch-test.sh) ([patch-tests.md](patch-tests.md)).
