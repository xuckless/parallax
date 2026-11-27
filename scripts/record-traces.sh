#!/usr/bin/env bash
# Records touch traces from examples/hello's trace pad on one platform: performs each scripted gesture with the
# platform's own input tool and saves the `touch(…)` lines GPUI received as a test fixture (docs/traces.md).
#
# Usage: scripts/record-traces.sh <android|web|ios|wayland|x11|windows> [gesture…]   (default: every gesture row)
# Needs: android: ANDROID_HOME, one running emulator or device (ANDROID_SERIAL picks one), cargo-parallax's
#        requirements (docs/cli.md). web: wasm-pack, Node 22+, Chromium (CHROMIUM overrides its path).
#        ios: ssh host pixl-mac with the iOS simulator and idb under /Users/Shared/parallax (docs/traces.md).
#        wayland, x11: Weston's build dependencies (scripts/build-weston.sh), Xwayland for x11; WESTON_SCALE (1 or 2)
#        sets the output scale; WESTON_RUN prefixes the Weston commands, e.g. `podman exec <container>` where
#        Weston is built in a container sharing this checkout's path and /tmp (docs/traces-desktop.md).
#        windows: Git Bash on Windows 10 1809+ with a desktop session (docs/desktop-touch-windows.md).
#        All: python3.
# Sourced (scripts/windows-touch-checks.sh does), it only defines the gestures and drivers, and runs nothing.
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck source=hello-android.sh
source "$repo/scripts/hello-android.sh"
platform=${1:?usage: record-traces.sh <android|web|ios|wayland|x11|windows> [gesture…]}
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
# hello's "Record touches" button (RECORD_BUTTON in examples/hello/src/lib.rs), and its box's first colour
# (COLOURS[0]): one tap on the box must log exactly one touch click.
record_button=a855f7
box_colour=3b82f6

# --- Frames: the gestures as touch frames, for the platforms that inject raw touches (android, wayland, x11, windows:
# each driver has a `<driver>_frames` sink). A frame is "ms finger…", a finger "slot:x:y" (logical px; down when new,
# else a move) or "slot:up"; fingers not listed stay put. The sink sends a frame, then waits its ms. Each builder
# prints one frame a line; `perform` routes a verb. Moves come every `frame_ms`, as from a 60 Hz touchscreen.
frame_ms=16
lerp() { awk -v a="$1" -v b="$2" -v i="$3" -v n="$4" 'BEGIN { printf "%.2f", a + (b - a) * i / n }'; }
frames_tap() { printf '%s\n' "$3 0:$1:$2" "0 0:up"; }
frames_taps() {
  local i
  for ((i = 0; i < $3; i++)); do printf '%s\n' "$5 0:$1:$2" "$4 0:up"; done
}
frames_swipe() {
  local i n=$(($5 / frame_ms))
  for ((i = 0; i <= n; i++)); do echo "$frame_ms 0:$(lerp "$1" "$3" $i $n):$(lerp "$2" "$4" $i $n)"; done
  echo "0 0:up"
}
# Two fingers `radius` either side of x, spread to radius·scale.
frames_pinch() {
  local i r n=$(($5 / frame_ms))
  for ((i = 0; i <= n; i++)); do
    r=$(lerp "$3" "$(awk -v r="$3" -v s="$4" 'BEGIN { print r * s }')" $i $n)
    echo "$frame_ms $(awk -v x="$1" -v y="$2" -v r="$r" 'BEGIN { printf "0:%.2f:%s 1:%.2f:%s", x - r, y, x + r, y }')"
  done
  echo "0 0:up 1:up"
}
# A gesture verb on the platform: its frames through the driver's sink, or the driver's own verb (web, ios).
perform() {
  local verb=$1 frames
  shift
  if declare -F "${driver}_frames" >/dev/null; then
    mapfile -t frames < <("frames_$verb" "$@")
    "${driver}_frames" "${frames[@]}"
  else
    "${driver}_$verb" "$@"
  fi
}

# --- Android: hello installed by cargo-parallax, touched by writing multi-touch events straight to the emulator's
# touchscreen. Not `adb shell input`: its swipe moves in a tight loop (~7 moves a millisecond, many stamped the same
# millisecond, which no velocity tracker can read) and it has no multi-touch.
app=$hello_app
adb() { "${ANDROID_HOME:?set ANDROID_HOME}/platform-tools/adb" "$@"; }
adb_shell() { adb shell "$@"; }
android_setup() {
  hello_android_run "${ANDROID_SERIAL:-}"
  density=$(adb_shell wm density | awk '{ d = $NF } END { print d / 160 }')
  read -r width height < <(adb_shell wm size | awk '{ s = $NF } END { sub("x", " ", s); print s }')
  # Only root may write to /dev/input.
  adb root >/dev/null && adb wait-for-device
  touchscreen=$(adb_shell getevent -p | awk '/^add device/ { d = $NF } /"virtio_input_multi_touch_1"/ { print d }')
  [[ -n $touchscreen ]] || { echo "record-traces: no virtio multi-touch screen: needs an emulator" >&2; return 1; }
}
android_launch() {
  adb_shell logcat -c
  adb_shell am start -S -W -n "$hello_component" >/dev/null
}
android_log() { adb_shell logcat -d | tr -d '\r'; }
android_screenshot() { adb exec-out screencap >"$1"; }
android_scale() { echo "$density"; }
# The frames sink writes one device script: per frame, type-B multi-touch input_events
# (24 bytes on the 64-bit emulator: a zero time the kernel fills in, type, code, value) through mksh's builtin
# `print`, then a sleep of `ms` less the ~3 ms toybox `sleep` takes to start. Axes run 0-32767 over the display;
# pressure must be above 0, or Android reads the slot as hovering.
touch_le() { local i; for ((i = 0; i < $2; i++)); do printf '\\x%02x' $((($1 >> (8 * i)) & 255)); done; }
touch_event() { printf '\\x00%.0s' {1..16}; touch_le "$1" 2; touch_le "$2" 2; touch_le "$3" 4; }
touch_axis() { awk -v v="$1" -v d="$density" -v n="$2" 'BEGIN { printf "%d", v * d * 32767 / n }'; }
android_frames() {
  local frame ms finger slot x y bytes script=$work/touch.sh
  local -A down=()
  echo "exec 3>$touchscreen" >"$script"
  for frame in "$@"; do
    read -r ms frame <<<"$frame"
    bytes=""
    for finger in $frame; do
      IFS=: read -r slot x y <<<"$finger"
      bytes+=$(touch_event 3 47 "$slot")
      if [[ $x == up ]]; then
        bytes+=$(touch_event 3 57 -1)
        unset "down[$slot]"
        continue
      fi
      [[ -n ${down[$slot]:-} ]] || bytes+=$(touch_event 3 57 $((slot + 1)))$(touch_event 3 58 512)
      down[$slot]=1
      bytes+=$(touch_event 3 53 "$(touch_axis "$x" "$width")")$(touch_event 3 54 "$(touch_axis "$y" "$height")")
    done
    bytes+=$(touch_event 0 0 0)
    echo "print -n '$bytes' >&3; sleep $(awk -v ms="$ms" 'BEGIN { print (ms > 3 ? ms - 3 : 0) / 1000 }')" >>"$script"
  done
  adb push -q "$script" /data/local/tmp/touch.sh && adb_shell sh /data/local/tmp/touch.sh
}

# --- Web: hello's wasm build in headless Chromium, driven over CDP by cdp-touch.mjs. CSS px are logical px.
# CDP_PORT and HTTP_PORT pick the ports; unset, the OS picks free ones, so two runs side by side never collide.
free_port() { python3 -I -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])'; }
cdp_port=${CDP_PORT:-$(free_port)}
http_port=${HTTP_PORT:-$(free_port)}
cdp() { node "$repo/scripts/cdp-touch.mjs" "$cdp_port" "$@"; }
web_setup() {
  wasm-pack build "$repo/examples/hello" --target web --no-typescript --no-pack --dev
  python3 -m http.server "$http_port" --bind 127.0.0.1 --directory "$repo/examples/hello" >/dev/null 2>&1 &
  # Console messages reach the log only through Chromium's own stderr logging.
  "${CHROMIUM:-chromium-browser}" --headless=new --remote-debugging-port="$cdp_port" --user-data-dir="$work/profile" \
    --window-size=412,915 --force-device-scale-factor=1 --touch-events=enabled --enable-logging=stderr --v=0 \
    about:blank 2>"$work/chrome.log" &
  trap 'kill $(jobs -p) 2>/dev/null' EXIT
  sleep 3
}
web_launch() {
  web_mark=$(wc -l <"$work/chrome.log")
  cdp open "http://127.0.0.1:$http_port/"
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

# --- Wayland and X11: hello's desktop bin in Weston's headless backend (build-weston.sh), touched through Weston's
# test plugin by weston-touch.py: real wl_touch events, stamped with CLOCK_MONOTONIC. The output is 400 × 700 logical
# px and the kiosk shell fills it with hello's one window, so output px are window px. x11 runs hello on the Xwayland
# Weston starts, which turns those wl_touch events into XInput 2 touch events.
# Weston's socket is in $weston_env, set only for its clients, so WESTON_RUN's own tools keep the session's. It lives
# under /tmp: a socket path must fit in 108 bytes.
weston_scale=${WESTON_SCALE:-1}
on_weston() { ${WESTON_RUN:-} env "${weston_env[@]}" "$@"; }
weston_setup() {
  weston_runtime=$(mktemp -d /tmp/parallax-weston.XXXXXX)
  weston_env=(XDG_RUNTIME_DIR="$weston_runtime" WAYLAND_DISPLAY=parallax-weston)
  weston_build=$(on_weston "$repo/scripts/build-weston.sh")
  cargo build --quiet -p hello --bin hello
  on_weston meson devenv -C "$weston_build" weston --backend=headless --renderer=pixman --shell=kiosk \
    --width=400 --height=700 --scale="$weston_scale" \
    --socket=parallax-weston --debug --modules="$weston_build/tests/test-plugin.so" "$@" >"$work/weston.log" 2>&1 &
  trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$weston_runtime"' EXIT
  for _ in {1..30}; do [[ -S $weston_runtime/parallax-weston ]] && return 0; sleep 1; done
  echo "record-traces: Weston never started, see $work/weston.log" >&2
  return 1
}
# Relaunches hello with `env` arguments that pick the display server.
weston_launch() {
  [[ -z ${hello_pid:-} ]] || { kill "$hello_pid" 2>/dev/null; wait "$hello_pid" 2>/dev/null || true; }
  env "$@" "$repo/target/debug/hello" 2>"$work/hello.log" &
  hello_pid=$!
}
weston_log() { cat "$work/hello.log"; }
weston_screenshot() {
  rm -rf "$work/shots" && mkdir "$work/shots"
  on_weston env XDG_PICTURES_DIR="$work/shots" "$weston_build/clients/weston-screenshooter"
  cat "$work/shots"/*.png >"$1"
}
weston_scale() { echo "$weston_scale"; }
weston_frames() { env "${weston_env[@]}" python3 -I "$repo/scripts/weston-touch.py" "$@"; }
wayland_setup() { weston_setup; }
wayland_launch() { weston_launch -u DISPLAY "${weston_env[@]}"; }
# Xwayland scales X11 windows up rather than give them the output scale, so x11 records at scale 1 only.
x11_setup() {
  ((weston_scale == 1)) || { echo "record-traces: x11 records at WESTON_SCALE=1 only" >&2; return 1; }
  weston_setup --xwayland
}
x11_launch() {
  # Weston starts Xwayland on the first X client and names its display in its log.
  local display
  display=$(grep -o 'xserver listening on display :[0-9]*' "$work/weston.log" | grep -o ':[0-9]*$' | tail -1)
  weston_launch -u WAYLAND_DISPLAY DISPLAY="${display:?Weston logged no Xwayland display}"
}

# --- Windows: hello's desktop bin, touched through a synthetic pointer device by crates/touch-inject
# (InjectSyntheticPointerInput), so it gets real WM_POINTER messages and Windows' own mouse copies of them. Git Bash;
# MSYS2_ARG_CONV_EXCL keeps frames from path conversion.
windows_bin=$repo/target/debug
windows_setup() {
  cargo build -p hello -p touch-inject
  trap windows_stop EXIT
}
windows_inject() { MSYS2_ARG_CONV_EXCL='*' "$windows_bin/touch-inject.exe" "$hello_pid" "$@"; }
windows_stop() { [[ -z ${hello_pid:-} ]] || taskkill //F //PID "$hello_pid" >/dev/null 2>&1 || true; }
windows_launch() {
  windows_stop
  # The cursor off the window, so no stray mouse move reaches the pad.
  "$windows_bin/touch-inject.exe" park
  # The pid through a file, not `$(…)`: hello inherits touch-inject's handles, so a pipe would stay open while it runs.
  "$windows_bin/touch-inject.exe" launch "$(cygpath -w "$windows_bin/hello.exe")" "$(cygpath -w "$work/hello.log")" \
    >"$work/hello.pid"
  hello_pid=$(<"$work/hello.pid")
  # Above the runner's console window, which comes back on top now and then.
  windows_inject raise
}
windows_log() { cat "$work/hello.log"; }
windows_screenshot() { windows_inject screenshot "$(cygpath -w "$1")"; }
windows_scale() { windows_inject scale; }
windows_frames() { windows_inject touch "$@"; }

# Who logs, screenshots and touches for each platform: its own functions, or a shared driver's. To add a platform
# that shares one, add a row.
declare -A drivers=([wayland]=weston [x11]=weston)
driver=${drivers[$platform]:-$platform}

# --- The run: launch, enter the trace pad, perform the gesture, keep what the pad logged.
wait_for() {
  local attempt
  for attempt in {1..30}; do
    # Not `grep -q`: it stops reading at the match, and pipefail then fails on the log command's SIGPIPE.
    "${driver}_log" | grep "$1" >/dev/null && return 0
    sleep 1
  done
  echo "record-traces: $platform never logged \"$1\"" >&2
  return 1
}

# "x y", logical px, of the centre of the topmost band of colour $1 on screen.
locate() {
  "${driver}_screenshot" "$work/screen.png"
  python3 -I "$repo/scripts/locate-colour.py" "$1" <"$work/screen.png" |
    awk -v s="$("${driver}_scale")" '{ printf "%d %d", $1 / s, $2 / s }'
}
# A fresh hello on its main screen, drawn: "window open" comes before the first frame, which a slow CI runner can take
# seconds to draw, and a tap before it hits nothing.
launch_main() {
  local attempt
  "${platform}_launch"
  wait_for "window open"
  for attempt in {1..30}; do
    if locate "$record_button" >/dev/null 2>&1; then
      sleep 0.5
      return 0
    fi
    sleep 1
  done
  echo "record-traces: $platform never drew the record button" >&2
  return 1
}
# A fresh hello on its trace pad; `button` is where the "Record touches" button is.
launch_pad() {
  launch_main
  # Unquoted on purpose: $button is "x y".
  perform tap $button 50
  wait_for "trace: recording"
}

[[ ${BASH_SOURCE[0]} == "$0" ]] || return 0

"${platform}_setup"
launch_main
button=$(locate "$record_button")

# One tap is one click: GPUI makes the click from the touch, and the platform must add no mouse events of its own.
perform tap $(locate "$box_colour") 50
sleep 1
clicks=$("${driver}_log" | grep -o 'box colour.*' || true)
if [[ $clicks != *"(touch click, count 1)" || $(wc -l <<<"$clicks") != 1 ]]; then
  echo "record-traces: one tap on the box logged: ${clicks:-nothing}" >&2
  exit 1
fi

wanted=" $* "
for row in "${gestures[@]}"; do
  IFS='|' read -r name command <<<"$row"
  [[ $# == 0 || $wanted == *" $name "* ]] || continue
  launch_pad
  read -r verb args <<<"$command"
  # Unquoted on purpose: $args is the verb's numbers.
  if ! perform "$verb" $args; then
    echo "record-traces: $platform $name skipped"
    continue
  fi
  sleep 1.5
  # Written aside first: a run that logged no touches (the pad missed the gesture) keeps the old fixture.
  trace=$work/$name.trace
  {
    echo "# $platform $name: $command"
    "${driver}_log" | grep -o 'touch([^)]*)' || true
  } >"$trace"
  events=$(($(wc -l <"$trace") - 1))
  if ((events == 0)); then
    echo "record-traces: $platform $name: no events, $fixtures/$name.trace kept" >&2
    continue
  fi
  mv "$trace" "$fixtures/$name.trace"
  echo "record-traces: $platform $name: $events events"
done
