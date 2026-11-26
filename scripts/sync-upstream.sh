#!/usr/bin/env bash
# Move upstream/ to a gpui-pre snapshot from crates.io: re-vendor it, rewrite upstream/UPSTREAM, reset
# patches/FORK_REVISION to 1 on a new version, repin the parallax-gpui* fork crates in Cargo.toml, check every patch
# (listing all that no longer apply), rebuild build/ with apply-patches.sh, regenerate the [patch.crates-io] rows from
# the fork crates it built, then `cargo check --workspace`.
# Usage: scripts/sync-upstream.sh [VERSION]    (default: latest gpui-pre on crates.io)
# Needs network (crates.io, github.com), curl and jq. Replaces upstream/ wholesale; review the result with git diff.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

die() { echo "error: $*" >&2; exit 1; }

version="${1:-$(cargo search gpui-pre --limit 20 | sed -n 's/^gpui-pre = "\([^"]*\)".*/\1/p')}"
[[ -n "$version" ]] || die "could not find the latest gpui-pre on crates.io; pass VERSION"
echo "syncing upstream/ to gpui-pre $version"

# Vendor roots: every gpui-pre-* crate a vendored manifest names, optional deps included, is reachable from these, so
# the fork crates are complete (fork-crates.sh fails on a missing one). bench-metrics and zlog are optional deps
# (gpui's bench-support; ztracing's tracing-tracy and web). To add a crate, add a row here and to the "How:" line
# of UPSTREAM below.
roots=(
    'gpui-pre = "=VERSION"'
    'gpui-pre-platform = { version = "=VERSION", features = ["gpui_wgpu"] }'
    'gpui-pre-wgpu = "=VERSION"'
    'gpui-pre-ios = "=VERSION"'
    'gpui-pre-web = "=VERSION"'
    'gpui-pre-bench-metrics = "=VERSION"'
    'gpui-pre-zlog = "=VERSION"'
)

# Scratch space lives under build/ (generated, git-ignored) so `git apply --directory` can reach it.
scratch="build/sync-upstream"
trap 'rm -rf "$scratch"' EXIT
rm -rf "$scratch"
mkdir -p "$scratch/crate/src" "$scratch/upstream"
touch "$scratch/crate/src/lib.rs"
{
    printf '[package]\nname = "parallax-vendor-scratch"\nversion = "0.0.0"\nedition = "2024"\n\n[workspace]\n\n[dependencies]\n'
    printf '%s\n' "${roots[@]//VERSION/$version}"
} > "$scratch/crate/Cargo.toml"

echo "vendoring (all targets)"
(cd "$scratch/crate" && cargo vendor --quiet vendor > /dev/null) || die "cargo vendor failed for gpui-pre $version"

# Copy only the gpui-pre-* crates, unmodified. A versioned dir (name-x.y.z) means two versions of one crate: refuse.
zed_revs=()
crate_rows=()
for dir in "$scratch"/crate/vendor/gpui-pre*/; do
    crate="$(basename "$dir")"
    manifest="$dir/Cargo.toml"
    crate_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$manifest" | head -n1)"
    [[ "$crate_version" == "$version" ]] || die "$crate is version '$crate_version', expected $version"
    zed_revs+=("$(sed -n 's/^zed-rev = "\(.*\)"$/\1/p' "$manifest")")
    crate_rows+=("$(printf '  %-29s%-8s%s' "$crate" "$crate_version" "$(sed -n 's/^zed-crate = "\(.*\)"$/\1/p' "$manifest")")")
    cp -a "$dir" "$scratch/upstream/$crate"
done
[[ ${#crate_rows[@]} -gt 0 ]] || die "no gpui-pre-* crates vendored"

mapfile -t revs < <(printf '%s\n' "${zed_revs[@]}" | sort -u)
[[ ${#revs[@]} -eq 1 && -n "${revs[0]}" ]] || die "crates disagree on [package.metadata.gpui-pre] zed-rev: ${revs[*]}"
zed_rev="${revs[0]}"

zed_date="$(curl -fsS "https://api.github.com/repos/zed-industries/zed/commits/$zed_rev" | jq -r '.commit.committer.date')" \
    || die "could not read zed commit $zed_rev from github.com"
zed_toolchain="$(curl -fsS "https://raw.githubusercontent.com/zed-industries/zed/$zed_rev/rust-toolchain.toml" \
    | sed -n 's/^channel = "\(.*\)"$/\1/p')" || die "could not read rust-toolchain.toml at zed $zed_rev"

# Crate set changes between versions are printed; the [patch.crates-io] rows are regenerated below to match.
mapfile -t old_crates < <(find upstream -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort)
mapfile -t new_crates < <(find "$scratch/upstream" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort)
added="$(comm -13 <(printf '%s\n' "${old_crates[@]}") <(printf '%s\n' "${new_crates[@]}"))"
removed="$(comm -23 <(printf '%s\n' "${old_crates[@]}") <(printf '%s\n' "${new_crates[@]}"))"
[[ -z "$added" ]] || printf 'crates added:\n%s\n' "$added"
[[ -z "$removed" ]] || printf 'crates removed:\n%s\n' "$removed"

cat > "$scratch/upstream/UPSTREAM" <<EOF
# Pristine GPUI snapshot. Never hand-edit anything in upstream/; change it only through patches/.

gpui-pre version: $version
Zed commit:       $zed_rev (zed-industries/zed, ${zed_date%%T*})
                  Read from [package.metadata.gpui-pre] zed-rev in every vendored Cargo.toml.
Zed toolchain:    $zed_toolchain (rust-toolchain.toml at that commit)
Vendored on:      $(date +%F)

How: \`cargo vendor\` of a scratch crate depending on gpui-pre, gpui-pre-platform (feature gpui_wgpu),
gpui-pre-wgpu, gpui-pre-ios, gpui-pre-web, gpui-pre-bench-metrics and gpui-pre-zlog, all =$version, for all
targets. Only the gpui-pre-* directories were copied here, unmodified (.cargo-checksum.json kept as a pristine
check). Third-party deps resolve from crates.io.

Crates (crates.io name, version, Zed crate):
$(printf '%s\n' "${crate_rows[@]}")
EOF

old_version="$(sed -n 's/^gpui-pre version: *\([^ ]*\)$/\1/p' upstream/UPSTREAM)"
rm -rf upstream
mv "$scratch/upstream" upstream
echo "replaced upstream/"

# A new upstream version starts the fork's patch revision over; re-vendoring the same version keeps it.
if [[ "$old_version" != "$version" ]]; then
    sed -i -E 's/^[0-9]+$/1/' patches/FORK_REVISION
    echo "reset patches/FORK_REVISION to 1"
fi
fork_version="$version-parallax.$(grep -v '^#' patches/FORK_REVISION | tr -d '[:space:]')"
sed -i -E "/package = \"parallax-gpui/ s/version = \"=[^\"]*\"/version = \"=$fork_version\"/" Cargo.toml
echo "pinned parallax-gpui* to =$fork_version in Cargo.toml"

rustup_channel="$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)"
[[ "$rustup_channel" == "$zed_toolchain" ]] \
    || echo "warning: Zed uses toolchain $zed_toolchain, rust-toolchain.toml has $rustup_channel (see AGENTS.md)" >&2

# Check every patch in order on a scratch copy, skipping failures so all of them are reported, not just the first.
check="$scratch/check"
mkdir -p "$check"
cp -a upstream/. "$check/"
failed=()
shopt -s nullglob
for patch in patches/*.patch; do
    git apply --directory="$check" "$patch" 2> /dev/null || failed+=("$patch")
done
if [[ ${#failed[@]} -gt 0 ]]; then
    echo "error: these patches no longer apply to gpui-pre $version (see docs/upstream.md):" >&2
    printf '  %s\n' "${failed[@]}" >&2
    exit 1
fi

scripts/apply-patches.sh

# Regenerate the [patch.crates-io] rows from the fork crates apply-patches.sh just built (fork-crates.sh's naming rule).
mapfile -t forks < <(find build/parallax-gpui -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort)
width=0
for fork in "${forks[@]}"; do if (( ${#fork} > width )); then width=${#fork}; fi; done
rows="$(for fork in "${forks[@]}"; do
    printf '%-*s = { path = "build/parallax-gpui/%s" }\n' "$width" "$fork" "$fork"
done)"
awk -v rows="$rows" '
    /^parallax-gpui[a-z-]* *= \{ path = "build\/parallax-gpui\// { next }
    { print }
    /^\[patch\.crates-io\]$/ { print rows }
' Cargo.toml > "$scratch/Cargo.toml"
mv "$scratch/Cargo.toml" Cargo.toml
echo "regenerated the [patch.crates-io] rows"

cargo check --workspace
echo "synced to gpui-pre $version (zed $zed_rev)"
