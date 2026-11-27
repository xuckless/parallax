#!/usr/bin/env bash
# Smoke test of examples/hello on one running Android device or emulator: builds, installs and launches it with
# cargo-parallax, drives it with adb, and after each step waits for the logcat line that proves the step worked.
# Runs the same locally and in CI (docs/android-smoke.md).
#
# Usage: scripts/android-smoke.sh [serial]     (default: the only device in `adb devices`)
# Needs: ANDROID_HOME with platform-tools and an NDK, Java 17+, python3 (see docs/cli.md, Requirements).
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck source=hello-android.sh
source "$repo/scripts/hello-android.sh"
adb="${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}/platform-tools/adb"
app=$hello_app
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

on_device() { "$adb" -s "$serial" shell "$@"; }

# Logcat is polled, never streamed: a streaming `adb logcat` stalled for good on a CI runner. A mark is the time of
# the newest line ("MM-DD hh:mm:ss.mmm"); log_since prints the lines logged at or after it. At, not only after: lines
# logged in the mark's millisecond count too, so a step's line is never missed; the mark line itself may come back,
# so two steps in a row never wait for the same pattern. The marks compare as text, which breaks across New Year
# (12-31 → 01-01): a run spanning midnight on 31 December can fail a step.
log_mark() { on_device logcat -d -t 1 -v time | tail -n 1 | cut -c1-18; }
log_since() {
  on_device logcat -d -b main,system,crash -v time -T "\"$1\"" | awk -v mark="$1" 'substr($0, 1, 18) >= mark'
}

# Taps the centre of the topmost band of the screen painted in colour $1 (RGB hex), found by locate-colour.py on a raw
# screencap. Looks again each second for up to 10 s, while a splash screen, keyboard or rotation settles.
tap_colour() {
  local point attempt
  for attempt in {1..10}; do
    point=$("$adb" -s "$serial" exec-out screencap | python3 -I "$repo/scripts/locate-colour.py" "$1" 2>/dev/null) && break
    ((attempt < 10)) || { echo "android-smoke: no band of colour $1 on screen" >&2; return 1; }
    sleep 1
  done
  # Unquoted on purpose: $point is "x y", two arguments.
  on_device input tap $point
}

# A system dialog over the app (another app's "isn't responding" or crash dialog) takes the taps meant for hello.
# hide_error_dialogs (the settings table) stops new ones but does not close one already up: a booted snapshot or a
# launcher that stalled while the emulator booted brings one along. Before each step: while a dialog has focus, stop
# the app it is for (its dialog closes with its process) and close system dialogs. A dialog for hello itself is left
# alone: "ANR in $app" in $crashed fails the step.
dismiss_dialogs() {
  local focus process attempt
  for attempt in {1..5}; do
    focus=$(on_device dumpsys window | grep -m 1 'mCurrentFocus=' || true)
    [[ $focus =~ (Application Not Responding|Application Error):\ ([A-Za-z0-9_.:]+) ]] || return 0
    process=${BASH_REMATCH[2]}
    [[ ${process%%:*} == "$app" ]] && return 0
    echo "android-smoke: closing a system dialog (try $attempt): ${BASH_REMATCH[1]}: $process"
    on_device am force-stop "${process%%:*}" || true
    on_device am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null || true
    sleep 1
  done
  echo "android-smoke: a system dialog still has focus after 5 tries: $focus" >&2
}

# The display's rotation (0-3) as the window manager reports it; dumpsys prints ROTATION_90 or 1 by Android version.
display_rotation() {
  local rotation
  rotation=$(on_device dumpsys window displays | grep -m 1 -oE 'mCurrentRotation=(ROTATION_)?[0-9]+' | grep -oE '[0-9]+$' || true)
  case $rotation in 90) echo 1 ;; 180) echo 2 ;; 270) echo 3 ;; *) echo "$rotation" ;; esac
}

# Turns the display to rotation $1 (auto-rotate is off: the settings table) and waits up to 10 s for the window
# manager to report it. One CI emulator ignored the settings write and never changed configuration, so a second try
# writes it again and also asks the window manager directly (`wm user-rotation lock`, API 30+).
rotate() {
  local attempt
  for attempt in 1 2; do
    on_device settings put system user_rotation "$1"
    ((attempt == 1)) || on_device wm user-rotation lock "$1" || true
    for _ in {1..10}; do
      [[ $(display_rotation) == "$1" ]] && return 0
      sleep 1
    done
    echo "android-smoke: the display is at rotation $(display_rotation), not $1, after 10 s (try $attempt of 2)" >&2
  done
  echo "android-smoke: the emulator did not rotate to $1 (user_rotation, then wm user-rotation lock)" >&2
  return 1
}

# The smoke run, in order: one `step` per row, with the step name, the command, the extended regex a new logcat line
# must match, and the seconds to wait. To add a step, add a row; the command may use on_device (adb shell), tap_colour,
# rotate and hello_android_run (hello-android.sh). Rows are trusted code: the command runs through `eval`. Double
# quotes expand $serial, $typed, ${#typed}, $hello_component and $drawn here, when the row is defined; anything the
# command should expand when it runs goes in single quotes. The fields are joined with the unit separator \x1f, which no command or regex uses (`|` is regex alternation).
# The rotate and resume steps wait for the first frame drawn on a surface of the new orientation (window.rs logs it once
# per surface), not for the surface event: that is logged before the surface is made, and a late surfaceChanged from
# the previous rotation could satisfy the next step.
sep=$'\x1f'
steps=()
step() { steps+=("$1$sep$2$sep$3$sep$4"); }
drawn="parallax: first frame drawn on a [0-9]+x[0-9]+"
step "launch" "hello_android_run $serial" "$drawn portrait surface" 60
step "tap the box" "tap_colour 3b82f6" "box colour ef4444 \(touch click" 20
step "focus the field" "tap_colour ffffff" "insets in pixels: .*ime Edges .*bottom: [1-9]" 20
step "type and submit" "on_device input text $typed && on_device input keyevent KEYCODE_ENTER" "text field submitted: ${#typed} bytes" 20
step "rotate to landscape" "rotate 1" "$drawn landscape surface" 30
step "rotate to portrait" "rotate 0" "$drawn portrait surface" 30
step "background" "on_device input keyevent KEYCODE_HOME" "parallax: lifecycle From\(.*, Stop\)" 20
step "resume" "on_device am start -n $hello_component" "$drawn portrait surface" 20
step "tap the box again" "tap_colour ef4444" "box colour 22c55e \(touch click" 20

# Lines that mean the app crashed, hung or cannot draw (host.rs logs the last two), at any step.
crashed="FATAL EXCEPTION|Fatal signal|ANR in $app|Process $app .* has died"
crashed+="|parallax: cannot attach the new surface|parallax: GPU device recovery failed"

fail() {
  log_since "$start" >"$log" || true
  echo "android-smoke: FAIL: $*" >&2
  echo "android-smoke: last app and crash lines from logcat:" >&2
  grep -E "parallax|AndroidRuntime|DEBUG|ActivityManager|$app" "$log" | tail -n 60 >&2 || true
  "$adb" -s "$serial" exec-out screencap -p >"$out/failure.png" || true
  echo "android-smoke: full logcat and the screen: $out/" >&2
  exit 1
}

# Device settings the run changes: "namespace key value". To change another, add a row; it is restored on exit.
# hide_error_dialogs: a slow emulator can show "Pixel Launcher isn't responding" over the app; Android then shows no
# new crash or ANR dialogs (the logcat lines that catch our own crashes stay). One already up: dismiss_dialogs.
# accelerometer_rotation, user_rotation: portrait with auto-rotate off: tap_colour expects portrait, and the rotate
# steps are the only rotations (they change user_rotation again).
settings=(
  "global hide_error_dialogs 1"
  "system accelerometer_rotation 0"
  "system user_rotation 0"
)
# Each setting's value before the run, in the same order; "null" means it was unset.
originals=()
for setting in "${settings[@]}"; do
  read -r namespace key _ <<<"$setting"
  originals+=("$(on_device settings get "$namespace" "$key" | tr -d '\r')")
done
restore_settings() {
  local i namespace key
  for i in "${!settings[@]}"; do
    read -r namespace key _ <<<"${settings[i]}"
    if [[ ${originals[i]} == null ]]; then
      on_device settings delete "$namespace" "$key" >/dev/null || true
    else
      on_device settings put "$namespace" "$key" "${originals[i]}" || true
    fi
  done
}
trap restore_settings EXIT
for setting in "${settings[@]}"; do
  read -r namespace key value <<<"$setting"
  on_device settings put "$namespace" "$key" "$value"
done
start=$(log_mark)

for row in "${steps[@]}"; do
  IFS=$sep read -r name command pattern timeout <<<"$row"
  echo "android-smoke: $name"
  dismiss_dialogs
  since=$(log_mark)
  eval "$command" || fail "$name: command failed: $command"
  deadline=$((SECONDS + timeout))
  until log_since "$since" >"$log" && grep -Eq -- "$pattern" "$log"; do
    if grep -Eq -- "$crashed" "$log"; then fail "$name: the app crashed, stopped responding or lost its surface"; fi
    ((SECONDS < deadline)) || fail "$name: no logcat line matching /$pattern/ within ${timeout}s"
    sleep 1
  done
done

log_since "$start" >"$log"
grep -Eq -- "$crashed" "$log" && fail "the app crashed, stopped responding or lost its surface"
[[ -n $(on_device pidof "$app") ]] || fail "the app is no longer running"
echo "android-smoke: all ${#steps[@]} steps passed on $serial"
grep -m 1 "renderer created" "$log" || true  # which GPU drew, for comparing runs
