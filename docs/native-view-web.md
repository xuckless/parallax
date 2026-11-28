# Native views on the web

Patch [0054](../patches/0054-web-native-view.patch): a DOM element (an iframe, a video, a canvas) shown under GPUI's
canvas through the wgpu renderer's hole ([native-view.md](native-view.md)). Code: gpui_web's new `native_views.rs`
(the DOM shell) and `native_view_layout.rs` (pure decisions); the slot lifecycle is ported from Flutter's
`content_manager.dart` and `slots.dart` ([PORTED.md](../third_party/flutter/PORTED.md)). Checks and results:
[native-view-web-checks.md](native-view-web-checks.md).

## Using it

`NativeView::new(NonNull::from(&element).cast(), window, cx)` with a `web_sys::HtmlElement` (GPUI keeps its own
reference). GPUI moves the element into its slot, so make it fresh: moving an `<iframe>` already in the document reloads
it. Give it `width` and `height` (`100%` fills the view; unset, GPUI sets `100%` and warns, as Flutter does).
`view.set_background(colour, cx)` colours the slots ([native-view-placement.md](native-view-placement.md)'s
`background`): a cross-origin iframe draws in its own process and shows its new size a frame after a resize, and the
colour fills that frame (white for most pages). With no colour the page behind the canvas shows there.
[examples/native-view-web](../examples/native-view-web/) does all of this ([examples.md](examples.md#native-view-web)).

## What GPUI does

- **DOM.** While views exist, a `<gpui-native-views>` layer before the canvas, which is raised above it (`position:
  relative; z-index: 1`); per view a `<gpui-native-view-slot>` (the part its content mask leaves, `overflow: hidden`)
  around a `<gpui-native-view>` (the view's bounds) around the element; above the slots, the shields. The layer
  follows the canvas's box (fractional, from `getBoundingClientRect`) wherever the page puts the canvas. The last view
  takes the layer and the document's listeners with it, and the canvas gets its own styles back.
- **Placement.** In `draw`, before the renderer draws: a view whose placement changed gets `transform:
  translate(..)`, `width` and `height` from the core's `visible` ([native-view-placement.md](native-view-placement.md)).
  The bounds are whole device pixels already, so the boxes sit on the hole's pixels at 1.25 too. Unchanged views are
  not touched. Not painted, or clipped away: `display: none` (an iframe keeps its page). DOM changes and the canvas
  reach the screen in the same browser frame.
- **Alpha.** WebGPU: the canvas is `premultiplied` only while a view shows (0047), logged as `native views: window
  surface alpha mode PreMultiplied`. WebGL2: the context is premultiplied from creation (the browser's default, which
  wgpu keeps), logged as `native views: WebGL2 canvas, premultiplied alpha (the context's default)`.
- **Input** (`NativeViewInput::View`, as macOS). A capture listener on the document sees each pointer event first.
  Events whose target is neither the canvas nor inside the layer (the rest of the page) are left alone. For the
  others it asks GPUI's hit test (`hit_test_native_view`) and sets the canvas's `pointer-events`
  (`native_view_layout::step`, one row per state × event × what the browser hit): `none` over an uncovered view, so
  the browser gives the element trusted events (iframe clicks and wheels, text selection, video controls); `auto`
  elsewhere. Over a view, GPUI's hitboxes painted after it (`PlatformWindow::on_native_view_occluders`) get
  transparent shields, so the pointer leaving an iframe for a GPUI div lands on GPUI's side; an event the browser
  gives a shield or the layer is copied to the canvas. One a shield takes where GPUI now hit-tests the view puts every
  shield back and is copied to the element under it (a same-document element; an iframe's own document gets no
  copy). A drag stays with its press's target. Views that start to show, and every touch release, give the views the
  pointer, so a tap needs no hover; after each frame a still mouse that GPUI's content moved a view under is given to
  the view, so the next wheel tick scrolls it.
- **Focus.** `set_focused_native_view(Some)` focuses the element unless DOM focus is inside it already (an `<input>`
  in it stays focused). DOM focus moving into a slot focuses the view's handle: the document's `focusin`, or the
  window's `blur` when focus moves into an iframe (the parent document sees no `focusin` then). `None`, or the view's
  slot leaving the document with focus inside it, gives DOM focus back to GPUI's IME mirror, so keys reach GPUI's
  field again.
- **Scale emulation.** DevTools' device mode and Playwright's `deviceScaleFactor` scale `devicePixelRatio` but not
  `devicePixelContentBoxSize`; gpui_web now trusts that box only where it agrees with the CSS box times the ratio
  (patch [0055](../patches/0055-web-canvas-device-pixel-size.patch)).

Costs: the capture listeners (and a non-passive document `wheel` listener) exist only while a view is attached; frames
that place nothing write no style; the shields are rebuilt only when GPUI's hitboxes over a view change.

## Tests

`native_view_layout::tests::placement_of_a_view` and `pointer_target_steps` run in
[patch-test.sh](../scripts/patch-test.sh)'s `web-pure` crate ([patch-tests.md](patch-tests.md));
`native_view_occluders_are_the_hitboxes_painted_over_it` with GPUI's tests. In a browser:
[native-view-web-checks.md](native-view-web-checks.md).
