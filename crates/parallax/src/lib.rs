//! Facade crate: re-exports GPUI and the Parallax crates an app needs, and starts the app with the platform
//! behaviour table applied (docs/behaviour.md).

pub use gpui;
pub use parallax_android;
pub use parallax_platform;

use parallax_platform::behaviour::{TuningSource, current_behaviour};

/// Who set the app's gesture tuning last: a GPUI global, so it lives in the app's own state. Unset until the
/// platform row or the app sets the tuning.
struct TuningOwner(TuningSource);

impl gpui::Global for TuningOwner {}

/// Sets the current target's row of the platform behaviour table (Flutter's per-platform gesture feel) as GPUI's
/// gesture tuning, for windows opened after it, with the touch slop the device reports (Android) resolved in, and
/// records the platform as its owner. [`run`] and [`android_main!`] call it before the app's `launch`; an app
/// overrides the tuning afterwards with [`set_gesture_tuning`] (docs/behaviour.md).
pub fn set_platform_gesture_tuning(cx: &mut gpui::App) -> Result<(), parallax_platform::Error> {
    let behaviour = current_behaviour()?;
    let reported_touch_slop = cfg_select! {
        target_os = "android" => parallax_android::touch_slop(),
        _ => None,
    };
    cx.set_gesture_tuning(behaviour.gesture_tuning(reported_touch_slop));
    cx.set_global(TuningOwner(TuningSource::Platform));
    Ok(())
}

/// The app's own gesture tuning: `cx.set_gesture_tuning` (windows opened afterwards use it), recording the app as its
/// owner, so a device touch slop change (Android, a density change) never overrides it. Use it rather than
/// `cx.set_gesture_tuning`, which the platform may retune (docs/behaviour.md).
pub fn set_gesture_tuning(cx: &mut gpui::App, tuning: gpui::GestureTuning) {
    cx.set_gesture_tuning(tuning);
    cx.set_global(TuningOwner(TuningSource::App));
}

/// [`set_platform_gesture_tuning`], logging why when the platform keeps its own tuning (no row for this target).
/// What [`run`] and [`android_main!`] call; public only for the macro.
#[doc(hidden)]
pub fn apply_platform_behaviour(cx: &mut gpui::App) {
    if let Err(error) = set_platform_gesture_tuning(cx) {
        log::warn!("keeping the platform's gesture tuning: {error}");
    }
}

/// An Activity's view reported a new device touch slop, `touch_slop`, while the app runs (Android, a density change):
/// sets the behaviour row's new slop on the app's tuning (windows opened later) and on that Activity's `window`'s
/// own tuning (`None` before it opens), while the platform owns the tuning (`Behaviour::retune`). What
/// [`android_main!`] hands parallax-android; public only for the macro.
#[doc(hidden)]
pub fn device_touch_slop_changed(
    cx: &mut gpui::App,
    window: Option<gpui::AnyWindowHandle>,
    touch_slop: gpui::Pixels,
) {
    let reported = Some(touch_slop);
    let behaviour = match current_behaviour() {
        Ok(behaviour) => behaviour,
        Err(error) => return log::warn!("keeping the platform's gesture tuning: {error}"),
    };
    // Unset: the row was never applied (it failed, logged), so no tuning is the platform's to change.
    let Some(&TuningOwner(source)) = cx.try_global::<TuningOwner>() else {
        return;
    };
    if let Some(tuning) = behaviour.retune(source, cx.gesture_tuning(), reported) {
        log::info!(
            "parallax: gesture tuning's touch slop now {:?}",
            tuning.touch_slop
        );
        cx.set_gesture_tuning(tuning);
    }
    if let Some(window) = window {
        let retuned = window.update(cx, |_, window, _| {
            if let Some(tuning) = behaviour.retune(source, window.gesture_tuning(), reported) {
                window.set_gesture_tuning(tuning);
            }
        });
        if let Err(error) = retuned {
            log::warn!("cannot retune a closing window: {error}");
        }
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

/// What [`android_main!`] runs once per process: the behaviour row, `launch` as the default way to open a later
/// Activity's window (`launch` may replace it), then `launch`. Public only for the macro.
#[doc(hidden)]
#[cfg(target_os = "android")]
pub fn android_launch(cx: &mut gpui::App, launch: fn(&mut gpui::App)) {
    apply_platform_behaviour(cx);
    if let Err(error) = parallax_android::on_activity_window(launch) {
        log::error!("parallax: later Activities get no window: {error}");
    }
    launch(cx)
}

/// Android entry point: `parallax_android::android_main!`, applying the platform behaviour row before `launch` (once
/// per process: it opens the first Activity's window) and again, per window, when the device's touch slop changes
/// (the facade's job: `parallax_android`'s own macro can't read the table, which `parallax_platform` builds on it).
///
/// Each later Activity's window comes from `parallax_android::on_activity_window`. The default is `launch` itself,
/// so an app written for one window keeps working, but binds its keys again per Activity; an app that calls
/// `on_activity_window` in `launch` replaces it (docs/android-multi-window.md).
#[macro_export]
macro_rules! android_main {
    ($launch:expr) => {
        $crate::parallax_android::android_main!(
            |cx: &mut $crate::gpui::App| $crate::android_launch(cx, $launch),
            $crate::device_touch_slop_changed
        );
    };
}
