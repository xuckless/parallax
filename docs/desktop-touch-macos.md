# Desktop touch: macOS

Patches [0025](../patches/0025-touch-macos-translation.patch) (the decisions, on plain values) and
[0026](../patches/0026-touch-macos-recognizer.patch) (the AppKit glue) feed direct touches (a touchscreen Mac, or an
iPad used as a Sidecar display) into GPUI as `TouchEvent`s, from macOS 27; [0024](../patches/0024-uptime-instant-apple.patch)
moves the time stamp maths into gpui_apple for them and iOS. Phase 3 `touch-macos` in [ROADMAP.md](../ROADMAP.md).

## How AppKit delivers touches (MacOSX27.2.sdk headers)

| Symbol | Header | Use here |
|---|---|---|
| `-[NSGestureRecognizer touches{Began,Moved,Ended,Cancelled}WithEvent:]` (10.12.2, `NSSubclassUse`) | NSGestureRecognizer.h | the four overrides |
| `NSGestureRecognizer.allowedTouchTypes` (10.12.2; defaults to `NSTouchTypeMaskDirect` as of macOS 27, "only Direct is supported") | NSGestureRecognizer.h | set to Direct explicitly |
| `-[NSEvent touchesMatchingPhase:inView:]` (10.6) | NSEvent.h | the changed touches, and those still down |
| `NSTouch.identity`, `.phase`, `-locationInView:` (Direct only), `.type` | NSTouch.h | id, phase, position |
| `NSScreen.touchCapabilities` (`.multiTouch`), `NSView.exclusiveGestureBehavior`, `NSPanGestureRecognizer.minimum/maximumNumberOfTouches`, `NSScrollView.touchScrollingEnabled`, `NSEvent.touchSwipeNavigationEnabled` | NSScreen.h, NSView.h, … | macOS 27.0, not needed |

`NSTouch` has a per-touch position, so one recognizer covers every finger; no per-count recognizers. The touches
reach recognizers only, not the view's responder methods (Apple's macOS 27 notes; the headers don't say).
`NSIsTouchNative` is in no header; the macOS 27.3 system libraries contain the string (AppKit reads it from the
main bundle's Info.plist).

## The glue

`GPUITouchRecognizer` (`gpui-pre-macos/src/window/touch_recognizer.rs`) is an `NSGestureRecognizer` subclass on
every GPUIView, added only when `isOperatingSystemAtLeastVersion(27.0)`. AppKit calls each `touches…WithEvent:`
callback that a touch of the event is in, all with the same event (a finger landing while another moves calls
`touchesBegan` and `touchesMoved`), so each callback (`handle_touches::<PHASE>`) takes only the event's touches in
its own phase (`TouchIds::track_callback`); read whole, the Sidecar check saw every landing finger start, cancel and
start again, and every other finger move twice. It sends, through the tables in `window/touch.rs`:

| `TouchEvent` field | From |
|---|---|
| `phase` | `PHASES` table: Began → `Started`, Moved, Ended, Cancelled |
| `id` | `TouchIds`: the next id (from 0 per window, wrapping) per live `NSTouch.identity` (`isEqual:`), given up when it ends; a move or end with no live begin is dropped, a begin of a live identity cancels the stale touch first |
| `position` | `locationInView:` on GPUIView, y flipped like mouse events; a cancel is where the touch last was (AppKit puts one the system took at (0, -32)) |
| `device` | `Touch` (allowed types are Direct only) |
| `time_stamp` | `NSEvent.timestamp` by its age against `systemUptime` (gpui_apple `uptime_instant`, shared with iOS; [touch-time-stamps.md](touch-time-stamps.md)) |

The recognizer's state follows `next_state(state, left)`, one row per legal transition: `Began` on the first touch,
`Changed`, then `Ended` or `Cancelled` once no finger is down; a sequence over within one event `Failed`s; nothing
moves on from a finished state, which AppKit resets. Its `reset` override cancels any touch still live where it last
was. Touches reach GPUI through `TouchQueue`: while GPUI's callback is busy, an end waits for it and a start or
move is dropped. While it holds touches, `handle_view_event` drops AppKit's copies of them (`TOUCH_COPIES`: mouse
events with `subtype` `NSEventSubtypeTouch`; scroll and magnify, which have no subtype, any mid-touch), and
GPUIView answers `mouseCancelled:` (macOS 26) with a mouse up, so no button stays down.

Emulation and `NSIsTouchNative`: [desktop-touch-macos-emulation.md](desktop-touch-macos-emulation.md).

## Limits

- Run on touch hardware once: the owner's Sidecar check (Wave 13; logs and report in
  `/Users/Shared/parallax/w13-macos-touch/` on the owner's Mac). Its fixes (one id per finger, cancels where the
  finger was) wait for the re-check in `/Users/Shared/parallax/w14-macos-touch/`.
- `NSScreen.touchCapabilities` says `.multiTouch` for every display while a Sidecar touch iPad is connected, so it
  can't tell which screen takes touch; GPUI reads the device per event instead.
- No force (direct `NSTouch` has none) or predicted position; coalesced touches (`coalescedTouchesForTouch:`) are
  not replayed, one sample per callback as on iOS.
- Apple's macOS touch target is 36 pt (`APPLE_MACOS_MIN_TOUCH_TARGET`, from Apple's "Bring Touch to your Mac app"
  talk, not the SDK headers): the behaviour table's macOS row ([behaviour.md](behaviour.md)).
- A real trackpad scroll or magnify while a finger is on the screen is dropped as a touch copy.

## Tests

`touch_phases`, `recognizer_states`, `touch_ids`, `appkit_callbacks` (the Sidecar logs replayed, one row per
callback: a finger landing or lifting while another moves, a Spotlight swipe's system cancel; every started id ends
or is cancelled), `touch_queue` and `touch_copies` in `touch.rs`: `cargo test -p
parallax-gpui-macos --features test-support --lib touch` inside `build/parallax-gpui/` on a Mac.
