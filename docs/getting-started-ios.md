# Getting started: iOS

Shared setup first: [getting-started.md](getting-started.md). iOS 17 and up, iPhone and iPad (native, not scaled).
Builds on macOS only (Xcode). Reference: [cli-ios.md](cli-ios.md).

## Prerequisites

| Need | Command |
|---|---|
| Xcode | from the App Store; open it once, then `sudo xcode-select -s /Applications/Xcode.app` |
| An iOS simulator runtime | Xcode → Settings → Components |
| Rust targets | `rustup target add aarch64-apple-ios-sim aarch64-apple-ios` (`x86_64-apple-ios` on an Intel Mac) |

Boot a simulator (or pass `--simulator` and cargo parallax boots it):

```sh
xcrun simctl list devices available | grep iPhone
xcrun simctl boot "iPhone 17"
open -a Simulator            # the window; the app runs without it
```

## Run hello on the simulator

```sh
cd examples/hello
cargo parallax run ios                          # the one booted simulator
cargo parallax run ios --simulator "iPhone 17"  # by name or udid
```

The app's log streams to the terminal until it exits; Ctrl-C stops it. `RUST_LOG` reaches it as
`SIMCTL_CHILD_RUST_LOG=debug`. The generated Xcode project lands in `target/parallax/ios/<package>/` and opens in
Xcode as is (after a Rust change, rerun cargo parallax: Xcode only copies the binary in).

## Run on your iPhone or iPad

parallax.toml holds only your Apple team id (Membership details on developer.apple.com); Xcode signs with the
account you are signed in to. Never put a password or certificate in the file.

```toml
[ios]
team = "ABCDE12345"
```

1. Xcode → Settings → Accounts: sign in with the Apple account of that team (once).
2. Connect the device by cable (or pair it over the network in Xcode's Devices window) and unlock it; on iOS 16+
   turn on Settings → Privacy & Security → Developer Mode (it restarts).
3. `cargo parallax run ios --device` (the only paired device) or `--device "<name|udid>"`. Xcode registers the
   device and makes the profile (`-allowProvisioningUpdates`).
4. First run on a device: trust the developer under Settings → General → VPN & Device Management, then run again.

`cargo parallax build ios [--release]` stops at the device `.app` and prints its path (unsigned without a team).

## Keys in parallax.toml

`ios.team` (none), `ios.min_ios` (`"17.0"`). `app.id` is the bundle id; iOS refuses `_`, so pick an id both Android
and iOS accept. The version shown is the crate's `version`. Table: [cli-ios.md](cli-ios.md#parallaxtoml-ios).

## Common errors

| Error says | Fix |
|---|---|
| `builds iOS apps on macOS only` | run it on a Mac |
| `no booted iOS simulator` (`no …; available: …`) | boot one, or pass `--simulator <name>` from the list it prints |
| `several …s` | pick one with `--simulator` / `--device <name|udid>` |
| `running on a device needs an Apple team id` | add `[ios] team = "…"` (above) |
| `ios.team … is not an Apple team id` | ten upper-case letters and digits |
| `app.id … is not an iOS bundle id` | letters, digits, `-` and `.` only |
| `ios.min_ios … is below 17` | Parallax supports iOS 17 and up |
| `package … has no bin target` | add `src/main.rs` calling `parallax::run` |
| `xcodebuild failed` | Xcode's message is above it; signing errors: check the account in Xcode → Settings → Accounts |
| `xcrun … not found` | install Xcode and run `xcode-select` (above) |
