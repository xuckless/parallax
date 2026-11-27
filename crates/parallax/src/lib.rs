//! Facade crate: re-exports GPUI and the Parallax crates an app needs, and starts the app with the platform
//! behaviour table applied (docs/behaviour.md).

pub use gpui;
pub use parallax_android;
pub use parallax_platform;

/// Sets the current target's row of the platform behaviour table (Flutter's per-platform gesture feel) as GPUI's
/// gesture tuning, for windows opened after it, with the touch slop the device reports (Android) resolved in. [`run`]
/// and [`android_main!`] call it before the app's `launch`; an app overrides the tuning afterwards with
/// `cx.set_gesture_tuning` (docs/behaviour.md).
pub fn set_platform_gesture_tuning(cx: &mut gpui::App) -> Result<(), parallax_platform::Error> {
    let behaviour = parallax_platform::behaviour::current_behaviour()?;
    let reported_touch_slop = cfg_select! {
        target_os = "android" => parallax_android::touch_slop(),
        _ => None,
    };
    cx.set_gesture_tuning(behaviour.gesture_tuning(reported_touch_slop));
    Ok(())
}

/// [`set_platform_gesture_tuning`], logging why when the platform keeps its own tuning (no row for this target).
/// What [`run`] and [`android_main!`] call; public only for the macro.
#[doc(hidden)]
pub fn apply_platform_behaviour(cx: &mut gpui::App) {
    if let Err(error) = set_platform_gesture_tuning(cx) {
        log::warn!("keeping the platform's gesture tuning: {error}");
    }
}

/// The device's touch slop moved from `old` to `new` while the app runs (Android, a density change): sets the
/// behaviour row's new slop on the app's tuning and its open windows, unless the app set its own
/// (`Behaviour::retune`). What [`android_main!`] hands parallax-android; public only for the macro.
#[doc(hidden)]
pub fn device_touch_slop_changed(
    cx: &mut gpui::App,
    old: Option<gpui::Pixels>,
    new: Option<gpui::Pixels>,
) {
    let retuned = parallax_platform::behaviour::current_behaviour()
        .map(|behaviour| behaviour.retune(cx.gesture_tuning(), old, new));
    match retuned {
        Ok(Some(tuning)) => {
            log::info!(
                "parallax: gesture tuning's touch slop now {:?}",
                tuning.touch_slop
            );
            cx.set_gesture_tuning(tuning);
            for window in cx.windows() {
                if let Err(error) =
                    window.update(cx, |_, window, _| window.set_gesture_tuning(tuning))
                {
                    log::warn!("cannot retune a closing window: {error}");
                }
            }
        }
        Ok(None) => {}
        Err(error) => log::warn!("keeping the platform's gesture tuning: {error}"),
    }
}

/// Desktop, iOS and web entry point: runs `launch` on the current target's GPUI platform, after applying its
/// platform behaviour row. Returns when the app quits (on the web, once the browser runs it). Android starts from
/// [`android_main!`] instead.
pub fn run(launch: impl FnOnce(&mut gpui::App) + 'static) -> Result<(), parallax_platform::Error> {
    let platform = parallax_platform::current_platform(false)?;
    gpui::Application::with_platform(platform).run(move |cx| {
        apply_platform_behaviour(cx);
        launch(cx)
    });
    Ok(())
}

/// Android entry point: `parallax_android::android_main!`, applying the platform behaviour row before `launch` and
/// again when the device's touch slop changes (the facade's job: `parallax_android`'s own macro can't read the table,
/// which `parallax_platform` builds on it).
#[macro_export]
macro_rules! android_main {
    ($launch:expr) => {
        $crate::parallax_android::android_main!(
            |cx: &mut $crate::gpui::App| {
                $crate::apply_platform_behaviour(cx);
                ($launch)(cx)
            },
            $crate::device_touch_slop_changed
        );
    };
}
