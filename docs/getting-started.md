# Getting started

One GPUI app, built and run on every target by one tool, `cargo parallax`. This page sets up what every target
shares; each target's page adds its own SDK and the one command that runs [hello](examples.md#hello) there.

| Target | Page | Builds on |
|---|---|---|
| Android | [getting-started-android.md](getting-started-android.md) | Linux, macOS (Windows: not yet, `HOSTS` in [android.rs](../crates/cli/src/android.rs)) |
| iOS | [getting-started-ios.md](getting-started-ios.md) | macOS with Xcode |
| Web | [getting-started-web.md](getting-started-web.md) | any |
| macOS | [getting-started-macos.md](getting-started-macos.md) | macOS |
| Windows | [getting-started-windows.md](getting-started-windows.md) | Windows |
| Linux | [getting-started-linux.md](getting-started-linux.md) | Linux |

## 1. Rust

Use the toolchain in [rust-toolchain.toml](../rust-toolchain.toml) (upstream GPUI's; `rustup` picks it up inside
this repo, and lists the Android, iOS and wasm targets). For your own app, copy that file next to its `Cargo.toml`.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # once; then rustup reads rust-toolchain.toml
```

## 2. cargo-parallax

```sh
cargo install cargo-parallax          # the published CLI
cargo install --path crates/cli       # or this checkout's, from the repo root
```

Inside this repo, build the patched GPUI fork once first (the workspace builds on it): `scripts/apply-patches.sh`
([upstream.md](upstream.md)). An app outside the repo gets the fork from crates.io (`parallax-gpui*`) and skips this.

## 3. The app crate

Every target runs the same `launch` and `open_window`; only the entry point differs
([examples.md](examples.md#hello), [lib.rs](../examples/hello/src/lib.rs), [main.rs](../examples/hello/src/main.rs)):

| Piece | What it is for |
|---|---|
| `parallax = "0.1"` in `[dependencies]` | GPUI (`parallax::gpui`) plus the Parallax crates and entry points |
| `[lib] crate-type = ["cdylib", "rlib"]` | Android loads the `cdylib` `.so`, the web the `.wasm`; `rlib` for the bin |
| `src/main.rs`: `parallax::run(\|cx\| { launch(cx); open_window(cx) })` | desktop and iOS |
| `parallax::android_main!(launch, open_window);` in lib.rs | Android: `open_window` once per Activity |
| `#[wasm_bindgen(start)] fn start()` calling `parallax::run` | the web; needs `wasm-bindgen` for `cfg(target_family = "wasm")` |

Every entry point applies the platform's feel first ([platform-feel.md](platform-feel.md)).

## 4. parallax.toml

Next to `Cargo.toml`; `cargo parallax` reads it from the current directory. Unknown keys are errors, so a typo
never goes unnoticed. hello's: [parallax.toml](../examples/hello/parallax.toml).

```toml
[app]
name = "Hello"                               # launcher / home-screen / window name
id = "io.github.xuckless.parallax.hello"     # Android application id and Apple bundle id: letters, digits, dots
icon = "icon.svg"                            # optional SVG: Android, iOS, macOS and web icons
```

Per-target tables (`[android]`, `[ios]`, `[web]`, `[macos]`) are optional; each target page lists its keys, and
[cli.md](cli.md) holds the full reference.

## 5. Run it

```sh
cd examples/hello
cargo parallax run              # this machine's desktop
cargo parallax run android      # or ios, web, macos, linux, windows
```

`run` builds, installs and starts the app with its log on the terminal; `build` stops at the package. `--release`
builds optimised. Every failure names its fix (one row each in [error.rs](../crates/cli/src/error.rs)); the target
pages list the common ones.

## Next

How touch reaches your app: [touch-and-mouse.md](touch-and-mouse.md). Native views (a browser page, video, map under
GPUI): [native-views-guide.md](native-views-guide.md). Who Parallax ports from: [credits.md](credits.md).
