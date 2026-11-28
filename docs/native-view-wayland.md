# Native views on Wayland

Patch [0049](../patches/0049-wayland-native-view.patch): the app's `wl_surface` as a `wl_subsurface` below the window,
its hole cut by the wgpu renderer ([native-view.md](native-view.md)). Ported from Chromium's
`wayland_subsurface.cc` and `wayland_frame_manager.cc` ([third_party/chromium/PORTED.md](../third_party/chromium/PORTED.md))
into gpui_linux's new `wayland/window/native_view.rs`; `window.rs` holds four hooks. X11 and XWayland: no native views.

## Using it

1. Take the window's `wl_display` (`window.display_handle()`, `RawDisplayHandle::Wayland`) and open your own event
   queue on it (`wayland_backend::client::Backend::from_foreign_display`), bind `wl_compositor` (and `wl_shm` or
   `zwp_linux_dmabuf_v1`) there, and make a `wl_surface`. Give it no role, no `wp_viewport`, no subsurfaces.
2. `NativeView::new(surface_proxy, window, cx)`. Keep the surface alive until the last handle is dropped.
3. Attach, damage and commit buffers whenever you like, from any thread. Any buffer size: the view is scaled to its
   element (`wp_viewport.set_destination`); for a sharp view, use the element's size times `window.scale_factor()`.

[examples/native-view-wayland](../examples/native-view-wayland/) does exactly this with shm buffers
([examples.md](examples.md#native-view-wayland)).

## What GPUI does

| Frame | Requests, in order (all before the renderer's `wl_surface.commit` of the window) |
|---|---|
| first paint | `get_subsurface`, `place_below(window)`, `set_sync`, empty input region, `set_position`, `set_destination`, view `commit` |
| moved / resized | `set_sync`, `set_position` and/or `set_destination` (only what changed), view `commit` (cached) |
| no longer painted | `set_sync`, `attach(null)`, view `commit` (Chromium's mutter workaround: the role stays) |
| painted again | a fresh subsurface, as on first paint |
| any frame that placed a view | after the window's commit: `set_desync` |

The view is synchronized only for frames that place it, so its move lands in the window's commit with the hole's, and
desynchronized otherwise, so the app's commits show at once and never wake GPUI. Positions round outwards to whole
logical pixels; at a fractional scale the hole is cut one physical pixel inside, so the window covers the edge. An
opaque window (server decorations, opaque background) keeps its opaque region, minus the shown views. The empty input
region sends all input to GPUI (`NativeViewInput::Window`).

After a hide, the surface has no buffer: commit a new one when the view shows again (`view.bounds(window)` is `Some`).
mutter may never release a buffer that was attached when a view hid: treat it as released.

## Checked (2026-11-27)

| Setup | Alpha mode | GPUI frames/s, page at 60 | Input | Seams |
|---|---|---|---|---|
| owner's GNOME 50 / mutter, RTX 5060, NVIDIA 620.73.04 Vulkan, 1.0, 300 Hz | PreMultiplied | 0 | — | — |
| headless mutter, NVIDIA Vulkan client, 1.0 and 1.25 | PreMultiplied | 0 | view-relative, div takes its own | none |
| headless mutter, Mesa lavapipe client, 1.0 and 1.25 | PreMultiplied | 0 | view-relative | none |

NVIDIA's WSI offers premultiplied here, so no GL fallback is needed on this driver. With `--animate` (the view moves
and resizes every GPUI frame) `WAYLAND_DEBUG` shows the order above every frame, and a screenshot at 1.25 shows one
physical pixel of the container, then the page, on every edge. The headless checks are one repeatable script,
[wayland-native-view-checks.sh](../scripts/wayland-native-view-checks.sh), also run by CI
([native-view-wayland-checks.md](native-view-wayland-checks.md)). Not yet run: a mutter compositing on Mesa (this
machine has only the NVIDIA GPU).

Cost on the owner's session (debug build, 10 s each, window 900×600 with a 640×400 view):

| Case | GPUI frames/s | gnome-shell CPU | GPU utilization |
|---|---|---|---|
| `--no-view` (idle window) | 0 | 1.9 % | 1.1 % |
| page at 60 fps | 0 | 5.4 % | 1.7 % |
| page at 120 fps | 0 | 8.1 % | 2.5 % |

## Tests

`native_view::tests::steps` (placement × frame → step) and `subsurface_bounds_round_outwards` (rounding and inset at
1, 2 and 1.25) run in [scripts/patch-test.sh](../scripts/patch-test.sh); the input rules in
[native-view.md](native-view.md#tests).
