# GPUI fork crates

Parallax publishes its patched GPUI to crates.io as `parallax-gpui*` crates, and every Parallax crate depends on them,
so every patch in [`patches/`](../patches/) reaches apps ([AGENTS.md](../AGENTS.md#upstream-and-patch-rules), patch
rule 11). They are built from [`upstream/`](../upstream/) + patches at each sync ([upstream.md](upstream.md)).

## What they are

One fork crate per vendored `gpui-pre*` crate (22 at gpui-pre 0.3.14), named by one rule in
[`fork-crates.sh`](../scripts/fork-crates.sh): `gpui-pre` → `parallax-gpui`, `gpui-pre-<x>` → `parallax-gpui-<x>`.
That is the one product-name exception to patch rule 9: the name says these are Parallax's build of GPUI.
Each keeps upstream's code, `[lib] name` (code still says `gpui`, `gpui_platform`, …), Apache-2.0 licence, licence
file and metadata. Changed: name, version, deps on each other, description, repository, and a README listing the
upstream version, Zed commit and applied patches, crediting Zed. [`NOTICE`](../NOTICE) credits Zed too. A crate our
patches port Flutter code into (`flutter_crates` in the script: `parallax-gpui`, `parallax-gpui-ios`, `parallax-gpui-macos`, `parallax-gpui-web`) is `Apache-2.0 AND BSD-3-Clause`,
ships Flutter's licence as `LICENSE-FLUTTER` and credits Flutter in its README.

## Versioning

`<gpui-pre version>-parallax.<revision>`, e.g. `0.3.14-parallax.1`: one rule, in
[`fork-version.sh`](../scripts/fork-version.sh) (`scripts/fork-version.sh` prints it); the revision is in
[`patches/FORK_REVISION`](../patches/FORK_REVISION). Parallax pins it exactly (`version = "=0.3.14-parallax.1"`): an
`=` requirement matches a pre-release, which a `^` one would not. crates.io lists these as pre-releases; build
metadata (`+parallax.1`) can't be used, crates.io rejects versions that differ only in it.

- **New upstream version:** [`sync-upstream.sh`](../scripts/sync-upstream.sh) runs `fork-version.sh reset` (revision 1).
- **Patches changed between syncs:** `scripts/fork-version.sh bump` raises the revision and rewrites the three
  `parallax-gpui*` pins in [`Cargo.toml`](../Cargo.toml). `fork-crates.sh` refuses to run while pins and revision disagree.
- **Content drift:** `FORK_REVISION` also records a sha256 of `upstream/UPSTREAM` + the sorted patches. Change either
  without a bump and `fork-crates.sh` fails, so a published version never gets new content. `bump` records the hash.
- **A patch upstream fixed:** delete it at the next sync ([upstream.md](upstream.md#when-a-patch-stops-applying)).

## Generate

[`apply-patches.sh`](../scripts/apply-patches.sh) runs `fork-crates.sh` last, so a fresh clone stays one command. It
is a separate script because a patch session edits `build/gpui-patched/` and regenerates only the fork crates
([patches.md](patches.md#making-one-from-a-change-in-buildgpui-patched)). Output: `build/parallax-gpui/<crate>/` and a
workspace manifest of all of them. Deps between fork crates carry the exact version and a sibling `path` (cargo drops
the path on publish). It fails loudly on a stale pin, a leftover `gpui-pre` name, a missing licence file, or a fork
crate some manifest names that is not vendored (optional deps included; add a vendor root in `sync-upstream.sh`).

## Publishing

[`publish-fork.sh`](../scripts/publish-fork.sh); compiled output goes to `target/parallax-gpui/`.

```sh
scripts/apply-patches.sh && scripts/publish-fork.sh --dry-run   # package and verify all, publish nothing
scripts/apply-patches.sh && scripts/publish-fork.sh             # real publish; needs `cargo login`
scripts/publish-fork.sh --published                             # 0 all on crates.io, 1 not yet, 2 error
```

- `--dry-run` is `cargo package --workspace`: each crate is verified against its just-packaged siblings.
- The real publish goes in topological order computed from `cargo metadata` (`tsort`), skips any version already
  on the [sparse index](https://index.crates.io), and on HTTP 429 sleeps until crates.io's "try again after" time,
  then retries. crates.io lets few **new** crate names through at once, then about one per 10 minutes: the first
  publish of all 22 names takes about 3 hours. Later versions of existing names go fast.

## CI

[ci.yml](../.github/workflows/ci.yml) ([ci.md](ci.md)) checks both halves:

- **Fork crates package:** `publish-fork.sh --dry-run` on Linux. Every job also builds the Parallax workspace on the
  same crates through the `[patch.crates-io]` rows.
- **Parallax crates package:** `cargo package` builds each tarball without the workspace `[patch]`, so it resolves
  `parallax-gpui*` from crates.io. It runs when `--published` exits 0; exit 1 (not yet) warns on pull requests and
  fails on `main` and `v*` tags; exit 2 (error) always fails. The list is `PUBLISHED_LIBS` in `ci.yml`.

So publish the fork crates first, then land the pin bump or Parallax release that needs them.
