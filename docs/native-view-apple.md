# Native views on macOS and iOS

A `gpui::NativeView` shows an app's own `NSView` (macOS) or `UIView` (iOS) under GPUI's content: the Metal renderer
in gpui_apple clears a transparent hole where the view's element paints, and the layer is non-opaque only while a
view shows (opaque again with none). No copy of the view's content is made, and the view's own frames cause no GPUI
frame. The plan and acceptance list: [ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview).

| | macOS | iOS |
|---|---|---|
| Code | upstream's [native_views.rs](../upstream/gpui-pre-macos/src/window/native_views.rs) | patch [0052](../patches/0052-ios-native-view.patch): `ios/window/native_views.rs` |
| Where the view goes | a clipping container below GPUI's `NSView` | a clipping container below GPUI's `UIView`, inside the view controller's plain root view (so it turns with GPUI's content) |
| Pointer input over it | the view's (AppKit hit-tests it): `NativeViewInput::View` | GPUI's view stays on top and takes every touch, so the element's listeners hear it: `NativeViewInput::Window`. The view gets none unless the app forwards touches |
| Keyboard | the view takes keys while its focus handle is focused (upstream) | not wired: the view never becomes first responder |
| Gap colour | `NativeView::set_background` colours the window behind the views | the same, on the `UIWindow` |

Shared, patch [0051](../patches/0051-native-view-in-step.patch):

- **In step.** A frame that moves, resizes, shows or hides a view is presented with the Core Animation transaction
  that places it (`MetalRenderer::draw_placing_native_views`), so hole and view reach the screen together; other
  frames present on their own and pay nothing.
- **Alpha mode logged:** `metal layer alpha: opaque` / `premultiplied, native views show through` (target
  `gpui_apple`, info), each time a view shows or goes.

Who takes input over a view (`NativeViewInput`, iOS's `Window`) is patch
[0048](../patches/0048-native-view-window-input.patch)'s: [native-view.md](native-view.md#input).

Lifecycle, clipping and z-order on iOS follow Flutter's `FlutterPlatformViewsController.mm` (oracle only: Flutter
draws over its views, Parallax draws under them). An app that wants the view to get touches forwards them from the
element's listeners, as Flutter's touch interceptor and React Native's `RCTSurfaceTouchHandler.mm` do; the
element's bounds are the view's frame in points.

## The example

[examples/native-view-apple](../examples/native-view-apple/): the view recolours itself 60 times a second on its own
`NSTimer`; a GPUI div covers part of it; "Move" moves and resizes its slot every frame; "Hide" drops it. Once a second:
`native-view-apple: gpui frames/s N, native view frames/s M`. Input logs: `slot: mouse down at …` (iOS, view-relative),
`NSView: mouseDown: at …` (macOS, the view's own coordinates), `div over the view: click N`. The gap colour is
magenta, so any frame where hole and view part shows it.

```sh
cargo run -p native-view-apple                                     # macOS
cargo build -p native-view-apple --target aarch64-apple-ios-sim    # iOS: run as a bare .app, as in traces.md
```

## Checks (2026-11-27)

iOS 26.5 simulator (iPhone 17, 3x; iPad Air 11-inch, 2x): 0 GPUI frames/s while the view runs at 63; taps and swipes
give view-relative mouse downs and scrolls; a tap on the div reaches only the div; the view never gets a touch.
Recorded while the slot moves every frame: 0 of 212 frames show the gap colour presented in step, 28 of 442 without.
Edges are exact at 3x. Known: during UIKit's rotation animation GPUI's content is stretched and the hole leaves the
view for about 0.3 s (the gap colour shows; a gap colour close to the view's hides it), then they settle in step. On
the iPad simulator the app runs scaled in a window (`UIRequiresFullScreen`), which leaves a one-pixel blend of the gap
colour at the view's edges.

macOS needs a window server, which `ssh` doesn't have: the owner's check is
`/Users/Shared/parallax/w15-macos-native-view/CHECK.md` on pixl-mac. Upstream's NativeView unit tests pass there.
