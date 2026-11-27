# Desktop touch on Linux

Touchscreens on Linux reach GPUI as `TouchEvent`s, always compiled in ("Desktop touch" under Decisions in
[AGENTS.md](../AGENTS.md)), so the same recognizers, pinch and overscroll run as on phones
([ROADMAP Phase 3](../ROADMAP.md#phase-3-desktop-touch)). Each display server has its own section; both port winit
v0.30.13 (Apache-2.0, credited in [NOTICE](../NOTICE)) and time touches with one shared clock (below).

**Time stamps.** Wayland compositors (libinput: Weston, Mutter, KWin, wlroots), Xorg and Xwayland stamp input with
`CLOCK_MONOTONIC` in ms, wrapping at `u32`. gpui_linux's `linux/clock.rs` `Clock` reads that clock next to
`Instant::now()`; the wrapping difference is the age. A stamp after the read counts as now; an age over 10 s means
another clock base (a remote display) and gives `None`, so GPUI times the touch at dispatch
([touch-time-stamps.md](touch-time-stamps.md)). Its table test, `event_time_stamps`, holds both backends' rows.

**Blocked windows.** A window a modal child blocks drops input, but a touch's end or cancel still reaches it
(`reaches_blocked_window` in `linux.rs`, table test `blocked_window_input`), so no touch stays open in GPUI.

## Wayland

[Patch 0022](../patches/0022-touch-wayland.patch) binds `wl_touch` in `gpui_linux`'s Wayland client
([client.rs](../upstream/gpui-pre-linux/src/linux/wayland/client.rs), which upstream left with pointer and keyboard
only). The logic is `wayland/touch.rs`, ported from winit's
[seat/touch/mod.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/wayland/seat/touch/mod.rs),
with Flutter's [fl_touch_manager.cc](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/linux/fl_touch_manager.cc)
as the second reference. `client.rs` only binds the object and dispatches.

| `wl_touch` event | GPUI |
|---|---|
| `down` on a GPUI window's surface | `Started`, the next `TouchId` (from 0, wrapping; Wayland reuses ids). A down on any other surface is dropped, and so is the rest of that touch |
| `motion` / `up` | `Moved` / `Ended` (at the last motion's position) |
| `cancel` (the compositor took the touches, e.g. for a system gesture) | `Cancelled` for every touch still down |
| `frame`, `shape`, `orientation` | ignored, as in winit: each event is delivered as it comes |

- **Device and position:** always `Touch` ([pointer-source.md](pointer-source.md)); surface-local coordinates,
  already logical pixels on Wayland (winit scales them to physical; GPUI doesn't).
- **Seat:** bound while the seat has the touch capability; a seat that loses it, or is replaced, cancels its
  touches. A down's serial counts as a press, so a tap can start a window move, open a popup or take the clipboard.
- **No dedupe:** Wayland compositors never make `wl_pointer` events from a touchscreen, so a tap fires once.
- **Not yet:** pens through `zwp_tablet_v2` (winit has it only in the v0.31 betas), touch force and contact shape.

## X11

[Patch 0023](../patches/0023-touch-x11.patch) feeds XInput 2 touch into gpui_linux's X11 client, ported from winit's
[event_processor.rs](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/x11/event_processor.rs)
(`xinput2_touch`, the `XI_Touch*` arm, the `XIPointerEmulated` check). Flutter's Linux embedder leaves X11 touch to
GTK ([fl_view.cc @ 3.50.1](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/linux/fl_view.cc)).
The spec is [XI2proto](https://gitlab.freedesktop.org/xorg/proto/xorgproto/-/blob/master/specs/XI2proto.txt).

| Piece | What | Where (`x11/event.rs`) |
|---|---|---|
| Selection | each window selects `XI_TouchBegin` / `Update` / `End` on the master devices; masks come from one table by the server's XInput version (touch 2.2, pinch 2.4), since a mask the server lacks fails the whole selection | `XinputVersion::window_events` |
| Phases | `TouchBegin` → `Started`, `TouchUpdate` → `Moved`, `TouchEnd` → `Ended` | `XI2_TOUCH_PHASES` |
| Touch | id = the event's `detail` (globally unique, increasing), `device: Touch`, logical px as pointer events convert theirs, no force (XI2 has pressure only as a valuator) | `touch_from_xinput` |
| Emulation | `XinputButtonPress` / `Release` / `Motion` flagged `POINTER_EMULATED` are dropped: GPUI makes its own mouse events from the touch, so a tap clicks once. The flag also marks smooth scrolling's emulated wheel clicks, which upstream already skipped | `is_pointer_emulated`, used in `client.rs` |

- **No `Cancelled`.** A client that selects touch without `XI_TouchOwnership` (winit, GDK, this backend) gets a
  touch only once it owns it, and an owned touch ends only with `XI_TouchEnd`. Selecting ownership would deliver
  touches early, then a `TouchEnd` when another client takes them; a row mapping that to `Cancelled` adds it.
- **Pen.** X11 tablets send pointer events with pressure valuators, not touches, and GPUI's mouse events carry no
  device ([pointer-source.md](pointer-source.md)), so a pen reads as `Mouse`, like web pens. Deferred.

## Tests

Table tests, run by [patch-test.sh](../scripts/patch-test.sh) with the rest of `parallax-gpui-linux`'s lib tests on
Linux ([patch-tests.md](patch-tests.md)):

| Test | Where | Rows |
|---|---|---|
| `event_time_stamps` | `clock.rs` | on time, a frame old, after the read, across the `u32` wrap, foreign clocks behind and ahead |
| `touches_follow_wl_touch_events` | `wayland/touch.rs` | event sequences → touches, id reuse, cancel, dropped events |
| `xi2_touch_events` | `x11/event.rs` | XI2 fixtures (type, detail, time, FP16.16 position) → `TouchEvent`; scale 2 → logical px; ownership is no phase; one or more rows per recorded gesture, as Xwayland sent them |
| `xi2_window_events_by_version` | `x11/event.rs` | XInput 2.0 to 3.0: which versions select touch and pinch |
| `pointer_emulation_filter` | `x11/event.rs` | real press kept, emulated press dropped |

Live, without a touchscreen: Weston's headless backend, touched through its test plugin, drives hello on Wayland
and, through Xwayland, on X11 ([traces-desktop.md](traces-desktop.md); CI's `linux-touch` job). Those runs check
that a tap clicks once and that ids, phases and logical px hold at scales 1 and 2. Not injectable there:
`wl_touch.cancel` (`weston_test` sends only down, motion and up; `touches_follow_wl_touch_events` covers it), and
Xwayland rounds touch positions to whole px.
