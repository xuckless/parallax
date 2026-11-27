#!/usr/bin/env bash
# Builds Weston with its headless backend, kiosk shell, Xwayland module, screenshooter and the test plugin
# (`test-plugin.so`, the `weston_test` global Weston's own test suite injects input with), for record-traces.sh's
# wayland and x11 harnesses (docs/traces-desktop.md). Distributions never package the test plugin ("weston-test.so
# should never be installed", weston-test.xml), so it is built from Weston's source, pinned below, into
# target/weston/ (nothing is installed). Prints the build directory, which `meson devenv -C` runs Weston from.
#
# Usage: scripts/build-weston.sh
# Needs: git, meson, ninja and Weston's build dependencies (CI's linux-touch job lists them: .github/workflows/ci.yml).
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# Weston 14.0.2: needs wayland 1.22 (Ubuntu 24.04) or later. Bump both lines together.
tag=14.0.2
commit=015b3b4d4c05da44a22349ea6e651d1a8f678c59
dir=$repo/target/weston
src=$dir/src-$tag
out=$dir/build-$tag

# Clone unless $src is already that commit's own checkout. A cache (CI's rust-cache keeps target/) can restore part
# of it, or none of its .git, so git would answer for the enclosing repository: start again then.
if [[ $(git -C "$src" rev-parse --show-toplevel HEAD 2>/dev/null) != "$src"$'\n'"$commit" ]]; then
  rm -rf "$src" "$out"
  git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$tag" https://gitlab.freedesktop.org/wayland/weston.git "$src"
  # The plugin assumes Weston's test runner; standalone it needs four NULL guards (see the patch's header).
  git -C "$src" apply "$repo/scripts/weston-test-standalone.patch"
fi
[[ $(git -C "$src" rev-parse HEAD) == "$commit" ]] || { echo "build-weston: $tag is not $commit" >&2; exit 1; }

# Everything off but what the harness runs: headless backend, pixman renderer, kiosk shell (toplevels fill the
# output, so window px are output px), Xwayland, the screenshooter and the tests (for the plugin).
if [[ ! -f $out/build.ninja ]]; then
  meson setup "$out" "$src" --buildtype=release -Dbackend-default=headless \
    -Dbackend-drm=false -Dbackend-drm-screencast-vaapi=false -Dbackend-pipewire=false -Dbackend-rdp=false \
    -Dscreenshare=false -Dbackend-vnc=false -Dbackend-wayland=false -Dbackend-x11=false -Drenderer-gl=false \
    -Dsystemd=false -Dremoting=false -Dpipewire=false -Dshell-desktop=false -Dshell-fullscreen=false \
    -Dshell-ivi=false -Dshell-kiosk=true -Dxwayland=true -Dcolor-management-lcms=false -Dimage-jpeg=false \
    -Dimage-webp=false -Dtools=[] -Ddemo-clients=false -Dsimple-clients=[] -Dwcap-decode=false -Dtests=true \
    -Dtest-junit-xml=false -Ddoc=false >/dev/null
fi
ninja -C "$out" frontend/weston kiosk-shell/kiosk-shell.so libweston/backend-headless/headless-backend.so \
  xwayland/xwayland.so tests/test-plugin.so clients/weston-screenshooter >/dev/null
echo "$out"
