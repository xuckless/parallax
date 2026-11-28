//! The GPUI side every target shares: the slots the native views show in, each with the gap fill under it and a
//! GPUI div over part of it, the buttons and options that move, grow, hide and drop them, GPUI's frame counter and
//! the once-a-second log line. What differs per target is data (`Look`) and the target's `make_view` rows (lib.rs).

use std::{any::Any, cell::Cell, rc::Rc, time::Duration};

use parallax::gpui::{
    AnyElement, App, BackGestureEvent, Context, DefiniteLength, DispatchPhase, Div, IntoElement,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, NativeView, Pixels,
    Point, Render, ScrollDelta, ScrollEvent, ScrollHandle, Stateful, Window, canvas, div, point,
    prelude::*, px, relative, rgb,
};
use web_time::Instant;

/// How a target's window looks: every colour, place and motion the checks look for. One `Look` per target.
pub struct Look {
    /// The desktop window's size in logical px; `None` takes the platform's (phones, the browser).
    pub window: Option<[f32; 2]>,
    pub background: u32,
    pub text: u32,
    /// `NativeView::set_background`: what shows where a view has not drawn yet (the checks' gap colour).
    pub underlay: u32,
    /// The slot's own colour under its view, if it paints one (the gap colour, or the web's "hole not cut").
    pub fill: Option<u32>,
    pub cover: Cover,
    pub layout: Layout,
    /// Where each view goes, by the name its `make_view` gives it, in paint order. Add a row per view.
    pub places: &'static [Place],
    pub buttons: u32,
    pub motion: Motion,
    /// What "Resize" adds to a moving slot's width and height.
    pub grow: [f32; 2],
}

/// The GPUI div over part of each view (painted after it, so it covers it and takes its own input).
pub struct Cover {
    pub edge: Edge,
    /// From the slot's `edge` and top, in logical px.
    pub at: [f32; 2],
    pub size: [f32; 2],
    pub colour: u32,
}

#[derive(Clone, Copy)]
pub enum Edge {
    Left,
    Right,
}

/// One slot: `[left, top, width, height]`, logical px or (in `Layout::Halves`) fractions of its half.
pub struct Place {
    pub name: &'static str,
    /// `Layout::Halves`: which half (0 the first).
    pub half: usize,
    pub rect: [f32; 4],
    pub cover: bool,
    /// Whether the motion and "Resize" move it.
    pub moves: bool,
}

pub enum Layout {
    /// Slots and buttons at fixed places in the window: `(action, [left, top, width, height])` per button.
    Fixed(&'static [(Action, [f32; 4])]),
    /// A row of buttons 60 px down, the target's extra elements, then the slots in what is left.
    Bar(&'static [Action]),
    /// The window cut in two along its long side, each half this colour.
    Halves(u32),
}

/// How the moving slots move.
pub enum Motion {
    /// Every GPUI frame, `[left, top, width, height]` each plus a term of the time.
    Wave([Term; 4]),
    /// `step` px right per frame, turning back every `leg` frames, `frames` in all (an even number of legs).
    Slide { step: f32, leg: u32, frames: u32 },
}

/// `offset + amp * shape(rate * t)`.
pub struct Term {
    pub offset: f32,
    pub amp: f32,
    pub rate: f32,
    pub shape: Shape,
}

pub enum Shape {
    Sin,
    AbsSin,
    Cos,
}

impl Term {
    pub const NONE: Term = Term::new(0., 0., 0., Shape::Sin);

    pub const fn new(offset: f32, amp: f32, rate: f32, shape: Shape) -> Self {
        Self {
            offset,
            amp,
            rate,
            shape,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    Move,
    Resize,
    Hide,
    Drop,
}

/// Action → its element id and label for (state off, state on). Add a row per action.
const ACTIONS: [(Action, &str, [&str; 2]); 4] = [
    (Action::Move, "move", ["Move", "Stop"]),
    (Action::Resize, "resize", ["Resize", "Resize"]),
    (Action::Hide, "show", ["Hide", "Show"]),
    (Action::Drop, "drop", ["Drop views", "Drop views"]),
];
/// "Drop views", found on screen by its colour (scripts/locate-colour.py).
const DROP_COLOUR: u32 = 0xffc857;
/// The buttons a view's element passes on to a view that takes input from the app.
const BUTTONS: [MouseButton; 3] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];

/// The options every target reads (desktop: `--name [value]`; Android: intent extras; the web: the page's query).
pub struct Options {
    /// Which of the target's `make_view` rows (the first by default).
    pub view: Option<String>,
    /// Start moving and resizing the moving slots every frame.
    pub animate: bool,
    /// The moving slot in a container scrolled by half its height, in a see-through window.
    pub clipped: bool,
    /// Wayland: the page switches its buffer size every 45 frames.
    pub resize: bool,
    /// Android: two overlapping tiles in place of the WebView.
    pub stack: bool,
    /// The same window without any view, a cost baseline.
    pub no_view: bool,
    /// Wayland: the page's frame rate.
    pub fps: u32,
    /// Quit after this many seconds.
    pub seconds: Option<u64>,
}

impl Options {
    /// `value(name)`: the option's value, `Some("")` for a flag given bare, `None` when absent.
    pub fn parse(value: impl Fn(&str) -> Option<String>) -> Self {
        let flag = |name| value(name).is_some_and(|value| value != "false");
        Self {
            view: value("view"),
            animate: flag("animate"),
            clipped: flag("clipped"),
            resize: flag("resize"),
            stack: flag("stack"),
            no_view: flag("no-view"),
            fps: value("fps").and_then(|fps| fps.parse().ok()).unwrap_or(60),
            seconds: value("seconds").and_then(|seconds| seconds.parse().ok()),
        }
    }
}

/// What a view's element heard, for a view that takes input from the app (WebView2): `at` in logical px from the
/// view's origin.
pub struct Forwarded {
    pub pointer: Pointer,
    pub at: Point<Pixels>,
    pub scale: f32,
    pub modifiers: Modifiers,
    pub held: Option<MouseButton>,
}

#[derive(Clone, Copy)]
pub enum Pointer {
    Down(MouseButton),
    Up(MouseButton),
    Move,
    Scroll(ScrollDelta),
    /// The pointer left the view (not logged).
    Leave,
}

/// A native view and the app's objects behind it.
pub struct Shown {
    pub name: &'static str,
    pub view: NativeView,
    /// The view's producer, its Java object, its page: dropped after the view (field order).
    pub keep: Box<dyn Any>,
    pub forward: Option<Rc<dyn Fn(Forwarded)>>,
    /// Each frame, where the view is: `[left, top, right, bottom]` device px, as GPUI snaps the hole (logged).
    pub place: Option<Rc<dyn Fn([i32; 4])>>,
}

impl Shown {
    pub fn new(name: &'static str, view: NativeView, keep: impl Any) -> Self {
        Self {
            name,
            view,
            keep: Box::new(keep),
            forward: None,
            place: None,
        }
    }
}

/// A target's own elements beside the slots (iOS's text fields, the web's GPUI field); each places itself.
pub trait Extra {
    fn render(&self, window: &mut Window, cx: &mut App) -> AnyElement;
    /// "Drop views": drop the native views it holds.
    fn drop_views(&mut self) {}
}

/// What a target's `make_view` made.
pub struct Made {
    pub views: Vec<Shown>,
    /// The views' own frames so far, for the log line.
    pub view_frames: Box<dyn Fn() -> u64>,
    pub extra: Option<Box<dyn Extra>>,
    /// A left press on the window that no view or div took (WebView2 takes the keyboard back).
    pub on_press_elsewhere: Option<Rc<dyn Fn()>>,
}

impl Made {
    pub fn new(views: Vec<Shown>, view_frames: impl Fn() -> u64 + 'static) -> Self {
        Self {
            views,
            view_frames: Box::new(view_frames),
            extra: None,
            on_press_elsewhere: None,
        }
    }
}

pub type MakeView = fn(&Options, &mut Window, &mut Context<Demo>) -> Result<Made, String>;

/// A target: its look and its `make_view` rows, `(name, make_view)`, picked by the `view` option.
pub struct Target {
    pub look: Look,
    pub variants: &'static [(&'static str, MakeView)],
    /// Each GPUI frame, the count so far, with or without views (the web hands it to the page).
    pub on_gpui_frame: Option<fn(u64)>,
}

/// Where the moving slots are.
#[derive(Clone, Copy)]
enum Moving {
    Still,
    Wave { since: Instant },
    Sliding { offset: f32 },
}

pub struct Demo {
    target: &'static Target,
    made: Made,
    /// Frames GPUI painted, counted by a canvas in the root.
    gpui_frames: Rc<Cell<u64>>,
    moving: Moving,
    grown: bool,
    hidden: bool,
    /// `--clipped`: the scrolled container that hides the moving slot's top half.
    scroll: Option<ScrollHandle>,
    /// The button pressed on a view, while it is held (`capture`).
    held: Option<(&'static str, MouseButton)>,
}

impl Demo {
    pub fn new(
        target: &'static Target,
        options: Options,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let look = &target.look;
        let make = target
            .variants
            .iter()
            .find(|(name, _)| Some(*name) == options.view.as_deref())
            .or(target.variants.first());
        let made = match (options.no_view, make) {
            (true, _) => Err("no-view".to_string()),
            (false, None) => Err("this target has no native view".to_string()),
            (false, Some((name, make))) => {
                log::info!("native-view: {name}");
                make(&options, window, cx)
            }
        };
        let made = made.unwrap_or_else(|error| {
            log::error!("native-view: no native view: {error}");
            Made::new(Vec::new(), || 0)
        });
        for shown in &made.views {
            shown.view.set_background(rgb(look.underlay), cx);
        }
        // Scrolled by half the slot: its top half is clipped away.
        let scroll = options.clipped.then(|| {
            let height = look
                .places
                .iter()
                .find(|place| place.moves)
                .map_or(0., |place| place.rect[3]);
            let scroll = ScrollHandle::new();
            scroll.set_offset(point(px(0.), px(-height / 2.)));
            scroll
        });
        // Once a second, without notifying, so the log itself draws no frame.
        let seconds = options.seconds;
        cx.spawn(async move |this, cx| {
            let (mut last_gpui, mut last_view) = (0, 0);
            for second in 1u64.. {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let Ok((gpui, view)) = this.update(cx, |demo, _| {
                    (demo.gpui_frames.get(), (demo.made.view_frames)())
                }) else {
                    return;
                };
                log::info!(
                    "native-view: gpui frames/s {}, view frames/s {}",
                    gpui - last_gpui,
                    view.saturating_sub(last_view)
                );
                (last_gpui, last_view) = (gpui, view);
                if seconds.is_some_and(|seconds| second >= seconds) {
                    // Dropping the views first stops their producers (Wayland's uses GPUI's connection).
                    this.update(cx, |demo, _| demo.made.views.clear()).ok();
                    cx.update(|cx| cx.quit());
                    return;
                }
            }
        })
        .detach();
        Self {
            target,
            made,
            gpui_frames: Rc::default(),
            moving: if options.animate {
                Moving::Wave {
                    since: Instant::now(),
                }
            } else {
                Moving::Still
            },
            grown: false,
            hidden: false,
            scroll,
            held: None,
        }
    }

    /// The view named `name` is replaced by `shown` (WebView2 after a GPU device loss), or added.
    pub fn replace(&mut self, shown: Shown, cx: &mut Context<Self>) {
        shown
            .view
            .set_background(rgb(self.target.look.underlay), cx);
        self.made.views.retain(|old| old.name != shown.name);
        self.made.views.push(shown);
        self.held = None;
        cx.notify();
    }

    /// Drops the view named `name` (the web's `nativeViewCommand("drop <name>")`).
    pub fn drop_view(&mut self, name: &str, cx: &mut Context<Self>) {
        self.made.views.retain(|shown| shown.name != name);
        cx.notify();
    }

    fn act(&mut self, action: Action, cx: &mut Context<Self>) {
        match action {
            Action::Move => {
                self.moving = match (self.moving, &self.target.look.motion) {
                    (Moving::Still, Motion::Wave(_)) => Moving::Wave {
                        since: Instant::now(),
                    },
                    // Right and back by a step per frame, logging when it starts and ends.
                    (Moving::Still, &Motion::Slide { step, leg, frames }) => {
                        log::info!("native-view: motion: start");
                        cx.spawn(async move |this, cx| {
                            for frame in 0..frames {
                                cx.background_executor()
                                    .timer(Duration::from_millis(16))
                                    .await;
                                let step = if (frame / leg).is_multiple_of(2) {
                                    step
                                } else {
                                    -step
                                };
                                this.update(cx, |this, cx| {
                                    if let Moving::Sliding { offset } = &mut this.moving {
                                        *offset += step;
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                            this.update(cx, |this, _| this.moving = Moving::Still).ok();
                            log::info!("native-view: motion: end");
                        })
                        .detach();
                        Moving::Sliding { offset: 0. }
                    }
                    (Moving::Wave { .. }, _) => Moving::Still,
                    (sliding @ Moving::Sliding { .. }, _) => sliding,
                }
            }
            Action::Resize => {
                self.grown = !self.grown;
                log::info!("native-view: resize, grown {}", self.grown);
            }
            Action::Hide => self.hidden = !self.hidden,
            Action::Drop => {
                self.made.views.clear();
                if let Some(extra) = &mut self.made.extra {
                    extra.drop_views();
                }
                log::info!("native-view: dropped the native views");
            }
        }
    }

    /// Logs pointer input over view `name` relative to the view, and passes it on to a view that takes it.
    fn pointer(
        &self,
        name: &str,
        pointer: Pointer,
        position: Point<Pixels>,
        modifiers: Modifiers,
        window: &Window,
    ) {
        let Some(shown) = self.made.views.iter().find(|shown| shown.name == name) else {
            return;
        };
        let Some(bounds) = shown.view.bounds(window) else {
            return;
        };
        let at = position - bounds.origin;
        let kind = match pointer {
            Pointer::Down(_) => Some("down"),
            Pointer::Up(_) => Some("up"),
            Pointer::Move => Some("move"),
            Pointer::Scroll(_) => Some("scroll"),
            Pointer::Leave => None,
        };
        if let Some(kind) = kind {
            log::info!(
                "native-view: {name} pointer {kind} at ({:.1}, {:.1})",
                f32::from(at.x),
                f32::from(at.y)
            );
        }
        if let Some(forward) = &shown.forward {
            forward(Forwarded {
                pointer,
                at,
                scale: window.scale_factor(),
                modifiers,
                held: self.held.map(|(_, button)| button),
            });
        }
    }

    /// The held button's release: from `capture` wherever the pointer is, or from the view's element for a quick
    /// click released before `capture` was painted. Whichever hears it first clears `held`.
    fn release(&mut self, event: &MouseUpEvent, window: &Window, cx: &mut Context<Self>) {
        let Some((name, button)) = self.held.filter(|(_, button)| *button == event.button) else {
            return;
        };
        self.pointer(
            name,
            Pointer::Up(button),
            event.position,
            event.modifiers,
            window,
        );
        self.held = None;
        let over = self
            .made
            .views
            .iter()
            .find(|shown| shown.name == name)
            .and_then(|shown| shown.view.bounds(window))
            .is_some_and(|bounds| bounds.contains(&event.position));
        if !over {
            self.pointer(
                name,
                Pointer::Leave,
                event.position,
                event.modifiers,
                window,
            );
        }
        cx.notify();
    }

    /// One place: the slot, its view (unless hidden or dropped), and the div over it.
    fn slot(
        &self,
        index: usize,
        place: &Place,
        t: Option<f32>,
        slide: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let look = &self.target.look;
        let [mut left, mut top, mut width, mut height] = place.rect;
        if place.moves {
            left += slide;
            if self.grown {
                width += look.grow[0];
                height += look.grow[1];
            }
            if let (Some(t), Motion::Wave(terms)) = (t, &look.motion) {
                let [dl, dt, dw, dh] = terms.each_ref().map(|term| {
                    let x = term.rate * t;
                    let wave = match term.shape {
                        Shape::Sin => x.sin(),
                        Shape::AbsSin => x.sin().abs(),
                        Shape::Cos => x.cos(),
                    };
                    term.offset + term.amp * wave
                });
                (left, top, width, height) = (left + dl, top + dt, width + dw, height + dh);
            }
        }
        let fractions = matches!(look.layout, Layout::Halves(_));
        let length = |value: f32| -> DefiniteLength {
            if fractions {
                relative(value)
            } else {
                px(value).into()
            }
        };
        let shown = self
            .made
            .views
            .iter()
            .find(|shown| shown.name == place.name)
            .filter(|_| !self.hidden);
        // Fills the view's element, so its prepaint has this frame's layout of the view, in device px as GPUI
        // snaps the hole: the app sizes its content from it, and it is logged for the checks.
        let placement = shown.and_then(|shown| shown.place.clone()).map(|place| {
            canvas(
                move |bounds, window, _| {
                    let scale = window.scale_factor();
                    let edge = |logical: Pixels| (f32::from(logical) * scale).round() as i32;
                    let edges =
                        [bounds.left(), bounds.top(), bounds.right(), bounds.bottom()].map(edge);
                    let [l, t, r, b] = edges;
                    log::info!("native-view: view placement: device {l} {t} {r} {b}");
                    place(edges);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        });
        let name = place.name;
        let cover = &look.cover;
        let over = div()
            .id(("cover", index))
            .absolute()
            .top(px(cover.at[1]))
            .map(|this| match cover.edge {
                Edge::Left => this.left(px(cover.at[0])),
                Edge::Right => this.right(px(cover.at[0])),
            })
            .w(px(cover.size[0]))
            .h(px(cover.size[1]))
            .bg(rgb(cover.colour))
            .occlude()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                log::info!("native-view: div over {name}: down");
                cx.stop_propagation();
            })
            .on_scroll(move |_, _, _| log::info!("native-view: div over {name}: scroll"));
        let slot = div()
            .flex_none()
            .w(length(width))
            .h(length(height))
            .when_some(look.fill, |this, fill| this.bg(rgb(fill)))
            // The view's element: it hears the pointer over the view where the window gets it (`NativeViewInput::Window`).
            .children(shown.map(|shown| {
                let name = shown.name;
                let mut element = shown
                    .view
                    .clone()
                    .into_element()
                    .id(name)
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, window, _| {
                        // Mid-drag, `capture` has the moves.
                        if this.held.is_none() {
                            this.pointer(
                                name,
                                Pointer::Move,
                                event.position,
                                event.modifiers,
                                window,
                            )
                        }
                    }))
                    .on_hover(cx.listener(move |this, hovered: &bool, window, _| {
                        // Mid-drag the view keeps the pointer until the release.
                        if !hovered && this.held.is_none() {
                            this.pointer(
                                name,
                                Pointer::Leave,
                                Point::default(),
                                Modifiers::default(),
                                window,
                            )
                        }
                    }))
                    .on_scroll(cx.listener(move |this, event: &ScrollEvent, window, _| {
                        let pointer = Pointer::Scroll(event.delta);
                        this.pointer(name, pointer, event.position, event.modifiers, window)
                    }));
                for button in BUTTONS {
                    element = element
                        .on_mouse_down(
                            button,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                this.pointer(
                                    name,
                                    Pointer::Down(button),
                                    event.position,
                                    event.modifiers,
                                    window,
                                );
                                this.held = Some((name, button));
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            button,
                            cx.listener(|this, event: &MouseUpEvent, window, cx| {
                                this.release(event, window, cx)
                            }),
                        );
                }
                element
            }))
            .children(placement)
            .when(place.cover, |this| this.child(over));
        // `--clipped`: inside a scrolled container of half its height.
        match (&self.scroll, place.moves) {
            (Some(scroll), true) => div()
                .id("scroller")
                .absolute()
                .left(length(left))
                .top(length(top))
                .w(length(width))
                .h(length(height / 2.))
                .overflow_y_scroll()
                .track_scroll(scroll)
                .child(slot)
                .into_any_element(),
            _ => slot
                .absolute()
                .left(length(left))
                .top(length(top))
                .into_any_element(),
        }
    }

    fn button(&self, action: Action, cx: &mut Context<Self>) -> Stateful<Div> {
        let on = match action {
            Action::Move => !matches!(self.moving, Moving::Still),
            Action::Hide => self.hidden,
            Action::Resize | Action::Drop => false,
        };
        let (id, labels) = ACTIONS
            .iter()
            .find(|(row, ..)| *row == action)
            .map_or(("", ["", ""]), |(_, id, labels)| (*id, *labels));
        let colour = if action == Action::Drop {
            DROP_COLOUR
        } else {
            self.target.look.buttons
        };
        div()
            .id(id)
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(colour))
            .child(labels[usize::from(on)])
            .on_click(cx.listener(move |this, _, _, cx| {
                this.act(action, cx);
                cx.notify();
            }))
    }
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = &self.target.look;
        let (t, slide) = match self.moving {
            Moving::Still => (None, 0.),
            Moving::Wave { since } => {
                window.request_animation_frame();
                (Some(since.elapsed().as_secs_f32()), 0.)
            }
            Moving::Sliding { offset } => (None, offset),
        };
        // The slots of one half (`Layout::Halves`), or of the window.
        let slots = |half: Option<usize>, cx: &mut Context<Self>| -> Vec<AnyElement> {
            let places = look.places.iter().enumerate();
            places
                .filter(|(_, place)| half.is_none_or(|half| place.half == half))
                .map(|(index, place)| self.slot(index, place, t, slide, cx))
                .collect()
        };
        let (frames, on_frame) = (self.gpui_frames.clone(), self.target.on_gpui_frame);
        let counter = canvas(
            |_, _, _| {},
            move |_, _, _, _| {
                frames.set(frames.get() + 1);
                if let Some(on_frame) = &on_frame {
                    on_frame(frames.get());
                }
            },
        )
        .absolute()
        .size_0();
        let elsewhere = self.made.on_press_elsewhere.clone();
        let extra = self
            .made
            .extra
            .as_ref()
            .map(|extra| extra.render(window, cx));
        // While a button pressed on a view is held: every move and its release, wherever the pointer is, as a
        // browser's own window capture would give them (the view's element hears the pointer only over it).
        let capture = self.held.map(|(name, button)| {
            let this = cx.entity();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moving = this.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            let (position, modifiers) = (event.position, event.modifiers);
                            moving.read(cx).pointer(
                                name,
                                Pointer::Move,
                                position,
                                modifiers,
                                window,
                            );
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && event.button == button {
                            this.update(cx, |this, cx| this.release(event, window, cx));
                        }
                    });
                },
            )
            .absolute()
            .size_0()
        });
        let root = div()
            .size_full()
            .text_color(rgb(look.text))
            // `--clipped`: see-through around the container, so a view showing past its clip would show.
            .when(self.scroll.is_none(), |this| this.bg(rgb(look.background)))
            // Claims Back, so Android's Activity stays (scripts/android-native-view-checks.sh waits for this line).
            .on_back_gesture(|event: &BackGestureEvent, _, _| {
                log::info!("native-view: back {:?}", event.phase)
            })
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                if let Some(elsewhere) = &elsewhere {
                    elsewhere();
                }
            })
            .children(capture)
            .child(counter);
        match look.layout {
            Layout::Fixed(buttons) => root
                .children(slots(None, cx))
                .children(buttons.iter().map(|(action, [left, top, width, height])| {
                    self.button(*action, cx)
                        .absolute()
                        .left(px(*left))
                        .top(px(*top))
                        .w(px(*width))
                        .h(px(*height))
                }))
                .children(extra),
            Layout::Bar(buttons) => root
                .flex()
                .flex_col()
                .gap_3()
                .pt(px(60.))
                .px_4()
                .child(div().flex().gap_2().children(buttons.iter().map(|action| {
                    // Wide, so scripts find it as a band of its colour.
                    self.button(*action, cx)
                        .when(*action == Action::Drop, |this| this.flex_1())
                })))
                .children(extra)
                .child(div().flex_1().relative().children(slots(None, cx))),
            Layout::Halves(colour) => {
                let insets = window.insets().safe_area;
                let viewport = window.viewport_size();
                let landscape = viewport.width > viewport.height;
                let halves = [0, 1].map(|half| {
                    let half = slots(Some(half), cx);
                    div()
                        .relative()
                        .flex_1()
                        .m_2()
                        .bg(rgb(colour))
                        .children(half)
                });
                root.flex()
                    .when(landscape, |this| this.flex_row())
                    .when(!landscape, |this| this.flex_col())
                    .pt(insets.top + px(8.))
                    .pb(insets.bottom + px(8.))
                    .pl(insets.left)
                    .pr(insets.right)
                    .children(halves)
                    .children(extra)
            }
        }
    }
}
