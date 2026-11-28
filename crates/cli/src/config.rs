//! `parallax.toml`: the app's declarative build description. Reference: docs/cli.md.

use std::path::PathBuf;

use serde::Deserialize;

use crate::android::{ABIS, Abi};
use crate::error::CliError;

/// Lowest `min_sdk` Parallax supports (AGENTS.md, Minimum OS: Android API 28).
const LOWEST_MIN_SDK: u32 = 28;

/// Every `parallax.toml` default, in one place. To add a key: add it here, to the `*Toml` struct and to docs/cli.md.
const ANDROID_DEFAULTS: AndroidDefaults = AndroidDefaults {
    min_sdk: 28,
    target_sdk: 35,
    abis: &["arm64-v8a", "x86_64"],
    // No app.icon → Android's default launcher icon.
    icon: None,
    // Flutter's Gradle plugin's fallbacks when the app sets no version (flutter_tools FlutterPlugin.kt @ 3.50.1).
    version: "1.0",
    version_code: 1,
};

struct AndroidDefaults {
    min_sdk: u32,
    target_sdk: u32,
    abis: &'static [&'static str],
    icon: Option<&'static str>,
    version: &'static str,
    version_code: u32,
}

/// Highest `versionCode` Google Play accepts.
const HIGHEST_VERSION_CODE: u32 = 2_100_000_000;

/// `parallax.toml` as written; `None` means "take the default".
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigToml {
    app: AppToml,
    #[serde(default)]
    android: AndroidToml,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppToml {
    name: String,
    id: String,
    icon: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct AndroidToml {
    min_sdk: Option<u32>,
    target_sdk: Option<u32>,
    abis: Option<Vec<String>>,
    version_code: Option<u32>,
    release: Option<ReleaseToml>,
}

/// `[android.release]`: the upload key. Passwords never go here (docs/cli-android-release.md).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseToml {
    keystore: Option<String>,
    key_alias: Option<String>,
}

/// `parallax.toml` with defaults filled in and every value checked.
#[derive(Debug, PartialEq)]
pub struct Config {
    /// Label shown under the launcher icon.
    pub name: String,
    /// Android application id (also the Gradle namespace).
    pub id: String,
    /// Launcher icon SVG, relative to `parallax.toml`.
    pub icon: Option<PathBuf>,
    /// User-visible version (Android `versionName`).
    pub version: String,
    pub android: Android,
}

#[derive(Debug, PartialEq)]
pub struct Android {
    pub min_sdk: u32,
    /// Also the `compileSdk`.
    pub target_sdk: u32,
    pub abis: Vec<&'static Abi>,
    pub version_code: u32,
    /// The key release builds are signed with; `None` until `[android.release]` is set.
    pub release: Option<ReleaseKey>,
}

#[derive(Debug, PartialEq)]
pub struct ReleaseKey {
    /// Keystore file, relative to `parallax.toml`.
    pub keystore: PathBuf,
    pub key_alias: String,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, CliError> {
        let raw: ConfigToml =
            toml::from_str(text).map_err(|e| CliError::ConfigSyntax(e.to_string()))?;
        if !is_application_id(&raw.app.id) {
            return Err(CliError::BadAppId(raw.app.id));
        }
        let min_sdk = raw.android.min_sdk.unwrap_or(ANDROID_DEFAULTS.min_sdk);
        let target_sdk = raw
            .android
            .target_sdk
            .unwrap_or(ANDROID_DEFAULTS.target_sdk);
        if min_sdk < LOWEST_MIN_SDK {
            return Err(CliError::MinSdkTooLow {
                min_sdk,
                lowest: LOWEST_MIN_SDK,
            });
        }
        if target_sdk < min_sdk {
            return Err(CliError::TargetBelowMin {
                min_sdk,
                target_sdk,
            });
        }
        let names = raw.android.abis.unwrap_or_else(|| {
            ANDROID_DEFAULTS
                .abis
                .iter()
                .map(|s| s.to_string())
                .collect()
        });
        if names.is_empty() {
            return Err(CliError::NoAbis);
        }
        let abis = names
            .iter()
            .map(|n| {
                ABIS.iter()
                    .find(|a| a.name == n)
                    .ok_or_else(|| CliError::UnknownAbi(n.clone()))
            })
            .collect::<Result<_, _>>()?;
        let version = raw
            .app
            .version
            .unwrap_or_else(|| ANDROID_DEFAULTS.version.to_string());
        if version.is_empty() {
            return Err(CliError::EmptyVersion);
        }
        let version_code = raw
            .android
            .version_code
            .unwrap_or(ANDROID_DEFAULTS.version_code);
        if !(1..=HIGHEST_VERSION_CODE).contains(&version_code) {
            return Err(CliError::VersionCodeOutOfRange {
                version_code,
                highest: HIGHEST_VERSION_CODE,
            });
        }
        let release = match raw.android.release {
            None => None,
            Some(release) => Some(ReleaseKey {
                keystore: PathBuf::from(
                    release
                        .keystore
                        .ok_or(CliError::ReleaseKeyIncomplete("keystore"))?,
                ),
                key_alias: release
                    .key_alias
                    .ok_or(CliError::ReleaseKeyIncomplete("key_alias"))?,
            }),
        };
        Ok(Config {
            name: raw.app.name,
            id: raw.app.id,
            icon: raw
                .app
                .icon
                .as_deref()
                .or(ANDROID_DEFAULTS.icon)
                .map(PathBuf::from),
            version,
            android: Android {
                min_sdk,
                target_sdk,
                abis,
                version_code,
                release,
            },
        })
    }
}

/// Android's rule: two or more dot-separated segments, each a letter followed by letters, digits or `_`.
fn is_application_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|s| {
            s.starts_with(|c: char| c.is_ascii_alphabetic())
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARM64: &Abi = &ABIS[0];
    const ARMV7: &Abi = &ABIS[1];
    const X86: &Abi = &ABIS[2];
    const X86_64: &Abi = &ABIS[3];

    fn config(min_sdk: u32, target_sdk: u32, abis: &[&'static Abi]) -> Config {
        Config {
            name: "Hello".into(),
            id: "io.github.xuckless.parallax.hello".into(),
            icon: None,
            version: "1.0".into(),
            android: Android {
                min_sdk,
                target_sdk,
                abis: abis.to_vec(),
                version_code: 1,
                release: None,
            },
        }
    }

    const APP: &str = "[app]\nname = \"Hello\"\nid = \"io.github.xuckless.parallax.hello\"\n";

    #[test]
    fn parse_table() {
        let cases: Vec<(&str, String, Result<Config, &str>)> = vec![
            (
                "defaults when [android] is absent",
                APP.into(),
                Ok(config(28, 35, &[ARM64, X86_64])),
            ),
            (
                "every key set",
                format!(
                    "{APP}[android]\nmin_sdk = 30\ntarget_sdk = 36\nabis = [\"x86_64\", \"armeabi-v7a\", \"x86\"]\n"
                ),
                Ok(config(30, 36, &[X86_64, ARMV7, X86])),
            ),
            (
                "one key set, rest default",
                format!("{APP}[android]\ntarget_sdk = 36\n"),
                Ok(config(28, 36, &[ARM64, X86_64])),
            ),
            (
                "icon set",
                format!("{APP}icon = \"../../assets/icon.svg\"\n"),
                Ok(Config {
                    icon: Some("../../assets/icon.svg".into()),
                    ..config(28, 35, &[ARM64, X86_64])
                }),
            ),
            (
                "icon not a string",
                format!("{APP}icon = 1\n"),
                Err("ConfigSyntax"),
            ),
            (
                "missing [app]",
                "[android]\nmin_sdk = 28\n".into(),
                Err("ConfigSyntax"),
            ),
            (
                "missing app.id",
                "[app]\nname = \"Hello\"\n".into(),
                Err("ConfigSyntax"),
            ),
            (
                "unknown key in [app]",
                format!("{APP}lib = \"hello\"\n"),
                Err("ConfigSyntax"),
            ),
            (
                "unknown key in [android]",
                format!("{APP}[android]\nmin_api = 28\n"),
                Err("ConfigSyntax"),
            ),
            (
                "unknown table",
                format!("{APP}[ios]\n"),
                Err("ConfigSyntax"),
            ),
            (
                "wrong type",
                format!("{APP}[android]\nmin_sdk = \"28\"\n"),
                Err("ConfigSyntax"),
            ),
            (
                "one-segment id",
                "[app]\nname = \"Hello\"\nid = \"hello\"\n".into(),
                Err("BadAppId"),
            ),
            (
                "id segment starts with digit",
                "[app]\nname = \"H\"\nid = \"io.1hello\"\n".into(),
                Err("BadAppId"),
            ),
            (
                "id with dash",
                "[app]\nname = \"H\"\nid = \"io.hello-world\"\n".into(),
                Err("BadAppId"),
            ),
            (
                "min_sdk below 28",
                format!("{APP}[android]\nmin_sdk = 27\n"),
                Err("MinSdkTooLow"),
            ),
            (
                "target below min",
                format!("{APP}[android]\nmin_sdk = 30\ntarget_sdk = 29\n"),
                Err("TargetBelowMin"),
            ),
            (
                "empty abis",
                format!("{APP}[android]\nabis = []\n"),
                Err("NoAbis"),
            ),
            (
                "version and version_code",
                format!("{APP}version = \"2.3.1\"\n[android]\nversion_code = 42\n"),
                Ok(Config {
                    version: "2.3.1".into(),
                    android: Android {
                        version_code: 42,
                        ..config(28, 35, &[ARM64, X86_64]).android
                    },
                    ..config(28, 35, &[ARM64, X86_64])
                }),
            ),
            (
                "empty version",
                format!("{APP}version = \"\"\n"),
                Err("EmptyVersion"),
            ),
            (
                "version_code 0",
                format!("{APP}[android]\nversion_code = 0\n"),
                Err("VersionCodeOutOfRange"),
            ),
            (
                "version_code above Play's limit",
                format!("{APP}[android]\nversion_code = 2100000001\n"),
                Err("VersionCodeOutOfRange"),
            ),
            (
                "release key",
                format!(
                    "{APP}[android.release]\nkeystore = \"keys/upload.jks\"\nkey_alias = \"upload\"\n"
                ),
                Ok(Config {
                    android: Android {
                        release: Some(ReleaseKey {
                            keystore: "keys/upload.jks".into(),
                            key_alias: "upload".into(),
                        }),
                        ..config(28, 35, &[ARM64, X86_64]).android
                    },
                    ..config(28, 35, &[ARM64, X86_64])
                }),
            ),
            (
                "release key without alias",
                format!("{APP}[android.release]\nkeystore = \"upload.jks\"\n"),
                Err("ReleaseKeyIncomplete"),
            ),
            (
                "release key without keystore",
                format!("{APP}[android.release]\nkey_alias = \"upload\"\n"),
                Err("ReleaseKeyIncomplete"),
            ),
            (
                "password in parallax.toml is rejected",
                format!(
                    "{APP}[android.release]\nkeystore = \"k.jks\"\nkey_alias = \"u\"\npassword = \"x\"\n"
                ),
                Err("ConfigSyntax"),
            ),
            (
                "unknown abi",
                format!("{APP}[android]\nabis = [\"mips\"]\n"),
                Err("UnknownAbi"),
            ),
        ];
        for (name, text, want) in cases {
            let got = Config::parse(&text).map_err(|e| e.variant());
            assert_eq!(got, want.map_err(String::from), "{name}");
        }
    }
}
