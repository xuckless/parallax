#!/usr/bin/env bash
# Rebuild the `parallax` branch of our Zed fork: the Zed commit in upstream/UPSTREAM plus one commit per
# patches/*.patch (translated with patch-paths.sh to-zed), then force-push that one branch. Run after each sync.
# Usage: scripts/sync-fork.sh [DIR]    (DIR: scratch Zed checkout, default build/zed-fork; reused across runs)
# Idempotent: commit dates are pinned to the base commit's, so an unchanged input rebuilds the same SHAs.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
zed_url="https://github.com/zed-industries/zed.git"
fork_url="https://github.com/xuckless/zed.git"
branch="parallax"
dir="$(realpath -m "${1:-$root/build/zed-fork}")"

die() { echo "sync-fork: $*" >&2; exit 1; }

base="$(sed -n 's/^Zed commit: *\([0-9a-f]\{40\}\).*/\1/p' "$root/upstream/UPSTREAM")"
[[ -n $base ]] || die "no Zed commit in upstream/UPSTREAM"

# Zed is huge: shallow, blobless fetch of the one base commit; sparse checkout pulls blobs only for files we touch.
[[ -d $dir/.git ]] || git init -q "$dir"
git -C "$dir" remote remove zed 2> /dev/null || true
git -C "$dir" remote add zed "$zed_url"
git -C "$dir" fetch -q --depth=1 --filter=blob:none zed "$base" || die "cannot fetch zed $base"

shopt -s nullglob
patches=("$root"/patches/*.patch)
translated="$dir.patches"
rm -rf "$translated"
mkdir -p "$translated"
paths=('/crates/gpui*/')
for patch in "${patches[@]}"; do
    out="$translated/$(basename "$patch")"
    "$root/scripts/patch-paths.sh" to-zed "$patch" > "$out" || die "cannot translate $patch"
    mapfile -t -O "${#paths[@]}" paths < <(sed -n 's|^+++ b/|/|p' "$out")
done

git -C "$dir" sparse-checkout set --no-cone "${paths[@]}"
git -C "$dir" checkout -q --force -B "$branch" "$base"
git -C "$dir" clean -qfdx

# Pin dates so a rerun on the same inputs yields the same commits and the push is a no-op.
GIT_AUTHOR_DATE="$(git -C "$dir" log -1 --format=%cI "$base")"
GIT_COMMITTER_DATE="$GIT_AUTHOR_DATE"
export GIT_AUTHOR_DATE GIT_COMMITTER_DATE

for patch in "${patches[@]}"; do
    out="$translated/$(basename "$patch")"
    git -C "$dir" apply --check "$out" || die "$(basename "$patch") does not apply to zed $base"
    git -C "$dir" apply --index "$out"
    # Subject = first header line, body = the rest of the header (everything above the first `diff --git`).
    header="$(sed '/^diff --git /,$d' "$patch")"
    git -C "$dir" commit -q -m "$(head -n 1 <<< "$header")" -m "$(tail -n +2 <<< "$header")"
done

git -C "$dir" push -q --force "$fork_url" "refs/heads/$branch:refs/heads/$branch" || die "push to $fork_url failed"
echo "pushed $fork_url $branch at $(git -C "$dir" rev-parse HEAD) (zed $base + ${#patches[@]} patches)"
