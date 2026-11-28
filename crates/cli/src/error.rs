//! Every way `cargo parallax` fails, each naming what to set or install.

use std::path::PathBuf;

use crate::android::{ABIS, HOSTS};

/// Every command and its flags, one line each (the rows of `COMMANDS` in main.rs).
const USAGE: &str = "cargo parallax run android [--release] [--device <serial>]
       cargo parallax run web [--release] [--port <n>] [--host <address>] [--open]
       cargo parallax build web [--release]";

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("usage: {USAGE}\n{0}")]
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
    #[error("parallax.toml: web.background {0:?} is not a CSS hex colour (#rgb or #rrggbb)")]
    BadColour(String),
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
        "{} wasm-bindgen {want} (the wasm-bindgen crate in Cargo.lock). Install that version: cargo install wasm-bindgen-cli --version {want}",
        match have { Some(have) => format!("wasm-bindgen on PATH is {have}, not"), None => "wasm-bindgen is not on PATH; the app needs".into() }
    )]
    WasmBindgen { have: Option<String>, want: String },
    #[error(
        "package {0} does not depend on wasm-bindgen. Add to its Cargo.toml: [target.'cfg(target_family = \"wasm\")'.dependencies] wasm-bindgen = \"0.2\", and a #[wasm_bindgen(start)] function"
    )]
    NoWasmBindgen(String),
    #[error(
        "several wasm-bindgen versions in Cargo.lock: {0:?}. Make them one (cargo update -p wasm-bindgen), so one wasm-bindgen-cli fits"
    )]
    SeveralWasmBindgens(Vec<String>),
    #[error("parallax.toml: web.assets entry {0} names no file")]
    AssetName(PathBuf),
    #[error(
        "parallax.toml: web.assets file {0:?} has the name of a file cargo parallax writes (index.html: use web.index)"
    )]
    AssetClash(String),
    #[error("cannot serve on {addr}: {source}. Pick another port with --port <n>")]
    Bind {
        addr: std::net::SocketAddr,
        source: std::io::Error,
    },
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
