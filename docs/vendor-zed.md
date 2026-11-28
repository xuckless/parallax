# Vendoring Zed without gpui-pre

The fallback for ROADMAP's open question "if gpui-pre snapshots stop": build `upstream/` from
[zed-industries/zed](https://github.com/zed-industries/zed)'s own tree, byte for byte the way gpui-pre would.
Normal syncs stay on crates.io ([upstream.md](upstream.md)).

```sh
scripts/sync-upstream.sh --zed 11e4141                  # version: after the last gpui-pre, e.g. 0.3.15-zed.11e4141
scripts/sync-upstream.sh --zed 11e4141 0.4.0-zed.1      # or name one
```

`--zed` replaces only the vendoring step: [`sync-upstream.sh`](../scripts/sync-upstream.sh) fetches Zed at the commit
(shallow, full checkout, `build/zed-vendor/`), runs [`vendor-zed.py`](../scripts/vendor-zed.py) into its scratch
`vendor/`, then does what it always does (UPSTREAM, fork revision, patch check, fork crates, `[patch]` rows, check).

## What vendor-zed.py does

A Python port of the staging step of gpui-kit's
[`script/bump-gpui.ts`](https://github.com/longbridge/gpui-kit/blob/85c05bf/script/bump-gpui.ts) (Apache-2.0, ledger row and
licence in [third_party/gpui-kit/](../third_party/gpui-kit/PORTED.md),
the commit that cut gpui-pre 0.3.14), the script that publishes gpui-pre:

1. Reads the crates in [`patch-paths.sh`](../scripts/patch-paths.sh)'s crate table from Zed's tree (one list of
   vendored crates) and fails if one needs a Zed crate that is not in it.
2. Swaps the git deps crates.io can't take (`OVERRIDES`: wgpu → 30.0.1, proptest → 1), drops optional git deps
   without a version with the features that enable them, relaxes exact external pins.
3. Writes a standalone workspace: each crate renamed `gpui-pre-*` at the version, `[lib]` keeping Zed's name,
   dev-deps dropped, `[package.metadata.gpui-pre]` recording the Zed crate, version and commit, Zed's `Cargo.lock`.
4. Reapplies gpui-pre's source adaptations (`ADAPTATIONS`), each marked `// Modified for gpui-pre`: proc-macro entry
   points through a facade, `actions!` crate-relative, gpui_apple's shader sources under `vendor/gpui`, wgpu 30.0.1
   API renames.
5. `cargo package --workspace --no-verify`, unpacks the `.crate` files into `OUT_DIR`, writes `.cargo-checksum.json`.

Verified: run on Zed `f126950` at version 0.3.14 it reproduces gpui-pre 0.3.14 exactly (every source file and
normalized `Cargo.toml`; only `Cargo.toml.orig` layout and the packaged `Cargo.lock` checksums differ).

## When it fails

Every adaptation and rule fails loudly when Zed changes what it relies on (a renamed `../gpui` path, a new git dep, a
proc-macro signature). Fix the rule in `vendor-zed.py`, and compare with gpui-kit's current script, which may already
handle it (it moved to AccessKit 0.25 on 2026-11-25; we keep Zed's). A new Zed crate in the closure needs a row in
`patch-paths.sh`. Rehearsal and lessons: [upstream-rehearsal.md](upstream-rehearsal.md).
