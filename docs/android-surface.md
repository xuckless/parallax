# Android surface

How [`parallax-android`](../crates/android/src/lib.rs) keeps each Activity's renderer on its `ParallaxSurfaceView`
surface across `SurfaceHolder` callbacks and GPU device loss. One state machine per Activity's window
([android-multi-window.md](android-multi-window.md)), [surface.rs](../crates/android/src/surface.rs):
`Surface::{None, Ready, Recovering, Lost}`, one `next(state, event) → (state, action)`, tested row by row on the host.
The Activity side: [android-lifecycle.md](android-lifecycle.md). Rules: [AGENTS.md](../AGENTS.md) (Explicit state).

## Whose surface

Each Activity's view feeds only its own window's state machine (Java tags every call with the Activity's id), so no
state records a view. `surfaceDestroyed` always releases the window (`Detach`): Android needs the `ANativeWindow`
released before that callback returns, and nothing may draw into it after. A `surfaceCreated` before the old
surface's `surfaceDestroyed` releases the old window at once (`Attach { previous }`).

## Resize

`Ready` and `Recovering` carry the window's size as Android last reported it. A `surfaceChanged` at that size (Android
sends one right after every `surfaceCreated`) is just `Draw`: the surface attached on `Created` already fits, so a
start or resume builds one surface, not two.

A `surfaceChanged` at a new size (rotation, split screen) is `Attach { previous: None }`, like a new window: the shell
unconfigures the renderer's surface, then `replace_surface` makes a new one on the same window at the new size. Not
`configure` on the old one: wgpu's GL backend keeps its EGL window surface, whose next buffer was dequeued at the old
size, and API 28 (no BLAST) drops a buffer of the wrong size, so the old frame stayed until the next input. Flutter does
the same ([`OnScreenSurfaceResize`](https://github.com/flutter/flutter/blob/master/engine/src/flutter/shell/platform/android/android_surface_gl_skia.cc)
destroys and recreates the EGL surface). The old surface goes first because Vulkan and EGL connect to an
`ANativeWindow` only once.

If `replace_surface` fails, the shell sends `AttachFailed`: the renderer has no surface, so the state moves to
`Recovering` and the rebuild below retries after the backoff, instead of drawing black until the next surface event.

## Device loss

Each vsync, `Shell::frame` sends the surface state machine ([surface.rs](../crates/android/src/surface.rs)) a `Frame`
saying whether the renderer's device is lost. A loss moves `Ready` to `Recovering { since, failures }`; once
`RECOVERY_DELAYS[failures]` has passed (upstream's 350 ms first, doubling per failure up to 5.6 s, without blocking the
Looper as upstream's sleep does), the action is `RecoverDevice`, and [`renderer::recover`](../crates/android/src/renderer.rs)
rebuilds it:

1. `WgpuRenderer::destroy` drops the surface and the renderer's hold on the lost device; `update_drawable_size`
   records the current size (a resize while `Recovering` only updates the state's size).
2. `seed` replaces the lost shared `GpuContext` with one from the first backend in the backend table that can drive the
   surface: the same table, in the same order, as at startup.
3. Upstream's `WgpuRenderer::recover` finds a live context, so it adopts ours instead of picking a backend itself, and
   refills the same atlas GPUI holds.

`Recovered` returns to `Ready` with `Redraw`: upstream's `recover` already made the surface at that size, so no
second one is built; GPUI hears the size and draws a full frame. `RecoveryFailed` restarts the wait. No patch is needed: stock
`gpui-pre` exposes `destroy`, `recover` and `WgpuContext::device_lost`.
