# Desktop touch: Windows

[Patch 0021](../patches/0021-touch-windows.patch) feeds Windows touch and pen into GPUI's `TouchEvent`, so a
touchscreen laptop gets GPUI's taps, pans, flings, long presses and pinches like a phone. It is always compiled in;
what a pointer does depends on its type at runtime ("Desktop touch" under Decisions in [AGENTS.md](../AGENTS.md)).
It ports [winit v0.30.13](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/windows/event_loop.rs)'s
`WM_POINTER*` handling into gpui_windows, with Flutter 3.50.1's Windows embedder
([flutter_window.cc](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/windows/flutter_window.cc))
as second reference ([PORTED.md](../third_party/flutter/PORTED.md)).

## What GPUI gets

| Windows input | GPUI input |
|---|---|
| Touch (`PT_TOUCH`) down, move, up | `TouchEvent`, `device: Touch`, phases Started, Moved, Ended |
| Pen (`PT_PEN`) in contact | `TouchEvent`, `device: Pen`, `force` from its pressure |
| Pen hovering in range | `MouseMove` (Flutter: a hover), through the usual `WM_MOUSEMOVE` handler |
| `POINTER_FLAG_CANCELED`, or the pointer's capture lost (`WM_POINTERCAPTURECHANGED`) | `TouchEvent` Cancelled, where it last was |
| Mouse messages Windows makes from touch or pen (`MI_WP_SIGNATURE`) | nothing: the touch already came, so a tap fires once |
| Mouse, touchpad | unchanged: `WM_MOUSE*` and Direct Manipulation (`EnableMouseInPointer` stays off) |
| Touch on a title bar (non-client) | unchanged: Windows' mouse copies still move the window |

Taps become clicks through GPUI's recognizer (`ClickEvent::Touch`, [core-patches.md](core-patches.md)), and
`Window::pointer_source()` names the device ([pointer-source.md](pointer-source.md)).

- **Ids:** each contact gets a new `TouchId`; Windows reuses pointer ids, GPUI never does.
- **Samples:** each message reads its own pointer's coalesced history (`GetPointerInfoHistory`), oldest first, as
  Flutter's embedder reads only the message's pointer, so a slow frame still delivers every sample. Not winit's whole
  frame plus `SkipPointerFrameMessages`: on CI's runner that stopped a pinch's frames after the first.
- **Positions:** sub-pixel, the HIMETRIC location mapped through `GetPointerDeviceRects`; `ptPixelLocation` when the
  digitizer has no rect.
- **Time stamps:** `PerformanceCount` (else `dwTime`) moved onto GPUI's clock by its age against
  `QueryPerformanceCounter` (else `GetTickCount`), as [touch-time-stamps.md](touch-time-stamps.md) describes.
- **Ignored:** a move, up or cancel for a pointer that went down elsewhere (title bar, another window), and a second
  down for one already down.
- **Not carried:** pen barrel and eraser buttons, tilt, rotation, the inverted (eraser) end; `TouchEvent` has no
  field for them.

## Where the code is

Decide in pure functions, act at the edges (AGENTS.md):

| Piece | File | What |
|---|---|---|
| Decisions | `gpui-pre-windows/src/pointer.rs` (new) | device table, flags → phase table, the `Contacts` state machine, the promoted-message table, the `Clock`, the HIMETRIC mapping; plain values, no `windows` crate |
| Glue | `gpui-pre-windows/src/events.rs` | `handle_msg`'s promoted-message filter, `handle_pointer_msg`, `handle_pointer_capture_changed_msg` |
| State | `gpui-pre-windows/src/window.rs` | `WindowsWindowState::pointer_contacts` |

`events.rs` asserts each WinUser.h value `pointer.rs` writes out against the `windows` crate at compile time.

## Tests

gpui_windows builds only on Windows, so [patch-test.sh](../scripts/patch-test.sh) builds `pointer.rs` on its own as
the host crate `windows-pure` ([patch-tests.md](patch-tests.md)). Its tables:

| Test | Checks |
|---|---|
| `pointer_samples_to_touches` | one row per pointer state × sample: taps, drags, cancels, pen strokes and hovers, two fingers, reused ids, and the rejected samples |
| `capture_loss_cancels_at_last_position` | a lost pointer cancels once, where it last was |
| `pressure_to_force` | winit's pressure normalization, carried on the touch |
| `promoted_mouse_messages` | the signature check, sign extension, real mouse and non-client messages kept |
| `pointer_time_stamps` | performance count over `dwTime`, stamps after the clock read, the tick wrap |
| `himetric_locations` | display origin, sub-pixel, the empty-rect fallback |

The glue is compile-checked by CI's `windows` row ([ci.md](ci.md)) and driven with real `WM_POINTER` input by the
`windows-touch` job: injected touch, pen and mouse, the Gate 3 traces and the runtime checks
([windows-touch-harness.md](windows-touch-harness.md)).

## Extending

A new pointer type is a row in `DEVICES`; a new flag rule a row in `SAMPLES` (first match wins); another mouse
message Windows promotes a row in `PROMOTED_MOUSE_MESSAGES`. When winit's next stable tag ships, diff its
`WM_POINTER` arms and port the fixes.
