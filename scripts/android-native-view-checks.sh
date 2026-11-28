#!/usr/bin/env bash
# Runtime checks of NativeView on Android (docs/native-view-android-checks.md): builds, installs and launches
# examples/native-view on one running emulator or device, records screenshots while the example moves and
# resizes the surface's slot every frame, reads them back with scripts/pixels.py, drives input with adb, and asserts on
# pixels and on what the example logged. Screenshots and logcat land in target/android-native-view/.
#
# Usage: scripts/android-native-view-checks.sh [serial]     (default: the only device in `adb devices`)
# Needs: ANDROID_HOME with platform-tools and an NDK, Java 17+, python3 (see docs/cli.md, Requirements).
set -euo pipefail

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
adb="${ANDROID_HOME:?set ANDROID_HOME to the Android SDK}/platform-tools/adb"
app=io.github.xuckless.parallax.nativeview
component=$app/io.github.xuckless.parallax.ParallaxActivity
out=$repo/target/android-native-view
mkdir -p "$out"

serial=${1:-}
if [[ -z $serial ]]; then
  devices=$("$adb" devices | awk 'NR > 1 && $2 == "device" { print $1 }')
  [[ $(wc -w <<<"$devices") == 1 ]] || { echo "android-native-view-checks: need exactly one adb device, or pass its serial; found: ${devices:-none}" >&2; exit 1; }
  serial=$devices
fi
on_device() { "$adb" -s "$serial" shell "$@"; }
px() { python3 -I "$repo/scripts/pixels.py" "$@"; }

# The example's once-a-second line and the renderers' alpha lines: the same two regexes in every native-view check
# script (docs/examples-native-view.md).
frames_line='native-view: gpui frames/s ([0-9]+), view frames/s ([0-9]+)'
alpha_line='alpha mode [A-Z][A-Za-z]+|alpha mode, only [A-Za-z]+|premultiplied alpha \(|metal layer alpha: [a-z]+'
# The example's colours (src/android.rs, src/android/java.rs): the surface (its red drifts), the GPUI div over it, the
# underlay (`set_background`, the gap colour), and the two stacked tiles (yellow painted last).
view=**c040
over=2050d0
gap=ff00ff
top=e0c020
bottom=e03030
# Screenshots per moving-slot run, pixels a colour needs to count as shown, and the edge ring the gap check leaves
# out: a growing SurfaceControl shows the underlay there for up to one producer frame (docs/native-view-android.md).
frames=12
shown=2000
ring=8

failures=0
fail() { echo "FAIL: $*"; failures=$((failures + 1)); }
pass() { echo "pass: $*"; }

# Launches the example with intent extras $@ and waits up to 30 s for its first frame; the log is $out/app.log.
launch() {
  on_device am force-stop "$app"
  on_device logcat -c
  on_device am start -W -n "$component" "$@" >/dev/null
  for _ in {1..30}; do
    on_device logcat -d >"$out/app.log"
    grep -q 'parallax: first frame drawn' "$out/app.log" && { sleep 3; on_device logcat -d >"$out/app.log"; return 0; }
    sleep 1
  done
  fail "no first frame after launching with $*"
  return 1
}
shot() { "$adb" -s "$serial" exec-out screencap >"$1"; }
# Whether pixel ($2, $3) of shot $1 is colour $4 (within pixels.py's tolerance).
is_colour() {
  read -r r g b <<<"$(px "$1" at "$2" "$3")"
  python3 -c "import sys; w = bytes.fromhex('$4'); sys.exit(not all(abs(a - c) <= 24 for a, c in zip(($r, $g, $b), w)))"
}

echo "android-native-view-checks: building and installing on $serial"
cargo build --quiet -p cargo-parallax --manifest-path "$repo/Cargo.toml"
(cd "$repo/examples/native-view" && "$repo/target/debug/cargo-parallax" run android --device "$serial") \
  >"$out/build.log" 2>&1 || { tail -20 "$out/build.log"; exit 1; }
api=$(on_device getprop ro.build.version.sdk | tr -d '\r')
echo "API $api"

# Still: 0 GPUI frames/s while only the surface draws; the alpha mode is logged; whether holes can show at all.
launch
sleep 4
on_device logcat -d >"$out/still.log"
idle=$(grep -oE "$frames_line" "$out/still.log" | tail -4 || true)
echo "$idle" | tail -1
[[ -n $idle ]] && ! grep -q 'gpui frames/s [1-9]' <<<"$idle" && grep -q 'view frames/s [1-9][0-9]' <<<"$idle" \
  && pass "0 GPUI frames/s while the surface draws" || fail "GPUI drew while only the surface changed"
alpha=$(grep -oE "$alpha_line" "$out/still.log" | tail -1 || true)
[[ -n $alpha ]] && pass "$alpha logged" || fail "no alpha mode logged"
shot "$out/still.raw"
read -r width height <<<"$(px "$out/still.raw" size)"
if [[ $alpha == *'only Opaque' ]]; then
  # GL below API 30 on the emulator: no Vulkan to move to (backend.rs FOR_NATIVE_VIEWS). Real devices use Vulkan.
  echo "skip: holes are black on this device's GL (API $api), the pixel rows can't see the views"
  black=1
else
  black=0
fi

# Moving slot, per kind of view: every recorded frame shows the view and the div over it, and no gap inside. Rows:
# kind, lowest API, whether a gap fails the row. A View and an in_surface View lag their hole by a frame on a move or
# a grow (docs/native-view-android-hosting.md): their gap frames are counted, not failed.
kinds=("view 28 known" "control 29 strict" "hosted 34 known")
for row in "${kinds[@]}"; do
  read -r kind since gaps <<<"$row"
  ((black || api < since)) && continue
  launch --es view "$kind" --ez animate true || continue
  for i in $(seq "$frames"); do shot "$out/$kind-$i.raw"; done
  missing=0 lagging=0
  for i in $(seq "$frames"); do
    s=$out/$kind-$i.raw
    v=$(px "$s" count "$view") d=$(px "$s" count "$over") g=$(px "$s" inside "$gap" "$ring" "$view" "$over")
    ((v >= shown && d >= shown)) || missing=$((missing + 1))
    ((g == 0)) || lagging=$((lagging + 1))
    ((v >= shown && d >= shown && g == 0)) || echo "  $kind frame $i: view $v px, div $d px, gap $g px"
  done
  if ((missing > 0)) || { [[ $gaps == strict ]] && ((lagging > 0)); }; then
    fail "$kind slot moving every frame: $missing of $frames frames without the view or div, $lagging with a gap"
  else
    pass "$kind slot moving every frame: view and div in all $frames frames, $lagging with a gap ($gaps)"
  fi
done

# Two overlapping views stack in GPUI's paint order, not attach order (the yellow tile is attached first).
stacks=(view)
((api >= 29)) && stacks+=(control)
for kind in "${stacks[@]}"; do
  ((black)) && break
  launch --es view "$kind" --ez stack true || continue
  s=$out/stack-$kind.raw
  shot "$s"
  read -r l t _ _ <<<"$(px "$s" bbox "$top" || echo "0 0 0 0")"
  # The yellow tile's top-left corner lies over the red tile: yellow there means yellow is on top.
  [[ $(px "$s" count "$bottom") -ge $shown ]] && is_colour "$s" $((l + 10)) $((t + 10)) "$top" \
    && pass "$kind tiles stack in paint order" || fail "$kind tiles don't stack in paint order (yellow at $l $t)"
done

# Keys stay GPUI's after a tap forwarded to the WebView: Back reaches GPUI's back listener.
launch
on_device input tap $((width / 2)) $((height * 3 / 4))
sleep 1
on_device input keyevent KEYCODE_BACK
sleep 1
on_device logcat -d >"$out/back.log"
grep -q 'native-view: back Committed' "$out/back.log" && pass "Back after tapping the WebView reaches GPUI" \
  || fail "Back after tapping the WebView did not reach GPUI"

on_device am force-stop "$app"
grep -E 'FATAL EXCEPTION|Fatal signal|cannot rebuild the renderer' "$out"/*.log && fail "the app crashed or lost its renderer"
echo "android-native-view-checks: $failures failed (shots and logs: $out)"
((failures == 0))
