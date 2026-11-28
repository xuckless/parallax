//! Every way `cargo parallax` fails, each naming what to set or install.

use std::path::PathBuf;

use crate::android::{ABIS, HOSTS};

/// How to make an upload key; the command from Flutter's Android deployment guide.
const KEYTOOL_GENKEY: &str = "keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload";

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(
        "usage: cargo parallax run android [--release] [--device <serial>]\n       cargo parallax build android [--release] [--apk|--aab]\n{0}"
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
    #[error("parallax.toml: app.version is empty; set it, e.g. version = \"1.0.0\"")]
    EmptyVersion,
    #[error(
        "parallax.toml: android.version_code {version_code} is outside 1..={highest}, the range Google Play accepts"
    )]
    VersionCodeOutOfRange { version_code: u32, highest: u32 },
    #[error(
        "parallax.toml: [android.release] has no {0}. It needs both keystore (path relative to parallax.toml) and key_alias"
    )]
    ReleaseKeyIncomplete(&'static str),
    #[error(
        "release builds are signed with the app's own key, and parallax.toml has no [android.release]. Create a key with\n  {genkey}\nthen add\n  [android.release]\n  keystore = \"upload-keystore.jks\"\n  key_alias = \"upload\"\nKeep the keystore out of version control. Debug builds need no key: drop --release", genkey = KEYTOOL_GENKEY
    )]
    ReleaseNotConfigured,
    #[error(
        "keystore {0} not found. android.release.keystore is relative to parallax.toml; to create one:\n  {genkey}", genkey = KEYTOOL_GENKEY
    )]
    KeystoreMissing(PathBuf),
    #[error(
        "no keystore password: set PARALLAX_KEYSTORE_PASSWORD (and PARALLAX_KEY_PASSWORD if the key's password differs), or run in a terminal to be asked"
    )]
    NoPassword,
    #[error("the keystore password is empty")]
    EmptyPassword,
    #[error(
        "keystore {0}: the password is wrong. Check PARALLAX_KEYSTORE_PASSWORD, or unset it to be asked"
    )]
    WrongKeystorePassword(PathBuf),
    #[error(
        "keystore {keystore} has no key {alias:?}. Fix android.release.key_alias; list the aliases with: keytool -list -keystore {keystore}"
    )]
    AliasNotInKeystore { alias: String, keystore: PathBuf },
    #[error("keytool cannot read keystore {keystore}: {message}")]
    KeystoreUnreadable { keystore: PathBuf, message: String },
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
