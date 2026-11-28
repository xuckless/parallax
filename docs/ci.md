# CI

[ci.yml](../.github/workflows/ci.yml) builds Parallax for the six targets of [ROADMAP Phase 0](../ROADMAP.md):
one job per target, no feature variants, plus one job each for the Java tests, the patch tests, Linux touch, Windows
touch and the Android smoke test. Runs: [GitHub Actions](https://github.com/xuckless/parallax/actions).

Every job checks out, installs the toolchain from [rust-toolchain.toml](../rust-toolchain.toml), runs
[apply-patches.sh](../scripts/apply-patches.sh) to generate `build/` (patched GPUI and the
[fork crates](gpui-fork-crates.md), never committed), then runs its checks. Cargo output is cached per target.

## Jobs

Eight jobs run on every push, pull request and manual `workflow_dispatch`. Three drive a device or a desktop:
`android-smoke` boots an emulator and builds a two-ABI APK (about 5 minutes warm, 7 cold), `linux-touch` builds
Weston, `windows-touch` injects into a desktop session. They run only where a merge is decided: pull requests, pushes
to `main` and manual runs; start them on a branch with `gh workflow run ci.yml --ref <branch>` (emulator setup:
[android-smoke-ci.md](android-smoke-ci.md#in-ci)). A newer push to the same ref cancels the older run.

| Target | Runner | Runs |
|---|---|---|
| linux | ubuntu-latest | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo package -p cargo-parallax`, `scripts/publish-fork.sh --dry-run`, package of the published libraries |
| windows | windows-latest | clippy (`-D warnings`), `cargo test --workspace` |
| macos | macos-latest | clippy (`-D warnings`), `cargo test --workspace` |
| ios | macos-latest | `cargo check --workspace` for `aarch64-apple-ios` and `aarch64-apple-ios-sim` |
| android | ubuntu-latest | `cargo clippy --workspace --all-targets --target aarch64-linux-android -- -D warnings`, package of the published libraries |
| wasm | ubuntu-latest | `cargo check --workspace --target wasm32-unknown-unknown`, package of the published libraries |
| java-tests | ubuntu-latest | [`scripts/java-test.sh`](android-tests.md) on JDK 21 and the runner's Android SDK; Gradle and Robolectric jars cached. Its own job (`java-tests`, not a matrix row): no Rust, no `build/` |
| patch-tests | ubuntu-latest | [`scripts/patch-test.sh`](../scripts/patch-test.sh): GPUI's own lib tests on the patched fork crate, every patch's tests among them, then gpui_windows's and gpui_web's pure modules as the host crates `windows-pure` and `web-pure`, and the Linux backend's ([patch-tests.md](patch-tests.md)). Its own job, not a matrix row |
| linux-touch | ubuntu-latest | Weston built from source, then [`record-traces.sh`](traces-desktop.md#linux-weston-headless) `wayland` at scale 2, `trace_replay`, `wayland` and `x11` at scale 1, `trace_replay`, then [`weston-selection-checks.sh`](text-selection-tests.md#live-checks); the recordings and logs are uploaded. Its own job |
| windows-touch | windows-latest | `scripts/record-traces.sh windows`, `scripts/windows-touch-checks.sh`, then `trace_replay` on the fresh traces, then `scripts/windows-native-view-checks.sh` ([native-view-windows.md](native-view-windows.md)); the traces, logs and NativeView screenshots are the `windows-touch` artifact ([windows-touch-harness.md](windows-touch-harness.md)). Its own job |
| android-smoke | ubuntu-latest + KVM | [`scripts/android-smoke.sh`](android-smoke.md) on an API 35 `google_apis` x86_64 emulator (Pixel 7 profile). Its own job, not a matrix row |

The Linux system libraries gpui_linux links are listed once, in `LINUX_PACKAGES` at the top of `ci.yml` (a subset of Zed's `script/linux`).

## Android and wasm

Both check the whole workspace, nothing excluded. **Android:** `parallax-platform` depends on upstream `gpui_platform`
only off Android (it has no Android arm), so every crate compiles there ([platform.md](platform.md)). **wasm:**
single-threaded on stable (AGENTS.md "Web threading"): the workspace takes `gpui_platform` without its default
`multithreaded` feature, whose `wasm_thread` needs nightly.

## Published crates

Linux runs `scripts/publish-fork.sh --dry-run` (the fork crates package), and rows with a `package_target` package
`PUBLISHED_LIBS` once their fork version is on crates.io; `cargo-parallax` is packaged on Linux every run. What
each check proves and the publish-first order: [gpui-fork-crates.md](gpui-fork-crates.md#ci).

## Run the same checks locally

The commands, the Android NDK setup and the desktop touch runs: [ci-local.md](ci-local.md).

## Add a target

1. Add its rustup target to [rust-toolchain.toml](../rust-toolchain.toml).
2. Add one row to the `matrix.include` table in [ci.yml](../.github/workflows/ci.yml): `target`, `os`, `checks`. Anything
   a target cannot build yet is an explicit `--exclude` with a comment saying why, never `continue-on-error`.
3. Add a row to the table above in the same change.
