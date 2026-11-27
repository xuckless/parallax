# Touch traces: record and replay

Gate 2 ([ROADMAP.md](../ROADMAP.md), Phase 2) asks that the same gestures recorded on the Android emulator, a browser
and the iOS simulator replay to the same gestures and scroll everywhere ([AGENTS.md](../AGENTS.md), Decisions
"Gesture parity"). Three parts: hello's trace pad records, [record-traces.sh](../scripts/record-traces.sh) drives
each platform, [trace_replay.rs](../crates/platform/tests/trace_replay.rs) replays.

## The recorder

hello's purple "Record touches" button swaps the window for a trace pad ([lib.rs](../examples/hello/src/lib.rs),
`trace_pad`). It takes every touch raw with `on_touch` (so nothing is turned into taps or scrolls) and logs one line
per `TouchEvent` GPUI receives, in the [trace format](gesture-recognizers.md#trace-format):
`trace: touch(at_ms, id, phase, x, y)`, ms since the pad's first touch by the platform's event time
(`TouchEvent::time_stamp`, [touch-time-stamps.md](touch-time-stamps.md)), window logical px. Only a relaunch leaves
the pad. It logs no text.

## Recording

```sh
scripts/apply-patches.sh
scripts/record-traces.sh web            # or android, ios; optional gesture names: record-traces.sh ios fling
```

It sets the platform up, finds the button by colour ([locate-colour.py](../scripts/locate-colour.py)), then per row of
its `gestures` table relaunches hello, taps the button, performs the gesture and writes
`crates/platform/tests/traces/<platform>/<gesture>.trace`. Gestures are in logical px and ms, the same everywhere.

| Platform | App | Driver | Limits |
|---|---|---|---|
| web | `wasm-pack` build served on 127.0.0.1:8765, headless Chromium | [cdp-touch.mjs](../scripts/cdp-touch.mjs): CDP `Input.dispatchTouchEvent` (Node 22, no deps) | headless frames every ~33 ms, so moves are ~33 ms apart |
| ios | hello's desktop bin built for `aarch64-apple-ios-sim` on the owner's Mac, packed with upstream's example `Info.plist` | Meta's [idb](https://github.com/facebook/idb) `ui tap / multi-tap / swipe / pinch` | none found |
| android | `cargo parallax run android` | `adb shell input` (taps as zero-length swipes) | no multi-touch: pinch skipped; each `input` call starts a JVM, so a double tap's gap may exceed 300 ms |

**iOS on the Mac** (`ssh pixl-mac`, everything under `/Users/Shared/parallax/`): the checkout is rsynced to
`traces/parallax`, built with `traces/cargo` and `traces/target` and the gap audit's 1.98.1 toolchain (read-only),
packed as `traces/Hello.app` and run on the `iPhone 17` simulator (`IOS_SIMULATOR` overrides). idb lives in `tools/`:
`idb-venv` (fb-idb 1.7.8 from its release wheel) and `idb-companion` (the 1.7.8 `macos-arm64` release, unpacked);
the script starts the companion on port 10882 when nothing listens there. Nothing is installed system-wide.

## Replay

`cargo test -p parallax-platform --test trace_replay` (part of `cargo test --workspace`, so CI runs it) replays
every recorded trace through GPUI's test platform under each behaviour row ([behaviour.md](behaviour.md)) plus the
web's, with a frame every 16 ms so flings run their momentum. Per trace it sums taps (click counts), long press,
the vertical scroll while a finger is down (pan) and after release (momentum), then checks:

- **Recognized:** each trace is its gesture under every row (`GESTURES`: tap, double tap, long press, drag that
  stops, fling).
- **Same everywhere:** pan and momentum agree across platforms within `SCROLL_TOLERANCE` (35 % of the larger). Pans
  differ only by a last move; a fling's momentum comes from a velocity estimate each platform samples at its own
  rate, timed by its event time stamps. `slow-drag` moves at
  40 px/s, under every row's minimum fling velocity, so how long a driver waits before lifting cannot make it fling.
- **Known differences** (`KNOWN_DIFFERENCES`): printed, not compared, each with its open fix. None today. Until
  touches carried the platform's event time (patch 0020), least-squares flings from the web, timed when GPUI
  received the browser's ~33 ms frames, came out ~40 % short of iOS's; with time stamps the fling traces agree within
  35 % under every row. What is left is the drivers' own speed: idb's 150 ms swipe takes ~200 ms on iOS.

`pinch` traces are recorded but not replayed until the scale recognizer is wired into touch dispatch: GPUI ignores the
second finger, and the platforms disagree on which finger is first.

## Extending

A gesture: a row in the script's `gestures` table and in `GESTURES`, then record it on every platform. A platform:
its `<platform>_setup / _launch / _log / _screenshot / _scale` and one function per verb in the script, and its
folder name in `PLATFORMS`.
