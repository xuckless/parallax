#!/usr/bin/env bash
# Rebase patches/*.patch onto a new upstream/, in order, after sync-upstream.sh reported patches that no longer apply.
# The tree build/rebase/ is a git repo: a copy of upstream/ plus one commit per patch. Each patch is classified as
#   clean / offset (git apply takes it as is) / fuzz (only GNU patch -F2 applies it: applied and regenerated in place,
#   header kept) / conflict (stops: GNU patch's .rej files are left in the tree to fix by hand).
# After fixing a conflict in build/rebase/ (delete the .rej files): `rebase-patches.sh regen NNNN`, then `resume`.
# Usage: scripts/rebase-patches.sh            start over from upstream/
#        scripts/rebase-patches.sh report     start over, but skip conflicts and go on (weekly drift job): lists every
#                                             conflict and exits 1 if there was one
#        scripts/rebase-patches.sh resume     continue after the last patch committed in the tree
#        scripts/rebase-patches.sh regen NNNN regenerate patches/NNNN-*.patch from the tree's uncommitted change
# A regenerated patch must keep its intent: compare its +/- lines with git diff, and read upstream's diff of every
# file it touches for silent drift (scripts/patch-drift.sh). See docs/upstream.md.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tree="build/rebase"
cd "$root"

die() { echo "rebase-patches: $*" >&2; exit 1; }
t() { git -C "$tree" -c user.name=rebase-patches -c user.email=rebase-patches@localhost "$@"; }

# The tree's uncommitted change becomes the patch's diff (header kept) and its commit.
regen() {
    local patch="$1"
    [[ -z $(find "$tree" -name '*.rej' -not -path '*/.git/*') ]] || die "delete the .rej files in $tree first"
    t add -A
    { sed '/^diff --git /,$d' "$patch"; t diff --cached; } > "$patch.new"
    mv "$patch.new" "$patch"
    t commit -qm "$(basename "$patch")"
}

# What a conflict does: stop for a hand fix, or (report) drop the half-applied patch, note it and go on.
on_conflict=stop
case "${1:-}" in
    ""|report)
        [[ ${1:-} == report ]] && on_conflict=skip
        rm -rf "$tree"
        mkdir -p "$tree"
        cp -a upstream/. "$tree/"
        t init -q
        t add -A
        t commit -qm upstream
        ;;
    resume) [[ -d $tree/.git ]] || die "no $tree; run without arguments first" ;;
    regen)
        patch="$(ls patches/"${2:?usage: rebase-patches.sh regen NNNN}"-*.patch)"
        regen "$patch"
        echo "$2 conflict (regenerated)"
        exit 0
        ;;
    *) die "unknown command '$1' (see the usage at the top)" ;;
esac

done="$(t log --format=%s)"
conflicts=()
shopt -s nullglob
for patch in patches/*.patch; do
    name="$(basename "$patch")"
    grep -qxF "$name" <<< "$done" && continue
    if out="$(cd "$tree" && git apply -v "$root/$patch" 2>&1)"; then
        offsets="$(grep -c 'offset' <<< "$out" || true)"
        if [[ $offsets -gt 0 ]]; then echo "${name:0:4} offset ($offsets hunks)"; else echo "${name:0:4} clean"; fi
        t add -A
        t commit -qm "$name"
    elif (cd "$tree" && patch -p1 --dry-run -F2 -s < "$root/$patch" > /dev/null 2>&1); then
        (cd "$tree" && patch -p1 -F2 -s --no-backup-if-mismatch < "$root/$patch")
        regen "$patch"
        echo "${name:0:4} fuzz (regenerated)"
    else
        grep -E '^error' <<< "$out" | head -10 >&2
        case $on_conflict in
            stop)
                (cd "$tree" && patch -p1 --no-backup-if-mismatch < "$root/$patch" > /dev/null || true)
                echo "${name:0:4} conflict: fix the .rej hunks in $tree, then: scripts/rebase-patches.sh regen ${name:0:4}"
                exit 1
                ;;
            skip)
                echo "${name:0:4} conflict (skipped; later patches that need it may fail too)"
                conflicts+=("${name:0:4}")
                ;;
        esac
    fi
done
if [[ ${#conflicts[@]} -gt 0 ]]; then
    echo "conflicts: ${conflicts[*]}"
    exit 1
fi
echo "every patch applies; rerun scripts/sync-upstream.sh"
