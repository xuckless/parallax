# Sourced, never run: examples/hello on Android as android-smoke.sh and record-traces.sh both install and start it,
# with cargo-parallax (docs/cli.md). The caller sets $repo, the repository root.

# hello's application id and launcher Activity (examples/hello/parallax.toml, ParallaxActivity).
hello_app=io.github.xuckless.parallax.hello
hello_component=$hello_app/io.github.xuckless.parallax.ParallaxActivity

# Builds cargo-parallax, then builds, installs and launches hello on device $1 (default: the only one adb sees).
hello_android_run() {
  cargo build --quiet -p cargo-parallax --manifest-path "$repo/Cargo.toml"
  (cd "$repo/examples/hello" && "$repo/target/debug/cargo-parallax" run android ${1:+--device "$1"})
}
