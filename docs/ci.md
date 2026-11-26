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
| linux | ubuntu-latest | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo package` of the published crates, [`scripts/java-test.sh`](android-tests.md) (JDK 21, Gradle and Robolectric jars cached) |
| windows | windows-latest | clippy (`-D warnings`), `cargo test --workspace` |
| macos | macos-latest | clippy (`-D warnings`), `cargo test --workspace` |
| ios | macos-latest | `cargo check --workspace` for `aarch64-apple-ios` and `aarch64-apple-ios-sim` |
| android | ubuntu-latest | `cargo clippy --workspace --all-targets --target aarch64-linux-android -- -D warnings`, `cargo package` of the published libraries |
| wasm | ubuntu-latest | `cargo check --workspace --target wasm32-unknown-unknown`, `cargo package` of the published libraries |

The Linux system libraries gpui_linux links are listed once, in `LINUX_PACKAGES` at the top of `ci.yml`
(a subset of Zed's `script/linux`).

## Android and wasm

Both check the whole workspace, nothing excluded.

- **Android:** `parallax-platform` depends on upstream `gpui_platform` only off Android (it has no Android arm), so
  every crate compiles there. See [platform.md](platform.md).
- **wasm:** single-threaded on stable (AGENTS.md "Web threading"). The workspace takes `gpui_platform` without its
  default `multithreaded` feature, whose `wasm_thread` needs nightly.

A target that cannot build a crate gets an explicit `--exclude` with a comment in `ci.yml`, never `continue-on-error`.

## Published crates build on stock upstream

`cargo package` verifies each crate by building its unpacked tarball under `target/package/`. That build ignores the
workspace `[patch]` and takes `gpui-pre` from crates.io (cargo 1.98; the `Compiling gpui-pre` lines show no path),
so it proves [AGENTS.md](../AGENTS.md#upstream-and-patch-rules) patch rule 11. Several `-p` in one call package
together, so dependents verify against the just-packaged crates before they are on crates.io. The crate list is
`PUBLISHED_LIBS` at the top of `ci.yml`; `cargo-parallax`, a host tool, is packaged on Linux only.
Local copies standing in for patches: [patches.md](patches.md#published-crates).

## Run the same checks locally

```sh
scripts/apply-patches.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/java-test.sh
cargo package -p parallax-android -p parallax-platform -p parallax -p cargo-parallax
cargo check --workspace --target wasm32-unknown-unknown
```

Android needs the NDK's clang as C compiler and linker (CI reads `$ANDROID_NDK_LATEST_HOME`; locally use your
`~/Android/Sdk/ndk/<version>`):

```sh
bin=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
CC_aarch64_linux_android=$bin/aarch64-linux-android28-clang AR_aarch64_linux_android=$bin/llvm-ar \
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$bin/aarch64-linux-android28-clang \
  cargo clippy --workspace --all-targets --target aarch64-linux-android -- -D warnings
```

iOS checks need macOS with Xcode: `cargo check --workspace --target aarch64-apple-ios`.

## Add a target

1. Add its rustup target to [rust-toolchain.toml](../rust-toolchain.toml).
2. Add one row to the `matrix.include` table in [ci.yml](../.github/workflows/ci.yml): `target`, `os`, `checks`.
   Anything a target cannot build yet is an explicit `--exclude` with a comment saying why.
3. Add a row to the table above in the same change.
