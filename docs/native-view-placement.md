# Native views: placement, one copy

Part of [native-view.md](native-view.md).

Every backend decides what shows of a painted view from gpui's pure `native_view_placement` module (patch
[0050](../patches/0050-native-view-placement.patch), table-tested by [patch-test.sh](../scripts/patch-test.sh)).
A `PaintNativeView`'s bounds and content mask are whole device pixels already (the element snaps both), so a backend
never rounds them again; only a coarser grid does.

| Function | What | Used by |
|---|---|---|
| `visible(view)` | the part the content mask leaves, `None` when it leaves nothing (the view hides) | every backend |
| `round_outwards(bounds, px_per_unit)` | whole units of a coarser grid, Chromium's overlay rule | Wayland (logical pixels) |
| `hole_inset(resamples)` | 1 device pixel inside the view where the compositor resamples, else 0 | Wayland (fractional scale), iOS (Display Zoom, rotation) |
| `background(views)` | the last painted shown view's `set_background` colour, `None` when none asks | every backend |

What shows under the views when `background` is `None`: black in an opaque window on macOS and Windows, the desktop
through a transparent window on every target, the desktop on Wayland (with a
colour, GPUI's underlay subsurface fills an opaque window's holes: [native-view-wayland.md](native-view-wayland.md)), the canvas's page behind it on the web, transparent on iOS and
Android.
