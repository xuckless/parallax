# Patch translation

How a patch written against the vendored `gpui-pre` layout becomes one for Zed's tree, and back
([AGENTS.md](../AGENTS.md#upstream-and-patch-rules), patch rule 10). Writing patches: [patches.md](patches.md).

## Translating

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

- **`Cargo.toml`: refused, or dropped.** crates.io rewrites manifests on publish (workspace inheritance resolved,
  `gpui-pre-*` package names, paths dropped). A vendored manifest hunk has no mechanical Zed equivalent, and a
  silently wrong translation is worse than none. `to-zed --drop-manifests` leaves every `*/Cargo.toml` section out
  instead; the Zed manifest change is hand-written once, as `patches/zed/<name>.manifest.patch` (only the Zed
  manifest's diff, e.g. 0018's UIKit features in Zed's workspace `Cargo.toml`).
  [`sync-fork.sh`](../scripts/sync-fork.sh) translates every patch with `--drop-manifests` and appends that file; it
  stops when a patch changes a `Cargo.toml` and has none. Change it only when the patch's manifest hunk changes.
  When Zed's manifest already has the change (0053: dev-dependencies the gpui-pre package dropped), the file holds
  only its header and the patch's Zed commit is empty.
- **Packaging artifacts: refused.** `Cargo.toml.orig`, `Cargo.lock`, `.cargo-checksum.json` exist only in the vendored copy.
- **gpui-pre's own files: refused.** `gpui-pre-apple/vendor/` (a copy of `crates/gpui` sources for its `build.rs`;
  patch `gpui-pre/` instead) and `gpui-pre-macros/src/gpui_pre_facade_paths.rs`.
- **Files gpui-pre rewrote: warning.** The snapshot is not byte-identical to Zed: gpui-pre edits a few sources and
  marks each with a first-line `// Modified for gpui-pre` comment (at 0.3.14: `gpui/src/action.rs`,
  `gpui_apple/build.rs`, `gpui_macros/src/gpui_macros.rs`, three `gpui_wgpu` files). `git apply` absorbs the
  one-line offset, but a hunk touching gpui-pre's edits must be fixed by hand.

Every other file is byte-identical. Zed's tree also has files the package drops (e.g. `crates/gpui_web/examples/`);
patches touching them translate but will not apply to `upstream/`.

## The crate table

One row per vendored crate in the `crates` table at the top of [`patch-paths.sh`](../scripts/patch-paths.sh):
`upstream/` dir → Zed dir. Rows were checked against Zed's tree at the UPSTREAM commit. Most are
`gpui-pre-x` → `crates/gpui_x` or `crates/x`; one is nested: `gpui-pre-derive-refineable` →
`crates/refineable/derive_refineable` (the longest matching Zed prefix wins in `to-vendored`).

To extend it, at a sync that vendors a new `gpui-pre-*` crate: read its `zed-crate` from
`[package.metadata.gpui-pre]` in its `Cargo.toml`, find that crate's directory in Zed's tree at the new commit
(`gh api "repos/zed-industries/zed/contents/crates?ref=<commit>"`), and add the row. Add a row to the
`untranslatable` table next to it when a sync finds a new packaging-only or gpui-pre-only file.
