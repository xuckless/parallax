//! `parallax.toml`: the app's declarative build description. Reference: docs/cli.md.

use std::path::PathBuf;

use serde::Deserialize;

use crate::android::{ABIS, Abi};
use crate::error::CliError;

/// Lowest `min_sdk` Parallax supports (AGENTS.md, Minimum OS: Android API 28).
const LOWEST_MIN_SDK: u32 = 28;

/// Lowest `ios.min_ios` major version Parallax supports (AGENTS.md, Minimum OS: iOS 17).
const LOWEST_MIN_IOS: u32 = 17;

/// Every `[ios]` default. To add a key: add it here, to `IosToml` and to docs/cli-ios.md.
const IOS_DEFAULTS: IosDefaults = IosDefaults {
    min_ios: "17.0",
    // No team → simulator runs and unsigned device builds only.
    team: None,
};

struct IosDefaults {
    min_ios: &'static str,
    team: Option<&'static str>,
}

/// Every `parallax.toml` default, in one place. To add a key: add it here, to the `*Toml` struct and to docs/cli.md.
const ANDROID_DEFAULTS: AndroidDefaults = AndroidDefaults {
    min_sdk: 28,
    target_sdk: 35,
    abis: &["arm64-v8a", "x86_64"],
    // No app.icon → Android's default launcher icon.
    icon: None,
};

struct AndroidDefaults {
    min_sdk: u32,
    target_sdk: u32,
    abis: &'static [&'static str],
    icon: Option<&'static str>,
}

/// `parallax.toml` as written; `None` means "take the default".
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigToml {
    app: AppToml,
    #[serde(default)]
    android: AndroidToml,
    #[serde(default)]
    ios: IosToml,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppToml {
    name: String,
    id: String,
    icon: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct AndroidToml {
    min_sdk: Option<u32>,
    target_sdk: Option<u32>,
    abis: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct IosToml {
    min_ios: Option<String>,
    team: Option<String>,
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
    pub android: Android,
    pub ios: Ios,
}

#[derive(Debug, PartialEq)]
pub struct Ios {
    /// `IPHONEOS_DEPLOYMENT_TARGET`, e.g. `17.0`.
    pub min_ios: String,
    /// Apple Developer team id (`DEVELOPMENT_TEAM`). Xcode signs device builds for it with the account signed in to
    /// Xcode; `parallax.toml` holds only the id, never credentials.
    pub team: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Android {
    pub min_sdk: u32,
    /// Also the `compileSdk`.
    pub target_sdk: u32,
    pub abis: Vec<&'static Abi>,
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
        let min_ios = raw
            .ios
            .min_ios
            .unwrap_or_else(|| IOS_DEFAULTS.min_ios.to_string());
        match ios_major(&min_ios) {
            None => return Err(CliError::BadMinIos(min_ios)),
            Some(major) if major < LOWEST_MIN_IOS => {
                return Err(CliError::MinIosTooLow {
                    min_ios,
                    lowest: LOWEST_MIN_IOS,
                });
            }
            Some(_) => {}
        }
        let team = raw.ios.team.or(IOS_DEFAULTS.team.map(String::from));
        if let Some(team) = team.as_ref().filter(|t| !is_team_id(t)) {
            return Err(CliError::BadTeam(team.clone()));
        }
        Ok(Config {
            name: raw.app.name,
            id: raw.app.id,
            icon: raw
                .app
                .icon
                .as_deref()
                .or(ANDROID_DEFAULTS.icon)
                .map(PathBuf::from),
            android: Android {
                min_sdk,
                target_sdk,
                abis,
            },
            ios: Ios { min_ios, team },
        })
    }
}

/// The major version of `17`, `17.0` or `17.0.1`; `None` if the string is not an iOS version.
fn ios_major(version: &str) -> Option<u32> {
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = |p: &&str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
    match parts.len() <= 3 && parts.iter().all(numeric) {
        true => parts[0].parse().ok(),
        false => None,
    }
}

/// Apple team ids are ten upper-case letters and digits (developer.apple.com → Account → Membership details).
fn is_team_id(team: &str) -> bool {
    team.len() == 10
        && team
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
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
            android: Android {
                min_sdk,
                target_sdk,
                abis: abis.to_vec(),
            },
            ios: Ios {
                min_ios: "17.0".into(),
                team: None,
            },
        }
    }

    fn ios(min_ios: &str, team: Option<&str>) -> Config {
        Config {
            ios: Ios {
                min_ios: min_ios.into(),
                team: team.map(String::from),
            },
            ..config(28, 35, &[ARM64, X86_64])
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
                format!("{APP}[tvos]\n"),
                Err("ConfigSyntax"),
            ),
            (
                "empty [ios] takes the defaults",
                format!("{APP}[ios]\n"),
                Ok(ios("17.0", None)),
            ),
            (
                "ios team and min_ios set",
                format!("{APP}[ios]\nteam = \"ABCDE12345\"\nmin_ios = \"18.2\"\n"),
                Ok(ios("18.2", Some("ABCDE12345"))),
            ),
            (
                "min_ios major only",
                format!("{APP}[ios]\nmin_ios = \"26\"\n"),
                Ok(ios("26", None)),
            ),
            (
                "min_ios below 17",
                format!("{APP}[ios]\nmin_ios = \"16.4\"\n"),
                Err("MinIosTooLow"),
            ),
            (
                "min_ios not a version",
                format!("{APP}[ios]\nmin_ios = \"17.x\"\n"),
                Err("BadMinIos"),
            ),
            (
                "min_ios empty part",
                format!("{APP}[ios]\nmin_ios = \"17.\"\n"),
                Err("BadMinIos"),
            ),
            (
                "team lower case",
                format!("{APP}[ios]\nteam = \"abcde12345\"\n"),
                Err("BadTeam"),
            ),
            (
                "team too short",
                format!("{APP}[ios]\nteam = \"ABCDE1234\"\n"),
                Err("BadTeam"),
            ),
            (
                "unknown key in [ios]",
                format!("{APP}[ios]\nprofile = \"dev\"\n"),
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
