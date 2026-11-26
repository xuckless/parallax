# ROADMAP

Parallax ships in five phases. Each one ends with a gate, and the next phase starts only after the gate passes.
No dates are set. Tick a box only when the work is merged and verified (see `AGENTS.md`).

Upstream GPUI already ships iOS, touch and gestures, web touch and IME, and a core accessibility tree.
Flutter has a decade of fixes for exactly these problems. So Parallax's work is: **Android built on Flutter's
ported embedding, desktop touch, Flutter's tests as the oracle for upstream, platform behaviour, accessibility
adapters and build tooling.**

## Phase 0: Foundation, gap audit and Flutter oracle

- [ ] Vendor the pinned `gpui-pre-*` crates (`gpui-pre`, `gpui-pre-ios`, `gpui-pre-web`, `gpui-pre-wgpu`, the desktop
      platform crates and their `gpui-pre-*` helpers) into `upstream/` with `cargo vendor`. Record the version
      and the Zed commit it snapshots in `upstream/UPSTREAM`.
- [ ] `scripts/apply-patches.sh`: pristine `upstream/` + `patches/` → `build/gpui-patched/`. Deterministic, idempotent, fails loudly.
- [ ] Wire `[patch]` in the root `Cargo.toml` to `build/gpui-patched/`.
- [ ] `scripts/sync-upstream.sh`: bump to the latest `gpui-pre` snapshot, re-vendor, re-apply patches, list the ones that no longer apply.
- [ ] Flutter stable tag `3.50.1` pinned in `third_party/flutter/PORTED.md`. `scripts/flutter-diff.sh`: for every ledger row,
      show the Flutter source diff between the pinned tag and a newer stable tag.
- [ ] `scripts/patch-paths.sh`: translate a patch between the vendored `gpui-pre` layout and Zed's `crates/gpui*` layout,
      so one patch applies to both.
- [ ] CI matrix: iOS, Android, wasm, macOS, Windows, Linux (six builds; no feature variants).
- [ ] Gap audit: run upstream's iOS and web examples, then write down what is missing for touch, IME,
      lifecycle (background, active, low memory), safe area and accessibility. The results go into the phases below.
- [ ] Android font audit: cosmic-text on Android needs system font discovery (`/system/etc/fonts.xml`), emoji and CJK
      fallback. Size the work and add it to Phase 1.
- [ ] Oracle inventory: list Flutter's test files for text input, gestures, velocity tracking and scroll physics,
      and which upstream GPUI code each one checks. This sets the order of Phase 3.
- [ ] `parallax-platform`: one target → platform table (desktop, web, iOS upstream; Android later).
- [ ] Reserve `parallax*` crate names on crates.io and the GitHub org name.
- [ ] Set up a GitHub fork of `zed-industries/zed` for filing upstream PRs (patches are re-based onto Zed main there).

**Gate 0:** a fresh clone builds `build/gpui-patched/` from scripts alone, CI is green with zero patches, and the
oracle inventory exists.

## Phase 1: Android on Flutter's embedding

GPUI runs on the Android main (UI) thread, so IME queries are answered synchronously. Flutter's self-contained input
files are ported as Java in `crates/android/java/`; `MethodChannel` calls become JNI calls into Rust. The Activity and
View are Parallax's own, small, written with Flutter's as reference.

- [ ] Activity and View: write a small Parallax Activity and `SurfaceView`, using `FlutterActivity`, `FlutterView` and
      `FlutterSurfaceView` as reference for lifecycle, surface and insets.
- [ ] Main-thread dispatcher: GPUI's foreground executor on the Android main `Looper`.
- [ ] Rendering: `gpui_wgpu` on the ported view's `Surface`, Vulkan with GL fallback. The backend choice is one visible table.
- [ ] Surface lifecycle as one Rust state enum (none, ready, lost); recover from surface and device loss.
- [ ] Touch: port `AndroidTouchProcessor` and `MotionEventTracker`; emit upstream `TouchEvent`.
- [ ] Keys: port `KeyboardManager`; Android key code → GPUI key as one Rust table. Back button → `BackGestureEvent`.
- [ ] Text input: port `InputConnectionAdaptor`, `ListenableEditingState`, `TextEditingDelta`, `FlutterTextUtils`,
      `ImeSyncDeferringInsetsCallback` and the input parts of `TextInputPlugin`. On the same thread, the Rust side
      answers queries straight from GPUI's `InputHandler` (both use UTF-16 offsets); Flutter's editing state is kept for
      its batch-edit logic. Ledger rows list the parts left out.
- [ ] Text: Android system fonts and fallback for cosmic-text, sized by the Phase 0 font audit.
- [ ] Port the matching Flutter Android tests (`InputConnectionAdaptorTest`, `TextInputPluginTest`, …) as tests of the Java port.
- [ ] Lifecycle: pause, resume, low memory, mapped to upstream observers (patch if upstream lacks them on Android).
- [ ] `cargo-parallax`: `parallax.toml` schema and `cargo parallax run android` (generates the Gradle project around the Java port).
- [ ] Rust unit tests: key-code table, surface state machine, editing-state bridge.
- [ ] Ledger rows in `third_party/flutter/PORTED.md` for every ported file.

**Gate 1:** the example app runs on an Android device (API 28+) with Gboard and Samsung Keyboard: typing, autocorrect,
composing, selection, emoji, rotation, background/resume.

## Phase 2: Desktop touch

Always compiled in; behaviour follows pointer type at runtime. Each patch feeds upstream `TouchEvent` and drops the
fake mouse events the OS makes from touch, so a tap fires once.

- [ ] `0001-touch-windows`: `WM_POINTER` touch and pen in `gpui_windows`.
- [ ] `0002-touch-linux`: Wayland `wl_touch` and X11 XInput2 touch in `gpui_linux`.
- [ ] `0003-touch-macos`: touchscreen events in `gpui_macos`. Confirm the macOS 27 API first (open question).
- [ ] Every pointer event carries its device (mouse, touch, pen) and whether it was synthesized from touch
      (patch if upstream lacks it).
- [ ] Trace recorder: capture raw touch traces on each device (phone, browser, each touch laptop) into test fixtures.
- [ ] Trace replay: run the traces through the gesture and physics test tables on every platform.
- [ ] File each patch upstream.

**Gate 2:** replaying the recorded traces gives the same gestures and scroll outcomes on a phone, a browser and a
touchscreen laptop on each desktop OS.

## Phase 3: Flutter oracle, platform behaviour and accessibility

Flutter's tests check upstream GPUI. Where upstream fails, Flutter's fix goes in as a small patch and is filed upstream.

- [ ] iOS text input: `FlutterTextInputPluginTest.mm` cases → table-driven tests against `gpui_ios`; patch the failures.
      Limit: once these patches total about 800 lines or Zed rejects 3, move iOS text input into its own Parallax crate.
- [ ] Web text editing: `text_editing.dart` per-browser strategies and their tests → tests against `gpui_web`; patch or `parallax-web`.
- [ ] Gestures and physics: `velocity_tracker`, `lsq_solver`, `scroll_physics`, `friction_simulation`, `arena` test
      cases → tests against upstream `gestures.rs` and `ScrollPhysics`; patch the failures.
- [ ] Platform behaviour table: target → behaviour (Flutter's `TargetPlatform` defaults), desktop touch included.
      Covers scroll physics preset, gesture tuning, touch → mouse emulation, haptics, touch-target minimum.
- [ ] Text selection behaviour: selection handles, magnifier, text toolbar, ported from Flutter's behaviour (no Material/Cupertino look).
- [ ] Accessibility: `accesskit_android` in `parallax-android`; `parallax-web` semantics DOM; iOS and macOS per the
      audit. Flutter's `AccessibilityBridge` and `SemanticsObject` tests as the oracle.
- [ ] Remaining lifecycle and safe-area gaps from the audit, as patches.
- [ ] `parallax-ext`: extension traits only where the audit shows real friction (e.g. `.hit_slop()`).

**Gate 3:** the ported Flutter test tables pass on every target, and VoiceOver, TalkBack and a desktop screen
reader can operate the example app.

## Phase 4: Tooling, docs and v0.1

- [ ] `cargo parallax run ios` (generates the Xcode project) and `cargo parallax run web`.
- [ ] `parallax` facade crate: one dependency for apps.
- [ ] Docs: getting started per target, the touch → mouse rule and its opt-out, the behaviour table, Flutter credits.
- [ ] Example app polished for every target.
- [ ] License review: `NOTICE`, `third_party/flutter/PORTED.md` complete, crate license fields, no GPL editor code.
- [ ] Publish to crates.io.

**Gate 4:** Parallax v0.1 published.

## Open questions

- [ ] macOS 27: how are touchscreen events exposed? Needed before `0003-touch-macos`.
- [ ] Which Android `Looper` integration does GPUI's dispatcher need (ALooper fd callbacks vs a Java `Handler`)?
- [ ] Which patches to file upstream first.
- [ ] Is upstream adding Android itself? (`crates/gpui/Cargo.toml` already has a `target_os = "android"` dependency block.) Check before Phase 1 starts.
- [ ] When `zed-industries/gpui` holds code, move `upstream/` to it. Same code as `gpui-pre`, so only the source changes.
- [ ] `gpui-pre` is published by one person. If its snapshots stop, fall back to vendoring Zed `crates/gpui*` with a generated manifest.
- [ ] Will Zed accept patches whose code is BSD-3 (Flutter-derived) into an Apache-2.0 crate? Ask before filing the first one.

## Upstream notes

Facts about upstream (GPUI and Flutter) that shaped this plan. Add a line when upstream surprises you.

| Date | Source | Note |
|---|---|---|
| 2026-11-25 | zed `8aeb64f7` | `zed-industries/gpui` repo holds only a README ("See you in a few weeks"). |
| 2026-11-25 | zed `8aeb64f7` | Zed's `crates/gpui*` use Zed's `workspace = true` deps and don't build standalone. Tracking `gpui-pre` instead. |
| 2026-11-25 | gpui-pre 0.3.14 = zed `f126950` | `gpui-pre-*` on crates.io: weekly unmodified snapshots of Zed GPUI (incl. `gpui-pre-ios`, `gpui-pre-web`). Chosen as the interim base. |
| 2026-11-25 | gpui-ce main | `gpui-ce` has no `gpui_ios` and is moving to Parley text with API drift. Not used. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_ios` exists (added 2026-10-14, #63068): UIKit, Metal, CoreText, text input (49 KB), safe area, back gesture. |
| 2026-11-25 | zed `8aeb64f7` | Core gpui has `TouchEvent`, `gestures.rs` (tap→mouse, long press, pan, pinch, drag) and `ScrollPhysics::{ios, android}`. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_web` has touch (`events.rs`) and IME (`ime_mirror.rs`). Core has AccessKit (`window/a11y.rs`). |
| 2026-11-25 | zed `8aeb64f7` | No touch input on `gpui_windows`, `gpui_linux` or `gpui_macos`. No Android backend. |
| 2026-11-25 | flutter 3.50.1 | Pinned stable tag (stable branch `ae9b586f34`, 2026-11-19). Ported files exist unchanged at this tag. |
| 2026-11-25 | flutter `322dfd0771` (master) | BSD-3-Clause. Android text input: `TextInputPlugin.java` 36 KB, `InputConnectionAdaptor.java` 21 KB. iOS: `FlutterTextInputPlugin.mm` 141 KB + 207 KB of tests. Web: `text_editing.dart` 113 KB. |
| 2026-11-25 | accesskit | `accesskit_android` 0.9.1 exists (permissive: Apache, MIT, Chromium-derived BSD). Use it, don't port Flutter's `AccessibilityBridge`. |
