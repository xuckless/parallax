# ROADMAP

Parallax ships in eight phases, in order, across two rolling release lines. Each phase ends with a gate, and the
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
| v0.1.x | 0–5 and NativeView: foundation, Android, gestures and behaviour, desktop touch, text input, native views, tooling | 0.1.1: the working prototype skeleton (Phase 1) |
| v0.2.x | 6: accessibility | the first accessibility adapter that lands |

- Every gate that passes ships a release. Merged, verified work between gates may ship one too.
- The v0.1.x line ends when Gate 5 passes; v0.2.x starts then.
- Every release: CI green on all six targets, `NOTICE` and `third_party/flutter/PORTED.md` current, crate license
  fields right, no GPL editor code, a `CHANGELOG.md` entry.

## Wave plan

Waves batch the phase tasks below for parallel agents (see `AGENTS.md` Workflow). Every wave ends with a Caveman review
and a correctness review, then merge; a wave that changes what apps see ships the next release. Waves 1–7 built
Phase 0 and the 0.1.1 skeleton. Through Wave 12, up to 5 agents per wave; after it, one wave per phase with up to
8 agents (`CARGO_BUILD_JOBS=4`, turns on heavy builds, at most 2 emulators), split into two batches where that looks
unsafe or overloading (owner decision, 2026-11-26).

**Port before write** (owner decision, 2026-11-26): every remaining item names the proven code it ports (upstream
GPUI, Flutter, winit, AccessKit); new code is only the glue.

| Wave | Phase | Parallel work | Ships |
|---|---|---|---|
| 8 | 1 | Zed fork `xuckless/zed` (`parallax` branch) · keys (`KeyboardManager`, key-code table, back) · fonts (`fonts.xml`, fallback, locale) · Robolectric setup · lifecycle + device loss | 0.1.2 |
| 9 | 1 | `parallax-gpui*` fork crates · core patches (pointer device, touch opt-out, keyboard type) · Android emoji via Canvas/Typeface · text input port (`InputConnectionAdaptor` and friends) · hello's text field and multi-script screen | 0.1.3 |
| 10 | 1 | Flutter's Android Java tests · emulator matrix API 28/33/35 · runtime locale · Android mouse · per-touch device · Gate 1 fixes | 0.1.4 = **Gate 1** |
| 11 | 2 | Flutter's velocity tracker, scroll simulations, recognizer tuning, scale and overscroll maths ported into the fork · platform behaviour table | 0.1.6 |
| 12 | 2 | Pinch: `.on_pinch` with Flutter's `ScaleUpdateDetails` fields, wired through deepest-wins claims · overscroll on by default per the behaviour table, per-container opt-out · trace recorders (Android emulator, browser, iOS simulator via `idb`) + replay across platforms · haptics hook porting Flutter's `PlatformPlugin` / `FlutterPlatformPlugin` haptics · Android runtime touch slop from `ViewConfiguration` · smoke-test flake fixes | 0.1.7 = **Gate 2** |
| 13 | 3 | Desktop touch: winit's touch and pen event handling ported into GPUI's Windows, Linux and macOS backends · synthetic injection tests | 0.1.8 = **Gate 3** |
| 14 | 4 | Text input + selection: Flutter's iOS `UITextInput` cases + fixes, web `text_editing.dart` vs `ime_mirror`, Flutter's selection behaviour, native `UITextInteraction` | 0.1.9 = **Gate 4** |
| 15 | NativeView | Native views under GPUI: Wayland subsurface, wgpu hole, Windows DirectComposition, iOS, Android, web (from Chromium and Flutter) | 0.1.10 = **NativeView gate** |
| 16 | 5 | Tooling + docs: `cargo parallax run ios` / `run web`, Android release signing + App Bundle (from `flutter_tools`), getting-started docs | 0.1.11 = **Gate 5**, closes v0.1.x |
| 17 | 6 | Accessibility: AccessKit adapters (Android, iOS), web semantics DOM | first 0.2.x |

Known limits of this plan:

- **All patches reach crates.io through our published GPUI fork** (`parallax-gpui*`, patch rule 11).
  Upstreaming stays paused (rule 14); patches upstream fixes on its own are dropped at the next sync.
- **Hardware:** the owner has no real phones and no touchscreen laptop; their iPad gives macOS touch through Sidecar
  (macOS 27), and they have an Apple developer account. Gates are checked on emulators, simulators and synthetic touch
  injection until hardware is available.
- **Deferred past Gate 2:** Android multi-window (now in Phase 4).

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
- [x] Reserve the `parallax*` crate names on crates.io (`parallax`, `parallax-platform`, `parallax-android`,
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
- [x] `cargo-parallax`: `parallax.toml` schema and `cargo parallax run android` (generates the Gradle project around the Java port).
- [x] Main-thread dispatcher: GPUI's foreground executor on the Android main `Looper` via `ALooper_addFd` + an eventfd
      (no Java `Handler`). Small patch: export upstream's `PriorityQueue*` on Android (today only windows/linux/wasm).
- [x] Activity and View: write a small Parallax Activity and `SurfaceView`, using `FlutterActivity`, `FlutterView` and
      `FlutterSurfaceView` as reference for lifecycle, surface and insets.
- [x] Rendering: `gpui_wgpu` on the ported view's `Surface`, Vulkan with GL fallback. The backend choice is one visible table.
      The renderer is created on the main thread (about 0.5 s on the emulator); measure on a device and move it off
      only if startup exceeds the budget.
- [x] Surface lifecycle as one Rust state enum (none, ready, lost); recover from surface and device loss. Needed in the
      skeleton: in the spike, returning from Home failed on a stale surface.
- [x] Insets: `targetSdk` 35 forces edge-to-edge, so the window draws under the system bars; report safe-area insets.
- [x] Touch: port `AndroidTouchProcessor` and `MotionEventTracker`; emit upstream `TouchEvent`.
- [x] Example app: one window with a box that changes colour on tap, built for desktop, web and Android.
- [x] Publish 0.1.1.

**Milestone 0.1.1:** the example app opens, draws and reacts to a tap on desktop, web and the Android emulator
(owner decision: releases are emulator-verified until real devices are available).

Core patches (needed by Android from the start; iOS and web get them too):

- [x] Every pointer event carries its device (mouse, touch, pen) and whether it was synthesized from touch. Upstream
      has neither ([gap audit](docs/audits/gap-audit.md)).
- [x] Per-element opt-out of touch → mouse emulation: build upstream's declared-but-unused `ClickEvent::Touch`.
- [x] `TextInputConfiguration`: keyboard type, secure entry and autofill hint.
- [x] Run each patch's own GPUI tests in CI: the `patch-tests` job runs `scripts/patch-test.sh`
      ([docs/patch-tests.md](docs/patch-tests.md)).
- [x] Per-touch device attribution (Wave 10): a recognized gesture reports the device of its own touch (patch 0003).
- [x] Back key's `pointer_source` (Wave 10): `BackGestureEvent::source` (`EdgeSwipe` / `Key`); Android's Back key is
      `Key` and reports no pointer (patches 0003, 0006, 0008).
- [x] Touch opt-out for custom elements (Wave 10): `Window::leave_touch_to_gestures` is public (patch 0004).
- [ ] Web pens as `Pen`: `gpui_web` sends pens down the mouse path, so they read as `Mouse` (see Upstream notes); needs
      a device field on mouse events (~95 struct literals across backends, [core-patches.md](docs/core-patches.md)).
- [ ] `PointerDevice` still defaults to Touch (`#[default]`); drop the Default (needs `TouchEvent` without Default) — patch 0003.
- [x] Fork `zed-industries/zed` (`xuckless/zed`, `parallax` branch). Filing is paused (AGENTS.md rule 14).

Full Android:

- [x] Runtime locale changes (Wave 10): `onConfigurationChanged` sends the new locales and the font fallback follows
      ([android-fonts.md](docs/android-fonts.md#locale-changes)).
- [x] Android autofill (Wave 10): `TextInputChannel.Configuration.Autofill` and the autofill parts of `TextInputPlugin`
      ported; GPUI's hint is sent ([android-autofill.md](docs/android-autofill.md)).
- [ ] Autofill save prompts: GPUI has no finish-autofill-context API, so Rust never sends `finishAutofillContext`.
- [ ] Autofill field groups: GPUI announces only the focused field, so `setClient` has no `fields`.
- [ ] Autofill suggestion already on screen doesn't follow a moved field (ported Java doesn't re-call
      `notifyViewEntered`; Flutter behaves the same).
- [x] Mouse and trackpad as a real mouse (Wave 10): MouseDown/MouseMove/MouseUp with buttons and click counting, hover
      and wheel through `onGenericMotionEvent`, trackpad pans as scrolls ([android-mouse.md](docs/android-mouse.md)).
- [ ] Mouse hover enter/exit: Flutter's processor ignores `ACTION_HOVER_ENTER/EXIT`, so `is_hovered` stays false and
      leaving the view sends no `MouseExited`.
- [ ] A second mouse button during a drag (`ACTION_BUTTON_PRESS`) sends no down or up of its own.
- [ ] Chorded mouse: GPUI's div fires `on_click` on the first MouseUp after a Left down, whichever button (upstream
      desktop behaviour) — see Upstream notes.
- [ ] Pointer icons: `set_cursor_style` on Android.
- [x] Email keyboard (Wave 10): GPUI's default no-suggestions turned Flutter's email variation into
      `VARIATION_FILTER`; email, URL and search now always send suggestions on.
- [ ] Emoji picker stall on API 28 (owner decision, deferred): 100 new emoji cost a median 92 ms once (3–9 frames);
      API 35 is within budget. The off-thread design is in [android-emoji.md](docs/android-emoji.md#cost); build it
      if Gate 1 or a real device shows it matters.
- [x] Device loss: recovery uses upstream's `recover`, which re-picks Vulkan or GL itself; make it follow the
      backend table.
- [x] Keys: port `KeyboardManager` and `KeyEmbedderResponder`; Android key code → GPUI key as one Rust table.
      Back button → `BackGestureEvent`.
- [x] Text input: port `InputConnectionAdaptor`, `ListenableEditingState`, `TextEditingDelta`, `FlutterTextUtils`,
      `ImeSyncDeferringInsetsCallback` and the input parts of `TextInputPlugin`. On the same thread, the Rust side
      answers queries straight from GPUI's `InputHandler` (both use UTF-16 offsets); Flutter's editing state is kept for
      its batch-edit logic. Ledger rows list the parts left out.
- [x] Java test setup copied from Flutter's `test_runner`: Gradle, JDK 21 toolchain, Robolectric 4.17, Mockito 5.20 (JVM, no device).
- [x] Port the matching Flutter Android tests (`InputConnectionAdaptorTest`, `TextInputPluginTest`, `KeyboardManagerTest`,
      `AndroidTouchProcessorTest`, `ListenableEditingStateTest`, `TextEditingDeltaTest`) as tests of the Java port.
      Cases pinned below API 28 are dropped and listed in the ledger.
- [x] Java tests in CI: the `java-tests` job runs `scripts/java-test.sh` ([docs/ci.md](docs/ci.md)).
- [x] Fonts ([audit](docs/audits/android-fonts.md)): a small upstream patch so `CosmicTextSystem` takes our own font
      list; a `fonts.xml` / `font_fallback.xml` parser with table tests; the Android fallback table (per-language CJK,
      emoji); load fonts by path and prefer the CBDT `NotoColorEmojiLegacy.ttf` (swash can't draw COLRv1; superseded in Wave 9: Android draws emoji itself via Canvas/Typeface over JNI, patch 0007, [docs/android-emoji.md](docs/android-emoji.md)); pass the
      app locale over JNI. Fonts load on the main thread at startup; measure startup and move them off it only if it
      exceeds the budget.
- [x] Multi-script example screen (Latin, CJK, Arabic, emoji) checked on API 28, 33 and 35 ([gate-1.md](docs/gate-1.md)).
- [x] Lifecycle: implement upstream's existing lifecycle, memory-warning, inset and back hooks for Android. Map
      `onDestroy` to an explicit choice (upstream has no terminate phase).
- [ ] Rust unit tests: key-code table, surface state machine, editing-state bridge, fonts.xml parser.
- [x] Ledger rows in `third_party/flutter/PORTED.md` for every ported file.
- [x] CI emulator smoke (Wave 10): the `android-smoke` job runs `scripts/android-smoke.sh` on an API 35 emulator
      ([docs/android-smoke.md](docs/android-smoke.md)).
- [x] Gate 1 run on API 28, 33 and 35 with Gboard, recorded in [docs/gate-1.md](docs/gate-1.md) (Wave 10).
- [x] Gate 1 fix: GL first on emulators (API 33 Vulkan drew black, then garbled glyphs).
- [x] Gate 1 fix: a resize makes a new surface (API 28 kept the old-size frame after rotation).
- [x] Gate 1 fix: Android's default focus highlight is off (it dimmed every colour).
- [x] Gate 1 fix: a composition left behind when focus moves is ended (patch 0010, `Pending::commit`).
- [ ] Gboard select mode (Shift + arrows) doesn't extend the selection — moved to Phase 4 (owner deferred).
- [x] Patch 0010 tells the native IME (Wave 10): `PlatformWindow::end_composition` blurs and refocuses the web IME
      mirror while the browser composes (checked in headless Chromium); Android and iOS restart their sessions on a
      field change, so they keep the no-op ([core-patches.md](docs/core-patches.md)).
- [ ] macOS: verify patch 0010's discardMarkedText with a real Japanese IME (needs the owner's console session on pixl-mac).
- [ ] Web smoke test in CI (headless browser) — hello panicked on web unnoticed (no fonts on the web until Wave 10
      embedded `WEB_FONTS`, [examples.md](docs/examples-run.md#web)).
- [x] macOS draws text again: `gpui_platform`'s `font-kit` feature is on (0.1.1–0.1.4 built without it, so
      `gpui_macos` used GPUI's `NoopTextSystem` and drew no text); hello's desktop binary logs through `env_logger`.
- [ ] Desktop smoke test in CI that proves text draws: on the macOS runner (it has a window server) open hello,
      capture its window (or render to an offscreen texture) and fail when a text-bearing area has no ink. Same idea
      for Linux (Xvfb) and Windows. Nothing caught 0.1.1–0.1.4 shipping with no text on macOS.
- [ ] Web CJK in hello: no CJK font is embedded (several MB); gpui_web's `CanvasFontFallback::EmojiAndCjk` could draw
      it from the browser's fonts, but `gpui_platform` builds `WebPlatform` with the default (emoji only).
- [ ] Emulator GL uses RGB565 (wgpu-hal `choose_config` without sRGB): dark colours are quantized on emulators only.

- [x] **Gate 1** (Wave 10, 0.1.4; emulator-verified per owner decision, [gate-1.md](docs/gate-1.md)): the example app
      runs on the Android emulator (API 28, 33 and 35 images) with Gboard: typing, autocorrect, composing, selection,
      emoji, rotation, background/resume. Samsung Keyboard and real devices are added once the owner has devices to
      test on (until then, releases are emulator-verified).

Gate 1 needs Play images (Gboard as users get it, updated from Play) for API 28, 33 and 35. Installed under `~/Android/Sdk`
(2026-11-26): `system-images;android-28;google_apis;x86_64` and `system-images;android-35;google_apis;x86_64`, used by
the `parallax_*` and `Pixel_7_API35` AVDs; none is a Play image and there is no API 33 image. To install:
`system-images;android-28;google_apis_playstore;x86_64`, `system-images;android-33;google_apis_playstore;x86_64`,
`system-images;android-35;google_apis_playstore;x86_64`, plus one AVD on each.

## Phase 2: Gestures, physics and platform behaviour

Flutter's gesture and physics tests check upstream `gestures.rs` and `ScrollPhysics`. Where upstream fails, Flutter's
fix goes in as a patch to our published fork, with a draft kept in `drafts/upstream/`. The tables built here are what Phase 3 replays desktop traces through.
Tests drive GPUI through its test platform (touch events in, gestures and scroll offsets out); a small public-seam
patch only for a case that can't be reached that way. Order follows the [oracle inventory](docs/audits/oracle-inventory.md).

- [x] Velocity: `velocity_tracker` and `lsq_solver` cases. Wave 11: Flutter's trackers ported (patch 0011,
      [velocity-tracking.md](docs/velocity-tracking.md)).
- [x] Physics: `friction_simulation`, clamping and `scroll_physics` (fling) cases. Wave 11: GPUI's `ScrollPhysics` is
      Flutter's (`Bouncing { deceleration_rate }`, `Clamping`), one type for flings, overscroll and the table, with
      Flutter's physics module on upstream's springs (patch 0012, [scroll-physics.md](docs/scroll-physics.md)).
- [x] Recognizers: tap, double-tap, long-press and drag cases. (Flutter's `arena` is skipped: GPUI resolves
      competing gestures by claims, not an arena.) Wave 11: `constants.dart` and the slop rule ported once into GPUI,
      Flutter's double-tap rule counted per target, `isFlingGesture`, executor clock (patch 0013,
      [gesture-recognizers.md](docs/gesture-recognizers.md)).
- [x] Platform behaviour table: target → behaviour (Flutter's `TargetPlatform` defaults), desktop touch included.
      Covers scroll physics, gesture tuning, overscroll, haptics, touch-target minimum (touch → mouse is the same
      everywhere: GPUI's default). Wave 11: the table in `parallax_platform::behaviour`, patch 0014's
      `App::set_gesture_tuning`, applied by every entry point (`parallax::run`, the facade's `android_main!`)
      ([behaviour.md](docs/behaviour.md)).
- [x] Pinch (Wave 12): `.on_pinch` delivering Flutter's `ScaleUpdateDetails` fields, wired into touch dispatch through
      the deepest-wins claims. Wave 11 ported the recognizer as maths (patch 0015,
      [scale-and-overscroll.md](docs/scale-and-overscroll.md)); Flutter's `scale` tests are the oracle. Wave 12:
      patch 0016, who wins the fingers follows Flutter's arena ([pinch.md](docs/pinch.md)).
- [x] Overscroll (Wave 12) for `div` scroll containers: on by default per the behaviour table's row (bounce, glow,
      stretch, clamp), with a per-container opt-out; ports Wave 11's maths (patch 0015) into GPUI's scroll containers.
      Wave 12: patch 0017, with Flutter's scroll activities ([overscroll.md](docs/overscroll.md)).
- [ ] Overscroll for `uniform_list` and `list` (deferred from Wave 12: their visible range assumes an in-range offset).
- [ ] Left from Wave 11:
      - [x] Haptics (Wave 12, patch 0018, [haptics.md](docs/haptics.md)): GPUI has no haptics hook, so the table's `haptics` are data only. Port Flutter's
            `PlatformPlugin` (Android `View.performHapticFeedback`) and `FlutterPlatformPlugin` (iOS
            `UIFeedbackGenerator`) haptics behind a platform hook; callers: long press, selection handles, pickers.
      - [x] Android touch slop (Wave 12) from `ViewConfiguration.getScaledTouchSlop()` at runtime, as Flutter's
            embedding sends it; the row holds AOSP's default 8 dp.
      - [ ] iOS `carriedMomentum`: repeated flings speed up ([scroll-physics.md](docs/scroll-physics.md)).
      - [ ] Momentum's stop tolerance is `toleranceFor` at a fixed device pixel ratio of 2; pass the window's scale factor
            (`tolerance_for` already takes it).
      - [x] Touch timestamps (Wave 12, patch 0020, [touch-time-stamps.md](docs/touch-time-stamps.md)): `TouchEvent` had no timestamp, so velocity trackers time samples by dispatch; a stalled
            main thread delivers a burst that reads as a velocity spike. Add the platform's event time to `TouchEvent`
            (Android `MotionEvent.getEventTime`, iOS `UITouch.timestamp`, web `PointerEvent.timeStamp`) and track by it.
      - [ ] Scale: trackpad pan/zoom cases of `scale_test.dart` (GPUI reports trackpad pinches as `PinchEvent`).
      - [ ] Recognizers: Flutter's multi-pointer drag cases (GPUI's recognizer ignores extra touches).
      - [ ] Row values GPUI doesn't read yet: pan and scale slops (GPUI derives them from the touch slop,
            `gpui::slops`), `dragStartDistanceMotionThreshold` and touch-target minimum, until widgets read them.
            Wave 12: overscroll style and the long-press haptic are read.
- [x] Trace recorder (Wave 12, [traces.md](docs/traces.md)): capture raw touch traces into test fixtures on the Android emulator, the iOS simulator
      (driven by `idb`) and a browser, until the owner has devices.
- [x] Trace replay (Wave 12): run the traces through the gesture and physics test tables on every platform
      (`trace_replay`, on `gpui::play_trace`).
- [x] Smoke-test flake fixes (Wave 12).

**Gate 2** (0.1.7, emulator/simulator-verified) — **passed 2026-11-27**: 6 gestures × 3 platforms × 6 behaviour rows.
The ported gesture and physics tables pass on every target, and the
Android emulator, iOS simulator and browser traces replay to the same gestures and scroll outcomes everywhere.

## Phase 3: Desktop touch

Wave 13. Always compiled in; behaviour follows pointer type at runtime. Each patch feeds upstream `TouchEvent` and
drops the fake mouse events the OS makes from touch, so a tap fires once. Ports winit's (Apache-2.0/MIT) touch and pen
event handling into GPUI's desktop backends; Flutter's Windows and Linux embedders are the second reference.

- [x] `touch-windows` (patch 0021): `WM_POINTER` touch and pen in `gpui_windows`, from winit's Windows backend.
      Verified with injected input on CI's `windows-touch` job (13 runtime checks).
- [x] `touch-linux`: Wayland `wl_touch` (patch 0022, live under Weston headless) and X11 XInput2 touch (patch 0023,
      live through Xwayland, plus fixture rows) in `gpui_linux`, from winit's Wayland and X11 backends.
- [x] `touch-macos` (patches 0024–0026): direct touches in `gpui_macos`, from winit's macOS backend, on the macOS 27
      touch APIs: gesture recognizers with `allowedTouchTypes = .direct`, `NSIsTouchNative` opt-in,
      `NSScreen.touchCapabilities`, Apple's 36 pt touch target on the macOS row. Built and unit-tested.
- [ ] macOS live check on touch: the owner's Sidecar run with the iPad
      (`/Users/Shared/parallax/w13-macos-touch/CHECK.md` on `pixl-mac`).
- [x] Synthetic touch injection for tests: Windows `InjectSyntheticPointerInput` through `crates/touch-inject` on CI's
      Windows runner; Linux through Weston's headless test protocol (`weston_test.send_touch`), which replaced the
      planned `uinput` (no root, no session, real `wl_touch` and Xwayland XInput 2 touch).
- [x] iOS touch ids (patch 0027): one `TouchId` per `UITouch` lifetime (found by the trace identity check).
- [ ] Record traces on a touchscreen laptop for each desktop OS (once one is available) and add them to the Phase 2
      fixtures; until then, injected traces.
- [ ] Pen traces on X11, Wayland and the web (Windows injects pens; no second platform records them yet).
- [ ] Keep a draft for each patch in `drafts/upstream/`.

**Gate 3** (0.1.8, synthetic-verified: the owner has no touchscreen) — **passed 2026-11-27**: the Windows, Wayland and
X11 traces replay to the same gestures and scroll outcomes as the Android, web and iOS traces, within tolerance
(fling −1030 / −1069 / −1058 px/s against −1070…−1100; pinch scale 2.0 everywhere). macOS is pending the owner's
Sidecar check.

## Phase 4: Text input oracle and selection

Wave 14. Flutter's text-input tests check upstream GPUI. Where upstream fails, Flutter's fix goes in as a
patch to our published fork, with a draft kept in `drafts/upstream/`. Sources: Flutter's iOS `UITextInput` tests and
fixes (39 + 15 cases), web `text_editing.dart` strategies and tests, Flutter's selection behaviour, native
`UITextInteraction`.

- [ ] iOS text input: the 39 `UITextInput` behaviour cases from `FlutterTextInputPluginTest.mm` as a table against
      `gpui_ios`, then the 15 mapped cases (text traits, update counts); patch the failures. The other 81 test Flutter
      internals and are skipped. Limit: once these patches total about 800 lines (the "Zed rejects 3" trigger returns if upstreaming resumes), move iOS text
      input into its own Parallax crate.
- [ ] iOS hardware keyboard: `pressesBegan`/`pressesEnded` → `KeyDown`/`KeyUp`, shortcuts and arrow keys.
- [ ] iOS keyboard inset: intersect the keyboard frame with the window (floating iPad keyboards) and animate it.
- [ ] Web text editing: `text_editing.dart` per-browser strategies and their tests → tests against `gpui_web`
      (`ime_mirror.rs`); patch or `parallax-web`.
- [ ] Web: app lifecycle (`visibilitychange`) and the on-screen keyboard inset.
- [ ] Text selection: native `UITextInteraction` (handles, magnifier, edit menu) on iOS; Flutter's selection
      behaviour ported for Android and touch desktops (no Material/Cupertino look).
- [ ] Remaining lifecycle and safe-area gaps from the audit, as patches.
- [ ] Gboard select mode (Shift + arrows) extends the selection (deferred from Gate 1).
- [ ] Android multi-window (deferred past Gate 2): one GPUI window per process, so in split-screen only the most
      recently started Activity draws; decide whether to support a window per Activity.
- [ ] `parallax-ext`: extension traits only where the audit shows real friction (e.g. `.hit_slop()`).

**Gate 4** (0.1.9): the ported Flutter text-input tables pass on every target.

## Phase: NativeView

Wave 15, from [issue #1](https://github.com/xuckless/parallax/issues/1). A zero-overhead cutout: an OS-native view
(a browser page, video, map) shows under GPUI's content through a hole GPUI paints, and the view's own frames cause
no GPUI render. Upstream has it on macOS only. Sources: Chromium's overlay and subsurface code (BSD-3), Flutter's
platform views, WebView2's visual hosting.

- [ ] Wayland: the view as a `wl_subsurface` placed below the window's surface, with an empty input region;
      `set_sync` only for frames that move, resize, show or hide it, `desync` otherwise; `wp_viewport` for its size;
      fractional scale rounds the subsurface outwards and cuts the hole a physical pixel smaller. Port Chromium's
      `ui/ozone/platform/wayland/host/wayland_subsurface.cc` and `wayland_frame_manager.cc` (BSD-3; with its mutter
      buffer-release workaround). GStreamer's `waylandsink` layout sequence is reference only (LGPL).
- [ ] wgpu renderer hole: `PrimitiveBatch::NativeViews` clears the view's bounds to transparent in paint order, with
      Chromium's `ReplaceExistingQuadWithHolePunch` semantics; premultiplied alpha while a view shows; log the
      alpha mode chosen. Risk: NVIDIA's Vulkan WSI offers only opaque, so fall back to GL there.
- [ ] Windows: GPUI already presents through DirectComposition. A root container visual, the app's visual below
      GPUI's swapchain visual, a D3D11 hole, placement in GPUI's own DComp `Commit`. Model Chromium's
      `dc_layer_overlay.cc` / `dc_layer_tree.cc` and WebView2's visual hosting; never child HWNDs.
- [ ] iOS: a `UIView` / `CALayer` under a non-opaque `CAMetalLayer`, the Metal hole shared with macOS; Flutter's
      `FlutterPlatformViewsController.mm` as the oracle.
- [ ] Android: a `SurfaceView` underlay; Flutter's HCPP `PlatformViewsController2.java` as reference (API 34+), with
      TLHC-style copies below it.
- [ ] Web: a DOM slot under a transparent canvas, from Flutter's `slots.dart` and `content_manager.dart`.
- [ ] Input over the view reaches the element's listeners with view-relative positions; elements painted over it
      take their own input.
- [ ] The view's own frames cause 0 GPUI renders.
- [ ] X11: deferred (no underlay); `NativeView::new` keeps failing there.

Acceptance (issue #1, per target): an example with a view under the window showing a buffer that changes every frame
and a div over part of it; moving and resizing keeps hole and view in step with no gap or one-frame lag; pointer and
scroll over the view arrive with view-relative positions; 0 GPUI frames/s while only the view updates; the alpha
mode chosen is logged; no seams at 1.0 and a fractional scale (1.25). On Wayland: checked on GNOME/mutter with
NVIDIA (proprietary, Vulkan) and Mesa.

**NativeView gate** (0.1.10): the acceptance above passes on every listed target.

## Phase 5: Tooling and docs

Wave 16. Sources: Flutter's `flutter_tools` (Xcode project generation, Android release signing and App Bundle).

- [ ] `cargo parallax run ios` (generates the Xcode project, as `flutter_tools` does) and `cargo parallax run web`.
- [ ] Store builds for app developers (as `flutter_tools` does): `cargo parallax build android --release` signs with the app's own key and
      outputs an App Bundle (`.aab`) for Google Play. Keystore path and key alias go in `parallax.toml`; passwords
      come from environment variables or a prompt, never from the file. (Development builds stay debug-signed.)
- [ ] Docs: getting started per target, the touch → mouse rule and its opt-out, the behaviour table, Flutter credits.
- [ ] Example app polished for every target.

**Gate 5** (0.1.11): a fresh clone runs the example app on every target through `cargo parallax`, and the docs cover each one.
Closes the v0.1.x line.

## Phase 6: Accessibility (v0.2.x)

Wave 17. Each adapter ships as the next 0.2.x release when it lands. Sources: AccessKit's adapters
(`accesskit_android`, `accesskit_ios`) and a web semantics DOM; Flutter's `AccessibilityBridge` and `SemanticsObject`
tests are the oracle.

- [ ] Android: `accesskit_android` in `parallax-android`, at the version matching upstream's AccessKit (0.24 today → 0.7.x).
- [ ] Web: `parallax-web` semantics DOM (upstream web has no screen-reader output).
- [ ] iOS: new `parallax-ios` crate wiring `accesskit_ios` (0.1.0 matches AccessKit 0.24), plus a small patch adding the
      hook `gpui_ios` needs to hand over its view and tree. Note: `accesskit_ios` uses objc2 0.5, `gpui_ios` 0.6.
- [ ] macOS, Windows, Linux: upstream already has AccessKit adapters; check them with VoiceOver, Narrator/NVDA and
      Orca, and patch the gaps.

**Gate 6:** VoiceOver, TalkBack and a desktop screen reader can operate the example app.

## Open questions

- [x] macOS 27: how are touchscreen events exposed? Answered in Phase 3's `touch-macos` (gesture recognizers with
      `allowedTouchTypes = .direct`, `NSIsTouchNative`, `NSScreen.touchCapabilities`; Sidecar with the owner's iPad).
- [ ] Is a touchscreen laptop available for Phase 3 traces? Still none; synthetic injection only. macOS touch comes
      from the owner's iPad through Sidecar.
- [ ] Which patches to file upstream first (when upstreaming resumes).
- [ ] When `zed-industries/gpui` holds code, move `upstream/` to it. Same code as `gpui-pre`, so only the source changes.
- [ ] `gpui-pre` is published by one person. If its snapshots stop, fall back to vendoring Zed `crates/gpui*` with a generated manifest.
- [ ] Will Zed accept patches whose code is BSD-3 (Flutter-derived) into an Apache-2.0 crate? Paused with upstreaming; notes in `drafts/upstream/` (git-ignored).

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
| 2026-11-26 | zed `f126950f` | `TouchEvent` carries no timestamp, so the recognizer's velocity tracker times samples by when they are dispatched (`background_executor().now()`): a main-thread stall that delivers queued moves together reads as a velocity spike. Flutter tracks by each event's own time. |
| 2026-11-26 | gpui-pre 0.3.14 = zed `f126950` | `TouchGestureRecognizer` read `Instant::now()` while its long-press timer ran on the executor, so the test platform could not drive tap timing; its click count timed and measured taps up to up (400 ms, 16 px) with no minimum interval, so a split touch counted as a double tap. Patch 0013 ([gesture-recognizers.md](docs/gesture-recognizers.md)). |
| 2026-11-26 | gpui-pre 0.3.14 = zed `f126950` | Core already has `GestureTuning` (touch slop, multi-tap interval and slop, long press, `ScrollPhysics`, min fling) read once per window from `Platform::gestures()`; only `gpui_web` fills it (user-agent physics). Its tap shadow in `events.rs` hard-codes `GestureTuning::default().touch_slop`. |
| 2026-11-26 | flutter 3.50.1 | Flutter's per-platform gesture values are few: the touch slop comes from the device (Android sends `ViewConfiguration`'s; iOS, web and desktop send none, so 18), the rest is per scroll physics class, plus overscroll, haptics and the velocity tracker per `TargetPlatform`. Android's stretch vs glow follows `useMaterial3`, not the Android version. |
| 2026-11-26 | fontdb 0.23, swash 0.2.10 | fontdb has no Android font discovery (empty list, no warning); swash draws CBDT and COLR v0, not COLRv1. See the [font audit](docs/audits/android-fonts.md). |
| 2026-11-26 | gpui-pre 0.3.14 on Android emulator | GPUI draws via `gpui_wgpu` on Vulkan and GLES 3.1 with no glue patch; a backend needs ~10 `Platform` + ~12 `PlatformWindow` methods for real (~1,500 lines). Start with `Application::run_embedded`. `PriorityQueue*` is not exported on Android. Emulator needs `-gpu host` (`swiftshader_indirect` crashed). See the [spike](docs/android-spike.md). |
| 2026-11-26 | gpui-pre 0.3.14 = zed `f126950` | `gpui_web` turns only `pointerType == "touch"` into a `TouchEvent`; pens take the mouse path, so with patch 0003 a web pen reads as `PointerDevice::Mouse` (`events.rs`). |
| 2026-11-26 | gpui-pre 0.3.14 = zed `f126950` | Chorded mouse: a div's `on_click` fires on the first `MouseUpEvent` after a Left `MouseDownEvent`, whichever button goes up (`elements/div.rs`, `pending_mouse_down`); same on every desktop backend. |
| 2026-11-26 | gpui-pre 0.3.14 vs flutter 3.50.1 | GPUI's private `VelocityTracker` approximates only Flutter's base least-squares tracker (no iOS/macOS strategy): it passes Flutter's recorded traces, but reports an endpoint velocity from 2 samples where Flutter reports zero. Replaced by the port in patch 0011 ([velocity-tracking.md](docs/velocity-tracking.md)). |
| 2026-11-26 | wgpu-hal 30.0.2 | GL on Android: `choose_config` asks for no colour sizes unless EGL has sRGB (1.5 or `EGL_KHR_gl_colorspace`), so EGL's sort picks a 16-bit RGB 565 config; the emulator's EGL has neither. `configure` keeps the EGL window surface on resize. |
| 2026-11-25 | gpui-ce main | `gpui-ce` has no `gpui_ios` and is moving to Parley text with API drift. Not used. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_ios` exists (added 2026-10-14, #63068): UIKit, Metal, CoreText, text input (49 KB), safe area, back gesture. |
| 2026-11-25 | zed `8aeb64f7` | Core gpui has `TouchEvent`, `gestures.rs` (tap→mouse, long press, pan, pinch, drag) and `ScrollPhysics::{ios, android}`. |
| 2026-11-25 | zed `8aeb64f7` | `gpui_web` has touch (`events.rs`) and IME (`ime_mirror.rs`). Core has AccessKit (`window/a11y.rs`). |
| 2026-11-25 | zed `8aeb64f7` | No touch input on `gpui_windows`, `gpui_linux` or `gpui_macos`. No Android backend. |
| 2026-11-25 | flutter 3.50.1 | Pinned stable tag (stable branch `ae9b586f34`, 2026-11-19). Ported files exist unchanged at this tag. |
| 2026-11-25 | flutter `322dfd0771` (master) | BSD-3-Clause. Android text input: `TextInputPlugin.java` 36 KB, `InputConnectionAdaptor.java` 21 KB. iOS: `FlutterTextInputPlugin.mm` 141 KB + 207 KB of tests. Web: `text_editing.dart` 113 KB. |
| 2026-11-25 | flutter GitHub releases | `releases/latest` returns `3.19.0-0.1.pre` (a 2024 beta not marked pre-release). Find stable tags from the tag list, highest plain `X.Y.Z`. |
| 2026-11-25 | accesskit | `accesskit_android` exists (permissive: Apache, MIT, Chromium-derived BSD). Use it, don't port Flutter's `AccessibilityBridge`. 0.9.1 needs AccessKit 0.25; upstream pins 0.24, so use the 0.7.x line until upstream bumps. |
| 2026-11-26 | zed `f126950f` | A tap's focus change is drawn inside the touch dispatch (`draw_before_touch_release`), so a platform learns of it after the left field's input handler is dropped; core ends a composition on focus moves only toward a native view (`end_composition_before_focusing_native_view`). Patch 0010 ends it in the draw. |
| 2026-11-26 | zed `f126950f` | `notify` inside a focus listener (`on_blur`, …) is ignored: listeners run in the draw's Focus phase, where `invalidate_view` marks nothing dirty. Defer it (`defer_in`). |
| 2026-11-26 | zed `f126950f` | `ScrollPhysics::android()` ran AOSP's `SplineOverScroller` spline, which Flutter replaced (flutter/flutter#120338) because it is not ballistic (starts below the release velocity, stops with a jump): fails Flutter's clamping tests. Patch 0012 runs Flutter's `ClampingScrollSimulation`. |
| 2026-11-26 | gpui-pre 0.3.14 | `gpui_platform`'s `font-kit` feature is off by default and is the only thing that gives `gpui_macos` a real text system; without it macOS silently uses `NoopTextSystem` (one `log::warn!`). `gpui_linux` and `gpui_ios` turn their font-kit on themselves. |
| 2026-11-27 | zed `f126950f` | `Window::invalidate_character_coordinates` queries `selected_bounds` in an `on_next_frame` callback, which runs before that frame's layout: called where the selection changes, it reports the caret's previous position. Call it after the paint that moved the caret (hello's text field does). On the web, `update_ime_position` was a no-op and `on_app_lifecycle` / `on_insets_changed` were never registered (patches 0034, 0035). |
| 2026-11-27 | gpui-pre 0.3.14 = zed `f126950` | `gpui-pre-ios`'s own `tests/text_input.rs` (a `harness = false` UIKit binary for the simulator) does not build as published: the package dropped its `slotmap` dev-dependency and objc2-ui-kit's `UIConversationContext` feature. Patch 0028 adds them back. |
| 2026-11-27 | gpui-pre 0.3.14 = zed `f126950` | gpui_ios's `insertText:`/`setMarkedText:`/`replaceRange:` used `NSString::to_string`, which for half a surrogate pair (what iOS 16's partial-emoji bug sends) reads a NULL `UTF8String`: undefined behaviour, an abort in debug builds. Patch 0029. |
| 2026-11-27 | Flutter `3.50.1` | Flutter's `KeyCodeMap.g.mm` maps `UIKeyInputEnd` to Enter's logical key (a table bug); patch 0031 maps it to End. |
| 2026-11-27 | gpui-pre 0.3.14 = zed `f126950` | GPUI's `SpringConfig` treats the iOS keyboard spring (mass 3, stiffness 1000, damping 500) as overdamped (about 70% of the way in 0.6 s), so it cannot stand in for Flutter's keyboard-inset spring; patch 0032 samples the keyboard's presentation layer instead. |
