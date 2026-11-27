# Examples

[`examples/`](../examples/) holds one app built for every target ([AGENTS.md](../AGENTS.md), Repo layout). Each
example is a workspace member, so [CI](ci.md) checks it on all six targets.

## hello

[examples/hello](../examples/hello/): one window with two text fields (prose, and an email field with an autofill
hint), a line in several scripts (`SCRIPTS`: Latin, CJK, Arabic, emoji) and a box. Each click or tap moves the box to
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
Android, bounce on iOS and macOS: [overscroll.md](overscroll.md)). "Record touches" swaps the window for a trace pad
that logs every touch in the trace format ([traces.md](traces.md)). `hello: lifecycle …` logs each app lifecycle
phase and `hello: keyboard inset …` each change of the keyboard's inset ([web-lifecycle.md](web-lifecycle.md)).

The field ([text_field.rs](../examples/hello/src/text_field.rs), configured per instance) is example-local:
upstream GPUI has no text field outside Zed's editor (GPL, never copied), so it is upstream's `examples/input.rs`
(Apache-2.0) cut down to one `EntityInputHandler` with UTF-16 ranges, composition and grapheme deletes; on Android
it drives the IME ([android-text-input.md](android-text-input.md)). A paint that moves its caret calls
`invalidate_character_coordinates`, so the web's IME mirror follows the caret ([web-text-input.md](web-text-input.md)).

What it shows: one shared `launch(cx)` and a thin entry point per target, all through the `parallax` facade.

| Target | Entry point | How it starts |
|---|---|---|
| Desktop | [main.rs](../examples/hello/src/main.rs) | `parallax::run(launch)`: the platform ([platform.md](platform.md)) with its behaviour row ([behaviour.md](behaviour.md)) |
| Android | `parallax::android_main!(launch)` in lib.rs | `ParallaxActivity` loads the `cdylib` ([android.md](android.md)) |
| Web | `start` (`#[wasm_bindgen(start)]`) in lib.rs: `parallax::run` | [index.html](../examples/hello/index.html) imports the wasm-pack module; `favicon.svg` links to the brand icon in [assets/brand/](../assets/brand/) |
| iOS | none yet | |

How to build and run it on each target: [examples-run.md](examples-run.md).
