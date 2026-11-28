//! `cargo parallax run ios` and `build ios`: cargo builds the app's binary, a generated Xcode project (Flutter's
//! Runner template) bundles and signs it, then `simctl` or `devicectl` installs and launches it with its log on the
//! terminal. Step by step in docs/cli-ios.md. Decisions are pure functions over plain values; `ios` is the thin shell.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::android::{self, CARGO_FIX, Profile, capture, render, step, write, xml_escape};
use crate::config::Config;
use crate::error::CliError;
use crate::icon;

/// Where the app runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Destination {
    Simulator,
    Device,
}

struct DestinationRow {
    destination: Destination,
    /// `xcodebuild -sdk`, and the suffix of Xcode's `<Configuration>-<sdk>` products directory.
    sdk: &'static str,
    /// `xcodebuild -destination` when no device is picked (`build ios`).
    generic: &'static str,
    /// What errors call one.
    what: &'static str,
    /// What errors call one that `run` can use without being named.
    ready_what: &'static str,
    /// The flag that names one.
    flag: &'static str,
    /// How to get a usable one.
    none_fix: &'static str,
}

/// Destination → Xcode SDK, generic `-destination` and how errors name it (Flutter's `XcodeSdk`). To add one: add
/// a row and its `RUST_TARGETS` rows.
const DESTINATIONS: [DestinationRow; 2] = [
    DestinationRow {
        destination: Destination::Simulator,
        sdk: "iphonesimulator",
        generic: "generic/platform=iOS Simulator",
        what: "iOS simulator",
        ready_what: "booted iOS simulator",
        flag: "--simulator",
        none_fix: "Pick one with --simulator <name|udid> (cargo parallax boots it), or boot one: xcrun simctl boot <name|udid>",
    },
    DestinationRow {
        destination: Destination::Device,
        sdk: "iphoneos",
        generic: "generic/platform=iOS",
        what: "iOS device",
        ready_what: "paired iOS device",
        flag: "--device",
        none_fix: "Connect an iPhone or iPad by USB, unlock it and trust this Mac (Xcode → Window → Devices and Simulators pairs it, also over Wi-Fi)",
    },
];

/// (destination, host CPU as `std::env::consts::ARCH`) → Rust target. Devices are arm64 whatever the host; an
/// Intel Mac's simulator runs x86_64. To add a host: add its rows (and `rustup target add` the target).
const RUST_TARGETS: [(Destination, &str, &str); 4] = [
    (Destination::Simulator, "aarch64", "aarch64-apple-ios-sim"),
    (Destination::Simulator, "x86_64", "x86_64-apple-ios"),
    (Destination::Device, "aarch64", "aarch64-apple-ios"),
    (Destination::Device, "x86_64", "aarch64-apple-ios"),
];

/// Files of the generated Xcode project, written fresh every run. `{{key}}` is filled from `project_vars`. To add
/// a file: put it under `templates/ios/` and add a row.
const PROJECT_FILES: [(&str, &str); 8] = [
    (
        "Runner.xcodeproj/project.pbxproj",
        include_str!("../templates/ios/Runner.xcodeproj/project.pbxproj"),
    ),
    (
        "Runner.xcodeproj/project.xcworkspace/contents.xcworkspacedata",
        include_str!(
            "../templates/ios/Runner.xcodeproj/project.xcworkspace/contents.xcworkspacedata"
        ),
    ),
    (
        "Runner.xcodeproj/xcshareddata/xcschemes/Runner.xcscheme",
        include_str!("../templates/ios/Runner.xcodeproj/xcshareddata/xcschemes/Runner.xcscheme"),
    ),
    (
        "Runner/Info.plist",
        include_str!("../templates/ios/Runner/Info.plist"),
    ),
    (
        "Runner/Base.lproj/LaunchScreen.storyboard",
        include_str!("../templates/ios/Runner/Base.lproj/LaunchScreen.storyboard"),
    ),
    (APP_ICON_CONTENTS, APP_ICON_JSON),
    (
        "Parallax/Debug.xcconfig",
        include_str!("../templates/ios/Parallax/Debug.xcconfig"),
    ),
    (
        "Parallax/Release.xcconfig",
        include_str!("../templates/ios/Parallax/Release.xcconfig"),
    ),
];

const APP_ICON_CONTENTS: &str = "Runner/Assets.xcassets/AppIcon.appiconset/Contents.json";
/// Flutter's AppIcon set; its `images` are the table of icon files `app.icon` is rendered to.
const APP_ICON_JSON: &str =
    include_str!("../templates/ios/Runner/Assets.xcassets/AppIcon.appiconset/Contents.json");

/// Flutter's `Generated.xcconfig`: the values each run writes, included by `Debug.xcconfig` and `Release.xcconfig`.
const GENERATED_XCCONFIG: &str = "Parallax/Generated.xcconfig";

const XCODE_FIX: &str = "Install Xcode from the App Store, open it once, then: sudo xcode-select -s /Applications/Xcode.app";

/// `run ios` installs and launches; `build ios` stops at the `.app`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Run,
    Build,
}

pub struct IosArgs {
    pub profile: Profile,
    pub destination: Destination,
    /// `--simulator <name|udid>` / `--device <name|udid>`; `None` takes the only ready one.
    pub pick: Option<String>,
}

/// How xcodebuild signs.
#[derive(Debug, PartialEq)]
enum Signing {
    /// `CODE_SIGNING_ALLOWED=NO`: simulators, and `build ios` without `ios.team`.
    Off,
    /// Automatic signing for this team, with the account signed in to Xcode.
    Team(String),
}

/// One simulator or device: `udid` names it to xcodebuild and simctl, `id` to devicectl.
#[derive(Debug, PartialEq)]
struct Candidate {
    name: String,
    udid: String,
    id: String,
    /// Booted (simulator) or paired (device): what `run` picks from when none is named.
    ready: bool,
}

pub fn run(app_dir: &Path, config: &Config, args: &[String]) -> Result<(), CliError> {
    ios(app_dir, config, Action::Run, &parse_run(args)?)
}

pub fn build(app_dir: &Path, config: &Config, args: &[String]) -> Result<(), CliError> {
    ios(app_dir, config, Action::Build, &parse_build(args)?)
}

/// `run ios` flags: `[--release] [--simulator <name|udid> | --device [<name|udid>]]`.
fn parse_run(args: &[String]) -> Result<IosArgs, CliError> {
    let mut parsed = IosArgs {
        profile: Profile::Debug,
        destination: Destination::Simulator,
        pick: None,
    };
    let mut picked = false;
    let mut rest = args.iter().peekable();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--release" => parsed.profile = Profile::Release,
            "--simulator" | "--device" if picked => {
                return Err(CliError::Usage(
                    "pass one of --simulator and --device".into(),
                ));
            }
            "--simulator" => {
                let name = rest
                    .next()
                    .ok_or_else(|| CliError::Usage("--simulator needs a name or udid".into()))?;
                parsed.pick = Some(name.clone());
                picked = true;
            }
            "--device" => {
                parsed.destination = Destination::Device;
                parsed.pick = rest.next_if(|a| !a.starts_with("--")).cloned();
                picked = true;
            }
            other => return Err(CliError::Usage(format!("unknown argument {other:?}"))),
        }
    }
    Ok(parsed)
}

/// `build ios` flags: `[--release]`. It always builds for devices.
fn parse_build(args: &[String]) -> Result<IosArgs, CliError> {
    let mut parsed = IosArgs {
        profile: Profile::Debug,
        destination: Destination::Device,
        pick: None,
    };
    for arg in args {
        match arg.as_str() {
            "--release" => parsed.profile = Profile::Release,
            other => return Err(CliError::Usage(format!("unknown argument {other:?}"))),
        }
    }
    Ok(parsed)
}

fn ios(app_dir: &Path, config: &Config, action: Action, args: &IosArgs) -> Result<(), CliError> {
    let os = std::env::consts::OS;
    if os != "macos" {
        return Err(CliError::IosNeedsMacos(os));
    }
    if !is_bundle_id(&config.id) {
        return Err(CliError::BadBundleId(config.id.clone()));
    }
    let row = destination_row(args.destination);
    let signing = signing(action, args.destination, config.ios.team.as_deref())?;
    // Rendered before the long build so a bad SVG fails fast; written with the other project files.
    let icon_files = match &config.icon {
        Some(svg) => icon::app_icon_set(&app_dir.join(svg), &icon_sizes(APP_ICON_JSON)?)?,
        None => Vec::new(),
    };

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = app_dir.join("Cargo.toml");
    let metadata = capture(
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
    )?;
    let (metadata, index) = android::app_package(&metadata, &manifest)?;
    let package = &metadata.packages[index];
    let bin = app_bin(&package.name, &package.targets)?;
    let project = metadata
        .target_directory
        .join("parallax/ios")
        .join(&package.name);

    // Fail on a missing simulator or device before the long build.
    let picked = match action {
        Action::Build => None,
        Action::Run => Some(pick_candidate(&project, row, args.pick.as_deref())?),
    };

    let arch = std::env::consts::ARCH;
    let triple = rust_target(args.destination, arch).ok_or(CliError::UnsupportedIosHost(arch))?;
    step(
        Command::new(&cargo)
            .args(["build", "--bin", &bin, "--manifest-path"])
            .arg(&manifest)
            .args(args.profile.cargo_args())
            .args(["--target", triple])
            .env("IPHONEOS_DEPLOYMENT_TARGET", &config.ios.min_ios),
        CARGO_FIX,
        "cargo build",
        format!("If the target is missing: rustup target add {triple}"),
    )?;

    let runner = project.join("Runner");
    if runner.exists() {
        std::fs::remove_dir_all(&runner).map_err(|source| CliError::Io {
            path: runner.clone(),
            source,
        })?;
    }
    let vars = project_vars(config);
    for (path, text) in &PROJECT_FILES {
        write(&project.join(path), render(text, &vars).as_bytes())?;
    }
    let icon_dir = Path::new(APP_ICON_CONTENTS)
        .parent()
        .unwrap_or(Path::new(""));
    for (name, bytes) in &icon_files {
        write(&project.join(icon_dir).join(name), bytes)?;
    }
    let binaries: Vec<(&str, PathBuf)> = DESTINATIONS
        .iter()
        .filter_map(|d| {
            let target = rust_target(d.destination, arch)?;
            let path = metadata
                .target_directory
                .join(target)
                .join(args.profile.dir())
                .join(&bin);
            Some((d.sdk, path))
        })
        .collect();
    write(
        &project.join(GENERATED_XCCONFIG),
        generated_xcconfig(&binaries, &bundle_version(&package.version), &signing).as_bytes(),
    )?;
    println!("cargo parallax: Xcode project in {}", project.display());

    let build_dir = project.join("build");
    let configuration = xcode_configuration(args.profile);
    step(
        Command::new("xcrun")
            .arg("xcodebuild")
            .args(xcodebuild_args(
                &build_dir,
                configuration,
                row,
                picked.as_ref().map(|c| c.udid.as_str()),
                &signing,
            ))
            .current_dir(&project),
        XCODE_FIX,
        "xcodebuild",
        format!(
            "See Xcode's output above; open {} in Xcode for details",
            project.join("Runner.xcodeproj").display()
        ),
    )?;
    let app = build_dir.join(format!("{configuration}-{}/Runner.app", row.sdk));
    let Some(target) = picked else {
        let signed = match &signing {
            Signing::Off => "unsigned (set ios.team in parallax.toml to sign)".to_string(),
            Signing::Team(team) => format!("signed for team {team}"),
        };
        println!("cargo parallax: built {}, {signed}", app.display());
        return Ok(());
    };
    match args.destination {
        Destination::Simulator => launch_simulator(&target, &app, &config.id),
        Destination::Device => launch_device(&target, &app, &config.id),
    }
}

/// Boots the simulator if needed (and waits for it), installs the app and runs it with its stdout and stderr on
/// the terminal until it exits (Ctrl-C stops it). The simulator's window is Xcode's app (Simulator, DeviceHub in
/// Xcode 27), which the user opens; the app runs without it.
fn launch_simulator(sim: &Candidate, app: &Path, bundle_id: &str) -> Result<(), CliError> {
    let simctl = |args: &[&str]| {
        let mut cmd = Command::new("xcrun");
        cmd.arg("simctl").args(args);
        cmd
    };
    step(
        &mut simctl(&["bootstatus", &sim.udid, "-b"]),
        XCODE_FIX,
        "xcrun simctl bootstatus -b",
        String::new(),
    )?;
    step(
        simctl(&["install", &sim.udid]).arg(app),
        XCODE_FIX,
        "xcrun simctl install",
        "This is sometimes caused by a malformed Info.plist".into(),
    )?;
    println!(
        "cargo parallax: launching {bundle_id} on {} ({}); its log follows, Ctrl-C stops it",
        sim.name, sim.udid
    );
    step(
        &mut simctl(&[
            "launch",
            "--console-pty",
            "--terminate-running-process",
            &sim.udid,
            bundle_id,
        ]),
        XCODE_FIX,
        "xcrun simctl launch",
        String::new(),
    )
}

/// Installs the app on the device and runs it with its stdout and stderr on the terminal until it exits.
fn launch_device(device: &Candidate, app: &Path, bundle_id: &str) -> Result<(), CliError> {
    step(
        Command::new("xcrun")
            .args(["devicectl", "device", "install", "app", "--device", &device.id])
            .arg(app),
        XCODE_FIX,
        "xcrun devicectl device install app",
        "Unlock the device; the first time, trust the developer in Settings → General → VPN & Device Management"
            .into(),
    )?;
    println!(
        "cargo parallax: launching {bundle_id} on {}; its log follows, Ctrl-C stops it",
        device.name
    );
    step(
        Command::new("xcrun").args([
            "devicectl",
            "device",
            "process",
            "launch",
            "--console",
            "--terminate-existing",
            "--device",
            &device.id,
            bundle_id,
        ]),
        XCODE_FIX,
        "xcrun devicectl device process launch",
        "Unlock the device and run again".into(),
    )
}

/// Lists simulators (`simctl`) or devices (`devicectl`) and picks one.
fn pick_candidate(
    project: &Path,
    row: &DestinationRow,
    requested: Option<&str>,
) -> Result<Candidate, CliError> {
    let candidates = match row.destination {
        Destination::Simulator => simulators(&capture(
            Command::new("xcrun").args(["simctl", "list", "devices", "available", "--json"]),
            "xcrun simctl list",
            XCODE_FIX,
        )?)?,
        Destination::Device => {
            // devicectl prints its table on stdout; the JSON goes to a file.
            let json = project.join("devicectl-devices.json");
            write(&json, b"")?;
            capture(
                Command::new("xcrun")
                    .args(["devicectl", "list", "devices", "--quiet", "--json-output"])
                    .arg(&json),
                "xcrun devicectl list devices",
                XCODE_FIX,
            )?;
            let bytes = std::fs::read(&json).map_err(|source| CliError::Io {
                path: json.clone(),
                source,
            })?;
            devices(&bytes)?
        }
    };
    pick(candidates, row, requested)
}

fn destination_row(destination: Destination) -> &'static DestinationRow {
    match destination {
        Destination::Simulator => &DESTINATIONS[0],
        Destination::Device => &DESTINATIONS[1],
    }
}

fn rust_target(destination: Destination, arch: &str) -> Option<&'static str> {
    RUST_TARGETS
        .iter()
        .find(|(d, a, _)| *d == destination && *a == arch)
        .map(|(_, _, target)| *target)
}

fn xcode_configuration(profile: Profile) -> &'static str {
    match profile {
        Profile::Debug => "Debug",
        Profile::Release => "Release",
    }
}

/// Simulators sign ad hoc; a device needs the team, except for `build ios`, which then builds unsigned.
fn signing(
    action: Action,
    destination: Destination,
    team: Option<&str>,
) -> Result<Signing, CliError> {
    match (destination, team, action) {
        (Destination::Simulator, _, _) => Ok(Signing::Off),
        (Destination::Device, Some(team), _) => Ok(Signing::Team(team.to_string())),
        (Destination::Device, None, Action::Build) => Ok(Signing::Off),
        (Destination::Device, None, Action::Run) => Err(CliError::TeamMissing),
    }
}

/// Flutter's `buildXcodeProject` arguments (mac.dart), less its plugins, result bundle and output pipe.
fn xcodebuild_args(
    build_dir: &Path,
    configuration: &str,
    row: &DestinationRow,
    udid: Option<&str>,
    signing: &Signing,
) -> Vec<String> {
    let mut args: Vec<String> = ["-project", "Runner.xcodeproj", "-scheme", "Runner"]
        .map(String::from)
        .into();
    args.extend([
        "-configuration".into(),
        configuration.into(),
        "-quiet".into(),
    ]);
    if let Signing::Team(_) = signing {
        args.extend([
            "-allowProvisioningUpdates".into(),
            "-allowProvisioningDeviceRegistration".into(),
        ]);
    }
    args.push(format!("BUILD_DIR={}", build_dir.display()));
    args.extend(["-sdk".into(), row.sdk.into(), "-destination".into()]);
    args.push(match udid {
        Some(udid) => format!("id={udid}"),
        None => row.generic.into(),
    });
    if let Signing::Off = signing {
        args.extend([
            "CODE_SIGNING_ALLOWED=NO".into(),
            "CODE_SIGNING_REQUIRED=NO".into(),
            "CODE_SIGNING_IDENTITY=\"\"".into(),
        ]);
    }
    args.extend(["COMPILER_INDEX_STORE_ENABLE=NO".into(), "build".into()]);
    args
}

/// `Parallax/Generated.xcconfig`: the binary per SDK, the bundle version and the team.
fn generated_xcconfig(binaries: &[(&str, PathBuf)], version: &str, signing: &Signing) -> String {
    let mut text = String::from(
        "// Generated by cargo-parallax every run, as flutter_tools writes Flutter/Generated.xcconfig. Do not edit.\n",
    );
    for (sdk, path) in binaries {
        text += &format!("PARALLAX_BINARY[sdk={sdk}*] = {}\n", path.display());
    }
    text += &format!("PARALLAX_BUILD_NAME = {version}\nPARALLAX_BUILD_NUMBER = {version}\n");
    if let Signing::Team(team) = signing {
        text += &format!("DEVELOPMENT_TEAM = {team}\n");
    }
    text
}

/// The package version as Apple takes it: `CFBundleShortVersionString` and `CFBundleVersion` are dot-separated
/// integers, so a SemVer pre-release or build suffix is dropped.
fn bundle_version(version: &str) -> String {
    version
        .split(['-', '+'])
        .next()
        .unwrap_or(version)
        .to_string()
}

/// Apple's rule: dot-separated segments of letters, digits and `-` (no `_`, which Android allows).
fn is_bundle_id(id: &str) -> bool {
    id.split('.')
        .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

/// iOS runs the app's binary: its only bin target, or the one named like the package.
fn app_bin(package: &str, targets: &[android::MetaTarget]) -> Result<String, CliError> {
    let bins: Vec<&str> = targets
        .iter()
        .filter(|t| t.crate_types.iter().any(|c| c == "bin"))
        .map(|t| t.name.as_str())
        .collect();
    match bins[..] {
        [] => Err(CliError::NoBin(package.to_string())),
        [one] => Ok(one.to_string()),
        _ if bins.contains(&package) => Ok(package.to_string()),
        _ => Err(CliError::SeveralBins {
            package: package.to_string(),
            bins: bins.iter().map(|b| b.to_string()).collect(),
        }),
    }
}

/// `{{key}}` values for the templates, escaped for the files they land in.
fn project_vars(config: &Config) -> Vec<(&'static str, String)> {
    vec![
        ("name", xml_escape(&config.name)),
        // A bundle id (checked by is_bundle_id) needs no quoting in project.pbxproj.
        ("id", config.id.clone()),
        ("min_ios", config.ios.min_ios.clone()),
    ]
}

#[derive(Deserialize)]
struct IconSet {
    images: Vec<IconImage>,
}

#[derive(Deserialize)]
struct IconImage {
    size: String,
    scale: String,
    filename: String,
}

/// (file name, pixel size) of every file in an AppIcon `Contents.json`: `size` points times `scale`, each file once.
fn icon_sizes(contents_json: &str) -> Result<Vec<(String, u32)>, CliError> {
    let bad = |reason: String| CliError::ToolOutput {
        tool: "AppIcon Contents.json",
        reason,
    };
    let set: IconSet = serde_json::from_str(contents_json).map_err(|e| bad(e.to_string()))?;
    let mut sizes: Vec<(String, u32)> = Vec::new();
    for image in set.images {
        let points = image
            .size
            .split('x')
            .next()
            .and_then(|w| w.parse::<f32>().ok());
        let scale = image
            .scale
            .strip_suffix('x')
            .and_then(|s| s.parse::<f32>().ok());
        let (Some(points), Some(scale)) = (points, scale) else {
            return Err(bad(format!(
                "{}: size {:?} scale {:?}",
                image.filename, image.size, image.scale
            )));
        };
        if !sizes.iter().any(|(name, _)| *name == image.filename) {
            sizes.push((image.filename, (points * scale).round() as u32));
        }
    }
    Ok(sizes)
}

#[derive(Deserialize)]
struct SimctlList {
    devices: std::collections::BTreeMap<String, Vec<SimctlDevice>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimctlDevice {
    udid: String,
    name: String,
    state: String,
    is_available: bool,
}

/// iOS simulators from `xcrun simctl list devices available --json` (watchOS, tvOS and visionOS ones skipped).
fn simulators(json: &[u8]) -> Result<Vec<Candidate>, CliError> {
    let list: SimctlList = serde_json::from_slice(json).map_err(|e| CliError::ToolOutput {
        tool: "xcrun simctl list",
        reason: e.to_string(),
    })?;
    Ok(list
        .devices
        .into_iter()
        .filter(|(runtime, _)| runtime.contains(".SimRuntime.iOS-"))
        .flat_map(|(_, sims)| sims)
        .filter(|sim| sim.is_available)
        .map(|sim| Candidate {
            ready: sim.state == "Booted",
            id: sim.udid.clone(),
            udid: sim.udid,
            name: sim.name,
        })
        .collect())
}

#[derive(Deserialize)]
struct DevicectlList {
    result: DevicectlResult,
}

#[derive(Deserialize)]
struct DevicectlResult {
    devices: Vec<DevicectlDevice>,
}

#[derive(Deserialize)]
struct DevicectlDevice {
    identifier: String,
    properties: DevicectlProperties,
}

#[derive(Deserialize)]
struct DevicectlProperties {
    hardware: DevicectlHardware,
    state: DevicectlState,
}

#[derive(Deserialize)]
struct DevicectlHardware {
    platform: String,
    reality: String,
    udid: String,
}

#[derive(Deserialize)]
struct DevicectlState {
    name: String,
}

/// Physical iOS devices (iPhone, iPad) from `xcrun devicectl list devices --json-output` (jsonVersion 5). devicectl
/// also lists simulators (`reality` "simulated") and other platforms; both are skipped.
fn devices(json: &[u8]) -> Result<Vec<Candidate>, CliError> {
    let list: DevicectlList = serde_json::from_slice(json).map_err(|e| CliError::ToolOutput {
        tool: "xcrun devicectl list devices",
        reason: e.to_string(),
    })?;
    Ok(list
        .result
        .devices
        .into_iter()
        .filter(|d| {
            d.properties.hardware.platform == "iOS" && d.properties.hardware.reality == "physical"
        })
        .map(|d| Candidate {
            name: d.properties.state.name,
            udid: d.properties.hardware.udid,
            id: d.identifier,
            ready: true,
        })
        .collect())
}

/// A named candidate (by name, udid or devicectl id) must exist; without a name exactly one must be ready.
fn pick(
    candidates: Vec<Candidate>,
    row: &DestinationRow,
    requested: Option<&str>,
) -> Result<Candidate, CliError> {
    let names = |c: &[Candidate]| c.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
    if let Some(requested) = requested {
        let available = names(&candidates);
        return candidates
            .into_iter()
            .find(|c| c.name == requested || c.udid == requested || c.id == requested)
            .ok_or_else(|| CliError::IosTargetNotFound {
                what: row.what,
                requested: requested.to_string(),
                available,
            });
    }
    let available = names(&candidates);
    let mut ready: Vec<Candidate> = candidates.into_iter().filter(|c| c.ready).collect();
    match ready.len() {
        0 => Err(CliError::NoIosTarget {
            what: row.ready_what,
            fix: row.none_fix,
            available,
        }),
        1 => Ok(ready.remove(0)),
        _ => Err(CliError::SeveralIosTargets {
            what: row.ready_what,
            flag: row.flag,
            names: names(&ready),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_run_table() {
        // (row name, flags after `run ios`, expected (profile dir, destination, pick) or a usage error)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            Result<(&'a str, Destination, Option<&'a str>), ()>,
        );
        let cases: [Case; 11] = [
            (
                "no flags: a booted simulator",
                &[],
                Ok(("debug", Destination::Simulator, None)),
            ),
            (
                "release",
                &["--release"],
                Ok(("release", Destination::Simulator, None)),
            ),
            (
                "simulator by name",
                &["--simulator", "iPad Pro 11-inch (M5)"],
                Ok((
                    "debug",
                    Destination::Simulator,
                    Some("iPad Pro 11-inch (M5)"),
                )),
            ),
            (
                "simulator then release",
                &["--simulator", "B03F", "--release"],
                Ok(("release", Destination::Simulator, Some("B03F"))),
            ),
            ("simulator without name", &["--simulator"], Err(())),
            (
                "device alone",
                &["--device"],
                Ok(("debug", Destination::Device, None)),
            ),
            (
                "device then flag",
                &["--device", "--release"],
                Ok(("release", Destination::Device, None)),
            ),
            (
                "device by name",
                &["--device", "Ali's iPad"],
                Ok(("debug", Destination::Device, Some("Ali's iPad"))),
            ),
            (
                "simulator and device",
                &["--simulator", "x", "--device"],
                Err(()),
            ),
            ("device twice", &["--device", "--device"], Err(())),
            ("unknown flag", &["--fast"], Err(())),
        ];
        for (name, flags, want) in cases {
            let got = parse_run(&args(flags))
                .map(|a| (a.profile.dir(), a.destination, a.pick))
                .map_err(|e| e.variant());
            let want = want
                .map(|(p, d, pick)| (p, d, pick.map(String::from)))
                .map_err(|()| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn parse_build_table() {
        let cases: [(&str, &[&str], Result<&str, ()>); 4] = [
            ("debug", &[], Ok("debug")),
            ("release", &["--release"], Ok("release")),
            ("no simulator builds", &["--simulator", "x"], Err(())),
            ("no device pick", &["--device"], Err(())),
        ];
        for (name, flags, want) in cases {
            let got = parse_build(&args(flags))
                .map(|a| (a.profile.dir(), a.destination, a.pick))
                .map_err(|e| e.variant());
            let want = want
                .map(|p| (p, Destination::Device, None))
                .map_err(|()| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn destination_table() {
        // (destination, sdk, generic destination, rust target on arm64, on x86_64)
        let cases = [
            (
                Destination::Simulator,
                "iphonesimulator",
                "generic/platform=iOS Simulator",
                "aarch64-apple-ios-sim",
                "x86_64-apple-ios",
            ),
            (
                Destination::Device,
                "iphoneos",
                "generic/platform=iOS",
                "aarch64-apple-ios",
                "aarch64-apple-ios",
            ),
        ];
        assert_eq!(
            DESTINATIONS.len(),
            cases.len(),
            "every destination row has a case"
        );
        for (destination, sdk, generic, arm, intel) in cases {
            let row = destination_row(destination);
            assert_eq!(
                row.destination, destination,
                "destination_row picks its own row"
            );
            assert_eq!(
                (
                    row.sdk,
                    row.generic,
                    rust_target(destination, "aarch64"),
                    rust_target(destination, "x86_64")
                ),
                (sdk, generic, Some(arm), Some(intel)),
                "{destination:?}"
            );
        }
        assert_eq!(
            rust_target(Destination::Device, "riscv64"),
            None,
            "unknown host"
        );
    }

    #[test]
    fn signing_table() {
        // (row name, action, destination, team, expected)
        type Case<'a> = (
            &'a str,
            Action,
            Destination,
            Option<&'a str>,
            Result<Signing, &'a str>,
        );
        let team = || Signing::Team("ABCDE12345".into());
        let cases: [Case; 6] = [
            (
                "simulator run ignores the team",
                Action::Run,
                Destination::Simulator,
                Some("ABCDE12345"),
                Ok(Signing::Off),
            ),
            (
                "simulator run without team",
                Action::Run,
                Destination::Simulator,
                None,
                Ok(Signing::Off),
            ),
            (
                "device run signs",
                Action::Run,
                Destination::Device,
                Some("ABCDE12345"),
                Ok(team()),
            ),
            (
                "device run needs a team",
                Action::Run,
                Destination::Device,
                None,
                Err("TeamMissing"),
            ),
            (
                "device build signs with a team",
                Action::Build,
                Destination::Device,
                Some("ABCDE12345"),
                Ok(team()),
            ),
            (
                "device build without team is unsigned",
                Action::Build,
                Destination::Device,
                None,
                Ok(Signing::Off),
            ),
        ];
        for (name, action, destination, team, want) in cases {
            let got = signing(action, destination, team).map_err(|e| e.variant());
            assert_eq!(got, want.map_err(String::from), "{name}");
        }
    }

    #[test]
    fn xcodebuild_args_table() {
        let build = Path::new("/t/parallax/ios/hello/build");
        let common = |tail: &[&str]| -> Vec<String> {
            let mut v = args(&["-project", "Runner.xcodeproj", "-scheme", "Runner"]);
            v.extend(args(tail));
            v
        };
        let off = [
            "CODE_SIGNING_ALLOWED=NO",
            "CODE_SIGNING_REQUIRED=NO",
            "CODE_SIGNING_IDENTITY=\"\"",
        ];
        let tail = ["COMPILER_INDEX_STORE_ENABLE=NO", "build"];
        // (row name, configuration, destination, picked udid, signing, expected arguments)
        type Case<'a> = (
            &'a str,
            &'a str,
            Destination,
            Option<&'a str>,
            Signing,
            Vec<String>,
        );
        let cases: [Case; 3] = [
            (
                "simulator run",
                "Debug",
                Destination::Simulator,
                Some("B03F"),
                Signing::Off,
                common(
                    &[
                        &[
                            "-configuration",
                            "Debug",
                            "-quiet",
                            "BUILD_DIR=/t/parallax/ios/hello/build",
                        ][..],
                        &["-sdk", "iphonesimulator", "-destination", "id=B03F"],
                        &off,
                        &tail,
                    ]
                    .concat(),
                ),
            ),
            (
                "signed device run",
                "Release",
                Destination::Device,
                Some("00008112-AB"),
                Signing::Team("ABCDE12345".into()),
                common(
                    &[
                        &["-configuration", "Release", "-quiet"][..],
                        &[
                            "-allowProvisioningUpdates",
                            "-allowProvisioningDeviceRegistration",
                        ],
                        &["BUILD_DIR=/t/parallax/ios/hello/build", "-sdk", "iphoneos"],
                        &["-destination", "id=00008112-AB"],
                        &tail,
                    ]
                    .concat(),
                ),
            ),
            (
                "unsigned device build",
                "Release",
                Destination::Device,
                None,
                Signing::Off,
                common(
                    &[
                        &[
                            "-configuration",
                            "Release",
                            "-quiet",
                            "BUILD_DIR=/t/parallax/ios/hello/build",
                        ][..],
                        &["-sdk", "iphoneos", "-destination", "generic/platform=iOS"],
                        &off,
                        &tail,
                    ]
                    .concat(),
                ),
            ),
        ];
        for (name, configuration, destination, udid, signing, want) in cases {
            let got = xcodebuild_args(
                build,
                configuration,
                destination_row(destination),
                udid,
                &signing,
            );
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn generated_xcconfig_table() {
        let binaries = [
            (
                "iphonesimulator",
                PathBuf::from("/t/aarch64-apple-ios-sim/debug/hello"),
            ),
            (
                "iphoneos",
                PathBuf::from("/t/aarch64-apple-ios/debug/hello"),
            ),
        ];
        let head = "// Generated by cargo-parallax every run, as flutter_tools writes Flutter/Generated.xcconfig. Do not edit.\n\
            PARALLAX_BINARY[sdk=iphonesimulator*] = /t/aarch64-apple-ios-sim/debug/hello\n\
            PARALLAX_BINARY[sdk=iphoneos*] = /t/aarch64-apple-ios/debug/hello\n\
            PARALLAX_BUILD_NAME = 0.1.10\nPARALLAX_BUILD_NUMBER = 0.1.10\n";
        let cases = [
            ("unsigned", Signing::Off, head.to_string()),
            (
                "team",
                Signing::Team("ABCDE12345".into()),
                format!("{head}DEVELOPMENT_TEAM = ABCDE12345\n"),
            ),
        ];
        for (name, signing, want) in cases {
            assert_eq!(
                generated_xcconfig(&binaries, "0.1.10", &signing),
                want,
                "{name}"
            );
        }
    }

    #[test]
    fn bundle_version_table() {
        let cases = [
            ("plain", "0.1.10", "0.1.10"),
            ("pre-release", "1.2.0-alpha.1", "1.2.0"),
            ("build metadata", "1.2.0+20261128", "1.2.0"),
        ];
        for (name, version, want) in cases {
            assert_eq!(bundle_version(version), want, "{name}");
        }
    }

    #[test]
    fn bundle_id_table() {
        let cases = [
            (
                "Parallax example",
                "io.github.xuckless.parallax.hello",
                true,
            ),
            ("dash", "com.example.my-app", true),
            ("underscore is Android only", "com.example.my_app", false),
            ("empty segment", "com..app", false),
            ("space", "com.my app", false),
        ];
        for (name, id, want) in cases {
            assert_eq!(is_bundle_id(id), want, "{name}");
        }
    }

    #[test]
    fn app_bin_table() {
        let target = |name: &str, kind: &str| android::MetaTarget {
            name: name.into(),
            crate_types: vec![kind.into()],
        };
        type Case = (
            &'static str,
            Vec<android::MetaTarget>,
            Result<&'static str, &'static str>,
        );
        let cases: Vec<Case> = vec![
            (
                "one bin",
                vec![target("hello", "cdylib"), target("hello", "bin")],
                Ok("hello"),
            ),
            ("bin named otherwise", vec![target("app", "bin")], Ok("app")),
            (
                "several, one named like the package",
                vec![target("tool", "bin"), target("hello", "bin")],
                Ok("hello"),
            ),
            (
                "several, none named like the package",
                vec![target("a", "bin"), target("b", "bin")],
                Err("SeveralBins"),
            ),
            ("lib only", vec![target("hello", "cdylib")], Err("NoBin")),
        ];
        for (name, targets, want) in cases {
            let got = app_bin("hello", &targets).map_err(|e| e.variant());
            assert_eq!(got, want.map(String::from).map_err(String::from), "{name}");
        }
    }

    #[test]
    fn icon_sizes_from_flutter_contents_json() -> Result<(), CliError> {
        // Flutter's AppIcon set: 19 slots, 15 files (the 20pt@2x, 29pt@1x/@2x and 40pt@2x files serve iPhone and iPad).
        let want = [
            ("Icon-App-20x20@2x.png", 40),
            ("Icon-App-20x20@3x.png", 60),
            ("Icon-App-29x29@1x.png", 29),
            ("Icon-App-29x29@2x.png", 58),
            ("Icon-App-29x29@3x.png", 87),
            ("Icon-App-40x40@2x.png", 80),
            ("Icon-App-40x40@3x.png", 120),
            ("Icon-App-60x60@2x.png", 120),
            ("Icon-App-60x60@3x.png", 180),
            ("Icon-App-20x20@1x.png", 20),
            ("Icon-App-40x40@1x.png", 40),
            ("Icon-App-76x76@1x.png", 76),
            ("Icon-App-76x76@2x.png", 152),
            ("Icon-App-83.5x83.5@2x.png", 167),
            ("Icon-App-1024x1024@1x.png", 1024),
        ]
        .map(|(f, px)| (f.to_string(), px));
        assert_eq!(icon_sizes(APP_ICON_JSON)?, want);
        assert_eq!(
            icon_sizes(r#"{"images":[{"size":"20","scale":"2x","filename":"a.png"}]}"#)?,
            [("a.png".to_string(), 40)],
            "size without height"
        );
        let bad = icon_sizes(r#"{"images":[{"size":"20x20","scale":"two","filename":"a.png"}]}"#)
            .map_err(|e| e.variant());
        assert_eq!(bad, Err("ToolOutput".to_string()), "bad scale");
        Ok(())
    }

    const SIMCTL: &str = r#"{"devices":{
        "com.apple.CoreSimulator.SimRuntime.iOS-26-5":[
            {"udid":"B03F","isAvailable":true,"state":"Shutdown","name":"iPhone 17 Pro","dataPath":"/x"},
            {"udid":"9267","isAvailable":true,"state":"Booted","name":"iPad Pro 13-inch (M5)"},
            {"udid":"DEAD","isAvailable":false,"state":"Shutdown","name":"iPhone 15"}],
        "com.apple.CoreSimulator.SimRuntime.iOS-18-6":[],
        "com.apple.CoreSimulator.SimRuntime.watchOS-26-0":[
            {"udid":"WATCH","isAvailable":true,"state":"Booted","name":"Apple Watch Ultra 3"}]}}"#;

    /// Trimmed from Xcode 27.2's `devicectl list devices --json-output` (jsonVersion 5): a simulator and a phone.
    const DEVICECTL: &str = r#"{"info":{"jsonVersion":5,"outcome":"success"},"result":{"devices":[
        {"identifier":"CB81","properties":{"hardware":{"platform":"iOS","reality":"simulated","udid":"CB81"},
            "state":{"bootState":"shutdown","name":"iPad Air 11-inch (M4)"}}},
        {"identifier":"4F2A-UUID","properties":{"hardware":{"platform":"iOS","reality":"physical","udid":"00008112-AB"},
            "state":{"name":"Ali's iPad"}}},
        {"identifier":"TV","properties":{"hardware":{"platform":"tvOS","reality":"physical","udid":"TVUDID"},
            "state":{"name":"Living Room"}}}]}}"#;

    #[test]
    fn listing_table() -> Result<(), CliError> {
        let sims: Vec<(String, String, bool)> = simulators(SIMCTL.as_bytes())?
            .into_iter()
            .map(|c| (c.name, c.udid, c.ready))
            .collect();
        assert_eq!(
            sims,
            [
                ("iPhone 17 Pro".to_string(), "B03F".to_string(), false),
                (
                    "iPad Pro 13-inch (M5)".to_string(),
                    "9267".to_string(),
                    true
                ),
            ],
            "iOS runtimes only, available only, Booted is ready"
        );
        assert_eq!(
            devices(DEVICECTL.as_bytes())?,
            [Candidate {
                name: "Ali's iPad".into(),
                udid: "00008112-AB".into(),
                id: "4F2A-UUID".into(),
                ready: true,
            }],
            "physical iOS devices only"
        );
        assert_eq!(
            simulators(b"not json").map_err(|e| e.variant()),
            Err("ToolOutput".to_string())
        );
        Ok(())
    }

    #[test]
    fn pick_table() {
        let candidate = |name: &str, udid: &str, ready: bool| Candidate {
            name: name.into(),
            udid: udid.into(),
            id: format!("id-{udid}"),
            ready,
        };
        let none_booted = || {
            vec![
                candidate("iPhone 17", "A", false),
                candidate("iPad", "B", false),
            ]
        };
        let one_booted = || {
            vec![
                candidate("iPhone 17", "A", false),
                candidate("iPad", "B", true),
            ]
        };
        let two_booted = || {
            vec![
                candidate("iPhone 17", "A", true),
                candidate("iPad", "B", true),
            ]
        };
        // (row name, candidates, requested, expected udid or error variant)
        type Case = (
            &'static str,
            Vec<Candidate>,
            Option<&'static str>,
            Result<&'static str, &'static str>,
        );
        let cases: [Case; 9] = [
            ("one booted", one_booted(), None, Ok("B")),
            ("none booted", none_booted(), None, Err("NoIosTarget")),
            ("nothing at all", vec![], None, Err("NoIosTarget")),
            ("two booted", two_booted(), None, Err("SeveralIosTargets")),
            (
                "by name, not booted",
                none_booted(),
                Some("iPhone 17"),
                Ok("A"),
            ),
            ("by udid", two_booted(), Some("B"), Ok("B")),
            ("by devicectl id", two_booted(), Some("id-A"), Ok("A")),
            (
                "unknown name",
                one_booted(),
                Some("iPhone 4"),
                Err("IosTargetNotFound"),
            ),
            (
                "name is exact",
                one_booted(),
                Some("iphone 17"),
                Err("IosTargetNotFound"),
            ),
        ];
        let row = destination_row(Destination::Simulator);
        for (name, candidates, requested, want) in cases {
            let got = pick(candidates, row, requested)
                .map(|c| c.udid)
                .map_err(|e| e.variant());
            assert_eq!(got, want.map(String::from).map_err(String::from), "{name}");
        }
    }

    #[test]
    fn templates_have_no_unfilled_placeholders() {
        let config = Config {
            name: "Tom & Jerry".into(),
            id: "a.b".into(),
            icon: None,
            android: crate::config::Android {
                min_sdk: 28,
                target_sdk: 35,
                abis: vec![],
            },
            ios: crate::config::Ios {
                min_ios: "17.0".into(),
                team: None,
            },
        };
        let vars = project_vars(&config);
        for (path, text) in &PROJECT_FILES {
            let out = render(text, &vars);
            assert!(!out.contains("{{"), "{path} keeps a placeholder");
            assert!(
                !out.contains("Tom & Jerry"),
                "{path} has the app name unescaped"
            );
        }
        let pbxproj = render(PROJECT_FILES[0].1, &vars);
        assert!(
            pbxproj.matches("TARGETED_DEVICE_FAMILY = \"1,2\";").count() == 2,
            "iPhone and iPad in both configurations: an iPad runs the app natively, not scaled"
        );
        assert!(pbxproj.contains("IPHONEOS_DEPLOYMENT_TARGET = 17.0;"));
        assert!(pbxproj.contains("PRODUCT_BUNDLE_IDENTIFIER = a.b;"));
    }
}
