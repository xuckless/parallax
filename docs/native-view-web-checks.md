# Native views on the web: checks

Part of [native-view-web.md](native-view-web.md). [web-checks.mjs](../scripts/web-checks.mjs)' `native-view` suite
drives [examples/native-view](../examples/native-view/) on the web in a headless browser.

## Running it

Build and serve the example on two ports (the second origin for the cross-origin iframe):

```sh
wasm-pack build examples/native-view --target web --no-typescript --no-pack --dev
python3 -m http.server 8770 --bind 127.0.0.1 --directory examples/native-view &
python3 -m http.server 8771 --bind 127.0.0.1 --directory examples/native-view &
node scripts/web-checks.mjs http://127.0.0.1:8770/ desktop|mobile <chromium port>|firefox native-view [scale]
```

Chromium as in [web-text-input.md](web-text-input.md) with `--window-size=900,700`; Firefox through Playwright there.

## What it checks

- **Content, in every screenshot** (still, `?offset`, eight while every view moves and resizes), read back with
  [pixels.py](../scripts/pixels.py): no green (the page's gap colour: a hole with nothing under it); no magenta (GPUI's
  box under each view: a hole not cut); the four blue divs within 10 % of their area; GPUI's `#202020` over half of
  the canvas outside the views; each view's own colour at points clear of its div and text (the iframes' frame.html
  gradient, not the slot's white; the canvas's `hsl(h 70% 60%)`; the form's orange). A blanked page (iframes
  `about:blank`, the canvas never drawn, the form uncoloured) and an example that paints no view both fail these.
- **Input:** a real click (or tap) into each element at view-relative (280, 190), the div over each takes its press,
  a wheel scrolls the same-origin iframe and a wheel over the div is GPUI's, a drag selects text in the cross-origin
  iframe, the canvas's `pointer-events` over and off a view; a shield moved over a view by hand gives its press to
  the element under it and goes back.
- **Focus:** an `<input>` inside the form keeps focus (and its typing) when GPUI focuses the view; GPUI's field gets
  Enter after focus was in an iframe, and after that focused iframe's view is dropped; with every view dropped the
  canvas has its own styles back and the layer is gone.
- **Page around the canvas** (`?offset`: body margin 8 px and a 40 px header above the canvas): the views' clicks,
  divs and colours at their offset places; the header takes a real press and scrolls by a real wheel while views
  show.
- **Costs:** 0 GPUI frames in 2 s while the canvas element draws; the alpha log; on Chromium, the pointer-move
  handling time and the main-thread time per animating frame, with and without views.

## Results

Checked 2026-11-28, every check passing (34 desktop, 29 mobile, where wheel, drag and hover don't apply):

| Browser | Renderer | Scales | Pointer move, views / none | Animating frame, views / none |
|---|---|---|---|---|
| Chromium 158 headless, desktop | WebGL2 (SwiftShader) | 1, 1.25 | 0.13 / 0.06 ms | 60 / 60 fps, 12.3 / 11.5 ms |
| Chromium 158, phone profile (touch, 2.625, CPU 4x slower) | WebGL2 (SwiftShader) | 2.625 | 0.35 / 0.13 ms | 12.3 / 12.3 fps, 83.9 / 84.3 ms |
| Chromium 158 headless `--use-angle=gl`, desktop | WebGPU | 1 | 0.16 / 0.13 ms | 60 / 60 fps, 2.7 / 2.0 ms |
| Firefox (Playwright), desktop and touch | WebGL2 | 1, 1.25 | — | — |

A WebGL2 canvas is premultiplied with or without views, so the views' cost per frame is the DOM slots and their
compositing, not transparency. Against the series before these checks were added, the stale-shield, dropped-view,
last-view, form-input, offset-canvas and page-content rows fail.

WebGPU (alpha `PreMultiplied` while views show) passes in headless Chromium with `--use-angle=gl`; with Vulkan on
NVIDIA Chromium shows WebGPU canvases black and GPUI falls back to WebGL2 ([web-webgpu.md](web-webgpu.md)). Owed:
Safari; an iframe's cross-origin process lagging its slot by a frame after a resize (filled by `set_background`); a
browser check for the wheel tick after GPUI's content scrolls a view under a still pointer (table rows only: the
example has no scrolling GPUI content).
