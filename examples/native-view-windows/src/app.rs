//! The window: the native view in an element that listens for the mouse, an orange cover over its corner, two
//! buttons, and a once-a-second log of GPUI frames and view presents.

use std::{cell::Cell, rc::Rc, sync::atomic::Ordering, time::Duration};

use parallax::gpui::{
    App, Bounds, Context, DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    NativeView, Pixels, Point, Render, ScrollEvent, Window, WindowBounds, WindowOptions, canvas,
    div, prelude::*, px, rgb, size,
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
/// How "move" slides the view: a step per GPUI frame, turning back every MOVE_LEG frames, MOVE_FRAMES frames in all
/// (an even number of legs, so it ends where it started). Long enough for the checks' 100 screenshots.
const MOVE_STEP: f32 = 3.;
const MOVE_LEG: u32 = 40;
const MOVE_FRAMES: u32 = 16 * MOVE_LEG;

struct Example {
    view: NativeView,
    content: Content,
    /// GPUI frames drawn, counted as the window paints.
    frames: Rc<Cell<u64>>,
    offset: f32,
    size_index: usize,
    /// The left button pressed on the view, while it is held (`capture`).
    held: bool,
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
        // Any size: the first frame's layout resizes it (`render`'s canvas).
        let attached = Content::start(1, 1).and_then(|(content, swap_chain)| {
            // SAFETY: `swap_chain` is a live composition swap chain; GPUI takes a reference of its own.
            let view = unsafe { NativeView::new(swap_chain.as_ptr(), window, cx) }?;
            Ok((content, view))
        });
        let (content, view) = attached.unwrap_or_else(|error| {
            log::error!("native view: cannot attach: {error:#}");
            std::process::exit(1)
        });
        view.set_background(rgb(GAP_COLOUR), cx);
        log::info!("native view: attached");
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
            held: false,
        }
    }

    /// The held button's release: from `capture` wherever the pointer is, or from the view's element for a quick
    /// click released before `capture` was painted. Whichever hears it first clears `held`.
    fn release(&mut self, event: &MouseUpEvent, window: &Window, cx: &mut Context<Self>) {
        if !self.held {
            return;
        }
        self.log_input("up", event.position, window);
        self.held = false;
        cx.notify();
    }

    /// While the button pressed on the view is held: every move and its release, wherever the pointer is, as a
    /// browser's own window capture would give them (the view's element hears the pointer only over it).
    fn capture(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let this = cx.entity();
        self.held.then(|| {
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moving = this.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            moving.read(cx).log_input("move", event.position, window);
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            this.update(cx, |this, cx| this.release(event, window, cx));
                        }
                    });
                },
            )
            .size_0()
        })
    }

    /// Slides the view right and back by a step per frame, logging when it starts and ends.
    fn slide(&mut self, cx: &mut Context<Self>) {
        log::info!("motion: start");
        cx.spawn(async move |this, cx| {
            for frame in 0..MOVE_FRAMES {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let step = if (frame / MOVE_LEG).is_multiple_of(2) {
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

impl Render for Example {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let [x, y] = VIEW_ORIGIN;
        let [width, height] = VIEW_SIZES[self.size_index];
        let frames = self.frames.clone();
        let content = self.content.clone();
        // Fills the view's element, so its prepaint gets this frame's layout of the view: the app sizes its swap
        // chain from it, and logs it for the checks, in device pixels as GPUI snaps the hole.
        let layout = canvas(
            move |bounds: Bounds<Pixels>, window, _| {
                frames.set(frames.get() + 1);
                let scale = window.scale_factor();
                let edge = |logical: Pixels| (f32::from(logical) * scale).round();
                let [left, top, right, bottom] = [
                    edge(bounds.left()),
                    edge(bounds.top()),
                    edge(bounds.right()),
                    edge(bounds.bottom()),
                ];
                log::info!("view placement: device {left} {top} {right} {bottom}");
                content.resize((right - left) as u32, (bottom - top) as u32);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
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
            .children(self.capture(cx))
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
                                    // Mid-drag, `capture` logs the moves.
                                    if !this.held {
                                        this.log_input("move", event.position, window)
                                    }
                                },
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                    this.log_input("down", event.position, window);
                                    this.held = true;
                                    cx.notify();
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                                    this.release(event, window, cx)
                                }),
                            )
                            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, _| {
                                this.log_input("scroll", event.position, window)
                            })),
                    )
                    .child(layout)
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
                button("resize", 420.).on_click(cx.listener(|this, _, _, cx| {
                    this.size_index = 1 - this.size_index;
                    log::info!("resize: to {:?}", VIEW_SIZES[this.size_index]);
                    cx.notify();
                })),
            )
    }
}
