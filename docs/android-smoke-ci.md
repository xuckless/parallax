# Android smoke test: tap targets, CI, limits

The rest of [android-smoke.md](android-smoke.md): how [android-smoke.sh](../scripts/android-smoke.sh) finds
what to tap, how CI runs it, and what it cannot check.

## Finding what to tap

GPUI draws everything itself, so there is no view tree to query. `tap_colour <rrggbb>` taps the middle of
the topmost band of rows where at least a quarter of the pixels have that colour, found on a raw `screencap` by
[locate-colour.py](../scripts/locate-colour.py) (also [record-traces.sh](traces.md)'s finder). It accepts each
channel within 8, because an emulator's GL draws to a 16-bit RGB565 surface (Vulkan is exact), and retries for 10 s
while a splash screen, the keyboard or a rotation settles. Status bar icons and glyphs never fill a quarter of a row.

## In CI

The [`android-smoke` job](../.github/workflows/ci.yml) runs the script inside
[reactivecircus/android-emulator-runner](https://github.com/ReactiveCircus/android-emulator-runner) on ubuntu with
KVM opened to the runner user, JDK 21 for Gradle, and the newest NDK named as `ANDROID_NDK_HOME` (the runner has
several, [cli.md](cli.md#requirements)). The AVD and a booted snapshot are cached; a cache miss boots once to make them.
The job's `AVD_*` env names the image once; the cache key is built from it, the emulator runner's tag (`AVD_RUNNER`,
kept equal to its `uses:` lines) and `AVD_CACHE_BUMP`, bumped by hand to drop a bad cached AVD.

Runners have no GPU. `-gpu auto` gives lavapipe for Vulkan (ANGLE on SwiftShader for GLES), and hello draws on it.
With `swiftshader_indirect` or `swangle_indirect` the app gets SwiftShader's Vulkan, where creating the renderer
never returned and `am start -W` gave up (2026-11-26).

## Limits

- Killed with `SIGKILL`, it cannot restore the device settings it changed.
- One emulator image in CI (API 35, `google_apis`, Pixel 7 profile). The API 28/33/35 Gboard matrix of
  [ROADMAP Gate 1](../ROADMAP.md) stays a local check.
- `input text` sends key events, not IME commits: composition and autocorrect are not covered here
  ([android-text-input.md](android-text-input.md)).
