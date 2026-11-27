//! What Java reports about the display, its density, the device's touch slop and the system bars, and what a new
//! density and touch slop report changes (pure: host.rs acts on it).

use std::time::Duration;

use gpui::{Bounds, DevicePixels, Edges, Pixels, Size, WindowInsets, point, px, size};

/// What Java last reported about the display and system bars, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Metrics {
    pub(crate) size: Size<DevicePixels>,
    pub(crate) density: f32,
    pub(crate) refresh_rate: f32,
    /// `ViewConfiguration.getScaledTouchSlop()`; `None` until Java first reports it.
    pub(crate) touch_slop: Option<DevicePixels>,
    pub(crate) safe_area: Edges<DevicePixels>,
    pub(crate) ime: Edges<DevicePixels>,
}

/// What a density and touch slop report changed ([`Metrics::rescaled`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rescaled {
    pub(crate) metrics: Metrics,
    /// GPUI's scale moved: the window must resize (a density change keeps the surface's pixel size).
    pub(crate) density_moved: bool,
    /// The logical touch slop moved, to this: the app's `TouchSlopChanged` hook hears it.
    pub(crate) touch_slop_moved: Option<Pixels>,
}

impl Metrics {
    /// Before the first surface: nothing drawn yet, so any density works; 1.0 keeps conversions finite.
    pub(crate) const BEFORE_FIRST_SURFACE: Metrics = Metrics {
        size: Size {
            width: DevicePixels(0),
            height: DevicePixels(0),
        },
        density: 1.0,
        refresh_rate: 0.0,
        touch_slop: None,
        safe_area: Edges {
            top: DevicePixels(0),
            right: DevicePixels(0),
            bottom: DevicePixels(0),
            left: DevicePixels(0),
        },
        ime: Edges {
            top: DevicePixels(0),
            right: DevicePixels(0),
            bottom: DevicePixels(0),
            left: DevicePixels(0),
        },
    };

    /// These metrics after Java reports `density` and the device's `touch_slop` (pixels), which it sends together
    /// with every surface and insets report, and what moved.
    pub(crate) fn rescaled(self, density: f32, touch_slop: DevicePixels) -> Rescaled {
        let metrics = Metrics {
            density,
            touch_slop: Some(touch_slop),
            ..self
        };
        let new_slop = metrics.touch_slop();
        Rescaled {
            metrics,
            density_moved: self.density != density,
            touch_slop_moved: new_slop.filter(|_| self.touch_slop() != new_slop),
        }
    }

    fn logical(&self, edges: Edges<DevicePixels>) -> Edges<Pixels> {
        edges.map(|edge| px(edge.0 as f32 / self.density))
    }

    /// The window (and display) bounds in logical pixels.
    pub(crate) fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(0.), px(0.)),
            size(
                px(self.size.width.0 as f32 / self.density),
                px(self.size.height.0 as f32 / self.density),
            ),
        )
    }

    /// The touch slop in logical pixels, as Flutter's `DeviceGestureSettings.fromView`: physical over the density.
    pub(crate) fn touch_slop(&self) -> Option<Pixels> {
        self.touch_slop.map(|slop| px(slop.0 as f32 / self.density))
    }

    pub(crate) fn refresh_interval(&self) -> Option<Duration> {
        (self.refresh_rate > 0.0).then(|| Duration::from_secs_f32(1.0 / self.refresh_rate))
    }

    pub(crate) fn insets(&self) -> WindowInsets {
        WindowInsets {
            safe_area: self.logical(self.safe_area),
            ime: self.logical(self.ime),
            // Not yet on Android: rounded display corners (`RoundedCorner`, API 31).
            corner_radii: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescaled_table() {
        let at = |density: f32, touch_slop: i32| Metrics {
            density,
            touch_slop: Some(DevicePixels(touch_slop)),
            ..Metrics::BEFORE_FIRST_SURFACE
        };
        // (row name, before, reported density and slop in pixels, density moved, logical slop moved to)
        let rows = [
            (
                "first report: the slop arrives",
                Metrics::BEFORE_FIRST_SURFACE,
                (2.625, 21),
                true,
                Some(8.),
            ),
            (
                "same density and slop: nothing moves",
                at(2.625, 21),
                (2.625, 21),
                false,
                None,
            ),
            (
                "density change, Android scales the slop with it: only the density",
                at(2.625, 21),
                (3.5, 28),
                true,
                None,
            ),
            (
                "density change that moves the logical slop (wm density 450): both",
                at(2.625, 21),
                (2.8125, 23),
                true,
                Some(23. / 2.8125),
            ),
            (
                "slop change at the same density: only the slop",
                at(2.625, 21),
                (2.625, 42),
                false,
                Some(16.),
            ),
        ];
        for (name, before, (density, touch_slop), density_moved, touch_slop_moved) in rows {
            let rescaled = before.rescaled(density, DevicePixels(touch_slop));
            assert_eq!(
                rescaled,
                Rescaled {
                    metrics: Metrics {
                        density,
                        touch_slop: Some(DevicePixels(touch_slop)),
                        ..before
                    },
                    density_moved,
                    touch_slop_moved: touch_slop_moved.map(px),
                },
                "{name}"
            );
        }
    }
}
