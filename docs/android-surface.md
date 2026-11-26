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

## Device loss

Each vsync, `Shell::frame` sends the surface state machine ([surface.rs](../crates/android/src/surface.rs)) a `Frame`
saying whether the renderer's device is lost. A loss moves `Ready` to `Recovering { since, failures }`; once
`RECOVERY_DELAYS[failures]` has passed (upstream's 350 ms first, doubling per failure up to 5.6 s, without blocking the
Looper as upstream's sleep does), the action is `RecoverDevice`, and [`renderer::recover`](../crates/android/src/renderer.rs)
rebuilds it:

1. `WgpuRenderer::destroy` drops the surface and the renderer's hold on the lost device.
2. `seed` replaces the lost shared `GpuContext` with one from the first backend in `BACKENDS` that can drive the
   surface: the same table, in the same order, as at startup.
3. Upstream's `WgpuRenderer::recover` finds a live context, so it adopts ours instead of picking a backend itself, and
   refills the same atlas GPUI holds.

`Recovered` returns to `Ready` and resizes to the current surface; `RecoveryFailed` restarts the wait. No patch is needed: stock
`gpui-pre` exposes `destroy`, `recover` and `WgpuContext::device_lost`.
