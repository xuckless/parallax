# Android smoke test

[android-smoke.sh](../scripts/android-smoke.sh) runs [examples/hello](examples.md#hello) on one running device or
emulator and checks, step by step, that it still opens, draws, takes taps and text, rotates and survives the
background. The same script runs locally and in the [`android-smoke` CI job](ci.md#jobs).

```sh
scripts/apply-patches.sh
scripts/android-smoke.sh [serial]   # default: the only device in `adb devices`
```

Needs what [cargo-parallax](cli-android.md#requirements) needs (`ANDROID_HOME`, an NDK, Java) plus `python3`. It hides
new system error dialogs (a slow emulator's "Pixel Launcher isn't responding" covered the app in CI), turns auto-rotate
off in portrait (the `settings` table; an `EXIT` trap puts the old values back, pass or fail), then runs the `steps`
table in order. It prints each step's name and exits 0 after `all N steps passed` and the renderer line (which GPU
drew); any failure prints `FAIL: <step>: <why>` and the last app lines of logcat, and exits 1. The run's logcat, and on failure the screen (`failure.png`), stay in
`target/android-smoke/`; CI uploads that folder as the `android-smoke` artifact when the job fails.

## Steps

Each row is `step <name> <command> <regex> <seconds>` (trusted code: the command runs through `eval`): run the
command, then poll logcat once a second (`adb logcat -d`; a streaming `adb logcat` stalled for good on a CI runner)
for a line logged at or after the command started that matches the regex. A crash, a native signal, an ANR for the
app, or a surface it cannot attach or a GPU device it cannot recover (`host.rs`'s error lines) fails the current step
at once.

| Step | Does | Waits for |
|---|---|---|
| launch | `hello_android_run` ([hello-android.sh](../scripts/hello-android.sh), shared with record-traces.sh): `cargo-parallax run android` (build, install, `am start -W`) | `parallax: first frame drawn on a WxH portrait surface` ([window.rs](../crates/android/src/window.rs), once per surface) |
| tap the box | taps the blue box | `box colour ef4444 (touch click` |
| focus the field | taps the first white field | an insets line with a non-zero IME bottom (the keyboard opened) |
| type and submit | `input text smoke`, then Enter | `text field submitted: 5 bytes` (the app logs the length, never the text) |
| rotate to landscape / portrait | `settings put system user_rotation` (auto-rotate turned off) | the first frame on a `landscape` / `portrait` surface (the new surface drew) |
| background | Home | `parallax: lifecycle From(…, Stop)` |
| resume | `am start` again | the first frame on a `portrait` surface |
| tap the box again | taps the red box | `box colour 22c55e (touch click` |

The rotate and resume steps wait for a frame drawn on the new surface, at the new orientation, not for the surface
event: that line is logged before the surface is made, and a late `surfaceChanged` from the previous rotation could
pass the next step. After the table it checks the whole log once more for crashes and that the app process is still
alive. To add a step, add a `step` row; commands may call `on_device` (`adb shell`), `tap_colour`, `rotate`, `hello_android_run`.

**Dialogs:** `hide_error_dialogs` stops new error dialogs but leaves one already up (a cached booted snapshot, or a
launcher that stalled during boot, brings it), and it took the taps in CI. Before each step `dismiss_dialogs` reads
the focused window (`dumpsys window`, `mCurrentFocus`); while it is an `Application Not Responding:` or
`Application Error:` dialog for another app, it force-stops that app (its dialog closes with the process) and
broadcasts `CLOSE_SYSTEM_DIALOGS`, up to 5 times. A dialog for hello is left to fail the step (`ANR in` the app).

**Rotation:** `rotate N` writes `user_rotation` and waits up to 10 s for `dumpsys window` (whole: API 28 prints it outside `displays`) to report
`mCurrentRotation` N. One CI emulator never changed configuration, so a second try writes the setting again plus
`wm user-rotation lock N`; after that it fails with `the emulator did not rotate to N`, before the frame wait.

Finding tap targets, the CI job and the limits: [android-smoke-ci.md](android-smoke-ci.md).
