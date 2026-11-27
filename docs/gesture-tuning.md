# Gesture tuning patches

Two core patches carry GPUI's `GestureTuning` (slops, timeouts, scroll physics, velocity tracker, overscroll,
long-press haptic) from the app to its windows. Parallax fills it from the [behaviour table](behaviour.md); the rest
of the core patches are in [core-patches.md](core-patches.md).

## 0014: the app's gesture tuning

[0014-app-gesture-tuning-override.patch](../patches/0014-app-gesture-tuning-override.patch). `App::set_gesture_tuning`
overrides the platform's `GestureTuning` for windows opened afterwards; `App::gesture_tuning()` is the override,
else the platform's, else GPUI's default. Each window reads it once and passes it to its backend in
`WindowParams::gesture_tuning`, so gpui_web's tap shadow (which decides whether a release may open the keyboard)
uses the same slop as the recognizer. Parallax feeds it from the [behaviour table](behaviour.md).

## 0019: retune an open window

[0019-window-set-gesture-tuning.patch](../patches/0019-window-set-gesture-tuning.patch) (needs 0014).
`Window::set_gesture_tuning` changes an open window's tuning in place: touches in progress keep their state and are
judged by the new values from the next event on. `Window::gesture_tuning()` reads it, so one value can be retuned:
`window.set_gesture_tuning(GestureTuning { touch_slop, ..window.gesture_tuning() })`. The facade does that when
Android's device touch slop changes with the density ([behaviour.md](behaviour.md#how-gpui-gets-it-automatically)).
The backend gets it too (`PlatformWindow::set_gesture_tuning`, a no-op by default): gpui_web's tap shadow follows
the new slop.

## Tests

On GPUI's test platform, run by [`scripts/patch-test.sh`](../scripts/patch-test.sh) ([patch-tests.md](patch-tests.md)):
0014 `app_gesture_tuning_overrides_the_platform_for_new_windows` (window.rs, one row per override: a touch of a given
travel taps or pans), 0019 `open_window_takes_a_new_gesture_tuning` (window.rs, one row per retune).
