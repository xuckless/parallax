# Android Java tests

The ported Flutter Java in [crates/android/java/](../crates/android/java/) is tested on the JVM with Robolectric, no
device, by porting Flutter's own tests ([AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth) rule 6:
ported tests are the spec). The Rust side has its own `cargo test` rows; this covers the Java up to the JNI seam.

## Run

```sh
scripts/java-test.sh            # gradle test; extra args go to Gradle, e.g. --tests '*normalTouch'
```

Needs `ANDROID_HOME` and a Java 17+ to start Gradle. First run downloads Gradle 9.8.2, AGP, `platforms;android-37`
(licenses must be accepted), JDK 21 (foojay, if none is installed) and Robolectric's `android-all` jars (one per
emulated SDK, ~1.5 GB, into `~/.m2/repository/org/robolectric`). After that a run takes about 25 s.
Report: `crates/android/java-tests/build/reports/tests/testDebugUnitTest/index.html`. CI runs it in its own `java-tests` job
([ci.md](ci.md)).

## Layout

| Path | What |
|---|---|
| [crates/android/java-tests/build.gradle](../crates/android/java-tests/build.gradle) | One AGP library module; `main` is `../java` (the crate's sources, as is), tests in `src/test/java` |
| [src/main/AndroidManifest.xml](../crates/android/java-tests/src/main/AndroidManifest.xml) | Declares `android.app.Activity` for `ActivityScenario`, as Flutter's test_runner manifest |
| [robolectric.properties](../crates/android/java-tests/src/test/resources/robolectric.properties) | `sdk=37`, as Flutter's test_runner |
| [ShadowParallaxSurfaceView.java](../crates/android/java-tests/src/test/java/io/github/xuckless/parallax/ShadowParallaxSurfaceView.java) | The native seam (below) |
| [scripts/java-test.sh](../scripts/java-test.sh) | Runs the CLI's pinned wrapper jar on this project |

Gradle wrapper: not a second copy. The script runs
[crates/cli/templates/android/gradle/wrapper/gradle-wrapper.jar](../crates/cli/templates/android/gradle/wrapper/)
with `--project-dir`; the wrapper reads the `gradle-wrapper.properties` beside its jar, so the version and checksum
pin ([cli.md](cli.md#generated-project)) are shared too.

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

## The native seam

Robolectric cannot load the Rust `.so`. Tests annotated `@Config(shadows = ShadowParallaxSurfaceView.class)` get
`ParallaxSurfaceView`'s static natives replaced by the shadow's `@Implementation` methods, each forwarding to a static
interface field that stands in for what Flutter's test mocked, under Flutter's method names, so the `verify(...)`
lines stay close to verbatim. A test mocks the field with Mockito and sets it in `@Before` (static because the natives
are). The shadow extends `ShadowSurfaceView`, so a test can also build a `ParallaxSurfaceView` (Flutter's
`FlutterView`). A new native a test reaches gets one more `@Implementation` and a row here.

| Native | Shadow field | Stands in for |
|---|---|---|
| `nativeDispatchPointerDataPacket` | `renderer` (`Renderer.dispatchPointerDataPacket`) | `FlutterRenderer` |
| `nativeInsets` | `renderer` (`Renderer.setViewportMetrics`, safe area → padding, IME → inset, density → `devicePixelRatio`, touch slop → `physicalTouchSlop`) | `FlutterRenderer.ViewportMetrics` |
| `nativeDispatchKeyData` | `keyData` (`KeyDataSink`, returns the reply) | the `flutter/keydata` `BinaryMessenger` |
| `nativeUpdateEditingState`, `nativePerformAction`, `nativeBoundsForRange` | `textInput` (`TextInput`) | `DartExecutor.send` on `flutter/textinput` |

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
13/13, `TextEditingDeltaTest` 2/2.
