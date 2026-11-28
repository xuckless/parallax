# WebGPU on the web: the canvas probe

gpui_web picks WebGPU first and WebGL2 when WebGPU fails (`WebBackendPreference::Auto`). A browser can give a working
WebGPU device whose canvases show nothing, so patch [0072](../patches/0072-web-webgpu-canvas-probe.patch) adds one
check before a window commits to WebGPU: `webgpu_canvas_probe::shows_drawing` (its own file, one hook in
platform.rs) clears a detached 4x4 WebGPU canvas to red and reads it back through a 2D `OffscreenCanvas` in the same
task. Anything but opaque red tries WebGL2 and logs

```
WebGPU initialization failed; falling back to WebGL2: WebGPU draws nothing to a canvas here (a test clear read back blank)
```

The WebGPU device is kept until WebGL2 works. A WebGL2 try that fails is made once more after the next
`requestAnimationFrame` (a context refused while the page starts up). If WebGL2 fails twice (its `webglcontextcreationerror` status messages
are in the log), the window still opens on WebGPU, logged as an error (`WebGL2 initialization failed too (..);
staying on WebGPU, whose canvas test read back blank: this window may show nothing`): a window that may show nothing
beats no window. An app that asks for `WebBackendPreference::WebGpu` explicitly is not probed. The probe costs one 4x4
clear at startup.

## What the probe catches (2026-11-28)

Chromium 158 (Fedora 44's build), NVIDIA's proprietary driver, RTX 5060, GNOME Wayland. A ten-line page that clears a
WebGPU canvas red every frame ([CDP](https://chromedevtools.github.io/devtools-protocol/) screenshots, and drawImage
of the canvas read back):

| Chromium flags (all with `--enable-unsafe-webgpu`) | Adapter | Canvas | Read back |
|---|---|---|---|
| `--ozone-platform=wayland --enable-features=Vulkan,WebGPU,VulkanFromANGLE --use-angle=vulkan` (headed) | NVIDIA | black (the owner saw an all-black window) | — |
| `--headless=new --enable-features=Vulkan --use-angle=vulkan` | NVIDIA | black | 0,0,0,0 |
| `--headless=new` (no other flag) | SwiftShader | black, device destroyed | 0,0,0,0 |
| `--headless=new --use-angle=gl` | SwiftShader | red | 255,0,0,255 |

The bug is Chromium's or the driver's, not GPUI's: the plain page shows it too, and GPUI's own WebGPU path draws hello
and native-view-web correctly where Chromium presents (`--use-angle=gl`). With the probe, the black configurations
fall back to WebGL2 and draw. In headless `--enable-features=Vulkan` Chromium's own compositor also fails ("Failed
to initialize vulkan surface"), so screenshots there are not usable.

## Checks

- [web-checks.mjs](../scripts/web-checks.mjs)' `native-view` suite under WebGPU (headless, `--use-angle=gl`, alpha
  mode logged `PreMultiplied`): 34/34 ([native-view-web-checks.md](native-view-web-checks.md)). Its content checks
  (GPUI's background, the divs, each view's colour) fail on a canvas that presents nothing.
- Headless `--enable-features=Vulkan --use-angle=vulkan` on NVIDIA: the probe logs the fallback and native-view-web
  opens on WebGL2; with the page's WebGL2 contexts made to fail (an init script returning `null` from
  `getContext("webgl2")` after firing `webglcontextcreationerror`), it logs the retry and both failures, the event's status
  message among them, and the window opens on WebGPU.
- Owed to the owner: a short look at native-view-web in a headed Chromium with the flags in the table's first row,
  which should now log the fallback and draw with WebGL2.
