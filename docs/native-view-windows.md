# NativeView on Windows

Patch [0050](../patches/0050-windows-native-view.patch) shows an app's own content under GPUI's window through
DirectComposition, which GPUI already presents with. No child window (no airspace: GPUI's `HWND` keeps all input), no
copy of the app's frames, and the app's presents reach the screen with no GPUI frame. Input goes to the view's element
([0048](../patches/0048-native-view-window-input.patch), `NativeViewInput::Window`). Plan: ROADMAP "Phase: NativeView".
A WebView2 page is a NativeView too (patch 0056): [native-view-webview2.md](native-view-webview2.md).

## How an app uses it

`NativeView::new(ptr, window, cx)` takes an `IUnknown*` of anything `IDCompositionVisual::SetContent` takes: in
practice a composition swap chain (`IDXGIFactory2::CreateSwapChainForComposition`) the app presents to on its own
D3D11 device and thread. GPUI keeps a reference until the last handle drops. Its content shows 1:1 in device pixels
from the element's top left, clipped to its content mask; size the swap chain from the element's laid-out bounds × scale
(the example reads them in a canvas's prepaint) and `ResizeBuffers` when they change. Until it does, `set_background`'s colour (black by default) shows where the content
doesn't reach. Listen on `view.clone().into_element()` (`on_mouse_move`, `on_mouse_down`, `on_scroll`, `on_key_down`)
and subtract `bounds.origin` for positions in the view. [native-view-windows](../examples/native-view-windows/src/)
is the whole pattern.

**CEF (wubble):** off-screen rendering with `shared_texture_enabled` hands `OnAcceleratedPaint` a D3D11 shared texture
handle each frame. Open it on the app's device (`ID3D11Device1::OpenSharedResource1`), `CopyResource` it into the
composition swap chain's back buffer, `Present`. That one GPU blit is CEF's OSR cost, not Parallax's: GPUI neither
copies nor draws for it.

## How it works

| Piece | Where |
|---|---|
| Visual tree, back to front under one root: background, the views (a clip visual over a content visual each, after Chromium's `VisualSubtree`), GPUI's own visual | `directx_renderer/native_view_tree.rs` (new in 0050), `DirectComposition::new` |
| What a frame shows (placement from core's `native_view_placement`, order, background) and the present step (`present_step`, one table), as plain values with host tests | `native_views.rs` (pure, in [patch-test.sh](../scripts/patch-test.sh)'s windows-pure row) |
| The hole: `PrimitiveBatch::NativeViews` writes transparent black, blending off, at the batch's depth, so later opaque quads keep their pixels | `native_view_*` in `shaders.hlsl` |
| GPUI's frame: rendered into the swap chain's back buffer; while a view shows, copied into a DComp surface instead of presented | `directx_renderer/native_view_surface.rs` (`Presenting::show`) |
| Frame order: render, show (present or copy), restack/place only what changed, then one `Commit` (Chromium's `CommitAndClearPendingOverlays`) | `prepare_native_views`, `present_native_views` |

**In step, exactly.** A swap chain's `Present` and a DComp `Commit` are not atomic (DComp has no API to tie them), so
with the swap chain alone about one frame in 20–40 of a sliding view showed a 3 px gap. A DComp surface
(`IDCompositionSurface::BeginDraw`/`EndDraw`) changes only on `Commit`, so while at least one view shows, GPUI's frame
is copied into one and its holes and the views' placement change in the same `Commit` (owner decision, 2026-11-27).

**Cost.** While a view shows: one GPU copy of GPUI's own frame (`CopySubresourceRegion`, window size) and one `Commit`
per GPUI frame, never per frame of a view's content; an idle window draws no GPUI frame, so copies nothing. With no
view shown GPUI presents its swap chain as upstream: no copy, no commit. The swap chain keeps upstream's
premultiplied alpha mode for every window (as does the surface), so a window without views is upstream's exactly and
showing a view recreates nothing. Leaving the surface takes two frames: the frame the last view goes is copied into the
surface (committed with the views' removal) and presented to the swap chain; the next presents and shows the swap
chain, which then already holds a current frame, so no stale one flashes. Logged at info: `native views: GPUI's
frame shows through a composition surface, alpha mode Premultiplied` and back. A view dropped while the window draws
leaves the tree at the next frame's start.
Each copy logs at debug (`gpui_windows::directx_renderer::native_view_surface`), which the checks count.

## Checks

[windows-native-view-checks.sh](../scripts/windows-native-view-checks.sh) runs in CI's `windows-touch` job
([ci.md](ci.md)) at 100 and 125 %: it launches the example through [touch-inject](windows-touch-harness.md), reads the
composed screen back (`BitBlt` with `CAPTUREBLT`, which includes DComp; [pixels.py](../scripts/pixels.py)) and
injects mouse moves, wheel and clicks. Per scale: alpha mode logged; 0 GPUI frames and 0 copies per second over 3 s
while the view presents (the example counts paints and presents per second); the hole shows the view and the cover
div over it shows (GPUI's visual is above the view's); one device pixel past each edge is GPUI's (no seam); no gap
colour in any of 100 screenshots (`touch-inject shots`) taken while the view slides, nor after a resize; one copy per
GPUI frame while it slides; a move, wheel and click over the view reach its element at 200 150 in the view; a click
on the cover stays the cover's. Then the same for the WebView2 page ([native-view-webview2.md](native-view-webview2.md)).
Logs and the screenshots with a gap: the `windows-touch` artifact, `target/native-view-windows/`.

Measured on the runner (WARP), CI runs 46110112660 and 46111332527, both scales: 0 gap shots of 100 while sliding;
597–629 GPUI frames during the slide with exactly as many surface copies (one per GPUI frame; the swap chain is not
presented then, by construction); 0 GPUI frames and 0 copies per second while the view presents 59–64/s. Before the surface
path, one run in four showed a gap shot. Not measurable there: the copy's GPU time and DWM's cost per view frame.

Cross-checking on Linux: `cargo clippy --target x86_64-pc-windows-gnu` works with a stub `x86_64-w64-mingw32-windres`
on `PATH` (gpui's build script embeds a manifest; `cargo check` never links it).
