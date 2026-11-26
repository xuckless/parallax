# Patch tests

Every core patch ships table-driven tests inside GPUI's own sources ([core-patches.md](core-patches.md#tests),
0006's in `window.rs`). They run as part of GPUI's lib tests on the patched fork crate `parallax-gpui`, which
`cargo test --workspace` never builds: Parallax's workspace only depends on it.

## Run them

```sh
scripts/apply-patches.sh
scripts/patch-test.sh                  # all of parallax-gpui's lib tests
scripts/patch-test.sh pointer_source   # libtest filters and flags pass through
```

[`patch-test.sh`](../scripts/patch-test.sh) copies [`build/parallax-gpui/`](gpui-fork-crates.md#generate) to
`build/patch-tests/` (generated, git-ignored, rebuilt every run) and runs
`cargo test -p parallax-gpui --features test-support --lib` there, compiled into `target/patch-tests/`. It never
changes `build/parallax-gpui/`, so what it tests is what [`publish-fork.sh`](../scripts/publish-fork.sh) packages,
plus two test-only changes the crate needs because it can't run its own tests as shipped:

| Shipped crate lacks | The scratch copy gets |
|---|---|
| `assets/fonts/…` that `svg_renderer.rs` tests `include_bytes!` from Zed's repo | the first two `.ttf` files under `/usr/share/fonts`, as stand-ins (Ubuntu: `fonts-dejavu-core`) |
| `util_macros` (Zed's `#[perf]` test attribute in `style.rs`) | the `#[perf]` lines and their `use` dropped |

## Skipped tests

The `skips` array in the script lists tests by full name, each with its reason: today the three `svg_renderer`
tests that look IBM Plex Sans and Lilex up by family name, which the stand-ins don't have. Every other test must
pass. Add a row only for a test this setup can't run, never for one a patch broke; fix the patch instead.

## CI

The `patch-tests` job in [ci.yml](../.github/workflows/ci.yml) ([ci.md](ci.md)) runs the script on Linux, separate
from the per-target build matrix.
