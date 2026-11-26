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

## Apps and the fork

There is no app opt-in (owner decision, [AGENTS.md rule 14](../AGENTS.md#upstream-and-patch-rules)). The fork holds
Zed's crate names (`gpui` 0.2.2, `workspace = true` manifests), so a `[patch]` of `gpui-pre` onto it can't resolve.
Apps get backend patches only when Zed merges them; published Parallax crates build on stock `gpui-pre` (rule 11).
For a local experiment only, an app can `[patch.crates-io]` each `gpui-pre-*` crate onto a Parallax checkout's
`build/gpui-patched/` (after `scripts/apply-patches.sh`), the same rows Parallax's own [`Cargo.toml`](../Cargo.toml)
uses. That is unsupported.
