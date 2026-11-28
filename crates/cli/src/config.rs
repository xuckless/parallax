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
};

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
    web: WebToml,
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
struct WebToml {
    title: Option<String>,
    background: Option<String>,
    index: Option<String>,
    assets: Option<Vec<String>>,
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
    pub web: Web,
}

#[derive(Debug, PartialEq)]
pub struct Android {
    pub min_sdk: u32,
    /// Also the `compileSdk`.
    pub target_sdk: u32,
    pub abis: Vec<&'static Abi>,
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
            web,
        })
    }
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
            web: web("Hello", "#ffffff"),
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
