# Patch tests

Every core patch ships table-driven tests inside GPUI's own sources ([core-patches.md](core-patches.md#tests),
0006's in `window.rs`). They run as part of GPUI's lib tests on the patched fork crate `parallax-gpui`, which
`cargo test --workspace` never builds: Parallax's workspace only depends on it. They run on the host, so the iOS
and web parts of a patch are only compile-checked, by those targets' builds in [ci.yml](../.github/workflows/ci.yml).

## Run them

```sh
scripts/apply-patches.sh
scripts/patch-test.sh                  # all of parallax-gpui's lib tests
scripts/patch-test.sh pointer_source   # libtest filters and flags pass through
```

It needs GNU sed (`sed -i`; on macOS, `gsed` as `sed`).

[`patch-test.sh`](../scripts/patch-test.sh) copies [`build/parallax-gpui/`](gpui-fork-crates.md#generate) to
`build/patch-tests/` (generated, git-ignored, rebuilt every run) and runs
`cargo test -p parallax-gpui --features test-support --lib` there, compiled into `target/patch-tests/`. It never
changes `build/parallax-gpui/`, so what it tests is what [`publish-fork.sh`](../scripts/publish-fork.sh) packages,
plus two test-only changes the crate needs because it can't run its own tests as shipped:

| Shipped crate lacks | The scratch copy gets |
|---|---|
| `assets/fonts/…` that `svg_renderer.rs` tests `include_bytes!` from Zed's repo | the first two `.ttf` files under `/usr/share/fonts`, as stand-ins (Ubuntu: `fonts-dejavu-core`) |
| `util_macros` (Zed's `#[perf]` test attribute in `style.rs`) | `#[test]` in place of `#[perf]`, its `use` dropped; the script fails if neither is there any more |

## Skipped tests

The `skips` array in the script lists tests by full name, each with its reason: today the four `svg_renderer`
tests that look IBM Plex Sans and Lilex up by family name, which the stand-ins don't have. Every other test must
pass. Add a row only for a test this setup can't run, never for one a patch broke; fix the patch instead.
libtest's `--skip` matches substrings, so the script lists the tests first (`--list`) and fails unless each row
names exactly one test; your own filters still pass straight to libtest.

## gpui_windows's pure modules

gpui_windows compiles only on Windows, and CI's Windows row never builds a dependency's tests. Its pure decision
modules (plain values, no `windows` crate) therefore run here: after GPUI's tests, the script adds the scratch crate
`windows-pure` to the scratch workspace, with each module in its `windows_pure` row included by `#[path]` on
`parallax-gpui`, and runs its lib tests. Today that is `pointer.rs` ([desktop-touch-windows.md](desktop-touch-windows.md)).
A module listed there must use only `gpui` and `std`.

## CI

The `patch-tests` job in [ci.yml](../.github/workflows/ci.yml) ([ci.md](ci.md)) runs the script on Linux, separate
from the per-target build matrix.
