# Code ported from Chromium

Chromium is BSD-3-Clause (see `LICENSE` here, from the pinned commit). Every file that ports or adapts Chromium code
gets one row. Pinned commit: `4fc26f32cd03f0e8443c4aa11d6859a901d90659`.

Sync rule: at each upstream sync, diff each source path between the pinned commit and Chromium's current one and
port the fixes that apply.

| Chromium path | Commit | Parallax path | Kind (port / model) | Dropped parts |
|---|---|---|---|---|
| `ui/gl/dc_layer_tree.cc` | `4fc26f3` | patch 0050: `gpui-pre-windows/src/directx_renderer/native_view_tree.rs`, `gpui-pre-windows/src/native_views.rs` | model (structure and order, rewritten in Rust) | Kept: one subtree per overlay (clip visual over a content visual offset to the quad's origin), subtrees stacked back to front with `AddVisual(…, TRUE, left_sibling)`, a background colour from a small solid surface scaled to size, `Commit` only when a property changed, presenting before committing. Dropped: `VisualTree2`/`DCLayerTree2`, video processor and swap chain presenters (`SwapChainPresenter`, YUV, HDR, letterboxing), delegated ink, rounded-corner and transform visuals (GPUI's hole is axis-aligned, its rounded corners are GPUI's own pixels), opacity, soft borders, subtree reuse matching by content (views have ids), D3D12 solid colour pool (one 1×1 swap chain) |
| `components/viz/service/display/dc_layer_overlay.cc` | `4fc26f3` | patch 0050: `gpui-pre-windows/src/shaders.hlsl` (`native_view_*`), `native_view_tree.rs` (`draw_native_view_holes`) | model | Kept: an underlay is shown through a transparent hole punched in the main surface in paint order (`ReplaceExistingQuadWithHolePunch` / underlay mode). Dropped: overlay candidate selection, damage, protected video, the overlay/underlay heuristics |
