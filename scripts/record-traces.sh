#!/usr/bin/env bash
# Records touch traces from examples/hello's trace pad on one platform: performs each scripted gesture with the
# platform's own input tool and saves the `touch(…)` lines GPUI received as a test fixture (docs/traces.md).
#
# Usage: scripts/record-traces.sh <android|web|ios> [gesture…]     (default: every row of the gesture table)
# Needs: android: ANDROID_HOME, one running emulator or device (ANDROID_SERIAL picks one), cargo-parallax's
#        requirements (docs/cli.md). web: wasm-pack, Node 22+, Chromium (CHROMIUM overrides its path).
#        ios: ssh host pixl-mac with the iOS simulator and idb under /Users/Shared/parallax (docs/traces.md).
#        All: python3.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
platform=${1:?usage: record-traces.sh <android|web|ios> [gesture…]}
shift
fixtures=$repo/crates/platform/tests/traces/$platform
work=$repo/target/record-traces/$platform
mkdir -p "$fixtures" "$work"

# The gestures, in logical px and ms, the same on every platform. To add one: a row here, and its expected outcome in
# the replay test (crates/platform/tests/trace_replay.rs). Verbs:
#   tap x y hold · taps x y count pause hold · swipe x1 y1 x2 y2 ms · pinch x y radius scale ms
# Every point sits inside the smallest window recorded (iOS: 402 × 874 points). slow-drag moves at 40 px/s, under
# every row's minimum fling velocity, so it never flings however long the driver waits before lifting.
gestures=(
  "tap|tap 200 400 50"
  "double-tap|taps 200 400 2 100 50"
  "long-press|tap 200 400 800"
  "slow-drag|swipe 200 600 200 400 5000"
  "fling|swipe 200 650 200 250 150"
  "pinch|pinch 200 400 50 2 500"
)
# hello's "Record touches" button (RECORD_BUTTON in examples/hello/src/lib.rs).
record_button=a855f7

# --- Android: adb `input` on hello installed by cargo-parallax. `input` takes device pixels and has no multi-touch.
app=io.github.xuckless.parallax.hello
adb_shell() { "${ANDROID_HOME:?set ANDROID_HOME}/platform-tools/adb" shell "$@"; }
android_setup() {
  cargo build --quiet -p cargo-parallax --manifest-path "$repo/Cargo.toml"
  (cd "$repo/examples/hello" && "$repo/target/debug/cargo-parallax" run android ${ANDROID_SERIAL:+--device "$ANDROID_SERIAL"})
  density=$(adb_shell wm density | awk '{ d = $NF } END { print d / 160 }')
}
android_launch() {
  adb_shell logcat -c
  adb_shell am start -S -W -n "$app/io.github.xuckless.parallax.ParallaxActivity" >/dev/null
}
android_log() { adb_shell logcat -d | tr -d '\r'; }
android_screenshot() { "$ANDROID_HOME/platform-tools/adb" exec-out screencap >"$1"; }
android_scale() { echo "$density"; }
px() { awk -v d="$density" '{ for (i = 1; i <= NF; i++) printf "%d ", $i * d }' <<<"$*"; }
android_tap() { adb_shell input swipe $(px "$1 $2 $1 $2") "$3"; }
android_taps() { for ((i = 0; i < $3; i++)); do adb_shell input tap $(px "$1 $2"); done; }
android_swipe() { adb_shell input swipe $(px "$1 $2 $3 $4") "$5"; }
android_pinch() { echo "skip: adb input has no multi-touch"; return 1; }

# --- Web: hello's wasm build in headless Chromium, driven over CDP by cdp-touch.mjs. CSS px are logical px.
cdp_port=9333
cdp() { node "$repo/scripts/cdp-touch.mjs" "$cdp_port" "$@"; }
web_setup() {
  wasm-pack build "$repo/examples/hello" --target web --no-typescript --no-pack --dev
  python3 -m http.server 8765 --bind 127.0.0.1 --directory "$repo/examples/hello" >/dev/null 2>&1 &
  # Console messages reach the log only through Chromium's own stderr logging.
  "${CHROMIUM:-chromium-browser}" --headless=new --remote-debugging-port=$cdp_port --user-data-dir="$work/profile" \
    --window-size=412,915 --force-device-scale-factor=1 --touch-events=enabled --enable-logging=stderr --v=0 \
    about:blank 2>"$work/chrome.log" &
  trap 'kill $(jobs -p) 2>/dev/null' EXIT
  sleep 3
}
web_launch() {
  web_mark=$(wc -l <"$work/chrome.log")
  cdp open http://127.0.0.1:8765/
}
web_log() { tail -n +"$((web_mark + 1))" "$work/chrome.log" | grep -a 'CONSOLE'; }
web_screenshot() { cdp screenshot "$1"; }
web_scale() { echo 1; }
web_tap() { cdp tap "$@"; }
web_taps() { cdp taps "$@"; }
web_swipe() { cdp swipe "$@"; }
web_pinch() { cdp pinch "$@"; }

# --- iOS: hello's desktop bin built for the simulator on the Mac and run as a bare .app; idb drives it in points.
mac=pixl-mac
mac_dir=/Users/Shared/parallax/traces
tools=/Users/Shared/parallax/tools
simulator=${IOS_SIMULATOR:-iPhone 17}
on_mac() { ssh "$mac" "$@"; }
idb() { on_mac "$tools/idb-venv/bin/idb --companion localhost:10882 $*"; }
ios_setup() {
  rsync -az --delete --exclude target --exclude .git --exclude build/gpui-patched --exclude build/patch-tests \
    --exclude examples/hello/pkg "$repo/" "$mac:$mac_dir/parallax/"
  on_mac "bash -s" <<EOF
set -euo pipefail
cd $mac_dir/parallax
# The gap audit's toolchain (1.98.1 with the iOS simulator target), used read-only; builds stay under $mac_dir.
export RUSTUP_HOME=/Users/Shared/parallax/gap-audit/rustup CARGO_HOME=$mac_dir/cargo CARGO_TARGET_DIR=$mac_dir/target
export TMPDIR=$mac_dir/tmp IPHONEOS_DEPLOYMENT_TARGET=17.0
mkdir -p \$TMPDIR $mac_dir/Hello.app
bin=\$RUSTUP_HOME/toolchains/1.98.1-aarch64-apple-darwin/bin
PATH=\$bin:\$PATH RUSTC=\$bin/rustc \$bin/cargo build --quiet -p hello --bin hello --target aarch64-apple-ios-sim
cp \$CARGO_TARGET_DIR/aarch64-apple-ios-sim/debug/hello $mac_dir/Hello.app/hello
# Upstream's example plist, renamed: the scene delegate gpui_ios registers.
sed -e 's#gpui_ios_example#hello#' -e 's#dev.zed.gpui-ios-example#$app#' -e 's#GPUIIosExample#Hello#' \
  -e 's#<string>15.0</string>#<string>17.0</string>#' upstream/gpui-pre-ios/examples/ios/app/Info.plist \
  >$mac_dir/Hello.app/Info.plist
codesign --force --sign - $mac_dir/Hello.app
xcrun simctl boot "$simulator" 2>/dev/null || true
xcrun simctl bootstatus "$simulator" -b >/dev/null
xcrun simctl install "$simulator" $mac_dir/Hello.app
nc -z localhost 10882 || (nohup $tools/idb-companion/idb_companion --udid \$(xcrun simctl list devices booted -j |
  python3 -c 'import json,sys; print([d["udid"] for r in json.load(sys.stdin)["devices"].values() for d in r if d["name"] == "$simulator"][0])') \
  --grpc-port 10882 >$tools/companion.log 2>&1 &) && sleep 3
EOF
}
ios_launch() {
  # simctl appends to the --stderr file: empty it, so the log holds this launch only.
  on_mac "xcrun simctl terminate '$simulator' $app 2>/dev/null; : >$mac_dir/hello.log; SIMCTL_CHILD_RUST_LOG=info xcrun simctl launch \
    --stderr=$mac_dir/hello.log '$simulator' $app >/dev/null"
}
ios_log() { on_mac "cat $mac_dir/hello.log"; }
ios_screenshot() { on_mac "xcrun simctl io '$simulator' screenshot $mac_dir/shot.png >/dev/null 2>&1 && cat $mac_dir/shot.png" >"$1"; }
ios_scale() { idb describe | grep -o 'density=[0-9.]*' | cut -d= -f2; }
ios_tap() { idb ui tap "$1" "$2" --duration "$(ms_to_s "$3")"; }
ios_taps() { idb ui multi-tap "$1" "$2" --count "$3" --pause "$(ms_to_s "$4")" --duration "$(ms_to_s "$5")"; }
ios_swipe() { idb ui swipe "$1" "$2" "$3" "$4" --duration "$(ms_to_s "$5")" --delta 10; }
ios_pinch() { idb ui pinch "$1" "$2" "$4" --radius "$3" --duration "$(ms_to_s "$5")"; }
ms_to_s() { awk -v ms="$1" 'BEGIN { print ms / 1000 }'; }

# --- The run: launch, enter the trace pad, perform the gesture, keep what the pad logged.
wait_for() {
  local attempt
  for attempt in {1..30}; do
    "${platform}_log" | grep -q "$1" && return 0
    sleep 1
  done
  echo "record-traces: $platform never logged \"$1\"" >&2
  return 1
}

"${platform}_setup"
"${platform}_launch"
wait_for "window open"
sleep 2
"${platform}_screenshot" "$work/screen.png"
read -r button_x button_y < <(python3 -I "$repo/scripts/locate-colour.py" "$record_button" <"$work/screen.png")
scale=$("${platform}_scale")
button=$(awk -v s="$scale" -v x="$button_x" -v y="$button_y" 'BEGIN { printf "%d %d", x / s, y / s }')

wanted=" $* "
for row in "${gestures[@]}"; do
  IFS='|' read -r name command <<<"$row"
  [[ $# == 0 || $wanted == *" $name "* ]] || continue
  "${platform}_launch"
  wait_for "window open"
  sleep 1
  "${platform}_tap" $button 50
  wait_for "trace: recording"
  read -r verb args <<<"$command"
  # Unquoted on purpose: $args is the verb's numbers.
  if ! "${platform}_$verb" $args; then
    echo "record-traces: $platform $name skipped"
    continue
  fi
  sleep 1.5
  {
    echo "# $platform $name: $command"
    "${platform}_log" | grep -o 'touch([^)]*)'
  } >"$fixtures/$name.trace"
  echo "record-traces: $platform $name: $(($(wc -l <"$fixtures/$name.trace") - 1)) events"
done
