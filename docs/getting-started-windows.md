# Getting started: Windows

Shared setup first: [getting-started.md](getting-started.md). Windows 10 and 11, rendered by GPUI's DirectX 11
backend. Builds on Windows only; desktop targets are never cross-built. Reference: [cli-desktop.md](cli-desktop.md).

## Prerequisites

| Need | Command |
|---|---|
| Rust's MSVC toolchain | rustup-init.exe from [rustup.rs](https://rustup.rs) (its default host, `x86_64-pc-windows-msvc`); rust-toolchain.toml picks the version |
| Visual Studio Build Tools: "Desktop development with C++" (the MSVC linker and the Windows SDK) | `winget install Microsoft.VisualStudio.2022.BuildTools --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"` |
| Git Bash, for this repo's scripts (`scripts/apply-patches.sh`) | `winget install Git.Git` |

## Run hello

```sh
cd examples/hello
cargo parallax run windows      # or just: cargo parallax run
```

On Windows this is `cargo run` for the host target (no installer, no MSIX): the log goes to the terminal through
env_logger (`RUST_LOG=debug` for more). `cargo parallax build windows [--release]` stops at the `.exe` in
`target/<profile>/`. There are no `[windows]` keys in parallax.toml.

## Touch and pen

Touchscreens and pens arrive as `WM_POINTER` input, turned into `TouchEvent`s with the device named (touch or pen);
GPUI's touch → mouse makes taps click ([touch-and-mouse.md](touch-and-mouse.md)). The Windows row of the behaviour
table (clamping scroll, no overscroll, 48 px touch targets) applies: [platform-feel.md](platform-feel.md). How it
works: [desktop-touch-windows.md](desktop-touch-windows.md).

## Native views

[examples/native-view](../examples/native-view/README.md) shows a composition swap chain, or with
`--view webview2` a WebView2 page (the runtime ships with Windows 10 and 11):

```sh
cargo run -p native-view -- --view webview2   # cargo parallax passes no app arguments yet
```

## Common errors

| Error says | Fix |
|---|---|
| `builds windows apps only on windows` | build on the Windows machine |
| `link.exe not found` / `linker 'link.exe' not found` | install the Build Tools workload above, then open a new terminal |
| `error: could not compile` in a `-sys` crate looking for `windows.h` | the Windows SDK is missing: the same workload |
| `scripts/apply-patches.sh: not found` | run it from Git Bash |
