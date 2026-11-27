# Desktop touch in the behaviour table

What a touch (or pen) gets on the macOS, Windows and Linux rows of the [behaviour table](behaviour.md)
([behaviour.rs](../crates/platform/src/behaviour.rs)): Flutter 3.50.1's rules for a touch on a desktop
`TargetPlatform`. A pointer's kind decides, never the OS ([AGENTS.md](../AGENTS.md), Decisions "Desktop touch"), so a
desktop touch is judged like a phone's touch with the desktop row's physics.

| What | Flutter's rule | Desktop rows |
|---|---|---|
| Slops | `computeHitSlop` / `computePanSlop` / `computeScaleSlop` (gestures/events.dart): touch, stylus, trackpad and unknown get `settings?.touchSlop ?? kTouchSlop`, a mouse `kPrecisePointer*` | the desktop embedders send no touch slop (`FlutterWindowMetricsEvent` in [embedder.h](https://github.com/flutter/flutter/blob/ae9b586f34a8443573f82ef57c3bf914a908cbf7/engine/src/flutter/shell/platform/embedder/embedder.h) has none), so `TouchSlop::Unset`: touch and pen 18 / 36 / 18, mouse 1 / 2 / 1 |
| Which pointers drag a scrollable | `ScrollBehavior.dragDevices` = `_kTouchLikeDeviceTypes` (touch, stylus, inverted stylus, trackpad, unknown) on every platform (widgets/scroll_configuration.dart); `MaterialScrollBehavior` keeps it | GPUI already matches: its touch recognizer pans every touch and pen into scrolls, a mouse drag never scrolls, a trackpad sends scroll events. No row field: it is the same on every row |
| Physics, tracker, overscroll | `getScrollPhysics`, `velocityTrackerBuilder`, `buildOverscrollIndicator` per platform, whatever the pointer | the row's: macOS bouncing (fast), Windows and Linux clamping with no indicator |
| Touch target | desktop Material shrink-wraps tap targets (`materialTapTargetSize` `shrinkWrap`, `VisualDensity.compact`; material/theme_data.dart) for every pointer | the touch size stays: Material's 48 on Windows and Linux; macOS takes Apple's 36 pt touch target (macOS 27 touch guidance), since Flutter has no macOS touch value (its 44 is Cupertino's, for iOS) |

GPUI's touch recognizer reads one slop, `GestureTuning::touch_slop`, for every `TouchEvent` (touch or pen): the
row's touch hit slop. Mouse input never reaches it. So no GPUI patch is needed for desktop touch feel; the
backend patches (0021 Windows, 0022 Wayland, 0023 X11, 0025–0026 macOS) only have to send `TouchEvent`s with the right
`device` ([pointer-source.md](pointer-source.md)).

## Not ported

- `ScrollBehavior.getMultitouchDragStrategy`: `averageBoundaryPointers` on macOS and iOS, `latestPointer` elsewhere
  (how two fingers dragging one scrollable move it). GPUI has no such choice yet; a row field waits for one.
- `buildScrollbar` (desktop rows wrap scrollables in a `RawScrollbar`): a widget, not feel.

## Tests

- `desktop_pointer_slops_table` (behaviour.rs): one row per pointer (touch, pen, mouse), each on another desktop, the
  slops it gets and the recognizer's slop. `behaviour_table`'s macOS row holds the 36 pt touch target.
- [trace_replay.rs](../crates/platform/tests/trace_replay.rs) replays every phone and browser trace under the
  desktop rows too: a touch taps, long-presses, pans, flings and pinches there as on a phone
  ([traces.md](traces.md)). Gate 3 adds traces recorded on each desktop.
