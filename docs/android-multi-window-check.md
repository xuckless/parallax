# Android multi-window: checking it

How to check [android-multi-window.md](android-multi-window.md)'s windows on an emulator.

## Check it

On an emulator with API 33+ (split screen through the WM shell; `hello` logs `window open, N in all` and
`Activity N destroyed: its window closes`, and `lifecycle …` lines once per phase, not once per window):

1. `cargo parallax run android --device <serial>` in `examples/hello`, then `adb shell input keyevent KEYCODE_HOME`.
2. A second instance in its own task (the template's `singleTop` only reuses the shown one):
   `adb shell am start -n io.github.xuckless.parallax.hello/io.github.xuckless.parallax.ParallaxActivity -f 0x18000000`
   (`NEW_TASK | MULTIPLE_TASK`).
3. `adb shell am stack list` for the two task ids, then split them:
   `adb shell dumpsys activity service SystemUIService WMShell splitscreen moveToSideStage <first task> 1`.
4. Tap each half's box and field, type into each; rotate (`settings put system user_rotation 1`); press Back in one
   half: the other goes full screen and keeps its state.

Checked on API 35 (Gboard 14.2, GL): both halves draw, take taps and text, rotate with their state, and a destroyed
half's window closes while the other keeps drawing.

An app closing its window finishes its Activity at once, even in the background: in hello press Ctrl-W
(`adb shell input keycombination KEYCODE_CTRL_LEFT KEYCODE_W`), then Home within 3 s. Logcat shows the Activity's
`Stop`, then `the app closed Activity N's window: it finishes` and its `Destroy` right after, not when the user
returns. Checked on API 35.
