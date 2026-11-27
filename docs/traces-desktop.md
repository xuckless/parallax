# Desktop touch traces (Gate 3)

Gate 3 ([ROADMAP.md](../ROADMAP.md), Phase 3) asks that touches injected on Windows (`InjectSyntheticPointerInput`)
and Linux (Weston's headless test protocol; X11 through translation tables), and on macOS later, replay to the same
gestures and scroll outcomes as the phone and browser traces ([traces.md](traces.md)). This page is the fixture
contract those harnesses write to; the replay side is already in place.

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

## Checking a harness

`cargo test -p parallax-platform --test trace_replay -- --nocapture` prints every outcome per gesture and row, the
new folder's beside android's, web's and ios's; `trace_line_table` in the same file pins the line format. A gesture
with no trace in a folder is skipped there, so a harness can land one gesture at a time.

A pen gesture needs a row in the script's `gestures` table and in `GESTURES` only once some platform records pens
(Windows can inject `PT_PEN`); its trace names `Pen` on every line, and the replay gives it the touch slops
([desktop-touch-behaviour.md](desktop-touch-behaviour.md)).
