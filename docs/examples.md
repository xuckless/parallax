# Examples

[`examples/`](../examples/) holds one app built for every target ([AGENTS.md](../AGENTS.md), Repo layout). Each
example is a workspace member, so [CI](ci.md) checks it on all six targets.

## hello

[examples/hello](../examples/hello/): one window with three text fields (prose, an email field with an autofill
hint, and a secure password field drawn in the clear), a line in several scripts (`SCRIPTS`: Latin, CJK, Arabic, emoji) and a box. Each click or tap moves the box to
the next colour of `COLOURS` in [lib.rs](../examples/hello/src/lib.rs), and the log line says whether the click came
from a mouse, a tap or a key; a tap outside a field blurs it. A two-finger or trackpad pinch resizes the box within
`ZOOM` and its label shows the zoom and the pinch's rotation; `hello: pinch end: …` logs each end
([pinch.md](pinch.md)). Hovering or long-pressing the box shows its colour as a tooltip; the long press plays the
platform's long-press haptic ([haptics.md](haptics.md)) with no app code. It is the app of Milestone 0.1.1 in
[ROADMAP.md](../ROADMAP.md): it opens, draws and reacts to a tap on desktop, web and Android.

Two Android checks ride along: 直骨 in `SCRIPTS` change glyph form with the app's language
([android-fonts.md](android-fonts.md#locale-changes)), and "Show emoji" swaps the box for 100 distinct emoji
(`EMOJI_GRID`) and logs `hello: emoji grid: frame …`, the cost of drawing them ([android-emoji.md](android-emoji.md#cost)).
The grid scrolls in a box shorter than its rows, so a swipe past an edge shows the platform's overscroll (stretch on
Android, bounce on iOS and macOS: [overscroll.md](overscroll.md)); tapping a row logs `hello: emoji row N tapped`,
and a swipe, sideways too, never does ([touch-drag.md](touch-drag.md)). Ctrl-W closes the window 3 s later (Android
finishes its Activity: [android-multi-window-check.md](android-multi-window-check.md)). "Record touches" swaps the window for a trace pad
that logs every touch in the trace format ([traces.md](traces.md)). `hello: lifecycle …` logs each app lifecycle
phase and `hello: keyboard inset …` each change of the keyboard's inset ([web-lifecycle.md](web-lifecycle.md)).
`hello::text_field: text field selection: base..extent` logs each selection the selection engine sets, offsets only
([text-selection-tests.md](text-selection-tests.md#live-checks)).

The field ([text_field.rs](../examples/hello/src/text_field.rs), configured per instance) is example-local:
upstream GPUI has no text field outside Zed's editor (GPL, never copied), so it is upstream's `examples/input.rs`
(Apache-2.0) cut down to one `EntityInputHandler` with UTF-16 ranges, composition and grapheme deletes; on Android
it drives the IME ([android-text-input.md](android-text-input.md)). A paint that moves its caret calls
`invalidate_character_coordinates`, so the web's IME mirror follows the caret ([web-text-input.md](web-text-input.md)).

What it shows: one shared `launch(cx)` (setup, once per process) and `open_window(cx)` (one window), and a thin entry
point per target, all through the `parallax` facade.

| Target | Entry point | How it starts |
|---|---|---|
| Desktop | [main.rs](../examples/hello/src/main.rs) | `parallax::run`, then `launch` and `open_window`: the platform ([platform.md](platform.md)) with its behaviour row ([behaviour.md](behaviour.md)) |
| Android | `parallax::android_main!(launch, open_window)` in lib.rs: `open_window` per Activity | `ParallaxActivity` loads the `cdylib` ([android.md](android.md)) |
| Web | `start` (`#[wasm_bindgen(start)]`) in lib.rs: `parallax::run` | [index.html](../examples/hello/index.html) imports the wasm-pack module; `favicon.svg` links to the brand icon in [assets/brand/](../assets/brand/) |
| iOS | none yet | |

How to build and run it on each target: [examples-run.md](examples-run.md).

## native-view-wayland

[examples/native-view-wayland](../examples/native-view-wayland/) (Linux, Wayland only): a GPUI window with a native
view ([native-view-wayland.md](native-view-wayland.md)). The "page" ([page.rs](../examples/native-view-wayland/src/page.rs))
owns a `wl_surface` on GPUI's connection and commits a new 640×400 shm buffer to it about 60 times a second from its
own thread; a blue GPUI div covers part of it. Once a second it logs `gpui frames/s: N, page frames/s: M` (GPUI's is
0 while only the page changes), and it logs the alpha mode GPUI chose and every pointer event over the view with its
view-relative position. Flags: `--fps N` (page rate), `--animate` (the view moves and resizes every GPUI frame),
`--no-view` (the same window without the view, a cost baseline), `--seconds N` (quit after N seconds).
