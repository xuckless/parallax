# Porting Flutter's Android tests

The test stack's versions and the steps for porting one more of Flutter's Java tests into
[the Robolectric project](android-tests.md).

## Versions

From Flutter 3.50.1's [test_runner/build.gradle](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test_runner/build.gradle);
bump them together with the pinned Flutter tag.

| Piece | Version |
|---|---|
| AGP / `compileSdk` / Robolectric SDK | 9.3.1 / 37 / 37 |
| Java | source/target 17; tests run on JDK 21 (Robolectric 4.17 refuses SDK 37 on 17) |
| Robolectric, JUnit | 4.17, 4.13.2, with Flutter's `--add-opens` set |
| Mockito | 5.20.0, loaded as `-javaagent` (inline mock maker, `MotionEvent` is final) |
| androidx.test core / ext junit, ICU4J | 1.6.1 / 1.2.1, 75.1 (answers the mocked `FlutterTextUtils.CodePoints`) |

Not taken: Play core (no ported test needs it), Flutter's `-Xlint:deprecation -Werror` (the crate's Java calls
deprecated APIs today), `minSdk` 24 (here 28).

## Port the next Flutter test

1. Fetch the file at the pinned tag (`gh api`, as [flutter-diff.sh](../scripts/flutter-diff.sh) does) into
   `src/test/java/io/github/xuckless/parallax/`, same name (test utils such as `KeyCodes` too).
2. Change only what the port changed: package, the renderer or channel the Parallax file replaced (shadow it),
   `io.flutter.Build.API_LEVELS` → `Build.VERSION_CODES`. Keep names, bodies and comments.
3. Cases for code the port dropped, or for behaviour below API 28, are dropped; `minSdk` below 28 is raised to 28, and
   a case pinned below 28 only because its API arrived there is re-pinned to 28. List all of it in the file header.
4. Add the header (AGENTS.md rule 3) and a ledger row in [PORTED.md](../third_party/flutter/PORTED.md), kind
   `oracle (test port)`, dropped cases in the last column.
5. Every kept case must pass. A failure is a bug in the port: fix the port, never the test.

Ported so far (cases kept of Flutter's; the ledger names the dropped ones): `AndroidTouchProcessorTest` 14/15,
`KeyboardManagerTest` 25/29, `InputConnectionAdaptorTest` 26/36, `TextInputPluginTest` 42/53, `ListenableEditingStateTest`
13/13, `TextEditingDeltaTest` 2/2, `PlatformPluginTest` 2/30 (the haptic cases, [haptics.md](haptics.md)).
