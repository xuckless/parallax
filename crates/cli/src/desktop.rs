//! `cargo parallax run|build linux|windows`: plain `cargo run` / `cargo build` of the app crate for the host, so every
//! target has one entry point. No bundle, no extra features: the workspace's `gpui_platform` features pick the
//! windowing backends (docs/cli-desktop.md). macOS builds a real `.app` instead (macos.rs).

use std::path::Path;
use std::process::Command;

use crate::android::{CARGO_FIX, RunArgs, step};
use crate::config::Config;
use crate::error::CliError;

pub fn run(app_dir: &Path, _config: &Config, args: &RunArgs) -> Result<(), CliError> {
    cargo(app_dir, "run", "cargo run", args)
}

pub fn build(app_dir: &Path, _config: &Config, args: &RunArgs) -> Result<(), CliError> {
    cargo(app_dir, "build", "cargo build", args)
}

fn cargo(
    app_dir: &Path,
    verb: &str,
    step_name: &'static str,
    args: &RunArgs,
) -> Result<(), CliError> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    step(
        Command::new(cargo)
            .args([verb, "--manifest-path"])
            .arg(app_dir.join("Cargo.toml"))
            .args(args.profile.cargo_args()),
        CARGO_FIX,
        step_name,
        "See cargo's output above".into(),
    )
}
