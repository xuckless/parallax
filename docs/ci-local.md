# CI checks, locally

The checks [ci.yml](../.github/workflows/ci.yml) runs ([ci.md](ci.md)), from a checkout:

```sh
scripts/apply-patches.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/java-test.sh
scripts/patch-test.sh
scripts/android-smoke.sh            # with one emulator or device running
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

iOS checks need macOS with Xcode: `cargo check --workspace --target aarch64-apple-ios`. Windows touch needs a Windows
desktop session: `scripts/record-traces.sh windows && scripts/windows-touch-checks.sh`. Linux touch needs Weston's
build dependencies: `WESTON_SCALE=2 scripts/record-traces.sh wayland`, then `wayland` and `x11` at scale 1, each
followed by `cargo test -p parallax-platform --test trace_replay` ([traces-desktop.md](traces-desktop.md)).
