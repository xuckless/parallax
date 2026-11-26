# Android lifecycle

How Activity callbacks reach GPUI's existing lifecycle hooks in [`parallax-android`](../crates/android/src/lib.rs), and
how the renderer survives GPU device loss. The backend around it: [android.md](android.md). Rules: [AGENTS.md](../AGENTS.md)
(Explicit state, Tests). Why upstream already has these hooks: [gap audit](audits/gap-audit.md) (Lifecycle API).

## The path

1. **Java.** [`ParallaxActivity`](../crates/android/java/io/github/xuckless/parallax/ParallaxActivity.java) forwards
   `onStart`, `onResume`, `onPause`, `onStop`, `onDestroy`, `onWindowFocusChanged` and `onTrimMemory`
   as one native, `ParallaxSurfaceView.nativeActivityEvent(code, value)` (a row in `natives` in
   [host.rs](../crates/android/src/host.rs)). `value` is the focus flag or the trim level.
2. **State machine.** [lifecycle.rs](../crates/android/src/lifecycle.rs): `Lifecycle::{Created, Started, Resumed}`
   (each with the last focus flag) and `Disconnected`; one `next(state, event) → (state, effect)`. Pure, tested row by
   row on the host (`cargo test -p parallax-android`), rejected events included.
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
| `onDestroy` | `Disconnected` | `Disconnected` | (already hidden) |
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
- Back is the keys port's (`on_should_close`); `quit` (`Activity.finish()`) is still a stub ([android.md](android.md#stubs-not-yet-on-android)).

To add a callback: an `EVENT_*` constant in `ParallaxActivity`, a row in `ActivityEvent::from_java`, the variant, and
the arms the compiler then asks for in `Lifecycle::next`, each with a test row.

## Device loss

Each vsync, `Shell::frame` asks the renderer whether its device is lost; the surface state machine
([surface.rs](../crates/android/src/surface.rs)) turns that into `RecoverDevice`, and
[`renderer::recover`](../crates/android/src/renderer.rs) rebuilds it:

1. `WgpuRenderer::destroy` drops the surface and the renderer's hold on the lost device.
2. `seed` replaces the lost shared `GpuContext` with one from the first backend in `BACKENDS` that can drive the
   surface: the same table, in the same order, as at startup.
3. Upstream's `WgpuRenderer::recover` finds a live context, so it adopts ours instead of picking a backend itself, and
   refills the same atlas GPUI holds.

On failure the renderer still reports its device lost and the next frame tries again. No patch is needed: stock
`gpui-pre` exposes `destroy`, `recover` and `WgpuContext::device_lost` (AGENTS.md, patch rule 11).
