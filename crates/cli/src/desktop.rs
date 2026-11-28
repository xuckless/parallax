//! `cargo parallax run|build linux|windows [--release]`: plain `cargo run` / `cargo build` of the app crate for the
//! host, so every target has one entry point. No bundle, no extra features: the workspace's `gpui_platform` features
//! pick the windowing backends (docs/cli-desktop.md). macOS builds a real `.app` instead (macos.rs).

use std::path::Path;
use std::process::Command;

use crate::cargo::{self, CARGO_FIX, Profile, step};
use crate::config::Config;
use crate::error::CliError;

pub fn run(app_dir: &Path, _config: &Config, flags: &[String]) -> Result<(), CliError> {
    cargo(app_dir, "run", "cargo run", flags)
}

pub fn build(app_dir: &Path, _config: &Config, flags: &[String]) -> Result<(), CliError> {
    cargo(app_dir, "build", "cargo build", flags)
}

fn cargo(
    app_dir: &Path,
    verb: &str,
    step_name: &'static str,
    flags: &[String],
) -> Result<(), CliError> {
    let profile = Profile::from_flags(flags)?;
    step(
        Command::new(cargo::cargo())
            .args([verb, "--manifest-path"])
            .arg(app_dir.join("Cargo.toml"))
            .args(profile.cargo_args()),
        CARGO_FIX,
        step_name,
        "See cargo's output above".into(),
    )
}
