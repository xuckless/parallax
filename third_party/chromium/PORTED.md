# Code ported from Chromium

Chromium is BSD-3-Clause (see `LICENSE` here, shipped as `LICENSE-CHROMIUM` by every crate that carries a port:
`parallax-gpui-linux`, through [fork-crates.sh](../../scripts/fork-crates.sh)'s `ported` table). Every file that ports
or adapts Chromium code gets one row, with what it left out (AGENTS.md porting rules 3, 4 and 10 apply as for Flutter).
Pinned Chromium commit: `4ea2b624ed0c500ec2f23ff17203dff91ad8faa2` (`main`, 2026-11-28).

Sync rule: at each upstream sync, diff each source path between the pinned commit and Chromium's current `main` and port
the fixes.

| Chromium path | Commit | Parallax path | Kind (port / oracle) | Dropped parts |
|---|---|---|---|---|
| `ui/ozone/platform/wayland/host/wayland_subsurface.cc` | `4ea2b624` | `patches/0049-wayland-native-view.patch` (`gpui-pre-linux/src/linux/wayland/window/native_view.rs`) | port | the sibling stacking list (`RemoveFromList`, `InsertAfter`/`InsertBefore`, `place_above` a sibling): every view goes directly below the window (`place_below(parent)`); the `WaylandSurface` wrapper and its root-window bookkeeping; `AdjustSubsurfaceBounds`'s rounding up of the origin (views round outwards, the hole is cut inside instead). Changed: `set_position` and `set_destination` are sent only when they changed |
| `ui/ozone/platform/wayland/host/wayland_frame_manager.cc` | `4ea2b624` | same file (`WaylandSubsurface::hide`) | port | everything but `PlayBackFrame`'s hiding of a subsurface and its mutter workaround (a null buffer committed in sync instead of destroying the role): frame records, buffer bookkeeping and release tracking (the app owns its buffers), presentation feedback, frame callbacks, explicit sync, the occlusion timer |
