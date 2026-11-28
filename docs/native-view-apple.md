# Native views on macOS and iOS

A `gpui::NativeView` shows an app's own `NSView` (macOS) or `UIView` (iOS) under GPUI's content: the Metal renderer
in gpui_apple clears a transparent hole where the view's element paints, and the layer is non-opaque only while a
view shows (opaque again with none). No copy of the view's content is made, and the view's own frames cause no GPUI
frame. The plan and acceptance list: [ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview).

| | macOS | iOS |
|---|---|---|
| Code | upstream's [native_views.rs](../upstream/gpui-pre-macos/src/window/native_views.rs) | patch [0062](../patches/0062-ios-native-view.patch): `ios/window/native_views.rs`, `ios/window/rotation.rs`, `ios/native_view_state.rs`, one-line hooks in upstream's files |
| Where the view goes | a clipping container below GPUI's `NSView` | a clipping container below GPUI's `UIView`, in a host view inside the view controller's plain root view |
| Pointer input over it | the view's (AppKit hit-tests it): `NativeViewInput::View` | GPUI's view stays on top and takes every touch, so the element's listeners hear it: `NativeViewInput::Window`. The view gets none unless the app forwards touches |
| Keyboard | the view takes keys while its focus handle is focused (upstream) | the app makes its view first responder ([below](#keyboard-on-ios)) |
| Gap colour | `NativeView::set_background` colours the window behind the views | the same, on the host view right behind GPUI's view |

Shared, patches [0059](../patches/0059-apple-native-view-placement.patch)–[0061](../patches/0061-apple-native-view-hole-inset.patch):

- **In step.** A frame that moves, resizes, shows or hides a view is presented with the Core Animation transaction
  that places it (`MetalRenderer::draw_placing_native_views`), so hole and view reach the screen together; other
  frames present on their own and pay nothing. iOS counts a restack as placing, and sets frames and the gap colour
  in one no-animation block. A scene that wasn't presented (no drawable) places nothing; the next one plans again.
- **Planning.** gpui_apple's `native_view_placement::plan_layout`, one copy for both backends (macOS's, moved out of
  upstream), works out which views a scene moves, shows, hides or restacks; table-tested on the host (`apple-pure`).
- **Edges.** gpui_apple's `native_view_placement.rs` turns core's `visible` part of a view (whole device pixels
  already, [native-view-placement.md](native-view-placement.md)) into frames in points for both backends, and where
  the screen resamples the layer the hole is cut one pixel inside it (core's `hole_inset(true)`), so no gap-colour
  seam blends in. iOS: during a rotation's turn and under Display Zoom (`nativeScale` != `scale`), decided by
  `Rotation::resamples` in `native_view_state.rs`; exact at 2x and 3x otherwise. macOS has no API for a scaled
  display mode's resampling, so it never insets.
- **Alpha mode logged:** `metal layer alpha: opaque` / `premultiplied, native views show through` (target
  `gpui_apple`, info), each time a view shows or goes.

Who takes input over a view (`NativeViewInput`, iOS's `Window`) is patch
[0049](../patches/0049-native-view-input-occlusion.patch)'s: [native-view.md](native-view.md#input). Lifecycle, clipping
and z-order on iOS follow Flutter's `FlutterPlatformViewsController.mm` (oracle only: Flutter draws over its views,
Parallax draws under them). An app that wants the view to get touches forwards them from the element's listeners, as
Flutter's touch interceptor and React Native's `RCTSurfaceTouchHandler.mm` do; the element's bounds are the view's
frame in points.

**Rotation on iOS** (`rotation.rs`). UIKit lays the root view out inside its rotation animation. The host takes the
new size at once (only its centre follows the turn) and GPUI draws the new size in that same transaction, so GPUI's
content, the holes and the views turn as one picture; nothing stretches. (Flutter's `FlutterViewController.mm` only
delays its update to mid-turn, which halves the stretch.) The nesting exists from the window's start, views or not:
GPUI's `CAMetalLayer` is its view's backing layer, so a view can only go beneath it as a sibling, and nesting GPUI's
view later would move it, and the text input view that holds the keyboard, between superviews of a live window.

## Keyboard on iOS

GPUI's view takes every touch, so the app moves keyboard focus itself, from the element's listener: focus the view's
handle (`window.focus(&native_view.focus_handle(cx), cx)`), then make its `UIView` first responder. While the handle is
focused GPUI's text input never becomes first responder (`set_focused_native_view`). When GPUI focus moves to a GPUI
text field, that field takes first responder back (UIKit resigns the app's view); when it moves anywhere else, the app
resigns its view (e.g. `cx.on_blur` of the handle). Dropping the focused view's last handle gives keyboard input back
to GPUI too, so a GPUI field focused next takes it (`KeyboardInput`, one transition table in `native_view_state.rs`).
Keys the app's view leaves come up the responder chain to GPUI.

## The example

[examples/native-view-apple](../examples/native-view-apple/): the view recolours itself 60 times a second on its own
`NSTimer`; an orange GPUI div covers part of it; "Move" moves and resizes its slot every frame; "Hide" stops painting
it; "Drop views" drops every native view handle. Once a second: `native-view-apple: gpui frames/s N, native view
frames/s M`. Input logs: `slot: mouse down at …` (iOS, view-relative), `NSView: mouseDown: at …` (macOS),
`div over the view: click N`. The gap colour is magenta, so any frame where hole and view part shows it. iOS also
has a GPUI field next to a native `UITextField` (`apple/typing.rs`), logging `UITextField: first responder yes/no`
and `UITextField: text "…"`.

```sh
cargo run -p native-view-apple                     # macOS
scripts/apple-native-view-checks.sh "iPhone 17"    # iOS simulator on pixl-mac, with its checks
```

Its checks: [native-view-apple-checks.md](native-view-apple-checks.md). Upstream's macOS examples build from the
fork crate (patch [0065](../patches/0065-macos-native-view-examples.patch)):
`cargo run --manifest-path build/parallax-gpui/Cargo.toml -p parallax-gpui --example native_view`.
