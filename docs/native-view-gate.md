# NativeView gate (0.1.10)

The gate ([ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview), issue #1): per target, an example view
that changes every frame with a div over part of it; moving and resizing keeps hole and view in step; pointer and
scroll over the view arrive view-relative and the div keeps its own; 0 GPUI frames/s while only the view updates; the
alpha mode logged; no seams at 1.0 and 1.25. Every target runs [examples/native-view](../examples/native-view/)
([examples-native-view.md](examples-native-view.md)); every check greps its `native-view: gpui frames/s N, view
frames/s M` line. Measured 2026-11-28 on the patches 0047–0072.

| Target | Proof | 0 GPUI frames/s (view) | In step, no gap | Input | Alpha logged | Scales, seams |
|---|---|---|---|---|---|---|
| Wayland, NVIDIA (owner's GNOME 50, RTX 5060) | [results](native-view-wayland-results.md), headless mutter on the NVIDIA client | 0 (page 60 and 120) | 8 of 8 moving shots | view-relative, div its own | `PreMultiplied` | 1, 1.25: none |
| Wayland, Mesa | [wayland-native-view-checks.sh](../scripts/wayland-native-view-checks.sh) both modes (34/34 host GL, 36/36 `--mesa-compositor`); CI `linux-touch` | 0 (60) | 8 of 8; clipped, crop, resize rows | view-relative, div its own | `PreMultiplied` | 1, 1.25: one physical pixel of GPUI past each edge |
| Windows (CI WARP) | [windows-native-view-checks.sh](../scripts/windows-native-view-checks.sh), CI `windows-touch` (run 46139825583) | 0, 0 copies (58–64) | 0 gap shots of 100 sliding, at 100 and 125 %; one surface copy per GPUI frame (595, 616) | move, wheel, click at 200 150 in the view; cover its own | `Premultiplied` | 100, 125 %: no seams |
| Windows, WebView2 (`--view webview2`) | the same script | 0 (64) | still and after input: no gap | move, click, wheel, key reach the page | as above | 100, 125 % |
| macOS | upstream's views, patches 0059–0061; `cargo clippy` on pixl-mac | owed (no window server over ssh) | owed | AppKit's own (`NativeViewInput::View`) | `metal layer alpha` | owed |
| iOS (simulator, iPhone 17 3x, iPad Air 2x) | [apple-native-view-checks.sh](../scripts/apple-native-view-checks.sh), 5 iPhone runs + 1 iPad | 0 (63) | 0 failing frames still, moving, rotating (0 gap pixels in all six) | `view pointer down at (44.0, 190.0)`, scroll, div its own | `premultiplied` | 2x, 3x, rotation: none |
| Android API 35 emulator | [android-native-view-checks.sh](../scripts/android-native-view-checks.sh) 5 runs | 0 (54–60) | `control` (SurfaceControl): 0 of 12 every run, strip ≤ 7 px; `view`/`hosted`: 0 of 12 past the ring | forwarded touches; Back reaches GPUI | `Inherit` | stacks in paint order |
| Android API 28 emulator | the same script | 0 (43) | skipped: GL holes black on the emulator (real devices use Vulkan) | Back | `only Opaque` warning | — |
| Web Chromium 158 (WebGL2) | [web-checks.mjs](../scripts/web-checks.mjs) `native-view` | 0 (60–61) | content rows | trusted clicks and wheel on the view; GPUI's divs keep theirs | `premultiplied` | 1, 1.25: 34/34; phone 2.625: 29/29 |
| Web Firefox (Playwright) | the same suite | 0 | content rows | as above | as above | 1, 1.25: 34/34; mobile 29/29 |
| Web WebGPU (`--use-angle=gl`) | the same suite | 0 | content rows | as above | `PreMultiplied` | 1: 34/34 |

Costs of a shown view on the web: pointer move 0.11 ms with views against 0.07 ms without (desktop), an animating frame
60 fps either way ([native-view-web-checks.md](native-view-web-checks.md)). On Chromium with Vulkan on NVIDIA,
WebGPU canvases show black; the probe falls back to WebGL2 ([web-webgpu.md](web-webgpu.md)).

Known by design: an Android `View` and `in_surface` View move a frame behind their hole on a move or grow (counted,
not failed; the `SurfaceControl` path is in step, [hosting](native-view-android-hosting.md)). X11 is deferred.

## Owed by the owner

- macOS: record the example on screen (`screencapture -v`) while moving and resizing the window, then run
  [apple-native-view-frames.py](../scripts/apple-native-view-frames.py) on it ([checks](native-view-apple-checks.md)).
- iPad: the example on the owner's iPad (the simulator passes).
- Wayland at 125 % on the real desktop ([owner-fractional-scale-check.py](../scripts/owner-fractional-scale-check.py)).
- WebGPU: a short headed look in Chromium with Vulkan on NVIDIA (the fallback should log and draw WebGL2).
- WebView2 on a real Windows machine: its popups and a lost GPU device (`on_composition_device_changed`).

## Verdict

**PASS** on every target the machines can run (Wayland NVIDIA + Mesa, Windows + WebView2, iOS, Android, web
Chromium + Firefox + WebGPU): every acceptance row above holds. The gate is ticked only after the owed checks,
macOS's on-screen recording first: until then macOS rests on upstream's views and the shared Metal code iOS proves.
