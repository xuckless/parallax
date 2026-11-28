# Examples: native views

Part of [examples.md](examples.md). [examples/native-view](../examples/native-view/) shows an OS-native view through
GPUI's hole ([native-view.md](native-view.md)) on every target: the view draws its own frames, a GPUI div covers part
of it, and GPUI draws nothing while only the view changes. One crate: the GPUI side is written once
([demo.rs](../examples/native-view/src/demo.rs)); a target adds only data and the ways it makes its views.

## The shared side

`demo.rs` draws each view in a slot (the slot's own colour under it where the target has one, the view's
`set_background` gap colour, and the cover div over its corner), moves the moving slots (`Motion`: a wave every frame,
or Windows' slide), and has the buttons ("Move", "Resize", "Hide", "Drop views"), GPUI's frame counter and the
once-a-second log. Where the window gets the pointer (Wayland, Windows, iOS, Android), a press on a view is followed with
window-level capture-phase listeners until its release, as a browser's own window capture would give it.

Each target module exports one `Target` ([lib.rs](../examples/native-view/src/lib.rs) picks it by `cfg`): its `Look`
(window size, colours, places, cover, layout, motion) and its `make_view` rows, picked by the `view` option:

| Target | Module | `make_view` rows | The view |
|---|---|---|---|
| Wayland | [wayland.rs](../examples/native-view/src/wayland.rs) | `wayland` | the app's `wl_surface`, a new shm buffer about 60 times a second from its own thread ([native-view-wayland.md](native-view-wayland.md)) |
| Windows | [windows.rs](../examples/native-view/src/windows.rs) | `swap-chain`, `webview2` | a composition swap chain presented every vsync; a WebView2 page in a visual on GPUI's device ([native-view-windows.md](native-view-windows.md), [webview2](native-view-webview2.md)) |
| macOS, iOS | [apple.rs](../examples/native-view/src/apple.rs) | `apple` | an `NSView` / `UIView` recoloured on a 60 Hz timer; iOS also a `UITextField` next to hello's field ([native-view-apple.md](native-view-apple.md)) |
| Android | [android.rs](../examples/native-view/src/android.rs) | `control`, `view`, `hosted` | a `SurfaceControl`, `SurfaceView` or `in_surface` surface filled from its own thread, and a WebView or (`stack`) two tiles ([native-view-android.md](native-view-android.md)) |
| Web | [web.rs](../examples/native-view/src/web.rs) | `dom` | two iframes, an animated canvas and a form that [index.html](../examples/native-view/index.html) makes ([native-view-web.md](native-view-web.md)) |

To add a target or a variant: a module with its `Target`, or a row in an existing one.

## Options

The same names everywhere: `--name [value]` on the desktop, intent extras on Android (`--ez animate true`,
`--es view view`), the page's query on the web (`?animate&no-view`).

| Option | What |
|---|---|
| `view NAME` | the `make_view` row (the first by default; Android below API 29 always makes a `view`) |
| `animate` | the moving slots move and resize every GPUI frame |
| `clipped` | the moving slot in a container scrolled by half its height, in a see-through window |
| `resize` | Wayland: the page switches its buffer size every 45 frames, each switch trailed by two old-size frames |
| `stack` | Android: two overlapping tiles in place of the WebView (the one attached first is painted last) |
| `no-view`, `fps N`, `seconds N` | the window without views (a cost baseline); Wayland's page rate; quit after N seconds |

## What it logs

One line format on every target, which every check script reads with the same regex
(`native-view: gpui frames/s ([0-9]+), view frames/s ([0-9]+)`): `native-view: gpui frames/s N, view frames/s M`
once a second. Pointer input over a view: `native-view: <view> pointer down|up|move|scroll at (x, y)`, relative to the
view; the cover's own: `native-view: div over <view>: down|scroll`. Also `native-view: view placement: device l t r b`
each frame for a view that sizes itself from it (Windows), `motion: start|end`, `back <phase>`, `dropped the native
views`, and per target `field key …` (web), `UITextField: …` (iOS), `page: …` (WebView2's posts).

The alpha mode is the renderer's own line (wgpu, gpui_windows, gpui_apple each word theirs in their patch); the scripts
read it with one regex: `alpha mode [A-Z][A-Za-z]+|alpha mode, only [A-Za-z]+|premultiplied alpha \(|metal layer
alpha: [a-z]+`.

## Running it

| Target | Command | Checks |
|---|---|---|
| Linux (Wayland) | `cargo run -p native-view -- [--animate] [--clipped] [--resize]` | [native-view-wayland-checks.md](native-view-wayland-checks.md), owner's look: [CHECK.md](../examples/native-view/CHECK.md) |
| Windows | `cargo run -p native-view [-- --view webview2]` (needs the WebView2 runtime) | [native-view-windows.md](native-view-windows.md#checks) |
| macOS | `cargo run -p native-view` | [native-view-apple-checks.md](native-view-apple-checks.md) |
| iOS | a bare .app, as [apple-native-view-checks.sh](../scripts/apple-native-view-checks.sh) builds it | [native-view-apple-checks.md](native-view-apple-checks.md) |
| Android | `cargo parallax run android` in `examples/native-view` | [native-view-android-checks.md](native-view-android-checks.md) |
| Web | [native-view-web-checks.md](native-view-web-checks.md#running-it) | the same |

WebView2's loader and bindings are credited in the example's [README.md](../examples/native-view/README.md).
