# Getting started: macOS

Shared setup first: [getting-started.md](getting-started.md). macOS 12 and up (GPUI's minimum). Builds on a Mac
only; desktop targets are never cross-built. Reference: [cli-desktop.md](cli-desktop.md#macos-a-real-app).

## Prerequisites

| Need | Command |
|---|---|
| Xcode command line tools (`xcrun metal` for GPUI's shaders, `lipo`, `codesign`) | `xcode-select --install` |
| For a universal binary only | `rustup target add x86_64-apple-darwin` (and `aarch64-apple-darwin` on an Intel Mac) |

## Run hello

```sh
cd examples/hello
cargo parallax run macos        # or just: cargo parallax run
```

It builds a real `<app.name>.app` in `target/parallax/macos/<package>/` (Info.plist, the icon from `app.icon`, an
ad-hoc signature) and starts its binary, so the log streams into the terminal and Ctrl-C stops it.
`cargo parallax build macos [--release]` stops at the bundle; double-click it or `open` it like any app.
`cargo run -p hello` still works for a quick look, without the bundle.

## Keys in parallax.toml

```toml
[macos]
touch_native = false          # see below
archs = ["arm64"]             # ["arm64", "x86_64"]: a universal binary
codesign = true               # ad-hoc signature, so Gatekeeper opens it on this Mac
```

The ad-hoc signature is not a Developer ID one: a bundle copied to another Mac is still quarantined.

## Touch on a Mac: Sidecar and touchscreens

macOS 27 sends direct touches from a touchscreen Mac or an iPad used as a Sidecar display
([desktop-touch-macos.md](desktop-touch-macos.md)). `macos.touch_native` picks who turns them into clicks:

| `touch_native` | What the app gets |
|---|---|
| `false` (default) | AppKit's emulation: tap → click, pan → scroll, pinch → magnify; GPUI also sees the touches |
| `true` (`NSIsTouchNative` in Info.plist) | touches only, as `TouchEvent`s; GPUI's own touch → mouse makes the clicks ([touch-and-mouse.md](touch-and-mouse.md)), with Flutter's feel |

To try it with an iPad: Control Center → Display → pick the iPad (Sidecar), drag hello's window onto it, and tap the
box; the log says `touch click` per tap. Why the two modes exist: [desktop-touch-macos-emulation.md](desktop-touch-macos-emulation.md).

## Common errors

| Error says | Fix |
|---|---|
| `builds macos apps only on macos` | build on the Mac; elsewhere use that machine's target |
| `has no bin target to put in the .app` | add `src/main.rs` calling `parallax::run` |
| `has several bin targets` | a .app holds one: move the others to `examples/` |
| `unknown macOS arch` / `macos.archs is empty` | `arm64` and/or `x86_64` |
| `xcrun: error: unable to find utility "metal"` (during cargo build) | `xcode-select --install`, or point `xcode-select -s` at Xcode |
| `cargo build failed` with `can't find crate for core` | `rustup target add x86_64-apple-darwin` for the second arch |
| No text drawn | the app must build `gpui_platform` with `font-kit` (the `parallax` crate does) |
