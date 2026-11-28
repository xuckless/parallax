# OS reduce-motion and reduce-transparency settings

GPUI apps follow the user's "reduce motion" and "reduce transparency" settings on every target. Upstream already has
`App::reduce_motion` (its animation element, animated SVGs and animated images render a static frame when it is set),
but no platform set it. [Patch 0039](../patches/0039-os-accessibility-display-settings.patch) adds the platform hook
and `App::reduce_transparency`; [0040](../patches/0040-linux-reduce-motion.patch),
[0041](../patches/0041-windows-web-accessibility-display.patch) and [parallax-android](../crates/android/src/accessibility.rs)
fill it per backend. Plan: [ROADMAP.md](../ROADMAP.md) Phase 4 follow-ups; Phase 7's motion and Phase 10's materials read them.

## The API

- `cx.reduce_motion()` / `cx.reduce_transparency()`: the OS's settings when the app started, then each change the user
  makes. Both start `false` where a platform reads nothing.
- `cx.set_reduce_motion(..)` / `cx.set_reduce_transparency(..)`: an app override. It holds until the user next changes
  that OS setting; then the OS wins. A report in which that setting did not move (the user changed the other one, or
  macOS's one notification for every display option) keeps the override. Either change redraws every window.
- Backends implement `Platform::accessibility_display_settings()` (read now; `None` while a late reader has not
  answered) and `on_accessibility_display_settings_change` (call back with `AccessibilityDisplaySettings` outside any
  GPUI borrow). A late reader's first report is the startup value: it fills only what the app has not set itself, so
  a `set_reduce_motion` in `launch` holds. Linux's portal is the one late reader.
- `App::follow_os_display` applies a report field by field, by `os_report_applies` (the OS's last value, its value
  now, who set the app's: `SetBy::{Os, App}`).

## Per target

| Target | Reduce motion | Reduce transparency | Follows changes by | Source |
|---|---|---|---|---|
| iOS | `UIAccessibilityIsReduceMotionEnabled` | `UIAccessibilityIsReduceTransparencyEnabled` | their two did-change notifications, deferred to the executor | React Native `RCTAccessibilityManager.mm` |
| macOS | `NSWorkspace.accessibilityDisplayShouldReduceMotion` | `…ShouldReduceTransparency` | `NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification`, deferred | AppKit |
| Android | `TRANSITION_ANIMATION_SCALE` or `ANIMATOR_DURATION_SCALE` is 0 ("Remove animations", developer options) | none (Android has no setting) | a `ContentObserver` on both, registered while resumed | React Native `AccessibilityInfoModule.kt` |
| Windows | `SPI_GETCLIENTAREAANIMATION` off (Animation effects) | `EnableTransparency` = 0 (Transparency effects) | `WM_SETTINGCHANGE` on the platform window, posted to itself first | Win32 |
| Linux | portal `org.freedesktop.appearance` `reduced-motion` = 1, or GNOME `enable-animations` false | none (no portal key) | the portal's `SettingChanged`, through gpui_linux's ashpd listener | xdg-desktop-portal |
| Web | `(prefers-reduced-motion: reduce)` | `(prefers-reduced-transparency: reduce)` (Chromium only) | each query's `change` event | CSS |

The pure decisions (raw values → booleans) are tables with a test each: Android's scale strings
([accessibility.rs](../crates/android/src/accessibility.rs), React Native's parsing: `"0"`, `"0.0"`, comma decimals,
junk), Windows' two values (gpui_windows `accessibility_display.rs`, host-tested by
[patch-test.sh](../scripts/patch-test.sh)'s `windows-pure` crate), the portal's two keys (gpui_linux `PortalMotion`),
and the app's override rules (core `test_os_report_applies`, `test_os_accessibility_display_settings`). Apple and the web read booleans.

## Android glue

[`AccessibilityInfoModule.java`](../crates/android/java/io/github/xuckless/parallax/AccessibilityInfoModule.java) (React
Native's module, Kotlin → Java, the reduce-motion parts) sends both scales' raw `Settings.Global` strings through
`ParallaxSurfaceView.nativeAccessibilitySettings` on each Activity's create and resume and on every change.
[host.rs](../crates/android/src/host.rs) keeps the latest in the `Shell` (GPUI reads it when it starts, after the first
Activity's create) and calls GPUI back only when the outcome moved. React Native and Flutter read only the
transition scale; Parallax adds the animator scale, the one Android's own `ValueAnimator` obeys. Both are app-wide:
several Activities send the same values.

## Checking it

- Android: `adb shell settings put global transition_animation_scale 0` (back: `1`); logcat shows
  `parallax: accessibility display settings: …`.
- Web: headless Chromium with CDP `Emulation.setEmulatedMedia` features `prefers-reduced-motion` /
  `prefers-reduced-transparency`.
- Linux: `gsettings set org.gnome.desktop.interface enable-animations false` (GNOME).
- macOS / iOS: System Settings > Accessibility > Display; on the simulator, Settings > Accessibility > Motion.
