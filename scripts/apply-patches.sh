#!/usr/bin/env bash
# Build build/gpui-patched/ from pristine upstream/ plus patches/*.patch, applied in sorted order.
# Patch paths are relative to upstream/ (e.g. a/gpui-pre/src/app.rs).
# build/ is generated and git-ignored: never edit it. Every run starts from a fresh copy.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="build/gpui-patched"
cd "$root"

rm -rf "$out"
mkdir -p "$out"
cp -a upstream/. "$out/"

shopt -s nullglob
for patch in patches/*.patch; do
    echo "applying $patch"
    git apply --directory="$out" "$patch" || { echo "error: $patch does not apply to upstream/" >&2; exit 1; }
done
echo "built $out"
