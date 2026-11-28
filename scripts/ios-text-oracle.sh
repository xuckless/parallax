#!/usr/bin/env bash
# Run Flutter's iOS text input oracle (patch 0028's table, with upstream's UIKit protocol tests) on an iOS simulator:
# builds parallax-gpui-ios's `text_input` test for aarch64-apple-ios-sim and runs it with `simctl spawn`, which gives
# the binary the main thread and UIKit without an app bundle. Exits non-zero if a row fails. See docs/ios-text-input.md.
#
# Usage: scripts/ios-text-oracle.sh [row filter]   e.g. scripts/ios-text-oracle.sh testDeletingBackward
# On macOS it runs here (CI's macOS runner); elsewhere it copies build/parallax-gpui/ to $MAC (default pixl-mac) over ssh
# and runs there, in $MAC_DIR. Env: IOS_SIMULATOR (device name, default "iPhone 17e"), MAC, MAC_DIR, RUST_TOOLCHAIN_DIR
# (a rustup home with rust-toolchain.toml's channel and the aarch64-apple-ios-sim target; default: the Mac's shared
# one, or rustup's).
# Run scripts/apply-patches.sh first.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
filter=${1:-}
[[ -d $repo/build/parallax-gpui ]] || { echo "ios-text-oracle: no build/parallax-gpui; run scripts/apply-patches.sh" >&2; exit 1; }

# The Mac half: build, boot the simulator if it is not booted, run, shut down only a simulator this run booted.
# Arguments: fork crate dir, work dir, simulator name, row filter, toolchain channel.
mac_run=$(cat <<'EOF'
set -euo pipefail
fork=$1 work=$2 simulator=$3 filter=$4 channel=$5
if [[ -n ${RUST_TOOLCHAIN_DIR:-} ]]; then
  export RUSTUP_HOME=$RUST_TOOLCHAIN_DIR
  bin=$RUSTUP_HOME/toolchains/$channel-aarch64-apple-darwin/bin
  export PATH=$bin:$PATH RUSTC=$bin/rustc
fi
export CARGO_TARGET_DIR=$work/target TMPDIR=$work/tmp IPHONEOS_DEPLOYMENT_TARGET=17.0 CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
mkdir -p "$TMPDIR"
cd "$fork"
build=(cargo test -p parallax-gpui-ios --test text_input --target aarch64-apple-ios-sim --no-run)
"${build[@]}" --quiet
binary=$("${build[@]}" --quiet --message-format=json | sed -n 's/.*"executable":"\([^"]*text_input[^"]*\)".*/\1/p' | tail -n 1)
[[ -n $binary ]] || { echo "ios-text-oracle: no text_input test binary" >&2; exit 1; }
udid=$(xcrun simctl list devices available -j | python3 -c '
import json, sys
name = sys.argv[1]
print(next(d["udid"] for r in json.load(sys.stdin)["devices"].values() for d in r if d["name"] == name))' "$simulator")
booted_here=no
if ! xcrun simctl list devices booted | grep -q "$udid"; then
  xcrun simctl boot "$udid"
  booted_here=yes
fi
trap '[[ $booted_here == yes ]] && xcrun simctl shutdown "$udid"' EXIT
xcrun simctl bootstatus "$udid" -b >/dev/null
xcrun simctl spawn "$udid" "$binary" ${filter:+"$filter"}
EOF
)

simulator=${IOS_SIMULATOR:-iPhone 17e}
channel=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$repo/rust-toolchain.toml")
if [[ $(uname) == Darwin ]]; then
  bash -c "$mac_run" mac-run "$repo/build/parallax-gpui" "$repo/target/ios-text-oracle" "$simulator" "$filter" "$channel"
else
  mac=${MAC:-pixl-mac}
  mac_dir=${MAC_DIR:-/Users/Shared/parallax/ios-text-oracle}
  toolchain=${RUST_TOOLCHAIN_DIR:-/Users/Shared/parallax/gap-audit/rustup}
  ssh "$mac" "mkdir -p '$mac_dir'"
  rsync -az --delete "$repo/build/parallax-gpui/" "$mac:$mac_dir/fork/"
  # shellcheck disable=SC2029 # the arguments are expanded here on purpose
  ssh "$mac" "RUST_TOOLCHAIN_DIR='$toolchain' CARGO_HOME='$mac_dir/cargo' bash -s -- '$mac_dir/fork' '$mac_dir' '$simulator' '$filter' '$channel'" <<<"$mac_run"
fi
