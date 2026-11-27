# Desktop touch on Linux

Touchscreens on Linux reach GPUI as `TouchEvent`s, always compiled in ("Desktop touch" under Decisions in
[AGENTS.md](../AGENTS.md)). From there GPUI's recognizer makes taps, pans and pinches as on every other target.

## Wayland

[Patch 0022](../patches/0022-touch-wayland.patch) binds `wl_touch` in `gpui_linux`'s Wayland client
([client.rs](../upstream/gpui-pre-linux/src/linux/wayland/client.rs), which upstream left with pointer and keyboard
only). The logic is `wayland/touch.rs`, ported from winit's
[seat/touch/mod.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/wayland/seat/touch/mod.rs)
@ v0.30.13 (Apache-2.0, credited in [NOTICE](../NOTICE)), with Flutter's
[fl_touch_manager.cc](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/linux/fl_touch_manager.cc)
as the second reference. `client.rs` only binds the object and dispatches.

| `wl_touch` event | GPUI |
|---|---|
| `down` on a GPUI window's surface | `Started`, a fresh `TouchId` (Wayland reuses ids; GPUI never does). A down on any other surface is dropped, and so is the rest of that touch |
| `motion` | `Moved` |
| `up` | `Ended`, at the last motion's position |
| `cancel` (the compositor took the touches, e.g. for a system gesture) | `Cancelled` for every touch still down |
| `frame`, `shape`, `orientation` | ignored, as in winit: each event is delivered as it comes |

- **Device:** always `PointerDevice::Touch` ([pointer-source.md](pointer-source.md)).
- **Position:** the event's surface-local coordinates, which on Wayland are already logical pixels (winit scales
  them to physical; GPUI doesn't).
- **Time stamp:** the event's milliseconds moved onto GPUI's clock by its age against `CLOCK_MONOTONIC`, which
  libinput-based compositors (Weston, Mutter, KWin, wlroots) stamp input with ([touch-time-stamps.md](touch-time-stamps.md)).
  A time more than 10 s old is on another base and the touch is timed at dispatch instead.
- **Seat:** bound once while the seat has the touch capability; a seat that loses it, or is replaced, cancels its
  touches. A down's serial counts as a press, so a tap can start a window move, open a popup or take the clipboard.
- **No dedupe:** Wayland compositors never make `wl_pointer` events from a touchscreen, so a tap fires once.

**Not yet:** pens through `zwp_tablet_v2` (winit has it only in the v0.31 betas, `wp_tablet_input_v2.rs`; a port
once it ships in a release), touch force and contact shape.

## Tests

`touches_follow_wl_touch_events` (event sequences → touches, id reuse, cancel, dropped events) and
`touch_time_stamps` (age, the `u32` wrap, future and foreign-clock stamps) are table tests in `touch.rs`.
[patch-test.sh](../scripts/patch-test.sh) runs them with the rest of `parallax-gpui-linux`'s lib tests on Linux
([patch-tests.md](patch-tests.md)). Live checks need a touchscreen or a compositor that injects touches: run
hello, open its trace pad and every touch logs a `trace: touch(…)` line ([traces.md](traces.md)).
