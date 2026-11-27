#!/usr/bin/env bash
# Runtime checks of Windows touch and pen (patch 0021, docs/desktop-touch-windows.md): injects touch, pen and mouse
# input into examples/hello through crates/touch-inject and asserts on what hello logged. Each check relaunches hello,
# on its main screen or its trace pad, runs one command and checks the log; logs land in target/record-traces/windows/checks/.
#
# Usage: scripts/windows-touch-checks.sh [check…]     (default: every row of the checks table)
# Needs: what record-traces.sh windows needs (Git Bash on Windows 10 1809+ with a desktop session, python3).
set -euo pipefail

# The gestures table, the frame builders, the Windows driver, launch_pad and locate.
# shellcheck source=record-traces.sh
source "$(dirname "${BASH_SOURCE[0]}")/record-traces.sh" windows
checks_dir=$work/checks
mkdir -p "$checks_dir"

# Frames beyond the gesture table's verbs, in its syntax (frames.rs in crates/touch-inject has the hover and cancel
# fingers). A pen moving in range above x1 y1 → x2 y2 over ms, never touching:
frames_hover() {
  local i n=$(($5 / frame_ms))
  for ((i = 0; i <= n; i++)); do echo "$frame_ms 0:$(lerp "$1" "$3" $i $n):$(lerp "$2" "$4" $i $n):hover"; done
  echo "0 0:up"
}
# A finger held at x y for ms, then cancelled (POINTER_FLAG_CANCELED):
frames_cancel() { printf '%s\n' "$3 0:$1:$2" "0 0:cancel"; }

# Commands a check runs, besides `perform <verb> …` (a touch):
gesture() {
  local row verb args
  for row in "${gestures[@]}"; do
    [[ ${row%%|*} == "$1" ]] || continue
    read -r verb args <<<"${row#*|}"
    # Unquoted on purpose: $args is the verb's numbers.
    perform "$verb" $args
    return
  done
  echo "windows-touch-checks: no gesture $1" >&2
  return 1
}
pen() {
  local verb=$1 frames
  shift
  mapfile -t frames < <("frames_$verb" "$@")
  windows_inject pen "${frames[@]}"
}
click() { windows_inject click "$@"; }

# The checks: name | screen (main or pad; pad@150: at 150 % display scale) | command (BOX: the box's centre) |
# assertions, each kind:argument:
#   lines:<regex>:<n>   log lines matching the regex (spaces written as .): exactly n, or n+ for at least n
#   all:<regex>         every touch(…) line matches          near:<x>:<y>   the first touch is within 1 px of x y
#   ids:<n>             n distinct touch ids                 unique         no touch line twice
#   rising              per id: Started, Moved…, Ended, time stamps increasing, strictly between moves
#   moved               the window's frame moved
# To add a check: a row here (and an assert_<kind> for a new kind).
checks=(
  "tap|pad|gesture tap|lines:Started,:1 lines:Ended,:1 lines:mouse_down:0 near:200:400"
  "touch-click|main|perform tap BOX 50|lines:touch.click,.count.1:1 lines:box.colour:1"
  "drag-order|pad|gesture slow-drag|rising lines:Moved,:200+ ids:1"
  "fling-order|pad|gesture fling|rising lines:Moved,:5+ ids:1"
  "pinch|pad|gesture pinch|ids:2 unique lines:Started,:2 lines:Ended,:2"
  "reused-id|pad|gesture double-tap|ids:2 lines:Started,:2 lines:Ended,:2"
  "cancel|pad|perform cancel 200 400 100|lines:Cancelled,:1 lines:Ended,:0"
  "pen|pad|pen swipe 200 600 200 400 300|all:Pen\) lines:Moved,.*Pen\).force.0\.50:15+ rising lines:Started,:1 lines:Ended,:1"
  "pen-hover|pad|pen hover 100 300 300 300 300|lines:touch\(:0 lines:mouse_move:5+ lines:mouse_down:0"
  "title-bar|pad|perform swipe 200 -14 300 -14 300|moved lines:touch\(:0"
  "mouse|pad|click 200 400|lines:mouse_down.*device:.Mouse,.synthesized:.false:1 lines:touch\(:0"
  "tap-150|pad@150|perform tap 120 300 50|near:120:300 lines:Started,:1 lines:Ended,:1"
  "drag-150|pad@150|perform swipe 100 200 300 500 300|near:100:200 rising"
)
# The resolution a scaled check runs at: Windows offers 150 % only from about 1920 × 1080 (touch-inject's `display`).
scaled_display="1920 1080"

# The touch lines as "at id phase x y device", one a line.
touches() { grep -o 'touch([^)]*)' "$log" | sed 's/touch(\(.*\))/\1/; s/,//g' || true; }
assert_lines() {
  local want=${1##*:} got
  got=$(grep -cE "${1%:*}" "$log" || true)
  if [[ $want == *+ ]]; then ((got >= ${want%+})); else ((got == want)); fi || { echo "$got lines"; return 1; }
}
assert_all() {
  local bad
  bad=$(grep -o 'touch(.*' "$log" | grep -cvE "$1" || true)
  ((bad == 0)) || { echo "$bad touch lines do not match"; return 1; }
}
assert_near() {
  touches | head -1 | awk -v x="${1%:*}" -v y="${1#*:}" '
    function d(a, b) { return a > b ? a - b : b - a }
    { at = $4 " " $5; exit !(d($4, x) <= 1 && d($5, y) <= 1) }
    END { if (at == "") exit 1 }' || { echo "first touch at $(touches | head -1)"; return 1; }
}
assert_ids() {
  local got
  got=$(touches | awk '{ print $2 }' | sort -u | wc -l)
  ((got == $1)) || { echo "$got ids"; return 1; }
}
assert_unique() {
  local twice
  twice=$(touches | sort | uniq -d)
  [[ -z $twice ]] || { echo "delivered twice: $twice"; return 1; }
}
assert_rising() {
  touches | awk '
    { id = $2 }
    !(id in last) && $3 != "Started" { print "id " id " begins " $3; bad = 1 }
    (id in done) { print "id " id " after its end"; bad = 1 }
    $3 == "Started" && (id in last) { print "id " id " starts twice"; bad = 1 }
    $3 == "Moved" && (id in last) && $1 <= last[id] && seen_move[id] { print "id " id " move at " $1 " ms after " last[id]; bad = 1 }
    (id in last) && $1 < last[id] { print "id " id " goes back to " $1 " ms"; bad = 1 }
    $3 == "Moved" { seen_move[id] = 1 }
    $3 == "Ended" || $3 == "Cancelled" { done[id] = 1 }
    { last[id] = $1 }
    END { for (id in last) if (!(id in done)) { print "id " id " never ends"; bad = 1 }; exit bad }'
}
assert_moved() {
  [[ $(windows_inject window-rect) != "$frame_before" ]] || { echo "window still at $frame_before"; return 1; }
}

windows_setup
launch_main
button=$(locate "$record_button")
box=$(locate 3b82f6)

wanted=" $* "
failed=()
for row in "${checks[@]}"; do
  IFS='|' read -r name screen command assertions <<<"$row"
  [[ $# == 0 || $wanted == *" $name "* ]] || continue
  IFS=@ read -r screen percent <<<"$screen"
  [[ -z $percent ]] || display_was=$("$windows_bin/touch-inject.exe" display $scaled_display "$percent")
  "launch_$screen"
  frame_before=$(windows_inject window-rect)
  read -ra words <<<"${command//BOX/$box}"
  "${words[@]}"
  sleep 1.5
  log=$checks_dir/$name.log
  windows_log >"$log"
  verdict=PASS
  for assertion in $assertions; do
    if ! why=$("assert_${assertion%%:*}" "${assertion#*:}"); then
      verdict=FAIL
      echo "windows-touch-checks: $name: $assertion: $why"
    fi
  done
  echo "windows-touch-checks: $verdict $name ($(touches | wc -l) touches)"
  [[ $verdict == PASS ]] || failed+=("$name")
  # Unquoted on purpose: "width height percent".
  [[ -z $percent ]] || "$windows_bin/touch-inject.exe" display $display_was >/dev/null
done
((${#failed[@]} == 0)) || { echo "windows-touch-checks: failed: ${failed[*]}"; exit 1; }
