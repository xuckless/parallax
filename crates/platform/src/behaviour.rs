// Ported from flutter/flutter @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7): the per-TargetPlatform
// defaults of (under packages/flutter/lib/src/) widgets/scroll_configuration.dart,
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

use gpui::{
    GestureTuning, Pixels, PointerDevice, ScrollDecelerationRate, ScrollPhysics,
    VelocityTrackerKind, px, slops,
};

use crate::{Error, Target};

// Flutter's gesture constants (gestures/constants.dart), slop rules (`gpui::slops`) and scroll physics
// (`gpui::ScrollPhysics`) are GPUI's, the one copy; this table holds only what Flutter picks per platform.

// material/constants.dart and cupertino/constants.dart.
pub const kMinInteractiveDimension: Pixels = px(48.0);
pub const kMinInteractiveDimensionCupertino: Pixels = px(44.0);

/// What a scrollable shows past its edge: bounce (the physics overscrolls), Material 3's stretch
/// (`StretchingOverscrollIndicator`, `useMaterial3` defaults to true), Material 2's glow, or nothing (clamped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overscroll {
    Bounce,
    Stretch,
    Glow,
    None,
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

/// Where a row's touch slop (the embedder's `DeviceGestureSettings.touchSlop`) comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchSlop {
    /// The embedder sends none: `gpui::slops` falls back to Flutter's `kTouchSlop`.
    Unset,
    /// The platform reports its own at runtime, in logical pixels (Android's embedding sends
    /// `ViewConfiguration.getScaledTouchSlop()` as `physicalTouchSlop`; `DeviceGestureSettings.fromView` divides it by
    /// the density). `fallback` holds until the platform has reported one.
    Device { fallback: Pixels },
}

impl TouchSlop {
    /// The `touchSlop` `gpui::slops` takes, given what the platform reported (`None`: nothing yet).
    pub fn resolve(self, reported: Option<Pixels>) -> Option<Pixels> {
        match self {
            TouchSlop::Unset => None,
            TouchSlop::Device { fallback } => Some(reported.unwrap_or(fallback)),
        }
    }
}

/// One row of [`TABLE`]: everything Flutter decides from a `TargetPlatform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behaviour {
    pub target: Target,
    /// The embedder's `DeviceGestureSettings.touchSlop`, [`TouchSlop::resolve`]d with the platform's reported value.
    /// `gpui::slops` turns it into the hit, pan and scale slops per pointer kind.
    pub touch_slop: TouchSlop,
    /// `ScrollBehavior.getScrollPhysics`.
    pub scroll_physics: ScrollPhysics,
    /// `ScrollBehavior.velocityTrackerBuilder`: which velocity estimate a fling starts from.
    pub velocity_tracker: VelocityTrackerKind,
    pub overscroll: Overscroll,
    pub haptics: Haptics,
    /// Smallest tap target a control should have: Material's 48 or Cupertino's 44 (the set `.adaptive` widgets
    /// pick per platform).
    pub min_touch_target: Pixels,
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
        // Android's embedding sends `ViewConfiguration.getScaledTouchSlop()` (FlutterView.sendViewportMetricsToFlutter);
        // parallax-android reports it. Until it has: AOSP's `config_viewConfigurationTouchSlop`, 8 dp.
        touch_slop: TouchSlop::Device { fallback: px(8.0) },
        scroll_physics: ScrollPhysics::Clamping,
        velocity_tracker: VelocityTrackerKind::LeastSquares,
        overscroll: Overscroll::Stretch,
        haptics: Haptics {
            long_press: Some(Haptic::Vibrate),
            selection_handle_drag: Some(Haptic::SelectionClick),
            ..NO_HAPTICS
        },
        min_touch_target: kMinInteractiveDimension,
    },
    Behaviour {
        target: Target::Ios,
        touch_slop: TouchSlop::Unset,
        scroll_physics: ScrollPhysics::Bouncing {
            deceleration_rate: ScrollDecelerationRate::Normal,
        },
        velocity_tracker: VelocityTrackerKind::IOSScrollViewFling,
        overscroll: Overscroll::Bounce,
        haptics: Haptics {
            long_press: Some(Haptic::HeavyImpact),
            picker_item: Some(Haptic::SelectionClick),
            ..NO_HAPTICS
        },
        min_touch_target: kMinInteractiveDimensionCupertino,
    },
    Behaviour {
        target: Target::MacOs,
        touch_slop: TouchSlop::Unset,
        scroll_physics: ScrollPhysics::Bouncing {
            deceleration_rate: ScrollDecelerationRate::Fast,
        },
        velocity_tracker: VelocityTrackerKind::MacOSScrollViewFling,
        overscroll: Overscroll::Bounce,
        haptics: NO_HAPTICS,
        min_touch_target: kMinInteractiveDimensionCupertino,
    },
    Behaviour {
        target: Target::Windows,
        touch_slop: TouchSlop::Unset,
        scroll_physics: ScrollPhysics::Clamping,
        velocity_tracker: VelocityTrackerKind::LeastSquares,
        overscroll: Overscroll::None,
        haptics: NO_HAPTICS,
        min_touch_target: kMinInteractiveDimension,
    },
    Behaviour {
        target: Target::Linux,
        touch_slop: TouchSlop::Unset,
        scroll_physics: ScrollPhysics::Clamping,
        velocity_tracker: VelocityTrackerKind::LeastSquares,
        overscroll: Overscroll::None,
        haptics: NO_HAPTICS,
        min_touch_target: kMinInteractiveDimension,
    },
];

impl Behaviour {
    /// The values GPUI's touch recognizer takes: the row's touch slop (resolved with `reported_touch_slop`, what the
    /// platform reported, `None` for nothing), physics and tracker; the rest are Flutter's constants, which
    /// `GestureTuning::default()` holds. Pass it, or a changed copy, to `App::set_gesture_tuning` (patch 0014) before
    /// opening a window.
    pub fn gesture_tuning(&self, reported_touch_slop: Option<Pixels>) -> GestureTuning {
        GestureTuning {
            touch_slop: self.hit_slop(reported_touch_slop),
            scroll_physics: self.scroll_physics,
            min_fling_velocity: self.scroll_physics.min_fling_velocity(),
            max_fling_velocity: self.scroll_physics.max_fling_velocity(),
            velocity_tracker: self.velocity_tracker,
            ..GestureTuning::default()
        }
    }

    /// `current` after the platform's reported touch slop moved from `old` to `new` (Android: a density change):
    /// `current` with the new hit slop, or `None` when the hit slop stays or when `current`'s is not the one this row
    /// gave for `old` (the app set its own, which wins).
    pub fn retune(
        &self,
        current: GestureTuning,
        old: Option<Pixels>,
        new: Option<Pixels>,
    ) -> Option<GestureTuning> {
        let (old, new) = (self.hit_slop(old), self.hit_slop(new));
        (current.touch_slop == old && old != new).then_some(GestureTuning {
            touch_slop: new,
            ..current
        })
    }

    fn hit_slop(&self, reported_touch_slop: Option<Pixels>) -> Pixels {
        slops(
            PointerDevice::Touch,
            self.touch_slop.resolve(reported_touch_slop),
        )
        .hit
    }
}

/// `target`'s row; the web, which has none, and an unlisted target are [`Error::UnsupportedTarget`].
pub fn behaviour(target: Option<Target>) -> Result<&'static Behaviour, Error> {
    TABLE
        .iter()
        .find(|row| Some(row.target) == target)
        .ok_or(Error::UnsupportedTarget)
}

/// The web's behaviour on a `host` OS: Flutter web takes the host's row, but its embedder never sets
/// `physicalTouchSlop`, so the slops are Flutter's (`kTouchSlop`, `kPanSlop`) even on Android.
pub fn web_behaviour(host: Target) -> Result<Behaviour, Error> {
    Ok(Behaviour {
        touch_slop: TouchSlop::Unset,
        ..*behaviour(Some(host))?
    })
}

/// The current target's row. On the web: [`web_behaviour`] of the browser's host OS, read from `navigator` as Flutter
/// web's `BrowserDetection.operatingSystem` does (no window, as in a worker: Flutter's unknown OS).
pub fn current_behaviour() -> Result<Behaviour, Error> {
    cfg_select! {
        target_family = "wasm" => {
            let navigator = web_sys::window().map(|window| window.navigator());
            let platform = navigator.as_ref().and_then(|nav| nav.platform().ok());
            let user_agent = navigator.as_ref().and_then(|nav| nav.user_agent().ok());
            let max_touch_points = navigator.as_ref().map_or(0, |nav| nav.max_touch_points());
            web_behaviour(detect_operating_system(
                platform.as_deref().unwrap_or_default(),
                user_agent.as_deref().unwrap_or_default(),
                max_touch_points,
            ))
        }
        _ => behaviour(crate::CURRENT_TARGET).copied(),
    }
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
    use std::time::Duration;

    use super::*;

    const BOUNCING: ScrollPhysics = ScrollPhysics::Bouncing {
        deceleration_rate: ScrollDecelerationRate::Normal,
    };
    const BOUNCING_FAST: ScrollPhysics = ScrollPhysics::Bouncing {
        deceleration_rate: ScrollDecelerationRate::Fast,
    };

    #[test]
    fn behaviour_table() {
        #[derive(Clone, Copy)]
        struct Expected {
            touch_slop: f32,
            pan_slop: f32,
            min_fling: f32,
            max_fling: f32,
            physics: ScrollPhysics,
            overscroll: Overscroll,
            long_press: Option<Haptic>,
            velocity_tracker: VelocityTrackerKind,
            min_touch_target: f32,
        }
        let android = Expected {
            touch_slop: 8.,
            pan_slop: 16.,
            min_fling: 50.,
            max_fling: 8000.,
            physics: ScrollPhysics::Clamping,
            overscroll: Overscroll::Stretch,
            long_press: Some(Haptic::Vibrate),
            velocity_tracker: VelocityTrackerKind::LeastSquares,
            min_touch_target: 48.,
        };
        // (row name with its Flutter sources, behaviour, touch slop the platform reported, expected values)
        let rows = [
            (
                "android before the device reports: config_viewConfigurationTouchSlop's 8 dp, getScrollPhysics \
                 clamping, least squares, MaterialScrollBehavior stretch, Feedback.forLongPress vibrate, \
                 kMinInteractiveDimension",
                behaviour(Some(Target::Android)).copied(),
                None,
                android,
            ),
            (
                "android with the device's slop: getScaledTouchSlop over the density (FlutterView.java, \
                 DeviceGestureSettings.fromView), pan slop twice it",
                behaviour(Some(Target::Android)).copied(),
                Some(px(10.5)),
                Expected {
                    touch_slop: 10.5,
                    pan_slop: 21.,
                    ..android
                },
            ),
            (
                "web on an android host: the android row with Flutter's kTouchSlop and kPanSlop (Flutter web never \
                 sets physicalTouchSlop)",
                web_behaviour(Target::Android),
                None,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    ..android
                },
            ),
            (
                "ios: kTouchSlop, kPanSlop, BouncingScrollPhysics minFlingVelocity kMinFlingVelocity * 2, \
                 Feedback.forLongPress heavyImpact, kMinInteractiveDimensionCupertino",
                behaviour(Some(Target::Ios)).copied(),
                None,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 100.,
                    max_fling: 8000.,
                    physics: BOUNCING,
                    overscroll: Overscroll::Bounce,
                    long_press: Some(Haptic::HeavyImpact),
                    velocity_tracker: VelocityTrackerKind::IOSScrollViewFling,
                    min_touch_target: 44.,
                },
            ),
            (
                "macos: kTouchSlop, BouncingScrollPhysics(fast) maxFlingVelocity kMaxFlingVelocity * 8, \
                 no Feedback, kMinInteractiveDimensionCupertino",
                behaviour(Some(Target::MacOs)).copied(),
                None,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 100.,
                    max_fling: 64000.,
                    physics: BOUNCING_FAST,
                    overscroll: Overscroll::Bounce,
                    long_press: None,
                    velocity_tracker: VelocityTrackerKind::MacOSScrollViewFling,
                    min_touch_target: 44.,
                },
            ),
            (
                "windows: kTouchSlop, ClampingScrollPhysics, no overscroll indicator, kMinInteractiveDimension",
                behaviour(Some(Target::Windows)).copied(),
                None,
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 50.,
                    max_fling: 8000.,
                    physics: ScrollPhysics::Clamping,
                    overscroll: Overscroll::None,
                    long_press: None,
                    velocity_tracker: VelocityTrackerKind::LeastSquares,
                    min_touch_target: 48.,
                },
            ),
            (
                "linux: kTouchSlop whatever is reported (its embedder sends none), ClampingScrollPhysics, no overscroll indicator, kMinInteractiveDimension",
                behaviour(Some(Target::Linux)).copied(),
                Some(px(8.)),
                Expected {
                    touch_slop: 18.,
                    pan_slop: 36.,
                    min_fling: 50.,
                    max_fling: 8000.,
                    physics: ScrollPhysics::Clamping,
                    overscroll: Overscroll::None,
                    long_press: None,
                    velocity_tracker: VelocityTrackerKind::LeastSquares,
                    min_touch_target: 48.,
                },
            ),
        ];
        for (name, found, reported, expected) in rows {
            let row = found.expect("every row's target has a behaviour");
            let tuning = row.gesture_tuning(reported);
            assert_eq!(tuning.touch_slop, px(expected.touch_slop), "{name}");
            assert_eq!(
                slops(PointerDevice::Pen, row.touch_slop.resolve(reported)).pan,
                px(expected.pan_slop),
                "{name}"
            );
            assert_eq!(tuning.min_fling_velocity, expected.min_fling, "{name}");
            assert_eq!(tuning.max_fling_velocity, expected.max_fling, "{name}");
            assert_eq!(
                tuning.multi_tap_interval,
                Duration::from_millis(300),
                "{name}"
            );
            assert_eq!(
                tuning.multi_tap_min_interval,
                Duration::from_millis(40),
                "{name}"
            );
            assert_eq!(
                tuning.long_press_duration,
                Duration::from_millis(500),
                "{name}"
            );
            assert_eq!(tuning.scroll_physics, expected.physics, "{name}");
            assert_eq!(row.overscroll, expected.overscroll, "{name}");
            assert_eq!(row.haptics.long_press, expected.long_press, "{name}");
            assert_eq!(tuning.velocity_tracker, expected.velocity_tracker, "{name}");
            assert_eq!(
                row.min_touch_target,
                px(expected.min_touch_target),
                "{name}"
            );
        }
        // Errors are not `PartialEq` on Android (they carry OS errors).
        assert!(
            matches!(behaviour(Some(Target::Web)), Err(Error::UnsupportedTarget)),
            "web: no row of its own"
        );
    }

    #[test]
    fn retune_table() {
        let android = behaviour(Some(Target::Android)).expect("android has a row");
        let ios = behaviour(Some(Target::Ios)).expect("ios has a row");
        let tuning = |touch_slop: f32| GestureTuning {
            touch_slop: px(touch_slop),
            ..android.gesture_tuning(None)
        };
        // (row name, behaviour, current tuning, reported slop before, after, expected touch slop or no change)
        let rows = [
            (
                "android: the first report replaces the 8 dp fallback",
                android,
                tuning(8.),
                None,
                Some(10.5),
                Some(10.5),
            ),
            (
                "android: a density change moves the device slop",
                android,
                tuning(10.5),
                Some(10.5),
                Some(8.),
                Some(8.),
            ),
            (
                "android: the same logical slop after a density change, no change",
                android,
                tuning(8.),
                Some(8.),
                Some(8.),
                None,
            ),
            (
                "android: a report equal to the fallback, no change",
                android,
                tuning(8.),
                None,
                Some(8.),
                None,
            ),
            (
                "android: the app's own slop wins",
                android,
                tuning(12.),
                Some(8.),
                Some(10.5),
                None,
            ),
            (
                "ios: its embedder sends no slop, a report changes nothing",
                ios,
                ios.gesture_tuning(None),
                None,
                Some(8.),
                None,
            ),
        ];
        for (name, row, current, old, new, expected) in rows {
            let retuned = row.retune(current, old.map(px), new.map(px));
            assert_eq!(
                retuned.map(|tuning| tuning.touch_slop),
                expected.map(px),
                "{name}"
            );
            if let Some(retuned) = retuned {
                assert_eq!(
                    GestureTuning {
                        touch_slop: current.touch_slop,
                        ..retuned
                    },
                    current,
                    "{name}: only the touch slop changes"
                );
            }
        }
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
