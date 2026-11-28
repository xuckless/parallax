//! A GPUI window on Wayland with a native view under it: the "page" (src/page.rs) commits a new shm buffer to its
//! own `wl_surface` about 60 times a second, GPUI shows it through a hole, and a GPUI div covers part of it.
//!
//! It logs, once a second, how many frames GPUI drew and how many the page committed (GPUI's must be 0 while only
//! the page changes), the surface alpha mode GPUI chose for the hole, and every pointer event over the view with its
//! position relative to the view. See docs/native-view-wayland.md.
//!
//! Arguments: `--fps N` sets the page's frame rate; `--animate` moves and resizes the view every GPUI frame, to
//! check that the hole and the subsurface stay in step; `--clipped` puts the view in a scrolled container that hides
//! its top half, in a see-through window, to check that only the visible half shows; `--resize` makes the page switch
//! its buffer size every 45 frames, each switch trailed by two frames of the old size (with `--clipped --animate`, the
//! race between GPUI's crop and the page's commits, which must never cost the connection);
//! `--seconds N` quits after N seconds; `--no-view` shows the same window without the view, to compare the
//! compositor's cost.

#[cfg(target_os = "linux")]
mod page;

#[cfg(target_os = "linux")]
fn main() -> Result<(), parallax::parallax_platform::Error> {
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or("warn,native_view_wayland=info,gpui_wgpu::wgpu_renderer=info"),
    )
    .init();
    let options = demo::Options::from_args();
    parallax::run(move |cx| demo::open(options, cx))
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("native-view-wayland runs on Linux under Wayland only");
}

#[cfg(target_os = "linux")]
mod demo {
    use crate::page::{self, Page};
    use gpui_linux::NativeViewBuffers;
    use parallax::gpui::{
        App, Bounds, Context, MouseButton, MouseDownEvent, MouseMoveEvent,
        MouseUpEvent, NativeView, Pixels, Point, ScrollEvent, ScrollHandle, Window,
        WindowBackgroundAppearance, WindowBounds, WindowOptions, canvas, div, point, prelude::*,
        px, rgb, size,
    };
    use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    use std::{
        cell::Cell,
        rc::Rc,
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };

    /// The view's container, shown wherever the view hasn't drawn: what the checks look for as a gap.
    const GAP_COLOUR: u32 = 0xc000c0;

    pub struct Options {
        fps: u32,
        animate: bool,
        clipped: bool,
        resize: bool,
        seconds: Option<u64>,
        no_view: bool,
    }

    impl Options {
        pub fn from_args() -> Self {
            let args: Vec<String> = std::env::args().collect();
            let value = |flag: &str| {
                args.iter()
                    .position(|arg| arg == flag)
                    .and_then(|index| args.get(index + 1))
            };
            Self {
                fps: value("--fps")
                    .and_then(|fps| fps.parse().ok())
                    .unwrap_or(60),
                animate: args.iter().any(|arg| arg == "--animate"),
                clipped: args.iter().any(|arg| arg == "--clipped"),
                resize: args.iter().any(|arg| arg == "--resize"),
                no_view: args.iter().any(|arg| arg == "--no-view"),
                seconds: value("--seconds").and_then(|seconds| seconds.parse().ok()),
            }
        }
    }

    pub fn open(options: Options, cx: &mut App) {
        let bounds = Bounds::centered(None, size(px(900.), px(600.)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // `--clipped`: see-through around the container, so a view showing past its clip would show.
                window_background: if options.clipped {
                    WindowBackgroundAppearance::Transparent
                } else {
                    WindowBackgroundAppearance::Opaque
                },
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Demo::new(options, window, cx)),
        );
        if let Err(error) = opened {
            log::error!("cannot open the window: {error}");
            cx.quit();
        }
    }

    struct Demo {
        /// `None` when the window isn't on Wayland or the page couldn't start.
        view: Option<(NativeView, Page)>,
        /// Frames GPUI painted, counted by a canvas in the root.
        gpui_frames: Rc<Cell<u64>>,
        animate: bool,
        /// `--clipped`: the scrolled container that hides the view's top half.
        scroll: Option<ScrollHandle>,
        started: Instant,
    }

    impl Demo {
        fn new(options: Options, window: &mut Window, cx: &mut Context<Self>) -> Self {
            let attached = if options.no_view {
                Err("--no-view".to_string())
            } else {
                Self::attach(options.fps, options.resize, window, cx)
            };
            let view = match attached {
                Ok(view) => Some(view),
                Err(error) => {
                    log::error!("no native view: {error}");
                    None
                }
            };
            let gpui_frames = Rc::new(Cell::new(0));
            let page_frames = view.as_ref().map(|(_, page)| page.frames.clone());
            let frames = gpui_frames.clone();
            let seconds = options.seconds;
            // Logs once a second without notifying, so the log itself draws no frame.
            cx.spawn(async move |this, cx| {
                let (mut last_gpui, mut last_page) = (0, 0);
                for second in 1u64.. {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let page = page_frames
                        .as_ref()
                        .map_or(0, |frames| frames.load(Ordering::Relaxed));
                    log::info!(
                        "gpui frames/s: {}, page frames/s: {}",
                        frames.get() - last_gpui,
                        page - last_page
                    );
                    (last_gpui, last_page) = (frames.get(), page);
                    if seconds.is_some_and(|seconds| second >= seconds) {
                        // Dropping the page stops its thread before GPUI closes the connection it uses.
                        this.update(cx, |this, _| this.view = None).ok();
                        cx.update(|cx| cx.quit());
                        return;
                    }
                }
            })
            .detach();
            // Scrolled by half the view: its top half is clipped away.
            let scroll = options.clipped.then(|| {
                let scroll = ScrollHandle::new();
                scroll.set_offset(point(px(0.), px(-200.)));
                scroll
            });
            Self {
                view,
                gpui_frames,
                animate: options.animate,
                scroll,
                started: Instant::now(),
            }
        }

        fn attach(
            fps: u32,
            resize: bool,
            window: &mut Window,
            cx: &mut App,
        ) -> Result<(NativeView, Page), String> {
            let display = window.display_handle().map_err(|error| error.to_string())?;
            let RawDisplayHandle::Wayland(display) = display.as_raw() else {
                return Err("the window is not on Wayland".into());
            };
            // SAFETY: the display is GPUI's own connection, alive as long as the app.
            let mut page = unsafe { Page::new(display.display, fps, resize) }?;
            let proxy = page.proxy().ok_or("the page surface has no proxy")?;
            // SAFETY: a live, role-less wl_surface made on the window's own display, kept alive by `page`, which
            // is dropped after the view (field order of the tuple).
            let view =
                unsafe { NativeView::new(proxy, window, cx) }.map_err(|error| error.to_string())?;
            // Shown where the page hasn't drawn yet: the container's colour, which the checks call the gap.
            view.set_background(rgb(GAP_COLOUR), cx);
            let buffers = NativeViewBuffers::of(&view).ok_or("the view is not a Wayland one")?;
            page.start(buffers)?;
            Ok((view, page))
        }

        fn log_pointer(&self, kind: &str, position: Point<Pixels>, window: &Window) {
            let Some((view, _)) = &self.view else {
                return;
            };
            if let Some(bounds) = view.bounds(window) {
                let relative = position - bounds.origin;
                log::info!(
                    "view pointer {kind} at ({:.1}, {:.1})",
                    f32::from(relative.x),
                    f32::from(relative.y)
                );
            }
        }
    }

    impl Render for Demo {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            // Where the view's container is.
            let (left, width) = if self.animate {
                window.request_animation_frame();
                let t = self.started.elapsed().as_secs_f32();
                (
                    60. + 120. * (t * 1.3).sin().abs(),
                    520. + 120. * (t * 0.9).sin(),
                )
            } else {
                (110., 640.)
            };
            let frames = self.gpui_frames.clone();
            // The view's container: where the view is, and what shows where the view hasn't drawn.
            let container = div()
                .flex_none()
                .w(px(width))
                .h(px(400.))
                .bg(rgb(GAP_COLOUR))
                .when_some(self.view.as_ref(), |this, (view, _)| {
                    this.child(
                        view.clone()
                            .into_element()
                            .on_mouse_move(cx.listener(
                                |this, event: &MouseMoveEvent, window, _| {
                                    this.log_pointer("move", event.position, window)
                                },
                            ))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseDownEvent, window, _| {
                                    this.log_pointer("down", event.position, window)
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, event: &MouseUpEvent, window, _| {
                                    this.log_pointer("up", event.position, window)
                                }),
                            )
                            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, _| {
                                this.log_pointer("scroll", event.position, window)
                            })),
                    )
                })
                // Painted after the view, so it covers it and takes its own input.
                .child(
                    div()
                        .id("over")
                        .absolute()
                        .top(px(24.))
                        .left(px(24.))
                        .w(px(200.))
                        .h(px(90.))
                        .bg(rgb(0x2050d0))
                        .text_color(rgb(0xffffff))
                        .p_2()
                        .occlude()
                        .on_mouse_down(MouseButton::Left, |_, _, _| {
                            log::info!("GPUI div over the view: down")
                        })
                        .child("a GPUI div over the view"),
                );
            // With `--clipped`, inside a scrolled container of half its height.
            let container = match &self.scroll {
                Some(scroll) => div()
                    .id("scroller")
                    .absolute()
                    .top(px(80.))
                    .left(px(left))
                    .w(px(width))
                    .h(px(200.))
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .child(container)
                    .into_any_element(),
                None => container
                    .absolute()
                    .top(px(80.))
                    .left(px(left))
                    .into_any_element(),
            };
            div()
                .size_full()
                .when(self.scroll.is_none(), |this| this.bg(rgb(0x202020)))
                .child(
                    canvas(|_, _, _| {}, move |_, _, _, _| frames.set(frames.get() + 1)).size_0(),
                )
                .child(container)
                .child(
                    div()
                        .absolute()
                        .top(px(20.))
                        .left(px(20.))
                        .text_color(rgb(0xffffff))
                        .child(format!(
                            "page buffer {}x{}, scaled to the view",
                            page::SIZES[0].0,
                            page::SIZES[0].1
                        )),
                )
        }
    }
}
