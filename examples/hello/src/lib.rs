//! One window: a prose field, an email field (with autofill), a line in several scripts, and a centred box; each
//! click or tap moves the box to the next colour in `COLOURS`. Tapping outside a field blurs it (Android hides the
//! keyboard).
//! The same `launch` runs on every target: `main.rs` on desktop, `android_main!` on Android, `start` on the web.

mod text_field;

use parallax::gpui::{
    App, Autocapitalize, AutofillHint, Bounds, ClickEvent, Context, Entity, KeyboardType,
    MouseButton, TextInputAction, TextInputConfiguration, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};

use text_field::TextField;

/// The box shows these in order, wrapping around. Add a row to add a colour.
const COLOURS: [u32; 4] = [0x3b82f6, 0xef4444, 0x22c55e, 0xeab308];

/// Shaping and fallback check: Latin, CJK (Chinese, Japanese), Arabic (right to left) and emoji in one line.
const SCRIPTS: &str = "Hello · 你好 · こんにちは · مرحبا · 😀👍🏽";

struct Hello {
    /// Index into `COLOURS`.
    colour: usize,
    prose: Entity<TextField>,
    email: Entity<TextField>,
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
            .child(self.prose.clone())
            .child(self.email.clone())
            .child(SCRIPTS)
            .child(
                div()
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
                    })),
            )
    }
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
            // The platform's autofill may offer the user's addresses. Suggestions on, as Flutter's default: Android
            // turns a field without suggestions into a visible-password one, which drops the email layout.
            email: cx.new(|cx| {
                TextField::new(
                    "Email",
                    TextInputConfiguration {
                        keyboard_type: KeyboardType::Email,
                        suggestions: true,
                        autofill: Some(AutofillHint::Email),
                        input_action: TextInputAction::Done,
                        ..TextInputConfiguration::default()
                    },
                    cx,
                )
            }),
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
