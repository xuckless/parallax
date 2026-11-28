//! Android's "Remove animations" setting as GPUI's reduce motion. Java's `AccessibilityInfoModule` observes the two
//! animation scales and sends their raw `Settings.Global` strings (`nativeAccessibilitySettings`); this decides.
//! Android has no reduce-transparency setting. How it fits together: docs/os-settings.md.
//!
//! Ported from facebook/react-native
//! packages/react-native/ReactAndroid/src/main/java/com/facebook/react/modules/accessibilityinfo/AccessibilityInfoModule.kt
//! (`isReduceMotionEnabledValue`) @ v0.88.1 (c30b7f86850d47422637a64c70508e7411dbd8ea). Copyright (c) Meta
//! Platforms, Inc. and affiliates. MIT, see LICENSE-REACT-NATIVE.

use gpui::AccessibilityDisplaySettings;

/// `transition_animation_scale`: `Settings.Global.TRANSITION_ANIMATION_SCALE`, which React Native and Flutter
/// read; `animator_duration_scale`: `ANIMATOR_DURATION_SCALE`, the one Android's own `ValueAnimator` obeys
/// (`ValueAnimator.areAnimatorsEnabled`). "Remove animations" sets both, and the three developer-options scales set
/// one each, so either at zero reduces motion. `None`: the setting was never written (a fresh device), animations on.
pub(crate) fn settings(
    transition_animation_scale: Option<&str>,
    animator_duration_scale: Option<&str>,
) -> AccessibilityDisplaySettings {
    let disabled = |raw: Option<&str>| {
        // Disabling animations in developer settings will set the animation scale to "0.0"
        // but setting "reduce motion" / "disable animations" will set the animation scale to "0".
        //
        // In some locales, the decimal separator is a comma instead of a dot,
        // but "toFloat" would crash failing to conver these.
        //
        // If the value is not a valid number, we assume reduced motion is not enabled.
        // (`trim`: Kotlin's `toFloat` is Java's `parseFloat`, which ignores surrounding whitespace.)
        raw.is_some_and(|raw| raw.trim().replace(',', ".").parse::<f32>() == Ok(0.0))
    };
    AccessibilityDisplaySettings {
        reduce_motion: disabled(transition_animation_scale) || disabled(animator_duration_scale),
        reduce_transparency: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_motion_from_animation_scales() {
        // (name, TRANSITION_ANIMATION_SCALE, ANIMATOR_DURATION_SCALE, reduce motion). React Native's (RN) cases come
        // from its comments in `isReduceMotionEnabledValue`. Extend with a row per value a device was seen to write.
        let rows = [
            ("never written", None, None, false),
            ("animations on", Some("1"), Some("1"), false),
            ("RN: Remove animations, 0", Some("0"), Some("0"), true),
            ("RN: developer options, 0.0", Some("0.0"), None, true),
            ("RN: comma decimals", Some("0,0"), None, true),
            ("RN: not a number", Some("off"), None, false),
            ("slower animations", Some("1.5"), Some("10"), false),
            ("animator scale off only", Some("1"), Some("0.0"), true),
            ("transition scale off only", Some("0"), Some("1"), true),
            ("surrounding whitespace", Some(" 0 "), None, true),
        ];
        for (name, transition, animator, reduce_motion) in rows {
            assert_eq!(
                settings(transition, animator),
                AccessibilityDisplaySettings {
                    reduce_motion,
                    reduce_transparency: false
                },
                "{name}"
            );
        }
    }
}
