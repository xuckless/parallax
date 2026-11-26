#!/usr/bin/env bash
# Publish the parallax-gpui* fork crates in build/parallax-gpui/ (made by apply-patches.sh) to crates.io, each after
# the fork crates it depends on. See docs/gpui-fork-crates.md.
# Usage: scripts/publish-fork.sh [--dry-run | --published]
#   (none)       publish every fork crate whose version is not on crates.io yet; waits out crates.io's new-crate rate
#                limit (HTTP 429) and retries, which can take hours for a set of new names. Needs `cargo login`.
#   --dry-run    package and verify every fork crate together (`cargo package --workspace`); publishes nothing.
#   --published  exit 0 if every fork crate's version is on crates.io, 1 if any is missing (CI gates on it).
# Compiled output goes to target/parallax-gpui/, so regenerating build/ keeps the compiled dependencies.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fork="$root/build/parallax-gpui"
target_dir="$root/target/parallax-gpui"
mode="${1:-publish}"

die() { echo "publish-fork: $*" >&2; exit 1; }

[[ -f $fork/Cargo.toml ]] || die "no $fork; run scripts/apply-patches.sh"
cd "$fork"

if [[ $mode == --dry-run ]]; then
    cargo package --workspace --target-dir "$target_dir"
    echo "packaged and verified every fork crate; nothing published"
    exit 0
fi
[[ $mode == publish || $mode == --published ]] || die "unknown option '$mode' (see the usage at the top)"

# Crate → version, and the publish order: a topological sort of every fork-crate dependency edge (all kinds), so a
# crate goes up only after what it names. tsort fails loudly on a cycle.
metadata="$(cargo metadata --no-deps --format-version 1)"
declare -A versions
while read -r name version; do versions[$name]=$version; done \
    < <(jq -r '.packages[] | "\(.name) \(.version)"' <<< "$metadata")
mapfile -t order < <(jq -r '.packages[] | .name as $crate | "\($crate) \($crate)",
    (.dependencies[] | select(.name | startswith("parallax-gpui")) | "\(.name) \($crate)")' <<< "$metadata" | tsort)
[[ ${#order[@]} == "${#versions[@]}" ]] || die "publish order has ${#order[@]} crates, the workspace ${#versions[@]}"

# Is name@version on crates.io? Asks the sparse index (what cargo reads): 200 lists the versions, 404 means a new name.
on_crates_io() {
    local name=$1 version=$2 body status found=1
    body="$(mktemp)"
    status="$(curl -sS -o "$body" -w '%{http_code}' "https://index.crates.io/${name:0:2}/${name:2:2}/$name")" \
        || die "cannot reach index.crates.io"
    case $status in
        200) jq -e --arg version "$version" 'select(.vers == $version)' "$body" > /dev/null && found=0 ;;
        404) ;;
        *) die "index.crates.io answered $status for $name" ;;
    esac
    rm -f "$body"
    return $found
}

if [[ $mode == --published ]]; then
    missing=0
    for name in "${order[@]}"; do
        on_crates_io "$name" "${versions[$name]}" || { echo "not on crates.io: $name ${versions[$name]}"; missing=1; }
    done
    exit $missing
fi

published=0
for name in "${order[@]}"; do
    if on_crates_io "$name" "${versions[$name]}"; then echo "skip $name ${versions[$name]}: already on crates.io"; continue; fi
    while true; do
        echo "publishing $name ${versions[$name]}"
        log="$(mktemp)"
        if cargo publish -p "$name" --target-dir "$target_dir" 2>&1 | tee "$log"; then rm -f "$log"; break; fi
        # crates.io's new-crate rate limit: "... Please try again after <HTTP date> or email help@crates.io ...".
        retry_at="$(sed -n 's/.*try again after \(.*\) or email.*/\1/p' "$log" | head -n1)"
        rm -f "$log"
        [[ -n $retry_at ]] || die "cargo publish failed for $name (not a rate limit; see above)"
        wait_s=$(( $(date -d "$retry_at" +%s) - $(date +%s) + 5 ))
        echo "rate limited: waiting $wait_s s, until $retry_at"
        sleep $(( wait_s > 0 ? wait_s : 5 ))
    done
    published=$((published + 1))
done
echo "published $published fork crates; the rest were already on crates.io"
