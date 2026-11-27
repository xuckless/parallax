# Desktop touch on Linux

Linux touchscreens reach GPUI as `TouchEvent`s, so the same recognizers, pinch and overscroll run as on phones
([ROADMAP Phase 3](../ROADMAP.md#phase-3-desktop-touch)). Each display server has its own section.

## X11

[Patch 0023](../patches/0023-touch-x11.patch) feeds XInput 2 touch into gpui_linux's X11 client, ported from
[winit v0.30.13](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/x11/event_processor.rs)
(`xinput2_touch`, the `XI_Touch*` arm, the `XIPointerEmulated` check). Flutter's Linux embedder leaves X11 touch to
GTK ([fl_view.cc @ 3.50.1](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/linux/fl_view.cc)),
which agrees: no cancel on X11. The spec is [XI2proto](https://gitlab.freedesktop.org/xorg/proto/xorgproto/-/blob/master/specs/XI2proto.txt).

| Piece | What | Where (`x11/event.rs` in gpui_linux) |
|---|---|---|
| Selection | each window selects `XI_TouchBegin` / `Update` / `End` on the master devices; masks come from one table by the server's XInput version (touch 2.2, pinch 2.4), since a mask the server lacks fails the whole selection | `XinputVersion::window_events` |
| Phases | `TouchBegin` → `Started`, `TouchUpdate` → `Moved`, `TouchEnd` → `Ended` | `XI2_TOUCH_PHASES` |
| Touch | id = the event's `detail` (XI2 ids are globally unique, increasing), `device: Touch`, position in logical px as pointer events convert theirs, no force (XI2 has pressure only as a valuator) | `touch_from_xinput` |
| Time stamp | the server's ms `time` moved onto GPUI's clock by its age ([touch-time-stamps.md](touch-time-stamps.md)) | `X11Clock` |
| Emulation | `XinputButtonPress` / `Release` / `Motion` flagged `POINTER_EMULATED` are dropped: GPUI makes its own mouse events from the touch, so a tap clicks once. The same flag marks smooth scrolling's emulated wheel clicks, which upstream already skipped | `is_pointer_emulated`, used in `client.rs` |

**No `Cancelled`.** A client that selects touch without `XI_TouchOwnership` (winit, GDK, this backend) gets a touch
only once it owns it, and an owned touch ends only with `XI_TouchEnd`. Selecting ownership would deliver touches
early, then a `TouchEnd` when another client takes them; a row mapping that to `Cancelled` is how to add it.

**Clock.** Xorg and Xwayland stamp events with `CLOCK_MONOTONIC` in ms, wrapping at `u32`. `X11Clock::read` reads
the local monotonic clock the same way next to `Instant::now()`; the wrapping difference is the age. A stamp after
the read counts as now. An age over 10 s either way means the server runs on another clock (a remote display), and
the stamp is `None`: GPUI times the touch at dispatch.

**Pen.** X11 tablets send pointer events with pressure valuators, not touches, and GPUI's mouse events carry no
device ([pointer-source.md](pointer-source.md)), so a pen reads as `Mouse`, like web pens. Deferred.

### Tests

Table tests in `x11/event.rs`, run by [patch-test.sh](../scripts/patch-test.sh) (parallax-gpui-linux's lib tests,
[patch-tests.md](patch-tests.md)):

| Test | Rows |
|---|---|
| `xi2_touch_events` | XI2 touch event fixtures (type, detail, time, FP16.16 position) → `TouchEvent`; scale 2 → logical px; ownership is not a phase |
| `x11_time_stamps` | on time, a frame old, after the read, across the `u32` wrap, foreign clocks behind and ahead |
| `xi2_window_events_by_version` | XInput 2.0 to 3.0: which versions select touch and pinch |
| `pointer_emulation_filter` | real press kept, emulated press dropped |

No touchscreen yet: recorded XI2 events (from `xinput test-xi2` or an injected `uinput` device) go into
`xi2_touch_events` as rows, the fixture format above.
