#!/usr/bin/env bash
# The one fork-version rule: parallax-gpui* crates are <gpui-pre version>-parallax.<revision>, the gpui-pre version
# from upstream/UPSTREAM and the revision from patches/FORK_REVISION. See docs/gpui-fork-crates.md.
# Usage: scripts/fork-version.sh          print the fork version
#        scripts/fork-version.sh bump     revision + 1 (patches changed between syncs)
#        scripts/fork-version.sh reset    revision 1 (new upstream version; sync-upstream.sh runs it)
# bump and reset rewrite the revision, the content hash (both in patches/FORK_REVISION) and the parallax-gpui* =pins
# in Cargo.toml together.
# Sourced (fork-crates.sh, sync-upstream.sh), it only defines the functions below.
set -euo pipefail

fork_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

fork_die() { echo "fork-version: $*" >&2; exit 1; }

fork_upstream_version() {
    local version
    version="$(sed -n 's/^gpui-pre version: *\([^ ]*\)$/\1/p' "$fork_root/upstream/UPSTREAM")"
    [[ -n $version ]] || fork_die "no gpui-pre version in upstream/UPSTREAM"
    echo "$version"
}

fork_revision() {
    local revision
    revision="$(sed -n 's/^revision = \([0-9]*\)$/\1/p' "$fork_root/patches/FORK_REVISION")"
    [[ $revision =~ ^[1-9][0-9]*$ ]] || fork_die "patches/FORK_REVISION needs a line 'revision = <positive integer>'"
    echo "$revision"
}

fork_version() { echo "$(fork_upstream_version)-parallax.$(fork_revision)"; }

# What a fork version is built from: upstream/UPSTREAM, then every patch in sorted order. A published version must
# never change content, so FORK_REVISION records this hash next to the revision.
fork_content_hash() {
    local patches
    patches="$(cd "$fork_root" && find patches -maxdepth 1 -name '*.patch' | LC_ALL=C sort)"
    (cd "$fork_root" && cat upstream/UPSTREAM $patches) | sha256sum | cut -d' ' -f1
}

fork_recorded_hash() { sed -n 's/^content = \([0-9a-f]\{64\}\)$/\1/p' "$fork_root/patches/FORK_REVISION"; }

# Fails when upstream/UPSTREAM or a patch changed since the revision was set.
fork_check() {
    [[ $(fork_recorded_hash) == "$(fork_content_hash)" ]] || fork_die "upstream/UPSTREAM or patches/ changed since \
revision $(fork_revision) was recorded; a published version must not change: run scripts/fork-version.sh bump"
}

# Writes revision $1 and the content hash, and repins every parallax-gpui* row in Cargo.toml to the resulting version.
fork_set_revision() {
    sed -i -E "s/^revision = [0-9]*$/revision = $1/; s/^content = .*$/content = $(fork_content_hash)/" \
        "$fork_root/patches/FORK_REVISION"
    local version
    version="$(fork_version)"
    sed -i -E "/package = \"parallax-gpui/ s/version = \"=[^\"]*\"/version = \"=$version\"/" "$fork_root/Cargo.toml"
    echo "fork version $version (patches/FORK_REVISION, Cargo.toml pins)"
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    case "${1:-}" in
        "") fork_version ;;
        bump) fork_set_revision $(( $(fork_revision) + 1 )) ;;
        reset) fork_set_revision 1 ;;
        *) fork_die "unknown command '$1' (see the usage at the top)" ;;
    esac
fi
