//! The GPUI side: a window with two native views and a GPUI div over the surface (see lib.rs).

use std::{
    cell::Cell,
    ptr::NonNull,
    rc::Rc,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use parallax::{
    gpui::{
        App, Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, NativeView,
        Pixels, Point, ScrollEvent, Stateful, TouchPhase, Window, WindowOptions, canvas, div,
        prelude::*, px, relative, rgb,
    },
    parallax_android::{self as android, TouchForwarding, jni::objects::GlobalRef},
};

use crate::java::{self, Producer};

/// Where the surface has not caught up with its slot (`NativeView::set_background`): nothing else on screen is this.
const GAP_COLOUR: u32 = 0xff00ff;

pub fn launch(_: &mut App) {}

pub fn open_window(cx: &mut App) {
    let opened = cx.open_window(WindowOptions::default(), |window, cx| {
        cx.new(|cx| Demo::new(window, cx))
    });
    if let Err(error) = opened {
        log::error!("cannot open the window: {error:#}");
    }
}

/// A native view and the app's Java object behind it, kept alive as long as the view.
struct Shown {
    view: NativeView,
    _object: GlobalRef,
}

struct Demo {
    /// `None` when a view could not be made (logged).
    surface: Option<(Shown, Producer)>,
    web: Option<Shown>,
    /// Frames GPUI painted, counted by a canvas in the root.
    gpui_frames: Rc<Cell<u64>>,
    animate: bool,
    started: Instant,
}

impl Demo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let made = android::with_activity(window, |env, activity| {
            let options = java::options(env, activity)?;
            let views = java::views(env, activity, options.surface)?;
            Ok::<_, parallax::parallax_android::jni::errors::Error>((
                options,
                views,
                env.get_java_vm()?,
            ))
        });
        let (options, views, vm) = match made {
            Ok(Ok(made)) => made,
            Ok(Err(error)) => return Self::empty(&error.to_string(), cx),
            Err(error) => return Self::empty(&error.to_string(), cx),
        };
        log::info!(
            "surface: {:?}, animate: {}",
            options.surface,
            options.animate
        );
        let surface = attach(&views.surface_view, window, cx).map(|view| {
            // Shown wherever the surface has not caught up with its slot: nothing else is magenta.
            view.set_background(rgb(GAP_COLOUR), cx);
            let producer = Producer::start(vm, views.surface.clone());
            (
                Shown {
                    view,
                    _object: views.surface_view.clone(),
                },
                producer,
            )
        });
        let web = attach(&views.web_view, window, cx).map(|view| Shown {
            view,
            _object: views.web_view.clone(),
        });
        if web.is_some() {
            let forwarded =
                android::forward_touches(window, views.web_view.as_obj(), TouchForwarding::On);
            if let Err(error) = forwarded {
                log::error!("touches stay GPUI's: {error}");
            }
        }
        let demo = Self {
            surface,
            web,
            gpui_frames: Rc::new(Cell::new(0)),
            animate: options.animate,
            started: Instant::now(),
        };
        demo.log_frames(cx);
        demo
    }

    fn empty(why: &str, cx: &mut Context<Self>) -> Self {
        log::error!("no native views: {why}");
        let demo = Self {
            surface: None,
            web: None,
            gpui_frames: Rc::new(Cell::new(0)),
            animate: false,
            started: Instant::now(),
        };
        demo.log_frames(cx);
        demo
    }

    /// Once a second, without notifying, so the log itself draws no frame.
    fn log_frames(&self, cx: &mut Context<Self>) {
        let gpui = self.gpui_frames.clone();
        let surface = self
            .surface
            .as_ref()
            .map(|(_, producer)| producer.frames.clone());
        cx.spawn(async move |_, cx| {
            let (mut last_gpui, mut last_surface) = (0, 0);
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let now = surface
                    .as_ref()
                    .map_or(0, |frames| frames.load(Ordering::Relaxed));
                log::info!(
                    "gpui frames/s {}, surface frames/s {}",
                    gpui.get() - last_gpui,
                    now - last_surface
                );
                (last_gpui, last_surface) = (gpui.get(), now);
            }
        })
        .detach();
    }

    fn log_pointer(
        view: &NativeView,
        name: &str,
        kind: &str,
        position: Point<Pixels>,
        window: &Window,
    ) {
        if let Some(bounds) = view.bounds(window) {
            let relative = position - bounds.origin;
            log::info!(
                "{name} pointer {kind} at ({:.1}, {:.1})",
                f32::from(relative.x),
                f32::from(relative.y)
            );
        }
    }
}

/// Attaches the app's `View` or `SurfaceControl` to the window.
fn attach(object: &GlobalRef, window: &mut Window, cx: &mut App) -> Option<NativeView> {
    let pointer = NonNull::new(object.as_obj().as_raw().cast())?;
    // SAFETY: a live global reference to a parentless View or a SurfaceControl, kept by `Shown` as long as the view.
    match unsafe { NativeView::new(pointer, window, cx) } {
        Ok(view) => Some(view),
        Err(error) => {
            log::error!("cannot attach a native view: {error:#}");
            None
        }
    }
}

/// `view`'s element with listeners that log pointer input relative to the view, as `name`.
fn logged(view: &NativeView, name: &'static str) -> Stateful<Div> {
    let (down, up, moved, scrolled) = (view.clone(), view.clone(), view.clone(), view.clone());
    view.clone()
        .into_element()
        .id(name)
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, _| {
                Demo::log_pointer(&down, name, "down", event.position, window)
            },
        )
        .on_mouse_up(MouseButton::Left, move |event: &MouseUpEvent, window, _| {
            Demo::log_pointer(&up, name, "up", event.position, window)
        })
        .on_mouse_move(move |event: &MouseMoveEvent, window, _| {
            Demo::log_pointer(&moved, name, "move", event.position, window)
        })
        // A pan's first step only: the momentum after it repeats the position the pan started at.
        .on_scroll(move |event: &ScrollEvent, window, _| {
            if event.touch_phase == TouchPhase::Started {
                Demo::log_pointer(&scrolled, name, "scroll", event.position, window)
            }
        })
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Where the surface's slot is in its half: still, or moving and resizing every frame.
        let (left, top, width, height) = if self.animate {
            window.request_animation_frame();
            let t = self.started.elapsed().as_secs_f32();
            (
                0.05 + 0.15 * (t * 1.3).sin().abs(),
                0.05 + 0.1 * (t * 0.7).sin().abs(),
                0.6 + 0.2 * (t * 0.9).sin(),
                0.7 + 0.15 * (t * 1.1).cos(),
            )
        } else {
            (0.05, 0.05, 0.9, 0.9)
        };
        let frames = self.gpui_frames.clone();
        let insets = window.insets().safe_area;
        let viewport = window.viewport_size();
        let landscape = viewport.width > viewport.height;
        div()
            .size_full()
            .flex()
            .when(landscape, |this| this.flex_row())
            .when(!landscape, |this| this.flex_col())
            .bg(rgb(0x202020))
            .pt(insets.top + px(8.))
            .pb(insets.bottom + px(8.))
            .pl(insets.left)
            .pr(insets.right)
            .child(canvas(|_, _, _| {}, move |_, _, _, _| frames.set(frames.get() + 1)).size_0())
            .child(div().relative().flex_1().m_2().bg(rgb(0x404040)).when_some(
                self.surface.as_ref(),
                |this, (shown, _)| {
                    this.child(
                        div()
                            .absolute()
                            .left(relative(left))
                            .top(relative(top))
                            .w(relative(width))
                            .h(relative(height))
                            .child(logged(&shown.view, "surface"))
                            // Painted after the view: covers part of it and takes its own taps.
                            .child(
                                div()
                                    .id("over")
                                    .absolute()
                                    .top(px(16.))
                                    .left(px(16.))
                                    .w(px(150.))
                                    .h(px(70.))
                                    .bg(rgb(0x2050d0))
                                    .text_color(rgb(0xffffff))
                                    .p_2()
                                    .occlude()
                                    .on_mouse_down(MouseButton::Left, |_, _, _| {
                                        log::info!("GPUI div over the surface: down")
                                    })
                                    .child("a GPUI div over the surface"),
                            ),
                    )
                },
            ))
            .child(
                div()
                    .flex_1()
                    .m_2()
                    .bg(rgb(0x404040))
                    .when_some(self.web.as_ref(), |this, shown| {
                        this.child(logged(&shown.view, "web"))
                    }),
            )
    }
}
