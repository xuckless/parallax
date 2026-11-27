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

The app's `launch` (`android_main!`) runs once per Activity, on its first surface, and opens one window:

1. The first Activity's surface starts GPUI (`run_embedded(launch)`) and replays the phases it missed.
2. A later Activity's first surface (split screen, a deep link, a relaunch after the last Activity was destroyed in
   the same process) runs `launch` in the running app.

Before running it the window moves `Waiting → Launched`; `Platform::open_window` binds the newest `Launched` window
(`Launched → Open`) and builds its renderer on the Activity's surface. A launch that opens no window leaves the
Activity black (logged); opening a window with no Activity waiting is `Error::NoActivityForWindow`, since Android
opens windows only through Activities. The facade applies the behaviour table before the first launch only
([behaviour.md](behaviour.md)), so a later Activity keeps the app's own tuning. A launch that binds keys binds them
again per Activity; GPUI tolerates the duplicate bindings.

`onDestroy` closes the window: its view state goes with it, as a desktop window's does. Rotation and other handled
configuration changes keep the Activity (manifest `configChanges`), so they keep the window.

## Touch slop

Each view reports its own density and touch slop. A moved slop calls the facade's `TouchSlopChanged` hook with that
Activity's window (`None` before it opens): the app's tuning (for windows opened later) and that window's own are
retuned.

## Check it

On an emulator with API 33+ (split screen through the WM shell; `hello` logs `window open, N in all` and
`Activity N destroyed: its window closes`):

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
