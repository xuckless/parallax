# Parallax docs

How Parallax's code works. Rules live in [AGENTS.md](../AGENTS.md); the plan lives in [ROADMAP.md](../ROADMAP.md).

| Doc | Covers |
|---|---|
| [flutter.md](flutter.md) | Recording Flutter ports in the ledger, port vs oracle, bumping the pinned tag with flutter-diff.sh |
| [upstream.md](upstream.md) | upstream/ snapshot, apply-patches.sh, sync-upstream.sh, fixing patches that stop applying |
| [zed-fork.md](zed-fork.md) | Our Zed fork's `parallax` branch on xuckless/zed (patches in Zed's layout, for upstream PRs), rebuilding it with sync-fork.sh, and how it differs from the published fork crates |
| [gpui-fork-crates.md](gpui-fork-crates.md) | The published `parallax-gpui*` fork crates: naming, versioning and content hash, fork-crates.sh, fork-version.sh, publish-fork.sh, CI checks |
| [patches.md](patches.md) | Writing, numbering and making patches; the patch table |
| [core-patches.md](core-patches.md) | Core patches 0003–0005, 0008, 0010 and 0014: pointer source, on_touch opt-out, TextInputConfiguration keyboard fields, composition ended on focus loss, the app's gesture tuning |
| [pointer-source.md](pointer-source.md) | Patch 0003: `Window::pointer_source()`, how core and each backend set the device and synthesized flag, the web pen limit |
| [gesture-tuning.md](gesture-tuning.md) | Patches 0014 and 0019: `App::set_gesture_tuning` for new windows, `Window::set_gesture_tuning` retunes an open one (Android's device touch slop), their tests |
| [velocity-tracking.md](velocity-tracking.md) | Patch 0011: Flutter's velocity trackers and least-squares solver ported into GPUI, the kinds, which platform gets which, the ported tests |
| [desktop-touch-linux.md](desktop-touch-linux.md) | Linux touchscreens: Wayland `wl_touch` (patch 0022) and X11 XInput 2 touch (patch 0023), both from winit, as `TouchEvent`s; the shared event clock, ids, selection by XInput version, emulated pointer events, cancels, seats, tests |
| [touch-time-stamps.md](touch-time-stamps.md) | Patch 0020: `TouchEvent::time_stamp`, the platform's event time on GPUI's clock, how Android, iOS and the web fill it, what reads it (velocity tracking), the fallback to dispatch time |
| [desktop-touch-windows.md](desktop-touch-windows.md) | Patch 0021: Windows touch and pen (`WM_POINTER`, ported from winit) as `TouchEvent`s, dropping the mouse copies Windows makes, pen hover, cancels, time stamps, the pure `pointer.rs` and its host tests |
| [desktop-touch-macos.md](desktop-touch-macos.md) | Patch 0024: direct touch on macOS 27 (touchscreen Macs, Sidecar iPads): the AppKit touch APIs, the touch recognizer on GPUIView, `NSIsTouchNative` and emulation, limits |
| [scale-and-overscroll.md](scale-and-overscroll.md) | Patch 0015: Flutter's scale (pinch/rotate) recognizer and glow and stretch overscroll state on 0012's physics; how to drive them, and what Wave 12 wires |
| [pinch.md](pinch.md) | Patch 0016: touch pinch through upstream's `on_pinch` / `PinchEvent`, the pinch arena's states (which element and fingers pinch), each platform's multi-touch path, the trace tests |
| [overscroll.md](overscroll.md) | Patch 0017: bounce, stretch and glow in `div` scroll containers, chosen by `GestureTuning::overscroll` from the behaviour table, the per-container `.overscroll(..)`, the ported `ScrollPosition` glue, limits and tests |
| [patch-tests.md](patch-tests.md) | patch-test.sh: running GPUI's own lib tests (every patch's tests) on the fork crate, stand-in fonts, skipped tests, the CI job |
| [gesture-recognizers.md](gesture-recognizers.md) | Patch 0013: Flutter's recognizer tests as table rows through the test platform, the tuning, slop, per-target tap-count and fling-distance fixes, the touch trace format for trace replay |
| [patch-translation.md](patch-translation.md) | patch-paths.sh: vendored ↔ Zed layout, refused files, the crate table |
| [platform.md](platform.md) | Target → platform table, how an app gets its GPUI platform, adding a target, errors on unsupported targets |
| [haptics.md](haptics.md) | Patch 0018 and Android: `Window::perform_haptic`, the long-press haptic from the behaviour table, each kind per platform (Android, iOS, web), the Android pull, the tests |
| [desktop-touch-behaviour.md](desktop-touch-behaviour.md) | Desktop touch on the macOS, Windows and Linux rows: Flutter's slops per pointer kind, drag devices, physics, touch target, what is not ported, the tests |
| [behaviour.md](behaviour.md) | Target → behaviour table (Flutter's `TargetPlatform` defaults): touch slop, scroll physics, tracker, overscroll, haptics, touch target; how every entry point applies it automatically, overriding and extending it |
| [audits/gap-audit.md](audits/gap-audit.md) | What upstream GPUI lacks for touch, text input, lifecycle, insets and accessibility per platform, with source links and the phase that fixes each |
| [ci.md](ci.md) | The nine CI jobs (six targets, Java tests, patch tests, Android smoke), what each checks, when each runs, running the checks locally, adding a target |
| [audits/android-fonts.md](audits/android-fonts.md) | Android font audit: cosmic-text/fontdb gaps, fonts.xml parsing, CJK and emoji fallback (COLRv1 vs CBDT), sizing and proposed Phase 1 tasks |
| [audits/oracle-inventory.md](audits/oracle-inventory.md) | Flutter 3.50.1 test files per area (gestures, text input, accessibility), the upstream GPUI code each checks, portability verdicts, Android/iOS test-infra risks, Phase 2/4 order |
| [audits/oracle-inventory-gestures.md](audits/oracle-inventory-gestures.md) | Oracle inventory part: Flutter's gesture, velocity and scroll-physics tests, case counts and salvage per file |
| [audits/oracle-inventory-text.md](audits/oracle-inventory-text.md) | Oracle inventory part: Flutter's text input and key tests (Java, Obj-C, Dart), case counts per file |
| [audits/oracle-inventory-a11y.md](audits/oracle-inventory-a11y.md) | Oracle inventory part: Flutter's accessibility tests for Phase 6 |
| [android-spike.md](android-spike.md) | Phase 1 emulator spike: ALooper dispatcher, minimum Platform/PlatformWindow surface, wgpu Vulkan vs GL on the emulator, tap→click, upstream Android status, emulator setup |
| [cli.md](cli.md) | cargo-parallax: parallax.toml keys and defaults, what run android does step by step, the generated Gradle project, SDK/NDK/JDK requirements and env vars |
| [android.md](android.md) | The Android backend: Activity → JNI → Platform, Looper dispatcher, surface state machine, renderer table, the android_main! entry point, running on the emulator |
| [android-stubs.md](android-stubs.md) | What GPUI asks of a platform that Android does not do yet, and what happens instead |
| [android-touch.md](android-touch.md) | Android touch: ported AndroidTouchProcessor → JNI pointer packet → Rust decoder → TouchEvent per device route, what is not yet wired, following Flutter packet changes |
| [android-mouse.md](android-mouse.md) | Android mouse, trackpad and wheel: device-kind routes, mouse action and button tables, click counting, scroll signs |
| [android-smoke.md](android-smoke.md) | android-smoke.sh: hello on an emulator, the step table (tap, type, rotate, background) |
| [android-smoke-ci.md](android-smoke-ci.md) | android-smoke.sh: finding tap targets by colour, the CI job, limits |
| [examples.md](examples.md) | The hello example: what it shows, its entry point per target |
| [examples-run.md](examples-run.md) | Running hello on desktop, web, Android and iOS (CI build only) |
| [android-tests.md](android-tests.md) | JVM tests of the ported Android Java: Robolectric project, native shadow, Parallax's own tests |
| [android-tests-porting.md](android-tests-porting.md) | The JVM test stack's versions, porting the next Flutter test, the tests ported so far |
| [android-keys.md](android-keys.md) | Android keys: ported KeyboardManager, key-data decoder and KEYS table, back → BackGestureEvent |
| [android-fonts.md](android-fonts.md) | Android system fonts: fonts.xml parser, fallback table, runtime locale changes, the FontSystem from patch 0002 |
| [android-text-input.md](android-text-input.md) | Android text input: ported TextInputPlugin/InputConnectionAdaptor, the Java↔GPUI sync protocol, configuration tables, limits |
| [android-autofill.md](android-autofill.md) | Android autofill: GPUI's hint → Flutter's `Configuration.Autofill`, the field id, AutofillManager calls, filled values through the merge path, the hint table |
| [android-emoji.md](android-emoji.md) | Android colour emoji: fonts.xml emoji faces drawn by Android over JNI, one glyph per cluster, metrics, emoji-picker cost |
| [android-lifecycle.md](android-lifecycle.md) | Android lifecycle: Activity callbacks → state machine → upstream lifecycle, memory and window hooks; onDestroy choice; several live Activities |
| [android-surface.md](android-surface.md) | Android surface: the surface state machine and device-loss recovery |
| [gate-1.md](gate-1.md) | Gate 1 on the Android emulator (API 28, 33, 35, Gboard): the results matrix, the failures found, and how to rerun it |
| [traces-desktop.md](traces-desktop.md) | Gate 3's desktop trace fixtures: the folders the replay reads, what a Windows, Wayland, X11 or macOS harness must write, the device column, the Weston headless harness for Wayland and X11, checking a harness |
| [traces.md](traces.md) | Touch traces for Gate 2: hello's trace pad recorder, record-traces.sh on the Android emulator, headless Chromium (CDP) and the iOS simulator (idb), the replay test and its tolerance |
| [scroll-physics.md](scroll-physics.md) | Patch 0012: GPUI's one `ScrollPhysics` (Flutter's bouncing and clamping physics) for flings, overscroll and the table, Flutter's physics module on GPUI's springs, what is not ported yet, the ported test tables |
