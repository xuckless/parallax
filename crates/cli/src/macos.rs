//! `cargo parallax run|build macos [--release]`: the app crate's bin built per `macos.archs`, wrapped in a real
//! `.app` (Info.plist, AppIcon.icns, ad-hoc signature), then run from inside the bundle so AppKit reads its Info.plist.
//! docs/cli-desktop.md. Decisions are pure functions over plain values; `bundle` is the thin shell.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cargo::{self, CARGO_FIX, Profile, step};
use crate::config::Config;
use crate::error::CliError;
use crate::template::{self, render, write, xml_escape};
use crate::{icon, version};

/// One macOS CPU architecture.
#[derive(Debug, PartialEq)]
pub struct Arch {
    /// Name in `parallax.toml`, as `lipo` and Apple name it.
    pub name: &'static str,
    pub rust_target: &'static str,
}

/// Arch → Rust target. To add one: add a row (and `rustup target add` its Rust target).
pub const ARCHS: [Arch; 2] = [
    Arch {
        name: "arm64",
        rust_target: "aarch64-apple-darwin",
    },
    Arch {
        name: "x86_64",
        rust_target: "x86_64-apple-darwin",
    },
];

/// GPUI's minimum macOS: gpui_apple's build.rs compiles its Metal shaders with `-mmacosx-version-min=12.0`.
const MIN_MACOS: &str = "12.0";

const INFO_PLIST: &str = include_str!("../templates/macos/Info.plist");

pub fn build(app_dir: &Path, config: &Config, flags: &[String]) -> Result<(), CliError> {
    bundle(app_dir, config, Profile::from_flags(flags)?).map(|_| ())
}

pub fn run(app_dir: &Path, config: &Config, flags: &[String]) -> Result<(), CliError> {
    let executable = bundle(app_dir, config, Profile::from_flags(flags)?)?;
    // Run from inside the bundle (not `open`), so the logs stream here and Ctrl-C stops the app.
    println!("cargo parallax: running {}", executable.display());
    step(
        &mut Command::new(&executable),
        "Run cargo parallax build macos and check the bundle",
        "the app",
        String::new(),
    )
}

/// Builds and assembles the bundle; returns the executable inside it.
fn bundle(app_dir: &Path, config: &Config, profile: Profile) -> Result<PathBuf, CliError> {
    // Rendered before the long build so a bad SVG fails fast.
    let icon = match &config.icon {
        Some(svg) => Some(icon::icns(&app_dir.join(svg))?),
        None => None,
    };
    let manifest = app_dir.join("Cargo.toml");
    let metadata = cargo::metadata(&manifest)?;
    let package = metadata.app_package(&manifest)?;
    let bin = package.bin()?;
    let bin = bin.as_str();

    let archs = &config.macos.archs;
    let mut build = Command::new(cargo::cargo());
    build
        .args(["build", "--bin", bin, "--manifest-path"])
        .arg(&manifest)
        .args(profile.cargo_args());
    for arch in archs {
        build.args(["--target", arch.rust_target]);
    }
    let targets: Vec<&str> = archs.iter().map(|a| a.rust_target).collect();
    step(
        &mut build,
        CARGO_FIX,
        "cargo build",
        format!(
            "If a target is missing: rustup target add {}",
            targets.join(" ")
        ),
    )?;

    let app = metadata
        .target_directory
        .join("parallax/macos")
        .join(&package.name)
        .join(format!("{}.app", config.name));
    template::clear(&app)?;
    let contents = app.join("Contents");
    let plist = info_plist(
        config,
        bin,
        config.version(&package.version),
        icon.is_some(),
    );
    write(&contents.join("Info.plist"), plist.as_bytes())?;
    if let Some(icns) = &icon {
        write(&contents.join("Resources/AppIcon.icns"), icns)?;
    }
    let macos_dir = contents.join("MacOS");
    std::fs::create_dir_all(&macos_dir).map_err(|source| CliError::Io {
        path: macos_dir.clone(),
        source,
    })?;
    let executable = macos_dir.join(bin);
    let built: Vec<PathBuf> = archs
        .iter()
        .map(|a| {
            metadata
                .target_directory
                .join(a.rust_target)
                .join(profile.dir())
                .join(bin)
        })
        .collect();
    match built.as_slice() {
        [one] => std::fs::copy(one, &executable)
            .map(|_| ())
            .map_err(|source| CliError::Io {
                path: one.clone(),
                source,
            })?,
        several => step(
            Command::new("lipo")
                .arg("-create")
                .args(several)
                .arg("-output")
                .arg(&executable),
            XCODE_FIX,
            "lipo -create",
            String::new(),
        )?,
    }
    if config.macos.codesign {
        step(
            Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(&app),
            XCODE_FIX,
            "codesign --sign -",
            "Or set macos.codesign = false in parallax.toml".into(),
        )?;
    }
    println!("cargo parallax: bundle at {}", app.display());
    Ok(executable)
}

const XCODE_FIX: &str = "Install the Xcode command line tools: xcode-select --install";

/// Info.plist from the template; every value XML-escaped.
fn info_plist(config: &Config, executable: &str, version: &str, has_icon: bool) -> String {
    let (build_name, build_number) = version::apple(version);
    let vars = [
        ("executable", xml_escape(executable)),
        ("icon_file", if has_icon { "AppIcon" } else { "" }.into()),
        ("id", xml_escape(&config.id)),
        ("name", xml_escape(&config.name)),
        ("build_name", build_name),
        ("build_number", build_number),
        ("min_macos", MIN_MACOS.into()),
        (
            "touch_native",
            if config.macos.touch_native {
                "<true/>"
            } else {
                "<false/>"
            }
            .into(),
        ),
    ];
    render(INFO_PLIST, &vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_table() {
        // (parallax.toml name, Rust target): `lipo -archs` names.
        let cases = [
            ("arm64", "aarch64-apple-darwin"),
            ("x86_64", "x86_64-apple-darwin"),
        ];
        assert_eq!(ARCHS.len(), cases.len(), "every arch row has a case");
        for (arch, (name, target)) in ARCHS.iter().zip(cases) {
            assert_eq!((arch.name, arch.rust_target), (name, target));
        }
    }

    fn config(name: &str, touch_native: bool) -> Config {
        let mut config =
            Config::parse("[app]\nname = \"x\"\nid = \"io.github.xuckless.parallax.hello\"\n")
                .expect("test parallax.toml");
        config.name = name.into();
        config.macos.touch_native = touch_native;
        config
    }

    #[test]
    fn info_plist_table() {
        // (row name, app.name, touch_native, has icon, strings the plist must hold)
        type Case<'a> = (&'a str, &'a str, bool, bool, &'a [&'a str]);
        let cases: [Case; 4] = [
            (
                "defaults: emulation, no icon",
                "Hello",
                false,
                false,
                &[
                    "<key>CFBundleExecutable</key>\n\t<string>hello</string>",
                    "<key>CFBundleIdentifier</key>\n\t<string>io.github.xuckless.parallax.hello</string>",
                    "<key>CFBundleName</key>\n\t<string>Hello</string>",
                    "<key>CFBundleIconFile</key>\n\t<string></string>",
                    "<key>CFBundleShortVersionString</key>\n\t<string>0.1.10</string>",
                    "<key>LSMinimumSystemVersion</key>\n\t<string>12.0</string>",
                    "<key>NSHighResolutionCapable</key>\n\t<true/>",
                    "<key>NSIsTouchNative</key>\n\t<false/>",
                ],
            ),
            (
                "touch_native = true",
                "Hello",
                true,
                false,
                &["<key>NSIsTouchNative</key>\n\t<true/>"],
            ),
            (
                "icon set",
                "Hello",
                false,
                true,
                &["<key>CFBundleIconFile</key>\n\t<string>AppIcon</string>"],
            ),
            (
                "name escaped",
                "Tom & Jerry",
                false,
                false,
                &["<string>Tom &amp; Jerry</string>"],
            ),
        ];
        for (name, app_name, touch_native, has_icon, wants) in cases {
            let plist = info_plist(&config(app_name, touch_native), "hello", "0.1.10", has_icon);
            assert!(!plist.contains("{{"), "{name}: placeholder left");
            for want in wants {
                assert!(plist.contains(want), "{name}: no {want:?} in\n{plist}");
            }
        }
    }
}
