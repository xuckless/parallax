# Examples: native views

Part of [examples.md](examples.md): the `native-view-*` examples, one per target, each showing an OS-native view
through GPUI's hole ([native-view.md](native-view.md)).

Where the window gets the pointer (Wayland, Windows, WebView2), a press on the view is followed with window-level
capture-phase listeners (`Window::on_mouse_event`) until its release, so a drag that leaves the view keeps its moves
and its release, as a browser's own window capture would give them; the view element's own listeners hear the pointer
only over it.

## native-view-wayland

[examples/native-view-wayland](../examples/native-view-wayland/) (Linux, Wayland only): a GPUI window with a native
view ([native-view-wayland.md](native-view-wayland.md)). The "page" ([page.rs](../examples/native-view-wayland/src/page.rs))
owns a `wl_surface` on GPUI's connection and commits a new 640×400 shm buffer to it about 60 times a second from its
own thread; a blue GPUI div covers part of it. Once a second it logs `gpui frames/s: N, page frames/s: M` (GPUI's is
0 while only the page changes), and it logs the alpha mode GPUI chose and every pointer event over the view with its
view-relative position. Flags: `--fps N` (page rate), `--animate` (the view moves and resizes every GPUI frame),
`--clipped` (the view half scrolled out of a container, in a see-through window: only its lower half may show),
`--resize` (the page switches its buffer size every 45 frames, each switch trailed by two old-size frames, committed
through `NativeViewBuffers` from its thread),
`--no-view` (the same window without the view, a cost baseline), `--seconds N` (quit after N seconds). The page's
yellow band across its top half shows whether a clipped view is cropped or squeezed.

## native-view-web

[examples/native-view-web](../examples/native-view-web/) (web only): four DOM elements under GPUI's canvas as native
views, made by [index.html](../examples/native-view-web/index.html): a same-origin iframe, a cross-origin one (the
other loopback name, the next port) a canvas redrawn every animation frame and a form (a box with an `<input>`), each with a GPUI div over part of it.
The iframes ([frame.html](../examples/native-view-web/frame.html)) post the clicks, wheels, scrolls and selections they
get to the page. `window.gpuiFrames` counts GPUI's frames. `?animate` moves and resizes the views every GPUI frame;
`?noview` is the same window without them. A focusable GPUI field logs its keys; `window.nativeViewCommand("field")`
focuses it and `("drop <name>")` drops a view. `?offset` puts the canvas at (8, 48) under page content. How to run
it: [native-view-web.md](native-view-web.md); the checks: [native-view-web-checks.md](native-view-web-checks.md).

## native-view-windows

[examples/native-view-windows](../examples/native-view-windows/): a `NativeView` on Windows, the app's own composition
swap chain presented every vsync from its own thread, under an orange div; "move" (slides it back and forth for about
ten seconds) and "resize" buttons; logs GPUI
frames and view presents per second and the input its element hears. A platform-specific check, so it does nothing
on other targets (it still builds there). `cargo run -p native-view-windows`; checks: [native-view-windows.md](native-view-windows.md).

## native-view-webview2

[examples/native-view-webview2](../examples/native-view-webview2/): a local HTML page through WebView2's visual hosting
as a `NativeView` on Windows, in a visual on GPUI's DirectComposition device, under an orange div; the mouse, wheel
and keys over it go to the page, which posts its events and frame rate back to the log. Needs the WebView2 runtime.
`cargo run -p native-view-webview2`; checks: [native-view-webview2.md](native-view-webview2.md).

## native-view-apple

[examples/native-view-apple](../examples/native-view-apple/): a native view (`NSView` on macOS, `UIView` on iOS)
that recolours itself every frame under an orange GPUI div, with a GPUI frame counter; "Drop views" drops every
native view handle (on iOS it checks that a GPUI field gets the keyboard back); macOS and iOS only (other targets
print a line and exit). What it shows and how to run it: [native-view-apple.md](native-view-apple.md#the-example).

## native-view-android

[examples/native-view-android](../examples/native-view-android/) (Android only): a surface the app fills every frame
from its own thread, with a GPUI div over it, and a WebView with forwarded touches, both under GPUI's surface.
`cargo parallax run android` in its directory; extras `--es surface view`, `--ez animate true`. Logcat says
`gpui frames/s N, surface frames/s M` once a second. See [native-view-android.md](native-view-android.md).
