//! One window: a prose field, an email field (with autofill), a line in several scripts, and a centred box; each
//! click or tap moves the box to the next colour in `COLOURS`, and a two-finger (or trackpad) pinch resizes it
//! within `ZOOM`, showing the zoom and the pinch's rotation; hovering or long-pressing it shows its colour as a
//! tooltip, and a long press plays the platform's long-press haptic (docs/haptics.md). Tapping outside a field
//! blurs it (Android hides the keyboard). A button swaps the box for a grid of `EMOJI_GRID` and back, logging how
//! long the frame that draws the grid took. Another swaps the whole window for a trace pad that logs every touch as
//! a `touch(…)` line, the recorder for gesture traces (docs/traces.md). It logs each app lifecycle phase and each change
//! of the keyboard inset.
//! The same `launch` runs on every target: `main.rs` on desktop, `android_main!` on Android, `start` on the web.

mod text_field;

use parallax::gpui::{
    AnyElement, App, Autocapitalize, AutofillHint, Bounds, ClickEvent, Context, Entity,
    KeyboardType, MouseButton, PinchEvent, Pixels, TextInputAction, TextInputConfiguration,
    TouchEvent, TouchPhase, TouchPinch, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use web_time::Instant;

use text_field::TextField;

/// The box shows these in order, wrapping around. Add a row to add a colour.
const COLOURS: [u32; 4] = [0x3b82f6, 0xef4444, 0x22c55e, 0xeab308];

/// How far a pinch may shrink or grow the box (160 px at 1).
const ZOOM: std::ops::RangeInclusive<f32> = 0.5..=2.0;

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
    /// The box's size factor, within `ZOOM`.
    zoom: f32,
    /// The last touch pinch's rotation since it started, in degrees.
    rotation: f32,
    prose: Entity<TextField>,
    email: Entity<TextField>,
    screen: Screen,
    /// The keyboard's inset (`WindowInsets::ime`) last drawn, logged when it changes.
    keyboard: Pixels,
}

/// What the window shows.
#[derive(Clone, Copy)]
enum Screen {
    /// The fields, the line of scripts, the buttons and what sits under them.
    Main(Shown),
    /// A trace pad: it takes every touch raw (no taps, scrolls or long presses) and logs each one, timed by its
    /// platform time stamp from the first (`None` until then). Only a relaunch leaves it.
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
        let insets = window.insets();
        let safe = insets.safe_area;
        if insets.ime.bottom != self.keyboard {
            self.keyboard = insets.ime.bottom;
            log::info!("keyboard inset {}", f32::from(self.keyboard));
        }
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
                // Near a picker's emoji size; rows of ten fit a phone's width. The grid scrolls in a box a little
                // shorter than its rows, so a touch past either edge shows the platform's overscroll (stretch on
                // Android, bounce on iOS and macOS; docs/overscroll.md).
                Shown::EmojiGrid => {
                    let emoji: Vec<char> = EMOJI_GRID
                        .iter()
                        .cloned()
                        .flatten()
                        .filter_map(char::from_u32)
                        .collect();
                    div()
                        .id("emoji-grid")
                        .h(px(300.))
                        .overflow_y_scroll()
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
                    .size(px(160.) * self.zoom)
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_lg()
                    .bg(rgb(COLOURS[self.colour]))
                    .text_color(rgb(0x1e1e2e))
                    .child(format!("×{:.2} {:.0}°", self.zoom, self.rotation))
                    .on_pinch(cx.listener(|this, event: &PinchEvent, _, cx| {
                        // `delta` is the change since the last event, from a trackpad or the fingers alike.
                        this.zoom =
                            (this.zoom * (1. + event.delta)).clamp(*ZOOM.start(), *ZOOM.end());
                        match (&event.touch, event.phase) {
                            (Some(TouchPinch::Update(update)), _) => {
                                this.rotation = update.rotation.to_degrees()
                            }
                            (Some(TouchPinch::End(end)), _) => log::info!(
                                "pinch end: zoom {:.2}, {} fingers left, velocity {:?}",
                                this.zoom,
                                end.pointer_count,
                                end.velocity
                            ),
                            // A trackpad's, so a macOS check can tell AppKit's magnify copy of a touch pinch
                            // (docs/desktop-touch-macos.md) from the fingers' own.
                            (None, TouchPhase::Ended) => {
                                log::info!("trackpad pinch end: zoom {:.2}", this.zoom)
                            }
                            (Some(TouchPinch::Start(_)), _) | (None, _) => {}
                        }
                        cx.notify();
                    }))
                    .tooltip({
                        let colour = COLOURS[self.colour];
                        move |_, cx| cx.new(|_| ColourTip(colour)).into()
                    })
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

/// The box's tooltip: its colour. On touch a long press shows it, which claims the press, so GPUI plays the
/// behaviour table's long-press haptic itself (docs/haptics.md); the app plays none.
struct ColourTip(u32);

impl Render for ColourTip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded_md()
            .bg(rgb(0x313244))
            .text_color(rgb(0xcdd6f4))
            .child(format!("#{:06x}", self.0))
    }
}

/// The trace recorder: one `trace: touch(at_ms, id, phase, x, y, device)` line per touch GPUI receives, in the trace format
/// of docs/gesture-recognizers.md (ms since the first touch by the platform's event time, `TouchEvent::time_stamp`,
/// or when GPUI received it where the platform has none; window logical px), ready to paste into a fixture, then
/// ` force <f>` when the touch has one. Mouse presses and moves log `trace: mouse_down(…)` / `trace: mouse_move(…)`.
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
            let at = event
                .time_stamp
                .unwrap_or_else(|| cx.background_executor().now());
            let Screen::TracePad(first) = &mut this.screen else {
                return;
            };
            let at_ms = at.duration_since(*first.get_or_insert(at)).as_millis();
            let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
            let force = event
                .force
                .map_or(String::new(), |force| format!(" force {force:.2}"));
            log::info!(
                "trace: touch({at_ms}, {}, {:?}, {x:.2}, {y:.2}, {:?}){force}",
                event.id.0,
                event.phase,
                event.device
            );
        }))
        // Mouse input the pad gets: touches never become mouse events here (`on_touch` opts out), so these come
        // from a mouse, or a pen hovering, as the platform sent them (scripts/touch-checks.sh asserts on them).
        .on_mouse_down(MouseButton::Left, |event, window, _| {
            log::info!(
                "trace: mouse_down({:.2}, {:.2}, {:?})",
                f32::from(event.position.x),
                f32::from(event.position.y),
                window.pointer_source()
            );
        })
        .on_mouse_move(|event, _, _| {
            log::info!(
                "trace: mouse_move({:.2}, {:.2})",
                f32::from(event.position.x),
                f32::from(event.position.y)
            );
        })
        .into_any_element()
}

/// Opens the one window. Android and the web size it themselves and ignore the bounds.
pub fn launch(cx: &mut App) {
    text_field::bind_keys(cx);
    // Tall enough for record-traces.sh's gestures (logical y up to 650) on desktop harnesses (docs/traces.md).
    let bounds = Bounds::centered(None, size(px(400.), px(700.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|cx| Hello {
            colour: 0,
            zoom: 1.,
            rotation: 0.,
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
            keyboard: px(0.),
        })
    });
    cx.on_app_lifecycle(|phase, _| log::info!("lifecycle {phase:?}"))
        .detach();
    match opened {
        // One per Android Activity (docs/android-multi-window.md): the count shows a destroyed one's window closed.
        Ok(_) => log::info!("window open, {} in all", cx.windows().len()),
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
