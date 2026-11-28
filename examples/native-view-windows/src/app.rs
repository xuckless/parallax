//! The window: the native view in an element that listens for the mouse, an orange cover over its corner, two
//! buttons, and a once-a-second log of GPUI frames and view presents.

use std::{cell::Cell, rc::Rc, sync::atomic::Ordering, time::Duration};

use parallax::gpui::{
    App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent, NativeView, Pixels, Point,
    Render, ScrollEvent, Window, WindowBounds, WindowOptions, canvas, div, prelude::*, px, rgb,
    size,
};

use crate::content::Content;

/// Where the view sits in the window, and its two sizes, in logical px. Off whole device pixels at 1.25 on purpose
/// (41 × 1.25 = 51.25), so the checks see GPUI's snapping and DirectComposition's placement agree there too.
const VIEW_ORIGIN: [f32; 2] = [41., 41.];
const VIEW_SIZES: [[f32; 2]; 2] = [[322., 242.], [402., 302.]];
/// The colour shown wherever the view's content doesn't reach. Magenta, so a gap is easy to find in a screenshot.
const GAP_COLOUR: u32 = 0xff00ff;
const COVER_COLOUR: u32 = 0xff8000;
const WINDOW_COLOUR: u32 = 0x303030;
/// How far and how long "move" slides the view: a step per GPUI frame.
const MOVE_STEP: f32 = 3.;
const MOVE_FRAMES: u32 = 120;

struct Example {
    view: NativeView,
    content: Content,
    /// GPUI frames drawn, counted as the window paints.
    frames: Rc<Cell<u64>>,
    offset: f32,
    size_index: usize,
}

pub fn open_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(640.), px(480.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| cx.new(|cx| Example::new(window, cx)));
    if let Err(error) = opened {
        log::error!("cannot open the window: {error}");
        cx.quit();
    }
}

impl Example {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let [width, height] = device_size(VIEW_SIZES[0], window.scale_factor());
        let attached = Content::start(width, height).and_then(|(content, swap_chain)| {
            // SAFETY: `swap_chain` is a live composition swap chain; GPUI takes a reference of its own.
            let view = unsafe { NativeView::new(swap_chain.as_ptr(), window, cx) }?;
            Ok((content, view))
        });
        let (content, view) = attached.unwrap_or_else(|error| {
            log::error!("native view: cannot attach: {error:#}");
            std::process::exit(1)
        });
        view.set_background(rgb(GAP_COLOUR), cx);
        log::info!("native view: attached, content {width}x{height}");
        let frames = Rc::new(Cell::new(0));
        let counted = frames.clone();
        let presents = content.presents.clone();
        cx.spawn(async move |_, cx| {
            let (mut last_frames, mut last_presents) = (0, 0);
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let (frames, presents) = (counted.get(), presents.load(Ordering::Relaxed));
                log::info!(
                    "per second: gpui frames {}, view presents {}",
                    frames - last_frames,
                    presents - last_presents
                );
                (last_frames, last_presents) = (frames, presents);
            }
        })
        .detach();
        Self {
            view,
            content,
            frames,
            offset: 0.,
            size_index: 0,
        }
    }

    /// Slides the view right by a step per frame, then back, logging where it ends.
    fn slide(&mut self, cx: &mut Context<Self>) {
        log::info!("motion: start");
        cx.spawn(async move |this, cx| {
            for frame in 0..MOVE_FRAMES {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let step = if frame < MOVE_FRAMES / 2 {
                    MOVE_STEP
                } else {
                    -MOVE_STEP
                };
                this.update(cx, |this, cx| {
                    this.offset += step;
                    cx.notify();
                })
                .ok();
            }
            log::info!("motion: end");
        })
        .detach();
    }

    fn resize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.size_index = 1 - self.size_index;
        let [width, height] = device_size(VIEW_SIZES[self.size_index], window.scale_factor());
        self.content.resize(width, height);
        log::info!("resize: content {width}x{height}");
        cx.notify();
    }

    /// Logs input at a window point by its position in the view, as the app would forward it to a browser.
    fn log_input(&self, kind: &str, position: Point<Pixels>, window: &Window) {
        let at = self
            .view
            .bounds(window)
            .map_or(position, |bounds| position - bounds.origin);
        log::info!(
            "view input: {kind} at {} {}",
            f32::from(at.x),
            f32::from(at.y)
        );
    }
}

/// The view's edges in device pixels, `[left, top, right, bottom]`, rounded as GPUI snaps them.
fn device_edges([x, y]: [f32; 2], [width, height]: [f32; 2], scale: f32) -> [f32; 4] {
    let edge = |logical: f32| (logical * scale).round();
    [edge(x), edge(y), edge(x + width), edge(y + height)]
}

/// The view's size in device pixels, what its swap chain is made at.
fn device_size(size: [f32; 2], scale: f32) -> [u32; 2] {
    let [left, top, right, bottom] = device_edges(VIEW_ORIGIN, size, scale);
    [(right - left) as u32, (bottom - top) as u32]
}

impl Render for Example {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let [x, y] = VIEW_ORIGIN;
        let [width, height] = VIEW_SIZES[self.size_index];
        let frames = self.frames.clone();
        // Where this frame lays the view out: what the checks expect the composed screen to show.
        let [left, top, right, bottom] =
            device_edges([x + self.offset, y], [width, height], window.scale_factor());
        log::info!("view placement: device {left} {top} {right} {bottom}");
        let button = |id: &'static str, top: f32| {
            div()
                .id(id)
                .absolute()
                .left(px(500.))
                .top(px(top))
                .w(px(100.))
                .h(px(40.))
                .bg(rgb(0x4060a0))
        };
        div()
            .size_full()
            .bg(rgb(WINDOW_COLOUR))
            .child(canvas(move |_, _, _| frames.set(frames.get() + 1), |_, _, _, _| {}).size_full())
            .child(
                div()
                    .id("page")
                    .absolute()
                    .left(px(x + self.offset))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height))
                    // The view's own element hears the mouse over it and keeps it from what is behind (patch 0048).
                    .child(
                        self.view
                            .clone()
                            .into_element()
                            .on_mouse_move(cx.listener(
                                |this, event: &MouseMoveEvent, window, _| {
                                    this.log_input("move", event.position, window)
                                },
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, _| {
                                    this.log_input("down", event.position, window)
                                }),
                            )
                            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, _| {
                                this.log_input("scroll", event.position, window)
                            })),
                    )
                    .child(
                        div()
                            .id("cover")
                            .absolute()
                            .top_0()
                            .left_0()
                            .w(px(100.))
                            .h(px(60.))
                            .bg(rgb(COVER_COLOUR))
                            .occlude()
                            .on_mouse_down(MouseButton::Left, |_, _, _| log::info!("cover: down")),
                    ),
            )
            .child(button("move", 360.).on_click(cx.listener(|this, _, _, cx| this.slide(cx))))
            .child(
                button("resize", 420.)
                    .on_click(cx.listener(|this, _, window, cx| this.resize(window, cx))),
            )
    }
}
