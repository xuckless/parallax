#!/usr/bin/env bash
# Runtime checks of NativeView on Windows (patches 0048 and 0050, docs/native-view-windows.md): runs
# examples/native-view-windows on the desktop, reads the composed screen back through crates/touch-inject, injects
# mouse input, and asserts on pixels and on what the example logged. Each scale relaunches the example; logs and
# screenshots land in target/native-view-windows/<percent>/.
#
# Usage: scripts/windows-native-view-checks.sh [percent…]     (default: 100 125, the display scales checked)
# Needs: Git Bash on Windows 10 1809+ with a desktop session, python3 (as CI's windows runners have).
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=$repo/target/debug
out=$repo/target/native-view-windows
percents=("${@:-100}")
[[ $# -gt 0 ]] || percents=(100 125)
# Windows offers 125 % only from a larger resolution than the runner's 1024 × 768.
scaled_display="1920 1080"
# The example's colours (src/app.rs): window, cover, gap; the view is green with little red (src/content.rs).
window_colour="48 48 48"
cover_colour="255 128 0"
gap_colour=ff00ff
failures=0

fail() {
  echo "FAIL [$percent]: $*"
  failures=$((failures + 1))
}
pass() { echo "pass [$percent]: $*"; }
inject() { MSYS2_ARG_CONV_EXCL='*' "$bin/touch-inject.exe" "$pid" "$@"; }
stop() { [[ -z ${pid:-} ]] || taskkill //F //PID "$pid" >/dev/null 2>&1 || true; }
trap stop EXIT
shot() { inject screenshot "$(cygpath -w "$1")"; }
pixel() { python3 -I "$repo/scripts/pixels.py" "$1" at "$2" "$3"; }
count() { python3 -I "$repo/scripts/pixels.py" "$1" count "$2"; }
lines() { grep -c -- "$1" "$work/example.log" || true; }

# A pixel's colour class: view (green, little red), or the r g b it is.
is_view() {
  local r g b
  read -r r g b <<<"$(pixel "$1" "$2" "$3")"
  ((r < 80 && g > 150))
}
is_colour() {
  local got
  got=$(pixel "$1" "$2" "$3")
  local -a want=($4) have=($got)
  local c
  for c in 0 1 2; do
    local d=$((have[c] - want[c]))
    ((d * d <= 24 * 24)) || return 1
  done
}

# Placement: the example's last "view placement: device l t r b" line.
placement() { grep 'view placement: device' "$work/example.log" | tail -1 | grep -o '[-0-9.]* [-0-9.]* [-0-9.]* [-0-9.]*$'; }

# Hole, stacking and seams on one screenshot: inside the view's uncovered part is the view, the cover over its corner
# is the cover (GPUI's visual is above the view's), and one device pixel past each edge is the window, none of it gap.
check_composition() {
  local file=$1 what=$2 l t r b
  read -r l t r b <<<"$(placement)"
  l=${l%.*} t=${t%.*} r=${r%.*} b=${b%.*}
  local mid_y=$(((t + b) / 2 + 20)) mid_x=$(((l + r) / 2 + 20))
  local ok=1
  is_view "$file" $((l + (r - l) * 3 / 4)) $(((t + b) / 2)) || { fail "$what: no view in the hole"; ok=0; }
  is_colour "$file" $((l + 5)) $((t + 5)) "$cover_colour" || { fail "$what: the cover is not over the view"; ok=0; }
  is_view "$file" "$l" "$mid_y" || { fail "$what: left edge column $l is not the view"; ok=0; }
  is_colour "$file" $((l - 1)) "$mid_y" "$window_colour" || { fail "$what: column $((l - 1)) left of the view is not the window"; ok=0; }
  is_view "$file" $((r - 1)) "$mid_y" || { fail "$what: right edge column $((r - 1)) is not the view"; ok=0; }
  is_colour "$file" "$r" "$mid_y" "$window_colour" || { fail "$what: column $r right of the view is not the window"; ok=0; }
  is_view "$file" "$mid_x" "$t" || { fail "$what: top edge row $t is not the view"; ok=0; }
  is_colour "$file" "$mid_x" $((t - 1)) "$window_colour" || { fail "$what: row $((t - 1)) above the view is not the window"; ok=0; }
  is_view "$file" "$mid_x" $((b - 1)) || { fail "$what: bottom edge row $((b - 1)) is not the view"; ok=0; }
  is_colour "$file" "$mid_x" "$b" "$window_colour" || { fail "$what: row $b below the view is not the window"; ok=0; }
  local gaps
  gaps=$(count "$file" "$gap_colour")
  ((gaps == 0)) || { fail "$what: $gaps gap pixels"; ok=0; }
  ((ok == 0)) || pass "$what: hole, cover above the view, no seams at $l $t $r $b, no gap"
}

# Mouse input over the view reaches its element with positions in the view; the cover keeps its own.
check_input() {
  local x=241 y=191 before
  inject move $x $y
  sleep 0.5
  # Within a pixel of 200 150: the cursor lands on a whole physical pixel.
  grep 'view input: move at' "$work/example.log" | tail -1 | awk '{ x = $(NF-1) - 200; y = $NF - 150;
    exit !(x * x <= 1.6 && y * y <= 1.6) }' && pass "a move over the view arrives at 200 150 in the view" \
    || fail "no move at 200 150 in the view"
  inject scroll $x $y -1
  sleep 0.5
  [[ $(lines 'view input: scroll at') -ge 1 ]] && pass "scroll over the view reaches its element" || fail "no scroll"
  inject click $x $y
  sleep 0.5
  [[ $(lines 'view input: down at') -eq 1 ]] && pass "a click on the view reaches its element" || fail "no click"
  before=$(lines 'view input: down at')
  inject click 61 61
  sleep 0.5
  [[ $(lines 'cover: down') -eq 1 && $(lines 'view input: down at') -eq $before ]] \
    && pass "the cover takes its own click" || fail "the cover's click went wrong"
}

# 0 GPUI frames while only the view presents: the last three per-second lines of a still window.
check_idle() {
  local idle
  idle=$(grep 'per second:' "$work/example.log" | tail -3)
  echo "$idle"
  if grep -q 'gpui frames [1-9]' <<<"$idle" || ! grep -q 'view presents [1-9][0-9]' <<<"$idle"; then
    fail "GPUI drew while only the view presented"
  else
    pass "0 GPUI frames per second while the view presents"
  fi
}

cargo build -p native-view-windows -p touch-inject
for percent in "${percents[@]}"; do
  work=$out/$percent
  rm -rf "$work"
  mkdir -p "$work"
  display_was=
  [[ $percent == 100 ]] || display_was=$("$bin/touch-inject.exe" display $scaled_display "$percent")
  "$bin/touch-inject.exe" park
  "$bin/touch-inject.exe" launch "$(cygpath -w "$bin/native-view-windows.exe")" "$(cygpath -w "$work/example.log")" \
    >"$work/pid"
  pid=$(<"$work/pid")
  inject raise
  sleep 6
  echo "scale $(inject scale)"
  grep -q 'alpha mode Premultiplied' "$work/example.log" && pass "alpha mode logged: Premultiplied while the view shows" \
    || fail "no alpha mode logged"
  check_idle
  shot "$work/still.raw"
  check_composition "$work/still.raw" "still"
  check_input
  # Moving: GPUI draws every step, the view's placement goes in each frame's own commit.
  # Shots as fast as touch-inject takes them while the view slides; counted after, so they sample more frames.
  inject click 550 380
  shots=20
  for i in $(seq 1 $shots); do shot "$work/move-$i.raw"; done
  sleep 2
  grep -q 'motion: end' "$work/example.log" || fail "the move never ended"
  out_of_step=0
  for i in $(seq 1 $shots); do
    gaps=$(count "$work/move-$i.raw" "$gap_colour")
    ((gaps == 0)) || { echo "moving, shot $i: $gaps gap pixels"; out_of_step=$((out_of_step + 1)); }
  done
  ((out_of_step == 0)) && pass "moved: no gap in $shots shots" \
    || fail "moving: hole and view out of step in $out_of_step of $shots shots"
  # Resizing: the view's own content follows a frame late, so the gap colour may show only until it does.
  inject click 550 440
  sleep 1.5
  shot "$work/resized.raw"
  check_composition "$work/resized.raw" "resized"
  sleep 3
  check_idle
  stop
  pid=
  [[ -z $display_was ]] || "$bin/touch-inject.exe" display $display_was >/dev/null
done
echo "$failures failure(s)"
((failures == 0))
