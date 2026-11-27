# Touch time stamps

[Patch 0020](../patches/0020-touch-event-timestamp.patch) gives every `TouchEvent` the platform's own event time,
`time_stamp: Option<Instant>`, as Flutter's `PointerEvent.timeStamp`
([events.dart](https://github.com/flutter/flutter/blob/3.50.1/packages/flutter/lib/src/gestures/events.dart)).
Velocity estimates read it, so a platform that delivers moves late or in bunches still measures how fast the finger
moved. Ledger rows: [PORTED.md](../third_party/flutter/PORTED.md).

## The clock

`time_stamp` is on GPUI's executor clock (`cx.background_executor().now()`, a `web_time::Instant`: the OS monotonic
clock natively, `performance.now()` in a browser), so it compares with every other GPUI time. Each backend reads its
OS clock and GPUI's together and takes the event's age off the current instant:

| Platform | Event time | Its clock, read at dispatch | Where |
|---|---|---|---|
| Android | `MotionEvent.getEventTime()`, the packet's `time_stamp` (µs) | `CLOCK_MONOTONIC` (`uptimeMillis`) | [pointer.rs](../crates/android/src/pointer.rs) `Clock`, read in [host.rs](../crates/android/src/host.rs) |
| iOS | `UITouch.timestamp` (s since boot) | `NSProcessInfo.systemUptime` | gpui_ios `touch_time_stamp` (0020) |
| Web | `PointerEvent.timeStamp` (ms) | `performance.now()` | gpui_web `event_time_stamp` (0020) |
| Windows | `POINTER_INFO.PerformanceCount`, else `dwTime` (ms) | `QueryPerformanceCounter`, else `GetTickCount` | gpui_windows `pointer::Clock` (0021, [desktop-touch-windows.md](desktop-touch-windows.md)) |
| Linux, Wayland | `wl_touch` event `time` (ms, `u32`) | `CLOCK_MONOTONIC`; over 10 s old is another base: `None` | gpui_linux `wayland/touch.rs` `Clock` (0022, [desktop-touch-linux.md](desktop-touch-linux.md#wayland)) |
| Other desktops, tests | none | | `None` |

A stamp later than the clock read counts as now. `None` (no event time, synthesized touches, tests) is timed when
GPUI dispatches the touch: each recognizer writes that fallback once, `event.time_stamp.unwrap_or(now)`.

## What reads it

- The touch recognizer ([gesture-recognizers.md](gesture-recognizers.md)): its velocity tracker's samples and the
  release estimate ([velocity-tracking.md](velocity-tracking.md)), so fling speed is the finger's.
- The pinch arena's scale recognizer ([pinch.md](pinch.md)): its focal point and scale velocity trackers, as
  Flutter's `ScaleGestureRecognizer` takes `timeStamp`.

Timers stay on the dispatch clock: long press, double-tap timeouts and fling momentum frames run on GPUI's executor,
as Flutter's run on its own `Timer`s.

## Traces

hello's trace pad times each `touch(at_ms, …)` line by `time_stamp` (dispatch time where it is `None`), so recorded
traces carry the platform's own spacing ([traces.md](traces.md)). The replay dispatches each touch at its `at_ms`
with `time_stamp: None`, which makes the dispatch time the stamp.

## Tests

| Test | Checks |
|---|---|
| `velocity_follows_event_time_stamps_not_dispatch` (gpui `gestures.rs`, [patch-test.sh](../scripts/patch-test.sh)) | the same 16 ms samples dispatched on time, at 33 ms frames and 50 ms late fling at one velocity |
| `touch_time_stamps` ([pointer.rs](../crates/android/src/pointer.rs)) | an Android record's stamp becomes an `Instant` by its age; a stamp after the clock read is now |
| `trace_replay` ([trace_replay.rs](../crates/platform/tests/trace_replay.rs)) | web and iOS flings, recorded with stamps, agree under every behaviour row |

## Extending

A backend that gains event times fills `time_stamp` the same way: its OS clock and `Instant::now()` read together,
the event's age taken off the instant. Never mix clocks: a stamp from another timeline must go through its age.
