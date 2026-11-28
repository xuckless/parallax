#!/usr/bin/env bash
# Runtime checks of NativeView on Wayland (patches 0047-0055, docs/native-view-wayland.md): runs
# examples/native-view-wayland in a private headless GNOME Shell (mutter as a compositor, its own D-Bus session,
# runtime dir and settings, so nothing of a desktop session is touched), injects pointer input through the Shell's
# Eval, reads its screenshots back with scripts/pixels.py, and asserts on pixels and on what the example logged.
# The app renders with Mesa lavapipe (CLIENT_ICD=host keeps the host's Vulkan driver). The Shell composites with
# whatever GL the host has (on a GPU-less CI runner, Mesa llvmpipe); --mesa-compositor makes it Mesa llvmpipe on any
# host, the Shell in a bubblewrap sandbox without /dev/dri and with only Mesa's EGL vendor. Each scale gets its own
# session; logs and screenshots land in target/native-view-wayland/<scale>/.
#
# Usage: scripts/wayland-native-view-checks.sh [--mesa-compositor] [scale...]   (default: 1 1.25, the scales checked)
# Needs: gnome-shell (with --headless), dbus-run-session, gdbus, python3; bwrap for --mesa-compositor. Without them
# it prints why and exits 0 (skipped), so a CI runner that can't run the Shell skips it visibly.
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bin=$repo/target/debug/native-view-wayland
out=$repo/target/native-view-wayland
# The example's colours (src/main.rs, src/page.rs): the view's container (the gap colour), the div over the view, the
# page's background (its red drifts) and its one-pixel yellow edge.
gap=c000c0
over=2050d0
page=**4080
edge=ffff00

# ---- Inside the private D-Bus session: one compositor, one scale. ----
if [[ ${1:-} == --session ]]; then
  scale=$2 work=$3
  failures=0
  fail() { echo "FAIL [$scale]: $*"; failures=$((failures + 1)); }
  pass() { echo "pass [$scale]: $*"; }
  px() { python3 -I "$repo/scripts/pixels.py" "$@"; }
  # Whether the pixel at x y is the page's background (its green and blue; its red drifts), as pixels.py matches.
  is_page() { read -r pr pg pb <<<"$(px "$1" at "$2" "$3")"; ((pg >= 40 && pg <= 88 && pb >= 104 && pb <= 152)); }
  # Whether count $1 is at least share $2 of a w x h area $3 x $4 (device pixels).
  at_least() { python3 -c "import sys; sys.exit(not $1 >= $2 * $3 * $4)"; }
  eval_js() { gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
    --method org.gnome.Shell.Eval "$1"; }
  shot() { gdbus call --session --dest org.gnome.Shell.Screenshot --object-path /org/gnome/Shell/Screenshot \
    --method org.gnome.Shell.Screenshot.Screenshot false false "$1" >/dev/null; }
  # Fractional scales need mutter's scaled framebuffers (default from GNOME 47; older mutter needs the flag).
  gsettings set org.gnome.mutter experimental-features "['scale-monitor-framebuffer']" 2>/dev/null || true
  gsettings set org.gnome.shell welcome-dialog-last-shown-version "999" 2>/dev/null || true
  # A plain black desktop, so no wallpaper pixel passes for the page.
  gsettings set org.gnome.desktop.background picture-options none 2>/dev/null || true
  gsettings set org.gnome.desktop.background primary-color '#000000' 2>/dev/null || true
  # --mesa-compositor: no DRM device and only Mesa's EGL vendor, so mutter falls back to llvmpipe.
  wrap=()
  [[ ${COMPOSITOR_GL:-host} == mesa ]] && wrap=(env __EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json
    LIBGL_ALWAYS_SOFTWARE=1 bwrap --dev-bind / / --tmpfs /dev/dri --die-with-parent --)
  "${wrap[@]}" gnome-shell --headless --unsafe-mode --wayland --no-x11 --virtual-monitor 1280x800 \
    >"$work/shell.log" 2>&1 &
  shell=$!
  trap 'kill $shell 2>/dev/null || true' EXIT
  for _ in $(seq 150); do [[ -S $XDG_RUNTIME_DIR/wayland-0 ]] && break; sleep 0.1; done
  [[ -S $XDG_RUNTIME_DIR/wayland-0 ]] || { echo "skip [$scale]: gnome-shell --headless did not start"; \
    tail -5 "$work/shell.log"; exit 0; }
  export WAYLAND_DISPLAY=wayland-0
  sleep 2
  # Which GL the compositor composites with: the GL driver libraries mapped into gnome-shell.
  shell_pid=$(pgrep -x -P "$shell" gnome-shell || echo "$shell")
  gl=$(grep -oE '/[^ ]*(libgallium|libnvidia-(eglcore|glcore)|_dri)[^ /]*\.so[^ ]*' "/proc/$shell_pid/maps" \
    | xargs -rn1 basename | sort -u | tr '\n' ' ')
  echo "compositor GL: ${gl:-unknown}"
  if [[ ${COMPOSITOR_GL:-host} == mesa ]]; then
    [[ $gl == *libgallium* && $gl != *nvidia* ]] && pass "the compositor runs on Mesa ($gl)" \
      || fail "the compositor is not on Mesa: $gl"
  fi
  if [[ $scale != 1 ]]; then
    dc() { gdbus call --session --dest org.gnome.Mutter.DisplayConfig --object-path /org/gnome/Mutter/DisplayConfig \
      --method "org.gnome.Mutter.DisplayConfig.$1" "${@:2}"; }
    serial=$(dc GetCurrentState | sed -E 's/^\(uint32 ([0-9]+),.*/\1/')
    dc ApplyMonitorsConfig "$serial" 1 "[(0, 0, $scale, 0, true, [('Meta-0', '1280x800@60.000', {})])]" "{}" \
      >/dev/null || fail "mutter refused scale $scale"
    sleep 1
  fi
  # Mesa names lavapipe's ICD lvp_icd.x86_64.json (Fedora) or lvp_icd.json (Ubuntu).
  if [[ ${CLIENT_ICD:-lvp} != host ]]; then
    lvp=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | grep -v i686 | head -1 || true)
    [[ -n $lvp ]] || { echo "FAIL [$scale]: no Mesa lavapipe ICD in /usr/share/vulkan/icd.d"; exit 1; }
    export VK_ICD_FILENAMES=$lvp
  fi

  # Still: the view shows the page under the div; GPUI draws nothing while only the page changes.
  "$bin" --seconds 60 >"$work/still.log" 2>&1 &
  app=$!
  sleep 4
  eval_js "Main.overview.hide()" >/dev/null
  sleep 2
  shot "$work/still.png"
  stage_width=$(eval_js "global.stage.width" | sed -E "s/^\(true, '([0-9.]+)'\)$/\1/")
  read -r shot_width _ <<<"$(px "$work/still.png" size)"
  factor=$(python3 -c "print($shot_width / $stage_width)")
  grep -q 'window surface alpha mode' "$work/still.log" \
    && pass "alpha mode logged: $(grep -o 'alpha mode [A-Za-z]*' "$work/still.log" | head -1)" \
    || fail "no alpha mode logged"
  # The view's container is the view's bounds: where the page or (at a fractional scale, one device pixel of) the
  # container shows.
  read -r l t r b <<<"$(px "$work/still.png" bbox "$gap" "$page" "$edge" || echo "0 0 0 0")"
  python3 -c "import sys; f=$factor; sys.exit(not (abs($r - $l - 640 * f) <= 2 and abs($b - $t - 400 * f) <= 2))" \
    && pass "the view shows at $l $t $r $b (device pixels), 640x400 at $factor" \
    || fail "the view is $l $t $r $b, not 640x400 at $factor"
  ring=$([[ $scale == 1 ]] && echo 0 || echo 1)
  inner=$(px "$work/still.png" inside "$gap" "$ring" "$edge")
  ((inner == 0)) && pass "no gap or seam inside the view (edge ring $ring)" || fail "$inner gap pixels inside the view"
  # The view's content, not just no gap: the page's colour fills most of it (all but the div, its bar and edges).
  shown=$(px "$work/still.png" count "$page")
  at_least "$shown" 0.8 "$(python3 -c "print(640 * $factor)")" "$(python3 -c "print(400 * $factor)")" \
    && pass "the page fills the view ($shown device pixels)" || fail "the page shows only $shown device pixels"
  read -r ox oy <<<"$(python3 -c "print(round($l + 124 * $factor), round($t + 80 * $factor))")"
  [[ $(px "$work/still.png" count "$over") -gt 1000 ]] && read -r cr cg cb <<<"$(px "$work/still.png" at "$ox" "$oy")" \
    && ((cb > 180 && cr < 80)) && pass "the div covers the view" || fail "no div over the view at $ox $oy"
  sleep 4
  idle=$(grep 'gpui frames/s' "$work/still.log" | sed -n '6,9p')
  echo "$idle"
  if [[ -z $idle ]] || grep -q 'gpui frames/s: [1-9]' <<<"$idle" || grep -qv 'page frames/s: [1-9][0-9]' <<<"$idle"
  then fail "GPUI drew while only the page changed"
  else pass "0 GPUI frames per second while the page commits"; fi

  # Input over the view reaches its element with view-relative positions; the div over it takes its own click.
  read -r vx vy dx dy bx by <<<"$(python3 -c "f=$factor
print($l / f + 400, $t / f + 250, $l / f + 124, $t / f + 80, $l / f - 50, $t / f + 200)")"
  eval_js "const Clutter = imports.gi.Clutter; globalThis.w15 = Clutter.get_default_backend().get_default_seat()
    .create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE); true" >/dev/null
  now='imports.gi.GLib.get_monotonic_time()'
  move() { eval_js "globalThis.w15.notify_absolute_motion($now, $1, $2); true" >/dev/null; sleep 0.4; }
  click() { eval_js "globalThis.w15.notify_button($now, 1, 1); globalThis.w15.notify_button($now, 1, 0); true" \
    >/dev/null; sleep 0.4; }
  # Whether the last `view pointer <kind>` in $log is within 2 pixels of (x, y).
  log=$work/still.log
  near() { grep "view pointer $1 at" "$log" | tail -1 | sed -E 's/.*\(([-0-9.]+), ([-0-9.]+)\)$/\1 \2/' \
    | awk -v x="$2" -v y="$3" '{ found = ($1 - x)^2 <= 4 && ($2 - y)^2 <= 4 } END { exit !found }'; }
  # A click on the view that should log a press at (x, y) in it. Now and then (seen twice in ten runs at 1.25) no
  # press is logged for the first click on the view after focusing; it is clicked once more, and the note says so.
  click_view() {
    click
    near down "$1" "$2" || { echo "note [$scale]: no press logged for a click on the view, clicking again"; click; }
  }
  # mutter keeps the first click on an unfocused window from it (the headless seat has no keyboard to focus it
  # otherwise), so the first click lands on the window's own background, left of the view; a second one there in
  # case the focus took longer than a click (seen with the Shell on llvmpipe).
  move "$bx" "$by"
  move "$bx" "$by"
  click
  click
  move "$vx" "$vy"
  move "$vx" "$vy"
  click_view 400 250
  eval_js "globalThis.w15.notify_discrete_scroll($now, imports.gi.Clutter.ScrollDirection.DOWN,
    imports.gi.Clutter.ScrollSource.WHEEL); true" >/dev/null
  sleep 0.4
  for kind in move down scroll; do
    near "$kind" 400 250 && pass "a $kind over the view arrives at 400 250 in the view" \
      || fail "no $kind at 400 250 in the view: $(grep "view pointer $kind" "$work/still.log" | tail -1)"
  done
  downs=$(grep -c 'view pointer down' "$work/still.log" || true)
  move "$dx" "$dy"
  click
  [[ $(grep -c 'GPUI div over the view: down' "$work/still.log" || true) -eq 1 \
    && $(grep -c 'view pointer down' "$work/still.log" || true) -eq $downs ]] \
    && pass "the div over the view takes its own click" || fail "the div's click went wrong"
  kill $app 2>/dev/null || true
  wait $app 2>/dev/null || true

  # Moving: the view moves and resizes every GPUI frame; hole and subsurface must land in the same commit.
  "$bin" --animate --seconds 9 >"$work/animate.log" 2>&1 &
  app=$!
  sleep 4
  eval_js "Main.overview.hide()" >/dev/null
  sleep 1
  shots=8
  for i in $(seq 1 $shots); do shot "$work/move-$i.png"; done
  out_of_step=0
  for i in $(seq 1 $shots); do
    inner=$(px "$work/move-$i.png" inside "$gap" "$((ring + 1))" "$edge")
    ((inner == 0)) || { echo "moving, shot $i: $inner gap pixels inside the view"; out_of_step=$((out_of_step + 1)); }
    # The view shows its content at its element's size: 400 high, 400 to 640 wide as it resizes (logical), most of
    # it the page's colour. An empty shot has no bbox and fails here, not passes for having no gap.
    read -r ml mt mr mb <<<"$(px "$work/move-$i.png" bbox "$gap" "$page" "$edge" || echo "0 0 0 0")"
    shown=$(px "$work/move-$i.png" count "$page")
    python3 -c "import sys; f=$factor; w=$mr-$ml; h=$mb-$mt
sys.exit(not (abs(h - 400 * f) <= 3 and 400 * f - 3 <= w <= 640 * f + 3 and $shown >= 0.7 * w * h))" \
      || { echo "moving, shot $i: the page shows $shown pixels in $ml $mt $mr $mb"; out_of_step=$((out_of_step + 1)); }
  done
  ((out_of_step == 0)) && pass "moving: the page and no gap in all $shots shots" \
    || fail "moving: hole and view out of step in $out_of_step of $shots shots"
  wait $app || true

  # Clipped: the view's top half is scrolled out of its container (and past the window's top edge). Only the lower
  # half shows, cropped from the page's buffer, and input is still relative to the whole view.
  "$bin" --clipped --seconds 30 >"$work/clipped.log" 2>&1 &
  app=$!
  sleep 4
  eval_js "Main.overview.hide()" >/dev/null
  sleep 1
  shot "$work/clipped.png"
  read -r l t r b <<<"$(px "$work/clipped.png" bbox "$gap" "$page" "$edge" || echo "0 0 0 0")"
  python3 -c "import sys; f=$factor; sys.exit(not (abs($r - $l - 640 * f) <= 2 and abs($b - $t - 200 * f) <= 2))" \
    && pass "clipped: only the visible part shows, at $l $t $r $b (640x200 at $factor), nothing past the clip" \
    || fail "clipped: the view shows at $l $t $r $b, not 640x200 at $factor"
  # The page's yellow band crosses its top half, 100 pixels down: squeezed into the visible part, it would show
  # 50 pixels down. Cropped, the page's background is there (two columns, as the white bar may cross one).
  at_y=$(python3 -c "print(round($t + 50 * $factor))")
  is_page "$work/clipped.png" "$(python3 -c "print(round($l + 400 * $factor))")" "$at_y" \
    || is_page "$work/clipped.png" "$(python3 -c "print(round($l + 200 * $factor))")" "$at_y" \
    && pass "clipped: the visible part shows the page's lower half" \
    || fail "clipped: no page at 50 pixels down the visible part (squeezed, or nothing shows)"
  shown=$(px "$work/clipped.png" count "$page")
  at_least "$shown" 0.8 "$(python3 -c "print(640 * $factor)")" "$(python3 -c "print(200 * $factor)")" \
    && pass "clipped: the page fills the visible part ($shown device pixels)" \
    || fail "clipped: the page shows only $shown device pixels"
  log=$work/clipped.log
  read -r vx vy ax ay bx by <<<"$(python3 -c "f=$factor
print($l / f + 400, $t / f + 100, $l / f + 400, $t / f - 40, $l / f - 50, $t / f + 100)")"
  move "$bx" "$by"
  move "$bx" "$by"
  click
  click
  move "$ax" "$ay"
  move "$ax" "$ay"
  ! near move 400 160 && pass "clipped: the pointer over the clipped-away part doesn't reach the view" \
    || fail "clipped: the clipped-away part took the pointer"
  move "$vx" "$vy"
  move "$vx" "$vy"
  click_view 400 300
  near down 400 300 && pass "clipped: a click in the visible part arrives at 400 300 in the whole view" \
    || fail "clipped: no click at 400 300 in the view: $(grep 'view pointer down' "$log" | tail -1)"
  kill $app 2>/dev/null || true
  wait $app 2>/dev/null || true

  # Resized while clipped and moving: from its own thread the page switches its buffer size every 45 frames, each
  # switch trailed by two frames of the old size (a browser's frames in flight), while GPUI re-crops the view every
  # frame. Every commit must carry a crop that fits its own buffer, or the compositor ends the app's connection.
  "$bin" --clipped --animate --resize --seconds 8 >"$work/resize.log" 2>&1 &
  app=$!
  sleep 3
  eval_js "Main.overview.hide()" >/dev/null
  status=0
  wait $app || status=$?
  seconds=$(grep -c 'gpui frames/s' "$work/resize.log" || true)
  late=$(grep 'gpui frames/s' "$work/resize.log" | tail -3)
  if ((status == 0 && seconds >= 7)) && ! grep -qiE 'protocol error|connection is gone|broken pipe' "$work/resize.log" \
    && ! grep -qE 'page frames/s: 0$' <<<"$late" && ! grep -q 'gpui frames/s: 0,' <<<"$late"; then
    pass "resized while clipped: the connection held for $seconds s, page and GPUI kept drawing"
  else
    fail "resized while clipped: exit $status after $seconds s: $(grep -iE 'error|frames/s' "$work/resize.log" | tail -3)"
  fi
  echo "$failures failure(s) at scale $scale"
  exit $((failures > 0))
fi

# ---- Outside: build, then one private session per scale. ----
tools=(gnome-shell dbus-run-session gdbus python3)
if [[ ${1:-} == --mesa-compositor ]]; then
  export COMPOSITOR_GL=mesa
  tools+=(bwrap)
  shift
fi
for tool in "${tools[@]}"; do
  command -v "$tool" >/dev/null || { echo "skip: no $tool; the Wayland native view checks need a headless GNOME Shell"; exit 0; }
done
scales=("${@:-1}")
[[ $# -gt 0 ]] || scales=(1 1.25)
cargo build -p native-view-wayland
failed=0
for scale in "${scales[@]}"; do
  work=$out/$scale
  rm -rf "$work"
  mkdir -p "$work/config" "$work/data" "$work/cache"
  # Wayland socket paths must fit sun_path (108 bytes), so the runtime dir is short.
  run=$(mktemp -d /tmp/nvw-XXXXXX)
  chmod 700 "$run"
  XDG_RUNTIME_DIR=$run XDG_CONFIG_HOME=$work/config XDG_DATA_HOME=$work/data XDG_CACHE_HOME=$work/cache \
    GSETTINGS_BACKEND=keyfile env -u WAYLAND_DISPLAY -u DISPLAY \
    dbus-run-session -- "$0" --session "$scale" "$work" || failed=1
  # The session's document portal may still hold its FUSE mount in the runtime dir for a moment.
  for mount in "$run"/doc "$run"/gvfs; do fusermount3 -u "$mount" 2>/dev/null || fusermount -u "$mount" 2>/dev/null || true; done
  sleep 1
  rm -rf "$run" 2>/dev/null || echo "note: $run is left behind (a FUSE mount)"
done
exit $failed
