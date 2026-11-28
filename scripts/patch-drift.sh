#!/usr/bin/env bash
# Silent-drift check for a sync: per patch, the files it touches that Zed changed between two commits, with the size of
# Zed's change. A patch can apply cleanly yet act on changed code; read Zed's diff of every file listed.
# Usage: scripts/patch-drift.sh OLD NEW [--diff]   (Zed commits: the old upstream/UPSTREAM one and the new one)
#        --diff also prints Zed's diff of each listed file once.
# Keeps a blobless Zed fetch in build/zed-drift/ (git-ignored, reused). Needs network (github.com).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
old="${1:?usage: scripts/patch-drift.sh OLD NEW [--diff]}" new="${2:?usage: scripts/patch-drift.sh OLD NEW [--diff]}"
zed="build/zed-drift"

[[ -d $zed/.git ]] || { git init -q "$zed" && git -C "$zed" remote add origin "https://github.com/zed-industries/zed.git"; }
git -C "$zed" fetch -q --depth=1 --filter=blob:none origin "$old" "$new"
changed="$(git -C "$zed" diff --name-only "$old" "$new" -- crates)"

shopt -s nullglob
listed=()
for patch in patches/*.patch; do
    hits=""
    # Zed-layout paths; manifests are left out (hand-written, patches/zed/).
    for file in $(scripts/patch-paths.sh to-zed --drop-manifests "$patch" 2> /dev/null | sed -n 's|^+++ b/||p'); do
        grep -qxF "$file" <<< "$changed" || continue
        hits+=" ${file#crates/} ($(git -C "$zed" diff --numstat "$old" "$new" -- "$file" | awk '{ print "+" $1 " -" $2 }'))"
        listed+=("$file")
    done
    echo "$(basename "$patch" .patch):${hits:- none}"
done
if [[ ${3:-} == --diff ]]; then
    mapfile -t files < <(printf '%s\n' "${listed[@]}" | sort -u)
    git -C "$zed" diff "$old" "$new" -- "${files[@]}"
fi
