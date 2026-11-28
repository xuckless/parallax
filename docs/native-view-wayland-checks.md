# Native views on Wayland: the live checks

[wayland-native-view-checks.sh](../scripts/wayland-native-view-checks.sh) runs
[examples/native-view-wayland](../examples/native-view-wayland/) ([examples.md](examples-native-view.md#native-view-wayland)) in a
private headless GNOME Shell and asserts on what the compositor showed and what the example logged: the acceptance
list of [ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview) for patches 0047–0049
([native-view-wayland.md](native-view-wayland.md)).

```sh
scripts/wayland-native-view-checks.sh            # scales 1 and 1.25
scripts/wayland-native-view-checks.sh 1.25       # one scale
scripts/wayland-native-view-checks.sh --mesa-compositor   # the Shell on Mesa llvmpipe too, whatever the host's GPU
CLIENT_ICD=host scripts/wayland-native-view-checks.sh   # the app on the host's Vulkan driver, not Mesa lavapipe
```

## How it runs

- One `dbus-run-session` per scale with its own runtime dir, config, data and cache dirs and the keyfile settings
  backend, so a desktop session (and its settings) is never touched; the script re-runs itself inside it
  (`--session`). `gnome-shell --headless --unsafe-mode --virtual-monitor 1280x800` is the compositor (mutter), on a
  plain black desktop; a scale other than 1 is applied through `org.gnome.Mutter.DisplayConfig`.
- The app renders with Mesa lavapipe (`VK_ICD_FILENAMES`), so the check is the same on a GPU-less CI runner.
- The Shell composites with the host's GL: the NVIDIA driver on the owner's machine, Mesa llvmpipe on a GPU-less
  runner. `--mesa-compositor` runs it on Mesa anywhere: in a `bwrap` sandbox with an empty `/dev/dri` and only Mesa's
  EGL vendor (`__EGL_VENDOR_LIBRARY_FILENAMES`, `LIBGL_ALWAYS_SOFTWARE=1`), so mutter falls back to llvmpipe. Every
  run prints `compositor GL:` (the GL libraries mapped into `gnome-shell`); with the flag it also checks they are
  Mesa's (`libgallium`) and not NVIDIA's.
- Screenshots come from `org.gnome.Shell.Screenshot`; [pixels.py](../scripts/pixels.py) reads them (PNG).
  Pointer input is a Clutter virtual pointer driven through `org.gnome.Shell.Eval` (why `--unsafe-mode`).
- mutter keeps the first click on an unfocused window from the app, and the headless seat has no keyboard to focus
  it, so the first click lands on the window's background.

## What it checks, per scale

| Check | How |
|---|---|
| Alpha mode logged | `window surface alpha mode` in the example's log |
| The view shows at its element's size | the container's and page's pixels span 640×400 logical |
| No gap or seam | no container (gap colour) pixel inside the view, past a one-device-pixel ring at a fractional scale |
| The div covers the view | its colour where it is painted over the view |
| 0 GPUI frames/s while only the page changes | seconds 6–9 of `gpui frames/s: 0, page frames/s: 60` |
| Pointer input reaches the view's element | a move, a click and a scroll at (400, 250) in the view log `view pointer … at (400.0, 250.0)` |
| The div over the view keeps its own input | its click logs `GPUI div over the view: down` and no view press |
| In step while moving | `--animate`: 8 screenshots with no gap pixel inside the view |
| A clipped view shows only its visible part | `--clipped` (top half scrolled out, see-through window): the view's pixels span 640×200, none past the clip |
| …cropped, not squeezed | no yellow at 50 logical pixels down the visible part, where the page's top-half band would land squeezed |
| Input over the clipped view | the pointer over the clipped-away part logs no view move; a click in the visible part logs `view pointer down at (400.0, 300.0)`, relative to the whole view |

| A resize while clipped never costs the connection | `--clipped --animate --resize` for 8 s (the page switches buffer size every 45 frames from its thread, two stale old-size frames after each switch, GPUI re-crops every frame): exits 0, no protocol error, page and GPUI frames in each of the last seconds |

Logs and screenshots: `target/native-view-wayland/<scale>/` (`still.log`, `still.png`, `animate.log`,
`move-*.png`, `clipped.log`, `clipped.png`, `resize.log`, `shell.log`).

## CI

CI's `linux-touch` job installs `gnome-shell` and runs the script ([ci.md](ci.md)); the directory above is uploaded
with its artifact. Its runner has no GPU, so there the Shell already composites on Mesa llvmpipe without the flag:
CI is the Mesa-compositor run, the flag repeats it on a machine with a GPU. Without `gnome-shell`, `dbus-run-session`, `gdbus` or `python3`, or when the Shell does not start,
the script prints `skip: …` and exits 0, so a runner that can't run it says so instead of failing.
