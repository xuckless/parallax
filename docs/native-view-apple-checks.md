# Native views on macOS and iOS: the checks

[apple-native-view-checks.sh](../scripts/apple-native-view-checks.sh) runs
[examples/native-view](../examples/native-view/) ([native-view-apple.md](native-view-apple.md#the-example))
on an iOS simulator on pixl-mac and checks the acceptance list of
[ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview) for patches 0059–0064. It rsyncs the repo (with
`build/parallax-gpui`, so run [apply-patches.sh](../scripts/apply-patches.sh) first), builds the example as a bare
.app from upstream's example Info.plist, drives it with idb as [traces.md](traces.md) does, and shuts its simulator
and idb companion down after.

```sh
scripts/apple-native-view-checks.sh "iPhone 17"                # 3x
scripts/apple-native-view-checks.sh "iPad Air 11-inch (M4)"    # 2x
scripts/apple-native-view-checks.sh "iPhone 17" /Users/Shared/parallax/my-dir   # its own dir on the Mac
```

| Row | Passes when |
|---|---|
| idle | the last report says `gpui frames/s 0` with the view at 50 frames/s or more |
| still | every frame of 3 s recorded with `simctl io recordVideo` passes the frame check |
| input | a tap on the view logs one view-relative `view pointer down`, a swipe on it `view pointer scroll`, a tap on the div one `div over view: down` |
| moving | the same while "Move" moves and resizes the slot every frame |
| rotating | the same through a rotation to landscape and back |
| keyboard | the native `UITextField` became first responder, "Drop views" dropped every native view, and typing "abc" and Return on the GPUI field logs hello's `text field submitted: 3 bytes` |

## The frame check

[apple-native-view-frames.py](../scripts/apple-native-view-frames.py) reads any recording ffmpeg decodes (a
`simctl` video, macOS's `screencapture -v`: the example's colours are the same there, or a numbered frame series)
and requires of **every** frame: the view's hues on at least 1% of it, the div's orange on at least 0.3%, no pixel
of the magenta gap colour, and no black within 3 px of the view or div unless that black reaches the frame's edge or
its top band (the window around a turning screen, the Dynamic Island). A frame where hole and view part shows magenta
(the example's `set_background`); a hole with nothing behind it shows black; a view that draws nothing or a hole that
isn't cut loses the view's colour. Colour classes and thresholds are tables at its top.

Shown to fail: with the example's view drawing nothing, every frame of the still, moving and rotating rows fails
(no view colour, up to 220 000 gap-colour pixels); with 0064's drop not handing the keyboard back, the keyboard row
fails (`text field submitted: 0 bytes`: Return reached GPUI, the text didn't). On the previous lane's recordings it
finds exactly the 38 of 617 gap frames of the rotation before the rigid turn.

## Results (2026-11-28)

iOS 26.5 simulator, `examples/native-view`, after patch [0063](../patches/0063-ios-native-view-rotation.patch)'s
turn fix: iPhone 17 (3x) five runs, iPad Air 11-inch M4 (2x) one, every row passing in all six. Idle at 0 GPUI and
63 view frames/s (`metal layer alpha: premultiplied`); still, moving and rotating 0 failing frames and 0 gap pixels of
220–227, 442–450 and 384–390 (iPad 218, 450, 404); input (`view pointer down at (44.0, 190.0)`, a scroll, the div's
press alone); keyboard to GPUI's field after the drop.

**The rotation's pixel** (before that fix: 1 run in 5, 3 of 12 back-to-back rotations). One frame at the turn's start
had a single gap-colour pixel (2×2 device pixels, about (250, 100, 250)) where the turning window's antialiased edge
crossed a native view's edge. Not H.264's 4:2:0 chroma: no neighbour has magenta chroma. Core Animation antialiases
the turning edge layer by layer, so the host's `set_background` colour under the view fringed it. The host now shows
no colour while the window turns (`Rotation::shows_background`); the rigid turn leaves no gap for it to cover.

macOS needs a window server, which `ssh` doesn't have: the owner records the example (`screencapture -v`, moving and
resizing the window) and runs the frame check on it, steps in `/Users/Shared/parallax/w15-fix-mobile-ios-macos/CHECK.md`
on pixl-mac.
