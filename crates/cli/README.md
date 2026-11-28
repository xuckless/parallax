# cargo-parallax

`cargo parallax` builds and runs a [Parallax](https://github.com/xuckless/parallax) app crate on every target from the
`parallax.toml` next to its `Cargo.toml`: a generated Gradle project on Android (release-signed App Bundles for Google
Play), a generated Xcode project on iOS (simulator or device), a wasm-bindgen site and local server on the web, a real
`.app` on macOS, and `cargo run` on Linux and Windows.

```sh
cargo install cargo-parallax
cargo parallax run android [--release] [--device <serial>]
cargo parallax build android [--release] [--apk|--aab]
cargo parallax run ios [--release] [--simulator <name|udid> | --device [<name|udid>]]
cargo parallax build ios [--release]
cargo parallax run web [--release] [--port <n>] [--host <address>] [--open]
cargo parallax build web [--release]
cargo parallax run|build macos|linux|windows [--release]
cargo parallax run [--release]   # this machine's desktop
```

Commands, keys, requirements and each step: [docs/cli.md](https://github.com/xuckless/parallax/blob/main/docs/cli.md)
and the per-target docs it links. Copyright 2026 Syed Ali (xuckless). Licensed Apache-2.0 AND BSD-3-Clause: it
includes the Gradle Wrapper jar (Apache-2.0) and code ported from Flutter's flutter_tools templates and version rules
and Flutter's Android deployment guide (BSD-3-Clause, LICENSE-FLUTTER and LICENSE-FLUTTER-WEBSITE); see
[NOTICE](https://github.com/xuckless/parallax/blob/main/NOTICE).
