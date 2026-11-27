// Ported from flutter/flutter @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7), under packages/flutter/lib/src/:
// widgets/text_selection.dart (_SelectionHandleOverlay.build's hit area, TextSelectionOverlay._getHandleDy and
// _buildMagnifier), rendering/editable.dart (getEndpointsForSelection, _paintHandleLayers' clamp),
// material/text_selection.dart (getHandleAnchor), widgets/text_selection_toolbar_anchors.dart
// (TextSelectionToolbarAnchors.fromSelection, getSelectionRect), material/text_selection_toolbar.dart (build's
// anchor padding), widgets/text_selection_toolbar_layout_delegate.dart (centerOn, getPositionForChild),
// widgets/magnifier.dart (MagnifierController.shiftWithinBounds) and material/magnifier.dart
// (TextMagnifier._determineMagnifierPositionAndFocalPoint, Magnifier's constants).
// Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.

#![allow(
    non_upper_case_globals,
    reason = "Flutter's constant names, kept so Flutter changes diff in"
)]

//! The overlay's geometry, as plain values: handle rects and hit areas, toolbar anchors and placement, the
//! magnifier's position. Pure functions of the layout; the engine in `mod.rs` calls them.

use std::time::Duration;

use gpui::{Bounds, Pixels, Point, Size, point, px, size};
use parallax_platform::behaviour::kMinInteractiveDimension;

use super::{Selection, TextLayout};

/// Which handle a drag or a rect belongs to: the selection's start or end (a collapsed selection has only `Start`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Start,
    End,
}

/// `TextSelectionHandleType`: the drawn handle's shape. Material's handle is a circle with a square in one quadrant
/// pointing at the text: `Left` points up-right, `Right` up-left, `Collapsed` straight up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleType {
    Left,
    Right,
    Collapsed,
}

/// One handle to draw: `rect` is the drawn handle, `hit` the area that takes a touch (at least
/// `kMinInteractiveDimension` square, centred on it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandleGeometry {
    pub handle: Handle,
    pub kind: HandleType,
    pub rect: Bounds<Pixels>,
    pub hit: Bounds<Pixels>,
}

/// `getEndpointsForSelection`: the bottom of the line at each end, `[start, end]`; a collapsed selection's both
/// ends are its caret's. `_paintHandleLayers` keeps them inside the field.
pub fn selection_endpoints(layout: &impl TextLayout, selection: Selection) -> [Point<Pixels>; 2] {
    let bounds = layout.bounds();
    let endpoint = |offset| {
        let caret = layout.caret_rect(offset);
        point(
            clamp_double(caret.left(), bounds.left(), bounds.right()),
            clamp_double(caret.bottom(), bounds.top(), bounds.bottom()),
        )
    };
    [endpoint(selection.start()), endpoint(selection.end())]
}

/// The handles a selection shows (`TextSelectionOverlay._updateSelectionOverlay`, `SelectionOverlay._build*Handle`):
/// one collapsed handle for a caret, a left and a right one for a range (left-to-right text).
pub fn handles(
    layout: &impl TextLayout,
    selection: Selection,
    handle_size: Pixels,
) -> Vec<HandleGeometry> {
    let [start, end] = selection_endpoints(layout, selection);
    let rows: &[(Handle, HandleType, Point<Pixels>)] = if selection.is_collapsed() {
        &[(Handle::Start, HandleType::Collapsed, start)]
    } else {
        &[
            (Handle::Start, HandleType::Left, start),
            (Handle::End, HandleType::Right, end),
        ]
    };
    rows.iter()
        .map(|&(handle, kind, endpoint)| {
            // Put the handle's anchor point on the endpoint.
            let rect = Bounds::new(
                endpoint - handle_anchor(kind, handle_size),
                size(handle_size, handle_size),
            );
            HandleGeometry {
                handle,
                kind,
                rect,
                hit: interactive_rect(rect),
            }
        })
        .collect()
}

/// Material's `getHandleAnchor`: the point of the handle that sits on the endpoint.
fn handle_anchor(kind: HandleType, handle_size: Pixels) -> Point<Pixels> {
    match kind {
        HandleType::Collapsed => point(handle_size / 2., px(-4.)),
        HandleType::Left => point(handle_size, px(0.)),
        HandleType::Right => point(px(0.), px(0.)),
    }
}

/// `_SelectionHandleOverlay.build`: "Make sure the GestureDetector is big enough to be easily interactive if the
/// handleRect is not empty."
fn interactive_rect(handle_rect: Bounds<Pixels>) -> Bounds<Pixels> {
    if handle_rect.size.width <= px(0.) || handle_rect.size.height <= px(0.) {
        return handle_rect;
    }
    let side = handle_rect
        .size
        .width
        .max(handle_rect.size.height)
        .max(kMinInteractiveDimension);
    Bounds::centered_at(handle_rect.center(), size(side, side))
}

/// `TextSelectionOverlay._getHandleDy`: the handle's y after a drag to `drag_dy`. It jumps a whole line at a time,
/// when the contact point reaches the same place on the handle on the next line. `None` when the layout is
/// degenerate (no line height, or a coordinate that isn't finite): the drag update is skipped.
pub fn handle_dy(
    drag_dy: Pixels,
    handle_dy: Pixels,
    preferred_line_height: Pixels,
) -> Option<Pixels> {
    let (drag, handle, line) = (
        drag_dy.as_f32(),
        handle_dy.as_f32(),
        preferred_line_height.as_f32(),
    );
    if line <= 0.0 || !drag.is_finite() || !handle.is_finite() {
        return None;
    }
    let distance_dragged = drag - handle;
    let drag_direction = if distance_dragged < 0.0 { -1.0 } else { 1.0 };
    let lines_dragged = drag_direction * (distance_dragged.abs() / line).floor();
    Some(px(handle + lines_dragged * line))
}

/// `TextSelectionToolbarAnchors`: where the toolbar tries to sit (`primary`, above the selection) and its fallback
/// (`secondary`, below it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolbarAnchors {
    pub primary: Point<Pixels>,
    pub secondary: Point<Pixels>,
}

/// `TextSelectionToolbarAnchors.fromSelection`. The glyph heights are the line height (Flutter measures the first
/// and last selected grapheme, falling back to it).
pub fn toolbar_anchors(layout: &impl TextLayout, selection: Selection) -> ToolbarAnchors {
    let region = layout.bounds();
    let glyph_height = layout.preferred_line_height();
    let [first, last] = selection_endpoints(layout, selection);
    let is_multiline = last.y - first.y > glyph_height / 2.;
    let (left, right) = if is_multiline {
        (region.left(), region.right())
    } else {
        (first.x, last.x)
    };
    let (top, bottom) = (first.y - glyph_height, last.y);
    let x = left + (right - left) / 2.;
    ToolbarAnchors {
        primary: point(x, clamp_double(top, region.top(), region.bottom())),
        secondary: point(x, clamp_double(bottom, region.top(), region.bottom())),
    }
}

// material/text_selection_toolbar.dart and material/text_selection.dart.
const kToolbarHeight: Pixels = px(44.0);
const kToolbarContentDistance: Pixels = px(8.0);
const kToolbarContentDistanceBelow: Pixels = px(20.0);
// CupertinoTextSelectionToolbar.kToolbarScreenPadding, which Material's toolbar pads the screen with.
const kToolbarScreenPadding: Pixels = px(8.0);

/// Material `TextSelectionToolbar.build` then `TextSelectionToolbarLayoutDelegate.getPositionForChild`: the
/// top-left of a `toolbar` sized toolbar in `screen` (whose top `safe_top` is covered by system bars), above the
/// selection when it fits, else below.
pub fn toolbar_origin(
    anchors: ToolbarAnchors,
    toolbar: Size<Pixels>,
    screen: Bounds<Pixels>,
    safe_top: Pixels,
) -> Point<Pixels> {
    let anchor_above = anchors.primary - point(px(0.), kToolbarContentDistance);
    let anchor_below = anchors.secondary + point(px(0.), kToolbarContentDistanceBelow);
    let padding_above = screen.top() + safe_top + kToolbarScreenPadding;
    let available_height = anchor_above.y - kToolbarContentDistance - padding_above;
    let fits_above = kToolbarHeight <= available_height;
    // The delegate lays out inside the padding.
    let local_adjustment = point(screen.left() + kToolbarScreenPadding, padding_above);
    let max_width = screen.size.width - kToolbarScreenPadding * 2.;
    let anchor = if fits_above {
        anchor_above
    } else {
        anchor_below
    } - local_adjustment;
    let y = if fits_above {
        (anchor.y - toolbar.height).max(px(0.))
    } else {
        anchor.y
    };
    point(center_on(anchor.x, toolbar.width, max_width), y) + local_adjustment
}

/// `TextSelectionToolbarLayoutDelegate.centerOn`: the distance from zero that centres `width` as closely as possible
/// on `position` while fitting between zero and `max`.
fn center_on(position: Pixels, width: Pixels, max: Pixels) -> Pixels {
    if position - width / 2. < px(0.) {
        px(0.)
    } else if position + width / 2. > max {
        max - width
    } else {
        position - width / 2.
    }
}

/// `MagnifierInfo`: what the magnifier positions itself by, in window coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagnifierInfo {
    pub field_bounds: Bounds<Pixels>,
    pub gesture_position: Point<Pixels>,
    pub caret_rect: Bounds<Pixels>,
    pub current_line_boundaries: Bounds<Pixels>,
}

/// `TextSelectionOverlay._buildMagnifier`: the info for a gesture at `gesture_position` over text `offset`.
pub fn magnifier_info(
    layout: &impl TextLayout,
    gesture_position: Point<Pixels>,
    offset: usize,
) -> MagnifierInfo {
    let line = layout.line_at(offset);
    MagnifierInfo {
        field_bounds: layout.bounds(),
        gesture_position,
        caret_rect: layout.caret_rect(offset),
        current_line_boundaries: Bounds::from_corners(
            layout.caret_rect(line.start).top_center(),
            layout.caret_rect(line.end).bottom_center(),
        ),
    }
}

// material/magnifier.dart's Magnifier.
pub const kDefaultMagnifierSize: Size<Pixels> = Size {
    width: px(77.37),
    height: px(37.9),
};
pub const kStandardVerticalFocalPointShift: Pixels = px(22.0);
const _magnification: f32 = 1.25;
/// `TextMagnifier.jumpBetweenLinesAnimationDuration`: how long the loupe animates when it changes lines.
pub const jumpBetweenLinesAnimationDuration: Duration = Duration::from_millis(70);

/// Where the magnifier is drawn: `rect` (`kDefaultMagnifierSize`), the extra focal point offset that keeps the lens
/// on the text (`Magnifier.additionalFocalPointOffset`, added to `kStandardVerticalFocalPointShift` below the
/// centre), and whether it just changed lines, so the app animates the move over
/// `jumpBetweenLinesAnimationDuration`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MagnifierGeometry {
    pub rect: Bounds<Pixels>,
    pub focal_point_offset: Point<Pixels>,
    pub jumped_line: bool,
}

/// `TextMagnifier._determineMagnifierPositionAndFocalPoint` (Android 12's loupe): it tracks the gesture's x within
/// the line, sits above the caret's line, stays on `screen` and never shows past the field. `previous` is the last
/// position (none for the first, which never animates).
pub fn text_magnifier(
    info: MagnifierInfo,
    screen: Bounds<Pixels>,
    previous: Option<MagnifierGeometry>,
) -> MagnifierGeometry {
    // Shifts the magnifier so it is drawn at its centre, and above the touch point.
    let basic_magnifier_offset = point(
        kDefaultMagnifierSize.width / 2.,
        kDefaultMagnifierSize.height + kStandardVerticalFocalPointShift,
    );
    // The magnifier tracks the gesture, but never past the edges of the line.
    let magnifier_x = clamp_double(
        info.gesture_position.x,
        info.current_line_boundaries.left(),
        info.current_line_boundaries.right(),
    );
    let unadjusted = Bounds::new(
        point(magnifier_x, info.caret_rect.center().y) - basic_magnifier_offset,
        kDefaultMagnifierSize,
    );
    let adjusted = shift_within_bounds(unadjusted, screen);
    // The insets from either edge the focal point must not point past, lest the lens show something out of bounds.
    let horizontal_max_focal_point_edge_insets =
        (kDefaultMagnifierSize.width / 2.) / _magnification;
    // A field too narrow to stay in bounds settles for pointing at its centre.
    let new_global_focal_point_x =
        if info.field_bounds.size.width < horizontal_max_focal_point_edge_insets * 2. {
            info.field_bounds.center().x
        } else {
            clamp_double(
                adjusted.center().x,
                info.field_bounds.left() + horizontal_max_focal_point_edge_insets,
                info.field_bounds.right() - horizontal_max_focal_point_edge_insets,
            )
        };
    MagnifierGeometry {
        rect: adjusted,
        focal_point_offset: point(
            new_global_focal_point_x - adjusted.center().x,
            // Pressed against the top of the screen, the focal point follows how far the magnifier moved.
            unadjusted.top() - adjusted.top(),
        ),
        jumped_line: previous.is_some_and(|previous| previous.rect.top() != adjusted.top()),
    }
}

/// `MagnifierController.shiftWithinBounds`: `rect` moved the shortest distance that puts it inside `bounds`.
pub fn shift_within_bounds(rect: Bounds<Pixels>, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let shift = |low: Pixels, high: Pixels, min: Pixels, max: Pixels| {
        if low < min {
            min - low
        } else if high > max {
            max - high
        } else {
            px(0.)
        }
    };
    let dx = shift(rect.left(), rect.right(), bounds.left(), bounds.right());
    let dy = shift(rect.top(), rect.bottom(), bounds.top(), bounds.bottom());
    Bounds::new(rect.origin + point(dx, dy), rect.size)
}

/// Flutter's `clampDouble`; unlike `f32::clamp` it never panics on `min > max` (it takes `max`).
fn clamp_double(x: Pixels, min: Pixels, max: Pixels) -> Pixels {
    x.max(min).min(max)
}
