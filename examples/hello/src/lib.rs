//! One window: a text field, a line in several scripts, and a centred box; each click or tap moves the box to
//! the next colour in `COLOURS`. Tapping outside the field blurs it (Android hides the keyboard). A button swaps the
//! box for a grid of `EMOJI_GRID` and back, logging how long the frame that draws the grid took.
//! The same `launch` runs on every target: `main.rs` on desktop, `android_main!` on Android, `start` on the web.

mod text_field;

use parallax::gpui::{
    App, Bounds, Context, Entity, MouseButton, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

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

struct Hello {
    /// Index into `COLOURS`.
    colour: usize,
    field: Entity<TextField>,
    /// The emoji grid's rows while it is shown.
    grid: Option<Vec<String>>,
}

impl Render for Hello {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Below the status bar; the keyboard covers the bottom, so the field sits at the top.
        let top = window.insets().safe_area.top;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(24.))
            .pt(top + px(24.))
            .px(px(24.))
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .text_size(px(20.))
            .on_mouse_down(MouseButton::Left, |_, window, cx| window.blur(cx))
            .child(self.field.clone())
            .child(SCRIPTS)
            .child(
                div()
                    .id("grid-toggle")
                    .px(px(16.))
                    .py(px(8.))
                    .rounded_md()
                    .bg(rgb(0x45475a))
                    .child(if self.grid.is_some() {
                        "Hide emoji"
                    } else {
                        "Show emoji"
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.grid = match this.grid {
                            Some(_) => None,
                            None => {
                                log_grid_frame(window);
                                Some(grid_rows())
                            }
                        };
                        cx.notify();
                    })),
            )
            .child(match &self.grid {
                // Near a picker's emoji size; rows of ten fit a phone's width and all rows its height.
                Some(rows) => div()
                    .text_size(px(28.))
                    .children(rows.iter().cloned())
                    .into_any_element(),
                None => div()
                    .id("box")
                    .size(px(160.))
                    .rounded_lg()
                    .bg(rgb(COLOURS[self.colour]))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.colour = (this.colour + 1) % COLOURS.len();
                        log::info!("box colour {:06x}", COLOURS[this.colour]);
                        cx.notify();
                    }))
                    .into_any_element(),
            })
    }
}

/// `EMOJI_GRID` as text rows of `GRID_COLUMNS`.
fn grid_rows() -> Vec<String> {
    let emoji: Vec<char> = EMOJI_GRID
        .iter()
        .cloned()
        .flatten()
        .filter_map(char::from_u32)
        .collect();
    emoji
        .chunks(GRID_COLUMNS)
        .map(|row| row.iter().collect())
        .collect()
}

/// Logs the time from the start of the frame that draws the grid to the start of the next: that frame's draw and
/// present, rounded up to a vsync (16.7 ms at 60 Hz when nothing is slow). Next-frame callbacks run before the draw.
fn log_grid_frame(window: &mut Window) {
    window.on_next_frame(|window, cx| {
        let started = cx.background_executor().now();
        window.on_next_frame(move |_, cx| {
            let took = cx.background_executor().now() - started;
            log::info!("emoji grid: frame {took:?}");
        });
    });
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
            field: cx.new(|cx| TextField::new("Type here", cx)),
            grid: None,
        })
    });
    match opened {
        Ok(_) => log::info!("window open"),
        Err(error) => log::error!("cannot open the window: {error:#}"),
    }
    cx.activate(true);
}

parallax::android_main!(launch);

/// Web entry point: wasm-bindgen runs it when the module loads.
#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    parallax::parallax_platform::web_init();
    match parallax::parallax_platform::current_platform(false) {
        Ok(platform) => parallax::gpui::Application::with_platform(platform).run(launch),
        Err(error) => log::error!("no web platform: {error}"),
    }
}
