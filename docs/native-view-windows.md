# NativeView on Windows

Patch [0050](../patches/0050-windows-native-view.patch) shows an app's own content under GPUI's window through
DirectComposition, which GPUI already presents with. No child window (no airspace: GPUI's `HWND` keeps all input), no
copy of the app's frames, and the app's presents reach the screen with no GPUI frame. Input goes to the view's element
([0048](../patches/0048-native-view-window-input.patch), `NativeViewInput::Window`). Plan: ROADMAP "Phase: NativeView".

## How an app uses it

`NativeView::new(ptr, window, cx)` takes an `IUnknown*` of anything `IDCompositionVisual::SetContent` takes: in
practice a composition swap chain (`IDXGIFactory2::CreateSwapChainForComposition`) the app presents to on its own
D3D11 device and thread. GPUI keeps a reference until the last handle drops. Its content shows 1:1 in device pixels
from the element's top left, clipped to its content mask; size the swap chain from `view.bounds(window)` × scale and
`ResizeBuffers` when it changes. Until it does, `set_background`'s colour (black by default) shows where the content
doesn't reach. Listen on `view.clone().into_element()` (`on_mouse_move`, `on_mouse_down`, `on_scroll`, `on_key_down`)
and subtract `bounds.origin` for positions in the view. [native-view-windows](../examples/native-view-windows/src/)
is the whole pattern.

**CEF (wubble):** off-screen rendering with `shared_texture_enabled` hands `OnAcceleratedPaint` a D3D11 shared texture
handle each frame. Open it on the app's device (`ID3D11Device1::OpenSharedResource1`), `CopyResource` it into the
composition swap chain's back buffer, `Present`. That one GPU blit is CEF's OSR cost, not Parallax's: GPUI neither
copies nor draws for it. **WebView2:** `CreateCoreWebView2CompositionController` wants a visual in the host's own
DComp tree (`RootVisualTarget`); GPUI doesn't expose its device yet, so WebView2 needs a follow-up hook.

## How it works

| Piece | Where |
|---|---|
| Visual tree, back to front under one root: background, the views (a clip visual over a content visual each, after Chromium's `VisualSubtree`), GPUI's swap chain | `directx_renderer/native_view_tree.rs` (new in 0050), `DirectComposition::new` |
| What a frame shows (placement in whole device px, order, background) and the alpha mode, as plain values with host tests | `native_views.rs` (pure, in [patch-test.sh](../scripts/patch-test.sh)'s windows-pure row) |
| The hole: `PrimitiveBatch::NativeViews` writes transparent black, blending off, at the batch's depth, so later opaque quads keep their pixels | `native_view_*` in `shaders.hlsl` |
| Frame order: render, `Present`, then restack/place only what changed and `Commit` only then (Chromium's `CommitAndClearPendingOverlays`), so hole and placement reach the same composition frame | `prepare_native_views`, `commit_native_views` |

Overhead: an opaque window with no view shown keeps `DXGI_ALPHA_MODE_IGNORE`; only while a view shows is the swap chain
`PREMULTIPLIED` (recreated on the switch, logged at info: `native views: composition swap chain alpha mode …`).
Everything outside the hole stays opaque. A frame that moves no view costs no `Commit`.

## Checks

[windows-native-view-checks.sh](../scripts/windows-native-view-checks.sh) runs in CI's `windows-touch` job
([ci.md](ci.md)) at 100 and 125 %: it launches the example through [touch-inject](windows-touch-harness.md), reads the
composed screen back (`BitBlt` with `CAPTUREBLT`, which includes DComp; [pixels.py](../scripts/pixels.py)) and
injects mouse moves, wheel and clicks. Per scale: alpha mode logged; 0 GPUI frames/s over 3 s while the view presents
(the example counts paints and presents per second); the hole shows the view and the cover div over it shows (GPUI's
visual is above the view's); one device pixel past each edge is GPUI's (no seam); no gap colour in 8 shots while the
view slides, nor after a resize; a move, wheel and click over the view reach its element at 200 150 in the view; a click
on the cover stays the cover's. Logs and screenshots: the `windows-touch` artifact, `target/native-view-windows/`.

Not measurable on the runner (WARP, no real GPU): DWM's GPU cost per view frame, and whether a present and a commit
can land in different DWM frames under load; DComp gives no API to tie them, Chromium relies on the same order.

Cross-checking on Linux: `cargo clippy --target x86_64-pc-windows-gnu` works with a stub `x86_64-w64-mingw32-windres`
on `PATH` (gpui's build script embeds a manifest; `cargo check` never links it).
