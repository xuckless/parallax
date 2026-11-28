#!/usr/bin/env python3
"""The owner's fractional-scale look at examples/native-view-wayland on the real GNOME desktop
(examples/native-view-wayland/CHECK.md, docs/native-view-wayland.md).

Builds the example, then for the length of the check: turns on mutter's `scale-monitor-framebuffer` experimental
feature, sets every monitor to the supported scale nearest 125 % (org.gnome.Mutter.DisplayConfig, a temporary
config: mutter saves nothing to monitors.xml and asks no "keep changes?"), and runs the example still, moving
(`--animate`) and clipped (`--clipped`), one after another. Then it puts back exactly what was there: the feature
setting (reset if it was never set) and the monitors' positions, scales, transforms and primary, and checks they
match. Before changing anything it writes what it found to $XDG_RUNTIME_DIR/parallax-fractional-check.json; if the
script was killed before restoring, `--restore` puts that back.

Usage: scripts/owner-fractional-scale-check.py [--seconds N]    (each run's length, default 20)
       scripts/owner-fractional-scale-check.py --restore        (after a killed run)
Needs: a GNOME Wayland session, python3-gobject (gi), cargo. Every monitor must share one scale.
"""

import json
import os
import signal
import subprocess
import sys
import time

from gi.repository import Gio, GLib

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SAVED = os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "parallax-fractional-check.json")
LOGS = os.path.join(REPO, "target", "native-view-wayland", "owner-1.25")
TARGET_SCALE = 1.25
FEATURE = "scale-monitor-framebuffer"
# (run name, example arguments, what to look at). Add a row for another look.
RUNS = [
    ("still", [], "the page under the blue div: no magenta line or gap on any edge of the view, a sharp page"),
    ("animate", ["--animate"], "the view moving and resizing: its edges stay on the magenta container's, no flicker"),
    ("clipped", ["--clipped"], "the view's lower half only, in a see-through window: nothing above the half"),
]
METHOD_VERIFY, METHOD_TEMPORARY = 0, 1


def display_config():
    return Gio.DBusProxy.new_for_bus_sync(
        Gio.BusType.SESSION, Gio.DBusProxyFlags.NONE, None, "org.gnome.Mutter.DisplayConfig",
        "/org/gnome/Mutter/DisplayConfig", "org.gnome.Mutter.DisplayConfig", None)


def current_state(proxy):
    """(serial, layout) where layout is the logical monitors as plain lists: x, y, scale, transform, primary, and
    each monitor's connector with its current mode id and that mode's supported scales."""
    serial, monitors, logical, _ = proxy.call_sync(
        "GetCurrentState", None, Gio.DBusCallFlags.NONE, -1, None).unpack()
    modes = {}
    for (connector, _, _, _), monitor_modes, _ in monitors:
        for mode_id, _, _, _, _, scales, props in monitor_modes:
            if props.get("is-current"):
                modes[connector] = (mode_id, list(scales))
    layout = [[x, y, scale, transform, primary, [[spec[0], *modes[spec[0]]] for spec in specs]]
              for x, y, scale, transform, primary, specs, _ in logical]
    return serial, layout


def apply(proxy, serial, layout, method):
    config = [(x, y, scale, transform, primary, [(connector, mode_id, {}) for connector, mode_id, _ in specs])
              for x, y, scale, transform, primary, specs in layout]
    proxy.call_sync("ApplyMonitorsConfig",
                    GLib.Variant("(uua(iiduba(ssa{sv}))a{sv})", (serial, method, config, {})),
                    Gio.DBusCallFlags.NONE, -1, None)


def same_layout(a, b):
    key = lambda layout: sorted((x, y, round(scale, 4), transform, primary, tuple(s[0] for s in specs))
                                for x, y, scale, transform, primary, specs in layout)
    return key(a) == key(b)


def scaled_layout(layout):
    """Every monitor at the supported scale nearest TARGET_SCALE, positions scaled to keep them touching."""
    if len({round(scale, 4) for _, _, scale, _, _, _ in layout}) != 1:
        sys.exit("every monitor must share one scale; set them alike in Settings → Displays, or check by hand")
    scaled = []
    for x, y, scale, transform, primary, specs in layout:
        new = min(specs[0][2] or [scale], key=lambda supported: abs(supported - TARGET_SCALE))
        scaled.append([round(x * scale / new), round(y * scale / new), new, transform, primary, specs])
    return scaled


def restore(saved):
    settings = Gio.Settings.new("org.gnome.mutter")
    if saved["features"] is None:
        settings.reset("experimental-features")
    else:
        settings.set_strv("experimental-features", saved["features"])
    Gio.Settings.sync()
    time.sleep(2)
    proxy = display_config()
    serial, layout = current_state(proxy)
    if not same_layout(layout, saved["layout"]):
        apply(proxy, serial, saved["layout"], METHOD_TEMPORARY)
        time.sleep(2)
        _, layout = current_state(proxy)
    if not same_layout(layout, saved["layout"]):
        sys.exit(f"NOT restored: monitors are {layout}, were {saved['layout']}; experimental-features was "
                 f"{saved['features']} (None: never set). Fix in Settings → Displays; {SAVED} is kept.")
    os.remove(SAVED)
    print("restored: experimental-features and every monitor's position and scale are as before")


def run_example(name, args, look, seconds):
    os.makedirs(LOGS, exist_ok=True)
    log = os.path.join(LOGS, f"{name}.log")
    print(f"\n{name} ({seconds} s): look at {look}")
    with open(log, "w") as out:
        subprocess.run([os.path.join(REPO, "target", "debug", "native-view-wayland"), *args,
                        "--seconds", str(seconds)], stdout=out, stderr=subprocess.STDOUT, check=False)
    lines = open(log).read().splitlines()
    alpha = next((line for line in lines if "alpha mode" in line), "no alpha mode logged")
    frames = [line.split("] ")[-1] for line in lines if "gpui frames/s" in line]
    print(f"  {alpha.split('] ')[-1]}\n  last seconds: {frames[-3:]}\n  log: {log}")


def main():
    if sys.argv[1:] == ["--restore"]:
        restore(json.load(open(SAVED)))
        return
    seconds = int(sys.argv[2]) if sys.argv[1:2] == ["--seconds"] else 20
    if os.path.exists(SAVED):
        sys.exit(f"{SAVED} exists: an earlier run did not restore. Run with --restore first.")
    if not os.environ.get("WAYLAND_DISPLAY"):
        sys.exit("run this in the GNOME Wayland session (no WAYLAND_DISPLAY)")
    subprocess.run(["cargo", "build", "-p", "native-view-wayland"], cwd=REPO, check=True)

    settings = Gio.Settings.new("org.gnome.mutter")
    user_value = settings.get_user_value("experimental-features")
    proxy = display_config()
    _, layout = current_state(proxy)
    saved = {"features": None if user_value is None else list(user_value.unpack()), "layout": layout}
    with open(SAVED, "w") as out:
        json.dump(saved, out)
    print(f"saved the current settings to {SAVED}")
    # Ctrl-C, a closed terminal or kill still restore.
    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, lambda *_: sys.exit("interrupted"))
    try:
        features = settings.get_strv("experimental-features")
        if FEATURE not in features:
            settings.set_strv("experimental-features", [*features, FEATURE])
            Gio.Settings.sync()
            time.sleep(2)
        serial, layout = current_state(proxy)
        scaled = scaled_layout(layout)
        apply(proxy, serial, scaled, METHOD_VERIFY)
        apply(proxy, serial, scaled, METHOD_TEMPORARY)
        time.sleep(2)
        print(f"monitors now at scale {sorted({scale for _, _, scale, _, _, _ in current_state(proxy)[1]})}")
        for name, args, look in RUNS:
            run_example(name, args, look, seconds)
    finally:
        restore(saved)


if __name__ == "__main__":
    main()
