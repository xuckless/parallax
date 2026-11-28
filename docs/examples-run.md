# Running hello

How to build and run [hello](examples.md#hello) on each target, from the repo root after
`scripts/apply-patches.sh`.

## Desktop (Linux, macOS, Windows)

```sh
cargo run -p hello
```

or, as one entry point for every target, `cd examples/hello && cargo parallax run` (the host's desktop: a signed
`Hello.app` on macOS, `cargo run` on Linux and Windows; [cli-desktop.md](cli-desktop.md)).

Linux needs the `LINUX_PACKAGES` of [ci.yml](../.github/workflows/ci.yml) (on Fedora: `libxkbcommon-x11-devel`,
`wayland-devel`, `fontconfig-devel`, `freetype-devel`). Desktop logs to stderr through [env_logger](https://docs.rs/env_logger):
warnings, plus `info` from hello; `RUST_LOG` overrides it (e.g. `RUST_LOG=debug`).

## Web

Single-threaded, stable Rust ([platform.md](platform.md)). Needs
[wasm-pack](https://github.com/drager/wasm-pack); it fetches the matching `wasm-bindgen` on first use.

```sh
wasm-pack build examples/hello --target web --no-typescript --no-pack --dev
python3 -m http.server 8765 --directory examples/hello
```

Open <http://127.0.0.1:8765/>. `pkg/` is git-ignored. A browser gives GPUI no system fonts, so `start` loads
`WEB_FONTS` ([fonts/](../examples/hello/fonts/), OFL) first: Noto Sans and Noto Sans Arabic. Emoji come from the
browser (gpui_web's Canvas fallback); CJK is not embedded (too large), so `SCRIPTS`' CJK shows missing glyphs. gpui_web tries WebGPU, then falls back to WebGL2; the console
shows which (`Browser graphics initialized successfully with …`) and `hello: box colour …` on each click or tap.
Drop `--dev` for an optimised build (wasm-pack then also fetches `wasm-opt`).

## Android

[cargo-parallax](cli.md) reads [parallax.toml](../examples/hello/parallax.toml) and builds both default ABIs.

```sh
cargo install --path crates/cli
cd examples/hello && cargo parallax run android --device <serial>
adb -s <serial> logcat -s parallax
```

Logcat shows the renderer, the surface and `hello: window open`; [android-smoke.md](android-smoke.md) checks it.

## iOS

CI checks it: `cargo check --workspace --target aarch64-apple-ios` ([ci.md](ci.md)). Until `cargo parallax run ios`,
[record-traces.sh](traces.md#recording) runs main.rs's bin on a simulator as a bare `.app` (`ios_setup`).
