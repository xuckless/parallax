//! Windows (docs/native-view-windows.md): the app's own composition swap chain, on its own D3D11 device and thread,
//! presented every vsync with a new colour (windows/content.rs), or with `--view webview2` a WebView2 page
//! (windows/webview2.rs). "Move" slides the view back and forth for about ten seconds, "Resize" toggles its size.

mod content;
mod page;
mod webview2;

use std::{rc::Rc, sync::atomic::Ordering};

use parallax::gpui::{Context, NativeView, Window};

use crate::demo::{
    Action, Cover, Demo, Edge, Layout, Look, Made, Motion, Options, Place, Shown, Target,
};
use content::Content;

pub const TARGET: Target = Target {
    look: Look {
        window: Some([640., 480.]),
        background: 0x303030,
        text: 0xffffff,
        // Magenta, so a gap is easy to find in a screenshot.
        underlay: 0xff00ff,
        fill: None,
        cover: Cover {
            edge: Edge::Left,
            at: [0., 0.],
            size: [100., 60.],
            colour: 0xff8000,
        },
        layout: Layout::Fixed(&[
            (Action::Move, [500., 360., 100., 40.]),
            (Action::Resize, [500., 420., 100., 40.]),
        ]),
        // Off whole device pixels at 1.25 on purpose (41 × 1.25 = 51.25), so the checks see GPUI's snapping and
        // DirectComposition's placement agree there too.
        places: &[Place {
            name: "view",
            half: 0,
            rect: [41., 41., 322., 242.],
            cover: true,
            moves: true,
        }],
        buttons: 0x4060a0,
        // Long enough for the checks' 100 screenshots; an even number of legs, so it ends where it started.
        motion: Motion::Slide {
            step: 3.,
            leg: 40,
            frames: 16 * 40,
        },
        grow: [80., 60.],
    },
    variants: &[("swap-chain", make_view), ("webview2", webview2::make_view)],
    on_gpui_frame: None,
};

fn make_view(_: &Options, window: &mut Window, cx: &mut Context<Demo>) -> Result<Made, String> {
    // Any size: the first frame's layout resizes it (`Shown::place`).
    let (content, swap_chain) = Content::start(1, 1).map_err(|error| format!("{error:#}"))?;
    // SAFETY: `swap_chain` is a live composition swap chain; GPUI takes a reference of its own.
    let view = unsafe { NativeView::new(swap_chain.as_ptr(), window, cx) }
        .map_err(|error| format!("{error:#}"))?;
    let presents = content.presents.clone();
    let mut shown = Shown::new("view", view, ());
    shown.place = Some(Rc::new(move |[left, top, right, bottom]: [i32; 4]| {
        content.resize((right - left) as u32, (bottom - top) as u32)
    }));
    Ok(Made::new(vec![shown], move || {
        presents.load(Ordering::Relaxed)
    }))
}
