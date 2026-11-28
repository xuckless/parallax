//! `app.version` (else the Cargo package version) → each platform's version fields, as flutter_tools turns pubspec's
//! `version: 1.2.3+4` into `FLUTTER_BUILD_NAME` / `FLUTTER_BUILD_NUMBER`: `flutter_manifest.dart` `buildName` /
//! `buildNumber` split at `+`; `xcode_build_settings.dart` `parsedBuildNumber` falls back to the build name and
//! `_xcodeBuildSettingsLines` to `1.0.0` / `1`; Android's `versionName` is the build name as is.
//!
//! Ported from flutter/flutter packages/flutter_tools/lib/src/{flutter_manifest.dart,build_info.dart,
//! ios/xcode_build_settings.dart} @ 3.50.1. Copyright 2014 The Flutter Authors. BSD-3-Clause, see
//! third_party/flutter/LICENSE.

/// `buildName`: the version up to `+`. Android's `versionName`.
pub fn build_name(version: &str) -> &str {
    version.split('+').next().unwrap_or(version)
}

/// (`CFBundleShortVersionString`, `CFBundleVersion`) for iOS and macOS.
pub fn apple(version: &str) -> (String, String) {
    let build_name = build_name(version);
    let build_number = version
        .split_once('+')
        .and_then(|(_, n)| validated_build_number(n).filter(|n| !n.is_empty()))
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn versions_table() {
        // (row name, version, versionName, CFBundleShortVersionString, CFBundleVersion): the split at `+` and the
        // fallbacks around the two validators.
        let cases = [
            ("plain semver", "0.1.10", "0.1.10", "0.1.10", "0.1.10"),
            (
                "build metadata is the build number",
                "1.2.3+45",
                "1.2.3",
                "1.2.3",
                "45",
            ),
            (
                "pre-release letters dropped",
                "0.2.0-beta.1",
                "0.2.0-beta.1",
                "0.2.0.1",
                "0.2.0.1",
            ),
            (
                "build number letters dropped",
                "1.0.0+build.7",
                "1.0.0",
                "1.0.0",
                "7",
            ),
            (
                "letters-only build number falls back to the name",
                "1.0.0+abc",
                "1.0.0",
                "1.0.0",
                "1.0.0",
            ),
            (
                "short name padded to three segments",
                "2",
                "2",
                "2.0.0",
                "2",
            ),
            ("no digits at all", "abc", "abc", "1.0.0", "1"),
        ];
        for (name, version, android, short, bundle) in cases {
            assert_eq!(
                (build_name(version), apple(version)),
                (android, (short.to_string(), bundle.to_string())),
                "{name}"
            );
        }
    }
}
