# Android lifecycle

How Activity callbacks reach GPUI's existing lifecycle hooks in [`parallax-android`](../crates/android/src/lib.rs). The
surface and GPU device loss: [android-surface.md](android-surface.md). The backend around it: [android.md](android.md). Rules: [AGENTS.md](../AGENTS.md)
(Explicit state, Tests). Why upstream already has these hooks: [gap audit](audits/gap-audit.md) (Lifecycle API).

## The path

1. **Java.** [`ParallaxActivity`](../crates/android/java/io/github/xuckless/parallax/ParallaxActivity.java) forwards
   `onStart`, `onResume`, `onPause`, `onStop`, `onDestroy`, `onWindowFocusChanged` and `onTrimMemory`
   as one native, `ParallaxSurfaceView.nativeActivityEvent(activity, code, value)` (a row in `natives` in
   [host.rs](../crates/android/src/host.rs)). `value` is the focus flag or the trim level; `activity` is the id
   `nativeActivityCreated` returned in `onCreate`, which the view's surface, frame and inset natives carry too.
2. **State machine.** [lifecycle.rs](../crates/android/src/lifecycle.rs): `Lifecycle::{Loaded, Live, Disconnected}`;
   `Live` holds every live Activity, in creation order, each with its `Stage::{Created, Started, Resumed}` and last
   focus flag. One `next(state, event) → (state, effect)`; the phases are the app's level change (its most
   foreground Activity's), read from the `UP` and `DOWN` tables. Pure, tested row by row on the host (`cargo test -p parallax-android`), rejected events
   included.
3. **Shell.** `Shell::lifecycle` stores the new state, tells that Activity's window (`on_active_status_change`,
   `on_visibility_change`) when its `is_active(id)` or `visibility(id)` changed, adds a window on `onCreate` and closes
   it on `onDestroy` ([android-multi-window.md](android-multi-window.md)), then tells the app (`Platform::on_app_lifecycle` or
   `on_memory_warning`). Apps observe them with upstream's `cx.on_app_lifecycle` and `cx.on_memory_warning`.

## The mapping

| Activity | State after | Phase GPUI sees | Window |
|---|---|---|---|
| `onStart` | `Started` | `Foreground` | visible |
| `onResume` | `Resumed` | `Active` if focused, else nothing yet | active if focused |
| focus gained / lost while resumed | `Resumed` | `Active` / `Inactive` | active / inactive |
| `onPause` | `Started` | `Inactive` if it was focused | inactive |
| `onStop` | `Created` | `Background` | hidden |
| `onDestroy` (last live) | `Disconnected` | `Disconnected` | (already hidden) |
| `onTrimMemory(level ≥ RUNNING_LOW)` | unchanged | memory warning | |

- **Active needs focus**, as in Flutter's `LifecycleChannel`: the notification shade or a system dialog takes focus
  without `onPause`, and that is upstream's `Inactive` ("visible but not receiving input"). Upstream's own table
  maps `Active` to `onResume` alone; on a phone the two differ only while something covers the app.
- **Trim levels** warn from `TRIM_MEMORY_RUNNING_LOW` (10) up, Flutter 3.50.1's rule exactly, so `UI_HIDDEN`
  (20, every Home press) warns too, and `onLowMemory` is not forwarded, as in Flutter.
- **Launch.** GPUI starts on the first surface, after `onStart` and usually `onResume`; right after launch the shell
  replays `launch_phases()` (`Foreground`, then `Active` if resumed and focused), as iOS reports them after its scene
  connects.
- **`onDestroy` closes the Activity's window; the last one is `Disconnected`, not quit.** Android may create a new
  Activity in the same process (back on the root, "Don't keep activities"), and GPUI's quit path (`App::shutdown`)
  ends the app for good; so GPUI keeps running, and the new Activity's launch opens a new window. `on_quit` never
  fires: Android kills processes without notice, so save state on `Background`.
- Back is the keys port's (`on_should_close`); `quit` (`Activity.finish()`) is still a stub ([android.md](android-stubs.md)).

## Several Activities

Each live Activity has its own GPUI window ([android-multi-window.md](android-multi-window.md)): a deep link stacks a
second `ParallaxActivity` (the template's `launchMode="singleTop"`, as in Flutter's, stops only a repeat on top), a
clear-task relaunch overlaps the old and the new, split screen shows two.

- **Windows follow their own Activity.** A window is active while its Activity is resumed and focused, visible from
  `onStart` to `onStop`; a covered Activity's window is hidden, the covering one's active.
- **The app follows the most foreground Activity.** Its phase moves only when the highest level among the live
  Activities moves: focus passing between two split-screen halves is `Inactive` then `Active`; the covering Activity
  stopping while the covered one is resumed changes nothing.
- **Memory warnings** come from the oldest live Activity's `onTrimMemory` only: Android sends every Activity the same
  call.
- **Late callbacks** from a destroyed Activity are rejected; ids are never reused.
- **`Lifecycle::contains`** is the one rule for view events: surface, frame, insets, touch and keys reach the window
  of a live Activity (the ported `AndroidTouchProcessor` and `KeyEmbedderResponder` carry the id in place of
  Flutter's renderer and messenger).

To add a callback: an `EVENT_*` constant in `ParallaxActivity`, a row in `Callback::from_java`, the variant, and
the arms the compiler then asks for in `Lifecycle::next`, each with a test row.
