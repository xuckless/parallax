#!/usr/bin/env bash
# `cargo parallax run linux` for one example inside a private headless GNOME Shell (mutter as a Wayland compositor,
# its own D-Bus session), waiting for the log lines that prove the app ran; nothing shows on a desktop. Gate 5's
# linux job runs it (docs/ci.md); it runs the same locally.
#
# Usage: scripts/linux-run-check.sh <example> <pattern>… [-- <app args>…]
#   Runs examples/<example> with `cargo parallax run linux [-- <app args>]`, passes when every extended-regex
#   <pattern> shows in its log within 5 minutes (the first run builds), then stops it.
# Needs: cargo-parallax on PATH (`cargo install --path crates/cli`), gnome-shell (with --headless), dbus-run-session.
# The log and the Shell's log go to target/linux-run-check/<example>/.
set -uo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)

if [[ ${1:-} == --session ]]; then
  shift
  work=$1 example=$2; shift 2
  patterns=() app=()
  while (($#)); do
    [[ $1 == -- ]] && { shift; app=("$@"); break; }
    patterns+=("$1"); shift
  done
  gnome-shell --headless --unsafe-mode --wayland --no-x11 --virtual-monitor 1280x800 >"$work/shell.log" 2>&1 &
  shell=$!
  for _ in $(seq 150); do [[ -S $XDG_RUNTIME_DIR/wayland-0 ]] && break; sleep 0.1; done
  [[ -S $XDG_RUNTIME_DIR/wayland-0 ]] || { echo "linux-run-check: gnome-shell --headless did not start"; tail -5 "$work/shell.log"; kill $shell; exit 1; }
  export WAYLAND_DISPLAY=wayland-0
  sleep 2
  (cd "$repo/examples/$example" && exec cargo parallax run linux ${app[@]+-- "${app[@]}"}) >"$work/app.log" 2>&1 &
  run=$!
  seen() { for p in "${patterns[@]}"; do grep -Eq "$p" "$work/app.log" || return 1; done; }
  for _ in $(seq 600); do seen && break; kill -0 $run 2>/dev/null || break; sleep 0.5; done
  status=0
  for p in "${patterns[@]}"; do
    if grep -Eq "$p" "$work/app.log"; then echo "ok $example: /$p/"; else echo "FAIL $example: no /$p/ in its log"; status=1; fi
  done
  ((status)) && tail -20 "$work/app.log"
  pkill -P $run 2>/dev/null; kill $run 2>/dev/null; sleep 1
  kill $shell
  exit $status
fi

[[ $# -ge 2 ]] || { echo "usage: scripts/linux-run-check.sh <example> <pattern>… [-- <app args>…]" >&2; exit 2; }
for tool in cargo-parallax gnome-shell dbus-run-session; do
  command -v "$tool" >/dev/null || { echo "linux-run-check: $tool not found" >&2; exit 2; }
done
work=$repo/target/linux-run-check/$1
mkdir -p "$work"
# A private runtime directory and settings, so the Shell never touches the user's session.
run=$(mktemp -d /tmp/linux-run-check-XXXXXX); chmod 700 "$run"
XDG_RUNTIME_DIR=$run GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME=$work/config env -u WAYLAND_DISPLAY -u DISPLAY \
  dbus-run-session -- "$0" --session "$work" "$@"
status=$?
for m in "$run"/doc "$run"/gvfs; do fusermount3 -u "$m" 2>/dev/null || true; done
rm -rf "$run" 2>/dev/null
exit $status
