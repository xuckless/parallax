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
   `Live` holds the owner and the other live Activities, each with its `Stage::{Created, Started, Resumed}` and last
   focus flag. One `next(state, event) → (state, effect)`; the phases are the owner's level change, read from the
   `UP` and `DOWN` tables. Pure, tested row by row on the host (`cargo test -p parallax-android`), rejected events
   included.
3. **Shell.** `Shell::lifecycle` stores the new state, tells the window (`on_active_status_change`,
   `on_visibility_change`) when `is_active()` or `visibility()` changed, then the app (`Platform::on_app_lifecycle` or
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
- **`onDestroy` is `Disconnected`, not quit.** Android may create a new Activity in the same process (back on the
  root, "Don't keep activities"), and GPUI's quit path (`App::shutdown`) closes every window for good; so GPUI keeps
  running, the surface was already released in `surfaceDestroyed`, and the next `onStart` reattaches. `on_quit` never
  fires: Android kills processes without notice, so save state on `Background`.
- Back is the keys port's (`on_should_close`); `quit` (`Activity.finish()`) is still a stub ([android.md](android-stubs.md)).

## Several Activities

GPUI has one window; the Activity that most recently ran `onStart` owns it. Others are live too: a deep link stacks a
second `ParallaxActivity` (the template's `launchMode="singleTop"`, as in Flutter's, stops only a repeat on top), a
clear-task relaunch overlaps the old and the new, multi-window shows two.

- **Ownership moves on `onStart`.** Back from a stacked Activity restarts the one below, which takes the window back
  before the top one stops. A new Activity owns nothing until it starts.
- **The owner destroyed** while others live: the most recently started of them takes over, and GPUI hears the step
  between the two (e.g. `Foreground`, `Active` when the fallback is resumed in multi-window).
- **Everyone else's callbacks** update only their own stage, quietly; ids are never reused, so a destroyed Activity's
  late callbacks are rejected.
- **`Lifecycle::accepts`** is the one rule for view events: surface, frame, insets, touch and keys count only from the
  owner's view (the ported `AndroidTouchProcessor` and `KeyEmbedderResponder` carry the id in place of Flutter's
  renderer and messenger). In multi-window the other one stays still.

To add a callback: an `EVENT_*` constant in `ParallaxActivity`, a row in `Callback::from_java`, the variant, and
the arms the compiler then asks for in `Lifecycle::next`, each with a test row.
