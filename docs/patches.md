# Patches

Parallax changes upstream GPUI only through numbered patches in [`patches/`](../patches/). The rules (when a patch
is allowed, size, `cfg` gating, header, filing upstream) are in [AGENTS.md, "Upstream and patch rules"](../AGENTS.md#upstream-and-patch-rules).
This page is the how-to.

## Writing a patch

- **Format:** a git-format diff (`git diff` or `git format-patch`), paths relative to `upstream/`:
  `a/gpui-pre-ios/src/ios.rs`. That is what [`apply-patches.sh`](../scripts/apply-patches.sh) feeds to
  `git apply --directory=build/gpui-patched`.
- **Name:** `NNNN-short-concern.patch`, four digits from `0001`, applied in sorted order. Never renumber a filed patch.
- **Header:** the free text above the first `diff --git` line (`git apply` ignores it). Contents per AGENTS.md patch rule 5.

## Making one from a change in `build/gpui-patched/`

`build/` is regenerated on every run, so edit it only to produce a diff, then move the diff into `patches/`:

```sh
scripts/apply-patches.sh                    # fresh build/gpui-patched/ with all current patches
cd build/gpui-patched && git init -q && git add -A && git commit -qm base
# ...edit files, then from the repo root: scripts/fork-crates.sh && cargo check (the workspace builds build/parallax-gpui/)...
git diff > ../../patches/0004-my-concern.patch   # paths come out as a/gpui-pre-*/..., as required
cd ../.. && rm -rf build/gpui-patched/.git
scripts/apply-patches.sh                    # must apply cleanly on top of the earlier patches
```

Add the header to the top of the file before committing it.

## Our Zed fork

[`scripts/sync-fork.sh`](../scripts/sync-fork.sh) rebuilds the `parallax` branch of
[`xuckless/zed`](https://github.com/xuckless/zed/tree/parallax): the UPSTREAM commit plus every patch, translated as
in [patch-translation.md](patch-translation.md) (Zed's `crates/` layout, [`patch-paths.sh`](../scripts/patch-paths.sh)), one commit each. Run it whenever a patch is added or changed.
Apps don't use that branch; they get every patch through the published `parallax-gpui*` crates. Both are in [zed-fork.md](zed-fork.md).

## Published crates

Every patch reaches crates.io through our fork crates ([AGENTS.md](../AGENTS.md#upstream-and-patch-rules), patch rule 11):
[`apply-patches.sh`](../scripts/apply-patches.sh) ends by running [`fork-crates.sh`](../scripts/fork-crates.sh), which
renames the patched `gpui-pre*` crates to `parallax-gpui*` in `build/parallax-gpui/`, and every Parallax crate depends
on those. Changing a patch between syncs means bumping [`patches/FORK_REVISION`](../patches/FORK_REVISION) and
publishing a new fork version: [gpui-fork-crates.md](gpui-fork-crates.md).

One row per patch: [patch-list.md](patch-list.md).
