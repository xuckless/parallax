//! Wayland (docs/native-view-wayland.md): the "page" (wayland/page.rs) owns a `wl_surface` on GPUI's own connection
//! and commits a new shm buffer to it about `--fps` times a second from its own thread, through the view's
//! `NativeViewBuffers`. Options: `--fps N`, `--resize` (the page switches its buffer size every 45 frames, each switch
//! trailed by two frames of the old size), and the shared ones (`--animate`, `--clipped`, `--no-view`, `--seconds`).

mod page;

use std::sync::atomic::Ordering;

use gpui_linux::NativeViewBuffers;
use parallax::gpui::{Context, NativeView, Window};
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

use crate::demo::{
    Cover, Demo, Edge, Layout, Look, Made, Motion, Options, Place, Shape, Shown, Target, Term,
};
use page::Page;

/// The container's colour is the gap colour too: what the checks look for where the page hasn't drawn.
const GAP: u32 = 0xc000c0;

pub const TARGET: Target = Target {
    look: Look {
        window: Some([900., 600.]),
        background: 0x202020,
        text: 0xffffff,
        underlay: GAP,
        fill: Some(GAP),
        cover: Cover {
            edge: Edge::Left,
            at: [24., 24.],
            size: [200., 90.],
            colour: 0x2050d0,
        },
        layout: Layout::Fixed(&[]),
        places: &[Place {
            name: "view",
            half: 0,
            rect: [110., 80., 640., 400.],
            cover: true,
            moves: true,
        }],
        buttons: 0x4060a0,
        // Left 60..180, width 400..640: the hole and the subsurface must stay in step at every size.
        motion: Motion::Wave([
            Term::new(-50., 120., 1.3, Shape::AbsSin),
            Term::NONE,
            Term::new(-120., 120., 0.9, Shape::Sin),
            Term::NONE,
        ]),
        grow: [0., 0.],
    },
    variants: &[("wayland", make_view)],
    on_gpui_frame: None,
};

fn make_view(
    options: &Options,
    window: &mut Window,
    cx: &mut Context<Demo>,
) -> Result<Made, String> {
    let display = window.display_handle().map_err(|error| error.to_string())?;
    let RawDisplayHandle::Wayland(display) = display.as_raw() else {
        return Err("the window is not on Wayland".into());
    };
    // SAFETY: the display is GPUI's own connection, alive as long as the app.
    let mut page = unsafe { Page::new(display.display, options.fps, options.resize) }?;
    let proxy = page.proxy().ok_or("the page surface has no proxy")?;
    // SAFETY: a live, role-less wl_surface made on the window's own display, kept alive by `page`, which `Shown`
    // drops after the view.
    let view = unsafe { NativeView::new(proxy, window, cx) }.map_err(|error| error.to_string())?;
    let buffers = NativeViewBuffers::of(&view).ok_or("the view is not a Wayland one")?;
    page.start(buffers)?;
    let frames = page.frames.clone();
    Ok(Made::new(vec![Shown::new("view", view, page)], move || {
        frames.load(Ordering::Relaxed)
    }))
}
