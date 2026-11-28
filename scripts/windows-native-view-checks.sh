#!/usr/bin/env bash
# Runtime checks of NativeView on Windows (patches 0048, 0050 and 0056, docs/native-view-windows.md): runs
# examples/native-view-windows (a composition swap chain) and examples/native-view-webview2 (a WebView2 page) on the
# desktop, reads the composed screen back through crates/touch-inject, injects mouse and key input, and asserts on
# pixels and on what the examples logged. Each scale relaunches each example; logs and screenshots land in
# target/native-view-windows/<percent>/ and target/native-view-windows/<percent>/webview2/.
#
# Usage: scripts/windows-native-view-checks.sh [percent…]     (default: 100 125, the display scales checked)
# Needs: Git Bash on Windows 10 1809+ with a desktop session, python3 (as CI's windows runners have), and for the
# WebView2 example the WebView2 runtime (CI installs it when the image lacks it, docs/ci.md).
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=$repo/target/debug
out=$repo/target/native-view-windows
percents=("${@:-100}")
[[ $# -gt 0 ]] || percents=(100 125)
# Windows offers 125 % only from a larger resolution than the runner's 1024 × 768.
scaled_display="1920 1080"
# The examples' colours (src/app.rs): window, cover, gap; the view is green with little red (the swap chain example's
# src/content.rs, the WebView2 example's src/page.html; the swap chain's blue changes every frame).
window_colour="48 48 48"
cover_colour="255 128 0"
gap_colour=ff00ff
view_colour=10c0**
cover_hex=ff8000
# Screenshots while the view slides: the owner's bar for "hole and view in step" (2026-11-27).
moving_shots=100
# The surface's line per GPUI frame copied (gpui_windows' native_view_surface.rs).
copied='GPUI frame copied into the surface'
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

# Starts example $1 with its log in $work, the mouse off its window and the window on top.
launch() {
  "$bin/touch-inject.exe" park
  "$bin/touch-inject.exe" launch "$(cygpath -w "$bin/$1.exe")" "$(cygpath -w "$work/example.log")" >"$work/pid"
  pid=$(<"$work/pid")
  inject raise
}

# Waits up to $2 seconds for a line matching $1 in the log.
wait_for() {
  local tries=$(($2 * 4))
  until grep -q -- "$1" "$work/example.log"; do
    ((tries-- > 0)) || return 1
    sleep 0.25
  done
}

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

# Mouse input over the swap chain view reaches its element with positions in the view; the cover keeps its own.
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

# 0 GPUI frames, so 0 copies, while only the view (or page) draws: the last three per-second lines of a still
# window, and no surface copy over those three seconds. $1 matches the view's own per-second count.
check_idle() {
  local idle copies_before
  copies_before=$(lines "$copied")
  sleep 3
  idle=$(grep 'per second:' "$work/example.log" | tail -3)
  echo "$idle"
  if grep -q 'gpui frames [1-9]' <<<"$idle" || ! grep -q "$1" "$work/example.log"; then
    fail "GPUI drew while only the view drew"
  elif (($(lines "$copied") != copies_before)); then
    fail "GPUI copied its frame while still"
  else
    pass "0 GPUI frames and 0 copies per second while only the view draws"
  fi
}

# While the view slides, every screenshot shows hole and view in step, and GPUI copies its frame once per frame.
check_moving() {
  local gaps out_of_step=0 frames copies
  inject click 550 380
  wait_for 'motion: start' 5 || fail "the move never started"
  inject shots "$moving_shots" "$(cygpath -w "$work/move-")"
  grep -q 'motion: end' "$work/example.log" && fail "the view stopped before the last of $moving_shots shots"
  wait_for 'motion: end' 30 || fail "the move never ended"
  # The view's visible part in device px: its size, less the cover over its corner (src/app.rs).
  local scale view_area gaps views covers
  scale=$(inject scale)
  view_area=$(python3 -c "s = $scale; print(round((322 * 242 - 100 * 60) * s * s))")
  for i in $(seq 1 "$moving_shots"); do
    # One read per shot: no gap, and the view's content and the cover there, so a blank shot can't pass.
    read -r gaps views covers <<<"$(python3 -I "$repo/scripts/pixels.py" "$work/move-$i.raw" counts "$gap_colour" \
      "$view_colour" "$cover_hex")"
    if ((gaps == 0 && views * 10 >= view_area * 8 && covers > 0)); then
      rm "$work/move-$i.raw"
    else
      echo "moving, shot $i: $gaps gap, $views view (of $view_area), $covers cover pixels (kept)"
      out_of_step=$((out_of_step + 1))
    fi
  done
  ((out_of_step == 0)) && pass "moved: the view, the cover and no gap in $moving_shots shots" \
    || fail "moving: hole and view out of step in $out_of_step of $moving_shots shots"
  # GPUI frames (one placement line each) and copies between the motion's start and end.
  read -r frames copies < <(awk -v copied="$copied" '/motion: start/ { on = 1 } /motion: end/ { on = 0 }
    on && /view placement: device/ { f++ } on && index($0, copied) { c++ } END { print f + 0, c + 0 }' \
    "$work/example.log")
  echo "moving: $frames GPUI frames, $copies surface copies"
  ((frames > 0 && copies >= frames - 1 && copies <= frames + 1)) \
    && pass "moving: one copy of GPUI's frame per GPUI frame ($copies for $frames)" \
    || fail "moving: $copies copies for $frames GPUI frames"
}

swap_chain_view() {
  launch native-view-windows
  sleep 6
  echo "scale $(inject scale)"
  grep -q 'alpha mode Premultiplied' "$work/example.log" \
    && pass "alpha mode logged: GPUI's surface Premultiplied while the view shows" || fail "no alpha mode logged"
  check_idle 'view presents [1-9][0-9]'
  shot "$work/still.raw"
  check_composition "$work/still.raw" "still"
  check_input
  check_moving
  # Resizing: the view's own content follows a frame late, so the gap colour may show only until it does.
  inject click 550 440
  sleep 1.5
  shot "$work/resized.raw"
  check_composition "$work/resized.raw" "resized"
  check_idle 'view presents [1-9][0-9]'
  stop
  pid=
}

# The WebView2 page: it renders in the hole, the mouse, wheel and keys over it reach the page, the cover keeps its
# click, and GPUI draws nothing while only the page animates.
webview2_view() {
  work=$work/webview2
  mkdir -p "$work"
  launch native-view-webview2
  if ! wait_for 'page: loaded' 60; then
    fail "webview2: the page never loaded"
    grep 'webview' "$work/example.log" || true
    stop
    pid=
    return
  fi
  pass "webview2: the page loaded into GPUI's DirectComposition tree"
  sleep 2
  shot "$work/still.raw"
  check_composition "$work/still.raw" "webview2 still"
  local x=241 y=191
  inject move $x $y
  sleep 0.5
  grep 'page: move at' "$work/example.log" | tail -1 | awk '{ x = $(NF-1) - 200; y = $NF - 150;
    exit !(x * x <= 1.6 && y * y <= 1.6) }' && pass "webview2: a move over the view reaches the page at 200 150" \
    || fail "webview2: no move at 200 150 in the page"
  inject click $x $y
  sleep 0.5
  grep 'page: down at' "$work/example.log" | tail -1 | awk '{ x = $(NF-1) - 200; y = $NF - 150;
    exit !(x * x <= 1.6 && y * y <= 1.6) }' && pass "webview2: a click on the view reaches the page at 200 150" \
    || fail "webview2: no click at 200 150 in the page"
  inject scroll $x $y -1
  sleep 0.5
  [[ $(lines 'page: wheel 1') -ge 1 ]] && pass "webview2: the wheel over the view reaches the page" \
    || fail "webview2: no wheel in the page"
  # VK_A, into the page, which took the keyboard focus with the click.
  inject key 65
  sleep 0.5
  [[ $(lines 'page: key a') -ge 1 ]] && pass "webview2: a key after the click reaches the page" \
    || fail "webview2: no key in the page"
  local downs
  downs=$(lines 'page: down at')
  inject click 61 61
  sleep 0.5
  [[ $(lines 'cover: down') -eq 1 && $(lines 'page: down at') -eq $downs ]] \
    && pass "webview2: the cover takes its own click" || fail "webview2: the cover's click went wrong"
  shot "$work/after-input.raw"
  check_composition "$work/after-input.raw" "webview2 after input"
  # Off the page, so the hover's own frames are over before the three idle seconds.
  inject move 550 400
  sleep 1.5
  check_idle 'page: frames per second [1-9][0-9]'
  grep 'page: frames per second' "$work/example.log" | tail -1
  stop
  pid=
}

cargo build -p native-view-windows -p native-view-webview2 -p touch-inject
for percent in "${percents[@]}"; do
  work=$out/$percent
  rm -rf "$work"
  mkdir -p "$work"
  display_was=
  [[ $percent == 100 ]] || display_was=$("$bin/touch-inject.exe" display $scaled_display "$percent")
  swap_chain_view
  webview2_view
  [[ -z $display_was ]] || "$bin/touch-inject.exe" display $display_was >/dev/null
done
echo "$failures failure(s)"
((failures == 0))
