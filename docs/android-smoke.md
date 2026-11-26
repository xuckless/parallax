# Android smoke test

[android-smoke.sh](../scripts/android-smoke.sh) runs [examples/hello](examples.md#hello) on one running device or
emulator and checks, step by step, that it still opens, draws, takes taps and text, rotates and survives the
background. The same script runs locally and in the [`android-smoke` CI job](ci.md#jobs).

```sh
scripts/apply-patches.sh
scripts/android-smoke.sh [serial]   # default: the only device in `adb devices`
```

Needs what [cargo-parallax](cli.md#requirements) needs (`ANDROID_HOME`, an NDK, Java) plus `python3`. It hides
system error dialogs (a slow emulator's "Pixel Launcher isn't responding" covered the app in CI), turns auto-rotate
off in portrait, then runs the `steps` table in order. It prints each step's name and exits 0 after
`all N steps passed` and the renderer line (which GPU drew); any failure prints `FAIL: <step>: <why>` and the last
app lines of logcat, and exits 1. The run's logcat, and on failure the screen (`failure.png`), stay in
`target/android-smoke/`; CI uploads that folder as the `android-smoke` artifact when the job fails.

## Steps

Each row is `name | command | regex | seconds`: run the command, then poll logcat once a second (`adb logcat -d`;
a streaming `adb logcat` stalled for good on a CI runner) for a line logged after the command started that matches
the regex. A crash, a native signal, an ANR for the app, or a surface it cannot attach or a GPU device it cannot
recover (`host.rs`'s error lines) fails the current step at once.

| Step | Does | Waits for |
|---|---|---|
| launch | `cargo-parallax run android` (build, install, `am start -W`) | `parallax: first frame drawn on a WxH portrait surface` ([window.rs](../crates/android/src/window.rs), once per surface) |
| tap the box | taps the blue box | `box colour ef4444 (touch click` |
| focus the field | taps the first white field | an insets line with a non-zero IME bottom (the keyboard opened) |
| type and submit | `input text smoke`, then Enter | `text field submitted: 5 bytes` (the app logs the length, never the text) |
| rotate to landscape / portrait | `settings put system user_rotation` (auto-rotate turned off) | the first frame on a `landscape` / `portrait` surface (the new surface drew) |
| background | Home | `parallax: lifecycle From(…, Stop)` |
| resume | `am start` again | the first frame on a `portrait` surface |
| tap the box again | taps the red box | `box colour 22c55e (touch click` |

The rotate and resume steps wait for a frame drawn on the new surface, at the new orientation, not for the surface
event: that line is logged before the surface is made, and a late `surfaceChanged` from the previous rotation could
pass the next step. After the table it checks the whole log once more for crashes and that the app process is still alive.

To add a step, add a row to `steps`. Commands may call `on_device` (`adb shell`), `tap_colour` and `launch`.

## Finding what to tap

GPUI draws everything itself, so there is no view tree to query. `tap_colour <rrggbb>` reads a raw `screencap` and
taps the middle of the topmost band of rows where at least a quarter of the pixels have that colour. It accepts each
channel within 8, because an emulator's GL draws to a 16-bit RGB565 surface (Vulkan is exact), and retries for 10 s
while a splash screen, the keyboard or a rotation settles. Status bar icons and glyphs never fill a quarter of a row.

## In CI

The [`android-smoke` job](../.github/workflows/ci.yml) runs the script inside
[reactivecircus/android-emulator-runner](https://github.com/ReactiveCircus/android-emulator-runner) on ubuntu with
KVM opened to the runner user, JDK 21 for Gradle, and the newest NDK named as `ANDROID_NDK_HOME` (the runner has
several, [cli.md](cli.md#requirements)). The AVD and a booted snapshot are cached; a cache miss boots once to make them.

Runners have no GPU. `-gpu auto` gives lavapipe for Vulkan (ANGLE on SwiftShader for GLES), and hello draws on it.
With `swiftshader_indirect` or `swangle_indirect` the app gets SwiftShader's Vulkan, where creating the renderer
never returned and `am start -W` gave up (2026-11-26).

## Limits

- It leaves the device with auto-rotate off (portrait) and error dialogs hidden (`hide_error_dialogs 1`).
- One emulator image in CI (API 35, `google_apis`, Pixel 7 profile). The API 28/33/35 Gboard matrix of
  [ROADMAP Gate 1](../ROADMAP.md) stays a local check.
- `input text` sends key events, not IME commits: composition and autocorrect are not covered here
  ([android-text-input.md](android-text-input.md)).
