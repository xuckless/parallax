#!/usr/bin/env bash
# Show, for every ledger row in third_party/flutter/PORTED.md, how that Flutter file changed between
# the row's tag and NEW_TAG (default: the newest stable tag, i.e. the highest plain X.Y.Z tag).
# Usage: scripts/flutter-diff.sh [NEW_TAG]
# Files are fetched one at a time through the authenticated `gh api` (needs `gh auth login`). Nothing is cached.
# A malformed ledger row, an unknown tag or a file missing at its row's tag is an error; a file missing at
# NEW_TAG (moved or deleted) is reported and counted.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ledger="$root/third_party/flutter/PORTED.md"
repo="flutter/flutter"
header='| Flutter path | Flutter tag | Parallax path | Kind (port / oracle) | Dropped parts |'
stable_tag='^[0-9]+\.[0-9]+\.[0-9]+$' # Flutter stable tags are plain X.Y.Z; betas end in -N.M.pre.

die() { echo "error: $*" >&2; exit 1; }

command -v gh >/dev/null || die "gh (GitHub CLI) not found"
gh auth status >/dev/null 2>&1 || die "gh is not logged in: run 'gh auth login'"

stable_tags="$(gh api --paginate "repos/$repo/git/matching-refs/tags/" --jq '.[].ref' |
    sed 's|^refs/tags/||' | grep -E "$stable_tag" | sort -V)"
[[ -n "$stable_tags" ]] || die "no stable tags found in $repo"

new_tag="${1:-$(tail -n 1 <<<"$stable_tags")}"
grep -qxF "$new_tag" <<<"$stable_tags" || die "'$new_tag' is not a stable tag of $repo"

# Ledger rows: every line after the header and its |---| separator, up to the first non-table line.
grep -qxF "$header" "$ledger" || die "ledger header not found in $ledger: expected '$header'"
rows="$(awk -v h="$header" '
    found == 2 && /^\|/ { print; next }
    found == 2 { exit }
    found == 1 { found = 2; next }
    $0 == h { found = 1 }' "$ledger")"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# fetch PATH TAG OUT: write PATH at TAG to OUT. Returns 1 only on 404; any other failure is fatal.
fetch() {
    if gh api -H 'Accept: application/vnd.github.raw' "repos/$repo/contents/$1?ref=$2" >"$3" 2>"$tmp/err"; then
        return 0
    fi
    grep -q 'HTTP 404' "$tmp/err" && return 1
    die "fetching $1 at $2 failed: $(cat "$tmp/err")"
}

# Validate every row before fetching anything, so a malformed ledger fails before any output.
paths=() tags=()
while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    [[ "$line" == \|*\| ]] || die "malformed ledger row (must start and end with '|'): $line"
    pipes="${line//[^|]/}"
    [[ ${#pipes} -eq 6 ]] || die "malformed ledger row (need 5 cells, got $((${#pipes} - 1))): $line"
    # Cells 1 and 2, trimmed of spaces and optional `code` backticks.
    path="$(cut -d'|' -f2 <<<"$line" | sed -E 's/^ *`?//; s/`? *$//')"
    tag="$(cut -d'|' -f3 <<<"$line" | sed -E 's/^ *`?//; s/`? *$//')"
    [[ "$path" =~ ^[^[:space:]]+$ ]] || die "malformed Flutter path '$path' in row: $line"
    grep -qxF "$tag" <<<"$stable_tags" || die "'$tag' is not a stable tag of $repo, in row: $line"
    paths+=("$path") tags+=("$tag")
done <<<"$rows"

changed=0 unchanged=0 missing=0
for i in "${!paths[@]}"; do
    path="${paths[$i]}" tag="${tags[$i]}"
    echo "=== $path ($tag -> $new_tag)"
    fetch "$path" "$tag" "$tmp/old" || die "$path does not exist at its ledger tag $tag"
    if ! fetch "$path" "$new_tag" "$tmp/new"; then
        echo "missing at $new_tag (moved or deleted)"
        missing=$((missing + 1))
        continue
    fi
    rc=0
    diff -u --label "$path@$tag" --label "$path@$new_tag" "$tmp/old" "$tmp/new" || rc=$?
    case "$rc" in
        0) echo "unchanged"; unchanged=$((unchanged + 1)) ;;
        1) changed=$((changed + 1)) ;;
        *) die "diff failed for $path" ;;
    esac
done

echo
echo "summary ($new_tag): $changed changed, $unchanged unchanged, $missing missing at $new_tag"
