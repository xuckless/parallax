// Ported from flutter/flutter packages/flutter/lib/src/services/haptic_feedback.dart @ 3.50.1
// (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors. BSD-3-Clause, see
// third_party/flutter/LICENSE.

//! GPUI's haptics on Android: each `gpui::Haptic` becomes the `HapticFeedback.vibrate` call Flutter's
//! `HapticFeedback` sends on `flutter/platform`, which Java's ported `PlatformChannel.HapticFeedbackType.fromValue`
//! reads and `PlatformPlugin.vibrateHapticFeedback` plays (docs/haptics.md). Java pulls them with the other framework
//! calls every vsync.

use gpui::Haptic;
use serde_json::{Value, json};

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

/// `haptics` as Flutter's `flutter/platform` calls, in order, in the `{"method", "args"}` JSON the view reads
/// (`args` null for `vibrate()`); `None` when there are none.
pub(crate) fn platform_calls(haptics: &[Haptic]) -> Option<Value> {
    (!haptics.is_empty()).then(|| {
        haptics
            .iter()
            .map(|&haptic| json!({"method": "HapticFeedback.vibrate", "args": flutter_argument(haptic)}))
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_calls_table() {
        // (row name, haptics GPUI played, the calls Java gets)
        let rows = [
            ("none: no calls", vec![], None),
            (
                "in order; vibrate() sends a null argument",
                vec![Haptic::HeavyImpact, Haptic::Vibrate],
                Some(json!([
                    {"method": "HapticFeedback.vibrate", "args": "HapticFeedbackType.heavyImpact"},
                    {"method": "HapticFeedback.vibrate", "args": null},
                ])),
            ),
        ];
        for (name, haptics, calls) in rows {
            assert_eq!(platform_calls(&haptics), calls, "{name}");
        }
    }
}
