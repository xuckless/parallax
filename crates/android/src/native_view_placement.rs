//! Where a native view is on GPUI's Android surface, and what Java is told when a frame changes it. Pure, so it is
//! tested on any host; `native_view.rs` holds one per attached view and takes the visible part and the background
//! from GPUI's core `native_view_placement` (docs/native-view-android.md).

use gpui::{Bounds, Rgba, ScaledPixels, point, size};

/// The underlay's colour as an Android colour int (ARGB) from the core's `background` (the last shown view's that
/// asks for one), drawn opaque as macOS paints its window; transparent for none, so the Activity's own window
/// background shows.
pub(crate) fn argb(background: Option<Rgba>) -> i32 {
    let Some(Rgba { r, g, b, .. }) = background else {
        return 0;
    };
    let byte = |channel: f32| (channel.clamp(0., 1.) * 255.).round() as u32;
    (0xff00_0000 | byte(r) << 16 | byte(g) << 8 | byte(b)) as i32
}

/// Where Java last placed a view, in GPUI surface pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Placement {
    Hidden,
    Shown {
        bounds: Bounds<i32>,
        /// The part GPUI's content mask lets show, in the view's own pixels.
        clip: Bounds<i32>,
        /// Paint order among the frame's `n` views: -n (painted first, bottom) to -1 (last, top). Java stacks
        /// SurfaceControls by it (`setLayer`, under GPUI's buffer at 0) and Views by Z.
        layer: i32,
    },
}

impl Placement {
    /// Where a scene shows the view: painted at position `rank` of `count` views, at `bounds` with `visible` the part
    /// its content mask leaves (core `visible`, both whole device pixels); hidden when the scene doesn't show it.
    pub(crate) fn of(
        shown_at: Option<(usize, Bounds<ScaledPixels>, Bounds<ScaledPixels>)>,
        count: usize,
    ) -> Self {
        let Some((rank, bounds, visible)) = shown_at else {
            return Placement::Hidden;
        };
        let pixels = |bounds: Bounds<ScaledPixels>| Bounds {
            origin: point(
                bounds.origin.x.0.round() as i32,
                bounds.origin.y.0.round() as i32,
            ),
            size: size(
                bounds.size.width.0.round() as i32,
                bounds.size.height.0.round() as i32,
            ),
        };
        let (bounds, visible) = (pixels(bounds), pixels(visible));
        Placement::Shown {
            bounds,
            clip: Bounds {
                origin: visible.origin - bounds.origin,
                size: visible.size,
            },
            layer: rank as i32 - count as i32,
        }
    }

    /// `NativeViews.place`'s record for view `handle` (`NativeViews.RECORD` ints): {handle, shown, x, y, width,
    /// height, layer, clip left, top, right, bottom}.
    pub(crate) fn record(self, handle: i32) -> [i32; 11] {
        match self {
            Placement::Hidden => [handle, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            Placement::Shown {
                bounds,
                clip,
                layer,
            } => [
                handle,
                1,
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
                layer,
                clip.origin.x,
                clip.origin.y,
                clip.origin.x + clip.size.width,
                clip.origin.y + clip.size.height,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Placement, argb};
    use gpui::{Bounds, Rgba, ScaledPixels, point, rgb, rgba, size};

    /// One row per case of turning the core's background into Java's colour int.
    #[test]
    fn background_becomes_an_opaque_colour_int() {
        let rows: [(&str, Option<Rgba>, i32); 3] = [
            ("none: transparent", None, 0),
            ("a colour", Some(rgb(0xff00ff)), 0xffff00ff_u32 as i32),
            (
                "translucent drawn opaque",
                Some(rgba(0x10203080)),
                0xff102030_u32 as i32,
            ),
        ];
        for (name, background, expected) in rows {
            assert_eq!(argb(background), expected, "{name}");
        }
    }

    fn scaled(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
        Bounds {
            origin: point(ScaledPixels(x), ScaledPixels(y)),
            size: size(ScaledPixels(width), ScaledPixels(height)),
        }
    }

    /// One row per kind of frame: what Java is told for view 7, or nothing when the placement stands. `shown_at` is
    /// (paint rank, bounds, visible part) among `count` views.
    #[test]
    fn placements_reach_java_only_when_they_change() {
        let shown = |x, y, width, height, layer| Placement::Shown {
            bounds: Bounds {
                origin: point(x, y),
                size: size(width, height),
            },
            clip: Bounds {
                origin: point(0, 0),
                size: size(width, height),
            },
            layer,
        };
        type Row = (
            &'static str,
            Placement,
            Option<(usize, Bounds<ScaledPixels>, Bounds<ScaledPixels>)>,
            usize,
            Option<[i32; 11]>,
        );
        let whole = |rank, x, y, width, height| {
            let bounds = scaled(x, y, width, height);
            Some((rank, bounds, bounds))
        };
        let rows: [Row; 10] = [
            ("hidden stays hidden", Placement::Hidden, None, 0, None),
            (
                "shown first",
                Placement::Hidden,
                whole(0, 10., 20., 300., 200.),
                1,
                Some([7, 1, 10, 20, 300, 200, -1, 0, 0, 300, 200]),
            ),
            (
                "same bounds: nothing",
                shown(10, 20, 300, 200, -1),
                whole(0, 10., 20., 300., 200.),
                1,
                None,
            ),
            (
                "moved",
                shown(10, 20, 300, 200, -1),
                whole(0, 14., 20., 300., 200.),
                1,
                Some([7, 1, 14, 20, 300, 200, -1, 0, 0, 300, 200]),
            ),
            (
                "resized",
                shown(10, 20, 300, 200, -1),
                whole(0, 10., 20., 320., 180.),
                1,
                Some([7, 1, 10, 20, 320, 180, -1, 0, 0, 320, 180]),
            ),
            (
                "hidden",
                shown(10, 20, 300, 200, -1),
                None,
                1,
                Some([7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            ),
            (
                "clipped by a scroll container: the clip in the view's own pixels",
                shown(10, 20, 300, 200, -1),
                Some((
                    0,
                    scaled(10., 20., 300., 200.),
                    scaled(10., 70., 300., 150.),
                )),
                1,
                Some([7, 1, 10, 20, 300, 200, -1, 0, 50, 300, 200]),
            ),
            (
                "painted first of two: the bottom layer",
                Placement::Hidden,
                whole(0, 10., 20., 300., 200.),
                2,
                Some([7, 1, 10, 20, 300, 200, -2, 0, 0, 300, 200]),
            ),
            (
                "painted last of two: the top layer, under GPUI's buffer",
                Placement::Hidden,
                whole(1, 10., 20., 300., 200.),
                2,
                Some([7, 1, 10, 20, 300, 200, -1, 0, 0, 300, 200]),
            ),
            (
                "a view painted before it went: restacked",
                shown(10, 20, 300, 200, -2),
                whole(0, 10., 20., 300., 200.),
                1,
                Some([7, 1, 10, 20, 300, 200, -1, 0, 0, 300, 200]),
            ),
        ];
        for (name, current, shown_at, count, expected) in rows {
            let next = Placement::of(shown_at, count);
            let record = (next != current).then(|| next.record(7));
            assert_eq!(record, expected, "{name}");
        }
    }
}
