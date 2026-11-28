# native-view-webview2

A WebView2 page as a `NativeView` on Windows, rendered into a visual on GPUI's own DirectComposition device
(fork patch 0056). How it works and how CI checks it: [docs/native-view-webview2.md](../../docs/native-view-webview2.md).

```sh
cargo run -p native-view-webview2
```

Needs the WebView2 Runtime (part of Windows 10 and 11).

## Credits

- `WebView2Loader` (`WebView2Loader.dll` / `WebView2LoaderStatic.lib`) is Microsoft's, from the
  [Microsoft.Web.WebView2](https://www.nuget.org/packages/Microsoft.Web.WebView2) SDK, Copyright (C) Microsoft
  Corporation, under that SDK's licence (`LICENSE.txt` in the package). It is bundled into this example's build by
  [webview2-com-sys](https://crates.io/crates/webview2-com-sys).
- [webview2-com](https://github.com/wravery/webview2-rs) (MIT), the Rust bindings to the WebView2 COM API.

This example is never published (`publish = false`), so the credit lives here rather than in the repo's `NOTICE`.
