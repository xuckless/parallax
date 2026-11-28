//! The web (docs/native-view-web.md): four DOM elements index.html makes, under GPUI's canvas as native views: a
//! same-origin iframe, a cross-origin iframe, a canvas the page redraws every animation frame (the view frames of the
//! log line) and a form (a box with an `<input>`). A focusable GPUI field logs the keys it gets. The page reads
//! `window.gpuiFrames` (GPUI's frames, which must not grow while only the elements change) and can call
//! `window.nativeViewCommand("field")` (focus the field) or `("drop <name>")` (drop that view). Options come from
//! the page's query (`?animate`, `?no-view`; index.html hands them over as `window.nativeViewOptions`).

use std::ptr::NonNull;

use parallax::gpui::{
    AnyElement, App, Context, FocusHandle, NativeView, Window, div, prelude::*, px, rgb,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use crate::demo::{
    Cover, Demo, Edge, Extra, Layout, Look, Made, Motion, Options, Place, Shape, Shown, Target,
    Term,
};

/// Text needs a font, and a browser gives GPUI none (hello's, OFL).
const FONT: &[u8] = include_bytes!("../../hello/fonts/NotoSans-Regular.ttf");

/// A view 400 × 260 at `(left, top)`, by the name index.html gives its element.
const fn view(name: &'static str, left: f32, top: f32) -> Place {
    Place {
        name,
        half: 0,
        rect: [left, top, 400., 260.],
        cover: true,
        moves: true,
    }
}

pub const TARGET: Target = Target {
    look: Look {
        window: None,
        background: 0x202020,
        text: 0xffffff,
        // The pages' and the canvas's top colour, for the frame a resized element lags.
        underlay: 0xffffff,
        // GPUI's box under each view: magenta in a screenshot is a hole not cut.
        fill: Some(0xc000c0),
        cover: Cover {
            edge: Edge::Left,
            at: [20., 20.],
            size: [160., 60.],
            colour: 0x2050d0,
        },
        layout: Layout::Fixed(&[]),
        places: &[
            view("same", 20., 60.),
            view("cross", 460., 60.),
            view("canvas", 20., 360.),
            view("form", 460., 360.),
        ],
        buttons: 0x4060a0,
        // Shifted by ±12 px and shrunk by up to 30 px every frame.
        motion: Motion::Wave([
            Term::new(0., 12., 2.1, Shape::Sin),
            Term::new(0., 12., 2.1, Shape::Sin),
            Term::new(0., -30., 1.7, Shape::AbsSin),
            Term::new(0., -30., 1.7, Shape::AbsSin),
        ]),
        grow: [0., 0.],
    },
    variants: &[("dom", make_view)],
    // GPUI's frames for the page (`window.gpuiFrames`), with or without views.
    on_gpui_frame: Some(|frames| set_page_value("gpuiFrames", &JsValue::from_f64(frames as f64))),
};

fn page_value(name: &str) -> JsValue {
    web_sys::window()
        .and_then(|window| js_sys::Reflect::get(&window, &JsValue::from_str(name)).ok())
        .unwrap_or(JsValue::UNDEFINED)
}

fn set_page_value(name: &str, value: &JsValue) {
    if let Some(page) = web_sys::window() {
        js_sys::Reflect::set(&page, &JsValue::from_str(name), value).ok();
    }
}

/// The page's query as options (index.html's `window.nativeViewOptions`).
pub fn options() -> Options {
    let options = page_value("nativeViewOptions");
    Options::parse(|name| {
        js_sys::Reflect::get(&options, &JsValue::from_str(name))
            .ok()
            .and_then(|value| value.as_string())
    })
}

fn make_view(_: &Options, window: &mut Window, cx: &mut Context<Demo>) -> Result<Made, String> {
    let elements = page_value("nativeViewElements");
    let mut views = Vec::new();
    for place in TARGET.look.places {
        let Some(element) = js_sys::Reflect::get(&elements, &JsValue::from_str(place.name))
            .ok()
            .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
        else {
            continue;
        };
        // SAFETY: a live HtmlElement, borrowed for the call; GPUI keeps its own reference.
        let view = unsafe { NativeView::new(NonNull::from(&element).cast(), window, cx) }
            .map_err(|error| format!("{}: {error:#}", place.name))?;
        views.push(Shown::new(place.name, view, ()));
    }
    let field = cx.focus_handle();
    // The page's way in, for the focus checks: the page lives as long as the app, so the closure is leaked.
    let (demo, focus) = (cx.entity().downgrade(), field.clone());
    let mut app = cx.to_async();
    let command = Closure::<dyn FnMut(String)>::new(move |command: String| {
        demo.update_in(&mut app, |demo, window, cx| {
            match command.strip_prefix("drop ") {
                Some(name) => demo.drop_view(name, cx),
                None => window.focus(&focus, cx),
            }
            log::info!("native-view: command {command}");
        })
        .ok();
    });
    set_page_value("nativeViewCommand", command.as_ref());
    command.forget();
    let mut made = Made::new(views, || {
        page_value("elementFrames").as_f64().unwrap_or(0.) as u64
    });
    made.extra = Some(Box::new(Field(field)));
    Ok(made)
}

/// A GPUI field that takes keys; `nativeViewCommand("field")` focuses it.
struct Field(FocusHandle);

impl Extra for Field {
    fn render(&self, _: &mut Window, _: &mut App) -> AnyElement {
        div()
            .id("field")
            .track_focus(&self.0)
            .absolute()
            .top(px(16.))
            .left(px(460.))
            .w(px(400.))
            .h(px(32.))
            .px_2()
            .bg(rgb(0x404040))
            .on_key_down(|event, _, _| log::info!("native-view: field key {}", event.keystroke.key))
            .child("A GPUI field")
            .into_any_element()
    }
}

/// Web entry point: wasm-bindgen runs it when the module loads.
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    parallax::parallax_platform::web_init();
    let started = parallax::run(|cx| match cx.text_system().add_fonts(vec![FONT.into()]) {
        Ok(()) => crate::open_window(cx),
        Err(error) => log::error!("native-view: cannot load the font: {error:#}"),
    });
    if let Err(error) = started {
        log::error!("native-view: no web platform: {error}");
    }
}
