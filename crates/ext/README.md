# parallax-ext

Flutter's text selection behaviour for Android and touch desktops, as a pure state machine (`text_selection`):
pointer input in (taps, double and triple taps, long presses, finger and mouse drags, handle touches), the new
selection and the overlay (handles, toolbar anchors and buttons, the magnifier's position) out, by each target's row of
the [parallax-platform](https://crates.io/crates/parallax-platform) behaviour table. Your app draws the overlay; iOS
keeps UIKit's selection. Part of [Parallax](https://github.com/xuckless/parallax), built on GPUI (`parallax-gpui`:
upstream `gpui-pre` plus Parallax patches); the `parallax` facade re-exports it as `parallax::parallax_ext`.

How to use it: [docs/text-selection.md](https://github.com/xuckless/parallax/blob/main/docs/text-selection.md); the
worked example is [hello's text field](https://github.com/xuckless/parallax/blob/main/examples/hello/src/text_field.rs).
Copyright 2026 Syed Ali (xuckless). Licensed Apache-2.0 AND BSD-3-Clause: the engine ports code from Flutter
(Copyright 2014 The Flutter Authors, LICENSE-FLUTTER).
