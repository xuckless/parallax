# WebView2 as a NativeView

Patch [0071](../patches/0071-windows-native-view-visuals.patch) lets a WebView2 page be a `NativeView` on Windows,
through WebView2's visual hosting: the page renders into a DirectComposition visual in GPUI's own tree, under GPUI's
hole, with no child window over GPUI's ([native-view-windows.md](native-view-windows.md) is the rest of the design).
[examples/native-view-webview2](../examples/native-view-webview2/src/) is the whole pattern.

## How an app uses it

1. `gpui_windows::composition_device(window)` gives the window's DirectComposition device (`None` with
   `GPUI_DISABLE_DIRECT_COMPOSITION` or while the window draws). WebView2's `RootVisualTarget` must be on the device
   of the tree it joins, so make a visual on it (`CreateVisual`).
2. `NativeView::new(visual.as_raw(), window, cx)`: GPUI adds the visual under the view's clip, offset to the element.
3. `CreateCoreWebView2EnvironmentWithOptions`, then `ICoreWebView2Environment3::CreateCoreWebView2CompositionController`
   on GPUI's `HWND`; in its handler `SetRootVisualTarget(visual)`, then `Commit` on GPUI's device (WebView2's content
   joins GPUI's tree, which shows only after a commit, and GPUI commits only for its own frames: without it the page
   stays hidden until GPUI next draws), `SetIsVisible(true)`, navigate. The handlers run
   from GPUI's own message loop on the main thread, so state shared with listeners can live in an `Rc`.
4. Place it at the element's laid-out bounds in device px, in the window (`SetBounds(left, top, right, bottom)`;
   WebView2's bounds are raw pixels and its rasterization scale follows the monitor, as GPUI's does). WebView2 draws at
   its visual's origin, which GPUI places, but positions its popups (a `<select>`'s list, the context menu, IME
   candidates) from these bounds, as in Microsoft's WinComp sample: at (0, 0) they would show off by the view's
   position.

## Input

WebView2's visual hosting takes no spatial input itself; the app sends it
([Microsoft's docs](https://learn.microsoft.com/microsoft-edge/webview2/concepts/windowed-vs-visual-hosting)).
The view element's listeners do (`NativeViewInput::Window`, patch 0049): `on_mouse_move`, `on_mouse_down` /
`on_mouse_up` per button (one table, `BUTTONS` in the example's `app.rs`), `on_scroll` and `on_hover` (leave) become
`ICoreWebView2CompositionController::SendMouseInput`, at the position in the element × scale, rounded. The view's
element hears the pointer only over it, so a button pressed on the page is followed with window-level capture-phase
listeners (`on_mouse_event`) until its release: every move and the release reach the page wherever the pointer goes,
as its own window's mouse capture would give them, and a leave is sent only with no button held (or after a release
off the page). The cover div over the page occludes it, so it keeps its own clicks.

Keys are not sent: WebView2 has no key-input call. A click on the page calls `ICoreWebView2Controller::MoveFocus`,
WebView2 takes the window's keyboard focus and reads keys itself; a click anywhere else in the window calls
`SetFocus` on GPUI's `HWND` to take them back (the page's listener stops propagation so that one doesn't run).

**Lost device.** GPUI rebuilds its tree on a new device; a visual of the old one cannot join it (GPUI logs and skips
it). `gpui_windows::on_composition_device_changed(window, callback)` says so once the new tree is in place; the
callback runs inside GPUI's recovery, so the example schedules (an `AsyncApp` spawn) making a new visual, controller
and `NativeView` on the new device.

## Checks

[windows-native-view-checks.sh](../scripts/windows-native-view-checks.sh) runs the example in CI's `windows-touch` job
at 100 and 125 % after the swap chain example. The job installs the Evergreen WebView2 runtime only when the image has
none ([ci.yml](../.github/workflows/ci.yml)). The page (`src/page.html`, loaded with `NavigateToString`) posts its
events and its animation frames per second through `window.chrome.webview.postMessage`; the example logs them
(`WebMessageReceived`). Per scale: the page loads; a screenshot shows it in the hole, the cover over its corner, no
seam, no gap colour; a move and click over the view reach the page at 200 150 CSS px; the wheel and a key (after the
click) reach it; the cover keeps its click; 0 GPUI frames per second over 3 s while the page animates.

Measured on the runner (WebView2 runtime 157 on the image, so the install step only logs it), CI run 46111332527,
both scales: every check passes; the page animates at 64 frames/s while GPUI draws 0 frames and copies nothing.
