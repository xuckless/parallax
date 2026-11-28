# Upstream GPUI

How Parallax gets GPUI, changes it, and moves to newer snapshots. The rules behind this are in
[AGENTS.md](../AGENTS.md) ("Decisions" and "Upstream and patch rules").

## upstream/: the pristine snapshot

[`upstream/`](../upstream/) holds the [`gpui-pre-*`](https://crates.io/crates/gpui-pre) crates from crates.io:
weekly snapshots of GPUI from [zed-industries/zed](https://github.com/zed-industries/zed). A few files are adapted
so the crates build standalone; each starts with `// Modified for gpui-pre` (`grep -rl "Modified for gpui-pre" upstream/`).
[`upstream/UPSTREAM`](../upstream/UPSTREAM) records the version, the Zed commit it snapshots, Zed's toolchain and
the crate list.

It is never hand-edited. Every crate keeps its `.cargo-checksum.json`, so any edit is detectable, and every change
we make is a patch that can be filed with Zed. Only the `gpui-pre-*` crates are vendored; third-party deps come from
crates.io.

## build/: what the workspace builds

[`scripts/apply-patches.sh`](../scripts/apply-patches.sh) copies `upstream/` to `build/gpui-patched/` and applies
`patches/*.patch` in sorted order, stopping at the first patch that fails. Patch paths are relative to `upstream/`
(`a/gpui-pre/src/app.rs`). It then runs [`fork-crates.sh`](../scripts/fork-crates.sh), which turns that into our
`parallax-gpui*` fork crates in `build/parallax-gpui/` ([gpui-fork-crates.md](gpui-fork-crates.md)). `build/` is
generated and git-ignored: never edit it, rerun the script.

Parallax crates depend on the fork crates (`gpui = { package = "parallax-gpui", version = "=…" }` in
[`Cargo.toml`](../Cargo.toml)), and its `[patch.crates-io]` block points every one at `build/parallax-gpui/<crate>`,
one row per crate in `UPSTREAM`. On a fresh clone:

```sh
scripts/apply-patches.sh && cargo check --workspace
```

## Syncing to a newer snapshot

Cadence: see the "Sync cadence" decision in [AGENTS.md](../AGENTS.md).

```sh
scripts/sync-upstream.sh          # latest gpui-pre on crates.io
scripts/sync-upstream.sh 0.3.15   # a specific version
scripts/sync-upstream.sh --zed 11e4141   # fallback if gpui-pre stops: vendor Zed's tree (vendor-zed.md)
```

[`scripts/sync-upstream.sh`](../scripts/sync-upstream.sh):

1. `cargo vendor`s a scratch crate on the vendor roots listed in the script (`gpui-pre`, `gpui-pre-platform`,
   `gpui-pre-wgpu`, `gpui-pre-ios`, `gpui-pre-web`, and the optional deps `gpui-pre-bench-metrics`, `gpui-pre-zlog`)
   and replaces `upstream/` wholesale with the `gpui-pre-*` crates. Every `gpui-pre-*` crate any vendored manifest
   names must be vendored, optional ones too, or the fork crates could not be published.
2. Rewrites `upstream/UPSTREAM` (Zed commit from each crate's `[package.metadata.gpui-pre] zed-rev`) and prints
   crates added or removed. On a new version it resets [`patches/FORK_REVISION`](../patches/FORK_REVISION) to 1, then
   repins the `parallax-gpui*` rows in `Cargo.toml` to `=<version>-parallax.<revision>`.
3. Warns if Zed's toolchain differs from [`rust-toolchain.toml`](../rust-toolchain.toml). Bumping it is a manual step.
4. Checks every patch in order and lists **all** that no longer apply, then exits non-zero.
5. Otherwise runs `apply-patches.sh` (fork crates included), regenerates the `[patch.crates-io]` rows from
   `build/parallax-gpui/`, and runs `cargo check --workspace`.

Review the result with `git diff`, then commit the sync as one concern (`upstream/`, `UPSTREAM`, `FORK_REVISION`,
`Cargo.toml`, `Cargo.lock`). To add a platform crate, add a vendor root in the script. Publish the new fork crates
before releasing Parallax on them ([gpui-fork-crates.md](gpui-fork-crates.md#publishing)).

Once it is committed, run [`scripts/sync-fork.sh`](../scripts/sync-fork.sh) to rebuild our Zed fork's `parallax`
branch on the new commit ([zed-fork.md](zed-fork.md)).

## When a patch stops applying

The sync leaves `upstream/` and `Cargo.toml` on the new version and lists the failing patches. For each one:

1. **Merged upstream?** If Zed took the change, delete the patch (AGENTS.md: patches are deleted once upstream
   merges them).
2. **Code moved?** [`scripts/rebase-patches.sh`](../scripts/rebase-patches.sh) applies them in order in
   `build/rebase/`, regenerates fuzzed ones and stops at a conflict for you to fix (header and number kept).
3. **No longer needed or wrong now?** Fix or drop it, and note why in `ROADMAP.md` "Upstream notes".

Patches that apply can still drift: [`scripts/patch-drift.sh`](../scripts/patch-drift.sh) lists, per patch, the files
Zed changed under it; read those diffs. Rerun `scripts/sync-upstream.sh <version>` until it ends with `synced to gpui-pre
<version>`. Lessons from a rehearsal: [upstream-rehearsal.md](upstream-rehearsal.md); a weekly CI job runs these
against Zed main ([upstream-drift.md](upstream-drift.md)). Upstream behaviour that surprised you goes under
"Upstream notes" in [ROADMAP.md](../ROADMAP.md) with the Zed commit.
