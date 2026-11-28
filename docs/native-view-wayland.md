# Native views on Wayland

Patch [0049](../patches/0049-wayland-native-view.patch): the app's `wl_surface` as a `wl_subsurface` below the window,
its hole cut by the wgpu renderer ([native-view.md](native-view.md)). Ported from Chromium's
`wayland_subsurface.cc` and `wayland_frame_manager.cc` ([third_party/chromium/PORTED.md](../third_party/chromium/PORTED.md))
into gpui_linux's new `wayland/window/native_view.rs`; `window.rs` holds the hooks. X11 and XWayland: no native views.
What was measured (setups, alpha mode, cost): [native-view-wayland-results.md](native-view-wayland-results.md).

## Using it

1. Take the window's `wl_display` (`window.display_handle()`, `RawDisplayHandle::Wayland`) and open your own event
   queue on it (`wayland_backend::client::Backend::from_foreign_display`), bind `wl_compositor` (and `wl_shm` or
   `zwp_linux_dmabuf_v1`) there, and make a `wl_surface`. Give it no role, no `wp_viewport`, no subsurfaces.
2. `NativeView::new(surface_proxy, window, cx)`. Keep the surface alive until the last handle is dropped.
3. `gpui_linux::NativeViewBuffers::of(&view)`: a `Send` handle. Commit **every** buffer through it, from any thread:
   `buffers.commit_buffer(&surface, &buffer, size)` attaches, damages and commits with the crop GPUI placed the view
   with, set for that buffer's size. Any size: the view is scaled to its element (`wp_viewport.set_destination`); for a
   sharp view, use the element's size times `window.scale_factor()`. Keep buffer scale 1 and the normal transform (GPUI
   sets both at attach): the crop is in buffer pixels.
4. `Committed::NotShown`: the view isn't painted (not yet, or hidden), nothing was attached and the buffer is free;
   commit again once `view.bounds(window)` is `Some`.
5. Your thread uses GPUI's connection: stop and join it before GPUI disconnects, in `cx.on_app_quit` or when the view's
   window closes (dropping whatever owns the thread with the window's root view does both). The example's `Page` joins
   its thread on drop.

`NativeView::set_background(colour)` fills the hole where the view hasn't drawn (its first show, a show after a hide, a
resize it hasn't caught up with), in an opaque window; without it the desktop shows there.

## Clipped views

A subsurface can't be clipped, so for a view its element's clip cuts, GPUI places the subsurface over the visible part
only (rounded outwards to whole logical pixels, like any view) and crops the buffer to match with
`wp_viewport.set_source`, in buffer pixels rounded outwards and kept inside the buffer. A source past the buffer's edge
is a protocol error that ends the app's connection, so the crop is kept as fractions of the view, and GPUI's placing
commits and the app's `commit_buffer` take turns under one lock: each sets the source for the buffer it commits with
(the attached one for GPUI's). A frame of an old size still in flight after a resize gets the old size's source.
Clipped away entirely, the view hides. Pointer positions stay relative to the whole view.

[examples/native-view](../examples/native-view/) ([wayland.rs](../examples/native-view/src/wayland.rs)) does exactly
this with shm buffers ([examples-native-view.md](examples-native-view.md)), `--resize` with stale old-size frames from
its thread.

## What GPUI does

| Frame | Requests, in order (all before the renderer's `wl_surface.commit` of the window) |
|---|---|
| first paint | `get_subsurface`, `set_sync`, empty input region, `set_position`, `set_destination`, `set_source` (for the attached buffer), view `commit` |
| moved / resized / clipped differently | `set_sync`, `set_position`, `set_destination` and/or `set_source` (only what changed), view `commit` (cached) |
| no longer painted | `set_sync`, unset source, `attach(null)`, view `commit` (Chromium's mutter workaround: the role stays) |
| painted again | the kept subsurface, configured as on first paint |
| the shown views or their order changed | `place_below(window)` for the underlay, then every shown view in paint order (the last is frontmost) |
| a background colour asked, opaque window | the underlay: GPUI's own subsurface under the views, a 1×1 buffer of the colour (`wp_single_pixel_buffer_v1`, else shm) stretched over the views' bounds |
| any frame that placed a view | after the window's commit: `set_desync` |
| a frame whose draw failed | the views placed back where the window's last commit shows them, so a retry of the old buffer shows them in step |

The view is synchronized only for frames that place it, so its move lands in the window's commit with the hole's, and
desynchronized otherwise, so the app's commits show at once and never wake GPUI. Positions round outwards to whole
logical pixels (core's `round_outwards`); at a fractional scale the hole is cut one physical pixel inside
(`hole_inset`), so the window covers the edge ([native-view-placement.md](native-view-placement.md)). An opaque window
(server decorations, opaque background) keeps its opaque region, minus the shown views. The empty input region sends
all input to GPUI (`NativeViewInput::Window`). mutter may never release a buffer that was attached when a view hid:
treat it as released.

## Tests

`native_view::tests::steps` (placement × frame → step, the kept subsurface on a re-show),
`clipped_views_show_their_visible_part` (the crop: half scrolled out at 1 and 1.25, a clip partway through a pixel,
clipped on the left at 2, clipped away), `frames_stack_views_in_paint_order_over_an_underlay` and
`sources_stay_inside_the_buffer` (a resize's smaller buffer included) run in [scripts/patch-test.sh](../scripts/patch-test.sh);
rounding and the inset in core's `native_view_placement::tests`; the input rules in [native-view.md](native-view.md#tests).
The live checks: [native-view-wayland-checks.md](native-view-wayland-checks.md).
