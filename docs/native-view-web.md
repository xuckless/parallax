# Native views on the web

Patch [0054](../patches/0054-web-native-view.patch): a DOM element (an iframe, a video, a canvas) shown under GPUI's
canvas through the wgpu renderer's hole ([native-view.md](native-view.md)). Code: gpui_web's new `native_views.rs`
(the DOM shell) and `native_view_layout.rs` (pure decisions); the slot lifecycle is ported from Flutter's
`content_manager.dart` and `slots.dart` ([PORTED.md](../third_party/flutter/PORTED.md)).

## Using it

`NativeView::new(NonNull::from(&element).cast(), window, cx)` with a `web_sys::HtmlElement` (GPUI keeps its own
reference). GPUI moves the element into its slot, so make it fresh: moving an `<iframe>` already in the document reloads
it. Give it `width` and `height` (`100%` fills the view; unset, GPUI sets `100%` and warns, as Flutter does).
`view.set_background(colour, cx)` colours the slot: a cross-origin iframe draws in its own process and shows its new
size a frame after a resize, and the slot's colour fills that frame (white for most pages).
[examples/native-view-web](../examples/native-view-web/) does all of this ([examples.md](examples.md#native-view-web)).

## What GPUI does

- **DOM.** A `<gpui-native-views>` layer before the canvas, which is raised above it (`position: relative;
  z-index: 1`); per view a `<gpui-native-view-slot>` (bounds clipped to the content mask, `overflow: hidden`) around a
  `<gpui-native-view>` (the view's bounds) around the element; above the slots, the shields.
- **Placement.** In `draw`, before the renderer draws: a view whose placement changed gets `transform:
  translate(..)`, `width` and `height`, rounded outwards to whole device pixels, so the canvas covers the overhang and
  no seam shows at 1.25. Unchanged views are not touched. Not painted: `display: none` (an iframe keeps its page).
  DOM changes and the canvas reach the screen in the same browser frame.
- **Alpha.** WebGPU: the canvas is `premultiplied` only while a view shows (0047), logged as `native views: window
  surface alpha mode PreMultiplied`. WebGL2: the context is premultiplied from creation (the browser's default, which
  wgpu keeps), logged as `native views: WebGL2 canvas, premultiplied alpha (the context's default)`.
- **Input** (`NativeViewInput::View`, as macOS). A capture listener on the document sees each pointer event first,
  asks GPUI's hit test (`hit_test_native_view`), and sets the canvas's `pointer-events` (`native_view_layout::step`,
  one row per state × event): `none` over an uncovered view, so the browser gives the element trusted events (iframe
  clicks and wheels, text selection, video controls); `auto` elsewhere. Over a view, GPUI's hitboxes painted after it
  (`PlatformWindow::on_native_view_occluders`) get transparent shields, so the pointer leaving an iframe for a GPUI
  div lands on GPUI's side; an event the browser gives a shield or the layer is copied to the canvas. A drag stays
  with its press's target. Views that start to show, and every touch release, give the views the pointer, so a tap
  needs no hover. Focus: `set_focused_native_view` focuses the element; its `focusin` focuses the view's handle.
- **Scale emulation.** DevTools' device mode and Playwright's `deviceScaleFactor` scale `devicePixelRatio` but not
  `devicePixelContentBoxSize`; gpui_web now trusts that box only where it agrees with the CSS box times the ratio (patch [0055](../patches/0055-web-canvas-device-pixel-size.patch)).

Costs: the capture listeners (and a non-passive document `wheel` listener) exist once a view was attached; frames that
place nothing write no style; the shields are rebuilt only when GPUI's hitboxes over a view change.

## Checks

Build and serve the example on two ports (the second origin for the cross-origin iframe):

```sh
wasm-pack build examples/native-view-web --target web --no-typescript --no-pack --dev
python3 -m http.server 8770 --bind 127.0.0.1 --directory examples/native-view-web &
python3 -m http.server 8771 --bind 127.0.0.1 --directory examples/native-view-web &
node scripts/web-checks.mjs http://127.0.0.1:8770/ desktop|mobile <chromium port>|firefox native-view [scale]
```

Chromium as in [web-text-input.md](web-text-input.md) with `--window-size=900,700`; Firefox through Playwright there.
The suite: the alpha log; 0 GPUI frames in 2 s while the canvas element draws; a real click (or tap) into each
element at view-relative (280, 190); the div over each takes its press; a wheel scrolls the same-origin iframe and a
wheel over the div is GPUI's; a drag selects text in the cross-origin iframe; the canvas's `pointer-events` over and
off a view; no green (the page's gap colour) in a still screenshot and 8 while every view moves and resizes. Chromium
also measures, with and without views, the pointer-move handling time and the main-thread time per animating frame.

Checked 2026-11-28, every check passing (16 desktop, 12 mobile, where wheel, drag and hover don't apply):

| Browser | Renderer | Scales | Pointer move, views / none | Animating frame, views / none |
|---|---|---|---|---|
| Chromium 158 headless, desktop | WebGL2 (SwiftShader) | 1, 1.25 | 0.24 / 0.20 ms | 60 / 60 fps, 14.6 / 13.6 ms |
| Chromium 158, phone profile (touch, 2.625, CPU 4x slower) | WebGL2 (SwiftShader) | 2.625 | 0.98 / 0.77 ms | 11 / 11.7 fps, 95.6 / 90.1 ms |
| Firefox (Playwright), desktop and touch | WebGL2 | 1, 1.25 | — | — |

The toggle costs about 0.03 ms per move on a desktop and 0.2 ms on the throttled phone profile. A WebGL2 canvas is
premultiplied with or without views, so the views' cost per frame (about 1 ms, 6 % on the phone profile) is the
DOM slots and their compositing, not transparency. WebGPU (alpha `PreMultiplied` while views show): every check
passes in headless Chromium with `--use-angle=gl`; with Vulkan on NVIDIA, Chromium shows WebGPU canvases black and
GPUI falls back to WebGL2 ([web-webgpu.md](web-webgpu.md)). Also owed: Safari, and an iframe's
cross-origin process lagging its slot by a frame after a resize (one device pixel, filled by `set_background`).

## Tests

`native_view_layout::tests::placement_of_a_view` and `pointer_target_steps` run in
[patch-test.sh](../scripts/patch-test.sh)'s `web-pure` crate ([patch-tests.md](patch-tests.md));
`native_view_occluders_are_the_hitboxes_painted_over_it` with GPUI's tests.
