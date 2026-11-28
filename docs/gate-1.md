# Gate 1 check

Gate 1 ([ROADMAP.md](../ROADMAP.md), Phase 1): [examples/hello](examples.md) on the Android emulator, API 28, 33
and 35, with Gboard. First run 2026-11-26 on `wave-10` (10e6361) found failures 1–5 below; re-check on
2026-11-26 on merged `wave-10` (64d3838), all three on GL (emulators try GL first).

**Gate 1: PASS.** Every row passes except Gboard select mode, an owner-deferred known limit (5).

## Results (re-check, 64d3838)

| Check | API 28 (Gboard 7.1) | API 33 (Gboard 12.4) | API 35 (Gboard 14.2) |
|---|---|---|---|
| 1. Launch, draw, tap the box | pass | pass (cold start ×3, relaunch after back) | pass |
| 2. Typing, backspace | pass | pass | pass |
| 2. Autocorrect | pass (worl → world) | pass (teh → The) | not testable: Gboard corrects nothing (Messages alike) |
| 2. Suggestion pick | pass (goo → good) | pass (good; backspace reopens it) | pass (good) |
| 3. English composing region | pass, underlined | pass, underlined | Gboard opens none (Messages alike) |
| 3. Japanese romaji → kana → kanji → commit | pass | pass | pass |
| 3. Korean jamo → syllables | pass (한국) | pass | pass |
| 3. Composing left on focus change | pass (Korean ㅎ and 한국, Japanese か) | pass (Korean, Japanese) | pass (Korean, Japanese) |
| 4. Tap moves caret; Gboard space-swipe | pass | pass | pass |
| 4. Gboard arrows, home/end, copy/paste | pass | pass | pass |
| 4. Gboard select mode (shift + arrows) | known limit (5) | known limit | known limit; pass in Wave 14 (5) |
| 4. Select all + replace via IME | pass | pass | pass |
| 5. Emoji panel inserts colour emoji, backspace | pass | pass | pass |
| 5. Multi-script line (Latin, CJK, Arabic, emoji) | pass | pass | pass |
| 6. Rotation with keyboard open (redraw, no input) | pass, both ways | pass, both ways | pass, both ways |
| 6. Home + resume, recents switch | pass | pass | pass |
| 7. Email field: email layout (0x21, @ key) | pass | pass | pass |
| 8. Back hides keyboard, then exits | pass | pass | pass |
| Colours: white field / background | 255 / 24,28,41 | 255 / 24,28,41 | 255 / 24,28,41 |
| Cold start (`am start -W`) | 0.30–0.35 s | 0.35–0.46 s | 0.47–0.52 s |

Focus change: compose, tap Email, tap back, type: the old text keeps no underline, the IME's next edit starts a new
composing range after it (logcat `Composing start` = old length), nothing is replaced. Colours: on GL, wgpu uses a
16-bit EGL config, so dark colours are quantized (`#1e1e2e` reads 24,28,41); white is exact. No crash, ANR or
native signal in logcat. Gboard behaviour seen in native fields too, not failures: after Home inside a composing
word the next letter still joins it (Messages on API 33 gives the same "Wordq"); Gboard 14's Japanese keyboard keeps
its alphanumeric mode from the email field when focus returns to the prose field (`inputType` is `0xc001` there).
The example has no long-press or double-tap selection; handles are Phase 4.

## Failures of the first run

1. ~~**API 33 Vulkan.**~~ Fixed: emulators try GL first ([backend.rs](../crates/android/src/backend.rs)).
2. ~~**Composing kept after focus moves.**~~ Fixed: GPUI ends a field's composition when focus leaves it
   ([patch 0010](core-patches.md)) and a new client never gets a composing range (`Pending::commit`,
   [android-text-input.md](android-text-input.md)).
3. ~~**API 28 rotation.**~~ Fixed: a resize makes a new surface ([android-surface.md](android-surface.md#resize)).
4. ~~**Dimmed colours.**~~ Fixed: Android's default focus highlight is off.
5. ~~**Gboard select mode**~~ (owner-deferred to Phase 4, fixed in Wave 14): Gboard sends a Shift press with meta
   state 0, then arrows with `META_SHIFT_ON` and no Shift held; Flutter's responder trusts a virtual keyboard's meta
   state and synthesizes no Shift, so GPUI got plain arrows. The meta state now reaches the keystroke
   ([android-keys.md](android-keys.md)); checked on API 35 (Gboard 14.2): Select + arrows and Home extend, Select off
   moves the caret.

## Rerun it

1. `git checkout <branch>`, `scripts/apply-patches.sh`, `cargo install --path crates/cli` ([cli.md](cli.md)).
2. One emulator at a time (memory budget in [AGENTS.md](../AGENTS.md)):
   `emulator -avd <avd> -port 5600 -memory 2048 -cores 2 -no-snapshot-save -gpu host`. AVDs: `parallax_fonts_28`,
   `parallax_gate_33`, `parallax_input_35`. All are `google_apis` images with Gboard preloaded, not Play images.
3. `cd examples/hello && cargo parallax run android --device emulator-5600`.
4. Gboard settings → Languages → add 日本語 (QWERTY) and 한국어 (2-Bulsik); long-press the globe key to switch.
   The API 28 AVD runs in Chinese with Gboard Pinyin.
5. Drive Gboard with `adb shell input tap` on its keys (Gboard's text-editing panel for arrows and copy/paste) and screenshot with `adb exec-out screencap -p`; rotate with
   `settings put system user_rotation 1|0` (after `accelerometer_rotation 0`).
6. To see IME edits as lengths and ranges, raise the logger to Debug in `on_load` ([host.rs](../crates/android/src/host.rs))
   in a local build only; never commit it, never log typed text.
7. Compare a doubtful case with a native field (Messages, Contacts) on the same AVD before calling it a failure.
