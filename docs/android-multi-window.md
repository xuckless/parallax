# Android multi-window

Each `ParallaxActivity` owns one GPUI window (owner decision, Wave 14): two of the app's Activities side by side in
split screen both draw and both take touch and text. All windows run on the one Android main thread, as desktop GPUI's
windows share one thread. The lifecycle behind it: [android-lifecycle.md](android-lifecycle.md); each window's
surface: [android-surface.md](android-surface.md). Rules: [AGENTS.md](../AGENTS.md) (Explicit state).

## The pieces

- **App side** (`Shell` in [host.rs](../crates/android/src/host.rs)): the lifecycle of every live Activity, the
  shared wgpu `GpuContext` (every window's renderer draws with it, as on GPUI's other wgpu platforms) and a map of
  Activity id → `ActivityWindow`. The map changes only in `Shell::lifecycle`: `onCreate` adds a window, `onDestroy`
  removes it and closes its GPUI window (`on_close`, so GPUI drops it on its next turn).
- **Window side** ([activity_window.rs](../crates/android/src/activity_window.rs)): one Activity's surface state
  machine, metrics (size, density, touch slop, insets), pointer and keyboard state, IME session, haptics and GPUI's
  callbacks. Where its GPUI window is, one enum: `Gpui::{Waiting, Launched, Open { handle, renderer }}`; the renderer
  lives only in `Open`.
- **Routing.** Java tags every native call with the id `nativeActivityCreated` gave the Activity. `with_window` in
  host.rs hands surface, frame, inset, touch, key and text-input calls to that Activity's window; a destroyed
  Activity's late calls find none and are dropped (a key goes back to Android unhandled).
- **Lifecycle.** Each window's `is_active` and `visibility` follow its own Activity's stage; the app's phase
  (`on_app_lifecycle`) follows its most foreground Activity, so focus moving between the two halves of a split screen
  is `Inactive`, then `Active`. `Platform::active_window` is the resumed, focused Activity's window.

## Opening a window

The app's `launch` (`android_main!`) runs once per process, as desktop GPUI's launch does: it binds keys, sets
globals and opens the first Activity's window. Each later Activity (split screen, a deep link, a relaunch after the
last Activity was destroyed in the same process) gets its window from the callback the app gave
`parallax_android::on_activity_window` in `launch`, as a desktop app opens a window from a reopen callback:

1. The first Activity's first surface starts GPUI (`run_embedded(launch)`) and replays the phases it missed.
2. A later Activity's first surface runs the `on_activity_window` callback in the running app.

Before either the window moves `Waiting → Requested`; `Platform::open_window` binds the newest `Requested` window
(`Requested → Open`) and builds its renderer on the Activity's surface. A callback that opens no window, or no
callback, leaves the Activity black (logged); opening a window with no Activity waiting is
`Error::NoActivityForWindow`, since Android opens windows only through Activities.

The facade's `parallax::android_main!` applies the behaviour table before `launch` ([behaviour.md](behaviour.md))
and sets `launch` itself as the default callback, so an app written for one window keeps working; it binds its keys
again per Activity, which GPUI tolerates. hello sets its own (`open_window`), so it binds them once.

## Closing a window

`onDestroy` closes the Activity's window: its view state goes with it, as a desktop window's does. Rotation and other
handled configuration changes keep the Activity (manifest `configChanges`), so they keep the window.

When the app closes its own window (`window.remove_window()`), GPUI drops it, which drops the renderer
(`Open → Closed`), and the Activity finishes: the next framework calls carry Flutter's `SystemNavigator.pop`
(`Closed → Finishing`), which the ported `PlatformPlugin.popSystemNavigator` plays as `Activity.finish()`. No empty
Activity stays on screen; the other Activities and their windows keep running.

## Touch slop

Each view reports its own density and touch slop. A moved slop calls the facade's `TouchSlopChanged` hook with that
Activity's window (`None` before it opens): the app's tuning (for windows opened later) and that window's own are
retuned.

## Check it

On an emulator with API 33+ (split screen through the WM shell; `hello` logs `window open, N in all` and
`Activity N destroyed: its window closes`, and `lifecycle …` lines once per phase, not once per window):

1. `cargo parallax run android --device <serial>` in `examples/hello`, then `adb shell input keyevent KEYCODE_HOME`.
2. A second instance in its own task (the template's `singleTop` only reuses the shown one):
   `adb shell am start -n io.github.xuckless.parallax.hello/io.github.xuckless.parallax.ParallaxActivity -f 0x18000000`
   (`NEW_TASK | MULTIPLE_TASK`).
3. `adb shell am stack list` for the two task ids, then split them:
   `adb shell dumpsys activity service SystemUIService WMShell splitscreen moveToSideStage <first task> 1`.
4. Tap each half's box and field, type into each; rotate (`settings put system user_rotation 1`); press Back in one
   half: the other goes full screen and keeps its state.

Checked on API 35 (Gboard 14.2, GL): both halves draw, take taps and text, rotate with their state, and a destroyed
half's window closes while the other keeps drawing.
