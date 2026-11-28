# cargo-parallax

`cargo parallax <verb> <target> [flags]` builds the app crate in the current directory for one target, from the
`parallax.toml` next to its `Cargo.toml`. Install: `cargo install --path crates/cli` (or `cargo install cargo-parallax`).

| Command | What it does | Doc |
|---|---|---|
| `run android [--release] [--device <serial>]` | NDK build, generated Gradle project, `adb install`, launch | [cli-android.md](cli-android.md) |
| `build android [--release] [--apk\|--aab]` | The same, no install; `--release`: a signed App Bundle | [cli-android-release.md](cli-android-release.md) |
| `run ios [--release] [--simulator <name\|udid> \| --device [<name\|udid>]]` | cargo build, generated Xcode project, simulator or device run with its log | [cli-ios.md](cli-ios.md) |
| `build ios [--release]` | Device build, signed when `ios.team` is set | [cli-ios.md](cli-ios.md) |
| `run web [--release] [--port <n>] [--host <address>] [--open]` | wasm32 build, wasm-bindgen, host page, local server | [cli-web.md](cli-web.md) |
| `build web [--release]` | The deployable site only; `--release` runs wasm-opt when installed | [cli-web.md](cli-web.md) |
| `run\|build macos [--release]` | A real `.app` (Info.plist, icon, ad-hoc signature), run from inside the bundle | [cli-desktop.md](cli-desktop.md) |
| `run\|build linux\|windows [--release]` | `cargo run` / `cargo build` of the app crate | [cli-desktop.md](cli-desktop.md) |
| `run\|build [--release]` | No target: this machine's desktop | [cli-desktop.md](cli-desktop.md) |
| `run <target> … -- <app args>` | The app's own arguments: desktop `cargo run -- …`, macOS and iOS process arguments, web the URL's query; Android has none yet (`AppArgsUnsupported`: ParallaxActivity passes the app no launch arguments) | each target's doc |

## Source

| File | Holds |
|---|---|
| [main.rs](../crates/cli/src/main.rs) | One `Target` enum, the `TARGETS` table (name, host it builds on, desktop or not) and the `COMMANDS` table (verb, target → handler). Each handler parses its own flags. To add a command: a row there and a line in `USAGE` ([error.rs](../crates/cli/src/error.rs)) |
| [cargo.rs](../crates/cli/src/cargo.rs) | What every target shares: `Profile` (`--release`, Cargo's dir, Gradle's and Xcode's build type), `cargo metadata` (the app package, its `cdylib`, its bin), running tools (`step`, `capture`) |
| [template.rs](../crates/cli/src/template.rs) | `{{key}}` templates, XML escaping, writing generated files, clearing a generated tree |
| [config.rs](../crates/cli/src/config.rs) | `parallax.toml`: one `*_DEFAULTS` table per section (`APP_`, `ANDROID_`, `IOS_`, `WEB_`, `MACOS_`), every value checked |
| [icon.rs](../crates/cli/src/icon.rs) | `app.icon` rendered for each target: Android launcher icon, iOS AppIcon set, macOS `AppIcon.icns` |
| [version.rs](../crates/cli/src/version.rs) | Flutter's build-name / build-number rules: `app.version` → Android `versionName`, Apple `CFBundleShortVersionString` / `CFBundleVersion` |
| [android.rs](../crates/cli/src/android.rs), [ios.rs](../crates/cli/src/ios.rs), [web.rs](../crates/cli/src/web.rs), [macos.rs](../crates/cli/src/macos.rs), [desktop.rs](../crates/cli/src/desktop.rs) | One module per target; templates in [templates/\<target\>/](../crates/cli/templates/) |
| [error.rs](../crates/cli/src/error.rs) | Every failure, each naming what to set or install |

A target with a host (iOS and macOS: macOS; Linux; Windows) is never cross-built: on another OS the command fails
with `WrongHost` before anything runs.

## parallax.toml

Unknown keys and tables are errors. The `[app]` table is shared by every target; each target's own table is in its
doc: `[android]` [cli-android.md](cli-android.md), `[android.release]` [cli-android-release.md](cli-android-release.md),
`[ios]` [cli-ios.md](cli-ios.md), `[web]` [cli-web.md](cli-web.md), `[macos]` [cli-desktop.md](cli-desktop.md).

| Key | Required | Default (`APP_DEFAULTS`) | Meaning |
|---|---|---|---|
| `app.name` | yes | | Launcher label, `CFBundleName`, the web page's title unless `web.title` |
| `app.id` | yes | | Android application id and Gradle namespace; Apple bundle id (no `_` on iOS), e.g. `io.github.xuckless.parallax.hello` |
| `app.icon` | no | none: each platform's default icon | SVG path relative to `parallax.toml`, rendered fresh each run ([icon.rs](../crates/cli/src/icon.rs)) |
| `app.version` | no | the Cargo package version | Flutter's `1.2.3+4` form: build name `1.2.3`, build number `4` ([version.rs](../crates/cli/src/version.rs)) |

There is no `lib` or `bin` key: names come from the crate's targets in `cargo metadata`, so they cannot drift from
`Cargo.toml`.

```toml
[app]
name = "Hello"
id = "io.github.xuckless.parallax.hello"
icon = "../../assets/brand/parallax-app-icon.svg"

[android]
abis = ["x86_64"]
```

CI runs every command from a clean checkout: [ci.md](ci.md).
