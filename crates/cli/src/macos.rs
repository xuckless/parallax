//! `cargo parallax run|build macos`: the app crate's bin built per `macos.archs`, wrapped in a real `.app` (Info.plist,
//! AppIcon.icns, ad-hoc signature), then run from inside the bundle so AppKit reads its Info.plist. docs/cli-desktop.md.
//! Decisions are pure functions over plain values; `bundle` is the thin shell.
//!
//! `ICONSET` and `bundle_versions` (with its validators and their tests) are ported from flutter/flutter
//! packages/flutter_tools (paths at each item) @ 3.50.1. Copyright 2014 The Flutter Authors. BSD-3-Clause, see
//! third_party/flutter/LICENSE.

use std::path::{Path, PathBuf};
use std::process::Command;

use icns::{IconFamily, IconType, Image, PixelFormat};

use crate::android::{CARGO_FIX, Metadata, RunArgs, capture, render, step, write, xml_escape};
use crate::config::Config;
use crate::error::CliError;
use crate::icon::Svg;

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

/// The iconset of flutter_tools' `macos.tmpl/Runner/Assets.xcassets/AppIcon.appiconset/Contents.json` @ 3.50.1
/// (16 to 512 pt at 1x and 2x) as the icns elements `iconutil` packs it into; each renders at its pixel size. To add a
/// size: add a row.
const ICONSET: [IconType; 10] = [
    IconType::RGBA32_16x16,
    IconType::RGBA32_16x16_2x,
    IconType::RGBA32_32x32,
    IconType::RGBA32_32x32_2x,
    IconType::RGBA32_128x128,
    IconType::RGBA32_128x128_2x,
    IconType::RGBA32_256x256,
    IconType::RGBA32_256x256_2x,
    IconType::RGBA32_512x512,
    IconType::RGBA32_512x512_2x,
];

/// Apple's macOS app icon grid: an 824 px body on the 1024 px canvas, the margin left for the shadow.
const ICON_SHARE: f32 = 824.0 / 1024.0;

pub fn build(app_dir: &Path, config: &Config, args: &RunArgs) -> Result<(), CliError> {
    bundle(app_dir, config, args).map(|_| ())
}

pub fn run(app_dir: &Path, config: &Config, args: &RunArgs) -> Result<(), CliError> {
    let executable = bundle(app_dir, config, args)?;
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
fn bundle(app_dir: &Path, config: &Config, args: &RunArgs) -> Result<PathBuf, CliError> {
    // Rendered before the long build so a bad SVG fails fast.
    let icon = match &config.icon {
        Some(svg) => Some(app_icon(&Svg::read(&app_dir.join(svg))?)?),
        None => None,
    };
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = app_dir.join("Cargo.toml");
    let metadata = Metadata::parse(&capture(
        Command::new(&cargo)
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--manifest-path",
            ])
            .arg(&manifest),
        "cargo metadata",
        CARGO_FIX,
    )?)?;
    let package = metadata.app_package(&manifest)?;
    let bins: Vec<&str> = package
        .targets
        .iter()
        .filter(|t| t.crate_types.iter().any(|c| c == "bin"))
        .map(|t| t.name.as_str())
        .collect();
    let bin = match bins.as_slice() {
        [one] => *one,
        [] => return Err(CliError::NoBin(package.name.clone())),
        _ => {
            return Err(CliError::SeveralBins {
                package: package.name.clone(),
                bins: bins.iter().map(|b| b.to_string()).collect(),
            });
        }
    };

    let archs = &config.macos.archs;
    let mut build = Command::new(&cargo);
    build
        .args(["build", "--bin", bin, "--manifest-path"])
        .arg(&manifest)
        .args(args.profile.cargo_args());
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
    if app.exists() {
        std::fs::remove_dir_all(&app).map_err(|source| CliError::Io {
            path: app.clone(),
            source,
        })?;
    }
    let contents = app.join("Contents");
    let plist = info_plist(config, bin, &package.version, icon.is_some());
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
                .join(args.profile.dir())
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
    let (build_name, build_number) = bundle_versions(version);
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

/// The package version → (`CFBundleShortVersionString`, `CFBundleVersion`), as flutter_tools turns pubspec's
/// `version: 1.2.3+4` into `FLUTTER_BUILD_NAME` / `FLUTTER_BUILD_NUMBER`: `flutter_manifest.dart` `buildName` /
/// `buildNumber` split at `+`; `xcode_build_settings.dart` `parsedBuildNumber` falls back to the build name and
/// `_xcodeBuildSettingsLines` to `1.0.0` / `1`.
///
/// Ported from flutter/flutter packages/flutter_tools/lib/src/{flutter_manifest.dart,build_info.dart,
/// ios/xcode_build_settings.dart} @ 3.50.1. Copyright 2014 The Flutter Authors. BSD-3-Clause, see
/// third_party/flutter/LICENSE.
fn bundle_versions(version: &str) -> (String, String) {
    let mut parts = version.split('+');
    let build_name = parts.next().unwrap_or(version);
    let build_number = parts
        .next()
        .and_then(|n| validated_build_number(n).filter(|n| !n.is_empty()))
        .or_else(|| validated_build_number(build_name));
    (
        validated_build_name(build_name).unwrap_or_else(|| "1.0.0".into()),
        build_number.unwrap_or_else(|| "1".into()),
    )
}

/// `validatedBuildNameForPlatform`, iOS/macOS branch: see CFBundleShortVersionString at
/// https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html
fn validated_build_name(build_name: &str) -> Option<String> {
    let mut segments = digit_segments(build_name)?;
    while segments.len() < 3 {
        segments.push("0".into());
    }
    Some(segments.join("."))
}

/// `validatedBuildNumberForPlatform`, iOS/macOS branch: see CFBundleVersion at the same page.
fn validated_build_number(build_number: &str) -> Option<String> {
    let mut segments = digit_segments(build_number)?;
    if segments.is_empty() {
        segments.push("0".into());
    }
    Some(segments.join("."))
}

/// Both validators' shared start: drop `[^\d\.]`, `null` if nothing is left, then the non-empty `.` segments.
fn digit_segments(text: &str) -> Option<Vec<String>> {
    let kept: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    match kept.is_empty() {
        true => None,
        false => Some(
            kept.split('.')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
        ),
    }
}

/// `app.icon` rendered at every `ICONSET` size into one `.icns` container.
fn app_icon(svg: &Svg) -> Result<Vec<u8>, CliError> {
    let mut family = IconFamily::new();
    for icon_type in ICONSET {
        let px = icon_type.pixel_width();
        let pixmap = svg.render(px, ICON_SHARE)?;
        let rgba: Vec<u8> = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        let image = Image::from_data(PixelFormat::RGBA, px, px, rgba)
            .map_err(|e| svg.invalid(e.to_string()))?;
        family
            .add_icon_with_type(&image, icon_type)
            .map_err(|e| svg.invalid(e.to_string()))?;
    }
    let mut icns = Vec::new();
    family
        .write(&mut icns)
        .map_err(|e| svg.invalid(e.to_string()))?;
    Ok(icns)
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

    #[test]
    fn validated_build_number_table() {
        // flutter_tools test/general.shard/build_info_test.dart @ 3.50.1, "Validate build number" /
        // "CFBundleVersion for iOS": one row per expect.
        let cases = [
            ("xyz", None),
            ("0.0.1", Some("0.0.1")),
            ("123.xyz", Some("123")),
            ("123.456.xyz", Some("123.456")),
        ];
        for (input, want) in cases {
            assert_eq!(
                validated_build_number(input).as_deref(),
                want,
                "CFBundleVersion for iOS: {input}"
            );
        }
    }

    #[test]
    fn validated_build_name_table() {
        // build_info_test.dart @ 3.50.1, "Validate build name" / "CFBundleShortVersionString for iOS".
        let cases = [
            ("xyz", None),
            ("0.0.1", Some("0.0.1")),
            ("123.456.xyz", Some("123.456.0")),
            ("123.xyz", Some("123.0.0")),
        ];
        for (input, want) in cases {
            assert_eq!(
                validated_build_name(input).as_deref(),
                want,
                "CFBundleShortVersionString for iOS: {input}"
            );
        }
    }

    #[test]
    fn bundle_versions_table() {
        // (row name, Cargo package version, CFBundleShortVersionString, CFBundleVersion): the split at `+` and the
        // fallbacks around the two validators.
        let cases = [
            ("plain semver", "0.1.10", "0.1.10", "0.1.10"),
            (
                "build metadata is the build number",
                "1.2.3+45",
                "1.2.3",
                "45",
            ),
            (
                "pre-release letters dropped",
                "0.2.0-beta.1",
                "0.2.0.1",
                "0.2.0.1",
            ),
            (
                "build number letters dropped",
                "1.0.0+build.7",
                "1.0.0",
                "7",
            ),
            (
                "letters-only build number falls back to the name",
                "1.0.0+abc",
                "1.0.0",
                "1.0.0",
            ),
            ("short name padded to three segments", "2", "2.0.0", "2"),
            ("no digits at all", "abc", "1.0.0", "1"),
        ];
        for (name, version, build_name, build_number) in cases {
            assert_eq!(
                bundle_versions(version),
                (build_name.to_string(), build_number.to_string()),
                "{name}"
            );
        }
    }

    fn config(name: &str, touch_native: bool) -> Config {
        Config {
            name: name.into(),
            id: "io.github.xuckless.parallax.hello".into(),
            icon: None,
            android: crate::config::Android {
                min_sdk: 28,
                target_sdk: 35,
                abis: vec![],
            },
            macos: crate::config::Macos {
                touch_native,
                archs: vec![&ARCHS[0]],
                codesign: true,
            },
        }
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

    #[test]
    fn app_icon_has_every_iconset_size() -> Result<(), CliError> {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#123456"/></svg>"##;
        let icns = app_icon(&Svg::parse(svg, Path::new("icon.svg"))?)?;
        assert_eq!(&icns[..4], b"icns", "container magic");
        let family = IconFamily::read(icns.as_slice()).map_err(|source| CliError::Io {
            path: "AppIcon.icns".into(),
            source,
        })?;
        // Contents.json's (size, scale) pairs, in pixels.
        let pixels = [16, 32, 32, 64, 128, 256, 256, 512, 512, 1024];
        for (icon_type, px) in ICONSET.into_iter().zip(pixels) {
            assert!(family.has_icon_with_type(icon_type), "{icon_type:?}");
            assert_eq!(icon_type.pixel_width(), px, "{icon_type:?} size");
        }
        Ok(())
    }
}
