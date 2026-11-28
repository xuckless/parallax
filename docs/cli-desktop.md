# cargo-parallax on the desktop

`cargo parallax run|build macos|linux|windows [--release]` builds the app crate in the current directory for this
machine. `cargo parallax run` (or `build`) with no target picks the host's desktop: the `TARGETS` row whose `host`
is `std::env::consts::OS` in [main.rs](../crates/cli/src/main.rs). Desktop targets are never cross-built: `run
windows` on Linux is a `WrongHost` error. Args, `parallax.toml` and errors: [cli.md](cli.md).

## macOS: a real `.app`

[macos.rs](../crates/cli/src/macos.rs) (`bundle`):

1. Renders `app.icon` into `AppIcon.icns` first, so a bad SVG fails before the build: the ten sizes of Flutter's
   macOS `AppIcon.appiconset` (`ICONSET`, 16–1024 px), each the SVG at 824/1024 of the canvas (Apple's icon grid),
   packed by the [icns](https://crates.io/crates/icns) crate with the PNG element types `iconutil` writes. Pure
   Rust, so tests run on any host.
2. `cargo metadata --no-deps`: the app package, its one bin target (none or several is an error: `NoBin`,
   `SeveralBins`), its version and the target directory.
3. `cargo build --bin <bin> --target <t>…`, one `--target` per `macos.archs` row (`ARCHS`: `arm64` →
   `aarch64-apple-darwin`, `x86_64` → `x86_64-apple-darwin`).
4. Writes `<target dir>/parallax/macos/<package>/<app.name>.app` from scratch: `Contents/Info.plist`,
   `Contents/Resources/AppIcon.icns`, `Contents/MacOS/<bin>` (copied for one arch; `lipo -create` for several, a
   universal binary).
5. `codesign --force --sign -` (ad-hoc) unless `macos.codesign = false`.
6. `run` then starts `Contents/MacOS/<bin>` directly (not `open`): AppKit still finds the bundle's Info.plist, the
   logs stream into the terminal and Ctrl-C stops the app.

[Info.plist](../crates/cli/templates/macos/Info.plist) is flutter_tools' `macos.tmpl/Runner/Info.plist` with its
`AppInfo.xcconfig` settings filled in (ledger: [PORTED.md](../third_party/flutter/PORTED.md)):

| Key | Value |
|---|---|
| `CFBundleIdentifier` / `CFBundleName` | `app.id` / `app.name` |
| `CFBundleExecutable` | the bin target's name |
| `CFBundleShortVersionString` / `CFBundleVersion` | `app.version`, else the package version, as Flutter turns `1.2.3+4` into build name / number ([version.rs](../crates/cli/src/version.rs)): `0.1.10` → `0.1.10` / `0.1.10` |
| `CFBundleIconFile` | `AppIcon` with `app.icon`, else empty (the generic icon) |
| `LSMinimumSystemVersion` | `12.0`: GPUI's minimum (gpui_apple compiles its Metal shaders for macOS 12) |
| `NSHighResolutionCapable` | `true` |
| `NSIsTouchNative` | `macos.touch_native` ([desktop-touch-macos-emulation.md](desktop-touch-macos-emulation.md)) |

`[macos]` keys (defaults: `MACOS_DEFAULTS` in [config.rs](../crates/cli/src/config.rs)):

| Key | Default | Meaning |
|---|---|---|
| `macos.touch_native` | `false` | `true`: direct touches reach GPUI only as `TouchEvent`s; `false`: AppKit's tap → click, pan → scroll emulation |
| `macos.archs` | `["arm64"]` | Archs to build; two make a universal binary (`rustup target add x86_64-apple-darwin` first) |
| `macos.codesign` | `true` | Ad-hoc sign the bundle, so Gatekeeper opens it without a "damaged" prompt. Not a Developer ID signature: a bundle downloaded on another Mac is still quarantined |

Needs the Xcode command line tools (`xcrun metal` for GPUI's shaders, `lipo`, `codesign`): `xcode-select --install`.

## Linux and Windows: `cargo run`

[desktop.rs](../crates/cli/src/desktop.rs) runs `cargo run` (or `cargo build`) `--manifest-path Cargo.toml
[--release]` for the host target: no bundle, no extra features, no `--target`. The workspace's `gpui_platform`
features decide the backends: Linux builds both `wayland` and `x11` and picks Wayland when `WAYLAND_DISPLAY` is set;
Windows has one backend (DirectX 11, gpui_windows). The app crate needs exactly one bin (cargo's own error
otherwise). Linux system packages: [examples-run.md](examples-run.md). App arguments after `--` go to `cargo run -- …`;
on macOS to the bundle's executable (`cargo parallax run windows -- --view webview2`).

## Checks

- `cargo test -p cargo-parallax`: main.rs `command_table` (verbs, targets, host picks, `WrongHost`), `cargo::bin_table`,
  `config::parse_table` (`[macos]` rows), `info_plist_table`, `version::versions_table`, Flutter's
  `CFBundleVersion for iOS` and `CFBundleShortVersionString for iOS` (`build_info_test.dart`), and
  `icon::icns_has_every_iconset_size`.
- macOS (Wave 16, pixl-mac, macOS 27.3): `build macos` for `hello` and `native-view` passes `plutil -lint` and
  `codesign --verify --strict`; `iconutil -c iconset` unpacks all ten sizes; `archs = ["arm64", "x86_64"]` gives a
  universal binary. Over ssh there is no window server, so `run macos` was seen to start and stay up, not to open a
  window; the owner's check (window, icon, `touch_native` on Sidecar) is in
  `/Users/Shared/parallax/w16-desktop-macos/CHECK.md` on the owner's Mac.
- Linux: `cargo parallax run linux` for `hello` in a private headless GNOME Shell logs `window open`.
- Windows: `cargo clippy -p cargo-parallax --target x86_64-pc-windows-gnu` compiles; the Gate 5 CI job runs it on
  the Windows runner.
