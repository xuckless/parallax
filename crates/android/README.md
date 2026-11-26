# parallax-android

GPUI's `Platform` and `PlatformWindow` for Android, part of [Parallax](https://github.com/xuckless/parallax), built
on GPUI (`parallax-gpui`: upstream `gpui-pre` plus Parallax patches). GPUI runs on the Android main Looper and draws with `gpui_wgpu` (Vulkan, GL fallback).
`java/` holds `ParallaxActivity` and Flutter's touch handling, ported as Java.

```rust
fn launch(cx: &mut parallax::gpui::App) { /* cx.open_window(...) */ }
parallax::android_main!(launch);
```

Pre-alpha, verified on the Android emulator (no real-device testing yet). Build and run apps with
[cargo-parallax](https://crates.io/crates/cargo-parallax). How it works:
[docs/android.md](https://github.com/xuckless/parallax/blob/main/docs/android.md).

Copyright 2026 Syed Ali (xuckless). Licensed Apache-2.0 AND BSD-3-Clause (code ported from Flutter, see
[PORTED.md](https://github.com/xuckless/parallax/blob/main/third_party/flutter/PORTED.md) and
[NOTICE](https://github.com/xuckless/parallax/blob/main/NOTICE)).
