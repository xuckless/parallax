#!/usr/bin/env bash
# Turn build/gpui-patched/ (upstream + patches) into the publishable parallax-gpui* fork crates in build/parallax-gpui/,
# plus a workspace manifest there so publish-fork.sh can package them together. apply-patches.sh runs this last; run it
# alone after editing build/gpui-patched/ while making a patch (docs/patches.md). build/ is generated: never edit it.
# Each crate keeps upstream's code, [lib] name, licence and metadata; only its name, version, inter-crate deps,
# description, repository and README change. Version: scripts/fork-version.sh. See docs/gpui-fork-crates.md.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
in="build/gpui-patched"
out="build/parallax-gpui"
cd "$root"
source scripts/fork-version.sh

die() { echo "fork-crates: $*" >&2; exit 1; }

# The one naming rule: gpui-pre → parallax-gpui, gpui-pre-<x> → parallax-gpui-<x>.
fork_name() { echo "parallax-gpui${1#gpui-pre}"; }

fork_check
upstream_version="$(fork_upstream_version)"
version="$(fork_version)"
zed_rev="$(sed -n 's/^Zed commit: *\([0-9a-f]\{40\}\).*/\1/p' upstream/UPSTREAM)"
[[ -n $zed_rev ]] || die "no Zed commit in upstream/UPSTREAM"
[[ -d $in ]] || die "no $in; run scripts/apply-patches.sh"

# The workspace pins the fork exactly; a stale pin would build one version and publish another.
stale="$(grep 'package = "parallax-gpui' Cargo.toml | grep -v "version = \"=$version\"" || true)"
[[ -z $stale ]] || die "Cargo.toml must pin every parallax-gpui* crate to =$version (scripts/fork-version.sh bump
rewrites both):
$stale"

# Patch list for the READMEs: file name and first header line of each patch.
shopt -s nullglob
patch_list="$(for patch in patches/*.patch; do printf -- '- `%s`: %s\n' "$(basename "$patch")" "$(head -n1 "$patch")"; done)"
[[ -n $patch_list ]] || patch_list="(none)"

rm -rf "$out"
mkdir -p "$out"
members=()
for dir in "$in"/gpui-pre*/; do
    crate="$(basename "$dir")"
    fork="$(fork_name "$crate")"
    dest="$out/$fork"
    cp -a "$dir" "$dest"
    # Packaging leftovers of the gpui-pre tarball (and the .git a patch session leaves); cargo regenerates them.
    rm -rf "$dest/.cargo-checksum.json" "$dest/Cargo.toml.orig" "$dest/Cargo.lock" "$dest/.git"
    [[ -f $dest/LICENSE-APACHE ]] || die "$crate has no LICENSE-APACHE"
    zed_crate="$(sed -n 's/^zed-crate = "\(.*\)"$/\1/p' "$dest/Cargo.toml")"
    [[ -n $zed_crate ]] || die "$crate has no [package.metadata.gpui-pre] zed-crate"

    # Manifests are crates.io-normalized: one key per line, every gpui-pre dep its own table holding `package`.
    # Per table: in [package] set name, version, description, repository, readme; in a gpui-pre dep table set the
    # package name, the exact version and a path to the sibling fork crate (cargo drops the path on publish; it is
    # what links the crates locally and orders `cargo package --workspace`). Every other line passes through.
    awk -v fork="$fork" -v version="$version" -v upstream_version="$upstream_version" \
        -v description="Parallax's build of GPUI $zed_crate: upstream gpui-pre $upstream_version (Zed ${zed_rev:0:7}) + Parallax patches" '
        BEGIN {
            # [package] keys we set (key → value); to change another key, add a row.
            package["name"] = fork
            package["version"] = version
            package["description"] = description
            package["repository"] = "https://github.com/xuckless/parallax"
            package["readme"] = "README.md"
        }
        function flush(   i, gpui_dep, line, key) {
            gpui_dep = 0
            for (i = 1; i <= n; i++) if (lines[i] ~ /^package = "gpui-pre(-[a-z-]+)?"$/) gpui_dep = 1
            for (i = 1; i <= n; i++) {
                line = lines[i]
                key = line
                sub(/ = .*/, "", key)
                if (table == "[package]" && key in package) line = key " = \"" package[key] "\""
                else if (gpui_dep && line == "version = \"=" upstream_version "\"") line = "version = \"=" version "\""
                else if (gpui_dep && line ~ /^package = /) {
                    sub(/"gpui-pre/, "\"parallax-gpui", line)
                    dep = line
                    gsub(/^package = "|"$/, "", dep)
                    line = line "\npath = \"../" dep "\""
                }
                print line
            }
            n = 0
        }
        /^\[/ { flush(); table = $0 }
        { lines[++n] = $0 }
        END { flush() }
    ' "$dir/Cargo.toml" > "$dest/Cargo.toml"

    # Anything still naming gpui-pre (outside the description and the gpui-pre metadata table) was missed.
    leftover="$(grep -n 'gpui-pre' "$dest/Cargo.toml" | grep -v '^[0-9]*:description = \|^[0-9]*:\[package.metadata.gpui-pre\]$' || true)"
    [[ -z $leftover ]] || die "$fork/Cargo.toml still names gpui-pre:
$leftover"
    [[ $(grep -c "^version = \"$version\"$" "$dest/Cargo.toml") == 1 ]] || die "$fork/Cargo.toml: package version not set"

    cat > "$dest/README.md" <<README
# $fork

Parallax's build of GPUI's \`$zed_crate\` crate, published so [Parallax](https://github.com/xuckless/parallax) crates
can ship its patches. The code is upstream [\`$crate\` $upstream_version](https://crates.io/crates/$crate/$upstream_version),
a snapshot of [zed-industries/zed@${zed_rev:0:7}](https://github.com/zed-industries/zed/tree/$zed_rev), plus these
Parallax patches:

$patch_list

The library name is upstream's, so code uses it unchanged. Version $version: the upstream version, then the patch
revision. Generated by [fork-crates.sh](https://github.com/xuckless/parallax/blob/main/scripts/fork-crates.sh).

GPUI is by Zed Industries, Apache-2.0 (see LICENSE-APACHE). Parallax is not affiliated with or endorsed by Zed Industries.
README
    members+=("$fork")
done
[[ ${#members[@]} -gt 0 ]] || die "no gpui-pre* crates in $in"

# Every fork crate a manifest depends on (optional ones too) must be one we build, or its publish cannot resolve.
missing="$(sed -n 's/^package = "\(parallax-gpui[a-z-]*\)"$/\1/p' "$out"/*/Cargo.toml | sort -u \
    | comm -23 - <(printf '%s\n' "${members[@]}" | sort))"
[[ -z $missing ]] || die "these are depended on but not vendored in upstream/ (add a vendor root in sync-upstream.sh):
$missing"

# The fork crates as one workspace, so `cargo package --workspace` verifies each crate against the just-packaged
# others before any is on crates.io. resolver 2, as upstream's manifests.
{
    printf '# Generated by scripts/fork-crates.sh. Never edit.\n[workspace]\nresolver = "2"\nmembers = [\n'
    printf '    "%s",\n' "${members[@]}"
    printf ']\n'
} > "$out/Cargo.toml"
echo "built $out: ${#members[@]} crates at $version"
