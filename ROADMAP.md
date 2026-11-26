# ROADMAP

Parallax ships in seven phases, in order, across two rolling release lines. Each phase ends with a gate, and the
next phase starts only after the gate passes. Each phase uses only what earlier phases produced.
No dates are set. Tick a box only when the work is merged and verified (see `AGENTS.md`).

Upstream GPUI already ships iOS, touch and gestures, web touch and IME, and a core accessibility tree.
Flutter has a decade of fixes for exactly these problems. So Parallax's work is: **Android built on Flutter's
ported embedding, desktop touch, Flutter's tests as the oracle for upstream, platform behaviour, accessibility
adapters and build tooling.**

## Releases

Two rolling release lines on crates.io. A release goes out whenever `main` is green and holds something new; there is
no fixed schedule and no planned last number.

| Line | Phases | First release |
|---|---|---|
| v0.1.x | 0–5: foundation, Android, gestures and behaviour, desktop touch, text input, tooling | 0.1.1: the working prototype skeleton (Phase 1) |
| v0.2.x | 6: accessibility | the first accessibility adapter that lands |

- Every gate that passes ships a release. Merged, verified work between gates may ship one too.
- The v0.1.x line ends when Gate 5 passes; v0.2.x starts then.
- Every release: CI green on all six targets, `NOTICE` and `third_party/flutter/PORTED.md` current, crate license
  fields right, no GPL editor code, a `CHANGELOG.md` entry.

## Phase 0: Foundation, gap audit and Flutter oracle

- [x] Vendor the pinned `gpui-pre-*` crates (`gpui-pre`, `gpui-pre-ios`, `gpui-pre-web`, `gpui-pre-wgpu`, the desktop
      platform crates and their `gpui-pre-*` helpers) into `upstream/` with `cargo vendor`. Record the version
      and the Zed commit it snapshots in `upstream/UPSTREAM`.
- [x] `scripts/apply-patches.sh`: pristine `upstream/` + `patches/` → `build/gpui-patched/`. Deterministic, idempotent, fails loudly.
- [x] Wire `[patch]` in the root `Cargo.toml` to `build/gpui-patched/`.
- [x] `scripts/sync-upstream.sh`: bump to the latest `gpui-pre` snapshot, re-vendor, re-apply patches, list the ones that no longer apply.
- [x] Flutter stable tag `3.50.1` pinned in `third_party/flutter/PORTED.md`. `scripts/flutter-diff.sh`: for every ledger row,
      show the Flutter source diff between the pinned tag and a newer stable tag.
- [x] `scripts/patch-paths.sh`: translate a patch between the vendored `gpui-pre` layout and Zed's `crates/gpui*` layout,
      so one patch applies to both.
- [x] CI matrix: iOS, Android, wasm, macOS, Windows, Linux (six builds; no feature variants).
- [x] Gap audit: run upstream's iOS and web examples, then write down what is missing for touch, IME,
      lifecycle (background, active, low memory), safe area and accessibility. The results go into the phases below.
      See [docs/audits/gap-audit.md](docs/audits/gap-audit.md).
- [x] Android font audit: cosmic-text on Android needs system font discovery (`/system/etc/fonts.xml`), emoji and CJK
      fallback. Size the work and add it to Phase 1. See [docs/audits/android-fonts.md](docs/audits/android-fonts.md).
- [x] Oracle inventory: list Flutter's test files for text input, gestures, velocity tracking and scroll physics,
      and which upstream GPUI code each one checks. This sets the order of Phases 2 and 4.
      See [docs/audits/oracle-inventory.md](docs/audits/oracle-inventory.md).
- [x] `parallax-platform`: one target → platform table (desktop, web, iOS upstream; Android later).
- [ ] Reserve the `parallax*` crate names on crates.io (`parallax`, `parallax-platform`, `parallax-android`,
      `parallax-web`, `parallax-ext`, `parallax-ios`, `cargo-parallax`).

**Gate 0:** a fresh clone builds `build/gpui-patched/` from scripts alone, CI is green with zero patches, and the
oracle inventory exists.

## Phase 1: Android on Flutter's embedding

GPUI runs on the Android main (UI) thread, so IME queries are answered synchronously. Flutter's self-contained input
files are ported as Java in `crates/android/java/`; `MethodChannel` calls become JNI calls into Rust. The Activity and
View are Parallax's own, small, written with Flutter's as reference. The phase runs in four steps: spike, skeleton
(ships as 0.1.1), core patches, full Android.

Spike (throwaway, Android emulator):

- [x] One Activity, `gpui_wgpu` drawing a box from the main thread, one tap changing its colour, on the Android
      emulator. Proves wgpu on the emulator's `Surface` and GPUI on the main `Looper` before the real code is written.
      Write down what it learns (Looper integration, Vulkan vs GL on the emulator) in `docs/`, then delete the code.
      Done: both worked; see [docs/android-spike.md](docs/android-spike.md).

Skeleton (0.1.1):

- [x] `parallax` facade crate: one dependency for apps, re-exporting GPUI and the Parallax crates.
- [ ] `cargo-parallax`: `parallax.toml` schema and `cargo parallax run android` (generates the Gradle project around the Java port).
- [ ] Main-thread dispatcher: GPUI's foreground executor on the Android main `Looper` via `ALooper_addFd` + an eventfd
      (no Java `Handler`). Small patch: export upstream's `PriorityQueue*` on Android (today only windows/linux/wasm).
- [ ] Activity and View: write a small Parallax Activity and `SurfaceView`, using `FlutterActivity`, `FlutterView` and
      `FlutterSurfaceView` as reference for lifecycle, surface and insets.
- [ ] Rendering: `gpui_wgpu` on the ported view's `Surface`, Vulkan with GL fallback. The backend choice is one visible table.
      The renderer is created on the main thread (about 0.5 s on the emulator); measure on a device and move it off
      only if startup exceeds the budget.
- [ ] Surface lifecycle as one Rust state enum (none, ready, lost); recover from surface and device loss. Needed in the
      skeleton: in the spike, returning from Home failed on a stale surface.
- [ ] Insets: `targetSdk` 35 forces edge-to-edge, so the window draws under the system bars; report safe-area insets.
- [ ] Touch: port `AndroidTouchProcessor` and `MotionEventTracker`; emit upstream `TouchEvent`.
- [ ] Example app: one window with a box that changes colour on tap, built for desktop, web and Android.
- [ ] Publish 0.1.1.

**Milestone 0.1.1:** the example app opens, draws and reacts to a tap on desktop, web and an Android device.

Core patches (needed by Android from the start; iOS and web get them too):

- [ ] Every pointer event carries its device (mouse, touch, pen) and whether it was synthesized from touch. Upstream
      has neither ([gap audit](docs/audits/gap-audit.md)).
- [ ] Per-element opt-out of touch → mouse emulation: build upstream's declared-but-unused `ClickEvent::Touch`.
- [ ] `TextInputConfiguration`: keyboard type, secure entry and autofill hint.
- [ ] Fork `zed-industries/zed` when the first of these is ready to file; file each one.

Full Android:

- [ ] Keys: port `KeyboardManager` and `KeyEmbedderResponder`; Android key code → GPUI key as one Rust table.
      Back button → `BackGestureEvent`.
- [ ] Text input: port `InputConnectionAdaptor`, `ListenableEditingState`, `TextEditingDelta`, `FlutterTextUtils`,
      `ImeSyncDeferringInsetsCallback` and the input parts of `TextInputPlugin`. On the same thread, the Rust side
      answers queries straight from GPUI's `InputHandler` (both use UTF-16 offsets); Flutter's editing state is kept for
      its batch-edit logic. Ledger rows list the parts left out.
- [ ] Java test setup copied from Flutter's `test_runner`: Gradle, JDK 17, Robolectric 4.17, Mockito 5.20 (JVM, no device).
- [ ] Port the matching Flutter Android tests (`InputConnectionAdaptorTest`, `TextInputPluginTest`, `KeyboardManagerTest`,
      `AndroidTouchProcessorTest`, `ListenableEditingStateTest`, `TextEditingDeltaTest`) as tests of the Java port.
      Cases pinned below API 28 are dropped and listed in the ledger.
- [ ] Fonts ([audit](docs/audits/android-fonts.md)): a small upstream patch so `CosmicTextSystem` takes our own font
      list; a `fonts.xml` / `font_fallback.xml` parser with table tests; the Android fallback table (per-language CJK,
      emoji); load fonts by path and prefer the CBDT `NotoColorEmojiLegacy.ttf` (swash can't draw COLRv1); pass the
      app locale over JNI. Fonts load on the main thread at startup; measure startup and move them off it only if it
      exceeds the budget.
- [ ] Multi-script example screen (Latin, CJK, Arabic, emoji) checked on API 28, 33 and 35.
- [ ] Lifecycle: implement upstream's existing lifecycle, memory-warning, inset and back hooks for Android. Map
      `onDestroy` to an explicit choice (upstream has no terminate phase).
- [ ] Rust unit tests: key-code table, surface state machine, editing-state bridge, fonts.xml parser.
- [ ] Ledger rows in `third_party/flutter/PORTED.md` for every ported file.

**Gate 1:** the example app runs on an Android device (API 28+) with Gboard and Samsung Keyboard: typing, autocorrect,
composing, selection, emoji, rotation, background/resume.

## Phase 2: Gestures, physics and platform behaviour

Flutter's gesture and physics tests check upstream `gestures.rs` and `ScrollPhysics`. Where upstream fails, Flutter's
fix goes in as a small patch and is filed upstream. The tables built here are what Phase 3 replays desktop traces through.
Tests drive GPUI through its test platform (touch events in, gestures and scroll offsets out); a small public-seam
patch only for a case that can't be reached that way. Order follows the [oracle inventory](docs/audits/oracle-inventory.md).

- [ ] Velocity: `velocity_tracker` and `lsq_solver` cases.
- [ ] Physics: `friction_simulation`, clamping and `scroll_physics` (fling) cases.
- [ ] Recognizers: tap, double-tap, long-press and drag cases. (Flutter's `arena` is skipped: GPUI resolves
      competing gestures by claims, not an arena.)
- [ ] Multi-touch and pinch in `gestures.rs` (upstream has none), with Flutter's `scale` tests as the oracle.
- [ ] Overscroll (bounce, glow, clamp): a row in the behaviour table and a scroll-container patch.
- [ ] Platform behaviour table: target → behaviour (Flutter's `TargetPlatform` defaults), desktop touch included.
      Covers scroll physics preset, gesture tuning (GPUI's touch slop is 8 px, Flutter's 18), touch → mouse
      emulation, overscroll, haptics, touch-target minimum.
- [ ] Trace recorder: capture raw touch traces into test fixtures, starting with an Android phone, an iPhone and a browser.
- [ ] Trace replay: run the traces through the gesture and physics test tables on every platform.

**Gate 2:** the ported gesture and physics tables pass on every target, and the phone and browser traces replay to the
same gestures and scroll outcomes everywhere.

## Phase 3: Desktop touch

Always compiled in; behaviour follows pointer type at runtime. Each patch feeds upstream `TouchEvent` and drops the
fake mouse events the OS makes from touch, so a tap fires once.

- [ ] `0001-touch-windows`: `WM_POINTER` touch and pen in `gpui_windows`.
- [ ] `0002-touch-linux`: Wayland `wl_touch` and X11 XInput2 touch in `gpui_linux`.
- [ ] `0003-touch-macos`: touchscreen events in `gpui_macos`. Confirm the macOS 27 API first (open question).
- [ ] Record traces on a touchscreen laptop for each desktop OS and add them to the Phase 2 fixtures.
- [ ] File each patch upstream.

**Gate 3:** the touchscreen laptop traces replay to the same gestures and scroll outcomes as the phone and browser
traces, on each desktop OS.

## Phase 4: Text input oracle and selection

Flutter's text-input tests check upstream GPUI. Where upstream fails, Flutter's fix goes in as a
small patch and is filed upstream.

- [ ] iOS text input: the 39 `UITextInput` behaviour cases from `FlutterTextInputPluginTest.mm` as a table against
      `gpui_ios`, then the 15 mapped cases (text traits, update counts); patch the failures. The other 81 test Flutter
      internals and are skipped. Limit: once these patches total about 800 lines or Zed rejects 3, move iOS text
      input into its own Parallax crate.
- [ ] iOS hardware keyboard: `pressesBegan`/`pressesEnded` → `KeyDown`/`KeyUp`, shortcuts and arrow keys.
- [ ] iOS keyboard inset: intersect the keyboard frame with the window (floating iPad keyboards) and animate it.
- [ ] Web text editing: `text_editing.dart` per-browser strategies and their tests → tests against `gpui_web`
      (`ime_mirror.rs`); patch or `parallax-web`.
- [ ] Web: app lifecycle (`visibilitychange`) and the on-screen keyboard inset.
- [ ] Text selection: native `UITextInteraction` (handles, magnifier, edit menu) on iOS; Flutter's selection
      behaviour ported for Android and touch desktops (no Material/Cupertino look).
- [ ] Remaining lifecycle and safe-area gaps from the audit, as patches.
- [ ] `parallax-ext`: extension traits only where the audit shows real friction (e.g. `.hit_slop()`).

**Gate 4:** the ported Flutter text-input tables pass on every target.

## Phase 5: Tooling and docs

- [ ] `cargo parallax run ios` (generates the Xcode project) and `cargo parallax run web`.
- [ ] Docs: getting started per target, the touch → mouse rule and its opt-out, the behaviour table, Flutter credits.
- [ ] Example app polished for every target.

**Gate 5:** a fresh clone runs the example app on every target through `cargo parallax`, and the docs cover each one.
Closes the v0.1.x line.

## Phase 6: Accessibility (v0.2.x)

Each adapter ships as the next 0.2.x release when it lands. Flutter's `AccessibilityBridge` and `SemanticsObject`
tests are the oracle.

- [ ] Android: `accesskit_android` in `parallax-android`, at the version matching upstream's AccessKit (0.24 today → 0.7.x).
- [ ] Web: `parallax-web` semantics DOM (upstream web has no screen-reader output).
- [ ] iOS: new `parallax-ios` crate wiring `accesskit_ios` (0.1.0 matches AccessKit 0.24), plus a small patch adding the
      hook `gpui_ios` needs to hand over its view and tree. Note: `accesskit_ios` uses objc2 0.5, `gpui_ios` 0.6.
- [ ] macOS, Windows, Linux: upstream already has AccessKit adapters; check them with VoiceOver, Narrator/NVDA and
      Orca, and patch the gaps.

**Gate 6:** VoiceOver, TalkBack and a desktop screen reader can operate the example app.

## Open questions

- [ ] macOS 27: how are touchscreen events exposed? Needed before `0003-touch-macos`.
- [x] Which Android `Looper` integration does GPUI's dispatcher need? `ALooper_addFd` with an eventfd ([spike](docs/android-spike.md)).
- [ ] Which patches to file upstream first.
- [x] Is upstream adding Android itself? No: the one Android PR (#61057) was closed as stale and too large (checked 2026-11-26).
- [ ] When `zed-industries/gpui` holds code, move `upstream/` to it. Same code as `gpui-pre`, so only the source changes.
- [ ] `gpui-pre` is published by one person. If its snapshots stop, fall back to vendoring Zed `crates/gpui*` with a generated manifest.
- [ ] Will Zed accept patches whose code is BSD-3 (Flutter-derived) into an Apache-2.0 crate? Ask before filing the first one.

## Upstream notes

Facts about upstream (GPUI and Flutter) that shaped this plan. Add a line when upstream surprises you.

| Date | Source | Note |
|---|---|---|
| 2026-11-25 | zed `8aeb64f7` | `zed-industries/gpui` repo holds only a README ("See you in a few weeks"). |
| 2026-11-25 | zed `8aeb64f7` | Zed's `crates/gpui*` use Zed's `workspace = true` deps and don't build standalone. Tracking `gpui-pre` instead. |
| 2026-11-25 | gpui-pre 0.3.14 = zed `f126950` | `gpui-pre-*` on crates.io: weekly snapshots of Zed GPUI (incl. `gpui-pre-ios`, `gpui-pre-web`). Chosen as the interim base. |
| 2026-11-25 | gpui-pre 0.3.14 = zed `f126950` | 20 `gpui-pre-*` crates vendored (15 MB); each Cargo.toml carries `[package.metadata.gpui-pre] zed-rev`. Builds standalone on Linux, no extra system packages. |
| 2026-11-25 | zed `f126950` | Zed pins Rust 1.98.1 (no crate declares `rust-version`). Our toolchain follows it, down from 1.100.0. |
| 2026-11-25 | gpui-pre 0.3.14 | Not byte-identical to Zed: 7 files carry `// Modified for gpui-pre` (`action.rs`, `gpui_macros.rs` + `gpui_pre_facade_paths.rs`, `gpui_apple/build.rs`, 3 wgpu files for wgpu 30). `gpui-pre-apple/vendor/` copies gpui sources; `gpui_web/examples/` is dropped. Everything else matches Zed byte for byte except `Cargo.toml`. |
| 2026-11-25 | gpui-pre 0.3.14 | `gpui` compiles for Android; `gpui_platform` does not (no Android arm). On wasm32, `gpui_platform`'s default `multithreaded` feature pulls `wasm_thread`, which needs nightly. |
| 2026-11-26 | gpui-pre 0.3.14, cargo 1.98.1 | `cargo check -p gpui-pre-wgpu --target aarch64-linux-android` panics inside cargo (`did not find features for gpui-pre-wgpu`) instead of reporting that the crate is not in the Android graph. |
| 2026-11-26 | gpui-pre 0.3.14 | `gpui_web` turns on `parking_lot/nightly` unconditionally on wasm, yet it builds on stable; only `multithreaded` (`wasm_thread`) needs nightly. |
| 2026-11-26 | gpui-pre 0.3.14 | Core has lifecycle, memory-warning, inset and back hooks; pointer events carry no device or synthesized tag; `ClickEvent::Touch` is declared but never built; no pinch or overscroll; `TextInputConfiguration` has no keyboard type. See the [gap audit](docs/audits/gap-audit.md). |
| 2026-11-26 | gpui-pre 0.3.14 | AccessKit adapters exist on macOS, Windows and Linux; none on iOS or web. AccessKit is pinned at 0.24. |
| 2026-11-26 | gpui-pre 0.3.14 | The vendored `gpui-pre-ios` example doesn't build as shipped (uses `gpui_platform`, not a dev-dependency). Built from crates.io `=0.3.14` instead; it runs on the iOS 26.5 simulator. |
| 2026-11-26 | gpui-pre 0.3.14 vs flutter 3.50.1 | GPUI's touch slop is 8 px, Flutter's 18 px. All gesture logic (`VelocityTracker`, fling, recognizers) is private to `gestures.rs`. |
| 2026-11-26 | fontdb 0.23, swash 0.2.10 | fontdb has no Android font discovery (empty list, no warning); swash draws CBDT and COLR v0, not COLRv1. See the [font audit](docs/audits/android-fonts.md). |
| 2026-11-26 | gpui-pre 0.3.14 on Android emulator | GPUI draws via `gpui_wgpu` on Vulkan and GLES 3.1 with no glue patch; a backend needs ~10 `Platform` + ~12 `PlatformWindow` methods for real (~1,500 lines). Start with `Application::run_embedded`. `PriorityQueue*` is not exported on Android. Emulator needs `-gpu host` (`swiftshader_indirect` crashed). See the [spike](docs/android-spike.md). |
| 2026-11-25 | gpui-ce main | `gpui-ce` has no `gpui_ios` and is moving to Parley text with API drift. Not used. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_ios` exists (added 2026-10-14, #63068): UIKit, Metal, CoreText, text input (49 KB), safe area, back gesture. |
| 2026-11-25 | zed `8aeb64f7` | Core gpui has `TouchEvent`, `gestures.rs` (tap→mouse, long press, pan, pinch, drag) and `ScrollPhysics::{ios, android}`. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_web` has touch (`events.rs`) and IME (`ime_mirror.rs`). Core has AccessKit (`window/a11y.rs`). |
| 2026-11-25 | zed `8aeb64f7` | No touch input on `gpui_windows`, `gpui_linux` or `gpui_macos`. No Android backend. |
| 2026-11-25 | flutter 3.50.1 | Pinned stable tag (stable branch `ae9b586f34`, 2026-11-19). Ported files exist unchanged at this tag. |
| 2026-11-25 | flutter `322dfd0771` (master) | BSD-3-Clause. Android text input: `TextInputPlugin.java` 36 KB, `InputConnectionAdaptor.java` 21 KB. iOS: `FlutterTextInputPlugin.mm` 141 KB + 207 KB of tests. Web: `text_editing.dart` 113 KB. |
| 2026-11-25 | flutter GitHub releases | `releases/latest` returns `3.19.0-0.1.pre` (a 2024 beta not marked pre-release). Find stable tags from the tag list, highest plain `X.Y.Z`. |
| 2026-11-25 | accesskit | `accesskit_android` exists (permissive: Apache, MIT, Chromium-derived BSD). Use it, don't port Flutter's `AccessibilityBridge`. 0.9.1 needs AccessKit 0.25; upstream pins 0.24, so use the 0.7.x line until upstream bumps. |
