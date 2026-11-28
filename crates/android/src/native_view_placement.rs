//! Where a native view is on GPUI's Android surface, and what Java is told when a frame changes it. Pure, so it is
//! tested on any host; `native_view.rs` holds one per attached view (docs/native-view-android.md).

use gpui::{Bounds, Hsla, Rgba, ScaledPixels, point, size};

/// The colour under a scene's views, as an Android colour int (ARGB), from their backgrounds in paint order: the last
/// view's that has one, opaque, as macOS paints its window (`NativeView::set_background`); transparent when none
/// asks, so the Activity's own window background shows.
pub(crate) fn background(painted: impl DoubleEndedIterator<Item = Hsla>) -> i32 {
    let Some(Rgba { r, g, b, .. }) = painted
        .rev()
        .map(Rgba::from)
        .find(|background| background.a > 0.)
    else {
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
        /// Paint order among the frame's `n` views: -n (painted first, bottom) to -1 (last, top). Java stacks
        /// SurfaceControls by it (`setLayer`, under GPUI's buffer at 0) and Views by Z.
        layer: i32,
    },
}

impl Placement {
    /// Where a scene shows the view: its snapped bounds (whole device pixels; rounded in case) at paint position
    /// `rank` of `count` views, or hidden when the scene doesn't paint it.
    pub(crate) fn of(shown_at: Option<(usize, Bounds<ScaledPixels>)>, count: usize) -> Self {
        let Some((rank, bounds)) = shown_at else {
            return Placement::Hidden;
        };
        let pixel = |value: ScaledPixels| value.0.round() as i32;
        let (left, top) = (pixel(bounds.origin.x), pixel(bounds.origin.y));
        let (right, bottom) = (
            pixel(bounds.origin.x + bounds.size.width),
            pixel(bounds.origin.y + bounds.size.height),
        );
        Placement::Shown {
            bounds: Bounds {
                origin: point(left, top),
                size: size(right - left, bottom - top),
            },
            layer: rank as i32 - count as i32,
        }
    }

    /// `NativeViews.place`'s record for view `handle` (`NativeViews.RECORD` ints): {handle, shown, x, y, width,
    /// height, layer, clip left, top, right, bottom}, the clip in the view's own pixels.
    pub(crate) fn record(self, handle: i32) -> [i32; 11] {
        match self {
            Placement::Hidden => [handle, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            Placement::Shown { bounds, layer } => [
                handle,
                1,
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
                layer,
                0,
                0,
                bounds.size.width,
                bounds.size.height,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Placement, background};
    use gpui::{Bounds, Hsla, ScaledPixels, point, rgb, rgba, size};

    /// One row per case of `NativeView::set_background`'s rule: the last view painted with a colour wins.
    #[test]
    fn background_is_the_last_painted_colour() {
        let view = |colour: u32| -> Hsla { rgb(colour).into() };
        let none = Hsla::transparent_black();
        let rows: [(&str, Vec<Hsla>, i32); 5] = [
            ("no views: transparent", vec![], 0),
            ("no colour asked: transparent", vec![none], 0),
            ("one colour", vec![view(0xff00ff)], 0xffff00ff_u32 as i32),
            (
                "the last painted wins",
                vec![view(0x00ff00), view(0x0000ff)],
                0xff0000ff_u32 as i32,
            ),
            (
                "translucent counts, drawn opaque; a later view without one does not",
                vec![rgba(0x10203080).into(), none],
                0xff102030_u32 as i32,
            ),
        ];
        for (name, views, expected) in rows {
            assert_eq!(background(views.into_iter()), expected, "{name}");
        }
    }

    fn scaled(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
        Bounds {
            origin: point(ScaledPixels(x), ScaledPixels(y)),
            size: size(ScaledPixels(width), ScaledPixels(height)),
        }
    }

    /// One row per kind of frame: what Java is told for view 7, or nothing when the placement stands. `shown_at` is
    /// (paint rank, bounds) among `count` views.
    #[test]
    fn placements_reach_java_only_when_they_change() {
        let shown = |x, y, width, height, layer| Placement::Shown {
            bounds: Bounds {
                origin: point(x, y),
                size: size(width, height),
            },
            layer,
        };
        type Row = (
            &'static str,
            Placement,
            Option<(usize, Bounds<ScaledPixels>)>,
            usize,
            Option<[i32; 11]>,
        );
        let at = Some((0, scaled(10., 20., 300., 200.)));
        let rows: [Row; 10] = [
            ("hidden stays hidden", Placement::Hidden, None, 0, None),
            (
                "shown first",
                Placement::Hidden,
                at,
                1,
                Some([7, 1, 10, 20, 300, 200, -1, 0, 0, 300, 200]),
            ),
            (
                "same bounds: nothing",
                shown(10, 20, 300, 200, -1),
                at,
                1,
                None,
            ),
            (
                "moved",
                shown(10, 20, 300, 200, -1),
                Some((0, scaled(14., 20., 300., 200.))),
                1,
                Some([7, 1, 14, 20, 300, 200, -1, 0, 0, 300, 200]),
            ),
            (
                "resized",
                shown(10, 20, 300, 200, -1),
                Some((0, scaled(10., 20., 320., 180.))),
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
                "fractional edges round to the pixels the hole covers",
                Placement::Hidden,
                Some((0, scaled(10.4, 19.6, 300.2, 200.))),
                1,
                Some([7, 1, 10, 20, 301, 200, -1, 0, 0, 301, 200]),
            ),
            (
                "painted first of two: the bottom layer",
                Placement::Hidden,
                at,
                2,
                Some([7, 1, 10, 20, 300, 200, -2, 0, 0, 300, 200]),
            ),
            (
                "painted last of two: the top layer, under GPUI's buffer",
                Placement::Hidden,
                Some((1, scaled(10., 20., 300., 200.))),
                2,
                Some([7, 1, 10, 20, 300, 200, -1, 0, 0, 300, 200]),
            ),
            (
                "a view painted before it went: restacked",
                shown(10, 20, 300, 200, -2),
                at,
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
