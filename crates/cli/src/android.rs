//! `cargo parallax run android` / `build android`: NDK build of the app's cdylib, a generated Gradle project, then adb
//! install and launch (`run`) or the APK / App Bundle alone (`build`, release-signed with `--release`). Step by step in
//! docs/cli-android.md and docs/cli-android-release.md. Decisions are pure functions over plain values; `run`, `build` and
//! `package` are the thin shell.
//! Task naming and the install retry are ported from flutter/flutter packages/flutter_tools/lib/src/android/gradle.dart
//! and android_device.dart @ 3.50.1. Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cargo::{self, CARGO_FIX, MetaPackage, Metadata, Profile, capture, spawn_error, step};
use crate::config::Config;
use crate::error::CliError;
use crate::template::{self, render, write, xml_escape};
use crate::{icon, version};

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
const PROJECT_FILES: [(&str, Content); 6] = [
    (
        "settings.gradle",
        Content::Text(include_str!("../templates/android/settings.gradle")),
    ),
    (
        "build.gradle",
        Content::Text(include_str!("../templates/android/build.gradle")),
    ),
    (
        "proguard-rules.pro",
        Content::Text(include_str!("../templates/android/proguard-rules.pro")),
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

/// What Gradle packages the app as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Package {
    /// Installs on a device; `--apk` for release is a signed universal APK.
    Apk,
    /// Android App Bundle, what Google Play takes.
    Aab,
}

impl Package {
    /// Gradle task prefix, directory under `build/outputs/` and file extension, as gradle.dart names them
    /// (flutter_tools `getAssembleTaskFor`, `getBundleTaskFor`, `getBundleDirectory` @ 3.50.1).
    fn gradle(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Package::Apk => ("assemble", "apk", "apk"),
            Package::Aab => ("bundle", "bundle", "aab"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Verb {
    /// Build, install and launch.
    Run,
    /// Build only.
    Build,
}

/// One `run android` / `build android` invocation, from its flags.
#[derive(Debug, PartialEq)]
struct Args {
    profile: Profile,
    package: Package,
    /// `run` only.
    device: Option<String>,
}

/// `run android [--release] [--device <serial>]`, `build android [--release] [--apk|--aab]`.
fn parse_args(verb: Verb, flags: &[String]) -> Result<Args, CliError> {
    let mut profile = Profile::Debug;
    let mut package = None;
    let mut device = None;
    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        let picked = match (verb, flag.as_str()) {
            (_, "--release") => {
                profile = Profile::Release;
                None
            }
            (Verb::Run, "--device") => {
                let serial = rest
                    .next()
                    .ok_or_else(|| CliError::Usage("--device needs a serial".into()))?;
                device = Some(serial.clone());
                None
            }
            (Verb::Build, "--apk") => Some(Package::Apk),
            (Verb::Build, "--aab") => Some(Package::Aab),
            (_, other) => return Err(CliError::Usage(format!("unknown argument {other:?}"))),
        };
        if let Some(picked) = picked {
            if package.is_some_and(|p| p != picked) {
                return Err(CliError::Usage("pick one of --apk and --aab".into()));
            }
            package = Some(picked);
        }
    }
    // `run` installs an APK. `build` makes what Google Play takes for a release, an App Bundle, and an APK for debug.
    let package = package.unwrap_or(match (verb, profile) {
        (Verb::Build, Profile::Release) => Package::Aab,
        (Verb::Build, Profile::Debug) | (Verb::Run, _) => Package::Apk,
    });
    Ok(Args {
        profile,
        package,
        device,
    })
}

/// How Gradle signs the package.
enum Signing {
    /// AGP's debug key (`~/.android/debug.keystore`): installs anywhere, never accepted by a store.
    Debug,
    /// The app's own key from `[android.release]`, checked with keytool before the build.
    Release(Passwords),
}

/// Environment variables the passwords come from, and that the signing Gradle run reads them from (build.gradle).
const KEYSTORE_PASSWORD_ENV: &str = "PARALLAX_KEYSTORE_PASSWORD";
const KEY_PASSWORD_ENV: &str = "PARALLAX_KEY_PASSWORD";

/// No `Debug`: never printed, never written to a file; it reaches keytool and Gradle only through their environment.
struct Passwords {
    keystore: String,
    key: String,
}

/// The keystore password from its env var, else from `prompt` (a terminal is attached); the key password from its
/// env var, else the keystore's (keytool's PKCS12 keystores have one password for both).
fn passwords(
    env: impl Fn(&str) -> Option<String>,
    prompt: Option<impl FnOnce() -> std::io::Result<String>>,
) -> Result<Passwords, CliError> {
    let keystore = match (env(KEYSTORE_PASSWORD_ENV), prompt) {
        (Some(password), _) => password,
        (None, Some(prompt)) => prompt().map_err(|source| CliError::Io {
            path: "/dev/tty".into(),
            source,
        })?,
        (None, None) => return Err(CliError::NoPassword),
    };
    if keystore.is_empty() {
        return Err(CliError::EmptyPassword);
    }
    let key = env(KEY_PASSWORD_ENV).unwrap_or_else(|| keystore.clone());
    Ok(Passwords { keystore, key })
}

/// Builds the error from the keystore path and the key alias.
type KeytoolError = fn(&Path, &str) -> CliError;

/// keytool's failure text → the error shown. To add a case: add a row with a phrase keytool prints for it.
const KEYTOOL_ERRORS: [(&str, KeytoolError); 2] = [
    ("password was incorrect", |keystore, _| {
        CliError::WrongKeystorePassword(keystore.to_path_buf())
    }),
    ("does not exist", |keystore, alias| {
        CliError::AliasNotInKeystore {
            alias: alias.to_string(),
            keystore: keystore.to_path_buf(),
        }
    }),
];

fn keytool_error(output: &str, keystore: &Path, alias: &str) -> CliError {
    KEYTOOL_ERRORS
        .iter()
        .find(|(phrase, _)| output.contains(phrase))
        .map(|(_, error)| error(keystore, alias))
        .unwrap_or_else(|| CliError::KeystoreUnreadable {
            keystore: keystore.to_path_buf(),
            message: output.trim().to_string(),
        })
}

/// Debug: AGP's debug key. Release: the configured key, its passwords asked for and checked (keytool opens the
/// keystore and finds the alias) before the long build, so a typo fails in a second.
fn signing(
    app_dir: &Path,
    config: &Config,
    profile: Profile,
    host: &Host,
) -> Result<Signing, CliError> {
    let key = match profile {
        Profile::Debug => return Ok(Signing::Debug),
        Profile::Release => config
            .android
            .release
            .as_ref()
            .ok_or(CliError::ReleaseNotConfigured)?,
    };
    let keystore = app_dir.join(&key.keystore);
    if !keystore.is_file() {
        return Err(CliError::KeystoreMissing(keystore));
    }
    let prompt = std::io::stdin()
        .is_terminal()
        .then_some(|| rpassword::prompt_password("cargo parallax: keystore password: "));
    let passwords = passwords(|var| std::env::var(var).ok(), prompt)?;
    let mut keytool = Command::new(jdk_tool(host, "keytool"));
    keytool
        .args(["-list", "-keystore"])
        .arg(&keystore)
        .args([
            "-alias",
            &key.key_alias,
            "-storepass:env",
            KEYSTORE_PASSWORD_ENV,
        ])
        .env(KEYSTORE_PASSWORD_ENV, &passwords.keystore);
    let out = keytool
        .output()
        .map_err(|e| spawn_error(&keytool, JAVA_FIX, e))?;
    match out.status.success() {
        true => Ok(Signing::Release(passwords)),
        false => {
            let text = [out.stdout, out.stderr].concat();
            Err(keytool_error(
                &String::from_utf8_lossy(&text),
                &keystore,
                &key.key_alias,
            ))
        }
    }
}

/// What `package` needs to know about the app crate, from `cargo metadata`.
struct AppCrate {
    package: String,
    /// `app.version`, else the package's.
    version: String,
    lib: String,
    target_dir: PathBuf,
    /// The parallax-android crate: its `java/` and `proguard-rules.pro`.
    android_dir: PathBuf,
}

/// The Android SDK, NDK and host layout every Android command needs.
struct Sdk {
    host: &'static Host,
    dir: PathBuf,
    ndk: PathBuf,
    ndk_version: String,
    toolchain: PathBuf,
}

impl Sdk {
    fn find() -> Result<Sdk, CliError> {
        let host = HOSTS
            .iter()
            .find(|h| h.os == std::env::consts::OS)
            .ok_or(CliError::UnsupportedHost(std::env::consts::OS))?;
        let dir = PathBuf::from(std::env::var_os("ANDROID_HOME").ok_or(CliError::EnvMissing {
            var: "ANDROID_HOME",
            what: "the Android SDK directory (Android Studio's default is ~/Android/Sdk on Linux, ~/Library/Android/sdk on macOS)",
        })?);
        let dir = existing(
            dir,
            "ANDROID_HOME",
            "Point ANDROID_HOME at the Android SDK directory",
        )?;
        let ndk = find_ndk(&dir)?;
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
        let toolchain = existing(
            ndk.join("toolchains/llvm/prebuilt")
                .join(host.ndk_tag)
                .join("bin"),
            "NDK LLVM toolchain",
            "Reinstall the NDK, or point ANDROID_NDK_HOME at a complete one",
        )?;
        Ok(Sdk {
            host,
            dir,
            ndk,
            ndk_version,
            toolchain,
        })
    }
}

/// `cargo parallax run android`: build, install, launch.
pub fn run(
    app_dir: &Path,
    config: &Config,
    flags: &[String],
    app: &[String],
) -> Result<(), CliError> {
    if !app.is_empty() {
        // `am start` could pass them as Intent extras, but ParallaxActivity hands the app no arguments.
        return Err(CliError::AppArgsUnsupported("android"));
    }
    let args = parse_args(Verb::Run, flags)?;
    let sdk = Sdk::find()?;
    let adb = existing(
        sdk.dir
            .join("platform-tools")
            .join(format!("adb{}", sdk.host.exe_suffix)),
        "adb",
        ADB_FIX,
    )?;
    // Fail on a missing device before the long build.
    let devices = capture(Command::new(&adb).arg("devices"), "adb devices", ADB_FIX)?;
    let serial = pick_device(
        &online_devices(&String::from_utf8_lossy(&devices)),
        args.device.as_deref(),
    )?;
    let apk = package(app_dir, config, &args, &sdk)?;

    let install = || {
        step(
            Command::new(&adb)
                .args(["-s", &serial, "install", "-r"])
                .arg(&apk),
            ADB_FIX,
            "adb install",
            String::new(),
        )
    };
    // As flutter_tools' AndroidDevice.installApp @ 3.50.1: an install that fails over an installed app (one signed
    // with another key, as after switching between debug and release) uninstalls it and installs again.
    if let Err(error) = install() {
        println!("cargo parallax: Uninstalling old version...");
        let uninstalled = Command::new(&adb)
            .args(["-s", &serial, "uninstall", &config.id])
            .status()
            .is_ok_and(|status| status.success());
        match uninstalled {
            true => install()?,
            false => return Err(error),
        }
    }
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

/// `cargo parallax build android`: build only, and say where the package is.
pub fn build(
    app_dir: &Path,
    config: &Config,
    flags: &[String],
    _: &[String],
) -> Result<(), CliError> {
    let args = parse_args(Verb::Build, flags)?;
    let sdk = Sdk::find()?;
    let out = package(app_dir, config, &args, &sdk)?;
    println!("cargo parallax: built {}", out.display());
    Ok(())
}

/// Builds the Rust library, writes the Gradle project and runs Gradle; returns the APK or App Bundle.
fn package(app_dir: &Path, config: &Config, args: &Args, sdk: &Sdk) -> Result<PathBuf, CliError> {
    // Asked for and checked before the long build, like the icon below.
    let signing = signing(app_dir, config, args.profile, sdk.host)?;
    // Rendered before the long build so a bad SVG fails fast; written with the other project files.
    let icon_files = match &config.icon {
        Some(svg) => icon::launcher_icon(&app_dir.join(svg))?,
        None => Vec::new(),
    };

    let manifest = app_dir.join("Cargo.toml");
    let metadata = cargo::metadata(&manifest)?;
    let app = app_crate(config, &metadata, metadata.app_package(&manifest)?)?;

    let abis = &config.android.abis;
    let mut build = Command::new(cargo::cargo());
    build
        .args(["build", "--lib", "--manifest-path"])
        .arg(&manifest)
        .args(args.profile.cargo_args());
    for abi in abis {
        build.args(["--target", abi.rust_target]);
    }
    build.envs(ndk_env(
        &sdk.toolchain,
        sdk.host,
        abis,
        config.android.min_sdk,
    ));
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
    template::clear(&src)?;
    let vars = project_vars(app_dir, config, &app, &sdk.ndk, &sdk.ndk_version);
    for (path, content) in &PROJECT_FILES {
        let bytes = match content {
            Content::Text(text) => render(text, &vars).into_bytes(),
            Content::Binary(bytes) => bytes.to_vec(),
        };
        write(&project.join(path), &bytes)?;
    }
    for (path, bytes) in &icon_files {
        write(&project.join(path), bytes)?;
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

    let (task, output) = gradle_task(args.profile, args.package, &app.package);
    // The wrapper jar finds gradle-wrapper.properties next to itself; this is what `gradlew` runs.
    let mut gradle = Command::new(jdk_tool(sdk.host, "java"));
    gradle
        .arg("-cp")
        .arg(project.join("gradle/wrapper/gradle-wrapper.jar"))
        .arg("org.gradle.wrapper.GradleWrapperMain")
        .args(["--console=plain", &task])
        .current_dir(&project)
        .env("ANDROID_HOME", &sdk.dir);
    match &signing {
        Signing::Debug => {}
        // A Gradle process of its own that exits with the build, so the passwords never stay in a daemon.
        Signing::Release(passwords) => {
            gradle
                .arg("--no-daemon")
                .env(KEYSTORE_PASSWORD_ENV, &passwords.keystore)
                .env(KEY_PASSWORD_ENV, &passwords.key);
        }
    }
    step(
        &mut gradle,
        JAVA_FIX,
        "gradle",
        format!(
            "See Gradle's output above (a signing failure names the keystore or key); the project is in {}",
            project.display()
        ),
    )?;
    Ok(project.join(output))
}

/// The Gradle task that makes `package` for `profile`, and its output relative to the Gradle project.
fn gradle_task(profile: Profile, package: Package, project: &str) -> (String, String) {
    let (task, outputs, extension) = package.gradle();
    let variant = profile.dir();
    (
        format!("{task}{}", profile.title()),
        format!("build/outputs/{outputs}/{variant}/{project}-{variant}.{extension}"),
    )
}

const ADB_FIX: &str = "Install the SDK platform-tools: sdkmanager \"platform-tools\"";
const JAVA_FIX: &str = "Install a JDK or JRE 17 or newer and put java on PATH, or set JAVA_HOME";

/// `java` or `keytool`: from `$JAVA_HOME/bin`, else from `PATH`.
fn jdk_tool(host: &Host, name: &str) -> PathBuf {
    match std::env::var_os("JAVA_HOME") {
        Some(home) => PathBuf::from(home)
            .join("bin")
            .join(format!("{name}{}", host.exe_suffix)),
        None => PathBuf::from(name),
    }
}

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

fn app_crate(
    config: &Config,
    metadata: &Metadata,
    package: &MetaPackage,
) -> Result<AppCrate, CliError> {
    let android = metadata
        .packages
        .iter()
        .find(|p| p.name == ANDROID_CRATE)
        .ok_or_else(|| CliError::NoParallaxAndroid(package.name.clone()))?;
    let android_dir = android
        .manifest_path
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    Ok(AppCrate {
        package: package.name.clone(),
        version: config.version(&package.version).to_string(),
        lib: package.cdylib()?,
        target_dir: metadata.target_directory.clone(),
        android_dir,
    })
}

/// `{{key}}` values for the templates, escaped for the one file each lands in.
fn project_vars(
    app_dir: &Path,
    config: &Config,
    app: &AppCrate,
    ndk: &Path,
    ndk_version: &str,
) -> Vec<(&'static str, String)> {
    // Groovy single-quoted strings: forward slashes keep Windows paths free of escapes.
    let groovy_path = |p: &Path| groovy_escape(&p.to_string_lossy().replace('\\', "/"));
    let release = config.android.release.as_ref();
    vec![
        ("name", xml_escape(&config.name)),
        (
            "icon",
            match config.icon {
                Some(_) => r#" android:icon="@mipmap/ic_launcher""#.into(),
                None => String::new(),
            },
        ),
        ("id", config.id.clone()),
        ("lib", app.lib.clone()),
        ("project", app.package.clone()),
        ("min_sdk", config.android.min_sdk.to_string()),
        ("target_sdk", config.android.target_sdk.to_string()),
        ("version", groovy_escape(version::build_name(&app.version))),
        ("version_code", config.android.version_code.to_string()),
        // Groovy expressions: `null` leaves the release signing config empty until [android.release] is set.
        (
            "key_alias",
            release.map_or("null".into(), |r| {
                format!("'{}'", groovy_escape(&r.key_alias))
            }),
        ),
        (
            "store_file",
            release.map_or("null".into(), |r| {
                format!("file('{}')", groovy_path(&app_dir.join(&r.keystore)))
            }),
        ),
        ("java_dir", groovy_path(&app.android_dir.join("java"))),
        (
            "android_rules",
            groovy_path(&app.android_dir.join("proguard-rules.pro")),
        ),
        ("ndk_dir", groovy_path(ndk)),
        ("ndk_version", ndk_version.to_string()),
    ]
}

/// Text for inside a Groovy single-quoted string.
fn groovy_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\'', "\\'")
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

    fn config(release: Option<crate::config::ReleaseKey>) -> Config {
        let mut config = Config::parse(
            "[app]\nname = \"Tom & Jerry\"\nid = \"a.b\"\n[android]\nabis = [\"x86_64\"]\n",
        )
        .expect("test parallax.toml");
        config.android.release = release;
        config
    }

    fn app() -> AppCrate {
        AppCrate {
            package: "hello".into(),
            version: "1.2.3+4".into(),
            lib: "hello".into(),
            target_dir: "/t".into(),
            android_dir: "/android".into(),
        }
    }

    #[test]
    fn templates_have_no_unfilled_placeholders() {
        let release = crate::config::ReleaseKey {
            keystore: "keys/upload.jks".into(),
            key_alias: "upload".into(),
        };
        for (row, config) in [
            ("no release key", config(None)),
            ("release key", config(Some(release))),
        ] {
            let vars = project_vars(
                Path::new("/app"),
                &config,
                &app(),
                Path::new("/ndk"),
                "27.1.12297006",
            );
            for (path, content) in &PROJECT_FILES {
                if let Content::Text(text) = content {
                    let out = render(text, &vars);
                    assert!(!out.contains("{{"), "{row}: {path} keeps a placeholder");
                    assert!(
                        !out.contains("Tom & Jerry"),
                        "{row}: {path} has the app name unescaped"
                    );
                }
            }
        }
    }

    #[test]
    fn signing_vars_table() {
        // (row name, [android.release], expected key_alias and store_file Groovy expressions)
        type Case<'a> = (&'a str, Option<(&'a str, &'a str)>, (&'a str, &'a str));
        let cases: [Case; 3] = [
            ("not configured", None, ("null", "null")),
            (
                "relative to parallax.toml",
                Some(("keys/upload.jks", "upload")),
                ("'upload'", "file('/app/keys/upload.jks')"),
            ),
            (
                "quote in alias",
                Some(("/abs/k.jks", "it's")),
                ("'it\\'s'", "file('/abs/k.jks')"),
            ),
        ];
        for (name, release, want) in cases {
            let release = release.map(|(keystore, alias)| crate::config::ReleaseKey {
                keystore: keystore.into(),
                key_alias: alias.into(),
            });
            let vars = project_vars(
                Path::new("/app"),
                &config(release),
                &app(),
                Path::new("/ndk"),
                "27",
            );
            let var = |key| {
                vars.iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.as_str())
            };
            assert_eq!(
                (var("key_alias"), var("store_file")),
                (Some(want.0), Some(want.1)),
                "{name}"
            );
        }
    }

    #[test]
    fn build_gradle_reads_the_password_env_vars() {
        let gradle = include_str!("../templates/android/build.gradle");
        for var in [KEYSTORE_PASSWORD_ENV, KEY_PASSWORD_ENV] {
            assert!(gradle.contains(&format!("System.getenv('{var}')")), "{var}");
        }
    }

    #[test]
    fn parse_args_table() {
        use Package::{Aab, Apk};
        use Profile::{Debug, Release};
        // (row name, verb, flags after the target, expected (profile, package, --device) or a usage error)
        type Case<'a> = (
            &'a str,
            Verb,
            &'a [&'a str],
            Result<(Profile, Package, Option<&'a str>), ()>,
        );
        let cases: [Case; 15] = [
            ("run", Verb::Run, &[], Ok((Debug, Apk, None))),
            (
                "run release",
                Verb::Run,
                &["--release"],
                Ok((Release, Apk, None)),
            ),
            (
                "run on a device",
                Verb::Run,
                &["--device", "emulator-5556"],
                Ok((Debug, Apk, Some("emulator-5556"))),
            ),
            (
                "run device without serial",
                Verb::Run,
                &["--device"],
                Err(()),
            ),
            ("run --aab", Verb::Run, &["--aab"], Err(())),
            ("run unknown flag", Verb::Run, &["--fast"], Err(())),
            ("build: debug APK", Verb::Build, &[], Ok((Debug, Apk, None))),
            (
                "build release: App Bundle",
                Verb::Build,
                &["--release"],
                Ok((Release, Aab, None)),
            ),
            (
                "build release --apk",
                Verb::Build,
                &["--release", "--apk"],
                Ok((Release, Apk, None)),
            ),
            (
                "build --aab --release, either order",
                Verb::Build,
                &["--aab", "--release"],
                Ok((Release, Aab, None)),
            ),
            (
                "build debug --aab",
                Verb::Build,
                &["--aab"],
                Ok((Debug, Aab, None)),
            ),
            (
                "build --apk twice",
                Verb::Build,
                &["--apk", "--apk"],
                Ok((Debug, Apk, None)),
            ),
            (
                "build --apk and --aab",
                Verb::Build,
                &["--apk", "--aab"],
                Err(()),
            ),
            ("build --device", Verb::Build, &["--device", "x"], Err(())),
            ("build unknown flag", Verb::Build, &["--split"], Err(())),
        ];
        for (name, verb, flags, want) in cases {
            let flags: Vec<String> = flags.iter().map(|s| s.to_string()).collect();
            let got = parse_args(verb, &flags)
                .map(|a| (a.profile, a.package, a.device))
                .map_err(|e| e.variant());
            let want = want
                .map(|(profile, package, device)| (profile, package, device.map(String::from)))
                .map_err(|()| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn gradle_task_table() {
        // (profile, package, expected task, expected output): flutter_tools' names for `flutter build apk/appbundle`.
        let cases = [
            (
                Profile::Debug,
                Package::Apk,
                "assembleDebug",
                "build/outputs/apk/debug/hello-debug.apk",
            ),
            (
                Profile::Release,
                Package::Apk,
                "assembleRelease",
                "build/outputs/apk/release/hello-release.apk",
            ),
            (
                Profile::Debug,
                Package::Aab,
                "bundleDebug",
                "build/outputs/bundle/debug/hello-debug.aab",
            ),
            (
                Profile::Release,
                Package::Aab,
                "bundleRelease",
                "build/outputs/bundle/release/hello-release.aab",
            ),
        ];
        for (profile, package, task, output) in cases {
            assert_eq!(
                gradle_task(profile, package, "hello"),
                (task.to_string(), output.to_string()),
                "{profile:?} {package:?}"
            );
        }
    }

    #[test]
    fn passwords_table() {
        // (row name, PARALLAX_KEYSTORE_PASSWORD, PARALLAX_KEY_PASSWORD, what the prompt returns (None: no terminal),
        // expected (keystore password, key password) or error variant)
        type Case<'a> = (
            &'a str,
            Option<&'a str>,
            Option<&'a str>,
            Option<&'a str>,
            Result<(&'a str, &'a str), &'a str>,
        );
        let cases: [Case; 7] = [
            (
                "both from env",
                Some("store"),
                Some("key"),
                None,
                Ok(("store", "key")),
            ),
            (
                "key password defaults to the keystore's",
                Some("store"),
                None,
                None,
                Ok(("store", "store")),
            ),
            (
                "env wins over the prompt",
                Some("store"),
                None,
                Some("typed"),
                Ok(("store", "store")),
            ),
            (
                "prompted on a terminal",
                None,
                None,
                Some("typed"),
                Ok(("typed", "typed")),
            ),
            (
                "prompted, key password from env",
                None,
                Some("key"),
                Some("typed"),
                Ok(("typed", "key")),
            ),
            (
                "no env, no terminal",
                None,
                Some("key"),
                None,
                Err("NoPassword"),
            ),
            ("empty", Some(""), None, None, Err("EmptyPassword")),
        ];
        for (name, store, key, typed, want) in cases {
            let env = |var: &str| match var {
                KEYSTORE_PASSWORD_ENV => store.map(String::from),
                KEY_PASSWORD_ENV => key.map(String::from),
                _ => None,
            };
            let prompt = typed.map(|t| move || Ok(t.to_string()));
            let got = passwords(env, prompt)
                .map(|p| (p.keystore, p.key))
                .map_err(|e| e.variant());
            let want = want
                .map(|(s, k)| (s.to_string(), k.to_string()))
                .map_err(String::from);
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn keytool_error_table() {
        // (row name, keytool's output, expected error variant): messages as keytool (OpenJDK 17–25) prints them.
        let cases = [
            (
                "wrong password",
                "keytool error: java.io.IOException: keystore password was incorrect\n",
                "WrongKeystorePassword",
            ),
            (
                "unknown alias",
                "keytool error: java.lang.Exception: Alias <uplaod> does not exist\n",
                "AliasNotInKeystore",
            ),
            (
                "not a keystore",
                "keytool error: java.io.IOException: Invalid keystore format\n",
                "KeystoreUnreadable",
            ),
        ];
        for (name, output, want) in cases {
            let got = keytool_error(output, Path::new("/k.jks"), "uplaod").variant();
            assert_eq!(got, want, "{name}");
        }
    }
}
