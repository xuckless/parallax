# Gate 4 check

Gate 4 ([ROADMAP.md](../ROADMAP.md), Phase 4): the ported Flutter text-input tables pass on every target, iOS on the
simulator (owner decision, 2026-11-27). Checked 2026-11-27 on merged `wave-14` (fork `0.3.14-parallax.11`, patches
0001–0046), after the Select All fix below.

**Gate 4: PASS.** Every ported table passes on every target it runs on; the owner's device checks below are owed,
not part of the gate.

## Ported tables

| Table (Flutter source) | Runs on | Where | Result |
|---|---|---|---|
| iOS `UITextInput` oracle (`FlutterTextInputPluginTest.mm`, patch 0028) | iOS simulator (iPad Pro 11-inch, iOS 26.5) | [ios-text-oracle.sh](../scripts/ios-text-oracle.sh) on pixl-mac | 47/47 (45 Flutter, 2 Parallax), plus the protocol smoke test |
| iOS keyboard, inset, spring (`FlutterKeyboardManager`, keyboard inset tests; `ios-pure`) | host | [patch-test.sh](../scripts/patch-test.sh) | 10/10 |
| Web `text_editing.dart` rules, page lifecycle, keyboard inset (`web-pure`) | host | patch-test.sh | 8/8 |
| Web IME mirror in a browser (patches 0034, 0035, 0046) | Chromium (headless) desktop / mobile; Firefox desktop / mobile | [web-checks.mjs](../scripts/web-checks.mjs), fresh browsers | 17/17, 15/15; 13 + 4 skipped, 12 + 3 skipped (Playwright can't compose, hide or freeze) |
| Android embedding (Robolectric: `InputConnectionAdaptorTest` 26, `TextInputPluginTest` 98, `KeyboardManagerTest` 25, `ListenableEditingStateTest` 13, `TextEditingDeltaTest` 2, `AndroidTouchProcessorTest` 23, `PlatformPluginTest` 16, others 2) | JVM | [java-test.sh](../scripts/java-test.sh) | 205/205 |
| Text selection engine (`text_selection_test.dart`, `text_field_test.dart`, `editable_text_test.dart`, magnifier) | host, every OS in CI | `cargo test -p parallax-ext` ([text-selection-tests.md](text-selection-tests.md)) | 57 gesture rows and the geometry, toolbar and magnifier tables pass |
| Behaviour table's `text_selection` column (Android, iOS, macOS, Windows, Linux; the web follows its OS) | host | `cargo test -p parallax-platform` | 5/5 rows |
| GPUI with every patch (gesture, touch-drag `tap_and_drag_test.dart` rows, recognizers) | host | patch-test.sh | 517 + 61 (gpui_linux) + 9 (`windows-pure`) |

macOS, Windows and Linux run the engine and behaviour tables in CI's `macos`, `windows` and `linux` rows (`cargo
test --workspace`); macOS also ran here on pixl-mac (clippy and tests clean).

## Selection checks

| Target | Check | Result |
|---|---|---|
| Android emulator (API 35) | hello: long press on a word, tap, double tap, finger drag (logged offsets) | word 4..7, caret 1..1, word 4..7, caret dragged to 8..8 |
| Android emulator (API 35, API 28) | [android-smoke.sh](../scripts/android-smoke.sh) | 9/9 on both (API 28 on a settled emulator; straight after a cold boot its first rotation came over 20 s late, a boot-load effect, not the app) |
| Linux (Weston headless) | [weston-selection-checks.sh](../scripts/weston-selection-checks.sh), CI `linux-touch` | see the CI run for this commit |
| iOS simulator | hello on iPad Pro 11-inch: double tap, handles, Select All, Cut, Paste via UIKit's menu | pass; Select All now selects (below) |

**Select All fix.** UIKit's keyboard collapses the selection it knows when it hears `selectionWillChange:`. The edit
actions told the input delegate only afterwards (deferred), so Select All's new selection collapsed to a caret at the
end. Patch 0036 now sends will before and did after each action; a Parallax oracle row checks the order.

## Owed by the owner (not part of the gate)

- iPad: `/Users/Shared/parallax/w14-ipad/CHECK.md` (rebuilt from this merge): hardware keyboard chords, Caps Lock, key
  repeat, floating and split keyboards, selection, autofill, Safari.
- Mac WebKit / Safari: `… desktop webkit` in a GUI session (`MAC-WEBKIT.md` there).
- macOS Sidecar touch re-check after Wave 14's patches ([desktop-touch-macos.md](desktop-touch-macos.md)).
- macOS Japanese IME in hello (candidate window at the caret, commit, focus change).
