# Desktop touch traces (Gate 3)

Gate 3 ([ROADMAP.md](../ROADMAP.md), Phase 3) asks that touches injected on Windows (`InjectSyntheticPointerInput`)
and Linux (Weston's headless test protocol; X11 through translation tables), and on macOS later, replay to the same
gestures and scroll outcomes as the phone and browser traces ([traces.md](traces.md)). This page is the fixture
contract those harnesses write to; the replay side is already in place. Windows has its harness:
`scripts/record-traces.sh windows` in CI's `windows-touch` job ([windows-touch-harness.md](windows-touch-harness.md)).

## Replay

`PLATFORMS` also lists `windows`, `wayland`, `x11` and `macos`; the replay reads their folders when present and
compares them with the phone and browser traces under every row, with the same tolerances. A desktop harness
([desktop-touch-behaviour.md](desktop-touch-behaviour.md) for what a desktop touch gets) must:

- run hello's desktop bin (`cargo run -p hello`), whose window is 400 × 700 logical px, big enough for the
  `gestures` table (y up to 650; on a 1024 × 768 screen keep the window clear of a taskbar). Its logger prints to
  stderr at the default `warn,hello=info`: `[… INFO  hello] window open`, `… trace: recording`, `… trace: touch(…)`.
- tap the purple "Record touches" button (`a855f7`), wait for `trace: recording`, inject the row's gesture in window
  logical px (convert with the window's scale), and write `crates/platform/tests/traces/<platform>/<gesture>.trace`:
  a `# <platform> <gesture>: <command>` line, then the `touch(…)` lines as logged (`grep -o 'touch([^)]*)'`).
- keep the 6-field lines (`Touch` a finger, `Pen` a pen; old 5-field lines read as `Touch`), and add its verbs to
  [record-traces.sh](../scripts/record-traces.sh) ([Extending](traces.md#extending)) or write the same files from its own driver.

## Linux: Weston headless

`scripts/record-traces.sh wayland` (or `x11`) records Linux live, with no touchscreen or session:
[build-weston.sh](../scripts/build-weston.sh) builds Weston 14.0.2 from source into `target/weston/` with its
headless backend, kiosk shell, Xwayland module, screenshooter and **test plugin**, the `weston_test` global Weston's
own suite injects input with. Distributions never ship that plugin, and outside Weston's runner it needs four NULL
guards ([weston-test-standalone.patch](../scripts/weston-test-standalone.patch)).
[weston-touch.py](../scripts/weston-touch.py) (python3, no libraries) sends `weston_test.send_touch`, so hello gets
real `wl_touch` events stamped with `CLOCK_MONOTONIC`; it plays the same frames as Android (16 ms apart).

- **Output:** 400 × 700 logical px, `WESTON_SCALE` 1 (default) or 2; the kiosk shell fills it with hello's window.
- **x11:** hello runs on the Xwayland Weston starts, which turns the touches into XInput 2 touch events: scale 1 only.
- **Checks:** before the gestures, one tap on hello's box must log exactly one touch click (no extra mouse events);
  `recorded_touches_keep_their_identity` (in `trace_replay`) checks every platform's touches: a new id per touch,
  `Started`, `Moved`…, then `Ended`, going down at the script's points (scale 1 and 2 give the same logical px).
- **Elsewhere:** needs Weston's build dependencies (CI's `linux-touch` job lists them). On a host without them,
  build and run Weston in a container that shares the checkout's path and `/tmp`, e.g.
  `WESTON_RUN="podman exec <container>" scripts/record-traces.sh wayland`.

## Checking a harness

`cargo test -p parallax-platform --test trace_replay -- --nocapture` prints every outcome per gesture and row, the
new folder's beside android's, web's and ios's; `trace_line_table` in the same file pins the line format. A gesture
with no trace in a folder is skipped there, so a harness can land one gesture at a time.

A pen gesture needs a row in the script's `gestures` table and in `GESTURES` only once a second platform records
pens (Windows injects `PT_PEN`, checked by windows-touch-checks.sh, not yet traced); its trace names `Pen` on every
line, and the replay gives it the touch slops ([desktop-touch-behaviour.md](desktop-touch-behaviour.md)).
