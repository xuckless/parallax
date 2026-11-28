# Native views on Wayland: what was measured

Part of [native-view-wayland.md](native-view-wayland.md): the setups patch 0049 ran on, the alpha mode each offered,
and what a native view costs. The repeatable checks are [native-view-wayland-checks.md](native-view-wayland-checks.md).

## Checked (2026-11-27)

| Setup | Alpha mode | GPUI frames/s, page at 60 | Input | Seams |
|---|---|---|---|---|
| owner's GNOME 50 / mutter, RTX 5060, NVIDIA 620.73.04 Vulkan, 1.0, 300 Hz | PreMultiplied | 0 | — | — |
| headless mutter, NVIDIA Vulkan client, 1.0 and 1.25 | PreMultiplied | 0 | view-relative, div takes its own | none |
| headless mutter, Mesa lavapipe client, 1.0 and 1.25 | PreMultiplied | 0 | view-relative | none |
| headless mutter on Mesa llvmpipe (`--mesa-compositor`), lavapipe client, 1.0 and 1.25 | PreMultiplied | 0 | view-relative, div takes its own | none |
| both headless setups, `--clipped` (2026-11-28) | PreMultiplied | — | view-relative to the whole view | nothing past the clip |

NVIDIA's WSI offers premultiplied here, so no GL fallback is needed on this driver. With `--animate` (the view moves
and resizes every GPUI frame) `WAYLAND_DEBUG` shows the order above every frame, and a screenshot at 1.25 shows one
physical pixel of the container, then the page, on every edge. The headless checks are one repeatable script,
[wayland-native-view-checks.sh](../scripts/wayland-native-view-checks.sh), also run by CI
([native-view-wayland-checks.md](native-view-wayland-checks.md)). Mutter compositing on Mesa is the script's
`--mesa-compositor` mode (and what CI's GPU-less runner does anyway). Owed: the owner's look at 125 % on the real
desktop, one command that restores every setting after ([CHECK.md](../examples/native-view/CHECK.md),
[owner-fractional-scale-check.py](../scripts/owner-fractional-scale-check.py)).

Cost on the owner's session (debug build, 10 s each, window 900×600 with a 640×400 view):

| Case | GPUI frames/s | gnome-shell CPU | GPU utilization |
|---|---|---|---|
| `--no-view` (idle window) | 0 | 1.9 % | 1.1 % |
| page at 60 fps | 0 | 5.4 % | 1.7 % |
| page at 120 fps | 0 | 8.1 % | 2.5 % |
