# cargo-parallax

`cargo parallax run android` builds a [Parallax](https://github.com/xuckless/parallax) app crate, wraps it in a
generated Gradle project, installs the APK and launches it. Configured by `parallax.toml` next to `Cargo.toml`.

```sh
cargo install cargo-parallax
cargo parallax run android [--release] [--device <serial>]
```

Keys, requirements (Android SDK, NDK, Java 17) and each step:
[docs/cli.md](https://github.com/xuckless/parallax/blob/main/docs/cli.md). Licensed Apache-2.0; includes the Gradle
Wrapper jar (Apache-2.0, see [NOTICE](https://github.com/xuckless/parallax/blob/main/NOTICE)).
