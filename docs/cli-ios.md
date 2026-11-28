# cargo parallax on iOS

`cargo parallax run ios [--release] [--simulator <name|udid> | --device [<name|udid>]]` builds the app crate's binary,
wraps it in a generated Xcode project, installs it and runs it with its log on the terminal.
`cargo parallax build ios [--release]` stops at the device `.app`. macOS only (Xcode); elsewhere `WrongHost`.
Source: [ios.rs](../crates/cli/src/ios.rs), [templates/ios/](../crates/cli/templates/ios/), keys in
[config.rs](../crates/cli/src/config.rs). Overview and the shared `[app]` keys: [cli.md](cli.md).

## parallax.toml `[ios]`

Defaults are one table, `IOS_DEFAULTS` in [config.rs](../crates/cli/src/config.rs).

| Key | Default | Meaning |
|---|---|---|
| `ios.team` | none | Apple team id (ten characters, Membership details on developer.apple.com). Needed for `--device`; `build ios` without it is unsigned. Only the id: Xcode signs with the account signed in under Xcode → Settings → Accounts. Never put credentials here |
| `ios.min_ios` | `"17.0"` | `IPHONEOS_DEPLOYMENT_TARGET` for cargo and Xcode; below 17 is an error ([AGENTS.md](../AGENTS.md), Minimum OS) |

`app.name` is the home-screen name, `app.id` the bundle id (iOS refuses `_`, which Android allows), `app.icon` is
rendered with `resvg` to every file of the AppIcon set, opaque over the SVG's tile colour (white if none). The bundle
versions are `app.version` (else the crate's) through Flutter's build-name / build-number rules ([version.rs](../crates/cli/src/version.rs)).

## What `run ios` does

1. `cargo metadata`: the app package, its bin target (the only one, or the one named like the package) and
   the target directory. iOS runs the binary (`fn main` calling `parallax::run`), like desktop.
2. Picks before the long build: `--simulator` matches an available iOS simulator's name or udid (`simctl list`),
   without it exactly one must be booted; `--device` takes the only paired physical iOS device (`devicectl list
   devices`), or the one named. `--device` needs `ios.team`.
3. `cargo build --bin <bin> --target <t>` with `IPHONEOS_DEPLOYMENT_TARGET`; destination × host CPU → Rust target is
   the `RUST_TARGETS` table (`aarch64-apple-ios-sim`, `x86_64-apple-ios` on Intel, `aarch64-apple-ios`).
4. Writes the project to `<target dir>/parallax/ios/<package>/`: `Runner/` is deleted and every `PROJECT_FILES` row is
   rewritten, plus `Parallax/Generated.xcconfig` (binary path per SDK, versions, `DEVELOPMENT_TEAM`). `build/` stays.
   It opens in Xcode as is; after a Rust change rerun cargo parallax (Xcode only copies the binary in).
5. `xcodebuild` with Flutter's arguments (`buildXcodeProject` in flutter_tools' `mac.dart`): `-quiet`,
   `BUILD_DIR=<project>/build`, `-sdk`, `-destination id=<udid>`; simulators and unsigned builds get
   `CODE_SIGNING_ALLOWED=NO`, a team gets `-allowProvisioningUpdates -allowProvisioningDeviceRegistration`.
6. Simulator: `simctl bootstatus -b` (boots it and waits), `simctl install`, `simctl launch --console-pty`. The
   window is Xcode's own app (Simulator; DeviceHub in Xcode 27), which you open; the app runs without it.
   Device: `devicectl device install app`, `devicectl device process launch --console`. The app's stdout and stderr
   (env_logger) stream until it exits; Ctrl-C stops it. Env for the app: `SIMCTL_CHILD_RUST_LOG=debug` (simulator),
   `DEVICECTL_CHILD_RUST_LOG=debug` (device). App arguments after `--` follow the bundle id on the launch line.

`--release` builds Rust with `--release` and Xcode's Release configuration. `build ios` prints the `.app` path,
`build/<Configuration>-iphoneos/Runner.app`.

## Generated project

Ported from Flutter's `packages/flutter_tools/templates/app/ios.tmpl` @ 3.50.1 (ledger:
[PORTED.md](../third_party/flutter/PORTED.md)). Flutter's object ids are kept, so later template fixes diff in.

| File | Notes |
|---|---|
| [project.pbxproj](../crates/cli/templates/ios/Runner.xcodeproj/project.pbxproj) | One `Runner` target: "Run Script" copies `$PARALLAX_BINARY` in as the executable, then Resources. `TARGETED_DEVICE_FAMILY = "1,2"`: iPads run natively, not scaled |
| [Info.plist](../crates/cli/templates/ios/Runner/Info.plist) | GPUI's scene delegate `GPUIIosSceneDelegate` under "Default Configuration", Flutter's orientations, 120 Hz on ProMotion phones |
| [LaunchScreen.storyboard](../crates/cli/templates/ios/Runner/Base.lproj/LaunchScreen.storyboard) | White until GPUI's first frame |
| [AppIcon Contents.json](../crates/cli/templates/ios/Runner/Assets.xcassets/AppIcon.appiconset/Contents.json) | Flutter's set, verbatim; its `images` are the table of PNGs `app.icon` is rendered to (no `app.icon`: no PNGs, iOS's blank icon) |
| [Debug/Release.xcconfig](../crates/cli/templates/ios/Parallax/) | Include `Generated.xcconfig` |
| [Runner.xcscheme](../crates/cli/templates/ios/Runner.xcodeproj/xcshareddata/xcschemes/Runner.xcscheme) | Shared scheme for `xcodebuild -scheme Runner` and Xcode |

## Requirements

| Need | If missing |
|---|---|
| Xcode (`xcrun xcodebuild`, `simctl`, `devicectl`) | Install it, open it once, `sudo xcode-select -s /Applications/Xcode.app` |
| Rust iOS targets | `rustup target add aarch64-apple-ios-sim aarch64-apple-ios` |
| An iOS simulator runtime | Xcode → Settings → Components |
| For `--device`: `ios.team`, a paired, unlocked device, the team's account in Xcode | First run: trust the developer on the device (Settings → General → VPN & Device Management) |
