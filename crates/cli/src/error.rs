//! Every way `cargo parallax` fails, each naming what to set or install.

use std::path::PathBuf;

use crate::android::{ABIS, HOSTS};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(
        "usage:\n  cargo parallax run android [--release] [--device <serial>]\n  cargo parallax run ios [--release] [--simulator <name|udid> | --device [<name|udid>]]\n  cargo parallax build ios [--release]\n{0}"
    )]
    Usage(String),
    #[error(
        "cannot read {path}: {source}. Run cargo parallax in the app crate's directory, next to parallax.toml"
    )]
    ConfigMissing {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parallax.toml: {0}")]
    ConfigSyntax(String),
    #[error(
        "parallax.toml: app.id {0:?} is not an Android application id (two or more dot-separated segments, each a letter then letters, digits or _)"
    )]
    BadAppId(String),
    #[error(
        "parallax.toml: android.min_sdk {min_sdk} is below {lowest}, the lowest API Parallax supports"
    )]
    MinSdkTooLow { min_sdk: u32, lowest: u32 },
    #[error("parallax.toml: android.target_sdk {target_sdk} is below android.min_sdk {min_sdk}")]
    TargetBelowMin { min_sdk: u32, target_sdk: u32 },
    #[error("parallax.toml: android.abis is empty; list at least one ABI")]
    NoAbis,
    #[error("parallax.toml: unknown ABI {0:?}; supported: {supported}", supported = ABIS.map(|a| a.name).join(", "))]
    UnknownAbi(String),
    #[error("cannot read app.icon {path}: {source}. app.icon is relative to parallax.toml")]
    IconRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("app.icon {path} is not a usable SVG: {reason}")]
    IconInvalid { path: PathBuf, reason: String },
    #[error("{var} is not set. Set it to {what}")]
    EnvMissing {
        var: &'static str,
        what: &'static str,
    },
    #[error("{what} not found at {path}. {fix}")]
    PathMissing {
        what: &'static str,
        path: PathBuf,
        fix: &'static str,
    },
    #[error(
        "no NDK under {0}. Install one (sdkmanager \"ndk;27.1.12297006\") or set ANDROID_NDK_HOME"
    )]
    NdkMissing(PathBuf),
    #[error("several NDKs under {dir}: {versions:?}. Set ANDROID_NDK_HOME to the one to use")]
    NdkAmbiguous { dir: PathBuf, versions: Vec<String> },
    #[error("cargo parallax cannot build Android apps on host OS {0:?} (supported: {supported})", supported = HOSTS.map(|h| h.os).join(", "))]
    UnsupportedHost(&'static str),
    #[error("{tool} not found ({source}). {fix}")]
    ToolMissing {
        tool: String,
        fix: &'static str,
        source: std::io::Error,
    },
    #[error("{step} failed ({status}). {hint}")]
    StepFailed {
        step: &'static str,
        status: std::process::ExitStatus,
        hint: String,
    },
    #[error("cargo metadata output did not parse: {0}")]
    Metadata(String),
    #[error(
        "no package with manifest {0} in cargo metadata; run cargo parallax in the app crate's directory"
    )]
    NoAppPackage(PathBuf),
    #[error(
        "package {0} has no cdylib target. Add to its Cargo.toml: [lib] crate-type = [\"cdylib\"]"
    )]
    NotCdylib(String),
    #[error(
        "package {0} does not depend on parallax-android, which holds the Activity's Java sources. Add it to [dependencies]"
    )]
    NoParallaxAndroid(String),
    #[error(
        "no Android device online (adb devices). Start an emulator or plug in a device with USB debugging on"
    )]
    NoDevice,
    #[error("several Android devices online: {0:?}. Pick one with --device <serial>")]
    SeveralDevices(Vec<String>),
    #[error("device {serial:?} is not online; online: {online:?}")]
    DeviceNotFound { serial: String, online: Vec<String> },
    #[error(
        "parallax.toml: ios.min_ios {min_ios:?} is below {lowest}, the lowest iOS Parallax supports"
    )]
    MinIosTooLow { min_ios: String, lowest: u32 },
    #[error("parallax.toml: ios.min_ios {0:?} is not an iOS version such as \"17.0\"")]
    BadMinIos(String),
    #[error(
        "parallax.toml: ios.team {0:?} is not an Apple team id (ten upper-case letters and digits, shown under Membership details on developer.apple.com)"
    )]
    BadTeam(String),
    #[error(
        "app.id {0:?} is not an iOS bundle id (letters, digits, - and . only; no _). Pick an id that suits both Android and iOS"
    )]
    BadBundleId(String),
    #[error(
        "cargo parallax builds iOS apps on macOS only (this is {0:?}): Xcode is needed. Run it on a Mac"
    )]
    IosNeedsMacos(&'static str),
    #[error("cargo parallax has no iOS Rust target for a {0:?} Mac (RUST_TARGETS in ios.rs)")]
    UnsupportedIosHost(&'static str),
    #[error(
        "running on a device needs an Apple team id: add to parallax.toml\n[ios]\nteam = \"<your team id>\"\nXcode signs with the account signed in under Xcode → Settings → Accounts"
    )]
    TeamMissing,
    #[error(
        "package {0} has no bin target. iOS runs the app's binary: add src/main.rs (or a [[bin]]) that calls parallax::run"
    )]
    NoBin(String),
    #[error("package {package} has several bin targets {bins:?} and none named {package}")]
    SeveralBins { package: String, bins: Vec<String> },
    #[error("no {what}. {fix}; available: {available:?}")]
    NoIosTarget {
        what: &'static str,
        fix: &'static str,
        available: Vec<String>,
    },
    #[error("several {what}s: {names:?}. Pick one with {flag} <name|udid>")]
    SeveralIosTargets {
        what: &'static str,
        flag: &'static str,
        names: Vec<String>,
    },
    #[error("no {what} named or with udid {requested:?}; available: {available:?}")]
    IosTargetNotFound {
        what: &'static str,
        requested: String,
        available: Vec<String>,
    },
    #[error("{tool} output did not parse: {reason}")]
    ToolOutput { tool: &'static str, reason: String },
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[cfg(test)]
impl CliError {
    /// The variant's name, so test tables can name the error they expect.
    pub fn variant(&self) -> String {
        format!("{self:?}")
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .collect()
    }
}
