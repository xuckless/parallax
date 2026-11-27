# Desktop touch: macOS

[Patch 0024](../patches/0024-touch-macos.patch) feeds direct touches (a touchscreen Mac, or an iPad used as a Sidecar
display) into GPUI as `TouchEvent`s, from macOS 27. Phase 3 `touch-macos` in [ROADMAP.md](../ROADMAP.md).

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

`GPUITouchRecognizer` (`gpui-pre-macos/src/window/touch.rs`, added by the patch) is an `NSGestureRecognizer`
subclass on every GPUIView, added only when `isOperatingSystemAtLeastVersion(27.0)`. Per event it reads the changed touches (stationary ones skipped) and sends:

| `TouchEvent` field | From |
|---|---|
| `phase` | `PHASES` table: Began → `Started`, Moved, Ended, Cancelled |
| `id` | `TouchIds`: a fresh id per live `NSTouch.identity` (`isEqual:`), dropped when it ends; AppKit may reuse identities, GPUI ids never repeat |
| `position` | `locationInView:` on GPUIView, y flipped like mouse events |
| `device` | `Touch` (allowed types are Direct only) |
| `time_stamp` | `NSEvent.timestamp` by its age against `systemUptime` (gpui_apple `uptime_instant`, shared with iOS; [touch-time-stamps.md](touch-time-stamps.md)) |

The recognizer claims the sequence (`Began` on the first touch, `Changed`, then `Ended` or `Cancelled` once no finger
is down: `next_state`): GPUI's own recognizers turn the touches into taps, pans, long presses and pinches.

## Emulation and `NSIsTouchNative`

Without the key AppKit emulates: tap → `mouseDown`/`mouseUp`, pan → trackpad scroll, long press → right click,
pinch / rotate → magnify / rotate, hold and move → drag. An app with its own touch pipeline opts out:

```xml
<key>NSIsTouchNative</key><true/>
```

in `Contents/Info.plist` of its `.app` (cargo-parallax does not bundle macOS apps yet; add the key to your bundler's
plist, e.g. cargo-bundle's `osx_info_plist_exts`). Then touches arrive only as `TouchEvent`s and GPUI's touch →
mouse ([pointer-source.md](pointer-source.md)) makes the clicks. Without the key, whether AppKit still emulates
after the recognizer claims the touches is unverified: a tap may arrive twice (touch click and mouse click). The
owner's Sidecar check settles it.

## Limits

- Not run on touch hardware yet: verified by `cargo clippy -D warnings`, the unit tests and a build on macOS 27.3.
  The live check (both bundles, Sidecar steps) is in `/Users/Shared/parallax/w13-macos-touch/` on the owner's Mac.
- `NSScreen.touchCapabilities` says `.multiTouch` for every display while a Sidecar touch iPad is connected, so it
  can't tell which screen takes touch; GPUI reads the device per event instead.
- No force (direct `NSTouch` has none) or predicted position; coalesced touches (`coalescedTouchesForTouch:`) are
  not replayed, one sample per callback as on iOS.
- Apple's macOS touch target is 36 pt (Apple's macOS 27 guidance, not in the SDK headers): the behaviour table's
  macOS row ([behaviour.md](behaviour.md)).

## Tests

`touch_phases`, `recognizer_states` and `touch_ids` in `touch.rs`: `cargo test -p parallax-gpui-macos --features
test-support --lib touch` inside `build/parallax-gpui/` on a Mac.
