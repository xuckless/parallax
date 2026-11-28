# Native views on macOS and iOS

A `gpui::NativeView` shows an app's own `NSView` (macOS) or `UIView` (iOS) under GPUI's content: the Metal renderer
in gpui_apple clears a transparent hole where the view's element paints, and the layer is non-opaque only while a
view shows (opaque again with none). No copy of the view's content is made, and the view's own frames cause no GPUI
frame. The plan and acceptance list: [ROADMAP.md, Phase: NativeView](../ROADMAP.md#phase-nativeview).

| | macOS | iOS |
|---|---|---|
| Code | upstream's [native_views.rs](../upstream/gpui-pre-macos/src/window/native_views.rs) | patch [0052](../patches/0052-ios-native-view.patch): `ios/window/native_views.rs` |
| Where the view goes | a clipping container below GPUI's `NSView` | a clipping container below GPUI's `UIView`, in a host view inside the view controller's plain root view |
| Pointer input over it | the view's (AppKit hit-tests it): `NativeViewInput::View` | GPUI's view stays on top and takes every touch, so the element's listeners hear it: `NativeViewInput::Window`. The view gets none unless the app forwards touches |
| Keyboard | the view takes keys while its focus handle is focused (upstream) | the app makes its view first responder ([below](#keyboard-on-ios)) |
| Gap colour | `NativeView::set_background` colours the window behind the views | the same, on the host view right behind GPUI's view |

Shared, patch [0051](../patches/0051-native-view-in-step.patch):

- **In step.** A frame that moves, resizes, shows or hides a view is presented with the Core Animation transaction
  that places it (`MetalRenderer::draw_placing_native_views`), so hole and view reach the screen together; other
  frames present on their own and pay nothing.
- **Edges.** gpui_apple's `native_view_placement.rs` turns core's `visible` part of a view (whole device pixels
  already, [native-view.md](native-view-placement.md)) into frames in points for both backends, and where
  the screen resamples the layer the hole is cut one pixel inside it (core's `hole_inset(true)`), so no gap-colour
  seam blends in.
  iOS: during a rotation's turn and under Display Zoom (`nativeScale` != `scale`); exact at 2x and 3x otherwise.
  macOS has no API for a scaled display mode's resampling, so it never insets.
- **Alpha mode logged:** `metal layer alpha: opaque` / `premultiplied, native views show through` (target
  `gpui_apple`, info), each time a view shows or goes.

Who takes input over a view (`NativeViewInput`, iOS's `Window`) is patch
[0048](../patches/0048-native-view-window-input.patch)'s: [native-view.md](native-view.md#input). Lifecycle, clipping
and z-order on iOS follow Flutter's `FlutterPlatformViewsController.mm` (oracle only: Flutter draws over its views,
Parallax draws under them). An app that wants the view to get touches forwards them from the element's listeners, as
Flutter's touch interceptor and React Native's `RCTSurfaceTouchHandler.mm` do; the element's bounds are the view's
frame in points.

**Rotation on iOS.** UIKit lays the root view out inside its rotation animation. The host takes the new size at once
(only its centre follows the turn) and GPUI draws the new size in that same transaction, so GPUI's content, the holes
and the views turn as one picture; nothing stretches. (Flutter's `FlutterViewController.mm` only delays its update to
mid-turn, which halves the stretch.)

## Keyboard on iOS

GPUI's view takes every touch, so the app moves keyboard focus itself, from the element's listener: focus the view's
handle (`window.focus(&native_view.focus_handle(cx), cx)`), then make its `UIView` first responder. While the handle is
focused GPUI's text input never becomes first responder (`set_focused_native_view`). When GPUI focus moves to a GPUI
text field, that field takes first responder back (UIKit resigns the app's view); when it moves anywhere else, the app
resigns its view (e.g. `cx.on_blur` of the handle). Keys the app's view leaves come up the responder chain to GPUI.

## The example

[examples/native-view-apple](../examples/native-view-apple/): the view recolours itself 60 times a second on its own
`NSTimer`; a GPUI div covers part of it; "Move" moves and resizes its slot every frame; "Hide" drops it. Once a second:
`native-view-apple: gpui frames/s N, native view frames/s M`. Input logs: `slot: mouse down at …` (iOS, view-relative),
`NSView: mouseDown: at …` (macOS), `div over the view: click N`. The gap colour is magenta, so any frame where hole
and view part shows it. iOS also has a GPUI field next to a native `UITextField` (`apple/typing.rs`), logging
`UITextField: first responder yes/no` and `UITextField: text "…"`.

```sh
cargo run -p native-view-apple                                     # macOS
cargo build -p native-view-apple --target aarch64-apple-ios-sim    # iOS: run as a bare .app, as in traces.md
```

A bare .app needs `UIDeviceFamily` `[1, 2]` in its Info.plist (Xcode adds it), or the iPad runs it as a 320 × 480
iPhone app scaled by a factor UIKit never reports, which leaves a one-pixel seam no app can detect. Upstream's macOS
examples build from the fork crate (patch [0053](../patches/0053-macos-native-view-examples.patch)):
`cargo run --manifest-path build/parallax-gpui/Cargo.toml -p parallax-gpui --example native_view`.

## Checks (2026-11-28)

iOS 26.5 simulator (iPhone 17, 3x; iPad Air 11-inch, 2x): 0 GPUI frames/s while the view runs at 63; taps and swipes
give view-relative mouse downs and scrolls; the div takes its own taps. Gap-colour frames in recordings: 0 of 228 with
the slot moving every frame; 0 of 436, 360 (iPhone) and 615 (iPad) through rotations to landscape and back (before:
38 of 617). Edges exact at 2x and 3x. Typing goes to the `UITextField` after a tap on it and back to the GPUI field
after a tap there, on both. macOS needs a window server, which `ssh` doesn't have: the owner's check is
`/Users/Shared/parallax/w15-macos-native-view/CHECK.md` on pixl-mac; upstream's NativeView unit tests pass there.
