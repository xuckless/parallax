# cargo parallax on Android

`cargo parallax run android [--release] [--device <serial>]` builds the app crate, wraps it in a generated Gradle
project, installs the APK and launches it; `build android [--release] [--apk|--aab]` stops after Gradle and prints the
package's path. `--release` means release-signed and R8-shrunk: [cli-android-release.md](cli-android-release.md).
Source: [android.rs](../crates/cli/src/android.rs), [templates/android/](../crates/cli/templates/android/). Overview
and `[app]` keys: [cli.md](cli.md).

## `[android]` keys

Defaults: `ANDROID_DEFAULTS` in [config.rs](../crates/cli/src/config.rs).

| Key | Default | Meaning |
|---|---|---|
| `android.min_sdk` | `28` | `minSdk`; below 28 is an error ([AGENTS.md](../AGENTS.md), Minimum OS) |
| `android.target_sdk` | `35` | `targetSdk` and `compileSdk`; must be ≥ `min_sdk` |
| `android.abis` | `["arm64-v8a", "x86_64"]` | Rows of `ABIS` in [android.rs](../crates/cli/src/android.rs) (`arm64-v8a`, `armeabi-v7a`, `x86`, `x86_64`) |
| `android.version_code` | `1` | `versionCode`, `1..=2100000000` (Google Play's range). `versionName` is `app.version`'s build name |
| `android.release.keystore`, `.key_alias` | none | The upload key, needed for `--release`: [cli-android-release.md](cli-android-release.md) |

`app.icon` becomes `mipmap-<density>/ic_launcher.png` at 48dp (`DENSITIES` in [icon.rs](../crates/cli/src/icon.rs))
and an adaptive icon in `mipmap-anydpi-v26/` (background: the SVG's top-left pixel colour, white if not opaque;
foreground: the whole SVG in the centre 72dp of the 108dp layer).

## What `run android` does

`build android` skips steps 2 and 7.

1. Reads `ANDROID_HOME`, finds the NDK and its `Pkg.Revision`, checks `platform-tools/adb`.
2. Picks the device before the long build: `--device` must be online in `adb devices`; without it exactly one must be.
   With `--release`, the key's passwords are read and checked here too.
3. `cargo metadata`: the app package, its `cdylib` name, the target directory, and the `parallax-android` package,
   whose `java/` holds `ParallaxActivity` and the ported Flutter embedding (path, git or crates.io dependency).
4. One `cargo build --lib --target <t>…` for all ABIs. The linker, `CC_*`, `CXX_*` and `AR_*` point at the NDK's
   `<clang target><min_sdk>-clang` (no cargo-ndk needed). ABI → Rust target → clang is `ABIS`; host OS → NDK layout
   is `HOSTS`.
5. Writes the Gradle project to `<target dir>/parallax/android/<package>/`: `src/` is deleted and every file in
   `PROJECT_FILES` is rewritten; the `.so` files go to `src/main/jniLibs/<abi>/`. Gradle's `build/` and `.gradle/`
   stay, for incremental builds. The project opens in Android Studio as is.
6. Runs the Gradle wrapper (`java -cp gradle/wrapper/gradle-wrapper.jar org.gradle.wrapper.GradleWrapperMain
   <task>`), as `gradlew` would: `assembleDebug`, or the release task. AGP strips the `.so` files with the NDK.
7. `adb -s <serial> install -r` (if that fails, `adb uninstall <app.id>` and install again, as `flutter run` does),
   then `am start -W -n <app.id>/io.github.xuckless.parallax.ParallaxActivity`. App arguments after `--` are
   `--name [value]` pairs, passed as string extras (`--es name value`; an app has no argv on Android).

## Generated project

| File | Notes |
|---|---|
| [AndroidManifest.xml](../crates/cli/templates/android/AndroidManifest.xml) | Launcher activity `ParallaxActivity` with Flutter's `android:configChanges` (rotation reaches `onConfigurationChanged`), `parallax.lib_name` meta-data, `android:icon` only with `app.icon` |
| [build.gradle](../crates/cli/templates/android/build.gradle) | AGP 9.3.1, Java toolchain 17, `ndkPath`/`ndkVersion`, Java sources from `parallax-android/java`, release signing and R8 |
| [proguard-rules.pro](../crates/cli/templates/android/proguard-rules.pro) | Flutter's R8 rules; the JNI keep rules are parallax-android's own |
| [settings.gradle](../crates/cli/templates/android/settings.gradle) | `google()`/`mavenCentral()`; the foojay resolver downloads JDK 17 when none is installed |
| [gradle-wrapper.properties](../crates/cli/templates/android/gradle/wrapper/gradle-wrapper.properties) | Gradle 9.8.2, pinned with `distributionSha256Sum` |
| `gradle-wrapper.jar` | Gradle 9.8.2's wrapper, sha256 `3b8a2577…ce68e98` (`services.gradle.org`); see [NOTICE](../NOTICE) |

## Requirements

| Need | How it is found | If missing |
|---|---|---|
| Android SDK with `platform-tools` | `ANDROID_HOME` (Studio's default is `~/Android/Sdk`) | Set it; `sdkmanager "platform-tools"` |
| Android NDK | `ANDROID_NDK_HOME`, else the only directory in `$ANDROID_HOME/ndk/` | `sdkmanager "ndk;27.1.12297006"` |
| Java 17+ (a JRE is enough) | `$JAVA_HOME/bin/java`, else `java` on `PATH` | Install one; Gradle fetches JDK 17 to compile |
| Rust targets | `rustup` | `rustup target add <rust target>` |
| A device or emulator | `adb devices` | Start one; emulator flags in [android-spike.md](android-spike.md) |

The first run downloads Gradle, AGP and JDK 17 to `~/.gradle`. AGP installs a missing `platforms;android-<target_sdk>`
or build-tools once its licenses are accepted (`sdkmanager --licenses`).
