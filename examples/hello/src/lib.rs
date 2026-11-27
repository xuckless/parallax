//! One window: a prose field, an email field (with autofill), a line in several scripts, and a centred box; each
//! click or tap moves the box to the next colour in `COLOURS`. Tapping outside a field blurs it (Android hides the
//! keyboard). A button swaps the box for a grid of `EMOJI_GRID` and back, logging how long the frame that draws
//! the grid took. Another swaps the whole window for a trace pad that logs every touch as a `touch(…)` line, the
//! recorder for gesture traces (docs/traces.md).
//! The same `launch` runs on every target: `main.rs` on desktop, `android_main!` on Android, `start` on the web.

mod text_field;

use parallax::gpui::{
    AnyElement, App, Autocapitalize, AutofillHint, Bounds, ClickEvent, Context, Entity,
    KeyboardType, MouseButton, TextInputAction, TextInputConfiguration, TouchEvent, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use web_time::Instant;

use text_field::TextField;

/// The box shows these in order, wrapping around. Add a row to add a colour.
const COLOURS: [u32; 4] = [0x3b82f6, 0xef4444, 0x22c55e, 0xeab308];

/// Shaping and fallback check: Latin, CJK (Chinese, Japanese), Arabic (right to left) and emoji in one line.
/// 直骨 are Han whose glyphs differ between Chinese and Japanese fonts: they follow the app's locales.
const SCRIPTS: &str = "Hello · 你好 · こんにちは · 直骨 · مرحبا · 😀👍🏽";

/// An emoji picker's first screen: 100 distinct emoji (smileys, then transport), every one in Android 9's font.
/// Each is a new cluster the first time, which Android draws on the main thread (docs/android-emoji.md).
const EMOJI_GRID: [std::ops::Range<u32>; 2] = [0x1F600..0x1F650, 0x1F680..0x1F694];

/// Emoji per grid row.
const GRID_COLUMNS: usize = 10;

/// The "Record touches" button: a colour nothing else on screen has, so scripts/record-traces.sh finds it on a
/// screenshot.
const RECORD_BUTTON: u32 = 0xa855f7;

struct Hello {
    /// Index into `COLOURS`.
    colour: usize,
    prose: Entity<TextField>,
    email: Entity<TextField>,
    screen: Screen,
}

/// What the window shows.
#[derive(Clone, Copy)]
enum Screen {
    /// The fields, the line of scripts, the buttons and what sits under them.
    Main(Shown),
    /// A trace pad: it takes every touch raw (no taps, scrolls or long presses) and logs each one, timed from the
    /// first (`None` until then). Only a relaunch leaves it.
    TracePad(Option<Instant>),
}

/// What sits under the button.
#[derive(Clone, Copy)]
enum Shown {
    /// The box that changes colour on each click.
    Box,
    /// `EMOJI_GRID` in rows of `GRID_COLUMNS`.
    EmojiGrid,
}

impl Render for Hello {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = match self.screen {
            Screen::Main(shown) => shown,
            Screen::TracePad(_) => return trace_pad(cx),
        };
        // Inside the status and navigation bars and any cutout on every side (in landscape the navigation bar sits
        // left or right); the keyboard covers the bottom, so the field sits at the top.
        let safe = window.insets().safe_area;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(24.))
            .pt(safe.top + px(24.))
            .pr(safe.right + px(24.))
            .pb(safe.bottom)
            .pl(safe.left + px(24.))
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .text_size(px(20.))
            .on_mouse_down(MouseButton::Left, |_, window, cx| window.blur(cx))
            .child(self.prose.clone())
            .child(self.email.clone())
            .child(SCRIPTS)
            .child(
                div()
                    .id("trace-toggle")
                    .px(px(16.))
                    .py(px(8.))
                    .rounded_md()
                    .bg(rgb(RECORD_BUTTON))
                    .child("Record touches")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.screen = Screen::TracePad(None);
                        log::info!("trace: recording");
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("grid-toggle")
                    .px(px(16.))
                    .py(px(8.))
                    .rounded_md()
                    .bg(rgb(0x45475a))
                    .child(match shown {
                        Shown::Box => "Show emoji",
                        Shown::EmojiGrid => "Hide emoji",
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.screen = Screen::Main(match shown {
                            Shown::Box => {
                                // Logs the time from the start of the frame that draws the grid to the start of
                                // the next: that frame's draw and present, rounded up to a vsync (16.7 ms at 60 Hz
                                // when nothing is slow). Next-frame callbacks run before the draw.
                                window.on_next_frame(|window, cx| {
                                    let started = cx.background_executor().now();
                                    window.on_next_frame(move |_, cx| {
                                        let took = cx.background_executor().now() - started;
                                        log::info!("emoji grid: frame {took:?}");
                                    });
                                });
                                Shown::EmojiGrid
                            }
                            Shown::EmojiGrid => Shown::Box,
                        });
                        cx.notify();
                    })),
            )
            .child(match shown {
                // Near a picker's emoji size; rows of ten fit a phone's width and all rows its height.
                Shown::EmojiGrid => {
                    let emoji: Vec<char> = EMOJI_GRID
                        .iter()
                        .cloned()
                        .flatten()
                        .filter_map(char::from_u32)
                        .collect();
                    div()
                        .text_size(px(28.))
                        .children(
                            emoji
                                .chunks(GRID_COLUMNS)
                                .map(|row| row.iter().collect::<String>()),
                        )
                        .into_any_element()
                }
                Shown::Box => div()
                    .id("box")
                    .size(px(160.))
                    .rounded_lg()
                    .bg(rgb(COLOURS[self.colour]))
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.colour = (this.colour + 1) % COLOURS.len();
                        let source = match event {
                            ClickEvent::Mouse(_) => "mouse",
                            ClickEvent::Keyboard(_) => "keyboard",
                            ClickEvent::Touch(_) => "touch",
                        };
                        log::info!(
                            "box colour {:06x} ({source} click, count {})",
                            COLOURS[this.colour],
                            event.click_count()
                        );
                        cx.notify();
                    }))
                    .into_any_element(),
            })
            .into_any_element()
    }
}

/// The trace recorder: one `trace: touch(at_ms, id, phase, x, y)` line per touch GPUI receives, in the trace format
/// of docs/gesture-recognizers.md (ms since the first touch, window logical px), ready to paste into a fixture.
fn trace_pad(cx: &mut Context<Hello>) -> AnyElement {
    div()
        .id("trace-pad")
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(0x11111b))
        .text_color(rgb(0xcdd6f4))
        .child("Recording touches")
        .on_touch(cx.listener(|this, event: &TouchEvent, _, cx| {
            let now = cx.background_executor().now();
            let Screen::TracePad(first) = &mut this.screen else {
                return;
            };
            let at_ms = now.duration_since(*first.get_or_insert(now)).as_millis();
            let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
            log::info!(
                "trace: touch({at_ms}, {}, {:?}, {x:.2}, {y:.2})",
                event.id.0,
                event.phase
            );
        }))
        .into_any_element()
}

/// Opens the one window. Android and the web size it themselves and ignore the bounds.
pub fn launch(cx: &mut App) {
    text_field::bind_keys(cx);
    let bounds = Bounds::centered(None, size(px(400.), px(400.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|cx| Hello {
            colour: 0,
            // Prose: the keyboard may suggest, correct and capitalize (GPUI's default turns all of that off).
            prose: cx.new(|cx| {
                TextField::new(
                    "Type here",
                    TextInputConfiguration {
                        autocorrect: true,
                        autocapitalize: Autocapitalize::Sentences,
                        suggestions: true,
                        input_action: TextInputAction::Done,
                        ..TextInputConfiguration::default()
                    },
                    cx,
                )
            }),
            // The platform's autofill may offer the user's addresses.
            email: cx.new(|cx| {
                TextField::new(
                    "Email",
                    TextInputConfiguration {
                        keyboard_type: KeyboardType::Email,
                        autofill: Some(AutofillHint::Email),
                        input_action: TextInputAction::Done,
                        ..TextInputConfiguration::default()
                    },
                    cx,
                )
            }),
            screen: Screen::Main(Shown::Box),
        })
    });
    match opened {
        Ok(_) => log::info!("window open"),
        Err(error) => log::error!("cannot open the window: {error:#}"),
    }
    cx.activate(true);
}

parallax::android_main!(launch);

/// Fonts the web build embeds: a browser gives GPUI no system fonts, and with none loaded the first text draw
/// panics. Noto Sans is in GPUI's fallback stack for `.SystemUIFont`; Noto Sans Arabic covers `SCRIPTS`' Arabic.
/// Emoji are drawn by the browser (gpui_web's Canvas fallback); CJK is not embedded (a CJK font is many MB), so
/// 你好 · こんにちは · 直骨 show as missing glyphs on the web. Add a row to add a font (OFL, see fonts/OFL.txt).
#[cfg(target_family = "wasm")]
const WEB_FONTS: [&[u8]; 2] = [
    include_bytes!("../fonts/NotoSans-Regular.ttf"),
    include_bytes!("../fonts/NotoSansArabic.ttf"),
];

/// Web entry point: wasm-bindgen runs it when the module loads.
#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    parallax::parallax_platform::web_init();
    // parallax::run applies the platform behaviour table first (docs/behaviour.md).
    let started = parallax::run(|cx| {
        let fonts = WEB_FONTS.iter().map(|font| (*font).into()).collect();
        match cx.text_system().add_fonts(fonts) {
            Ok(()) => launch(cx),
            Err(error) => log::error!("cannot load the embedded fonts: {error:#}"),
        }
    });
    if let Err(error) = started {
        log::error!("no web platform: {error}");
    }
}
