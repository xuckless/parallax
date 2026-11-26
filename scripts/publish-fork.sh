#!/usr/bin/env bash
# Publish the parallax-gpui* fork crates in build/parallax-gpui/ (made by apply-patches.sh) to crates.io, each after
# the fork crates it depends on. See docs/gpui-fork-crates.md.
# Usage: scripts/publish-fork.sh [--dry-run | --published]
#   (none)       publish every fork crate whose version is not on crates.io yet; waits out crates.io's new-crate rate
#                limit (HTTP 429) and retries, which can take hours for a set of new names. Needs `cargo login`.
#   --dry-run    package and verify every fork crate together (`cargo package --workspace`); publishes nothing.
#   --published  exit 0 if every fork crate's version is on crates.io, 1 if any is missing (CI gates on it).
# Exit codes: 0 done, 1 not published (--published only), 2 error (bad option, index unreachable, failed publish).
# Compiled output goes to target/parallax-gpui/, so regenerating build/ keeps the compiled dependencies.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fork="$root/build/parallax-gpui"
target_dir="$root/target/parallax-gpui"

die() { echo "publish-fork: $*" >&2; exit 2; }

# Arguments first, before any work.
[[ $# -le 1 ]] || die "takes at most one option (see the usage at the top)"
case "${1:-}" in
    "") mode=publish ;;
    --dry-run) mode=dry-run ;;
    --published) mode=published ;;
    *) die "unknown option '$1' (see the usage at the top)" ;;
esac

[[ -f $fork/Cargo.toml ]] || die "no $fork; run scripts/apply-patches.sh"
cd "$fork"

if [[ $mode == dry-run ]]; then
    cargo package --workspace --target-dir "$target_dir"
    echo "packaged and verified every fork crate; nothing published"
    exit 0
fi

# Scratch files (index answers, publish logs) live here and go on every exit path.
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

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
    local name=$1 version=$2 body="$scratch/index" status jq_status=0
    status="$(curl -sS -o "$body" -w '%{http_code}' "https://index.crates.io/${name:0:2}/${name:2:2}/$name")" \
        || die "cannot reach index.crates.io"
    case $status in
        200) ;;
        404) return 1 ;;
        *) die "index.crates.io answered $status for $name" ;;
    esac
    # jq -e: 0 a line matched, 1 or 4 none did; anything else is an unreadable answer.
    jq -e --arg version "$version" 'select(.vers == $version)' "$body" > /dev/null || jq_status=$?
    case $jq_status in
        0) return 0 ;;
        1 | 4) return 1 ;;
        *) die "cannot read the index entry for $name" ;;
    esac
}

if [[ $mode == published ]]; then
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
        log="$scratch/publish.log"
        if cargo publish -p "$name" --target-dir "$target_dir" 2>&1 | tee "$log"; then break; fi
        # Already there, though the index did not show it yet (lag after a partial run): a skip, not a failure.
        if grep -q 'already exists\|already uploaded' "$log"; then
            echo "skip $name ${versions[$name]}: crates.io already has it"
            continue 2
        fi
        # crates.io's new-crate rate limit: "... Please try again after <HTTP date> or email help@crates.io ...".
        retry_at="$(sed -n 's/.*try again after \(.*\) or email.*/\1/p' "$log" | head -n1)"
        [[ -n $retry_at ]] || die "cargo publish failed for $name (not a rate limit; see above)"
        wait_s=$(( $(date -d "$retry_at" +%s) - $(date +%s) + 5 ))
        echo "rate limited: waiting $wait_s s, until $retry_at"
        sleep $(( wait_s > 0 ? wait_s : 5 ))
    done
    published=$((published + 1))
done
echo "published $published fork crates; the rest were already on crates.io"
