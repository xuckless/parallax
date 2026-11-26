# Patches

Parallax changes upstream GPUI only through numbered patches in [`patches/`](../patches/). The rules (when a patch
is allowed, size, `cfg` gating, header, filing upstream) are in [AGENTS.md, "Upstream and patch rules"](../AGENTS.md#upstream-and-patch-rules).
This page is the how-to.

## The patches

| Patch | Concern | Doc |
|---|---|---|
| [0001](../patches/0001-export-priority-queue-android.patch) | Export GPUI's priority queues on Android | [android.md](android.md) |
| [0002](../patches/0002-cosmic-text-system-with-font-system.patch) | `CosmicTextSystem` from a caller-built `FontSystem` | [android-fonts.md](android-fonts.md) |
| [0003](../patches/0003-pointer-source.patch) | `TouchEvent::device`, `Window::pointer_source()` | [core-patches.md](core-patches.md) |
| [0004](../patches/0004-touch-opt-out.patch) | `on_touch` opts an element out of touch → mouse; taps click as `ClickEvent::Touch` | [core-patches.md](core-patches.md) |
| [0005](../patches/0005-text-input-keyboard-type.patch) | Keyboard type, secure entry, autofill hint in `TextInputConfiguration` | [core-patches.md](core-patches.md) |

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

## Translating for filing upstream

Patches are filed against [zed-industries/zed](https://github.com/zed-industries/zed) at the commit in
[`upstream/UPSTREAM`](../upstream/UPSTREAM), where the same crates live under `crates/`.
[`patch-paths.sh`](../scripts/patch-paths.sh) rewrites only the path lines of each file header; hunks are untouched:

```sh
scripts/patch-paths.sh to-zed patches/0004-my-concern.patch > /tmp/0004-zed.patch
git -C ~/src/zed apply --check /tmp/0004-zed.patch     # zed checked out at the UPSTREAM commit
scripts/patch-paths.sh to-vendored /tmp/0004-zed.patch # back again, e.g. for a fix made in Zed's tree
```

It fails loudly (exit 1, nothing usable on stdout) on any path outside the crate table and on files with no faithful
counterpart in Zed's tree:

- **`Cargo.toml`: refused.** crates.io rewrites manifests on publish (workspace inheritance resolved, `gpui-pre-*`
  package names, paths dropped). A vendored manifest hunk has no mechanical Zed equivalent, and a silently wrong
  translation is worse than none. Hand-write the Zed manifest change when filing.
- **Packaging artifacts: refused.** `Cargo.toml.orig`, `Cargo.lock`, `.cargo-checksum.json` exist only in the vendored copy.
- **gpui-pre's own files: refused.** `gpui-pre-apple/vendor/` (a copy of `crates/gpui` sources for its `build.rs`;
  patch `gpui-pre/` instead) and `gpui-pre-macros/src/gpui_pre_facade_paths.rs`.
- **Files gpui-pre rewrote: warning.** The snapshot is not byte-identical to Zed: gpui-pre edits a few sources and
  marks each with a first-line `// Modified for gpui-pre` comment (at 0.3.14: `gpui/src/action.rs`,
  `gpui_apple/build.rs`, `gpui_macros/src/gpui_macros.rs`, three `gpui_wgpu` files). `git apply` absorbs the
  one-line offset, but a hunk touching gpui-pre's edits must be fixed by hand.

Every other file is byte-identical. Zed's tree also has files the package drops (e.g. `crates/gpui_web/examples/`);
patches touching them translate but will not apply to `upstream/`.

## Our Zed fork

[`scripts/sync-fork.sh`](../scripts/sync-fork.sh) rebuilds the `parallax` branch of
[`xuckless/zed`](https://github.com/xuckless/zed/tree/parallax): the UPSTREAM commit plus every patch, translated as
above, one commit each. Run it whenever a patch is added or changed. Apps don't use that branch; they get every patch
through the published `parallax-gpui*` crates. Both are in [zed-fork.md](zed-fork.md).

## The crate table

One row per vendored crate in the `crates` table at the top of [`patch-paths.sh`](../scripts/patch-paths.sh):
`upstream/` dir → Zed dir. Rows were checked against Zed's tree at the UPSTREAM commit. Most are
`gpui-pre-x` → `crates/gpui_x` or `crates/x`; one is nested: `gpui-pre-derive-refineable` →
`crates/refineable/derive_refineable` (the longest matching Zed prefix wins in `to-vendored`).

To extend it, at a sync that vendors a new `gpui-pre-*` crate: read its `zed-crate` from
`[package.metadata.gpui-pre]` in its `Cargo.toml`, find that crate's directory in Zed's tree at the new commit
(`gh api "repos/zed-industries/zed/contents/crates?ref=<commit>"`), and add the row. Add a row to the
`untranslatable` table next to it when a sync finds a new packaging-only or gpui-pre-only file.

## Published crates

Every patch reaches crates.io through our fork crates ([AGENTS.md](../AGENTS.md#upstream-and-patch-rules), patch rule 11):
[`apply-patches.sh`](../scripts/apply-patches.sh) ends by running [`fork-crates.sh`](../scripts/fork-crates.sh), which
renames the patched `gpui-pre*` crates to `parallax-gpui*` in `build/parallax-gpui/`, and every Parallax crate depends
on those. Changing a patch between syncs means bumping [`patches/FORK_REVISION`](../patches/FORK_REVISION) and
publishing a new fork version: [gpui-fork-crates.md](gpui-fork-crates.md).
