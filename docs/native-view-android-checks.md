# Android native view checks

[android-native-view-checks.sh](../scripts/android-native-view-checks.sh) runs
[examples/native-view](../examples/native-view/) on one emulator or device and asserts on what reaches
the screen and the log ([native-view-android.md](native-view-android.md) explains the code). Run it with one device
up: `scripts/android-native-view-checks.sh [serial]`. Screenshots and logcat land in `target/android-native-view/`.

| Row | How |
|---|---|
| 0 GPUI frames/s while only the surface draws | the example's once-a-second log line |
| alpha mode logged | the renderer's `window surface alpha mode` line, or its `only Opaque` warning (holes black) |
| moving slot, per kind (`view`; `control` API 29+; `hosted` API 34+) | `--ez animate true` moves and resizes the slot every frame; 12 raw screenshots in a row, each read by [pixels.py](../scripts/pixels.py): the surface's colour (`**c040`) and the GPUI div's (`2050d0`) each over 2000 pixels, and no underlay (`ff00ff`, the gap) more than 8 pixels inside their box (the ring is the growing edge, [hosting](native-view-android-hosting.md#the-growing-edge)). A gap fails `control`; `view` and `hosted` lag a frame by design, so their gap frames are counted, not failed |
| two overlapping views stack in paint order (`view`, `control`) | `--ez stack true`: the yellow tile is attached first but painted last; its top-left corner, over the red tile, must be yellow |
| Back after tapping the WebView reaches GPUI | a tap forwarded to the WebView, then `KEYCODE_BACK`: GPUI's back listener logs `Committed` |

Where holes are black (GL below API 30, [hosting](native-view-android-hosting.md#gl-and-vulkan)) the pixel rows are
skipped, said so in the output.

A check that can't fail checks nothing: each pixel row was run once against a broken example and failed (results
below).

## Results (2026-11-28)

| Device | Result |
|---|---|
| API 35 emulator (GL first, rebuilt on Vulkan when the views show) | every row passes. Frames with a gap: `control` 0 of 12 in three runs of four (the fourth had two frames whose growing strip, the underlay while the producer catches up, was wider than the ring); `view` 1–3 of 12 (a frame behind); `hosted` 0–1 of 12 (its relayout after a grow) |
| API 28 emulator (GL: the renderer logs `only Opaque`) | frames/s, alpha line and Back pass; the pixel rows are skipped, as holes are black there |

Shown to fail on a broken example: with the surface's producer drawing nothing, the frames/s row and every moving-slot
row fail (12 of 12 frames without the view); with the Views' Z not set, the `view` stack row fails (red on top); with
Back not routed to GPUI, the Back row fails (the Activity takes it).
