#!/usr/bin/env bash
# Translate a git-format patch between the vendored layout (paths relative to upstream/, a/gpui-pre-ios/src/x.rs)
# and Zed's layout (a/crates/gpui_ios/src/x.rs), so one patch applies to both. Writes the result to stdout.
# Usage: scripts/patch-paths.sh to-zed|to-vendored <in.patch>
# Only header lines (diff --git, ---, +++, rename/copy from/to) are rewritten; hunk bodies pass through untouched.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Vendored crate dir → its dir in zed-industries/zed at the commit in upstream/UPSTREAM.
# To extend: add one row per new upstream/gpui-pre-* dir, Zed path checked against Zed's tree at that commit.
crates="
gpui-pre                    crates/gpui
gpui-pre-apple              crates/gpui_apple
gpui-pre-bench-metrics      crates/bench_metrics
gpui-pre-collections        crates/collections
gpui-pre-derive-refineable  crates/refineable/derive_refineable
gpui-pre-http-client        crates/http_client
gpui-pre-ios                crates/gpui_ios
gpui-pre-linux              crates/gpui_linux
gpui-pre-macos              crates/gpui_macos
gpui-pre-macros             crates/gpui_macros
gpui-pre-platform           crates/gpui_platform
gpui-pre-refineable         crates/refineable
gpui-pre-scheduler          crates/scheduler
gpui-pre-shared-string      crates/gpui_shared_string
gpui-pre-sum-tree           crates/sum_tree
gpui-pre-util               crates/gpui_util
gpui-pre-web                crates/gpui_web
gpui-pre-wgpu               crates/gpui_wgpu
gpui-pre-windows            crates/gpui_windows
gpui-pre-zlog               crates/zlog
gpui-pre-ztracing           crates/ztracing
gpui-pre-ztracing-macro     crates/ztracing_macro
"

# Vendored paths with no faithful counterpart in Zed's tree (glob → reason). The script refuses them.
# To extend: add a row when a sync finds a file that crates.io packaging or gpui-pre adds or rewrites wholesale.
untranslatable="
*/Cargo.toml                                  normalized on publish; hand-write the Zed manifest change
*/Cargo.toml.orig                             crates.io packaging artifact
*/Cargo.lock                                  crates.io packaging artifact
*/.cargo-checksum.json                        cargo vendor artifact
gpui-pre-apple/vendor/*                       gpui-pre copy of crates/gpui sources; patch gpui-pre/ instead
gpui-pre-macros/src/gpui_pre_facade_paths.rs  gpui-pre-only file
"

die() { echo "patch-paths: $*" >&2; exit 1; }

[[ $# -eq 2 && ( $1 == to-zed || $1 == to-vendored ) ]] || die "usage: $0 to-zed|to-vendored <in.patch>"
direction="$1"
patch="$2"
[[ -f $patch ]] || die "no such patch: $patch"

declare -a vendored=() zed=()
while read -r v z; do
    [[ -n $v ]] && vendored+=("$v") && zed+=("$z")
done <<< "$crates"

# Echo the translated path, or die if it is not in $crates or its vendored side is in $untranslatable.
translate() {
    local path="$1" vpath="" out="" best=-1 i
    [[ $path == /dev/null ]] && { echo "$path"; return; }
    for i in "${!vendored[@]}"; do
        if [[ $direction == to-zed && $path == "${vendored[$i]}/"* ]]; then
            vpath="$path"
            out="${zed[$i]}/${path#"${vendored[$i]}/"}"
        elif [[ $direction == to-vendored && $path == "${zed[$i]}/"* && ${#zed[$i]} -gt $best ]]; then
            # Longest prefix wins: crates/refineable/derive_refineable sits inside crates/refineable.
            best=${#zed[$i]}
            out="${vendored[$i]}/${path#"${zed[$i]}/"}"
            vpath="$out"
        fi
    done
    [[ -n $out ]] || die "path not in the crate table: $path"
    while read -r glob reason; do
        # shellcheck disable=SC2053 # $glob is a pattern on purpose
        [[ -n $glob && $vpath == $glob ]] && die "cannot translate $path: $reason"
    done <<< "$untranslatable"
    echo "$out"
}

# Patch sections: preamble (commit message) → header (per file) → hunk; a new "diff --git" starts a header.
section=preamble
result=""
while IFS= read -r line || [[ -n $line ]]; do
    case "$section:$line" in
        *:"diff --git "*)
            section=header
            read -r _ _ a b <<< "$line"
            [[ $a == a/* && $b == b/* ]] || die "unsupported diff line (quoted or spaced path?): $line"
            # Separate assignments: a failing $(translate) exits only when it is the whole assignment.
            from="${b#b/}"
            a="$(translate "${a#a/}")"
            b="$(translate "$from")"
            line="diff --git a/$a b/$b"
            # gpui-pre marks every source file it rewrote with a first-line comment; warn once per file.
            vpath="$from"
            [[ $direction == to-vendored ]] && vpath="$b"
            if [[ -f $root/upstream/$vpath ]] && head -n 1 "$root/upstream/$vpath" | grep -q '^// Modified for gpui-pre'; then
                echo "patch-paths: warning: gpui-pre rewrote $vpath; hunks near its edits may not apply to the other layout" >&2
            fi
            ;;
        header:"@@"*) section=hunk ;;
        header:"--- a/"* | header:"+++ b/"*) path="$(translate "${line:6}")"; line="${line:0:6}$path" ;;
        header:"--- /dev/null" | header:"+++ /dev/null") ;;
        header:"rename from "* | header:"copy from "*) path="$(translate "${line#* from }")"; line="${line%% from *} from $path" ;;
        header:"rename to "* | header:"copy to "*) path="$(translate "${line#* to }")"; line="${line%% to *} to $path" ;;
        *) ;;
    esac
    result+="$line"$'\n'
done < "$patch"
[[ $section != preamble ]] || die "no 'diff --git' header found: $patch is not a git-format patch"
printf '%s' "$result"
