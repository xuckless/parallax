#!/usr/bin/env bash
# Live checks of hello's text field selection under Weston's headless backend (Wayland): mouse clicks, multi-clicks,
# drags and shift-clicks, and a touch long press and handle drag, sent through Weston's test plugin (weston-touch.py),
# then the field's last `text field selection: base..extent` log line checked (docs/text-selection.md#live-checks).
# Each check relaunches hello, types the text into its first field and finds the glyphs on screen; logs land in
# target/record-traces/wayland/selection/.
#
# Usage: scripts/weston-selection-checks.sh [check…]     (default: every row of the checks table)
# Needs: what record-traces.sh wayland needs (Weston's build dependencies, python3; WESTON_RUN, WESTON_SCALE).
set -euo pipefail

# The Weston driver, the touch frame builders, launch_main and locate.
# shellcheck source=record-traces.sh
source "$(dirname "${BASH_SOURCE[0]}")/record-traces.sh" wayland
checks_dir=$work/selection
mkdir -p "$checks_dir"

# The text typed into the field, and the evdev key code of each of its characters (to change the text, add codes).
text="abc def ghi"
declare -A keycodes=([a]=30 [b]=48 [c]=46 [d]=32 [e]=18 [f]=33 [g]=34 [h]=35 [i]=23 [" "]=57)
shift_key=42

# Mouse frames in weston-touch.py's syntax, beside record-traces.sh's touch builders (frames_tap, frames_swipe):
# `count` clicks at x y; a press at x1 y dragged to x2 (on the `count`th click); a shift-click.
frames_click() {
  local i
  echo "16 pointer:$1:$2"
  for ((i = 0; i < ${3:-1}; i++)); do printf '%s\n' "0 button:down" "60 button:up"; done
}
frames_mousedrag() {
  local i n=10
  (($4 > 1)) && frames_click "$1" "$2" $(($4 - 1))
  printf '%s\n' "16 pointer:$1:$2" "16 button:down"
  for ((i = 1; i <= n; i++)); do echo "$frame_ms pointer:$(lerp "$1" "$3" $i $n):$2"; done
  echo "16 button:up"
}
frames_shiftclick() { printf '%s\n' "16 pointer:$1:$2" "0 key:$shift_key:down" "0 button:down" "60 button:up" "16 key:$shift_key:up"; }

# The checks: name | steps, `;` apart, each a frame builder (click, mousedrag, shiftclick, tap, swipe) and its numbers |
# the selection hello must log last, as an extended regex. In the numbers, `@N` is the x of text offset N (fractions:
# inside a glyph), `@<` and `@>` a little left and right of the text, `Y` the text's middle and `H` the selection
# handles' (26 px below it). Mouse rows follow the Linux row of the behaviour table (docs/behaviour.md): the
# selection is set on the press. To add a check, add a row.
checks=(
  "click|click @5.5 Y|([56])\.\.\1"
  "click-start|click @< Y|0\.\.0"
  "double-click|click @5.5 Y 2|4\.\.7"
  "triple-click|click @5.5 Y 3|0\.\.11"
  "drag-back|mousedrag @> Y @5.5 1|11\.\.[56]"
  "double-click-drag|mousedrag @1.5 Y @5.5 2|0\.\.7"
  "shift-click|click @< Y; shiftclick @5.5 Y|0\.\.[56]"
  "touch-long-press|tap @5.5 Y 800|4\.\.7"
  "touch-handle-drag|tap @5.5 Y 800; swipe @7.8 H @> H 300|4\.\.11"
)

# Types $text into the field at x y, then parks the pointer below everything.
type_text() {
  local frames=() i code
  mapfile -t frames < <(frames_click "$1" "$2")
  for ((i = 0; i < ${#text}; i++)); do
    code=${keycodes["${text:i:1}"]}
    frames+=("30 key:$code:down" "30 key:$code:up")
  done
  frames+=("300 pointer:200:690")
  weston_frames "${frames[@]}"
}
# The steps' frames with @N, @<, @>, Y and H replaced, from the glyphs' extent "x0 x1" and the text's y.
frames_of() {
  local x0=$2 x1=$3 y=$4 step verb args
  IFS=';' read -ra steps <<<"$1"
  for step in "${steps[@]}"; do
    read -r verb args <<<"$step"
    args=$(awk -v x0="$x0" -v x1="$x1" -v y="$y" -v n="${#text}" '{
      for (i = 1; i <= NF; i++) {
        if ($i == "@<") $i = x0 - 4
        else if ($i == "@>") $i = x1 + 8
        else if ($i ~ /^@/) $i = sprintf("%.1f", x0 + substr($i, 2) * (x1 - x0) / n)
        else if ($i == "Y") $i = y
        else if ($i == "H") $i = y + 26
      }
      print }' <<<"$args")
    # Unquoted on purpose: $args is the builder's numbers.
    "frames_$verb" $args
  done
}

"${platform}_setup"
wanted=" $* "
failed=()
for row in "${checks[@]}"; do
  IFS='|' read -r name steps want <<<"$row"
  [[ $# == 0 || $wanted == *" $name "* ]] || continue
  launch_main
  # Unquoted on purpose: locate prints "x y".
  type_text $(locate ffffff)
  sleep 0.5
  "${driver}_screenshot" "$work/screen.png"
  read -r x0 x1 y < <(python3 -I "$repo/scripts/locate-colour.py" ffffff ink <"$work/screen.png" |
    awk -v s="$("${driver}_scale")" '{ printf "%.1f %.1f %d\n", $1 / s, $2 / s, $3 / s }')
  mapfile -t frames < <(frames_of "$steps" "$x0" "$x1" "$y")
  weston_frames "${frames[@]}"
  sleep 1
  log=$checks_dir/$name.log
  "${driver}_log" >"$log"
  got=$(grep -o 'text field selection: [0-9]*\.\.[0-9]*' "$log" | tail -1 | sed 's/.*: //' || true)
  if grep -qxE "$want" <<<"$got"; then
    echo "weston-selection-checks: PASS $name ($got)"
  else
    echo "weston-selection-checks: FAIL $name: selection ${got:-none}, want $want (text at x $x0–$x1, y $y)"
    failed+=("$name")
  fi
done
((${#failed[@]} == 0)) || { echo "weston-selection-checks: failed: ${failed[*]}"; exit 1; }
