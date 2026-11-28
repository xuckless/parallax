//! Android (docs/native-view-android.md): a surface the app fills with a new colour every frame from its own thread,
//! with a GPUI div over part of it, and a WebView showing a local page, scrolled and tapped through forwarded touches
//! (or, `--ez stack true`, two overlapping tiles). Both sit under GPUI's surface and show through its holes.
//!
//! `--es view control|view|hosted` picks the surface: a `SurfaceControl` child of GPUI's surface (the default, API
//! 29+), an ordinary `SurfaceView` (API 28 always), or a `SurfaceView` in a `SurfaceControlViewHost` of its own
//! (`parallax_android::in_surface`, API 34+). The page logs its scrolls and taps to the `chromium` tag.

mod java;

use std::{ptr::NonNull, sync::Arc, sync::atomic::Ordering};

use parallax::{
    gpui::{App, Context, NativeView, Window},
    parallax_android::{self as android, TouchForwarding, jni::objects::GlobalRef},
};

use crate::demo::{
    Cover, Demo, Edge, Layout, Look, Made, Motion, Options, Place, Shape, Shown, Target, Term,
};
use java::{Fill, Producer, Second, SurfaceKind, TILE_COLOURS};

pub const TARGET: Target = Target {
    look: Look {
        window: None,
        background: 0x202020,
        text: 0xffffff,
        // Where the surface has not caught up with its slot: nothing else on screen is this.
        underlay: 0xff00ff,
        fill: None,
        // Inside the surface at its narrowest (0.4 of a 395 dp half: 158 dp), so the box of view and div that the
        // checks measure their edge ring from is the view's. At 16 + 150 dp it stuck out of the narrowest slot by
        // 8 dp, and a one-frame growing strip there counted as a gap (scripts/android-native-view-checks.sh).
        cover: Cover {
            edge: Edge::Left,
            at: [16., 16.],
            size: [120., 70.],
            colour: 0x2050d0,
        },
        layout: Layout::Halves(0x404040),
        // The second half has the WebView, or the tiles: the one attached first (yellow) is painted last.
        places: &[
            Place {
                name: "surface",
                half: 0,
                rect: [0.05, 0.05, 0.9, 0.9],
                cover: true,
                moves: true,
            },
            Place {
                name: "web",
                half: 1,
                rect: [0., 0., 1., 1.],
                cover: false,
                moves: false,
            },
            Place {
                name: "red",
                half: 1,
                rect: [0.05, 0.05, 0.6, 0.6],
                cover: false,
                moves: false,
            },
            Place {
                name: "yellow",
                half: 1,
                rect: [0.35, 0.35, 0.6, 0.6],
                cover: false,
                moves: false,
            },
        ],
        buttons: 0x4060a0,
        // Fractions of the half: left .05–.2, top .05–.15, width .4–.8, height .55–.85.
        motion: Motion::Wave([
            Term::new(0., 0.15, 1.3, Shape::AbsSin),
            Term::new(0., 0.1, 0.7, Shape::AbsSin),
            Term::new(-0.3, 0.2, 0.9, Shape::Sin),
            Term::new(-0.2, 0.15, 1.1, Shape::Cos),
        ]),
        grow: [0., 0.],
    },
    // One row per kind of surface; the first is the default. Add a row per kind.
    variants: &[
        ("control", |options, window, cx| {
            make_view(SurfaceKind::Control, options, window, cx)
        }),
        ("view", |options, window, cx| {
            make_view(SurfaceKind::View, options, window, cx)
        }),
        ("hosted", |options, window, cx| {
            make_view(SurfaceKind::Hosted, options, window, cx)
        }),
    ],
    on_gpui_frame: None,
};

/// The intent's extras as options (`--ez animate true`, `--es view control`): `Bundle.get`'s value as a string.
pub fn options(window: &mut Window) -> Option<Options> {
    let read = android::with_activity(window, java::extras)
        .map_err(|error| error.to_string())
        .and_then(|extras| extras.map_err(|error| error.to_string()));
    let extras = read
        .inspect_err(|error| log::error!("native-view: cannot read the intent: {error}"))
        .ok()?;
    Some(Options::parse(|name| {
        extras
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }))
}

fn make_view(
    kind: SurfaceKind,
    options: &Options,
    window: &mut Window,
    cx: &mut Context<Demo>,
) -> Result<Made, String> {
    let made = android::with_activity(window, |env, activity| {
        let views = java::views(env, activity, kind, options.stack)?;
        Ok::<_, parallax::parallax_android::jni::errors::Error>((
            views,
            Arc::new(env.get_java_vm()?),
        ))
    });
    let (views, vm) = made
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    log::info!(
        "native-view: surface {:?}, stack {}",
        views.kind,
        options.stack
    );
    let view = attach(&views.surface_view, window, cx)?;
    let producer = Producer::start(vm.clone(), views.surface.clone(), Fill::Changing);
    let frames = producer.frames.clone();
    let mut shown = vec![Shown::new("surface", view, (views.surface_view, producer))];
    match views.second {
        Second::Web(web_view) => {
            let view = attach(&web_view, window, cx)?;
            if let Err(error) =
                android::forward_touches(window, web_view.as_obj(), TouchForwarding::On)
            {
                log::error!("native-view: touches stay GPUI's: {error}");
            }
            shown.push(Shown::new("web", view, web_view));
        }
        // In attach order, the yellow one first.
        Second::Tiles(tiles) => {
            for ((object, surface), (name, colour)) in tiles.into_iter().zip(TILE_COLOURS) {
                let view = attach(&object, window, cx)?;
                let producer = surface
                    .map(|surface| Producer::start(vm.clone(), surface, Fill::Solid(colour)));
                shown.push(Shown::new(name, view, (object, producer)));
            }
        }
    }
    Ok(Made::new(shown, move || frames.load(Ordering::Relaxed)))
}

/// Attaches the app's `View` or `SurfaceControl` to the window.
fn attach(object: &GlobalRef, window: &mut Window, cx: &mut App) -> Result<NativeView, String> {
    let pointer = NonNull::new(object.as_obj().as_raw().cast()).ok_or("a null object")?;
    // SAFETY: a live global reference to a parentless View or a SurfaceControl, kept by `Shown` as long as the view.
    unsafe { NativeView::new(pointer, window, cx) }
        .map_err(|error| format!("cannot attach a native view: {error:#}"))
}
