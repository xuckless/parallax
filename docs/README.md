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
