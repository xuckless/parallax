# Our Zed fork

[`xuckless/zed`](https://github.com/xuckless/zed) carries Parallax's patches in Zed's own layout until Zed replies
([AGENTS.md](../AGENTS.md#upstream-and-patch-rules), patch rule 14). Its
[`parallax`](https://github.com/xuckless/zed/tree/parallax) branch is the Zed commit in
[`upstream/UPSTREAM`](../upstream/UPSTREAM) plus one commit per patch in [`patches/`](../patches/), translated with
[`patch-paths.sh`](../scripts/patch-paths.sh) ([patches.md](patches.md#translating-for-filing-upstream)).
Upstream PRs are cut from it; drafts of them live in [`upstream-drafts/`](upstream-drafts/).

## Rebuilding the branch: sync-fork.sh

Run [`scripts/sync-fork.sh`](../scripts/sync-fork.sh) after every upstream sync ([upstream.md](upstream.md#syncing-to-a-newer-snapshot))
and after adding or changing a patch:

```sh
scripts/sync-fork.sh              # scratch checkout in build/zed-fork/ (git-ignored, reused)
scripts/sync-fork.sh /path/to/dir # or anywhere else
```

It fetches only the base commit (shallow, blobless) into the scratch checkout, sparse-checks-out `crates/gpui*` and
the files the patches touch, recreates `parallax` at the base, then per patch runs `git apply --check`, applies it
and commits it: subject = the patch's first header line, body = the rest of its header. Last, it force-pushes
**only** `refs/heads/parallax` to the fork. Commit dates are pinned to the base commit's, so rerunning on the same
inputs rebuilds the same SHAs and the push is a no-op.

It stops (exit 1) on a patch that does not translate or apply, or a failed fetch or push. Fix the patch in
`patches/` (see [upstream.md](upstream.md#when-a-patch-stops-applying)) and rerun; never commit to the fork by hand,
since the next run throws hand-made commits away. Pushing needs `gh auth login` as the fork's owner.

## Using the fork's patches in an app

**A `[patch]` onto the fork does not work.** Apps and Parallax's crates depend on the crates.io names (`gpui-pre`,
`gpui-pre-platform`, ...), version 0.3.x. The fork holds Zed's crates: `gpui` 0.2.2, `gpui_platform`, ... with
`workspace = true` manifests. Cargo matches a patch by package name, so

```toml
[patch.crates-io]
gpui-pre = { git = "https://github.com/xuckless/zed", branch = "parallax" }
```

fails with "patch location ... does not contain packages matching `gpui-pre`", and adding `package = "gpui"`
patches crates.io's `gpui` instead ("patch ... was not used in the crate graph"). Checked with cargo 1.98.1.

**What works today:** patch onto a Parallax checkout's `build/gpui-patched/`, which keeps the crates.io names. It is
the same block Parallax's own [`Cargo.toml`](../Cargo.toml) uses, one row per crate in `UPSTREAM`:

```sh
git clone https://github.com/xuckless/parallax && parallax/scripts/apply-patches.sh
```

```toml
[patch.crates-io]
gpui-pre          = { path = "../parallax/build/gpui-patched/gpui-pre" }
gpui-pre-platform = { path = "../parallax/build/gpui-patched/gpui-pre-platform" }
# ...one row for every gpui-pre-* crate in upstream/UPSTREAM
```

Rerun `apply-patches.sh` after pulling Parallax. The app's `gpui-pre` version must equal the one in `UPSTREAM`.
