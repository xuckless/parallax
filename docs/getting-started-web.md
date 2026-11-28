# Getting started: the web

Shared setup first: [getting-started.md](getting-started.md). Builds on any host; runs in browsers with WebGPU or
WebGL2 (gpui_web tries WebGPU, then falls back: [web-webgpu.md](web-webgpu.md)). Single-threaded, stable Rust
([platform.md](platform.md)). Reference: [cli-web.md](cli-web.md).

## Prerequisites

| Need | Command |
|---|---|
| The wasm target | `rustup target add wasm32-unknown-unknown` (listed in this repo's toolchain) |
| `wasm-bindgen` CLI, the exact version of the `wasm-bindgen` crate in `Cargo.lock` | `cargo install wasm-bindgen-cli --version 0.2.130` (the version cargo parallax asks for; it installs nothing itself) |
| Optional, for `--release`: binaryen's `wasm-opt` | your package manager (`binaryen`); without it the build says so and stays unoptimised |

The app crate needs `wasm-bindgen` under `[target.'cfg(target_family = "wasm")'.dependencies]` and a
`#[wasm_bindgen(start)]` function that calls `parallax::run` (hello's `start` in [lib.rs](../examples/hello/src/lib.rs)).
A browser gives GPUI no system fonts: embed the ones you draw with (hello's `WEB_FONTS`).

## Run hello

```sh
cd examples/hello
cargo parallax run web                          # serves http://127.0.0.1:8080/
cargo parallax run web --open                   # and opens the system browser
cargo parallax run web --host 0.0.0.0 --port 9000   # reachable from a phone on the same network
```

The server prints each request; the app logs to the browser's console. Ctrl-C stops it. It is a development server
(no TLS, no compression), with `Cache-Control: no-cache`, so a rebuild shows on reload.

## Deploy

```sh
cargo parallax build web --release
```

writes the whole site to `target/parallax/web/<package>/` (`index.html`, `manifest.json`, `favicon.svg`, `pkg/`).
Upload that directory to any static host; it must serve `.wasm` as `application/wasm` (most do).

## Keys in parallax.toml

`[web]`: `title` (`app.name`), `background` (`#ffffff`, the page until GPUI's first frame), `index` (your own host
page in place of the template), `assets` (files copied next to it). `app.icon` becomes the favicon and manifest
icon. Table: [cli-web.md](cli-web.md#parallaxtoml-web); [examples/native-view](../examples/native-view/parallax.toml)
uses `index` and `assets`.

## Common errors

| Error says | Fix |
|---|---|
| `wasm-bindgen is not on PATH` / `wasm-bindgen on PATH is X, not Y` | `cargo install wasm-bindgen-cli --version Y` (the version it names) |
| `several wasm-bindgen versions in Cargo.lock` | `cargo update -p wasm-bindgen`, so one CLI fits |
| `does not depend on wasm-bindgen` | add it for `cfg(target_family = "wasm")` and a `#[wasm_bindgen(start)]` function |
| `package … has no cdylib target` | `[lib] crate-type = ["cdylib", "rlib"]` |
| `cannot serve on 127.0.0.1:8080` | the port is taken: `--port <n>` |
| `web.background … is not a CSS hex colour` | `#rgb` or `#rrggbb` |
| `web.assets file … has the name of a file cargo parallax writes` | rename it; for your own page use `web.index` |
| A blank window, console says `this window may show nothing` | WebGPU draws nothing there and WebGL2 failed too: [web-webgpu.md](web-webgpu.md) lists the browser setups |
