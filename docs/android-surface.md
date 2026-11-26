# Android surface

How [`parallax-android`](../crates/android/src/lib.rs) keeps the renderer on the `ParallaxSurfaceView` surface across
`SurfaceHolder` callbacks and GPU device loss. One state machine, [surface.rs](../crates/android/src/surface.rs):
`Surface::{None, Ready, Recovering, Lost}`, one `next(state, event) → (state, action)`, tested row by row on the host.
The Activity side: [android-lifecycle.md](android-lifecycle.md). Rules: [AGENTS.md](../AGENTS.md) (Explicit state).

## Whose surface

`Ready` and `Recovering` record the Activity whose view created the window. Only the owning Activity's view may
create or resize it ([android-lifecycle.md](android-lifecycle.md#several-activities)), but `surfaceDestroyed` is
always taken from the window's own view, owner or not: Android needs the `ANativeWindow` released before that
callback returns, and nothing may draw into it after. Both orders of two views are rows in `surface::tests`:

- **New `surfaceCreated` first:** `Attach` releases the old window at once; the old view's late `surfaceDestroyed`
  is rejected, since the current window is not its own.
- **Old `surfaceDestroyed` first:** `Detach` releases it and the state is `Lost` until the new view's surface.

## Resize

`surfaceChanged` (rotation, split screen) is `Attach { previous: None }`, like a new window: the shell unconfigures the
renderer's surface, then `replace_surface` makes a new one on the same window at the new size. Not `configure` on
the old one: wgpu's GL backend keeps its EGL window surface, whose next buffer was dequeued at the old size, and API 28
(no BLAST) drops a buffer of the wrong size, so the old frame stayed until the next input. Flutter does the same
([`OnScreenSurfaceResize`](https://github.com/flutter/flutter/blob/master/engine/src/flutter/shell/platform/android/android_surface_gl_skia.cc)
destroys and recreates the EGL surface). The old surface goes first because Vulkan and EGL connect to an
`ANativeWindow` only once. `Recovered` attaches the same way, at the current size.

## Device loss

Each vsync, `Shell::frame` sends the surface state machine ([surface.rs](../crates/android/src/surface.rs)) a `Frame`
saying whether the renderer's device is lost. A loss moves `Ready` to `Recovering { since, failures }`; once
`RECOVERY_DELAYS[failures]` has passed (upstream's 350 ms first, doubling per failure up to 5.6 s, without blocking the
Looper as upstream's sleep does), the action is `RecoverDevice`, and [`renderer::recover`](../crates/android/src/renderer.rs)
rebuilds it:

1. `WgpuRenderer::destroy` drops the surface and the renderer's hold on the lost device.
2. `seed` replaces the lost shared `GpuContext` with one from the first backend in the backend table that can drive the
   surface: the same table, in the same order, as at startup.
3. Upstream's `WgpuRenderer::recover` finds a live context, so it adopts ours instead of picking a backend itself, and
   refills the same atlas GPUI holds.

`Recovered` returns to `Ready` and attaches a surface at the current size; `RecoveryFailed` restarts the wait. No patch is needed: stock
`gpui-pre` exposes `destroy`, `recover` and `WgpuContext::device_lost`.
