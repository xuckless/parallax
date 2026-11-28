# Android release builds

`cargo parallax build android --release` makes an App Bundle (`.aab`) for Google Play, signed with the app's own
upload key; `--apk` makes a signed universal APK instead. `run android --release` installs and launches that APK.
Debug builds (`run android`, `build android`) stay signed with AGP's debug key and need nothing below. Commands and
flags: [cli.md](cli.md). Source: [android.rs](../crates/cli/src/android.rs) (`signing`, `passwords`, `package`),
[build.gradle](../crates/cli/templates/android/build.gradle).

## Setup

Make an upload key once (the command from Flutter's deployment guide) and keep it out of version control:

```sh
keytool -genkey -v -keystore upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload
```

```toml
[app]
version = "1.2.0"          # versionName, default "1.0"
[android]
version_code = 7           # versionCode, 1..=2100000000, default 1; raise it for every upload
[android.release]
keystore = "upload-keystore.jks"   # relative to parallax.toml (absolute works too)
key_alias = "upload"
```

## Passwords

Never in `parallax.toml` (a `password` key there is an error) and never written to a file.

| Password | Comes from |
|---|---|
| Keystore | `PARALLAX_KEYSTORE_PASSWORD`; unset and stdin is a terminal: asked, not echoed ([rpassword](https://crates.io/crates/rpassword)) |
| Key | `PARALLAX_KEY_PASSWORD`; unset: the keystore's (keytool's PKCS12 keystores have one password) |

Before the long build, `keytool -list -alias <key_alias> -storepass:env PARALLAX_KEYSTORE_PASSWORD` opens the
keystore, so a wrong password or alias fails in a second. Gradle gets both passwords only in its environment
(`System.getenv` in build.gradle's `signingConfigs.release`), and the signing run uses `--no-daemon`, so they leave
with the process instead of staying in a Gradle daemon. Neither appears in the generated project, in `target/`, or in
any log.

## Errors

| Error | Fix it says |
|---|---|
| `ReleaseNotConfigured` | The `keytool -genkey` command and the `[android.release]` table to add; or drop `--release` |
| `KeystoreMissing` | The path is relative to `parallax.toml`; how to create one |
| `ReleaseKeyIncomplete` | `[android.release]` needs both `keystore` and `key_alias` |
| `NoPassword` | Set `PARALLAX_KEYSTORE_PASSWORD`, or run in a terminal |
| `WrongKeystorePassword` | Check `PARALLAX_KEYSTORE_PASSWORD`, or unset it to be asked |
| `AliasNotInKeystore` | Fix `key_alias`; `keytool -list -keystore <path>` lists the aliases |
| Wrong key password | Gradle's own message (`Failed to read key <alias> from store …`), above `gradle failed` |

keytool's messages → errors is the `KEYTOOL_ERRORS` table; add a row for a new phrase.

## What a release build changes

| | Debug | Release |
|---|---|---|
| Rust | `cargo build` | `cargo build --release` |
| Gradle task → output under the project's `build/outputs/` | `assembleDebug` → `apk/debug/<pkg>-debug.apk` | `bundleRelease` → `bundle/release/<pkg>-release.aab`; `--apk`: `assembleRelease` → `apk/release/<pkg>-release.apk` |
| Signing | AGP's debug key | `[android.release]` |
| R8 | off | `minifyEnabled`, `shrinkResources`, `proguard-android-optimize.txt` (as Flutter's Gradle plugin) |

R8 reads three rules files: AGP's optimize defaults, the generated `proguard-rules.pro` (Flutter's
`flutter_proguard_rules.pro`) and parallax-android's [proguard-rules.pro](../crates/android/proguard-rules.pro). The
last keeps all of `io.github.xuckless.parallax`: Rust registers natives and calls methods there by name over JNI,
which R8 cannot see; without it R8 drops `EmojiRasterizer` and the methods Rust calls. `run android --release` over a
debug install (another key) uninstalls the app, data included, and installs again, as `flutter run` does.

## Checking a bundle

Google Play signs the APKs it serves. To try a bundle locally, [bundletool](https://github.com/google/bundletool)
makes a universal APK signed with the same key (`file:<(…)` keeps the password out of `ps`):

```sh
java -jar bundletool.jar build-apks --mode=universal --bundle=<pkg>-release.aab --output=app.apks \
  --ks=upload-keystore.jks --ks-key-alias=upload --ks-pass=file:<(printf %s "$PARALLAX_KEYSTORE_PASSWORD")
java -jar bundletool.jar install-apks --apks=app.apks
```

`apksigner verify --print-certs` (SDK build-tools) shows an APK's signer, `keytool -printcert -jarfile` a bundle's.
Wave 16 checked hello this way on the API 35 emulator: the `.aab`'s universal APK, `--apk` and `run --release` each
passed [android-smoke.md](android-smoke.md)'s nine steps (touch, text input, rotation, lifecycle) after R8; with the
keep rule removed, R8 dropped 69 of parallax-android's 80 classes.
