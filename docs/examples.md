# Examples

[`examples/`](../examples/) holds one app built for every target ([AGENTS.md](../AGENTS.md), Repo layout). Each
example is a workspace member, so [CI](ci.md) checks it on all six targets.

## hello

[examples/hello](../examples/hello/): one window with two text fields (prose, and an email field with an autofill
hint), a line in several scripts (`SCRIPTS`: Latin,
CJK, Arabic, emoji) and a box. Each click or tap moves the box to the next colour of `COLOURS` in
[lib.rs](../examples/hello/src/lib.rs), and the log line says whether the click came from a mouse, a tap or a key; a
tap outside a field blurs it. It is the app of Milestone 0.1.1 in
[ROADMAP.md](../ROADMAP.md): it opens, draws and reacts to a tap on desktop, web and Android.

Two Android checks ride along: 直骨 in `SCRIPTS` change glyph form with the app's language
([android-fonts.md](android-fonts.md#locale-changes)), and "Show emoji" swaps the box for 100 distinct emoji
(`EMOJI_GRID`) and logs `hello: emoji grid: frame …`, the cost of drawing them
([android-emoji.md](android-emoji.md#cost)). "Record touches" swaps the window for a trace pad that logs every touch
in the trace format ([traces.md](traces.md)).

The field ([text_field.rs](../examples/hello/src/text_field.rs), configured per instance) is example-local: upstream GPUI has no text field
outside Zed's editor (GPL, never copied), so it is upstream's `examples/input.rs` (Apache-2.0) cut down to one
`EntityInputHandler` with UTF-16 ranges, composition and grapheme deletes. On Android it drives the IME
([android-text-input.md](android-text-input.md)).

What it shows: one shared `launch(cx)` and a thin entry point per target, all through the `parallax` facade.

| Target | Entry point | How it starts |
|---|---|---|
| Desktop | [main.rs](../examples/hello/src/main.rs) | `parallax::run(launch)`: the platform ([platform.md](platform.md)) with its behaviour row ([behaviour.md](behaviour.md)) |
| Android | `parallax::android_main!(launch)` in lib.rs | `ParallaxActivity` loads the `cdylib` ([android.md](android.md)) |
| Web | `start` (`#[wasm_bindgen(start)]`) in lib.rs: `parallax::run` | [index.html](../examples/hello/index.html) imports the wasm-pack module; `favicon.svg` links to the brand icon in [assets/brand/](../assets/brand/) |
| iOS | none yet | |

Run every command from the repo root after `scripts/apply-patches.sh`.

### Desktop (Linux, macOS, Windows)

```sh
cargo run -p hello
```

Linux needs the `LINUX_PACKAGES` of [ci.yml](../.github/workflows/ci.yml) (on Fedora: `libxkbcommon-x11-devel`,
`wayland-devel`, `fontconfig-devel`, `freetype-devel`). Desktop logs to stderr through
[env_logger](https://docs.rs/env_logger): warnings, plus `info` from hello; `RUST_LOG` overrides it
(e.g. `RUST_LOG=debug`).

### Web

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

### Android

[cargo-parallax](cli.md) reads [parallax.toml](../examples/hello/parallax.toml) and builds both default ABIs.

```sh
cargo install --path crates/cli
cd examples/hello && cargo parallax run android --device <serial>
adb -s <serial> logcat -s parallax
```

Logcat shows the renderer, the surface and `hello: window open`. Taps reach GPUI once the touch port lands
([android.md](android.md), touch seam); until then the box draws but does not change colour.

### iOS

CI checks it: `cargo check --workspace --target aarch64-apple-ios` ([ci.md](ci.md)). Until `cargo parallax run ios`,
[record-traces.sh](traces.md#recording) runs main.rs's bin on a simulator as a bare `.app` (`ios_setup`).
