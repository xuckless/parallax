# CI

[ci.yml](../.github/workflows/ci.yml) builds Parallax for the six targets of [ROADMAP Phase 0](../ROADMAP.md):
one job per target, no feature variants. Runs: [GitHub Actions](https://github.com/xuckless/parallax/actions).

Every job checks out, installs the toolchain from [rust-toolchain.toml](../rust-toolchain.toml), runs
[apply-patches.sh](../scripts/apply-patches.sh) to generate `build/gpui-patched/` (never committed), then runs its checks.
Cargo output is cached per target.

## Jobs

All six jobs run on every push, every pull request and on manual `workflow_dispatch`. A newer push to the same ref
cancels the older run.

| Target | Runner | Runs |
|---|---|---|
| linux | ubuntu-latest | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |
| windows | windows-latest | clippy (`-D warnings`), `cargo test --workspace` |
| macos | macos-latest | clippy (`-D warnings`), `cargo test --workspace` |
| ios | macos-latest | `cargo check --workspace` for `aarch64-apple-ios` and `aarch64-apple-ios-sim` |
| android | ubuntu-latest | `cargo check --workspace --exclude parallax` and `cargo check -p gpui-pre`, `aarch64-linux-android` |
| wasm | ubuntu-latest | `cargo check --workspace --exclude parallax`, `wasm32-unknown-unknown` |

The Linux system libraries gpui_linux links are listed once, in `LINUX_PACKAGES` at the top of `ci.yml`
(a subset of Zed's `script/linux`).

## What Android and wasm exclude today

- **Android** skips the `parallax` facade. Upstream `gpui_platform::current_platform()` is a `cfg_select!` with no
  Android arm, so it does not compile there. GPUI core (`gpui-pre`) does compile and is checked on its own.
  The facade comes back once `parallax-android` provides the platform.
- **wasm** skips the `parallax` facade. GPUI on wasm needs nightly Rust: `wasm_thread` uses `#![feature]` and
  `gpui_web` turns on `parking_lot/nightly`. Parallax pins upstream's stable channel, and upstream's own web gallery
  builds with nightly.

Both exclusions are explicit `--exclude` flags with a comment in `ci.yml`, never `continue-on-error`.

## Run the same checks locally

```sh
scripts/apply-patches.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check --workspace --exclude parallax --target wasm32-unknown-unknown
```

Android needs the NDK's clang as C compiler and linker (CI reads `$ANDROID_NDK_LATEST_HOME`; locally use your
`~/Android/Sdk/ndk/<version>`):

```sh
bin=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
CC_aarch64_linux_android=$bin/aarch64-linux-android28-clang AR_aarch64_linux_android=$bin/llvm-ar \
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$bin/aarch64-linux-android28-clang \
  cargo check --workspace --exclude parallax --target aarch64-linux-android
```

iOS checks need macOS with Xcode: `cargo check --workspace --target aarch64-apple-ios`.

## Add a target

1. Add its rustup target to [rust-toolchain.toml](../rust-toolchain.toml).
2. Add one row to the `matrix.include` table in [ci.yml](../.github/workflows/ci.yml): `target`, `os`, `checks`.
   Anything a target cannot build yet is an explicit `--exclude` with a comment saying why.
3. Add a row to the table above in the same change.
