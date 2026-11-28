# Getting started: Linux

Shared setup first: [getting-started.md](getting-started.md). Wayland and X11 are both built in; the app picks
Wayland when `WAYLAND_DISPLAY` is set, X11 otherwise. Rendered by GPUI's wgpu backend (Vulkan). Builds on Linux only;
desktop targets are never cross-built. Reference: [cli-desktop.md](cli-desktop.md).

## Prerequisites

The system libraries GPUI links (CI's `LINUX_PACKAGES` in [ci.yml](../.github/workflows/ci.yml)), plus a C compiler
and a Vulkan driver:

| Distro | Command |
|---|---|
| Debian, Ubuntu | `sudo apt install build-essential libfontconfig-dev libfreetype-dev libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev mesa-vulkan-drivers` |
| Fedora | `sudo dnf install gcc fontconfig-devel freetype-devel wayland-devel libxkbcommon-x11-devel libxcb-devel mesa-vulkan-drivers` |
| Arch | `sudo pacman -S base-devel fontconfig freetype2 wayland libxkbcommon-x11 libxcb vulkan-icd-loader` (plus your GPU's Vulkan driver) |

## Run hello

```sh
cd examples/hello
cargo parallax run linux        # or just: cargo parallax run
```

On Linux this is `cargo run` for the host target (no Flatpak, no package): the log goes to the terminal through
env_logger (`RUST_LOG=debug` for more) and shows hello's `window open, 1 in all`. `cargo parallax build linux
[--release]` stops at the binary in `target/<profile>/`. There are no `[linux]` keys in parallax.toml.

Force X11 on a Wayland session with `WAYLAND_DISPLAY= cargo parallax run linux` (XWayland).

## Touch

Touchscreens arrive as Wayland `wl_touch` or X11 XInput 2 touch events (ported from winit), turned into `TouchEvent`s;
GPUI's touch → mouse makes taps click ([touch-and-mouse.md](touch-and-mouse.md)). The Linux row of the behaviour
table applies ([platform-feel.md](platform-feel.md)). How it works: [desktop-touch-linux.md](desktop-touch-linux.md).

## Native views

On Wayland, [examples/native-view](../examples/native-view/README.md) shows the app's own `wl_surface` under GPUI's
window (`cargo parallax run linux` in `examples/native-view`). X11 has no native views yet.

## Common errors

| Error says | Fix |
|---|---|
| `builds linux apps only on linux` | build on the Linux machine |
| `The system library 'xkbcommon-x11' required by crate …` / `wayland-client` / `fontconfig` | install the packages above (pkg-config finds the `-dev` / `-devel` ones) |
| `linker 'cc' not found` | `build-essential` / `gcc` |
| The window opens but stays empty, or `no adapter` / `Vulkan` errors in the log | install your GPU's Vulkan driver (`vulkaninfo` should list it) |
| Nothing opens over ssh or in a container | it needs a Wayland or X11 display: `WAYLAND_DISPLAY` or `DISPLAY` set |
