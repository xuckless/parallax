# Gap audit: gpui-pre 0.3.14 (zed `f126950`)

What upstream GPUI lacks for touch, text input, lifecycle, insets and accessibility, per platform. Phase 0 task in
[ROADMAP.md](../../ROADMAP.md). MISSING = nothing there; PARTIAL = what is and isn't there; PRESENT = usable as is.
Audited from source in [`upstream/`](../../upstream/). The upstream iOS example was also built from crates.io
`=0.3.14` and launched on an iPhone 17 Pro simulator (iOS 26.5): it draws inside the safe area. It was not tapped,
since `simctl` has no input, and the web example was not run. Re-check every row at each sync.

## Core (`gpui-pre`)

| Area | Status | Evidence | Phase |
|---|---|---|---|
| `TouchEvent` | PARTIAL: id, phase, position, predicted position, force. No device (finger/pen/indirect pointer). Its own doc says per-touch hit-test dispatch is "pending" | [interactive.rs#L113](../../upstream/gpui-pre/src/interactive.rs#L113) | 2 |
| Pointer device + synthesized tag | MISSING: `MouseDownEvent` has no device or synthesized field; taps become plain mouse events. Only signal is window-wide `last_input_was_touch()` | [interactive.rs#L148](../../upstream/gpui-pre/src/interactive.rs#L148), [window.rs#L3825](../../upstream/gpui-pre/src/window.rs#L3825) | 2 |
| Touch → mouse opt-out per element | MISSING: every tap is synthesized as mouse down/up; no listener sees raw touches. `ClickEvent::Touch` / `TouchClickEvent` is declared but never built | [window.rs#L6331](../../upstream/gpui-pre/src/window.rs#L6331), [interactive.rs#L275](../../upstream/gpui-pre/src/interactive.rs#L275) | 2 |
| Gestures | PARTIAL: tap, multi-tap, long press, pan, fling, touch drag, back gesture. Pinch from touch not implemented; extra fingers ignored while one is tracked (`PinchEvent` comes only from trackpads) | [gestures.rs#L523](../../upstream/gpui-pre/src/gestures.rs#L523) | 2 |
| `GestureTuning` / `PlatformGestures` | PRESENT: slop, multi-tap, long-press, physics, min fling. Default is iOS-flavoured; only web overrides it | [gestures.rs#L110](../../upstream/gpui-pre/src/gestures.rs#L110), [#L477](../../upstream/gpui-pre/src/gestures.rs#L477) | 2 (table) |
| `ScrollPhysics` | PARTIAL: `ios()` exponential and `android()` `OverScroller` spline, lsq2 velocity tracker. No overscroll (bounce, glow, clamp): left to containers, and none implements it | [gestures.rs#L140](../../upstream/gpui-pre/src/gestures.rs#L140), [#L1086](../../upstream/gpui-pre/src/gestures.rs#L1086) | 2 |
| Lifecycle API | PRESENT: `AppLifecyclePhase` (Active, Inactive, Background, Foreground, Disconnected) with Android mapping documented, `on_memory_warning`. No termination phase; `Disconnected` is not one | [platform.rs#L1217](../../upstream/gpui-pre/src/platform.rs#L1217), [app.rs#L1893](../../upstream/gpui-pre/src/app.rs#L1893) | 1 (use it) |
| Insets API | PRESENT: `WindowInsets { safe_area, ime, corner_radii }`, `insets()`, `on_insets_changed`, back handler. Changes only refresh the window; no app-level observer | [platform.rs#L1240](../../upstream/gpui-pre/src/platform.rs#L1240), [#L1493](../../upstream/gpui-pre/src/platform.rs#L1493), [window.rs#L2158](../../upstream/gpui-pre/src/window.rs#L2158) | 1 (use it) |
| Keyboard type | MISSING: `TextInputConfiguration` has autocorrect, autocapitalize, suggestions, action; no keyboard type (number, email, URL, phone), no secure entry, no autofill hint | [platform.rs#L2634](../../upstream/gpui-pre/src/platform.rs#L2634) | 4 |
| A11y tree | PRESENT: AccessKit 0.24 tree built in prepaint, `A11yCallbacks`, per-window `a11y_init`; `div` gets Click and Focus actions. Text runs and text selection only if the app builds them | [window/a11y.rs](../../upstream/gpui-pre/src/window/a11y.rs), [platform.rs#L1524](../../upstream/gpui-pre/src/platform.rs#L1524), [Cargo.toml#L290](../../upstream/gpui-pre/Cargo.toml#L290) | 6 |

## iOS (`gpui-pre-ios`)

| Area | Status | Evidence | Phase |
|---|---|---|---|
| Touch | PARTIAL: `touchesBegan…` → `TouchEvent` with force, multitouch on. No `UITouch.type` (Pencil, iPad pointer clicks arrive as finger touches), no coalesced or predicted samples, no hover | [window.rs#L527](../../upstream/gpui-pre-ios/src/ios/window.rs#L527), [#L925](../../upstream/gpui-pre-ios/src/ios/window.rs#L925) | 2 |
| Trackpad/wheel scroll, back swipe | PRESENT: scroll-only pan recognizer; edge-pan → `BackGestureEvent` | [window.rs#L362](../../upstream/gpui-pre-ios/src/ios/window.rs#L362) | none |
| `UITextInput` | PARTIAL: marked text, selection rects, caret/first rect, hit testing, string tokenizer, autocorrect/caps/spell/inline-prediction/return-key traits. Missing: keyboard type (always `Default`), secure entry, `textContentType`, writing direction (Natural only), dictation placeholders, `textWillChange` before the edit | [text_input.rs#L225](../../upstream/gpui-pre-ios/src/ios/text_input.rs#L225), [#L309](../../upstream/gpui-pre-ios/src/ios/text_input.rs#L309), [#L403](../../upstream/gpui-pre-ios/src/ios/text_input.rs#L403), [#L184](../../upstream/gpui-pre-ios/src/ios/text_input.rs#L184) | 4 |
| Selection UI | MISSING: no `UITextInteraction`, so no handles, loupe or edit menu (copy, paste, select all) | [text_input.rs](../../upstream/gpui-pre-ios/src/ios/text_input.rs) (no use of it) | 4 |
| Hardware keyboard | MISSING: no `pressesBegan`/`UIKeyCommand`, so no `KeyDown`/`KeyUp`, shortcuts, arrows, Esc or Tab as keys; text still arrives via `insertText`. Caps Lock stubbed, layout change a no-op | [window.rs#L889](../../upstream/gpui-pre-ios/src/ios/window.rs#L889), [platform.rs#L459](../../upstream/gpui-pre-ios/src/ios/platform.rs#L459) | 4 |
| Lifecycle | PRESENT: scene active, inactive, foreground, background, disconnect; display link paused in background; memory warning; `applicationWillTerminate` → `on_quit` | [application.rs#L253](../../upstream/gpui-pre-ios/src/ios/application.rs#L253), [#L345](../../upstream/gpui-pre-ios/src/ios/application.rs#L345) | none |
| Safe area | PRESENT: `safeAreaInsets` on every layout, display corner radii on iOS 26 | [window.rs#L654](../../upstream/gpui-pre-ios/src/ios/window.rs#L654), [#L745](../../upstream/gpui-pre-ios/src/ios/window.rs#L745) | none |
| Keyboard inset | PARTIAL: raw `UIKeyboardFrameEnd` height, not intersected with the view (wrong for floating/undocked iPad keyboards and split view); jumps to the end frame, despite the trait contract to follow the animation | [window.rs#L478](../../upstream/gpui-pre-ios/src/ios/window.rs#L478), [platform.rs#L1500](../../upstream/gpui-pre/src/platform.rs#L1500) | 4 |
| VoiceOver | MISSING: no `a11y_init`/`a11y_tree_update` and no UIAccessibility code; the core tree never reaches iOS. `accesskit_ios` 0.1.0 matches AccessKit 0.24 but uses objc2 0.5 (gpui_ios: 0.6) | [window.rs#L1006](../../upstream/gpui-pre-ios/src/ios/window.rs#L1006) (trait impl, no a11y methods), [Cargo.toml#L81](../../upstream/gpui-pre-ios/Cargo.toml#L81) | 6 |

## Web (`gpui-pre-web`)

| Area | Status | Evidence | Phase |
|---|---|---|---|
| Touch / pointer | PARTIAL: `pointerType == "touch"` → `TouchEvent` with predicted position. Pen goes down the mouse path; `pressure` ignored (`force: None`) | [events.rs#L254](../../upstream/gpui-pre-web/src/events.rs#L254), [#L276](../../upstream/gpui-pre-web/src/events.rs#L276), [#L286](../../upstream/gpui-pre-web/src/events.rs#L286) | 2 |
| IME (`ime_mirror.rs`) | PARTIAL: hidden `<textarea>` mirroring 512 UTF-16 units around the caret, composition and `beforeinput`, Android IME fixes, autocorrect/caps/enterkeyhint. One strategy for all browsers (Flutter keeps one per browser); `inputmode` only toggles `none` | [ime_mirror.rs#L1](../../upstream/gpui-pre-web/src/ime_mirror.rs#L1), [#L144](../../upstream/gpui-pre-web/src/ime_mirror.rs#L144) | 4: [0034](../web-text-input.md) ports Flutter's attribute, `inputmode` and placement rules; `type=password` open |
| Focus | PRESENT: window `focus`/`blur` → active status | [events.rs#L1176](../../upstream/gpui-pre-web/src/events.rs#L1176) | none |
| Lifecycle | PARTIAL: `visibilitychange` → window visibility. No `on_app_lifecycle` (no `pagehide`, `freeze`, `resume`, bfcache) | [window.rs#L525](../../upstream/gpui-pre-web/src/window.rs#L525) | 4: [0035](../web-lifecycle.md) |
| Safe area / keyboard | PARTIAL: `env(safe-area-inset-*)` probes feed `safe_area`; `ime` is always zero (keyboard only via `visual_viewport_bounds`); `on_insets_changed` not implemented | [viewport.rs#L78](../../upstream/gpui-pre-web/src/viewport.rs#L78), [window.rs#L835](../../upstream/gpui-pre-web/src/window.rs#L835) | 4: [0035](../web-lifecycle.md) fills `ime` and `on_insets_changed` |
| Screen reader | MISSING: canvas only, no DOM semantics, no ARIA, no `a11y_init` | [window.rs](../../upstream/gpui-pre-web/src/window.rs) (no a11y methods) | 6 |

## Desktop (`gpui-pre-macos`, `-windows`, `-linux`)

| Area | Status | Evidence | Phase |
|---|---|---|---|
| Touch, Windows | MISSING: no `WM_POINTER`/`WM_TOUCH`; touch reaches GPUI as OS-synthesized `WM_LBUTTONDOWN`, untagged | [events.rs#L135](../../upstream/gpui-pre-windows/src/events.rs#L135) | 3 |
| Touch, Linux | MISSING: Wayland binds only keyboard and pointer seat capabilities; X11 XI2 mask has no touch events | [wayland/client.rs#L1962](../../upstream/gpui-pre-linux/src/linux/wayland/client.rs#L1962), [x11/window.rs#L724](../../upstream/gpui-pre-linux/src/linux/x11/window.rs#L724) | 3 |
| Touch, macOS | MISSING: mouse, pressure and trackpad pinch only | [events.rs#L203](../../upstream/gpui-pre-macos/src/events.rs#L203) | 3 |
| AccessKit adapters | PRESENT on all three: `accesskit_macos`, `accesskit_windows`, `accesskit_unix` (Wayland and X11). Screen-reader behaviour not yet checked | [macos window.rs#L2531](../../upstream/gpui-pre-macos/src/window.rs#L2531), [windows window.rs#L1082](../../upstream/gpui-pre-windows/src/window.rs#L1082), [wayland window.rs#L2282](../../upstream/gpui-pre-linux/src/linux/wayland/window.rs#L2282) | 6 |
| Lifecycle | PRESENT: activation, visibility, sleep/wake, quit. `on_app_lifecycle` is mobile-only by design | [macos platform.rs#L1072](../../upstream/gpui-pre-macos/src/platform.rs#L1072), [platform.rs#L506](../../upstream/gpui-pre/src/platform.rs#L506) | none |

## Changes to the roadmap

- Phase 2: replace "patch if upstream lacks it" with a firm patch. Upstream lacks pointer device and synthesized tags, and pen on iOS and web.
- Phase 2, add: core patch that delivers raw touches and lets an element opt out of tap → mouse (build `ClickEvent::Touch`).
- Phase 2, add: multi-touch and pinch recognition in `gestures.rs` (Flutter `scale.dart` as oracle).
- Phase 2, add: overscroll (bounce, glow, clamp) to the behaviour table and a scroll-container patch.
- Phase 1: lifecycle, memory warning, insets and back are already in the core traits with the Android mapping; implement them, drop "patch if upstream lacks them". Map `onDestroy` to an explicit choice (core has no terminate phase).
- Phase 1 and 4, add: keyboard type, secure entry and autofill hint in `TextInputConfiguration` (core patch; Android, iOS and web need it).
- Phase 4, add: iOS hardware keyboard (`pressesBegan` → `KeyDown`/`KeyUp`), iOS keyboard inset (intersect, animate), web `on_app_lifecycle` and `ime` inset.
- Phase 4: selection handles on iOS need a decision for the owner: native `UITextInteraction` or Flutter's behaviour port.
- Phase 6 iOS: a whole adapter (`accesskit_ios`), not "gaps as patches". It exceeds the 200-line patch limit and needs objc2 0.5 next to 0.6. Owner decision.
- Phase 6 Android: `accesskit_android` 0.9.1 needs AccessKit 0.25; upstream pins 0.24, so use 0.7.x or bump upstream. Fix the Upstream note.
- Upstream notes, add: the vendored `gpui-pre-ios` example does not build (uses `gpui_platform`, not a dev-dependency); macOS, Windows and Linux AccessKit adapters exist.
