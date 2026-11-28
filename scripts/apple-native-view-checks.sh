#!/usr/bin/env bash
# Runs examples/native-view on an iOS simulator on the Mac and checks it (docs/native-view-apple.md#checks):
#   idle        0 GPUI frames/s while only the native view changes, the view at 50 or more
#   still       every frame of a 3 s recording shows the view and the div over it, and no gap
#   input       a tap and a swipe on the view reach the slot's listeners, a tap on the div only the div's
#   moving      the same while "Move" moves and resizes the slot every frame
#   rotating    the same through a rotation to landscape and back
#   keyboard    after the native UITextField took the keyboard and every native view was dropped, a tap on the GPUI
#               field types there (hello logs `text field submitted: 3 bytes` on Return)
# Frames are checked by apple-native-view-frames.py. Prints PASS or FAIL per row; exits 1 if any row fails.
# Usage: scripts/apple-native-view-checks.sh [simulator] [mac dir]
#   simulator  a simulator's name (default "iPhone 17"; the iPad check: "iPad Air 11-inch (M4)")
#   mac dir    where on pixl-mac the repo, build and recordings go (default /Users/Shared/parallax/native-view-checks)
# Needs ssh host pixl-mac with the shared rustup, idb and ffmpeg under /Users/Shared/parallax (docs/traces.md). It
# shuts its simulator and idb companion down when done.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
simulator=${1:-iPhone 17}
mac_dir=${2:-/Users/Shared/parallax/native-view-checks}
channel=$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$repo/rust-toolchain.toml")

# Of build/, the Mac needs only the fork crates the workspace builds from (scripts/apply-patches.sh).
rsync -az --delete --exclude /target --exclude .git --include /build/parallax-gpui --exclude '/build/*' \
    "$repo/" "pixl-mac:$mac_dir/parallax/"

# The part that runs on the Mac goes there as a file: idb and simctl would read a script fed to bash's stdin.
ssh pixl-mac "cat >'$mac_dir/checks.sh'" <<'EOF'
set -euo pipefail
dir=$1 simulator=$2 channel=$3
tools=/Users/Shared/parallax/tools
port=10884
export RUSTUP_HOME=/Users/Shared/parallax/gap-audit/rustup CARGO_HOME=$dir/cargo CARGO_TARGET_DIR=$dir/target
export TMPDIR=$dir/tmp IPHONEOS_DEPLOYMENT_TARGET=17.0 CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-3}
bin=$RUSTUP_HOME/toolchains/$channel-aarch64-apple-darwin/bin
export PATH=$bin:/opt/homebrew/bin:$PATH RUSTC=$bin/rustc
run=$dir/run app=$dir/run/NativeView.app id=io.github.xuckless.parallax.nativeview
mkdir -p "$TMPDIR" "$app"
cd "$dir/parallax"

cargo build --quiet -p native-view --target aarch64-apple-ios-sim
cp "$CARGO_TARGET_DIR/aarch64-apple-ios-sim/debug/native-view" "$app/"
# Upstream's example plist, renamed. UIDeviceFamily [1, 2] (Xcode adds it): without it the iPad runs the app as a
# scaled 320 x 480 iPhone app.
sed -e 's#gpui_ios_example#native-view#' -e "s#dev.zed.gpui-ios-example#$id#" -e 's#GPUIIosExample#NativeView#' \
    -e 's#<string>15.0</string>#<string>17.0</string>#' upstream/gpui-pre-ios/examples/ios/app/Info.plist >"$app/Info.plist"
plutil -insert UIDeviceFamily -json '[1,2]' "$app/Info.plist"
codesign --force --sign - "$app" >/dev/null 2>&1

udid=$(xcrun simctl list devices available -j | python3 -c "import json, sys
print(next(d['udid'] for r in json.load(sys.stdin)['devices'].values() for d in r if d['name'] == sys.argv[1]))" "$simulator")
xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b >/dev/null
nohup "$tools/idb-companion/idb_companion" --udid "$udid" --grpc-port $port >"$run/companion.log" 2>&1 &
companion=$!
trap 'kill $companion 2>/dev/null; xcrun simctl shutdown "$udid" 2>/dev/null' EXIT
sleep 3
idb() { "$tools/idb-venv/bin/idb" --companion localhost:$port "$@"; }

xcrun simctl install "$udid" "$app"
: >"$run/app.log"
SIMCTL_CHILD_RUST_LOG=warn,native_view=info,hello=info,gpui_apple=info \
    xcrun simctl launch --terminate-running-process --stderr="$run/app.log" "$udid" "$id" >/dev/null
sleep 6

failed=0
row() { # row NAME OK DETAIL
    printf '%-10s %s  %s\n' "$1" "$([[ $2 == 1 ]] && echo PASS || echo FAIL)" "$3"
    [[ $2 == 1 ]] || failed=1
}
# Taps are in points, from the example's layout: buttons and fields sit 60 pt down, from 16 pt in (src/apple.rs).
scale=$(idb describe | grep -o 'density=[0-9.]*' | cut -d= -f2)
width=$(idb describe | grep -o 'width_points=[0-9]*' | cut -d= -f2)
tap() { idb ui tap "$1" "$2"; sleep "${3:-1}"; }
record() { # record NAME ACTION...: records the screen while the action runs, then checks every frame
    local name=$1
    shift
    xcrun simctl io "$udid" recordVideo --codec h264 --force "$run/$name.mp4" >/dev/null 2>&1 &
    local recorder=$!
    sleep 1
    "$@"
    kill -INT $recorder
    wait $recorder || true
    local out ok=1
    out=$(python3 -I scripts/apple-native-view-frames.py "$run/$name.mp4") || ok=0
    row "$name" $ok "$(tail -1 <<<"$out")"
    [[ $ok == 1 ]] || head -5 <<<"$out"
}
since() { tail -n +"$(($1 + 1))" "$run/app.log"; }
lines() { wc -l <"$run/app.log"; }

# The example's once-a-second line and the renderers' alpha lines: the same two regexes in every native-view check
# script (docs/examples-native-view.md).
frames_line='native-view: gpui frames/s ([0-9]+), view frames/s ([0-9]+)'
alpha_line='alpha mode [A-Z][A-Za-z]+|alpha mode, only [A-Za-z]+|premultiplied alpha \(|metal layer alpha: [a-z]+'
sleep 3
idle=$(grep -oE "$frames_line" "$run/app.log" | tail -1)
read -r gpui native < <(sed -E "s#$frames_line#\1 \2#" <<<"$idle") || true
row idle $([[ ${gpui:-1} == 0 && ${native:-0} -ge 50 ]] && echo 1 || echo 0) "$idle; $(grep -oE "$alpha_line" "$run/app.log" | tail -1)"

record still sleep 3

# input: the slot is 56..376 x 184..424 pt while still; the div 16 pt in from its top right (src/apple.rs).
mark=$(lines)
tap 100 380
tap 290 235
idb ui swipe 150 400 150 300 --duration 0.3
sleep 1
events=$(since "$mark" | grep -oE 'view pointer (down|scroll) at .*|div over view: down')
downs=$(grep -c 'view pointer down' <<<"$events" || true)
divs=$(grep -c 'div over view: down' <<<"$events" || true)
summary=$(sed -E 's/(view pointer scroll).*/\1/' <<<"$events" | uniq | paste -sd ';' -)
row input $([[ $downs == 1 && $events == *"view pointer scroll"* && $divs == 1 ]] && echo 1 || echo 0) "$summary"
moving() { tap 47 80; sleep 4; tap 47 80; }
record moving moving
rotating() { idb rotation LANDSCAPE_LEFT; sleep 2.5; idb rotation PORTRAIT; sleep 2.5; }
record rotating rotating

# keyboard: the native field takes the keyboard, every native view is dropped ("Drop views", found by its amber),
# then the GPUI field must take typing.
mark=$(lines)
tap $((width - 96)) 138 1.5
xcrun simctl io "$udid" screenshot "$run/shot.png" >/dev/null 2>&1
read -r x y < <(python3 -I scripts/locate-colour.py ffc857 <"$run/shot.png")
tap "$(awk -v v="$x" -v s="$scale" 'BEGIN { print int(v / s) }')" "$(awk -v v="$y" -v s="$scale" 'BEGIN { print int(v / s) }')"
tap 115 138 1.5
idb ui text "abc"
sleep 0.5
idb ui key 40
sleep 1
events=$(since "$mark" | grep -oE 'UITextField: first responder (yes|no)|dropped the native views|text field submitted: [0-9]+ bytes' | paste -sd ';' -)
row keyboard $([[ $events == *"first responder yes"*"dropped the native views"*"text field submitted: 3 bytes"* ]] && echo 1 || echo 0) "$events"

grep -c panicked "$run/app.log" >/dev/null && { row panics 0 "$(grep -m1 panicked "$run/app.log")"; }
exit $failed
EOF
ssh pixl-mac bash "$mac_dir/checks.sh" "$mac_dir" "'$simulator'" "$channel"
