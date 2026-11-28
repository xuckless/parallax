# Owner check: native views at 125 % on the GNOME desktop

The headless checks ([native-view-wayland-checks.md](../../docs/native-view-wayland-checks.md)) read pixels at 1.25;
this is the look by eye on the real desktop (RTX 5060, NVIDIA driver) at 125 %, about two minutes.

```sh
# from the repo root, in your GNOME session
scripts/apply-patches.sh                   # once, if build/parallax-gpui/ is missing or stale
scripts/owner-fractional-scale-check.py    # each run 20 s; --seconds N to change
```

What it does: builds the example, saves your settings to `$XDG_RUNTIME_DIR/parallax-fractional-check.json`, turns on
mutter's `scale-monitor-framebuffer`, sets every monitor to 125 % (a temporary config: nothing is saved, no "keep
changes?" dialog), runs the example three times, then restores the setting (reset if you never set it) and every
monitor's position and scale, and says `restored: …`. Ctrl-C restores too. If it was killed hard, run
`scripts/owner-fractional-scale-check.py --restore`. Every monitor must share one scale (it refuses otherwise).

| Run | Look for |
|---|---|
| still | the page (blue, a white bar sweeping, yellow frame) under the blue div; no magenta line or gap on any edge, the page sharp |
| animate | the view moving and resizing with its magenta container: no magenta flashing at its edges, no lag |
| clipped | only the lower half of the page (no yellow band), in a see-through window; nothing above it |

After each run it prints the alpha mode, the last seconds of `gpui frames/s: N, page frames/s: M` (still and clipped:
GPUI 0) and its log under `target/native-view-wayland/owner-1.25/`. Report anything odd with the log; a screenshot
helps (PrtSc during the run).

Self-tested (2026-11-28) only in a private headless GNOME Shell: full run and SIGKILL + `--restore` both put the
setting and the monitor config back exactly. It has never touched your session.
