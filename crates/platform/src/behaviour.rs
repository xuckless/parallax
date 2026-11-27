// Ported from flutter/flutter @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7): the constants of
// packages/flutter/lib/src/gestures/constants.dart and the per-TargetPlatform defaults of (under
// packages/flutter/lib/src/) gestures/events.dart (computeHitSlop and friends), gestures/gesture_settings.dart,
// widgets/scroll_configuration.dart, widgets/scroll_physics.dart, widgets/scroll_simulation.dart,
// widgets/feedback.dart, widgets/text_selection.dart, cupertino/picker.dart, material/app.dart, material/constants.dart,
// cupertino/constants.dart; Android's touch slop from engine/src/flutter/shell/platform/android/io/flutter/embedding/
// android/FlutterView.java; the browser OS detection of engine/src/flutter/lib/web_ui/lib/ui_web/src/ui_web/
// browser_detection.dart and foundation/_platform_web.dart. Ledger rows: third_party/flutter/PORTED.md.
// Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.

//! Target → platform behaviour: Flutter's `TargetPlatform` defaults as one table. How to read, extend and override
//! it: `docs/behaviour.md`.

#![allow(
    non_upper_case_globals,
    reason = "Flutter's constant names, kept so Flutter changes diff in"
)]

use std::time::Duration;

use gpui::{GestureTuning, Pixels, PointerDevice, ScrollPhysics, px};

use crate::{Error, Target};

// gestures/constants.dart. Flutter uses these on every platform; only the touch slop is a device setting (below).
pub const kLongPressTimeout: Duration = Duration::from_millis(500);
pub const kDoubleTapTimeout: Duration = Duration::from_millis(300);
pub const kDoubleTapMinTime: Duration = Duration::from_millis(40);
pub const kDoubleTapSlop: Pixels = px(100.0);
pub const kTouchSlop: Pixels = px(18.0);
pub const kPagingTouchSlop: Pixels = px(36.0); // kTouchSlop * 2.0
pub const kPanSlop: Pixels = px(36.0); // kTouchSlop * 2.0
pub const kScaleSlop: Pixels = kTouchSlop;
pub const kDoubleTapTouchSlop: Pixels = kTouchSlop;
pub const kMinFlingVelocity: f32 = 50.0;
pub const kMaxFlingVelocity: f32 = 8000.0;
pub const kPrecisePointerHitSlop: Pixels = px(1.0);
pub const kPrecisePointerPanSlop: Pixels = px(2.0); // kPrecisePointerHitSlop * 2.0
pub const kPrecisePointerScaleSlop: Pixels = kPrecisePointerHitSlop;
// material/constants.dart and cupertino/constants.dart.
pub const kMinInteractiveDimension: Pixels = px(48.0);
pub const kMinInteractiveDimensionCupertino: Pixels = px(44.0);

/// How far a pointer travels before it stops being a tap (`hit`), starts a pan (`pan`) or a scale (`scale`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slops {
    pub hit: Pixels,
    pub pan: Pixels,
    pub scale: Pixels,
}

/// `computeHitSlop`, `computePanSlop`, `computeScaleSlop` for a mouse: the same on every platform.
pub const PRECISE_POINTER_SLOPS: Slops = Slops {
    hit: kPrecisePointerHitSlop,
    pan: kPrecisePointerPanSlop,
    scale: kPrecisePointerScaleSlop,
};

/// Touch slops with no device setting (`DeviceGestureSettings.touchSlop` null): iOS, web and desktop embedders.
const FLUTTER_TOUCH_SLOPS: Slops = Slops {
    hit: kTouchSlop,
    pan: kPanSlop,
    scale: kScaleSlop,
};

/// Android's embedding sends `ViewConfiguration.getScaledTouchSlop()` (AOSP `config_viewConfigurationTouchSlop`,
/// 8 dp); `DeviceGestureSettings.panSlop` doubles it. `computeScaleSlop` ignores the setting.
const ANDROID_TOUCH_SLOPS: Slops = Slops {
    hit: px(8.0),
    pan: px(16.0),
    scale: kScaleSlop,
};

/// The physics classes `ScrollBehavior.getScrollPhysics` picks from: `BouncingScrollPhysics` (iOS),
/// `BouncingScrollPhysics(decelerationRate: fast)` (macOS), `ClampingScrollPhysics` (the rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPreset {
    Bouncing,
    BouncingDesktop,
    Clamping,
}

/// What a physics class sets, and GPUI's model of its fling. One named const per preset below.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollPresetRow {
    pub preset: ScrollPreset,
    /// GPUI's model of the ballistic simulation (`BouncingScrollSimulation` / `ClampingScrollSimulation`).
    pub physics: ScrollPhysics,
    pub min_fling_velocity: f32,
    pub max_fling_velocity: f32,
    /// `dragStartDistanceMotionThreshold`: `None` where the physics leaves it unset.
    pub drag_start_distance_motion_threshold: Option<Pixels>,
}

// To add a preset: a `ScrollPreset` variant and a const here, values from its Flutter class.

/// `BouncingScrollPhysics`. Its `FrictionSimulation(0.135)` is 0.135^(1/1000) per ms: UIScrollView's 0.998.
pub const BOUNCING: ScrollPresetRow = ScrollPresetRow {
    preset: ScrollPreset::Bouncing,
    physics: ScrollPhysics::Exponential {
        decay_per_ms: 0.998,
    },
    min_fling_velocity: kMinFlingVelocity * 2.0,
    max_fling_velocity: kMaxFlingVelocity,
    drag_start_distance_motion_threshold: Some(px(3.5)),
};

/// `BouncingScrollPhysics(decelerationRate: ScrollDecelerationRate.fast)`. Flutter also adds a constant
/// deceleration of 1400 there, which GPUI's exponential model has no term for.
pub const BOUNCING_DESKTOP: ScrollPresetRow = ScrollPresetRow {
    preset: ScrollPreset::BouncingDesktop,
    max_fling_velocity: kMaxFlingVelocity * 8.0,
    ..BOUNCING
};

/// `ClampingScrollPhysics`. `ClampingScrollSimulation` is Android's OverScroller spline: friction 0.015 at 160
/// logical pixels per inch, GPUI's `ScrollPhysics::android()`.
pub const CLAMPING: ScrollPresetRow = ScrollPresetRow {
    preset: ScrollPreset::Clamping,
    physics: ScrollPhysics::FrictionSpline {
        friction: 0.015,
        pixels_per_inch: 160.0,
    },
    min_fling_velocity: kMinFlingVelocity,
    max_fling_velocity: kMaxFlingVelocity,
    drag_start_distance_motion_threshold: None,
};

/// What a scrollable shows past its edge: bounce (the physics overscrolls), Material 3's stretch
/// (`StretchingOverscrollIndicator`, `useMaterial3` defaults to true), Material 2's glow, or nothing (clamped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overscroll {
    Bounce,
    Stretch,
    Glow,
    None,
}

/// `ScrollBehavior.velocityTrackerBuilder`: which velocity estimate a fling starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VelocityTracker {
    /// `IOSScrollViewFlingVelocityTracker`.
    IosScrollView,
    /// `MacOSScrollViewFlingVelocityTracker`.
    MacOsScrollView,
    /// `VelocityTracker.withKind`: least squares.
    LeastSquares,
}

/// A `HapticFeedback` call (services/haptic_feedback.dart).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Haptic {
    Vibrate,
    HeavyImpact,
    SelectionClick,
}

/// The haptic Flutter's widgets play per event, `None` for none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Haptics {
    /// `Feedback.forTap` (Android plays a click sound, no haptic).
    pub tap: Option<Haptic>,
    /// `Feedback.forLongPress`.
    pub long_press: Option<Haptic>,
    /// A text selection handle moves a character (`SelectionOverlay.selectionEndpoints`).
    pub selection_handle_drag: Option<Haptic>,
    /// A picker's selected item changes (`CupertinoPicker._handleHapticFeedback`).
    pub picker_item: Option<Haptic>,
}

/// One row of [`TABLE`]: everything Flutter decides from a `TargetPlatform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behaviour {
    pub target: Target,
    /// Slops for touch and pen (`PointerDeviceKind.touch` / `.stylus`); a mouse uses [`PRECISE_POINTER_SLOPS`].
    pub touch_slops: Slops,
    /// `ScrollBehavior.getScrollPhysics`.
    pub scroll: ScrollPresetRow,
    pub velocity_tracker: VelocityTracker,
    pub overscroll: Overscroll,
    pub haptics: Haptics,
    /// Smallest tap target a control should have: Material's 48 or Cupertino's 44 (the set `.adaptive` widgets
    /// pick per platform).
    pub min_touch_target: Pixels,
    /// Taps become mouse presses, pans scroll events (GPUI's recognizer; elements opt out with `on_touch`).
    pub touch_to_mouse: bool,
}

const NO_HAPTICS: Haptics = Haptics {
    tap: None,
    long_press: None,
    selection_handle_drag: None,
    picker_item: None,
};

/// Target → behaviour, one row per Flutter `TargetPlatform` (the web has none: it takes its host OS's row, as Flutter
/// web does). To add a target: a row here with each value from the Flutter function its field names.
pub const TABLE: [Behaviour; 5] = [
    Behaviour {
        target: Target::Android,
        touch_slops: ANDROID_TOUCH_SLOPS,
        scroll: CLAMPING,
        velocity_tracker: VelocityTracker::LeastSquares,
        overscroll: Overscroll::Stretch,
        haptics: Haptics {
            long_press: Some(Haptic::Vibrate),
            selection_handle_drag: Some(Haptic::SelectionClick),
            ..NO_HAPTICS
        },
        min_touch_target: kMinInteractiveDimension,
        touch_to_mouse: true,
    },
    Behaviour {
        target: Target::Ios,
        touch_slops: FLUTTER_TOUCH_SLOPS,
        scroll: BOUNCING,
        velocity_tracker: VelocityTracker::IosScrollView,
        overscroll: Overscroll::Bounce,
        haptics: Haptics {
            long_press: Some(Haptic::HeavyImpact),
            picker_item: Some(Haptic::SelectionClick),
            ..NO_HAPTICS
        },
        min_touch_target: kMinInteractiveDimensionCupertino,
        touch_to_mouse: true,
    },
    Behaviour {
        target: Target::MacOs,
        touch_slops: FLUTTER_TOUCH_SLOPS,
        scroll: BOUNCING_DESKTOP,
        velocity_tracker: VelocityTracker::MacOsScrollView,
        overscroll: Overscroll::Bounce,
        haptics: NO_HAPTICS,
        min_touch_target: kMinInteractiveDimensionCupertino,
        touch_to_mouse: true,
    },
    Behaviour {
        target: Target::Windows,
        touch_slops: FLUTTER_TOUCH_SLOPS,
        scroll: CLAMPING,
        velocity_tracker: VelocityTracker::LeastSquares,
        overscroll: Overscroll::None,
        haptics: NO_HAPTICS,
        min_touch_target: kMinInteractiveDimension,
        touch_to_mouse: true,
    },
    Behaviour {
        target: Target::Linux,
        touch_slops: FLUTTER_TOUCH_SLOPS,
        scroll: CLAMPING,
        velocity_tracker: VelocityTracker::LeastSquares,
        overscroll: Overscroll::None,
        haptics: NO_HAPTICS,
        min_touch_target: kMinInteractiveDimension,
        touch_to_mouse: true,
    },
];

impl Behaviour {
    /// `computeHitSlop` / `computePanSlop` / `computeScaleSlop`: precise for a mouse, the row's for touch and pen.
    pub fn slops(&self, device: PointerDevice) -> Slops {
        match device {
            PointerDevice::Mouse => PRECISE_POINTER_SLOPS,
            PointerDevice::Touch | PointerDevice::Pen => self.touch_slops,
        }
    }

    /// The values GPUI's touch recognizer takes. Pass it, or a changed copy, to `App::set_gesture_tuning`
    /// (patch 0014) before opening a window.
    pub fn gesture_tuning(&self) -> GestureTuning {
        GestureTuning {
            touch_slop: self.touch_slops.hit,
            multi_tap_interval: kDoubleTapTimeout,
            multi_tap_slop: kDoubleTapSlop,
            long_press_duration: kLongPressTimeout,
            scroll_physics: self.scroll.physics,
            min_fling_velocity: self.scroll.min_fling_velocity,
        }
    }
}

/// `target`'s row; the web, which has none, and an unlisted target are [`Error::UnsupportedTarget`].
pub fn behaviour(target: Option<Target>) -> Result<&'static Behaviour, Error> {
    TABLE
        .iter()
        .find(|row| Some(row.target) == target)
        .ok_or(Error::UnsupportedTarget)
}

/// The current target's row. On the web: the browser's host OS's row ([`detect_operating_system`]).
pub fn current_behaviour() -> Result<&'static Behaviour, Error> {
    behaviour(cfg_select! {
        target_family = "wasm" => Some(browser_target()),
        _ => crate::CURRENT_TARGET,
    })
}

/// Reads `navigator`, as Flutter web's `BrowserDetection.operatingSystem` does. No window (a worker): Flutter's
/// unknown OS.
#[cfg(target_family = "wasm")]
fn browser_target() -> Target {
    let navigator = web_sys::window().map(|window| window.navigator());
    let platform = navigator.as_ref().and_then(|nav| nav.platform().ok());
    let user_agent = navigator.as_ref().and_then(|nav| nav.user_agent().ok());
    let max_touch_points = navigator.as_ref().map_or(0, |nav| nav.max_touch_points());
    detect_operating_system(
        platform.as_deref().unwrap_or_default(),
        user_agent.as_deref().unwrap_or_default(),
        max_touch_points,
    )
}

/// `BrowserDetection.detectOperatingSystem` then `_operatingSystemToTargetPlatform` (foundation/_platform_web.dart),
/// which resolves an unknown OS to Android. Rules run in order; the first match wins.
pub fn detect_operating_system(platform: &str, user_agent: &str, max_touch_points: i32) -> Target {
    let lower = platform.to_lowercase();
    // (rule, target). To extend: add a row where Flutter adds a branch, in the same position.
    let rules: [(bool, Target); 6] = [
        // iDevices requesting a "desktop site" spoof a Mac; more than 2 touch points means a touch device.
        (
            platform.starts_with("Mac") && max_touch_points > 2,
            Target::Ios,
        ),
        (platform.starts_with("Mac"), Target::MacOs),
        (
            ["iphone", "ipad", "ipod"]
                .iter()
                .any(|device| lower.contains(device)),
            Target::Ios,
        ),
        // Android reports itself as "Linux armv8l" in navigator.platform, so the user agent decides.
        (user_agent.contains("Android"), Target::Android),
        (platform.starts_with("Linux"), Target::Linux),
        (platform.starts_with("Win"), Target::Windows),
    ];
    rules
        .iter()
        .find(|(matches, _)| *matches)
        .map_or(Target::Android, |(_, target)| *target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behaviour_table() {
        struct Expected {
            touch_slop: f32,
            pan_slop: f32,
            min_fling: f32,
            max_fling: f32,
            preset: ScrollPreset,
            overscroll: Overscroll,
            long_press: Option<Haptic>,
            min_touch_target: f32,
        }
        // (row name with its Flutter sources, target, expected values)
        let rows = [
            (
                "android: ViewConfiguration touch slop (FlutterView.java), getScrollPhysics clamping, \
                 MaterialScrollBehavior stretch, Feedback.forLongPress vibrate, kMinInteractiveDimension",
                Target::Android,
                Expected {
                    touch_slop: 8.,
                    pan_slop: 16.,
                    min_fling: 50.,
                    max_fling: 8000.,
                    preset: ScrollPreset::Clamping,
                    overscroll: Overscroll::Stretch,
                    long_press: Some(Haptic::Vibrate),
                    min_touch_target: 48.,
                },
            ),
            (
                "ios: kTouchSlop, kPanSlop, BouncingScrollPhysics minFlingVelocity kMinFlingVelocity * 2, \
                 Feedback.forLongPress heavyImpact, kMinInteractiveDimensionCupertino",
                Target::Ios,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 100.,
                    max_fling: 8000.,
                    preset: ScrollPreset::Bouncing,
                    overscroll: Overscroll::Bounce,
                    long_press: Some(Haptic::HeavyImpact),
                    min_touch_target: 44.,
                },
            ),
            (
                "macos: kTouchSlop, BouncingScrollPhysics(fast) maxFlingVelocity kMaxFlingVelocity * 8, \
                 no Feedback, kMinInteractiveDimensionCupertino",
                Target::MacOs,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 100.,
                    max_fling: 64000.,
                    preset: ScrollPreset::BouncingDesktop,
                    overscroll: Overscroll::Bounce,
                    long_press: None,
                    min_touch_target: 44.,
                },
            ),
            (
                "windows: kTouchSlop, ClampingScrollPhysics, no overscroll indicator, kMinInteractiveDimension",
                Target::Windows,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 50.,
                    max_fling: 8000.,
                    preset: ScrollPreset::Clamping,
                    overscroll: Overscroll::None,
                    long_press: None,
                    min_touch_target: 48.,
                },
            ),
            (
                "linux: kTouchSlop, ClampingScrollPhysics, no overscroll indicator, kMinInteractiveDimension",
                Target::Linux,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 50.,
                    max_fling: 8000.,
                    preset: ScrollPreset::Clamping,
                    overscroll: Overscroll::None,
                    long_press: None,
                    min_touch_target: 48.,
                },
            ),
        ];
        for (name, target, expected) in rows {
            let found = behaviour(Some(target));
            assert!(found.is_ok(), "{name}: has a row");
            let Ok(row) = found else { continue };
            let tuning = row.gesture_tuning();
            assert_eq!(tuning.touch_slop, px(expected.touch_slop), "{name}");
            assert_eq!(
                row.slops(PointerDevice::Pen).pan,
                px(expected.pan_slop),
                "{name}"
            );
            assert_eq!(
                row.slops(PointerDevice::Mouse),
                PRECISE_POINTER_SLOPS,
                "{name}"
            );
            assert_eq!(tuning.min_fling_velocity, expected.min_fling, "{name}");
            assert_eq!(row.scroll.max_fling_velocity, expected.max_fling, "{name}");
            assert_eq!(
                tuning.multi_tap_interval,
                Duration::from_millis(300),
                "{name}"
            );
            assert_eq!(
                tuning.long_press_duration,
                Duration::from_millis(500),
                "{name}"
            );
            assert_eq!(row.scroll.preset, expected.preset, "{name}");
            assert_eq!(row.overscroll, expected.overscroll, "{name}");
            assert_eq!(row.haptics.long_press, expected.long_press, "{name}");
            assert_eq!(
                row.min_touch_target,
                px(expected.min_touch_target),
                "{name}"
            );
            assert!(row.touch_to_mouse, "{name}");
        }
        // Errors are not `PartialEq` on Android (they carry OS errors).
        assert!(
            matches!(behaviour(Some(Target::Web)), Err(Error::UnsupportedTarget)),
            "web: no row of its own"
        );
    }

    #[test]
    fn detect_operating_system_table() {
        // (row name, navigator.platform, navigator.userAgent, navigator.maxTouchPoints, target)
        let rows = [
            (
                "mac",
                "MacIntel",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
                0,
                Target::MacOs,
            ),
            (
                "ipad requesting a desktop site",
                "MacIntel",
                "Mozilla/5.0 (Macintosh)",
                5,
                Target::Ios,
            ),
            (
                "iphone",
                "iPhone",
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)",
                5,
                Target::Ios,
            ),
            (
                "ipad",
                "iPad",
                "Mozilla/5.0 (iPad; CPU OS 17_0 like Mac OS X)",
                5,
                Target::Ios,
            ),
            (
                "android reports linux",
                "Linux armv8l",
                "Mozilla/5.0 (Linux; Android 14; Pixel 8)",
                5,
                Target::Android,
            ),
            (
                "linux",
                "Linux x86_64",
                "Mozilla/5.0 (X11; Linux x86_64)",
                0,
                Target::Linux,
            ),
            (
                "windows",
                "Win32",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
                0,
                Target::Windows,
            ),
            (
                "unknown resolves to android",
                "FreeBSD amd64",
                "Mozilla/5.0 (X11; FreeBSD amd64)",
                0,
                Target::Android,
            ),
        ];
        for (name, platform, user_agent, max_touch_points, expected) in rows {
            assert_eq!(
                detect_operating_system(platform, user_agent, max_touch_points),
                expected,
                "{name}"
            );
        }
    }
}
