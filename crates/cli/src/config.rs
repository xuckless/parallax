//! `parallax.toml`: the app's declarative build description. Reference: docs/cli.md; per-target tables in
//! docs/cli-android-release.md, docs/cli-ios.md, docs/cli-web.md and docs/cli-desktop.md.

use std::path::PathBuf;

use serde::Deserialize;

use crate::android::{ABIS, Abi};
use crate::error::CliError;
use crate::macos::{ARCHS, Arch};

/// Lowest `min_sdk` Parallax supports (AGENTS.md, Minimum OS: Android API 28).
const LOWEST_MIN_SDK: u32 = 28;

/// Lowest `ios.min_ios` major version Parallax supports (AGENTS.md, Minimum OS: iOS 17).
const LOWEST_MIN_IOS: u32 = 17;

/// Highest `versionCode` Google Play accepts.
const HIGHEST_VERSION_CODE: u32 = 2_100_000_000;

/// `[app]` defaults. To add a key: add it here, to `AppToml`, `Config` and docs/cli.md.
const APP_DEFAULTS: AppDefaults = AppDefaults {
    // No app.icon → each platform's default icon.
    icon: None,
    // No app.version → the Cargo package version (`Config::version`).
    version: None,
};

struct AppDefaults {
    icon: Option<&'static str>,
    version: Option<&'static str>,
}

/// `[android]` defaults. To add a key: add it here, to `AndroidToml`, `Android` and docs/cli-android.md.
const ANDROID_DEFAULTS: AndroidDefaults = AndroidDefaults {
    min_sdk: 28,
    target_sdk: 35,
    abis: &["arm64-v8a", "x86_64"],
    // Flutter's Gradle plugin's fallback when the app sets none (flutter_tools FlutterPlugin.kt @ 3.50.1).
    version_code: 1,
};

struct AndroidDefaults {
    min_sdk: u32,
    target_sdk: u32,
    abis: &'static [&'static str],
    version_code: u32,
}

/// `[ios]` defaults. To add a key: add it here, to `IosToml`, `Ios` and docs/cli-ios.md.
const IOS_DEFAULTS: IosDefaults = IosDefaults {
    min_ios: "17.0",
    // No team → simulator runs and unsigned device builds only.
    team: None,
};

struct IosDefaults {
    min_ios: &'static str,
    team: Option<&'static str>,
}

/// `[web]` defaults. To add a key: add it here, to `WebToml`, `Web` and docs/cli-web.md.
const WEB_DEFAULTS: WebDefaults = WebDefaults {
    // No web.title → app.name.
    title: None,
    // The page behind the canvas until GPUI's first frame, and the manifest's colours.
    background: "#ffffff",
    // No web.index → cargo-parallax's templates/web/index.html.
    index: None,
    assets: &[],
};

struct WebDefaults {
    title: Option<&'static str>,
    background: &'static str,
    index: Option<&'static str>,
    assets: &'static [&'static str],
}

/// `[macos]` defaults. To add a key: add it here, to `MacosToml`, `Macos` and docs/cli-desktop.md.
const MACOS_DEFAULTS: MacosDefaults = MacosDefaults {
    // AppKit's emulation (tap → click, pan → scroll) unless the app opts into raw touches
    // (docs/desktop-touch-macos-emulation.md).
    touch_native: false,
    archs: &["arm64"],
    // Ad-hoc signature, so the bundle launches without Gatekeeper's "damaged" prompt.
    codesign: true,
};

struct MacosDefaults {
    touch_native: bool,
    archs: &'static [&'static str],
    codesign: bool,
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
    #[serde(default)]
    web: WebToml,
    #[serde(default)]
    macos: MacosToml,
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

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct IosToml {
    min_ios: Option<String>,
    team: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WebToml {
    title: Option<String>,
    background: Option<String>,
    index: Option<String>,
    assets: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct MacosToml {
    touch_native: Option<bool>,
    archs: Option<Vec<String>>,
    codesign: Option<bool>,
}

/// `parallax.toml` with defaults filled in and every value checked.
#[derive(Debug, PartialEq)]
pub struct Config {
    /// Label shown under the launcher icon.
    pub name: String,
    /// Android application id (also the Gradle namespace) and Apple bundle id.
    pub id: String,
    /// App icon SVG, relative to `parallax.toml`.
    pub icon: Option<PathBuf>,
    /// `app.version`; `None` → the Cargo package version (`Config::version`).
    pub version: Option<String>,
    pub android: Android,
    pub ios: Ios,
    pub web: Web,
    pub macos: Macos,
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

#[derive(Debug, PartialEq)]
pub struct Ios {
    /// `IPHONEOS_DEPLOYMENT_TARGET`, e.g. `17.0`.
    pub min_ios: String,
    /// Apple Developer team id (`DEVELOPMENT_TEAM`). Xcode signs device builds for it with the account signed in to
    /// Xcode; `parallax.toml` holds only the id, never credentials.
    pub team: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Web {
    /// The page's `<title>` and the web app manifest's name.
    pub title: String,
    /// `#rgb` or `#rrggbb`.
    pub background: String,
    /// The app's own host page, relative to `parallax.toml`, in place of the template.
    pub index: Option<PathBuf>,
    /// Files copied next to `index.html`, relative to `parallax.toml`.
    pub assets: Vec<PathBuf>,
}

#[derive(Debug, PartialEq)]
pub struct Macos {
    /// `NSIsTouchNative` in Info.plist: touches reach GPUI only as `TouchEvent`s, AppKit makes no mouse copies.
    pub touch_native: bool,
    /// One arch is copied into the bundle; several are joined with `lipo` into a universal binary.
    pub archs: Vec<&'static Arch>,
    /// Ad-hoc `codesign --sign -` of the finished bundle.
    pub codesign: bool,
}

/// Names from `parallax.toml` (else `defaults`) looked up in a target's table by `name`; an empty list is `empty`.
fn lookup<T>(
    names: Option<Vec<String>>,
    defaults: &[&str],
    table: &'static [T],
    name: fn(&T) -> &str,
    empty: CliError,
    unknown: fn(String) -> CliError,
) -> Result<Vec<&'static T>, CliError> {
    let names = names.unwrap_or_else(|| defaults.iter().map(|s| s.to_string()).collect());
    if names.is_empty() {
        return Err(empty);
    }
    names
        .into_iter()
        .map(|n| table.iter().find(|row| name(row) == n).ok_or(unknown(n)))
        .collect()
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, CliError> {
        let raw: ConfigToml =
            toml::from_str(text).map_err(|e| CliError::ConfigSyntax(e.to_string()))?;
        if !is_application_id(&raw.app.id) {
            return Err(CliError::BadAppId(raw.app.id));
        }
        let version = raw.app.version.or(APP_DEFAULTS.version.map(String::from));
        if version.as_deref() == Some("") {
            return Err(CliError::EmptyVersion);
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
        let abis = lookup(
            raw.android.abis,
            ANDROID_DEFAULTS.abis,
            &ABIS,
            |a| a.name,
            CliError::NoAbis,
            CliError::UnknownAbi,
        )?;
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

        let background = raw
            .web
            .background
            .unwrap_or_else(|| WEB_DEFAULTS.background.into());
        if !is_hex_colour(&background) {
            return Err(CliError::BadColour(background));
        }
        let web = Web {
            title: raw
                .web
                .title
                .or(WEB_DEFAULTS.title.map(String::from))
                .unwrap_or_else(|| raw.app.name.clone()),
            background,
            index: raw
                .web
                .index
                .as_deref()
                .or(WEB_DEFAULTS.index)
                .map(PathBuf::from),
            assets: raw
                .web
                .assets
                .unwrap_or_else(|| WEB_DEFAULTS.assets.iter().map(|s| s.to_string()).collect())
                .into_iter()
                .map(PathBuf::from)
                .collect(),
        };

        let archs = lookup(
            raw.macos.archs,
            MACOS_DEFAULTS.archs,
            &ARCHS,
            |a| a.name,
            CliError::NoArchs,
            CliError::UnknownArch,
        )?;

        Ok(Config {
            name: raw.app.name,
            id: raw.app.id,
            icon: raw
                .app
                .icon
                .as_deref()
                .or(APP_DEFAULTS.icon)
                .map(PathBuf::from),
            version,
            android: Android {
                min_sdk,
                target_sdk,
                abis,
                version_code,
                release,
            },
            ios: Ios { min_ios, team },
            web,
            macos: Macos {
                touch_native: raw
                    .macos
                    .touch_native
                    .unwrap_or(MACOS_DEFAULTS.touch_native),
                archs,
                codesign: raw.macos.codesign.unwrap_or(MACOS_DEFAULTS.codesign),
            },
        })
    }

    /// `app.version`, else `package_version` (the Cargo package's): what every platform's version fields come from
    /// (version.rs).
    pub fn version<'a>(&'a self, package_version: &'a str) -> &'a str {
        self.version.as_deref().unwrap_or(package_version)
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

/// CSS hex colour: `#` then 3 or 6 hex digits (it lands in CSS and the manifest unescaped).
fn is_hex_colour(colour: &str) -> bool {
    colour.strip_prefix('#').is_some_and(|hex| {
        [3, 6].contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit())
    })
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
    const ARM64_MAC: &Arch = &ARCHS[0];
    const X86_64_MAC: &Arch = &ARCHS[1];

    fn config(min_sdk: u32, target_sdk: u32, abis: &[&'static Abi]) -> Config {
        Config {
            name: "Hello".into(),
            id: "io.github.xuckless.parallax.hello".into(),
            icon: None,
            version: None,
            android: Android {
                min_sdk,
                target_sdk,
                abis: abis.to_vec(),
                version_code: 1,
                release: None,
            },
            ios: Ios {
                min_ios: "17.0".into(),
                team: None,
            },
            web: web("Hello", "#ffffff"),
            macos: macos(false, &[ARM64_MAC], true),
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

    fn web(title: &str, background: &str) -> Web {
        Web {
            title: title.into(),
            background: background.into(),
            index: None,
            assets: Vec::new(),
        }
    }

    fn macos(touch_native: bool, archs: &[&'static Arch], codesign: bool) -> Macos {
        Macos {
            touch_native,
            archs: archs.to_vec(),
            codesign,
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
                "every [web] key set",
                format!(
                    "{APP}[web]\ntitle = \"Hi\"\nbackground = \"#102030\"\nindex = \"web/index.html\"\nassets = [\"frame.html\"]\n"
                ),
                Ok(Config {
                    web: Web {
                        index: Some("web/index.html".into()),
                        assets: vec!["frame.html".into()],
                        ..web("Hi", "#102030")
                    },
                    ..config(28, 35, &[ARM64, X86_64])
                }),
            ),
            (
                "short web.background",
                format!("{APP}[web]\nbackground = \"#fff\"\n"),
                Ok(Config {
                    web: web("Hello", "#fff"),
                    ..config(28, 35, &[ARM64, X86_64])
                }),
            ),
            (
                "web.background without #",
                format!("{APP}[web]\nbackground = \"ffffff\"\n"),
                Err("BadColour"),
            ),
            (
                "web.background a CSS name",
                format!("{APP}[web]\nbackground = \"red\"\n"),
                Err("BadColour"),
            ),
            (
                "web.background four digits",
                format!("{APP}[web]\nbackground = \"#ffff\"\n"),
                Err("BadColour"),
            ),
            (
                "unknown key in [web]",
                format!("{APP}[web]\nport = 8080\n"),
                Err("ConfigSyntax"),
            ),
            (
                "[macos] every key set",
                format!(
                    "{APP}[macos]\ntouch_native = true\narchs = [\"arm64\", \"x86_64\"]\ncodesign = false\n"
                ),
                Ok(Config {
                    macos: macos(true, &[ARM64_MAC, X86_64_MAC], false),
                    ..config(28, 35, &[ARM64, X86_64])
                }),
            ),
            (
                "[macos] touch_native only, rest default",
                format!("{APP}[macos]\ntouch_native = true\n"),
                Ok(Config {
                    macos: macos(true, &[ARM64_MAC], true),
                    ..config(28, 35, &[ARM64, X86_64])
                }),
            ),
            (
                "[macos] unknown key",
                format!("{APP}[macos]\nsign = true\n"),
                Err("ConfigSyntax"),
            ),
            (
                "[macos] touch_native not a bool",
                format!("{APP}[macos]\ntouch_native = \"yes\"\n"),
                Err("ConfigSyntax"),
            ),
            (
                "[macos] empty archs",
                format!("{APP}[macos]\narchs = []\n"),
                Err("NoArchs"),
            ),
            (
                "[macos] unknown arch",
                format!("{APP}[macos]\narchs = [\"aarch64\"]\n"),
                Err("UnknownArch"),
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
                    version: Some("2.3.1".into()),
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
