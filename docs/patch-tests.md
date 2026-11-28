# Patch tests

Every core patch ships table-driven tests inside GPUI's own sources ([core-patches.md](core-patches.md#tests),
0006's in `window.rs`). They run as part of GPUI's lib tests on the patched fork crate `parallax-gpui`, which
`cargo test --workspace` never builds: Parallax's workspace only depends on it. They run on the host, so the iOS
and web parts of a patch are only compile-checked, by those targets' builds in [ci.yml](../.github/workflows/ci.yml).

## Run them

```sh
scripts/apply-patches.sh
scripts/patch-test.sh                  # all of parallax-gpui's lib tests, then parallax-gpui-linux's
scripts/patch-test.sh pointer_source   # libtest filters and flags pass through
```

It needs GNU sed (`sed -i`; on macOS, `gsed` as `sed`).

[`patch-test.sh`](../scripts/patch-test.sh) copies [`build/parallax-gpui/`](gpui-fork-crates.md#generate) to
`build/patch-tests/` (generated, git-ignored, rebuilt every run) and runs
`cargo test -p parallax-gpui --features test-support --lib` there, compiled into `target/patch-tests/`, then the
lib tests of each backend crate in its `host_backends` list (on Linux, `parallax-gpui-linux`: patches 0022, 0023 and
0049's tables, [desktop-touch-linux.md](desktop-touch-linux.md#tests), [native-view-wayland.md](native-view-wayland.md#tests);
`parallax-gpui-wgpu`: patch 0047's hole, drawn by its headless renderer on the host's Vulkan, [native-view.md](native-view.md#tests)). It never
changes `build/parallax-gpui/`, so what it tests is what [`publish-fork.sh`](../scripts/publish-fork.sh) packages,
plus the test-only changes the crates need because they can't run their own tests as shipped:

| Shipped crate lacks | The scratch copy gets |
|---|---|
| `assets/fonts/…` that `svg_renderer.rs` and gpui_wgpu's `cosmic_text_system.rs` tests `include_bytes!` from Zed's repo | the first three `.ttf` files under `/usr/share/fonts`, as stand-ins (Ubuntu: `fonts-dejavu-core`) |
| gpui_wgpu's `naga` dev-dependency (its shader tests) | `naga` 30 with `wgsl-in`; the script fails once the crate ships dev-dependencies of its own |
| `util_macros` (Zed's `#[perf]` test attribute in `style.rs`) | `#[test]` in place of `#[perf]`, its `use` dropped; the script fails if neither is there any more |

## Skipped tests

The `skips` array in the script lists tests by full name, each with its reason: today the four `svg_renderer`
tests that look IBM Plex Sans and Lilex up by family name, which the stand-ins don't have. Every other test must
pass. Add a row only for a test this setup can't run, never for one a patch broke; fix the patch instead.
libtest's `--skip` matches substrings, so the script lists the tests first (`--list`) and fails unless each row
names exactly one test; your own filters still pass straight to libtest. `backend_skips` does the same for the backend
crates, where a row may name a whole module: today gpui_wgpu's `cosmic_text_system::tests::`, which shape text in
Zed's fonts by family name.

## Backends' pure modules

gpui_windows, gpui_ios and gpui_apple compile only on their own OS and gpui_web only on wasm32, and CI never builds a dependency's
tests on those rows. Their pure decision modules (plain values, no `windows`, objc2 or `web_sys` crate) therefore run
here: after GPUI's tests, the script adds one scratch crate per `pure_crates` row (`windows-pure`, `web-pure`,
`ios-pure`, `apple-pure`) to the scratch workspace, with each module of the row included by `#[path]` on `parallax-gpui`, and runs
its lib tests, in the backend's own edition (read from its fork manifest). Today: gpui_windows's `pointer.rs`
([desktop-touch-windows.md](desktop-touch-windows.md)), `accessibility_display.rs` ([os-settings.md](os-settings.md)) and
`native_views.rs` ([native-view-windows.md](native-view-windows.md)); gpui_web's `text_editing.rs`
([web-text-input.md](web-text-input.md)), `page_lifecycle.rs` and `keyboard_inset.rs`
([web-lifecycle.md](web-lifecycle.md)); gpui_ios's `ios/keyboard.rs`, `ios/keyboard_inset.rs` and `ios/spring_animation.rs`
([ios-keyboard.md](ios-keyboard.md)); gpui_apple's `native_view_placement.rs` ([native-view-apple.md](native-view-apple.md)). A listed module must use only `gpui` and `std`.

## CI

The `patch-tests` job in [ci.yml](../.github/workflows/ci.yml) ([ci.md](ci.md)) runs the script on Linux, separate
from the per-target build matrix.
