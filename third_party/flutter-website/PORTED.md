# Code adapted from Flutter's documentation

Flutter's documentation site ([flutter/website](https://github.com/flutter/website)) licenses its code samples under
a BSD 3-Clause licence and its prose under CC BY 3.0 (see `LICENSE` here, its `LICENSE`). Only code samples are
adapted; `cargo-parallax` ships the licence as `LICENSE-FLUTTER-WEBSITE`. Every file that adapts a sample gets one row
(AGENTS.md porting rules 3, 4 and 10 apply as for Flutter). Pinned commit: `7c32e59eb1a26ff5f1e510230dea96c33b5f168a`.

Sync rule: at each Flutter stable bump (third_party/flutter/PORTED.md), diff each source page between the pinned commit
and the site's `main`, and port changes to the samples.

| Website path | Commit | Parallax path | Kind (port / oracle) | Dropped parts |
|---|---|---|---|---|
| `sites/docs/src/content/deployment/android.md` | `7c32e59` | `crates/cli/templates/android/build.gradle` (`signingConfigs.release`, the release `signingConfig`), `crates/cli/src/error.rs` (`KEYTOOL_GENKEY`) | port (Groovy sample) | the `key.properties` file and the code that loads it: alias and keystore path come from `parallax.toml`'s `[android.release]`, the passwords from the environment of the Gradle run (`System.getenv`); the Kotlin DSL variant; `~/` in the `keytool -genkey` command (the keystore goes next to `parallax.toml`) |
