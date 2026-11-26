//! `cargo parallax run android`: NDK build of the app's cdylib, a generated Gradle project, adb install and launch.
//! Step by step in docs/cli.md. Decisions are pure functions over plain values; `run` is the thin shell.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::config::Config;
use crate::error::CliError;

/// One Android ABI. `clang_target` is the NDK's clang prefix (it differs from the Rust target only for armv7).
#[derive(Debug, PartialEq)]
pub struct Abi {
    /// Name in `parallax.toml` and in `jniLibs/<name>/`.
    pub name: &'static str,
    pub rust_target: &'static str,
    pub clang_target: &'static str,
}

/// ABI → Rust target → NDK clang. To add an ABI: add a row (and `rustup target add` its Rust target).
pub const ABIS: [Abi; 4] = [
    Abi {
        name: "arm64-v8a",
        rust_target: "aarch64-linux-android",
        clang_target: "aarch64-linux-android",
    },
    Abi {
        name: "armeabi-v7a",
        rust_target: "armv7-linux-androideabi",
        clang_target: "armv7a-linux-androideabi",
    },
    Abi {
        name: "x86",
        rust_target: "i686-linux-android",
        clang_target: "i686-linux-android",
    },
    Abi {
        name: "x86_64",
        rust_target: "x86_64-linux-android",
        clang_target: "x86_64-linux-android",
    },
];

/// The machine running the build, as the NDK and SDK lay out their binaries.
pub struct Host {
    /// `std::env::consts::OS`.
    pub os: &'static str,
    /// Directory under the NDK's `toolchains/llvm/prebuilt/`; the NDK ships x86_64 host binaries only.
    ndk_tag: &'static str,
    /// Suffix of the NDK's per-API clang wrappers (`.cmd` scripts on Windows).
    clang_suffix: &'static str,
    /// Suffix of real executables (`llvm-ar`, `adb`).
    exe_suffix: &'static str,
}

/// Host OS → NDK and SDK binary layout. To add a host: add a row.
pub const HOSTS: [Host; 3] = [
    Host {
        os: "linux",
        ndk_tag: "linux-x86_64",
        clang_suffix: "",
        exe_suffix: "",
    },
    Host {
        os: "macos",
        ndk_tag: "darwin-x86_64",
        clang_suffix: "",
        exe_suffix: "",
    },
    Host {
        os: "windows",
        ndk_tag: "windows-x86_64",
        clang_suffix: ".cmd",
        exe_suffix: ".exe",
    },
];

/// The launcher Activity, shipped as Java by the parallax-android crate.
const ACTIVITY: &str = "io.github.xuckless.parallax.ParallaxActivity";

/// The crate whose `java/` directory holds the Activity and the ported Flutter embedding.
const ANDROID_CRATE: &str = "parallax-android";

enum Content {
    Text(&'static str),
    Binary(&'static [u8]),
}

/// Files of the generated Gradle project, written fresh every run. `{{key}}` in text files is filled from
/// `project_vars`. To add a file: put it under `templates/android/` and add a row.
const PROJECT_FILES: [(&str, Content); 5] = [
    (
        "settings.gradle",
        Content::Text(include_str!("../templates/android/settings.gradle")),
    ),
    (
        "build.gradle",
        Content::Text(include_str!("../templates/android/build.gradle")),
    ),
    (
        "src/main/AndroidManifest.xml",
        Content::Text(include_str!("../templates/android/AndroidManifest.xml")),
    ),
    (
        "gradle/wrapper/gradle-wrapper.properties",
        Content::Text(include_str!(
            "../templates/android/gradle/wrapper/gradle-wrapper.properties"
        )),
    ),
    // Gradle 9.8.2's wrapper, sha256 3b8a2577…ce68e98 (services.gradle.org/distributions/gradle-9.8.2-wrapper.jar.sha256).
    (
        "gradle/wrapper/gradle-wrapper.jar",
        Content::Binary(include_bytes!(
            "../templates/android/gradle/wrapper/gradle-wrapper.jar"
        )),
    ),
];

/// Cargo profile → `cargo build` flag and output directory.
#[derive(Clone, Copy)]
pub enum Profile {
    Debug,
    Release,
}

impl Profile {
    fn cargo_args(self) -> &'static [&'static str] {
        match self {
            Profile::Debug => &[],
            Profile::Release => &["--release"],
        }
    }

    pub fn dir(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Release => "release",
        }
    }
}

pub struct RunArgs {
    pub profile: Profile,
    pub device: Option<String>,
}

/// What `run` needs to know about the app crate, from `cargo metadata`.
struct AppCrate {
    package: String,
    lib: String,
    target_dir: PathBuf,
    java_dir: PathBuf,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<MetaPackage>,
    target_directory: PathBuf,
}

#[derive(Deserialize)]
struct MetaPackage {
    name: String,
    manifest_path: PathBuf,
    targets: Vec<MetaTarget>,
}

#[derive(Deserialize)]
struct MetaTarget {
    name: String,
    crate_types: Vec<String>,
}

pub fn run(app_dir: &Path, config: &Config, args: &RunArgs) -> Result<(), CliError> {
    let host = HOSTS
        .iter()
        .find(|h| h.os == std::env::consts::OS)
        .ok_or(CliError::UnsupportedHost(std::env::consts::OS))?;
    let sdk = PathBuf::from(std::env::var_os("ANDROID_HOME").ok_or(CliError::EnvMissing {
        var: "ANDROID_HOME",
        what: "the Android SDK directory (Android Studio's default is ~/Android/Sdk on Linux, ~/Library/Android/sdk on macOS)",
    })?);
    let sdk = existing(
        sdk,
        "ANDROID_HOME",
        "Point ANDROID_HOME at the Android SDK directory",
    )?;
    let ndk = find_ndk(&sdk)?;
    // AGP wants the NDK's version next to its path, or it warns and skips stripping the .so files.
    let props = ndk.join("source.properties");
    let ndk_version = std::fs::read_to_string(&props)
        .ok()
        .and_then(|text| ndk_revision(&text))
        .ok_or(CliError::PathMissing {
            what: "NDK Pkg.Revision",
            path: props,
            fix: "Reinstall the NDK, or point ANDROID_NDK_HOME at a complete one",
        })?;
    let adb = existing(
        sdk.join("platform-tools")
            .join(format!("adb{}", host.exe_suffix)),
        "adb",
        ADB_FIX,
    )?;
    let toolchain = existing(
        ndk.join("toolchains/llvm/prebuilt")
            .join(host.ndk_tag)
            .join("bin"),
        "NDK LLVM toolchain",
        "Reinstall the NDK, or point ANDROID_NDK_HOME at a complete one",
    )?;

    // Fail on a missing device before the long build.
    let devices = capture(Command::new(&adb).arg("devices"), "adb devices", ADB_FIX)?;
    let serial = pick_device(
        &online_devices(&String::from_utf8_lossy(&devices)),
        args.device.as_deref(),
    )?;

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = app_dir.join("Cargo.toml");
    let metadata = capture(
        Command::new(&cargo)
            .args(["metadata", "--format-version", "1", "--manifest-path"])
            .arg(&manifest),
        "cargo metadata",
        CARGO_FIX,
    )?;
    let app = app_crate(&metadata, &manifest)?;

    let abis = &config.android.abis;
    let mut build = Command::new(&cargo);
    build
        .args(["build", "--lib", "--manifest-path"])
        .arg(&manifest)
        .args(args.profile.cargo_args());
    for abi in abis {
        build.args(["--target", abi.rust_target]);
    }
    build.envs(ndk_env(&toolchain, host, abis, config.android.min_sdk));
    let targets: Vec<&str> = abis.iter().map(|a| a.rust_target).collect();
    step(
        &mut build,
        CARGO_FIX,
        "cargo build",
        format!(
            "If a target is missing: rustup target add {}",
            targets.join(" ")
        ),
    )?;

    let project = app.target_dir.join("parallax/android").join(&app.package);
    let src = project.join("src");
    if src.exists() {
        std::fs::remove_dir_all(&src).map_err(|source| CliError::Io {
            path: src.clone(),
            source,
        })?;
    }
    for (path, content) in &PROJECT_FILES {
        let bytes = match content {
            Content::Text(text) => {
                render(text, &project_vars(config, &app, &ndk, &ndk_version)).into_bytes()
            }
            Content::Binary(bytes) => bytes.to_vec(),
        };
        write(&project.join(path), &bytes)?;
    }
    let so_name = format!("lib{}.so", app.lib);
    for abi in abis {
        let from = app
            .target_dir
            .join(abi.rust_target)
            .join(args.profile.dir())
            .join(&so_name);
        let to = src.join("main/jniLibs").join(abi.name).join(&so_name);
        let bytes = std::fs::read(&from).map_err(|source| CliError::Io {
            path: from.clone(),
            source,
        })?;
        write(&to, &bytes)?;
    }
    println!("cargo parallax: Gradle project in {}", project.display());

    // The wrapper jar finds gradle-wrapper.properties next to itself; this is what `gradlew` runs.
    let java = match std::env::var_os("JAVA_HOME") {
        Some(home) => PathBuf::from(home)
            .join("bin")
            .join(format!("java{}", host.exe_suffix)),
        None => PathBuf::from("java"),
    };
    step(
        Command::new(&java)
            .arg("-cp")
            .arg(project.join("gradle/wrapper/gradle-wrapper.jar"))
            .arg("org.gradle.wrapper.GradleWrapperMain")
            .args(["--console=plain", "assembleDebug"])
            .current_dir(&project)
            .env("ANDROID_HOME", &sdk),
        JAVA_FIX,
        "gradle assembleDebug",
        format!(
            "See Gradle's output above; the project is in {}",
            project.display()
        ),
    )?;

    let apk = project.join(format!("build/outputs/apk/debug/{}-debug.apk", app.package));
    step(
        Command::new(&adb)
            .args(["-s", &serial, "install", "-r"])
            .arg(&apk),
        ADB_FIX,
        "adb install",
        String::new(),
    )?;
    let component = format!("{}/{ACTIVITY}", config.id);
    step(
        Command::new(&adb).args([
            "-s", &serial, "shell", "am", "start", "-W", "-n", &component,
        ]),
        ADB_FIX,
        "adb shell am start",
        String::new(),
    )?;
    println!("cargo parallax: launched {component} on {serial}. Logs: adb -s {serial} logcat");
    Ok(())
}

const ADB_FIX: &str = "Install the SDK platform-tools: sdkmanager \"platform-tools\"";
const CARGO_FIX: &str = "Install Rust from https://rustup.rs";
const JAVA_FIX: &str = "Install a JDK or JRE 17 or newer and put java on PATH, or set JAVA_HOME";

/// `$ANDROID_NDK_HOME`, else the only NDK under `$ANDROID_HOME/ndk/`. Several there is an error, not a guess.
fn find_ndk(sdk: &Path) -> Result<PathBuf, CliError> {
    if let Some(ndk) = std::env::var_os("ANDROID_NDK_HOME") {
        return existing(
            PathBuf::from(ndk),
            "ANDROID_NDK_HOME",
            "Point ANDROID_NDK_HOME at an installed NDK",
        );
    }
    let dir = sdk.join("ndk");
    let mut versions: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|_| CliError::NdkMissing(dir.clone()))?
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    match versions.len() {
        0 => Err(CliError::NdkMissing(dir)),
        1 => Ok(dir.join(versions.remove(0))),
        _ => Err(CliError::NdkAmbiguous { dir, versions }),
    }
}

fn existing(path: PathBuf, what: &'static str, fix: &'static str) -> Result<PathBuf, CliError> {
    match path.exists() {
        true => Ok(path),
        false => Err(CliError::PathMissing { what, path, fix }),
    }
}

/// `Pkg.Revision` from the NDK's `source.properties`.
fn ndk_revision(source_properties: &str) -> Option<String> {
    source_properties.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "Pkg.Revision").then(|| value.trim().to_string())
    })
}

/// Linker and C toolchain env for every ABI, so `cargo build` and `cc`-based build scripts use the NDK's clang
/// for API `min_sdk`.
fn ndk_env(toolchain: &Path, host: &Host, abis: &[&Abi], min_sdk: u32) -> Vec<(String, PathBuf)> {
    let ar = toolchain.join(format!("llvm-ar{}", host.exe_suffix));
    abis.iter()
        .flat_map(|abi| {
            let clang = format!("{}{min_sdk}-clang", abi.clang_target);
            let cc = toolchain.join(format!("{clang}{}", host.clang_suffix));
            let cxx = toolchain.join(format!("{clang}++{}", host.clang_suffix));
            let env_target = abi.rust_target.replace('-', "_");
            [
                (
                    format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
                    cc.clone(),
                ),
                (format!("CC_{env_target}"), cc),
                (format!("CXX_{env_target}"), cxx),
                (format!("AR_{env_target}"), ar.clone()),
            ]
        })
        .collect()
}

/// Serials in state `device` from `adb devices` output (skips `offline`, `unauthorized`).
fn online_devices(adb_devices: &str) -> Vec<String> {
    adb_devices
        .lines()
        .skip(1)
        .filter_map(
            |line| match line.split_whitespace().collect::<Vec<_>>()[..] {
                [serial, "device"] => Some(serial.to_string()),
                _ => None,
            },
        )
        .collect()
}

/// `--device` must be online; without it, exactly one device must be.
fn pick_device(online: &[String], requested: Option<&str>) -> Result<String, CliError> {
    match (requested, online) {
        (Some(serial), _) if online.iter().any(|s| s == serial) => Ok(serial.to_string()),
        (Some(serial), _) => Err(CliError::DeviceNotFound {
            serial: serial.to_string(),
            online: online.to_vec(),
        }),
        (None, []) => Err(CliError::NoDevice),
        (None, [one]) => Ok(one.clone()),
        (None, _) => Err(CliError::SeveralDevices(online.to_vec())),
    }
}

fn app_crate(metadata: &[u8], manifest: &Path) -> Result<AppCrate, CliError> {
    let metadata: Metadata =
        serde_json::from_slice(metadata).map_err(|e| CliError::Metadata(e.to_string()))?;
    // cargo metadata reports canonical manifest paths.
    let manifest = manifest.canonicalize().map_err(|source| CliError::Io {
        path: manifest.to_path_buf(),
        source,
    })?;
    let package = metadata
        .packages
        .iter()
        .find(|p| p.manifest_path == manifest)
        .ok_or_else(|| CliError::NoAppPackage(manifest.clone()))?;
    let lib = package
        .targets
        .iter()
        .find(|t| t.crate_types.iter().any(|c| c == "cdylib"))
        .ok_or_else(|| CliError::NotCdylib(package.name.clone()))?;
    let android = metadata
        .packages
        .iter()
        .find(|p| p.name == ANDROID_CRATE)
        .ok_or_else(|| CliError::NoParallaxAndroid(package.name.clone()))?;
    let java_dir = android
        .manifest_path
        .parent()
        .unwrap_or(Path::new(""))
        .join("java");
    Ok(AppCrate {
        package: package.name.clone(),
        lib: lib.name.replace('-', "_"),
        target_dir: metadata.target_directory,
        java_dir,
    })
}

/// `{{key}}` values for the templates, escaped for the one file each lands in.
fn project_vars(
    config: &Config,
    app: &AppCrate,
    ndk: &Path,
    ndk_version: &str,
) -> Vec<(&'static str, String)> {
    // Groovy single-quoted strings: forward slashes keep Windows paths free of escapes.
    let groovy_path = |p: &Path| p.to_string_lossy().replace('\\', "/").replace('\'', "\\'");
    vec![
        ("name", xml_escape(&config.name)),
        ("id", config.id.clone()),
        ("lib", app.lib.clone()),
        ("project", app.package.clone()),
        ("min_sdk", config.android.min_sdk.to_string()),
        ("target_sdk", config.android.target_sdk.to_string()),
        ("java_dir", groovy_path(&app.java_dir)),
        ("ndk_dir", groovy_path(ndk)),
        ("ndk_version", ndk_version.to_string()),
    ]
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn render(template: &str, vars: &[(&str, String)]) -> String {
    vars.iter()
        .fold(template.to_string(), |text, (key, value)| {
            text.replace(&format!("{{{{{key}}}}}"), value)
        })
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let io = |source| CliError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(path, bytes).map_err(io)
}

fn spawn_error(cmd: &Command, fix: &'static str, source: std::io::Error) -> CliError {
    CliError::ToolMissing {
        tool: cmd.get_program().to_string_lossy().into_owned(),
        fix,
        source,
    }
}

/// Runs `cmd`, returning stdout; stderr goes to the terminal.
fn capture(cmd: &mut Command, step: &'static str, fix: &'static str) -> Result<Vec<u8>, CliError> {
    let out = cmd
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| spawn_error(cmd, fix, e))?;
    match out.status.success() {
        true => Ok(out.stdout),
        false => Err(CliError::StepFailed {
            step,
            status: out.status,
            hint: String::new(),
        }),
    }
}

/// Runs `cmd` with its output on the terminal.
fn step(
    cmd: &mut Command,
    fix: &'static str,
    step: &'static str,
    hint: String,
) -> Result<(), CliError> {
    let status = cmd.status().map_err(|e| spawn_error(cmd, fix, e))?;
    match status.success() {
        true => Ok(()),
        false => Err(CliError::StepFailed { step, status, hint }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_table() {
        // (name, rust target, clang target): the NDK names armv7's clang `armv7a-…`, Rust names it `armv7-…`.
        let cases = [
            (
                "arm64-v8a",
                "aarch64-linux-android",
                "aarch64-linux-android",
            ),
            (
                "armeabi-v7a",
                "armv7-linux-androideabi",
                "armv7a-linux-androideabi",
            ),
            ("x86", "i686-linux-android", "i686-linux-android"),
            ("x86_64", "x86_64-linux-android", "x86_64-linux-android"),
        ];
        assert_eq!(ABIS.len(), cases.len(), "every ABI row has a case");
        for (abi, case) in ABIS.iter().zip(cases) {
            assert_eq!((abi.name, abi.rust_target, abi.clang_target), case);
        }
    }

    #[test]
    fn ndk_env_table() {
        let bin = Path::new("/ndk/bin");
        let x86_64 = &ABIS[3];
        let armv7 = &ABIS[1];
        // (row name, host, ABI, expected env)
        type Case<'a> = (&'a str, &'a Host, &'a Abi, [(&'a str, &'a str); 4]);
        let cases: [Case; 2] = [
            (
                "linux x86_64",
                &HOSTS[0],
                x86_64,
                [
                    (
                        "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER",
                        "/ndk/bin/x86_64-linux-android28-clang",
                    ),
                    (
                        "CC_x86_64_linux_android",
                        "/ndk/bin/x86_64-linux-android28-clang",
                    ),
                    (
                        "CXX_x86_64_linux_android",
                        "/ndk/bin/x86_64-linux-android28-clang++",
                    ),
                    ("AR_x86_64_linux_android", "/ndk/bin/llvm-ar"),
                ],
            ),
            (
                "windows armv7",
                &HOSTS[2],
                armv7,
                [
                    (
                        "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER",
                        "/ndk/bin/armv7a-linux-androideabi28-clang.cmd",
                    ),
                    (
                        "CC_armv7_linux_androideabi",
                        "/ndk/bin/armv7a-linux-androideabi28-clang.cmd",
                    ),
                    (
                        "CXX_armv7_linux_androideabi",
                        "/ndk/bin/armv7a-linux-androideabi28-clang++.cmd",
                    ),
                    ("AR_armv7_linux_androideabi", "/ndk/bin/llvm-ar.exe"),
                ],
            ),
        ];
        for (name, host, abi, want) in cases {
            let got = ndk_env(bin, host, &[abi], 28);
            let want: Vec<(String, PathBuf)> = want
                .iter()
                .map(|(k, v)| (k.to_string(), PathBuf::from(v)))
                .collect();
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn device_table() {
        let adb = "List of devices attached\nemulator-5554\tdevice\nemulator-5556\tdevice\nR5CT\tunauthorized\nX\toffline\n\n";
        assert_eq!(
            online_devices(adb),
            ["emulator-5554", "emulator-5556"],
            "online_devices skips non-device states"
        );
        let one = vec!["emulator-5556".to_string()];
        let two = online_devices(adb);
        // (row name, online serials, --device, expected serial or error variant)
        type Case<'a> = (
            &'a str,
            &'a [String],
            Option<&'a str>,
            Result<&'a str, &'a str>,
        );
        let cases: [Case; 6] = [
            ("none online", &[], None, Err("NoDevice")),
            ("one online", &one, None, Ok("emulator-5556")),
            ("two online, none picked", &two, None, Err("SeveralDevices")),
            (
                "two online, one picked",
                &two,
                Some("emulator-5556"),
                Ok("emulator-5556"),
            ),
            ("picked one offline", &two, Some("X"), Err("DeviceNotFound")),
            (
                "picked with none online",
                &[],
                Some("emulator-5556"),
                Err("DeviceNotFound"),
            ),
        ];
        for (name, online, requested, want) in cases {
            let got = pick_device(online, requested).map_err(|e| e.variant());
            assert_eq!(got, want.map(String::from).map_err(String::from), "{name}");
        }
    }

    #[test]
    fn ndk_revision_table() {
        let cases = [
            (
                "NDK 27 file",
                "Pkg.Desc = Android NDK\nPkg.Revision = 27.1.12297006\nPkg.BaseRevision = 27.1.12297006\n",
                Some("27.1.12297006"),
            ),
            ("no revision", "Pkg.Desc = Android NDK\n", None),
            ("empty", "", None),
        ];
        for (name, text, want) in cases {
            assert_eq!(ndk_revision(text).as_deref(), want, "{name}");
        }
    }

    #[test]
    fn templates_have_no_unfilled_placeholders() {
        let config = Config {
            name: "Tom & Jerry".into(),
            id: "a.b".into(),
            android: crate::config::Android {
                min_sdk: 28,
                target_sdk: 35,
                abis: vec![&ABIS[3]],
            },
        };
        let app = AppCrate {
            package: "hello".into(),
            lib: "hello".into(),
            target_dir: "/t".into(),
            java_dir: "/android/java".into(),
        };
        let vars = project_vars(&config, &app, Path::new("/ndk"), "27.1.12297006");
        for (path, content) in &PROJECT_FILES {
            if let Content::Text(text) = content {
                let out = render(text, &vars);
                assert!(!out.contains("{{"), "{path} keeps a placeholder");
                assert!(
                    !out.contains("Tom & Jerry"),
                    "{path} has the app name unescaped"
                );
            }
        }
    }
}
