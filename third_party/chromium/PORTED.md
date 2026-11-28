# Code ported from Chromium

Chromium is BSD-3-Clause (see `LICENSE` here, shipped as `LICENSE-CHROMIUM` by every crate that carries a port:
`parallax-gpui-linux` and `parallax-gpui-windows`, through [fork-crates.sh](../../scripts/fork-crates.sh)'s `ported`
table). Every file that ports or adapts Chromium code gets one row, with what it left out (AGENTS.md porting rules 3, 4
and 10 apply as for Flutter). Each row pins its own Chromium commit (`main`, both 2026-11): Wayland rows
`4ea2b624ed0c500ec2f23ff17203dff91ad8faa2`, Windows rows `4fc26f32cd03f0e8443c4aa11d6859a901d90659`.

Sync rule: at each upstream sync, diff each source path between its row's commit and Chromium's current `main` and
port the fixes that apply.

| Chromium path | Commit | Parallax path | Kind (port / model / oracle) | Dropped parts |
|---|---|---|---|---|
| `ui/ozone/platform/wayland/host/wayland_subsurface.cc` | `4ea2b624` | `patches/0049-wayland-native-view.patch` (`gpui-pre-linux/src/linux/wayland/window/native_view.rs`) | port | the sibling stacking list (`RemoveFromList`, `InsertAfter`/`InsertBefore`, `place_above` a sibling): every view goes directly below the window (`place_below(parent)`); the `WaylandSurface` wrapper and its root-window bookkeeping; `AdjustSubsurfaceBounds`'s rounding up of the origin (views round outwards, the hole is cut inside instead). Changed: `set_position` and `set_destination` are sent only when they changed |
| `ui/ozone/platform/wayland/host/wayland_frame_manager.cc` | `4ea2b624` | same file (`WaylandSubsurface::hide`) | port | everything but `PlayBackFrame`'s hiding of a subsurface and its mutter workaround (a null buffer committed in sync instead of destroying the role): frame records, buffer bookkeeping and release tracking (the app owns its buffers), presentation feedback, frame callbacks, explicit sync, the occlusion timer |
| `ui/gl/dc_layer_tree.cc` | `4fc26f3` | patch 0050: `gpui-pre-windows/src/directx_renderer/native_view_tree.rs`, `gpui-pre-windows/src/native_views.rs` | model (structure and order, rewritten in Rust) | Kept: one subtree per overlay (clip visual over a content visual offset to the quad's origin), subtrees stacked back to front with `AddVisual(…, TRUE, left_sibling)`, a background colour from a small solid surface scaled to size, `Commit` only when a property changed, presenting before committing. Dropped: `VisualTree2`/`DCLayerTree2`, video processor and swap chain presenters (`SwapChainPresenter`, YUV, HDR, letterboxing), delegated ink, rounded-corner and transform visuals (GPUI's hole is axis-aligned, its rounded corners are GPUI's own pixels), opacity, soft borders, subtree reuse matching by content (views have ids), D3D12 solid colour pool (one 1×1 swap chain) |
| `components/viz/service/display/dc_layer_overlay.cc` | `4fc26f3` | patch 0050: `gpui-pre-windows/src/shaders.hlsl` (`native_view_*`), `native_view_tree.rs` (`draw_native_view_holes`) | model | Kept: an underlay is shown through a transparent hole punched in the main surface in paint order (`ReplaceExistingQuadWithHolePunch` / underlay mode). Dropped: overlay candidate selection, damage, protected video, the overlay/underlay heuristics |
