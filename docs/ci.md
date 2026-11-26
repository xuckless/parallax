# CI

[ci.yml](../.github/workflows/ci.yml) builds Parallax for the six targets of [ROADMAP Phase 0](../ROADMAP.md):
one job per target, no feature variants. Runs: [GitHub Actions](https://github.com/xuckless/parallax/actions).

Every job checks out, installs the toolchain from [rust-toolchain.toml](../rust-toolchain.toml), runs
[apply-patches.sh](../scripts/apply-patches.sh) to generate `build/` (patched GPUI and the
[fork crates](gpui-fork-crates.md), never committed), then runs its checks. Cargo output is cached per target.

## Jobs

All six jobs run on every push, every pull request and on manual `workflow_dispatch`. A newer push to the same ref
cancels the older run.

| Target | Runner | Runs |
|---|---|---|
| linux | ubuntu-latest | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo package -p cargo-parallax`, `scripts/publish-fork.sh --dry-run`, package of the published libraries, [`scripts/java-test.sh`](android-tests.md) (JDK 21, Gradle and Robolectric jars cached) |
| windows | windows-latest | clippy (`-D warnings`), `cargo test --workspace` |
| macos | macos-latest | clippy (`-D warnings`), `cargo test --workspace` |
| ios | macos-latest | `cargo check --workspace` for `aarch64-apple-ios` and `aarch64-apple-ios-sim` |
| android | ubuntu-latest | `cargo clippy --workspace --all-targets --target aarch64-linux-android -- -D warnings`, package of the published libraries |
| wasm | ubuntu-latest | `cargo check --workspace --target wasm32-unknown-unknown`, package of the published libraries |

The Linux system libraries gpui_linux links are listed once, in `LINUX_PACKAGES` at the top of `ci.yml`
(a subset of Zed's `script/linux`).

## Android and wasm

Both check the whole workspace, nothing excluded.

- **Android:** `parallax-platform` depends on upstream `gpui_platform` only off Android (it has no Android arm), so
  every crate compiles there. See [platform.md](platform.md).
- **wasm:** single-threaded on stable (AGENTS.md "Web threading"). The workspace takes `gpui_platform` without its
  default `multithreaded` feature, whose `wasm_thread` needs nightly.

A target that cannot build a crate gets an explicit `--exclude` with a comment in `ci.yml`, never `continue-on-error`.

## Published crates

Two checks, because Parallax crates depend on our [fork crates](gpui-fork-crates.md) (AGENTS.md patch rule 11):

- **Fork crates package.** `scripts/publish-fork.sh --dry-run` (Linux) runs `cargo package --workspace` on
  `build/parallax-gpui/`: each fork crate is built from its tarball against its just-packaged siblings.
  Every job also builds the whole Parallax workspace on those same crates, through the `[patch.crates-io]` rows.
- **Parallax crates package.** `cargo package` builds each tarball without the workspace `[patch]`, so it resolves
  `parallax-gpui*` from crates.io. It runs only when `scripts/publish-fork.sh --published` finds the pinned version
  there; otherwise the step warns. So publish the fork crates first ([gpui-fork-crates.md](gpui-fork-crates.md#publishing)),
  then land the pin bump or release on them. List: `PUBLISHED_LIBS` in `ci.yml`, packaged per row's `package_target`;
  `cargo-parallax` (no GPUI) is packaged on Linux every run.

## Run the same checks locally

```sh
scripts/apply-patches.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/java-test.sh
cargo package -p cargo-parallax
scripts/publish-fork.sh --dry-run
scripts/publish-fork.sh --published && cargo package -p parallax-android -p parallax-platform -p parallax
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
