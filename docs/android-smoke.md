# Android smoke test

[android-smoke.sh](../scripts/android-smoke.sh) runs [examples/hello](examples.md#hello) on one running device or
emulator and checks, step by step, that it still opens, draws, takes taps and text, rotates and survives the
background. The same script runs locally and in the [`android-smoke` CI job](ci.md#android-smoke).

```sh
scripts/apply-patches.sh
scripts/android-smoke.sh [serial]   # default: the only device in `adb devices`
```

Needs what [cargo-parallax](cli.md#requirements) needs (`ANDROID_HOME`, an NDK, Java) plus `python3`. It clears
logcat, builds `cargo-parallax`, then runs the `steps` table in order. It prints each step's name and exits 0 after
`all N steps passed`; any failure prints `FAIL: <step>: <why>` and the last app lines of logcat, and exits 1.

## Steps

Each row is `name | command | regex | seconds`: run the command, then wait for a **new** logcat line matching the
regex. A crash, a native signal or an ANR for the app fails the current step at once.

| Step | Does | Waits for |
|---|---|---|
| launch | `cargo-parallax run android` (build, install, `am start -W`) | `parallax: first frame drawn` ([window.rs](../crates/android/src/window.rs), logged once per window) |
| tap the box | taps the blue box | `box colour ef4444 (touch click` |
| focus the field | taps the first white field | an insets line with a non-zero IME bottom (the keyboard opened) |
| type and submit | `input text smoke`, then Enter | `text field submitted: 5 bytes` (the app logs the length, never the text) |
| rotate to landscape / portrait | `settings put system user_rotation` (auto-rotate turned off) | `parallax: surface … Resize` |
| background | Home | `parallax: lifecycle From(…, Stop)` |
| resume | `am start` again | `parallax: surface … Attach` (a new surface) |
| tap the box again | taps the red box | `box colour 22c55e (touch click` |

After the table it checks the whole log once more for crashes and that the app process is still alive.

To add a step, add a row to `steps`. Commands may call `on_device` (`adb shell`), `tap_colour` and `launch`.

## Finding what to tap

GPUI draws everything itself, so there is no view tree to query. `tap_colour <rrggbb>` reads a raw `screencap` and
taps the middle of the topmost band of rows where at least a quarter of the pixels have that colour. It accepts the
colour scaled by 0.5–1.1, because the emulator shows hello's colours dimmed (about ×0.88), and retries for 10 s
while a splash screen, the keyboard or a rotation settles. Status bar icons and glyphs never fill a quarter of a row.

## Limits

- It leaves auto-rotate off on the device (`accelerometer_rotation 0`, portrait).
- One emulator image in CI (API 35, `google_apis`, Pixel 7 profile). The API 28/33/35 Gboard matrix of
  [ROADMAP Gate 1](../ROADMAP.md) stays a local check.
- `input text` sends key events, not IME commits: composition and autocorrect are not covered here
  ([android-text-input.md](android-text-input.md)).
