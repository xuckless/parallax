# parallax-platform

Picks the GPUI platform for the current target from one table: upstream `gpui_platform` on desktop, web and iOS,
[parallax-android](https://crates.io/crates/parallax-android) on Android. Part of
[Parallax](https://github.com/xuckless/parallax), built on GPUI (`parallax-gpui`: upstream `gpui-pre` plus Parallax patches).

It also holds the target → behaviour table: Flutter's per-platform gesture slops, timeouts, scroll physics, overscroll,
haptics and touch-target minimum. The `parallax` facade's entry points (`parallax::run`, `parallax::android_main!`)
hand its gesture tuning to GPUI (`App::set_gesture_tuning`) before the app launches.

How it works and how to add a target: [docs/platform.md](https://github.com/xuckless/parallax/blob/main/docs/platform.md),
[docs/behaviour.md](https://github.com/xuckless/parallax/blob/main/docs/behaviour.md).
Copyright 2026 Syed Ali (xuckless). Licensed Apache-2.0 AND BSD-3-Clause: the behaviour table ports code from Flutter
(Copyright 2014 The Flutter Authors, LICENSE-FLUTTER).
