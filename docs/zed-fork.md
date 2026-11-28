# Our Zed fork

[`xuckless/zed`](https://github.com/xuckless/zed) carries Parallax's patches in Zed's own layout until Zed replies
([AGENTS.md](../AGENTS.md#upstream-and-patch-rules), patch rule 14). Its
[`parallax`](https://github.com/xuckless/zed/tree/parallax) branch is the Zed commit in
[`upstream/UPSTREAM`](../upstream/UPSTREAM) plus one commit per patch in [`patches/`](../patches/), translated with
[`patch-paths.sh`](../scripts/patch-paths.sh) ([patches.md](patches.md#translating-for-filing-upstream)); a patch with
a manifest hunk gets its hand-written Zed manifest change from `patches/zed/<name>.manifest.patch` appended
([patch-translation.md](patch-translation.md)).
Upstream PRs are cut from it; drafts of them live in `drafts/upstream/` (git-ignored, owner-only).

## Rebuilding the branch: sync-fork.sh

Run [`scripts/sync-fork.sh`](../scripts/sync-fork.sh) after every upstream sync ([upstream.md](upstream.md#syncing-to-a-newer-snapshot))
and after adding or changing a patch:

```sh
scripts/sync-fork.sh              # scratch checkout in build/zed-fork/ (git-ignored, reused)
scripts/sync-fork.sh /path/to/dir # or anywhere else
scripts/sync-fork.sh --no-push    # build the branch, push nothing: checks every patch against Zed's tree
```

It fetches only the base commit (shallow, blobless) into the scratch checkout, sparse-checks-out `crates/gpui*` and
the files the patches touch, recreates `parallax` at the base, then per patch runs `git apply --check`, applies it
and commits it: subject = the patch's first header line, body = the rest of its header. Last, it force-pushes
**only** `refs/heads/parallax` to the fork. Commit dates are pinned to the base commit's, so rerunning on the same
inputs rebuilds the same SHAs and the push is a no-op.

It stops (exit 1) on a patch that does not translate or apply, a patch changing a `Cargo.toml` with no
`patches/zed/<name>.manifest.patch`, or a failed fetch or push. Fix the patch in
`patches/` (see [upstream.md](upstream.md#when-a-patch-stops-applying)) and rerun; never commit to the fork by hand,
since the next run throws hand-made commits away. Pushing needs `gh auth login` as the fork's owner.

## The fork crates vs this branch

Two things carry the same patches. This branch is Zed's layout and crate names (`gpui` 0.2.2, `workspace = true`
manifests): it exists so upstream PRs can be cut from it, and apps can't `[patch]` onto it. Apps get the patches
from the published [`parallax-gpui*` fork crates](gpui-fork-crates.md) instead, built from `upstream/` + `patches/`
in crates.io's `gpui-pre` layout (rule 11). Both are rebuilt at every sync and whenever a patch changes.
