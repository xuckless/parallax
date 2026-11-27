//! Facade crate: re-exports GPUI and the Parallax crates an app needs.

pub use gpui;
pub use parallax_android;
pub use parallax_platform;

/// Sets the current target's row of the platform behaviour table (Flutter's per-platform gesture feel) as GPUI's
/// gesture tuning, for windows opened after it. [`android_main!`] calls it before the app's `launch`; a desktop or
/// web entry point calls it itself. An app may override the tuning afterwards (docs/behaviour.md).
pub fn set_platform_gesture_tuning(cx: &mut gpui::App) {
    match parallax_platform::behaviour::current_behaviour() {
        Ok(behaviour) => cx.set_gesture_tuning(behaviour.gesture_tuning()),
        Err(error) => log::warn!("keeping the platform's gesture tuning: {error}"),
    }
}

/// Android entry point: `parallax_android::android_main!`, calling [`set_platform_gesture_tuning`] before `launch`.
#[macro_export]
macro_rules! android_main {
    ($launch:expr) => {
        $crate::parallax_android::android_main!(|cx: &mut $crate::gpui::App| {
            $crate::set_platform_gesture_tuning(cx);
            ($launch)(cx)
        });
    };
}
