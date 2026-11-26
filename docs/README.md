# Parallax docs

How Parallax's code works. Rules live in [AGENTS.md](../AGENTS.md); the plan lives in [ROADMAP.md](../ROADMAP.md).

| Doc | Covers |
|---|---|
| [flutter.md](flutter.md) | Recording Flutter ports in the ledger, port vs oracle, bumping the pinned tag with flutter-diff.sh |
| [upstream.md](upstream.md) | upstream/ snapshot, apply-patches.sh, sync-upstream.sh, fixing patches that stop applying |
| [patches.md](patches.md) | Writing, numbering and making patches; translating them to Zed's layout with patch-paths.sh; the crate table |
| [platform.md](platform.md) | Target → platform table, how an app gets its GPUI platform, adding a target, errors on unsupported targets |
| [audits/gap-audit.md](audits/gap-audit.md) | What upstream GPUI lacks for touch, text input, lifecycle, insets and accessibility per platform, with source links and the phase that fixes each |
| [ci.md](ci.md) | The six CI jobs, what each checks, running the checks locally, adding a target |
| [audits/android-fonts.md](audits/android-fonts.md) | Android font audit: cosmic-text/fontdb gaps, fonts.xml parsing, CJK and emoji fallback (COLRv1 vs CBDT), sizing and proposed Phase 1 tasks |
| [audits/oracle-inventory.md](audits/oracle-inventory.md) | Flutter 3.50.1 test files per area (gestures, text input, accessibility), the upstream GPUI code each checks, portability verdicts, Android/iOS test-infra risks, Phase 2/4 order |
| [audits/gap-audit.md](audits/gap-audit.md) | What upstream GPUI lacks for touch, text input, lifecycle, insets and accessibility per platform, with source links and the phase that fixes each |
