# parallax

One GPUI app on desktop, web, iOS and Android. This facade re-exports `gpui` (`parallax-gpui`: upstream `gpui-pre` plus Parallax patches),
[parallax-platform](https://crates.io/crates/parallax-platform),
[parallax-android](https://crates.io/crates/parallax-android) and
[parallax-ext](https://crates.io/crates/parallax-ext) (Flutter's text selection behaviour). Built on GPUI; not affiliated with Zed Industries.

Entry points apply each platform's feel (Flutter's gesture and scroll tuning) before your code runs:

```rust
fn launch(cx: &mut parallax::gpui::App) { /* bind keys, set globals: once per process */ }
fn open_window(cx: &mut parallax::gpui::App) { /* cx.open_window(...) */ }
fn main() -> Result<(), parallax::parallax_platform::Error> { // desktop, iOS, web
    parallax::run(|cx| { launch(cx); open_window(cx) })
}
parallax::android_main!(launch, open_window); // Android: open_window runs once per Activity
```

Pre-alpha. Start with the [hello example](https://github.com/xuckless/parallax/blob/main/docs/examples.md); plan and
status in [ROADMAP.md](https://github.com/xuckless/parallax/blob/main/ROADMAP.md). Copyright 2026 Syed Ali (xuckless). Licensed Apache-2.0; its dependencies `parallax-gpui`, `parallax-platform` and `parallax-ext` include code ported from Flutter
(Apache-2.0 AND BSD-3-Clause).
