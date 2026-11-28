# Gesture recognizer tests and traces

Flutter's recognizer tests (tap, double tap, long press, drag) are the oracle for GPUI's touch recognizer
(`TouchGestureRecognizer` in `gestures.rs`). Rules: [AGENTS.md](../AGENTS.md#flutter-is-the-second-source-of-truth)
(rule 6, Decisions "Gesture oracle tests" and "Gesture parity"). They ship in
[patch 0013](../patches/0013-gesture-recognizer-flutter-oracle.patch), with the recognizer fixes they forced.

## What 0013 changes

| Change | Flutter source | Before |
|---|---|---|
| `gestures/constants.rs` (`gpui::constants`): all of `constants.dart`, Flutter's names, the one copy | `constants.dart` | `MAX_FLING_VELOCITY` only |
| `GestureTuning::default()` = Flutter's constants: `touch_slop` 18 px, `multi_tap_interval` 300 ms, `multi_tap_slop` 100 px, `multi_tap_min_interval` 40 ms (new), `long_press_duration` 500 ms; fling limits of its `ScrollPhysics::ios()` (`min_fling_velocity` 100) | `constants.dart` (kTouchSlop, kDoubleTapTimeout, kDoubleTapSlop, kDoubleTapMinTime, kLongPressTimeout), `BouncingScrollPhysics` | 8 px, 400 ms, 16 px, none, 50 |
| `gpui::slops(kind, touch_slop)`: hit, pan and scale slops per pointer kind, the one copy | `events.dart` (computeHitSlop, computePanSlop, computeScaleSlop) | none |
| A tap's click count continues when it starts < interval after the last tap's up, within the slop of where that tap started, ≥ min interval after that tap started, and on the same target (innermost hitbox) | `multitap.dart`, `DoubleTapGestureRecognizer.addAllowedPointer` | up-to-up time and distance, window-wide |
| A release flings only if fast enough and the velocity estimate's offset along the axis exceeds the physics' `min_fling_distance` (kTouchSlop) | `monodrag.dart`, `isFlingGesture` | speed only |
| The recognizer reads `cx.background_executor().now()`, not `Instant::now()` | none | wall clock: tests couldn't drive it |

The values are the same on every platform in Flutter. The [platform behaviour table](behaviour.md) feeds per-target
values through `App::set_gesture_tuning` (patch 0014); the field names above are what it fills. GPUI keeps counting
past 2 (triple click), and a second tap too far from the first starts a new sequence (Flutter's double-tap
recognizer ignores it). Flutter counts per widget (a recognizer sees only its widget's pointers); GPUI's one
recognizer per window counts per target, the bounds of the innermost hitbox the window finds under the touch.

## The test

`gestures::recognizer_tests::flutter_recognizer_tests_pass` in `gestures/recognizer_tests.rs`: one row per Flutter
test, named after it. `replay` opens a test window whose element records what the window dispatches, sets the row's
tuning (`Window::set_gesture_tuning`), feeds the trace, and runs the test clock on so timers (long press) fire at
their time. Run it with `scripts/patch-test.sh recognizer` ([patch-tests.md](patch-tests.md)).

| Row field | Meaning |
|---|---|
| `tuning` | `GestureTuning`, Flutter's unless the Flutter test changed one (e.g. a 100 ms long press) |
| `claim` | `Nothing`, `LongPress` or `TouchDrag`: what the element claims, standing in for the recognizer the Flutter test builds (taps and pans are always on) |
| `targets` | hitboxes the element inserts (tap targets); empty for the Flutter rows |
| `trace` | the recorded touch events, below |
| `until_ms` | the clock runs on to here after the trace |
| `expected` | `(ms, Seen)`: `Tap(click_count)`, `Scroll(phase, dx, dy)`, `LongPress(phase, x, y)`, `TouchDrag(phase, x, y)` |

Flutter's arena maps to claims: a drag recognizer alone in its arena wins at the down event (a claimed `TouchDrag`);
one competing with a tap waits for the slop (a pan, `Scroll`). `Compete(kind)` and `LongPressAndCompete(kind)` rows
(patch 0044, `tap_and_drag_test.dart`) compete for the drag instead: [touch-drag.md](touch-drag.md).

## Trace format

A trace is a slice of `TraceEvent`, one per platform touch event, in order:

```rust
touch(at_ms, id, phase, x, y) // ms since the trace started, touch id, Started/Moved/Ended/Cancelled, logical px
```

Fixture files may add a sixth field, the device (`Touch` or `Pen`; `Touch` when absent), which
[trace_replay.rs](../crates/platform/tests/trace_replay.rs) reads ([traces-desktop.md](traces-desktop.md)).

Times are absolute from the trace start, so events at the same time are the same frame. Positions are window
logical pixels. A recorder emits one `touch(…)` line per `TouchEvent` it sees, rebased to its first event's time; the
replay engine needs nothing else. Pressure and predicted positions are not in the format yet.

The one player is `gpui::play_trace(cx, window, trace, frame_ms, until_ms, on_step)` (patch 0013, `test-support`):
`(at_ms, PlatformInput)` pairs (`TraceEvent::to_timed_input`, or any scroll event) dispatched on the test clock, with
a frame every `frame_ms` when it is `Some` (fling momentum, overscroll), and `on_step` called with
`TraceStep::Input(&input)` after each dispatch and `TraceStep::Frame(ms)` after each frame. The recognizer, pinch
and overscroll tests and [trace_replay.rs](../crates/platform/tests/trace_replay.rs) all use it.

## Ported and dropped

43 Flutter tests, 44 rows (12 tap, 10 double tap, 11 long press, 11 drag), plus two per-target rows (`targets`:
the hitboxes the element inserts). Before 0013's fixes, 5 failed. The fling rule has its own table,
`gestures::tests::fling_needs_flutters_velocity_and_distance`. Dropped
(82), listed per file in the [ledger](../third_party/flutter/PORTED.md):

- **Arena:** yield to / held arena, arena reject, passive gestures, two recognizers on one pointer, microtask order.
- **Buttons and devices:** secondary/tertiary buttons, button changes, device-kind filters, mouse drags.
- **Multi-pointer:** the touch recognizer ignores extra touches while one is active; on an `on_pinch` element they
  pinch ([pinch.md](pinch.md)).
- **Velocity and fling:** max-velocity clamps, synthesized events, quick flicks: the velocity and physics tables.
- **No GPUI twin:** tap-down and tap-move callbacks, details `toString`, exception logging, pointer pan/zoom.
