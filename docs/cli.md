# cargo-parallax

`cargo parallax run android [--release] [--device <serial>]` builds the app crate in the current directory, wraps it
in a generated Gradle project, installs the APK and launches it. Source: [main.rs](../crates/cli/src/main.rs) (args),
[config.rs](../crates/cli/src/config.rs) (`parallax.toml`), [android.rs](../crates/cli/src/android.rs) (the run),
[error.rs](../crates/cli/src/error.rs) (every failure and its fix), [templates/android/](../crates/cli/templates/android/).
Install: `cargo install --path crates/cli`.

## parallax.toml

Next to the app's `Cargo.toml`. Unknown keys and tables are errors. Defaults are one table, `ANDROID_DEFAULTS` in
[config.rs](../crates/cli/src/config.rs).

| Key | Required | Default | Meaning |
|---|---|---|---|
| `app.name` | yes | | Launcher label |
| `app.id` | yes | | Android application id and Gradle namespace, e.g. `io.github.xuckless.parallax.hello` |
| `android.min_sdk` | no | `28` | `minSdk`; below 28 is an error ([AGENTS.md](../AGENTS.md), Minimum OS) |
| `android.target_sdk` | no | `35` | `targetSdk` and `compileSdk`; must be ≥ `min_sdk` |
| `android.abis` | no | `["arm64-v8a", "x86_64"]` | ABIs to build; rows of the `ABIS` table in [android.rs](../crates/cli/src/android.rs) (`arm64-v8a`, `armeabi-v7a`, `x86`, `x86_64`) |

There is no `lib` key: the library name comes from the crate's `cdylib` target in `cargo metadata`, so it cannot
drift from `Cargo.toml`.

```toml
[app]
name = "Hello"
id = "io.github.xuckless.parallax.hello"

[android]
abis = ["x86_64"]
```

## What `run android` does

1. Reads `ANDROID_HOME`, finds the NDK and its `Pkg.Revision`, checks `platform-tools/adb`.
2. Picks the device before the long build: `--device` must be online in `adb devices`; without it exactly one must be.
3. `cargo metadata`: the app package (its `Cargo.toml` is in the current directory), its `cdylib` name, the target
   directory, and the `parallax-android` package, whose `java/` directory holds `ParallaxActivity` and the ported
   Flutter embedding (works for a path, git or crates.io dependency).
4. One `cargo build --lib --target <t>…` for all ABIs. The linker, `CC_*`, `CXX_*` and `AR_*` point at the NDK's
   `<clang target><min_sdk>-clang` (env set by the CLI; no cargo-ndk needed). ABI → Rust target → clang is the
   `ABIS` table; host OS → NDK layout is the `HOSTS` table.
5. Writes the Gradle project to `<target dir>/parallax/android/<package>/`: `src/` is deleted and every file in
   `PROJECT_FILES` is rewritten from the templates; the `.so` files go to `src/main/jniLibs/<abi>/`. Gradle's own
   `build/` and `.gradle/` stay, for incremental builds. The project opens in Android Studio as is.
6. Runs the Gradle wrapper (`java -cp gradle/wrapper/gradle-wrapper.jar org.gradle.wrapper.GradleWrapperMain
   assembleDebug`), as `gradlew` would. AGP strips the `.so` files with the NDK.
7. `adb -s <serial> install -r`, then `am start -W -n <app.id>/io.github.xuckless.parallax.ParallaxActivity`.

`--release` builds the Rust library with `--release`; the APK is always the debug variant (debug-signed), so it
installs without a keystore.

## Generated project

| File | Notes |
|---|---|
| [AndroidManifest.xml](../crates/cli/templates/android/AndroidManifest.xml) | Launcher activity `io.github.xuckless.parallax.ParallaxActivity`; `<application>` carries `<meta-data android:name="parallax.lib_name" android:value="<cdylib name>"/>` |
| [build.gradle](../crates/cli/templates/android/build.gradle) | AGP 9.3.1 (Flutter's `test_runner` uses the same), Java toolchain 17, `ndkPath`/`ndkVersion`, Java sources from `parallax-android/java` |
| [settings.gradle](../crates/cli/templates/android/settings.gradle) | `google()`/`mavenCentral()`; the foojay toolchain resolver downloads JDK 17 when none is installed |
| [gradle-wrapper.properties](../crates/cli/templates/android/gradle/wrapper/gradle-wrapper.properties) | Gradle 9.8.2, pinned with `distributionSha256Sum` |
| `gradle-wrapper.jar` | Gradle 9.8.2's wrapper, sha256 `3b8a2577…ce68e98` (matches `services.gradle.org`'s `gradle-9.8.2-wrapper.jar.sha256`); see [NOTICE](../NOTICE) |

## Requirements

| Need | How it is found | If missing |
|---|---|---|
| Android SDK with `platform-tools` | `ANDROID_HOME` (required; Studio's default is `~/Android/Sdk`) | Set `ANDROID_HOME`; `sdkmanager "platform-tools"` |
| Android NDK | `ANDROID_NDK_HOME`, else the only directory in `$ANDROID_HOME/ndk/` (several is an error) | `sdkmanager "ndk;27.1.12297006"` or set `ANDROID_NDK_HOME` |
| Java 17+ (a JRE is enough) | `$JAVA_HOME/bin/java`, else `java` on `PATH` | Install one; Gradle fetches JDK 17 for compiling itself |
| Rust targets | `rustup` (the repo's toolchain lists the Android ones) | `rustup target add <rust target>` |
| Network on first run | Gradle 9.8.2, AGP, JDK 17 download to `~/.gradle` | |
| A device or emulator | `adb devices` | Start one; emulator flags in [android-spike.md](android-spike.md) |

AGP downloads a missing `platforms;android-<target_sdk>` or build-tools into the SDK if its licenses are accepted
(`sdkmanager --licenses`).
