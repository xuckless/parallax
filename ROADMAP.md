# ROADMAP

Parallax ships in five phases. Each one ends with a gate, and the next phase starts only after the gate passes.
No dates are set. Tick a box only when the work is merged and verified (see `AGENTS.md`).

Upstream already ships iOS, touch and gestures, web touch and IME, and a core accessibility tree.
So Parallax's work is: **Android, desktop touch, accessibility adapters, lifecycle gaps and build tooling.**

## Phase 0: Foundation and gap audit

- [ ] Vendor the pinned `gpui-pre-*` crates (`gpui-pre`, `gpui-pre-ios`, `gpui-pre-web`, `gpui-pre-wgpu`, the desktop
      platform crates and their `gpui-pre-*` helpers) into `upstream/` with `cargo vendor`. Record the version
      and the Zed commit it snapshots in `upstream/UPSTREAM`.
- [ ] `scripts/apply-patches.sh`: pristine `upstream/` + `patches/` → `build/gpui-patched/`. Deterministic, idempotent, fails loudly.
- [ ] Wire `[patch]` in the root `Cargo.toml` to `build/gpui-patched/`.
- [ ] `scripts/sync-upstream.sh`: bump to the latest `gpui-pre` snapshot, re-vendor, re-apply patches, list the ones that no longer apply.
- [ ] CI matrix: iOS, Android, wasm, macOS, Windows, Linux, each with and without feature `touch`.
- [ ] Gap audit: run upstream's iOS and web examples, then write down what is missing for touch, IME,
      lifecycle (background, active, low memory), safe area and accessibility. The results go into the phases below.
- [ ] `parallax-platform`: one target → platform table (desktop, web, iOS upstream; Android later).
- [ ] Reserve `parallax*` crate names on crates.io and the GitHub org name.
- [ ] Set up a GitHub fork of `zed-industries/zed` for filing upstream PRs (patches are re-based onto Zed main there).

**Gate 0:** a fresh clone builds `build/gpui-patched/` from scripts alone, and the CI matrix is green with zero patches.

## Phase 1: Android

- [ ] `parallax-android`: GameActivity via `android-activity`, `Platform` and `PlatformWindow`.
- [ ] Rendering: `gpui_wgpu` on `ANativeWindow`, Vulkan with GL fallback. The backend choice is one visible table.
- [ ] Surface lifecycle as one state enum (none, ready, lost); recover from surface and device loss.
- [ ] Input: touch → upstream `TouchEvent`, key codes → GPUI keys (one table), back button → `BackGestureEvent`.
- [ ] Text input: GameTextInput / `InputConnection` bridged to `InputHandler`. Soft keyboard shown on focus.
- [ ] Insets: system bars and IME insets fed to upstream's safe-area API.
- [ ] Lifecycle: pause, resume, low memory, mapped to upstream observers (patch if upstream lacks them on Android).
- [ ] `cargo-parallax`: `parallax.toml` schema and `cargo parallax run android` (generates the Gradle project).
- [ ] Unit tests: key-code table, surface state machine, inset maths.

**Gate 1:** the example app runs on an Android device (API 28+) with working text input, rotation and background/resume.

## Phase 2: Desktop touch

All behind feature `touch`. Each patch feeds upstream `TouchEvent` and drops the fake mouse events the OS
makes from touch, so a tap fires once.

- [ ] `0001-touch-windows`: `WM_POINTER` touch and pen in `gpui_windows`.
- [ ] `0002-touch-linux`: Wayland `wl_touch` and X11 XInput2 touch in `gpui_linux`.
- [ ] `0003-touch-macos`: touchscreen events in `gpui_macos`. Confirm the macOS 27 API first (open question).
- [ ] Pointer type (mouse, touch, pen) on events, if upstream does not carry it yet.
- [ ] Target → `ScrollPhysics` / `GestureTuning` preset table, including desktop touch.
- [ ] File each patch upstream.

**Gate 2:** the same gestures feel alike on a phone, a browser and a touchscreen laptop on each desktop OS.

## Phase 3: Accessibility and lifecycle gaps

- [ ] Android AccessKit adapter in `parallax-android`.
- [ ] `parallax-web`: semantics DOM mirroring the AccessKit tree, so screen readers can read the canvas.
- [ ] iOS and macOS accessibility: patch or adapter, per the Phase 0 audit.
- [ ] Remaining lifecycle and safe-area gaps from the audit, as patches.
- [ ] `parallax-ext`: extension traits only where the audit shows real friction (e.g. `.hit_slop()`).

**Gate 3:** VoiceOver, TalkBack and a desktop screen reader can navigate and operate the example app.

## Phase 4: Tooling, docs and v0.1

- [ ] `cargo parallax run ios` (generates the Xcode project) and `cargo parallax run web`.
- [ ] `parallax` facade crate: one dependency for apps.
- [ ] Docs: getting started per target, the touch → mouse rule and its opt-out, the preset tables.
- [ ] Example app polished for every target.
- [ ] License review: `NOTICE`, vendored licenses, no GPL editor code.
- [ ] Publish to crates.io under Apache 2.0.

**Gate 4:** Parallax v0.1 published.

## Open questions

- [ ] macOS 27: how are touchscreen events exposed? Needed before `0003-touch-macos`.
- [ ] Which patches to file upstream first.
- [ ] When `zed-industries/gpui` holds code, move `upstream/` to it. Same code as `gpui-pre`, so only the source changes.
- [ ] `gpui-pre` is published by one person. If its snapshots stop, fall back to vendoring Zed `crates/gpui*` with a generated manifest.
- [ ] Is upstream adding Android itself? (`crates/gpui/Cargo.toml` already has a `target_os = "android"` dependency block.) Check before Phase 1 starts.

## Upstream notes

Facts about upstream that shaped this plan. Add a line when upstream surprises you.

| Date | Upstream commit | Note |
|---|---|---|
| 2026-11-25 | zed `8aeb64f7` | `zed-industries/gpui` repo holds only a README ("See you in a few weeks"). |
| 2026-11-25 | zed `8aeb64f7` | Zed's `crates/gpui*` use Zed's `workspace = true` deps and don't build standalone. Tracking `gpui-pre` instead. |
| 2026-11-25 | gpui-pre 0.3.14 = zed `f126950` | `gpui-pre-*` on crates.io: weekly unmodified snapshots of Zed GPUI (incl. `gpui-pre-ios`, `gpui-pre-web`). Chosen as the interim base. |
| 2026-11-25 | gpui-ce main | `gpui-ce` has no `gpui_ios` and is moving to Parley text with API drift. Not used. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_ios` exists (added 2026-10-14, #63068): UIKit, Metal, CoreText, text input, safe area, back gesture. |
| 2026-11-25 | zed `8aeb64f7` | Core gpui has `TouchEvent`, `gestures.rs` (tap→mouse, long press, pan, pinch, drag) and `ScrollPhysics::{ios, android}`. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_web` has touch (`events.rs`) and IME (`ime_mirror.rs`). Core has AccessKit (`window/a11y.rs`). |
| 2026-11-25 | zed `8aeb64f7` | No touch input on `gpui_windows`, `gpui_linux` or `gpui_macos`. No Android backend. |
