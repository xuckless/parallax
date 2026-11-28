# Getting started: Android

Shared setup first: [getting-started.md](getting-started.md). Android 9 (API 28) and up. Builds on Linux and macOS.
Reference: [cli.md](cli.md); how the backend works: [android.md](android.md).

## Prerequisites

| Need | Command |
|---|---|
| Android SDK with `platform-tools` | Android Studio, or the command-line tools; then `export ANDROID_HOME=~/Android/Sdk` (macOS: `~/Library/Android/sdk`) |
| NDK | `sdkmanager "ndk;27.1.12297006"`; with several installed, `export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>` |
| Java 17+ (a JRE is enough) | your distro's OpenJDK, or `JAVA_HOME`; Gradle fetches JDK 17 for itself |
| Rust targets | `rustup target add aarch64-linux-android x86_64-linux-android` (listed in this repo's toolchain) |
| SDK licences | `sdkmanager --licenses`, so Gradle may fetch the platform and build tools |
| An emulator or a device | below |

An emulator (x86_64 image, GL first on emulators, [android-spike.md](android-spike.md)):

```sh
sdkmanager "system-images;android-35;google_apis;x86_64" "emulator"
avdmanager create avd -n pixel -k "system-images;android-35;google_apis;x86_64" -d pixel_7
emulator -avd pixel -memory 2048 -cores 2 -gpu host &
```

A phone: turn on USB debugging (Settings → Developer options) and check it shows in `adb devices`.

## Run hello

```sh
cd examples/hello
cargo parallax run android                     # one device online
cargo parallax run android --device emulator-5554
adb logcat -s parallax                          # its log, in another terminal
```

The first run downloads Gradle and the Android Gradle plugin (a few minutes); later runs are incremental. The
generated Gradle project lands in `target/parallax/android/<package>/` and opens in Android Studio as is.

## Keys in parallax.toml

`[android]`: `min_sdk` (28), `target_sdk` (35), `abis` (`["arm64-v8a", "x86_64"]`; one ABI builds faster),
`version_code` (1); `app.version` is the `versionName` (`1.0`). Table: [cli.md](cli.md#parallaxtoml).

## Release builds for Google Play

`cargo parallax build android --release` makes a signed App Bundle (`.aab`); `--apk` a signed universal APK;
`run android --release` installs that APK. Make an upload key once, point `[android.release]` at it, and give the
password in `PARALLAX_KEYSTORE_PASSWORD` or at the prompt; it never goes in `parallax.toml`:

```sh
keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload
```

```toml
[android.release]
keystore = "upload-keystore.jks"   # relative to parallax.toml; keep it out of version control
key_alias = "upload"
```

Details, R8 and checking a bundle: [cli-android-release.md](cli-android-release.md).

## Common errors

| Error says | Fix |
|---|---|
| `ANDROID_HOME is not set` | `export ANDROID_HOME=…` (above) |
| `no NDK under …` / `several NDKs under …` | `sdkmanager "ndk;27.1.12297006"` / set `ANDROID_NDK_HOME` |
| `no Android device online` | start the emulator or plug in the phone; `adb devices` must list it |
| `several Android devices online` | `--device <serial>` from `adb devices` |
| `package … has no cdylib target` | `[lib] crate-type = ["cdylib", "rlib"]` in the app's `Cargo.toml` |
| `does not depend on parallax-android` | depend on `parallax` (it pulls parallax-android in) |
| `app.id … is not an Android application id` | two or more dot-separated parts, each a letter then letters, digits or `_` |
| `android.min_sdk … is below 28` | Parallax supports API 28 and up |
| `parallax.toml has no [android.release]` | the release setup above, or drop `--release` |
| `the password is wrong` | check `PARALLAX_KEYSTORE_PASSWORD`, or unset it to be asked |
| `gradle … failed` | Gradle's own message is above it; a first run needs the network |
