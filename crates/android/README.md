# parallax-android

GPUI's `Platform` and `PlatformWindow` for Android, part of [Parallax](https://github.com/xuckless/parallax), built
on GPUI (`parallax-gpui`: upstream `gpui-pre` plus Parallax patches). GPUI runs on the Android main Looper and draws with `gpui_wgpu` (Vulkan, GL fallback).
`java/` holds `ParallaxActivity` and Flutter's touch, keyboard, text-input and autofill handling, ported as Java.
It covers touch, mouse and trackpad, hardware keys and Back, IME text input (Gboard, CJK composing), autofill,
system fonts with locale fallback, colour emoji drawn by Android, lifecycle and device-loss recovery.

```rust
fn launch(cx: &mut parallax::gpui::App) { /* bind keys, set globals: once per process */ }
fn open_window(cx: &mut parallax::gpui::App) { /* cx.open_window(...): once per Activity */ }
parallax::android_main!(launch, open_window); // the facade's macro also applies Android's gesture tuning
```

Pre-alpha, verified on the Android emulator (no real-device testing yet). Build and run apps with
[cargo-parallax](https://crates.io/crates/cargo-parallax). How it works:
[docs/android.md](https://github.com/xuckless/parallax/blob/main/docs/android.md).

Copyright 2026 Syed Ali (xuckless). Licensed Apache-2.0 AND BSD-3-Clause (code ported from Flutter, see
[PORTED.md](https://github.com/xuckless/parallax/blob/main/third_party/flutter/PORTED.md) and
[NOTICE](https://github.com/xuckless/parallax/blob/main/NOTICE)).
