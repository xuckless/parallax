#!/usr/bin/env bash
# Run GPUI's own lib tests, every patch's tests among them, on the patched fork crate parallax-gpui.
# On Linux it also runs parallax-gpui-linux's lib tests (host_backends below).
# The crate can't run them as shipped, so this tests a scratch copy of build/parallax-gpui/ in build/patch-tests/
# (generated, git-ignored, rebuilt every run) with two test-only changes, and never touches build/parallax-gpui/:
#   - stand-in fonts at the two paths svg_renderer.rs includes from Zed's assets/, which is not vendored;
#   - #[test] for Zed's #[perf] in style.rs (#[perf] wraps #[test]; its util_macros is not vendored).
# Then it tests gpui_windows's and gpui_ios's pure modules (pure_crates below) on the host as scratch crates.
# Usage: scripts/patch-test.sh [libtest args...]   e.g. scripts/patch-test.sh pointer_source
# Needs GNU sed (sed -i). Run scripts/apply-patches.sh first. See docs/patch-tests.md.
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
style="$out/crates/parallax-gpui/src/style.rs"
# Fails when upstream drops #[perf] or its import, so this edit can't silently stop applying.
grep -Eq '^\s*#\[perf\]$' "$style" && grep -q '^\s*use util_macros::perf;$' "$style" \
    || die "style.rs no longer has #[perf] tests and their util_macros import: update this script"
sed -i -E 's/^(\s*)#\[perf\]$/\1#[test]/; /^\s*use util_macros::perf;$/d' "$style"

# Tests skipped by full name, one row each with the reason. Add a row only for a test this setup can't run, never
# for a test a patch broke.
skips=(
    # These four look the bundled fonts up by family name ("IBM Plex Sans", "Lilex"); stand-ins have other names.
    svg_renderer::tests::fix_generic_font_families_monospace_resolves_to_lilex
    svg_renderer::tests::fix_generic_font_families_sets_all_families
    svg_renderer::tests::measured_text_width_matches_rasterized_advance
    svg_renderer::tests::test_select_emoji_font_skips_family_without_glyph
)
test_cmd=(cargo test --manifest-path "$out/crates/Cargo.toml" -p parallax-gpui --features test-support --lib
    --target-dir target/patch-tests --)

# libtest's --skip matches substrings, so each row must name exactly one test: then it skips that test and no other,
# and user filters ("$@") still work as libtest filters. A renamed or removed test fails here too.
mapfile -t tests < <("${test_cmd[@]}" --list --format terse | sed -n 's/: test$//p')
skip_args=()
for test in "${skips[@]}"; do
    matches="$(printf '%s\n' "${tests[@]}" | grep -cF -- "$test" || true)"
    # Not grep -q: quitting at the first match can kill printf mid-write, which pipefail reads as no match.
    printf '%s\n' "${tests[@]}" | grep -xF -- "$test" >/dev/null && [[ $matches == 1 ]] \
        || die "skip '$test' must name exactly one test; it matches $matches"
    skip_args+=(--skip "$test")
done

"${test_cmd[@]}" "${skip_args[@]}" "$@"

# gpui_windows and gpui_ios build only on their own OS and gpui_web only on wasm32, so their pure decision modules
# (plain values, no `windows`, objc2 or `web_sys` crate) are tested here as one host crate per backend on
# parallax-gpui, members of the scratch workspace.
# One row per backend: "scratch crate:fork crate:modules", each module its path under the crate's src/ without
# ".rs", space-separated. To add a module, add its path to its backend's row. A module listed must use only `gpui`
# and `std`.
pure_crates=(
    "windows-pure:parallax-gpui-windows:pointer accessibility_display native_views"
    "web-pure:parallax-gpui-web:text_editing page_lifecycle keyboard_inset"
    "ios-pure:parallax-gpui-ios:ios/keyboard ios/keyboard_inset ios/spring_animation"
)
for row in "${pure_crates[@]}"; do
    IFS=: read -r name backend modules <<<"$row"
    pure="$out/crates/$name"
    mkdir -p "$pure/src"
    # The modules' edition is their backend's own.
    edition="$(sed -n 's/^edition = "\(.*\)"$/\1/p' "$out/crates/$backend/Cargo.toml")"
    [[ -n $edition ]] || die "no edition in $backend's Cargo.toml"
    cat > "$pure/Cargo.toml" <<EOF
[package]
name = "$name"
version = "0.0.0"
edition = "$edition"
publish = false

[dependencies]
gpui = { package = "parallax-gpui", path = "../parallax-gpui", features = ["test-support"] }
EOF
    {
        echo "#![allow(dead_code)] // their callers are $backend's target-only glue"
        for module in $modules; do
            printf '#[path = "../../%s/src/%s.rs"]\nmod %s;\n' "$backend" "$module" "${module##*/}"
        done
    } > "$pure/src/lib.rs"
    sed -i "s/^members = \[\$/members = [\n    \"$name\",/" "$out/crates/Cargo.toml"
    grep -qx "    \"$name\"," "$out/crates/Cargo.toml" || die "could not add $name to the scratch workspace"
    cargo test --manifest-path "$out/crates/Cargo.toml" -p "$name" --lib --target-dir target/patch-tests -- "$@"
done

# Backend crates whose lib tests also run here, where their patches put table tests; one row per crate this host
# builds. Add a row when a backend patch ships tests the host can run.
host_backends=()
[[ $(uname -s) == Linux ]] && host_backends+=(parallax-gpui-linux)   # 0022's Wayland and 0023's X11 touch tables
for crate in "${host_backends[@]}"; do
    cargo test --manifest-path "$out/crates/Cargo.toml" -p "$crate" --features test-support --lib \
        --target-dir target/patch-tests -- "$@"
done
