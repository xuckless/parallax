//! What every target shares: the cargo profile, the app crate as `cargo metadata` reports it, and running tools
//! with their output on the terminal.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::error::CliError;

pub const CARGO_FIX: &str = "Install Rust from https://rustup.rs";

/// Cargo profile, picked by `--release`, with the native build type it goes with (Gradle's, Xcode's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    pub fn cargo_args(self) -> &'static [&'static str] {
        match self {
            Profile::Debug => &[],
            Profile::Release => &["--release"],
        }
    }

    /// Cargo's output directory, and the build type as Gradle names output directories and files.
    pub fn dir(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }

    /// The build type as Gradle names tasks and Xcode names configurations.
    pub fn title(self) -> &'static str {
        match self {
            Profile::Debug => "Debug",
            Profile::Release => "Release",
        }
    }

    /// Flags of a command whose only flag is `--release`.
    pub fn from_flags(flags: &[String]) -> Result<Profile, CliError> {
        flags
            .iter()
            .try_fold(Profile::Debug, |_, flag| match flag.as_str() {
                "--release" => Ok(Profile::Release),
                other => Err(CliError::Usage(format!("unknown argument {other:?}"))),
            })
    }
}

/// `cargo metadata --format-version 1`, the parts `cargo parallax` reads.
#[derive(Deserialize)]
pub struct Metadata {
    pub packages: Vec<MetaPackage>,
    pub target_directory: PathBuf,
}

#[derive(Deserialize)]
pub struct MetaPackage {
    pub name: String,
    pub version: String,
    pub manifest_path: PathBuf,
    pub targets: Vec<MetaTarget>,
}

#[derive(Deserialize)]
pub struct MetaTarget {
    pub name: String,
    pub crate_types: Vec<String>,
}

/// The cargo that runs us (`CARGO`, set by `cargo parallax`), else `cargo` on PATH.
pub fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// `cargo metadata` of the workspace holding `manifest`, dependencies included (Android finds parallax-android in
/// it, the web wasm-bindgen's version).
pub fn metadata(manifest: &Path) -> Result<Metadata, CliError> {
    let out = capture(
        Command::new(cargo())
            .args(["metadata", "--format-version", "1", "--manifest-path"])
            .arg(manifest),
        "cargo metadata",
        CARGO_FIX,
    )?;
    serde_json::from_slice(&out).map_err(|e| CliError::Metadata(e.to_string()))
}

impl Metadata {
    /// The app's package: the one whose Cargo.toml is `manifest`.
    pub fn app_package(&self, manifest: &Path) -> Result<&MetaPackage, CliError> {
        // cargo metadata reports canonical manifest paths.
        let manifest = manifest.canonicalize().map_err(|source| CliError::Io {
            path: manifest.to_path_buf(),
            source,
        })?;
        self.packages
            .iter()
            .find(|p| p.manifest_path == manifest)
            .ok_or(CliError::NoAppPackage(manifest))
    }
}

impl MetaPackage {
    /// The library name of the `cdylib` (Android's `.so`, the web's `.wasm`), `-` as `_`.
    pub fn cdylib(&self) -> Result<String, CliError> {
        self.targets
            .iter()
            .find(|t| t.crate_types.iter().any(|c| c == "cdylib"))
            .map(|t| t.name.replace('-', "_"))
            .ok_or_else(|| CliError::NotCdylib(self.name.clone()))
    }

    /// The binary iOS and macOS bundle: the only bin target, or the one named like the package.
    pub fn bin(&self) -> Result<String, CliError> {
        let bins: Vec<&str> = self
            .targets
            .iter()
            .filter(|t| t.crate_types.iter().any(|c| c == "bin"))
            .map(|t| t.name.as_str())
            .collect();
        match bins[..] {
            [] => Err(CliError::NoBin(self.name.clone())),
            [one] => Ok(one.to_string()),
            _ if bins.contains(&self.name.as_str()) => Ok(self.name.clone()),
            _ => Err(CliError::SeveralBins {
                package: self.name.clone(),
                bins: bins.iter().map(|b| b.to_string()).collect(),
            }),
        }
    }
}

pub fn spawn_error(cmd: &Command, fix: &'static str, source: std::io::Error) -> CliError {
    CliError::ToolMissing {
        tool: cmd.get_program().to_string_lossy().into_owned(),
        fix,
        source,
    }
}

/// Runs `cmd`, returning stdout; stderr goes to the terminal.
pub fn capture(
    cmd: &mut Command,
    step: &'static str,
    fix: &'static str,
) -> Result<Vec<u8>, CliError> {
    let out = cmd
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| spawn_error(cmd, fix, e))?;
    match out.status.success() {
        true => Ok(out.stdout),
        false => Err(CliError::StepFailed {
            step,
            status: out.status,
            hint: String::new(),
        }),
    }
}

/// Runs `cmd` with its output on the terminal.
pub fn step(
    cmd: &mut Command,
    fix: &'static str,
    step: &'static str,
    hint: String,
) -> Result<(), CliError> {
    let status = cmd.status().map_err(|e| spawn_error(cmd, fix, e))?;
    match status.success() {
        true => Ok(()),
        false => Err(CliError::StepFailed { step, status, hint }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_flags_table() {
        // (row name, flags, expected profile dir or the error variant)
        let cases: [(&str, &[&str], Result<&str, &str>); 4] = [
            ("no flags", &[], Ok("debug")),
            ("release", &["--release"], Ok("release")),
            ("release twice", &["--release", "--release"], Ok("release")),
            ("unknown flag", &["--device"], Err("Usage")),
        ];
        for (name, flags, want) in cases {
            let flags: Vec<String> = flags.iter().map(|s| s.to_string()).collect();
            let got = Profile::from_flags(&flags)
                .map(Profile::dir)
                .map_err(|e| e.variant());
            assert_eq!(got, want.map_err(String::from), "{name}");
        }
    }

    #[test]
    fn bin_table() {
        let target = |name: &str, kind: &str| MetaTarget {
            name: name.into(),
            crate_types: vec![kind.into()],
        };
        // (row name, targets, expected bin or the error variant)
        type Case = (
            &'static str,
            Vec<MetaTarget>,
            Result<&'static str, &'static str>,
        );
        let cases: Vec<Case> = vec![
            ("one bin", vec![target("hello", "bin")], Ok("hello")),
            (
                "bin next to a cdylib",
                vec![target("hello", "cdylib"), target("app", "bin")],
                Ok("app"),
            ),
            (
                "several, one named like the package",
                vec![target("tool", "bin"), target("hello", "bin")],
                Ok("hello"),
            ),
            ("no bin", vec![target("hello", "cdylib")], Err("NoBin")),
            (
                "several, none named like the package",
                vec![target("a", "bin"), target("b", "bin")],
                Err("SeveralBins"),
            ),
        ];
        for (name, targets, want) in cases {
            let package = MetaPackage {
                name: "hello".into(),
                version: "0.1.0".into(),
                manifest_path: "/app/Cargo.toml".into(),
                targets,
            };
            let got = package.bin().map_err(|e| e.variant());
            assert_eq!(got, want.map(String::from).map_err(String::from), "{name}");
        }
    }
}
