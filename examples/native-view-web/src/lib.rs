//! A GPUI window in the browser with four DOM elements under its canvas as native views: a same-origin iframe, a
//! cross-origin iframe, a canvas that index.html redraws every animation frame and a form (a box with an `<input>`).
//! A GPUI div covers part of each and logs the presses it takes; a focusable GPUI field logs the keys it gets. The
//! page reads `window.gpuiFrames` (frames GPUI painted, which must not grow while only the elements change) and can
//! call `window.nativeViewCommand("field")` (focus the field) or `("drop <name>")` (drop that view). GPUI logs the surface alpha mode it chose. `?animate` moves and resizes the views every GPUI frame. See
//! docs/native-view-web.md; scripts/web-checks.mjs drives it.

#[cfg(target_family = "wasm")]
mod demo {
    use parallax::gpui::{
        App, Context, FocusHandle, MouseButton, NativeView, Window, WindowOptions, canvas, div,
        prelude::*, px, rgb,
    };
    use std::{cell::Cell, ptr::NonNull, rc::Rc};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use web_time::Instant;

    /// Text needs a font, and a browser gives GPUI none (hello's, OFL).
    const FONT: &[u8] = include_bytes!("../../hello/fonts/NotoSans-Regular.ttf");

    /// The views by the name index.html gives their elements, and where each goes, in CSS pixels:
    /// (name, left, top). Each is `VIEW` big, with its div `OVER` big at `OVER_AT` inside it. Add a row per view.
    const VIEWS: [(&str, f32, f32); 4] = [
        ("same", 20., 60.),
        ("cross", 460., 60.),
        ("canvas", 20., 360.),
        ("form", 460., 360.),
    ];
    const VIEW: (f32, f32) = (400., 260.);
    const OVER: (f32, f32) = (160., 60.);
    const OVER_AT: (f32, f32) = (20., 20.);

    struct Demo {
        views: Vec<(&'static str, NativeView)>,
        /// A GPUI field that takes keys; `nativeViewCommand` focuses it.
        field: FocusHandle,
        /// Frames GPUI painted, counted by a canvas in the root.
        gpui_frames: Rc<Cell<u64>>,
        animate: bool,
        started: Instant,
    }

    fn page_value(name: &str) -> JsValue {
        web_sys::window()
            .and_then(|window| js_sys::Reflect::get(&window, &JsValue::from_str(name)).ok())
            .unwrap_or(JsValue::UNDEFINED)
    }

    impl Demo {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let elements = page_value("nativeViewElements");
            let views = VIEWS
                .iter()
                .filter_map(|&(name, _, _)| {
                    let element = js_sys::Reflect::get(&elements, &JsValue::from_str(name))
                        .ok()?
                        .dyn_into::<web_sys::HtmlElement>()
                        .ok()?;
                    // SAFETY: a live HtmlElement, borrowed for the call; GPUI keeps its own reference.
                    let view =
                        unsafe { NativeView::new(NonNull::from(&element).cast(), window, cx) };
                    match view {
                        Ok(view) => {
                            // The pages' and the canvas's top colour, for the frame a resized element lags.
                            view.set_background(rgb(0xffffff), cx);
                            Some((name, view))
                        }
                        Err(error) => {
                            log::error!("native-view-web: no native view {name}: {error:#}");
                            None
                        }
                    }
                })
                .collect();
            // The page's way in, for the focus checks: the page lives as long as the app, so the closure is leaked.
            let demo = cx.entity().downgrade();
            let mut app = cx.to_async();
            let command = Closure::<dyn FnMut(String)>::new(move |command: String| {
                demo.update_in(&mut app, |demo, window, cx| {
                    match command.strip_prefix("drop ") {
                        Some(name) => demo.views.retain(|(view_name, _)| *view_name != name),
                        None => window.focus(&demo.field, cx),
                    }
                    log::info!("native-view-web: command {command}");
                    cx.notify();
                })
                .ok();
            });
            if let Some(page) = web_sys::window() {
                js_sys::Reflect::set(
                    &page,
                    &JsValue::from_str("nativeViewCommand"),
                    command.as_ref(),
                )
                .ok();
            }
            command.forget();
            Self {
                views,
                field: cx.focus_handle(),
                gpui_frames: Rc::new(Cell::new(0)),
                animate: page_value("nativeViewAnimate").is_truthy(),
                started: Instant::now(),
            }
        }
    }

    impl Render for Demo {
        fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            // Moves and resizes every view each frame, to check the hole and the element stay in step.
            let (shift, grow) = if self.animate {
                window.request_animation_frame();
                let t = self.started.elapsed().as_secs_f32();
                (12. * (t * 2.1).sin(), 30. * (t * 1.7).sin().abs())
            } else {
                (0., 0.)
            };
            let frames = self.gpui_frames.clone();
            div()
                .size_full()
                .bg(rgb(0x202020))
                .text_color(rgb(0xffffff))
                .child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, _, _| {
                            frames.set(frames.get() + 1);
                            if let Some(page) = web_sys::window() {
                                js_sys::Reflect::set(
                                    &page,
                                    &JsValue::from_str("gpuiFrames"),
                                    &JsValue::from_f64(frames.get() as f64),
                                )
                                .ok();
                            }
                        },
                    )
                    .size_0(),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(20.))
                        .left(px(20.))
                        .child("Native views under a GPUI canvas"),
                )
                .child(
                    div()
                        .id("field")
                        .track_focus(&self.field)
                        .absolute()
                        .top(px(16.))
                        .left(px(460.))
                        .w(px(400.))
                        .h(px(32.))
                        .px_2()
                        .bg(rgb(0x404040))
                        .on_key_down(|event, _, _| {
                            log::info!("native-view-web: field key {}", event.keystroke.key)
                        })
                        .child("A GPUI field"),
                )
                .children(VIEWS.iter().map(|&(name, left, top)| {
                    let view = self
                        .views
                        .iter()
                        .find(|(view_name, _)| *view_name == name)
                        .map(|(_, view)| view.clone());
                    div()
                        .absolute()
                        .left(px(left + shift))
                        .top(px(top + shift))
                        .w(px(VIEW.0 - grow))
                        .h(px(VIEW.1 - grow))
                        // Shows where the view isn't.
                        .bg(rgb(0xc000c0))
                        .children(view)
                        // Painted after the view, so it covers it and takes its own input.
                        .child(
                            div()
                                .id(name)
                                .absolute()
                                .left(px(OVER_AT.0))
                                .top(px(OVER_AT.1))
                                .w(px(OVER.0))
                                .h(px(OVER.1))
                                .p_2()
                                .bg(rgb(0x2050d0))
                                .occlude()
                                .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                    log::info!("native-view-web: GPUI div over {name}: down")
                                })
                                .on_scroll(move |_, _, _| {
                                    log::info!("native-view-web: GPUI div over {name}: scroll")
                                })
                                .child(format!("GPUI over {name}")),
                        )
                }))
        }
    }

    pub fn open(cx: &mut App) {
        if let Err(error) = cx.text_system().add_fonts(vec![FONT.into()]) {
            log::error!("native-view-web: cannot load the font: {error:#}");
            return;
        }
        match cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| Demo::new(window, cx))
        }) {
            Ok(_) => log::info!("native-view-web: window open"),
            Err(error) => log::error!("native-view-web: cannot open the window: {error:#}"),
        }
    }
}

/// Web entry point: wasm-bindgen runs it when the module loads.
#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    parallax::parallax_platform::web_init();
    if let Err(error) = parallax::run(demo::open) {
        log::error!("native-view-web: no web platform: {error}");
    }
}
