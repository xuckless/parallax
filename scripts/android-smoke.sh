#!/usr/bin/env bash
# Smoke test of examples/hello on one running Android device or emulator: builds, installs and launches it with
# cargo-parallax, drives it with adb, and after each step waits for the logcat line that proves the step worked.
# Runs the same locally and in CI (docs/android-smoke.md).
#
# Usage: scripts/android-smoke.sh [serial]     (default: the only device in `adb devices`)
# Needs: ANDROID_HOME with platform-tools and an NDK, Java 17+, python3 (see docs/cli.md, Requirements).
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
adb="${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}/platform-tools/adb"
app=io.github.xuckless.parallax.hello
component=$app/io.github.xuckless.parallax.ParallaxActivity
# Typed into the prose field; the app logs only its length, never the text (docs/examples.md).
typed=smoke

serial=${1:-}
if [[ -z $serial ]]; then
  devices=$("$adb" devices | awk 'NR > 1 && $2 == "device" { print $1 }')
  [[ $(wc -w <<<"$devices") == 1 ]] || { echo "android-smoke: need exactly one adb device, or pass its serial; found: ${devices:-none}" >&2; exit 1; }
  serial=$devices
fi

# The run's logcat, and the screen when a step fails; kept after the run (CI uploads it on failure).
out=$repo/target/android-smoke
log=$out/logcat.txt
mkdir -p "$out"
rm -f "$out/failure.png"
logcat_pid=
trap '[[ -n $logcat_pid ]] && kill "$logcat_pid" 2>/dev/null' EXIT

on_device() { "$adb" -s "$serial" shell "$@"; }

# Taps the centre of the topmost horizontal band of the screen painted in colour $1 (RGB hex): the band is the rows
# where at least a quarter of the sampled pixels match, so status bar icons and glyphs never count. A pixel matches
# when it is the colour scaled by 0.5 to 1.1: the emulator's screen shows hello's colours dimmed (about 0.88).
# Looks again each second for up to 10 s, while a splash screen, keyboard or rotation settles.
locate='
import struct, sys
data = sys.stdin.buffer.read()
width, height = struct.unpack_from("<II", data)
pixels = data[len(data) - width * height * 4:]  # RGBA_8888 after a 12- or 16-byte header
want = bytes.fromhex(sys.argv[1])
step = 8

def matches(pixel):
    scale = sum(pixel) / sum(want)
    return 0.5 <= scale <= 1.1 and all(abs(p - scale * w) <= 8 for p, w in zip(pixel, want))

band = []  # (y, mean x) of matching rows in the first band
for y in range(0, height, step):
    row = [x for x in range(0, width, step) if matches(pixels[(y * width + x) * 4:(y * width + x) * 4 + 3])]
    if len(row) * step * 4 >= width:
        band.append((y, sum(row) // len(row)))
    elif band:
        break
if not band:
    sys.exit("no band of colour " + sys.argv[1] + " on screen")
print(band[len(band) // 2][1], band[len(band) // 2][0])
'
tap_colour() {
  local point attempt
  for attempt in {1..10}; do
    point=$("$adb" -s "$serial" exec-out screencap | python3 -I -c "$locate" "$1" 2>/dev/null) && break
    ((attempt < 10)) || { echo "android-smoke: no band of colour $1 on screen" >&2; return 1; }
    sleep 1
  done
  on_device input tap $point
}

launch() {
  cargo build --quiet -p cargo-parallax --manifest-path "$repo/Cargo.toml"
  (cd "$repo/examples/hello" && "$repo/target/debug/cargo-parallax" run android --device "$serial")
}

# The smoke run, in order: step name | command | extended regex a new logcat line must match | seconds to wait.
# To add a step, add a row; the command may use on_device (adb shell), tap_colour and launch.
steps=(
  "launch|launch|parallax: first frame drawn|60"
  "tap the box|tap_colour 3b82f6|box colour ef4444 \(touch click|20"
  "focus the field|tap_colour ffffff|insets in pixels: .*ime Edges .*bottom: [1-9]|20"
  "type and submit|on_device input text $typed && on_device input keyevent KEYCODE_ENTER|text field submitted: ${#typed} bytes|20"
  "rotate to landscape|on_device settings put system user_rotation 1|parallax: surface .*Resize|30"
  "rotate to portrait|on_device settings put system user_rotation 0|parallax: surface .*Resize|30"
  "background|on_device input keyevent KEYCODE_HOME|parallax: lifecycle From\(.*, Stop\)|20"
  "resume|on_device am start -n $component|parallax: surface .*Attach|20"
  "tap the box again|tap_colour ef4444|box colour 22c55e \(touch click|20"
)

# Lines that mean the app crashed or hung, at any step.
crashed="FATAL EXCEPTION|Fatal signal|ANR in $app|Process $app .* has died"

fail() {
  echo "android-smoke: FAIL: $*" >&2
  echo "android-smoke: last app and crash lines from logcat:" >&2
  grep -E "parallax|AndroidRuntime|DEBUG|ActivityManager|$app" "$log" | tail -n 60 >&2 || true
  "$adb" -s "$serial" exec-out screencap -p >"$out/failure.png" || true
  echo "android-smoke: full logcat and the screen: $out/" >&2
  exit 1
}

# A slow emulator can show "Pixel Launcher isn't responding" over the app; Android then shows no crash or ANR
# dialogs at all (the logcat lines that catch our own crashes stay).
on_device settings put global hide_error_dialogs 1
# Portrait, auto-rotate off. Set well before the rotate steps: turning auto-rotate off makes Android store the current
# rotation as user_rotation a moment later, which would undo a user_rotation written right after it.
on_device settings put system accelerometer_rotation 0
on_device settings put system user_rotation 0
on_device logcat -b all -c
: >"$log"  # exists before the first step reads it, however late logcat starts
"$adb" -s "$serial" logcat -b main,system,crash -v time >"$log" &
logcat_pid=$!

for row in "${steps[@]}"; do
  IFS='|' read -r name command pattern timeout <<<"$row"
  echo "android-smoke: $name"
  from=$(($(wc -l <"$log") + 1))
  eval "$command" || fail "$name: command failed: $command"
  deadline=$((SECONDS + timeout))
  # Not `tail | grep -q`: grep stops at the first match, tail dies of SIGPIPE, and pipefail calls that a miss.
  until grep -Eq -- "$pattern" <(tail -n "+$from" "$log"); do
    if grep -Eq -- "$crashed" <(tail -n "+$from" "$log"); then fail "$name: the app crashed or stopped responding"; fi
    ((SECONDS < deadline)) || fail "$name: no logcat line matching /$pattern/ within ${timeout}s"
    sleep 1
  done
done

grep -Eq -- "$crashed" "$log" && fail "the app crashed or stopped responding"
[[ -n $(on_device pidof "$app") ]] || fail "the app is no longer running"
echo "android-smoke: all ${#steps[@]} steps passed on $serial"
grep -m 1 "renderer created" "$log" || true  # which GPU drew, for comparing runs
