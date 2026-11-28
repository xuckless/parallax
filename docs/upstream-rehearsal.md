# Upstream rehearsal: Zed main through the fallback

On 2026-11-27 the owner asked for a trial sync, to see whether a future rebase would go smoothly. gpui-pre had no
release after 0.3.14 (Zed `f126950`, 2026-11-22), so it ran the fallback ([vendor-zed.md](vendor-zed.md)) to Zed main
`11e4141` (2026-11-27) on branch `rehearsal-upstream-zed-main`, never merged. Five days of Zed: 48 files in the
vendored crates, +5472 −1183, and Rust 1.98.1 → 1.100.0 (followed).

## Numbers

| Result | Patches |
|---|---|
| Clean | 15: 0001 0002 0007 0011 0012 0015 0024 0025 0027 0029 0030 0033 0036 0040 0043 |
| Offset only (git apply takes them) | 22: 0003–0006 0009 0010 0013 0017–0023 0031 0034 0037–0039 0044–0046 |
| Fuzz (regenerated, same +/- lines) | 5: 0008 0014 0016 (gpui window.rs, app.rs, Linux clients), 0028 0032 (manifest context) |
| Conflict (fixed by hand) | 4: 0026, 0035, 0041, 0042 |
| Dropped | 0: upstream fixed nothing we patch |

The four conflicts took about 5 minutes; the whole rehearsal, including writing `vendor-zed.py`, about 40 minutes.
Every local check passed afterwards: patch-test (557 tests), host fmt, clippy and tests, wasm32 check and clippy,
Android clippy, Java tests, android-smoke on API 35, macOS clippy, iOS and iOS-sim check, and the iOS text oracle.
Only wasm32 clippy `--all-targets` fails (a dev-dependency's getrandom), which CI does not run.
CI (run 46098310779, dispatched) passed all 11 jobs. `sync-fork.sh --no-push` applied all 46 patches and 7 manifest
patches to Zed main.

## Conflicts and drift

- **0026** (macOS touch recognizer): Zed reworked the display link (`MetalFrames` import gone, window-state init
  re-indented). Code moved; two hooks re-placed.
- **0035** (web keyboard inset): Zed now holds the canvas at its keyboard-free height while a keyboard may be up and
  added `WebViewport::keyboard_height`. Our inset formula is unchanged, but it now sees the keyboard on Chrome for iOS
  too. Zed's `keyboard_height` overlaps our Flutter port: decide at the real sync whether to share it.
- **0041** (Linux reduce motion): Zed added a GNOME text-scale reader in the same portal spots. Neighbour added.
- **0042** (Windows accessibility settings): Zed took `WM_USER + 11` for a new message. A fuzzy apply would compile
  and send our message to Zed's handler. The rehearsal moved ours to `+ 12`; main has since moved it to `WM_APP + 1`, out of Zed's range
  (AGENTS patch rule 4). This conflict was about meaning, not text.
- **0046**, applied with offsets but drifted: Zed's new iOS refocus rule read the user agent again. Fixed so it reads
  the OS value stored once.
- **0028**, applied but broke a build: its manifest hunk pins `=0.3.14`. `fork-crates.sh` rewrote only the current
  version's pins, so patch-test.sh could not resolve. Any gpui-pre bump would hit this. Fixed in `fork-crates.sh`.
- Checked and fine: 0009 (Zed's new text-layout cache keys on the font generation that `invalidate_fonts` bumps);
  0017 and 0045 alongside Zed's new keyed layout cache; macOS early draw after key input, which never runs for touch
  (a latency idea, not drift).

## Gaps re-checked ("Parallax adds", AGENTS.md)

Zed added no desktop touch, no Android, no reduce-motion readers and no selection, and did not touch `gpui_ios`.
New: `Platform::text_scale_factor` and `on_text_scale_factor_change` (Linux only), an OS display setting that
Android, iOS, macOS, Windows and the web could fill. Zed is also active in web keyboard handling, our 0034/0035/0046
area.

## Script gaps found and fixed

- No fallback: added `vendor-zed.py` and `sync-upstream.sh --zed`.
- `sync-upstream.sh` checks each patch alone: it listed 24 failing, but 15 of those were dependants of failures.
  Added `rebase-patches.sh` (in order, classified, fuzz regenerated). `patch-drift.sh` lists the files Zed changed
  under each patch. `sync-fork.sh --no-push` is the dry fork run.
- `fork-crates.sh` exact-pin rule (above). The UPSTREAM crate table overflowed with long versions.
- The Mac scripts named 1.98.1. They now read `rust-toolchain.toml`. The Mac's shared rustup still has only 1.98.1:
  install 1.100.0 there before the real sync (this run used its own rustup home).

## Recommendations

1. **The fallback works.** Keep `vendor-zed.py` working, and compare it with gpui-kit's script at each sync. Risks:
   Zed's git wgpu against crates.io 30.0.1 (the rule fails loudly), gpui-kit choices we don't share (AccessKit
   0.25), and the version line becomes ours.
2. **Add a weekly CI job (report only)**: `sync-upstream.sh --zed main`, `rebase-patches.sh`, `sync-fork.sh
   --no-push` and `patch-drift.sh`, so drift shows up while it is small. Five days cost 9 regenerated patches.
3. **Keep no hand-made "touches" list.** `patch-drift.sh` derives it, so it cannot go stale.
4. **Keep numbering out of Zed's way.** Parallax's Windows messages should use their own range (e.g. `WM_APP + n`),
   and any other shared numbering should do the same.
5. **Use new files plus small hooks.** 0026's own file applied clean; only its two hooks in `window.rs` conflicted.
   `gpui/src/window.rs` takes offsets in 13 patches: keep logic out of it.
6. **Line up with Zed's shapes.** Give 0039's OS display hook Zed's getter plus change-callback form (as
   `text_scale_factor`), and consider reading text scale on our targets.
