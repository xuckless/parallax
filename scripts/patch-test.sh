#!/usr/bin/env bash
# Run GPUI's own lib tests, every patch's tests among them, on the patched fork crate parallax-gpui.
# The crate can't run them as shipped, so this tests a scratch copy of build/parallax-gpui/ in build/patch-tests/
# (generated, git-ignored, rebuilt every run) with two test-only changes, and never touches build/parallax-gpui/:
#   - stand-in fonts at the two paths svg_renderer.rs includes from Zed's assets/, which is not vendored;
#   - no #[perf] attributes in style.rs, whose util_macros is not vendored.
# Usage: scripts/patch-test.sh [libtest args...]   e.g. scripts/patch-test.sh pointer_source
# Run scripts/apply-patches.sh first. See docs/patch-tests.md.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
in="build/parallax-gpui"
out="build/patch-tests"
cd "$root"

die() { echo "patch-test: $*" >&2; exit 1; }

[[ -d $in ]] || die "no $in; run scripts/apply-patches.sh"
rm -rf "$out"
mkdir -p "$out"
cp -a "$in" "$out/crates"

# svg_renderer.rs includes "../../../assets/fonts/<path>" from crates/parallax-gpui/src/, so assets/ sits in $out.
# Any two fonts do; the tests that need Zed's own are in `skips`.
included=(ibm-plex-sans/IBMPlexSans-Regular.ttf lilex/Lilex-Regular.ttf)
mapfile -t fonts < <(find /usr/share/fonts -name '*.ttf' | LC_ALL=C sort | head -n 2)
(( ${#fonts[@]} == 2 )) || die "need two .ttf files under /usr/share/fonts as stand-in fonts (Ubuntu: fonts-dejavu-core)"
for i in 0 1; do
    mkdir -p "$(dirname "$out/assets/fonts/${included[$i]}")"
    cp "${fonts[$i]}" "$out/assets/fonts/${included[$i]}"
done
sed -i -E '/^\s*(#\[perf\]|use util_macros::perf;)$/d' "$out/crates/parallax-gpui/src/style.rs"

# Tests skipped by full name, one row each with the reason. Add a row only for a test this setup can't run, never
# for a test a patch broke.
skips=(
    # These three look the bundled fonts up by family name ("IBM Plex Sans", "Lilex"); stand-ins have other names.
    svg_renderer::tests::fix_generic_font_families_monospace_resolves_to_lilex
    svg_renderer::tests::fix_generic_font_families_sets_all_families
    svg_renderer::tests::test_select_emoji_font_skips_family_without_glyph
)
skip_args=()
for test in "${skips[@]}"; do skip_args+=(--skip "$test"); done

cargo test --manifest-path "$out/crates/Cargo.toml" -p parallax-gpui --features test-support --lib \
    --target-dir target/patch-tests -- "${skip_args[@]}" "$@"
