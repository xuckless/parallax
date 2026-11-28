# cargo-parallax

`cargo parallax run android` builds a [Parallax](https://github.com/xuckless/parallax) app crate, wraps it in a
generated Gradle project, installs the APK and launches it; `cargo parallax build android --release` makes a
release-signed App Bundle for Google Play. Configured by `parallax.toml` next to `Cargo.toml`.

```sh
cargo install cargo-parallax
cargo parallax run android [--release] [--device <serial>]
cargo parallax build android [--release] [--apk|--aab]
```

Keys, requirements (Android SDK, NDK, Java 17) and each step:
[docs/cli.md](https://github.com/xuckless/parallax/blob/main/docs/cli.md); release signing:
[docs/cli-android-release.md](https://github.com/xuckless/parallax/blob/main/docs/cli-android-release.md).
Copyright 2026 Syed Ali (xuckless). Licensed Apache-2.0 AND BSD-3-Clause: it includes the Gradle Wrapper jar
(Apache-2.0) and code ported from Flutter (BSD-3-Clause, LICENSE-FLUTTER and LICENSE-FLUTTER-WEBSITE); see
[NOTICE](https://github.com/xuckless/parallax/blob/main/NOTICE).
