# Gate 1 check

Gate 1 ([ROADMAP.md](../ROADMAP.md), Phase 1): [examples/hello](examples.md) on the Android emulator, API 28, 33
and 35, with Gboard. Last run: 2026-11-26 on `wave-10` (10e6361). **Not passed**: see Failures.

## Results

| Check | API 28 (Gboard 7.1, GL) | API 33 (Gboard 12.4) | API 35 (Gboard 14.2, Vulkan) |
|---|---|---|---|
| 1. Launch, draw, tap the box | pass | pass (fixed: GL first on emulators; Vulkan drew black, then garbled glyphs) | pass |
| 2. Typing, backspace | pass | pass | pass |
| 2. Autocorrect | pass (worl → world) | pass (teh → The) | not testable: Gboard corrects nothing, in Messages either |
| 2. Suggestion / candidate pick | pass (Pinyin 你好) | pass (world, backspace reopens it) | pass (Teh → The) |
| 3. English composing region | pass, underlined | pass, underlined | Gboard opens none (Messages alike) |
| 3. Japanese romaji → kana → kanji → commit | pass | pass | pass |
| 3. Korean jamo → syllables | pass | pass | pass |
| 3. Composing left on focus change | **fail** | **fail** | not reproduced (1 try) |
| 4. Tap moves caret; Gboard space-swipe | pass | pass | pass |
| 4. Gboard arrows, home/end, copy/paste | not run | not run | pass |
| 4. Gboard select mode (shift + arrows) | **fail** | **fail** | **fail** |
| 4. Select all + replace via IME | pass | pass | pass |
| 5. Emoji panel inserts colour emoji, backspace | pass | pass | pass |
| 5. Multi-script line (Latin, CJK, Arabic, emoji) | pass | pass (GL only) | pass |
| 6. Rotation with keyboard open | pass (fixed: was a stale frame) | pass (GL) | pass |
| 6. Home + resume, recents switch | pass | pass | pass |
| 7. Email field: email layout | pass (0x21, @ key) | pass | pass |
| 8. Back hides keyboard, then exits | pass | pass | pass |
| Cold start (`am start -W`) | 0.38–0.53 s | 1.3–2.3 s | 0.9–1.0 s |

Every API level: text field white draws as grey (239, 178 on GL, 224 of 255) while Gboard's white is 255 in the
same screenshot. The example has no long-press or double-tap selection; handles are Phase 4, not a failure. No app
crash or ANR in logcat.

## Failures, worst first

1. ~~**API 33 Vulkan.**~~ Fixed: emulators try GL first ([backend.rs](../crates/android/src/backend.rs)); cold start
   and relaunch draw on API 33.
2. **Composing kept after focus moves.** Compose (Korean ㅏ), tap Email: the prose field keeps its underline; on
   refocus the IME gets that range back and the next jamo replaces it (typed text lost). No `finishComposingText`
   reaches GPUI before the new client; see the sync protocol in [android-text-input.md](android-text-input.md).
3. ~~**API 28 rotation.**~~ Fixed: a resize makes a new surface ([android-surface.md](android-surface.md#resize));
   wgpu's GL kept the EGL surface, whose next buffer had the old size, and API 28 drops such a buffer.
4. **Dimmed colours** on every level (above).
5. **Gboard select mode** sends Shift down/up, then plain arrows; a `TextView` keeps Shift as sticky meta state, the
   ported `KeyboardManager` path does not, so the caret moves instead of extending.

## Rerun it

1. `git checkout <branch>`, `scripts/apply-patches.sh`, `cargo install --path crates/cli` ([cli.md](cli.md)).
2. One emulator at a time (memory budget in [AGENTS.md](../AGENTS.md)):
   `emulator -avd <avd> -port 5600 -memory 2048 -cores 2 -no-snapshot-save -gpu host`. AVDs: `parallax_fonts_28`,
   `parallax_gate_33`, `parallax_input_35`. All are `google_apis` images with Gboard preloaded, not Play images.
3. `cd examples/hello && cargo parallax run android --device emulator-5600`.
4. Gboard settings → Languages → add 日本語 (QWERTY) and 한국어 (2-Bulsik); long-press the globe key to switch.
   The API 28 AVD runs in Chinese with Gboard Pinyin.
5. Drive Gboard with `adb shell input tap` on its keys and screenshot with `adb exec-out screencap -p`; rotate with
   `settings put system user_rotation 1|0` (after `accelerometer_rotation 0`).
6. To see IME edits as lengths and ranges, raise the logger to Debug in `on_load` ([host.rs](../crates/android/src/host.rs))
   in a local build only; never commit it, never log typed text.
7. Compare a doubtful case with a native field (Messages, Contacts) on the same AVD before calling it a failure.
