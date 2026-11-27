// Ported from flutter/flutter packages/flutter/lib/src/services/haptic_feedback.dart @ 3.50.1
// (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors. BSD-3-Clause, see
// third_party/flutter/LICENSE.

//! GPUI's haptics on Android: each `gpui::Haptic` becomes the argument Flutter's `HapticFeedback` sends with
//! `HapticFeedback.vibrate`, which Java's ported `PlatformChannel.HapticFeedbackType.fromValue` reads and
//! `PlatformPlugin.vibrateHapticFeedback` plays (docs/haptics.md). Java asks for the pending ones every vsync.

use gpui::Haptic;

/// The `HapticFeedback.vibrate` argument for `haptic`; `None` is `HapticFeedback.vibrate()`'s missing argument
/// (Java's `STANDARD`). The match is the table: exhaustive, so a new kind cannot be missed.
pub(crate) fn flutter_argument(haptic: Haptic) -> Option<&'static str> {
    match haptic {
        Haptic::Vibrate => None,
        Haptic::LightImpact => Some("HapticFeedbackType.lightImpact"),
        Haptic::MediumImpact => Some("HapticFeedbackType.mediumImpact"),
        Haptic::HeavyImpact => Some("HapticFeedbackType.heavyImpact"),
        Haptic::SelectionClick => Some("HapticFeedbackType.selectionClick"),
        Haptic::SuccessNotification => Some("HapticFeedbackType.successNotification"),
        Haptic::WarningNotification => Some("HapticFeedbackType.warningNotification"),
        Haptic::ErrorNotification => Some("HapticFeedbackType.errorNotification"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flutter_argument_table() {
        // (row name: the haptic_feedback.dart call, haptic, argument). Java's PlatformPluginTest checks each
        // argument's Android HapticFeedbackConstants.
        let rows = [
            (
                "vibrate: no argument (STANDARD, LONG_PRESS)",
                Haptic::Vibrate,
                None,
            ),
            (
                "lightImpact (VIRTUAL_KEY)",
                Haptic::LightImpact,
                Some("HapticFeedbackType.lightImpact"),
            ),
            (
                "mediumImpact (KEYBOARD_TAP)",
                Haptic::MediumImpact,
                Some("HapticFeedbackType.mediumImpact"),
            ),
            (
                "heavyImpact (CONTEXT_CLICK)",
                Haptic::HeavyImpact,
                Some("HapticFeedbackType.heavyImpact"),
            ),
            (
                "selectionClick (CLOCK_TICK)",
                Haptic::SelectionClick,
                Some("HapticFeedbackType.selectionClick"),
            ),
            (
                "successNotification (CONFIRM from API 30)",
                Haptic::SuccessNotification,
                Some("HapticFeedbackType.successNotification"),
            ),
            (
                "warningNotification (KEYBOARD_TAP from API 30)",
                Haptic::WarningNotification,
                Some("HapticFeedbackType.warningNotification"),
            ),
            (
                "errorNotification (REJECT from API 30)",
                Haptic::ErrorNotification,
                Some("HapticFeedbackType.errorNotification"),
            ),
        ];
        for (name, haptic, argument) in rows {
            assert_eq!(flutter_argument(haptic), argument, "{name}");
        }
    }
}
