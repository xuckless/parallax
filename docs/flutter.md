# Flutter ports and the pinned tag

Parallax ports Flutter's platform glue and uses Flutter's tests as an oracle for upstream GPUI.
Why, and the rules for ported code, are in
[AGENTS.md: Flutter is the second source of truth](../AGENTS.md#flutter-is-the-second-source-of-truth).
Flutter source: [github.com/flutter/flutter](https://github.com/flutter/flutter).

## Port vs oracle

- **Port:** Flutter code copied into Parallax (Java kept as Java, Dart translated to Rust). Flutter fixes must be carried over.
- **Oracle:** Flutter tests or behaviour that Parallax checks upstream GPUI against. Nothing is copied but the test cases.

Both kinds get a ledger row, so both are re-checked when the pinned tag moves.

## Recording a port

1. Put the header from rule 3 on the ported file.
2. Add one row to the ledger in [PORTED.md](../third_party/flutter/PORTED.md): Flutter path, the tag it came from,
   Parallax path, kind, and what was dropped (rule 10).
3. Check that [NOTICE](../NOTICE) and the crate's `license` field still hold (rule 5).
   Flutter's license text is in [third_party/flutter/LICENSE](../third_party/flutter/LICENSE).

The ledger table's header line must stay exactly as it is: [flutter-diff.sh](../scripts/flutter-diff.sh) finds the
table by it. Each row has five cells, `|`-separated; the path and tag cells may be wrapped in backticks.
The tag is the Flutter stable tag the file was ported from, e.g. `3.50.1`.

## Bumping the pinned tag

Track stable tags only (plain `X.Y.Z`; betas end in `-N.M.pre`). Needs `gh` logged in (`gh auth login`).

1. Run `scripts/flutter-diff.sh` to diff every ledger row against the newest stable tag, or
   `scripts/flutter-diff.sh 3.51.0` for a specific one.
2. Read each file's section:
   - `unchanged`: nothing to do.
   - a unified diff: port the fix into the Parallax file (keep Flutter's names, rule 2). For an oracle row,
     add or update the test-table rows (rule 6).
   - `missing at <tag>`: the file moved or was deleted. Find where it went in Flutter's history and update the row's path.
3. Update the ledger rows' tag column to the new tag, and the pinned tag line at the top of PORTED.md.
4. Run the workspace checks from [AGENTS.md: Workflow](../AGENTS.md#workflow) and commit one concern per commit.

The script fetches each file at both tags through `gh api`. It caches nothing and writes nothing to the repo.
It stops with an error on a malformed row, a tag that is not a stable Flutter tag, or a file missing at its own row's tag.

GitHub's "latest release" for flutter/flutter is a 2024 beta, not the newest stable, so the script picks the
highest `X.Y.Z` tag instead.
