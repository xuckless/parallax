# Windows touch harness (Gate 3)

CI's `windows-touch` job ([ci.md](ci.md)) drives hello's desktop bin on the `windows-latest` runner with real
`WM_POINTER` input, records the Windows gesture traces for the replay ([traces-desktop.md](traces-desktop.md)) and
checks [patch 0021](desktop-touch-windows.md) at run time. Three pieces, all dev-only:

| Piece | What |
|---|---|
| [touch-inject](../crates/touch-inject/src/main.rs) | Rust tool on Microsoft's synthetic pointer device (`CreateSyntheticPointerDevice` / `InjectSyntheticPointerInput`, Windows 10 1809+) for touch and pen, `SendInput` for mouse clicks; also launches hello, raises, screenshots and measures its window, parks the cursor and sets the display's resolution and scale. Never published |
| [record-traces.sh](../scripts/record-traces.sh) `windows` | the gesture table and frame builders shared with Android, a `windows_frames` sink into touch-inject ([traces.md](traces.md)) |
| [windows-touch-checks.sh](../scripts/windows-touch-checks.sh) | sources record-traces.sh, runs its `checks` table: one relaunch, command and log assertions per row |

Run them in Git Bash on a Windows desktop: `scripts/apply-patches.sh`, then `scripts/record-traces.sh windows` and
`scripts/windows-touch-checks.sh [check…]`. Logs land in `target/record-traces/windows/`; CI uploads them with the
fresh traces as the `windows-touch` artifact. Commit the traces when they change.

## How touch-inject injects

A gesture is record-traces.sh's frames (`ms finger…`, logical px), turned into injections by the pure
[frames.rs](../crates/touch-inject/src/frames.rs): every pointer in range goes in every injection (the API's
"all contacts"), flags from one `(state, finger) → flags, next` table (InjectTouchInput's documented sequences), and a
held contact is re-sent in place every 16 ms, as a digitizer does, or Windows cancels it. Contacts press at 512 of
1024, so a pen's `force` reads 0.50. Points become physical pixels with the window's DPI; the tool is per-monitor
DPI aware. hello is started by touch-inject itself (Git Bash's `$!` is not hello's Windows pid) and kept topmost: the
runner's own console window comes back on top of new windows now and then. `display` uses the undocumented
DisplayConfig scale packets (see [scale.rs](../crates/touch-inject/src/scale.rs)); Windows offers 150 % only from
about 1920 × 1080, so a scaled check moves the runner from 1024 × 768 first and restores it after.

## The checks

| Check | Asserts | Result |
|---|---|---|
| tap | one Started, one Ended, no `mouse_down` on the pad, within 1 px of the target | pass |
| touch-click | tapping the box logs one touch click, count 1 | pass |
| drag-order, fling-order | per id Started, Moved…, Ended, time stamps rising | pass |
| pinch | two ids, no line twice, two Started and two Ended | pass (after the fix below) |
| reused-id | a double tap is two ids | pass, but see below |
| cancel | a held touch cancelled by a display change logs Cancelled, no Ended | see below |
| pen | every line `Pen`, moves carry `force 0.50` | pass; the lift carries no pressure |
| pen-hover | a hovering pen: mouse moves only, no touch, no press | pass |
| title-bar | a touch drag on the title bar moves the window, no touch | pass |
| mouse | a real click is `device: Mouse, synthesized: false`, no touch | pass |
| tap-150, drag-150 | positions within 1 px at 150 % | pass |

## What injected input taught us

- **Frames (fixed in 0021):** winit's read of a whole frame plus `SkipPointerFrameMessages` stopped Windows sending
  a pinch's frames after the first; one finger was later cancelled by capture loss. Each message now reads only its
  own pointer's history, as Flutter's embedder does.
- **Time stamps:** injected samples carry both `dwTime` and `PerformanceCount` (10 MHz, 0.1 ms steps), so 0021 uses
  `PerformanceCount`; moves are 16 ms apart in the traces.
- **Pointer ids:** every injected contact gets a new Windows pointer id, so Windows' id reuse cannot be forced; the
  "a reused pointer id is a new touch" row of 0021's `pointer_samples_to_touches` covers it.
- **Cancels:** an injected up marked `POINTER_FLAG_CANCELED` arrives as a plain up, so the harness cancels a held touch
  by changing the display, which cancels every injected contact.
- **Mouse copies:** none reached hello: Windows promotes no mouse messages from a pointer whose `WM_POINTER` messages
  the window handles, so the promoted-message filter is not exercised here.
- **Replay:** all six Windows traces replay as their gestures under every row and match android, web and ios within
  the replay's tolerances (fling momentum within 4 % of android's).

## Extending

A check is a row in the script's `checks` table (an `assert_<kind>` for a new kind); a new verb is a `frames_<verb>`
builder; a new device or flag sequence is a row in frames.rs's table.
