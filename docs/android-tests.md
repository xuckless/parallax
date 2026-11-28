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

Parallax's own: `HapticArgumentsTest` reads the argument match in [haptics.rs](../crates/android/src/haptics.rs) (the
one list; build.gradle passes its path as `parallax.hapticsRs`) and checks every argument resolves through
`HapticFeedbackType.fromValue`, one Android type each. `NativeViewTouchDispatcherTest` drives the ported
`JSTouchDispatcher` with a fake hit test into a `View` that `NativeViews` laid out: one row per gesture rule
([native-view-android.md](native-view-android.md)).

Versions of the test stack and how to port the next Flutter test: [android-tests-porting.md](android-tests-porting.md).
