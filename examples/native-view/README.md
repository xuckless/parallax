# native-view

An OS-native view under a GPUI window as a `NativeView`, on every target: what it shows, its options and its log
lines are in [docs/examples-native-view.md](../../docs/examples-native-view.md).

```sh
cargo run -p native-view                         # desktop: Wayland, Windows or macOS
cargo run -p native-view -- --view webview2      # Windows: a WebView2 page
```

`--view webview2` needs the WebView2 Runtime (part of Windows 10 and 11). How it works and how CI checks it:
[docs/native-view-webview2.md](../../docs/native-view-webview2.md).

## Credits

- `WebView2Loader` (`WebView2Loader.dll` / `WebView2LoaderStatic.lib`) is Microsoft's, from the
  [Microsoft.Web.WebView2](https://www.nuget.org/packages/Microsoft.Web.WebView2) SDK, Copyright (C) Microsoft
  Corporation, under that SDK's licence (`LICENSE.txt` in the package). It is bundled into this example's Windows
  build by [webview2-com-sys](https://crates.io/crates/webview2-com-sys).
- [webview2-com](https://github.com/wravery/webview2-rs) (MIT), the Rust bindings to the WebView2 COM API.
- The web build embeds hello's Noto Sans (OFL, [../hello/fonts](../hello/fonts/)).

This example is never published (`publish = false`), so the credit lives here rather than in the repo's `NOTICE`.
