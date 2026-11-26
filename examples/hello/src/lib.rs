//! One window with a centred box; each click or tap moves it to the next colour in `COLOURS`.
//! The same `launch` runs on every target: `main.rs` on desktop, `android_main!` on Android, `start` on the web.

use parallax::gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};

/// The box shows these in order, wrapping around. Add a row to add a colour.
const COLOURS: [u32; 4] = [0x3b82f6, 0xef4444, 0x22c55e, 0xeab308];

struct Hello {
    /// Index into `COLOURS`.
    colour: usize,
}

impl Render for Hello {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x1e1e2e))
            .child(
                div()
                    .id("box")
                    .size(px(160.))
                    .rounded_lg()
                    .bg(rgb(COLOURS[self.colour]))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.colour = (this.colour + 1) % COLOURS.len();
                        log::info!("box colour {:06x}", COLOURS[this.colour]);
                        cx.notify();
                    })),
            )
    }
}

/// Opens the one window. Android and the web size it themselves and ignore the bounds.
pub fn launch(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(400.), px(400.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    match cx.open_window(options, |_, cx| cx.new(|_| Hello { colour: 0 })) {
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
