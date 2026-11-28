# native-view

An OS-native view inside a GPUI window, as a `NativeView`, on every target. The view draws its own frames, GPUI
shows it through a hole and covers one corner with a GPUI div, and the frame counter shows GPUI draws nothing while
only the view changes. A guide to the API: [docs/native-views-guide.md](../../docs/native-views-guide.md); options,
log lines and how each check reads them: [docs/examples-native-view.md](../../docs/examples-native-view.md).

## What each target shows

| Target | The native view | Try |
|---|---|---|
| Linux (Wayland) | the app's own `wl_surface`, a new colour about 60 times a second from its own thread | `--animate`, `--clipped` (half scrolled out of a container); X11 has no native views |
| Windows | a composition swap chain on its own D3D11 device, a new colour every vsync | "Move" slides it, "Resize" toggles its size; `--view webview2`: a WebView2 page you can click and scroll |
| macOS | an `NSView` recoloured on a 60 Hz timer; it takes the mouse over it itself | "Move", "Hide", "Drop views" |
| iOS | a `UIView` recoloured the same way, plus a `UITextField` next to a GPUI field: the keyboard moves between them | tap each field |
| Android | a surface filled with a new colour every frame from its own thread, and a WebView page scrolled and tapped through forwarded touches | Back; options are intent extras to `adb shell am start` (`--es view control\|view\|hosted`, `--ez stack true`: two overlapping tiles) |
| Web | two iframes (same and cross origin), a canvas redrawn every animation frame and a form with an `<input>`, from [index.html](index.html) | scroll and type in the frames; options are the page's query (`?animate`, `?no-view`) |

Every target logs `native-view: gpui frames/s N, view frames/s M` once a second (N stays 0 while only the views
change) and each pointer event over a view as `native-view: <view> pointer <kind> at (x, y)`.

## Run it

```sh
cd examples/native-view
cargo parallax run              # this machine's desktop: Wayland, Windows or macOS
cargo parallax run android      # or ios, web
cargo parallax run windows -- --view webview2   # Windows: everything after -- goes to the app
```

`--view webview2` needs the WebView2 Runtime (part of Windows 10 and 11). How it works and how CI checks it:
[docs/native-view-webview2.md](../../docs/native-view-webview2.md). The owner's look-by-eye check at 125 % on GNOME:
[CHECK.md](CHECK.md).

## Credits

- `WebView2Loader` (`WebView2Loader.dll` / `WebView2LoaderStatic.lib`) is Microsoft's, from the
  [Microsoft.Web.WebView2](https://www.nuget.org/packages/Microsoft.Web.WebView2) SDK, Copyright (C) Microsoft
  Corporation, under that SDK's licence (`LICENSE.txt` in the package). It is bundled into this example's Windows
  build by [webview2-com-sys](https://crates.io/crates/webview2-com-sys).
- [webview2-com](https://github.com/wravery/webview2-rs) (MIT), the Rust bindings to the WebView2 COM API.
- The web build embeds hello's Noto Sans (OFL, [../hello/fonts](../hello/fonts/)).

This example is never published (`publish = false`), so the credit lives here rather than in the repo's `NOTICE`.
