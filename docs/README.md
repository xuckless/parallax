# Parallax docs

How Parallax's code works. Rules live in [AGENTS.md](../AGENTS.md); the plan lives in [ROADMAP.md](../ROADMAP.md).

| Doc | Covers |
|---|---|
| [flutter.md](flutter.md) | Recording Flutter ports in the ledger, port vs oracle, bumping the pinned tag with flutter-diff.sh |
| [upstream.md](upstream.md) | upstream/ snapshot, apply-patches.sh, sync-upstream.sh, fixing patches that stop applying |
| [zed-fork.md](zed-fork.md) | Our Zed fork's `parallax` branch on xuckless/zed (patches in Zed's layout, for upstream PRs), rebuilding it with sync-fork.sh, and how it differs from the published fork crates |
| [gpui-fork-crates.md](gpui-fork-crates.md) | The published `parallax-gpui*` fork crates: naming, versioning and content hash, fork-crates.sh, fork-version.sh, publish-fork.sh, CI checks |
| [patches.md](patches.md) | Writing, numbering and making patches; the patch table |
| [core-patches.md](core-patches.md) | Core patches 0003–0005: pointer source, on_touch opt-out, TextInputConfiguration keyboard fields |
| [patch-tests.md](patch-tests.md) | patch-test.sh: running GPUI's own lib tests (every patch's tests) on the fork crate, stand-in fonts, skipped tests, the CI job |
| [patch-translation.md](patch-translation.md) | patch-paths.sh: vendored ↔ Zed layout, refused files, the crate table |
| [platform.md](platform.md) | Target → platform table, how an app gets its GPUI platform, adding a target, errors on unsupported targets |
| [audits/gap-audit.md](audits/gap-audit.md) | What upstream GPUI lacks for touch, text input, lifecycle, insets and accessibility per platform, with source links and the phase that fixes each |
| [ci.md](ci.md) | The seven CI jobs (six targets, Java tests), what each checks, running the checks locally, adding a target |
| [audits/android-fonts.md](audits/android-fonts.md) | Android font audit: cosmic-text/fontdb gaps, fonts.xml parsing, CJK and emoji fallback (COLRv1 vs CBDT), sizing and proposed Phase 1 tasks |
| [audits/oracle-inventory.md](audits/oracle-inventory.md) | Flutter 3.50.1 test files per area (gestures, text input, accessibility), the upstream GPUI code each checks, portability verdicts, Android/iOS test-infra risks, Phase 2/4 order |
| [android-spike.md](android-spike.md) | Phase 1 emulator spike: ALooper dispatcher, minimum Platform/PlatformWindow surface, wgpu Vulkan vs GL on the emulator, tap→click, upstream Android status, emulator setup |
| [cli.md](cli.md) | cargo-parallax: parallax.toml keys and defaults, what run android does step by step, the generated Gradle project, SDK/NDK/JDK requirements and env vars |
| [android.md](android.md) | The Android backend: Activity → JNI → Platform, Looper dispatcher, surface state machine, renderer table, the android_main! entry point, stubs, running on the emulator |
| [android-touch.md](android-touch.md) | Android touch: ported AndroidTouchProcessor → JNI pointer packet → Rust decoder → TouchEvent, what is not yet wired, following Flutter packet changes |
| [examples.md](examples.md) | The hello example: what it shows, and running it on desktop, web, Android and iOS (CI build only) |
| [android-tests.md](android-tests.md) | JVM tests of the ported Android Java: Robolectric project, versions, native shadow, ported tests, porting the next Flutter test |
| [android-keys.md](android-keys.md) | Android keys: ported KeyboardManager, key-data decoder and KEYS table, back → BackGestureEvent |
| [android-fonts.md](android-fonts.md) | Android system fonts: fonts.xml parser, fallback table, runtime locale changes, the FontSystem from patch 0002 |
| [android-text-input.md](android-text-input.md) | Android text input: ported TextInputPlugin/InputConnectionAdaptor, the Java↔GPUI sync protocol, configuration tables, limits |
| [android-emoji.md](android-emoji.md) | Android colour emoji: fonts.xml emoji faces drawn by Android over JNI, one glyph per cluster, metrics, emoji-picker cost |
| [android-lifecycle.md](android-lifecycle.md) | Android lifecycle: Activity callbacks → state machine → upstream lifecycle, memory and window hooks; onDestroy choice; several live Activities |
| [android-surface.md](android-surface.md) | Android surface: the surface state machine and device-loss recovery |
