// Ported from flutter/flutter @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7), under packages/flutter/lib/src/:
// widgets/text_selection.dart (TextSelectionGestureDetectorBuilder, _TextSelectionGestureDetectorState,
// TextSelectionOverlay's handle drags, SelectionOverlay.showMagnifier and selectionEndpoints' haptic),
// rendering/editable.dart (selectPositionAt, selectWordsInRange, getWordAtOffset), widgets/editable_text.dart
// (showToolbar, hideToolbar, toggleToolbar, _handleSelectionChanged's keyboard request) and material/text_field.dart
// (_shouldShowSelectionHandles, _handleSelectionChanged's desktop toolbar rule).
// Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.

//! Flutter's text selection behaviour for Android and touch desktops, as one state machine: pointer input in, the
//! new selection and the overlay (handles, toolbar, magnifier) out. The app supplies the text layout through
//! [`TextLayout`] and draws the overlay itself. iOS uses UIKit's own selection instead
//! ([`TextSelection::for_behaviour`] returns `None` there). How it works: docs/text-selection.md.

mod geometry;
#[cfg(test)]
mod tests;

use std::ops::Range;

use gpui::{Bounds, Haptic, Pixels, Point, PointerDevice, point, slops};
use parallax_platform::behaviour::{
    Behaviour, DragRecognizer, HandleDrag, LongPressSelects, MagnifierKind, SecondaryTap,
    SelectionRules, ShiftSelect, TapCount, TapEdge, TextBoundaryKind, TextSelectionControls,
    TouchDragSelects,
};

pub use geometry::{
    Handle, HandleGeometry, HandleType, MagnifierGeometry, MagnifierInfo, ToolbarAnchors,
    handle_dy, jumpBetweenLinesAnimationDuration, kDefaultMagnifierSize,
    kStandardVerticalFocalPointShift, magnifier_info, selection_endpoints, shift_within_bounds,
    text_magnifier, toolbar_anchors, toolbar_origin,
};

/// Flutter's `TextSelection`: `base` stays put, `extent` moves; offsets are the app's (any unit its layout uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub base: usize,
    pub extent: usize,
}

impl Selection {
    pub fn collapsed(offset: usize) -> Self {
        Self {
            base: offset,
            extent: offset,
        }
    }

    pub fn start(&self) -> usize {
        self.base.min(self.extent)
    }

    pub fn end(&self) -> usize {
        self.base.max(self.extent)
    }

    pub fn is_collapsed(&self) -> bool {
        self.base == self.extent
    }
}

/// The app's text layout, in window coordinates. Flutter's `RenderEditable` queries, one method each.
pub trait TextLayout {
    /// The text's length, in the unit of the offsets.
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// `getPositionForPoint`: the caret offset closest to `point`.
    fn position_for_point(&self, point: Point<Pixels>) -> usize;
    /// `getWordBoundary`: the word (or run of spaces) around `offset`.
    fn word_boundary(&self, offset: usize) -> Range<usize>;
    /// `getLineAtOffset`: the line holding `offset`.
    fn line_at(&self, offset: usize) -> Range<usize>;
    /// `ParagraphBoundary`: the paragraph (up to a line break) holding `offset`.
    fn paragraph_at(&self, offset: usize) -> Range<usize>;
    /// `getLocalRectForCaret`: the caret at `offset`, a line high.
    fn caret_rect(&self, offset: usize) -> Bounds<Pixels>;
    /// `preferredLineHeight`.
    fn preferred_line_height(&self) -> Pixels;
    /// The field's text area (`paintBounds`).
    fn bounds(&self) -> Bounds<Pixels>;
}

/// The field as the engine sees it on each call: what the app owns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Field {
    pub selection: Selection,
    pub focused: bool,
    pub lines: Lines,
    /// The window's area the magnifier must stay in.
    pub screen: Bounds<Pixels>,
}

/// `maxLines == 1` or not: a single-line field selects all on a triple tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lines {
    Single,
    Multi,
}

/// Pointer input over the field, in window coordinates. `tap_count` is GPUI's click count (Flutter's
/// `consecutiveTapCount`). A touch tap arrives as `Down` then `Up` (GPUI recognizes it on release); a long press
/// as `LongPress*` (GPUI only long-presses touches, as Flutter's recognizer only takes them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    Down {
        position: Point<Pixels>,
        device: PointerDevice,
        tap_count: usize,
        shift: bool,
    },
    Move {
        position: Point<Pixels>,
    },
    Up,
    SecondaryTap {
        position: Point<Pixels>,
        device: PointerDevice,
    },
    LongPressStart {
        position: Point<Pixels>,
    },
    LongPressMove {
        position: Point<Pixels>,
    },
    LongPressEnd,
    LongPressCancel,
    HandleDragStart {
        handle: Handle,
        position: Point<Pixels>,
    },
    HandleDragUpdate {
        position: Point<Pixels>,
    },
    HandleDragEnd,
    /// The text changed from the keyboard or IME (`SelectionChangedCause.keyboard`): the overlay hides.
    Typed,
    /// The field lost focus: the overlay goes away.
    Blurred,
}

/// What an input asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Effects {
    /// The new selection (`userUpdateTextEditingValue`), when it changed.
    pub selection: Option<Selection>,
    /// `EditableText._handleSelectionChanged`'s `requestKeyboard` (every gesture change), or a tap's.
    pub request_keyboard: bool,
    /// Play this (`SelectionOverlay.selectionEndpoints`' handle drag click on Android).
    pub haptic: Option<Haptic>,
}

/// What the overlay shows now, for the app to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Overlay {
    pub handles: Vec<HandleGeometry>,
    pub toolbar: Option<ToolbarAnchors>,
    pub magnifier: Option<MagnifierGeometry>,
}

/// `SelectionChangedCause`, the subset gestures produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    Tap,
    DoubleTap,
    LongPress,
    Drag,
}

/// Which overlay parts the last gesture's pointer allows (`_shouldShowSelectionToolbar` and
/// `_shouldShowSelectionHandles`, set by `onTapDown`, `onSecondaryTapDown` and `onDragSelectionStart`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Controls {
    /// A finger or stylus: toolbar and handles.
    Touch,
    /// A mouse: neither.
    Pointer,
    /// A mouse's secondary tap: the toolbar only.
    SecondaryPointer,
}

impl Controls {
    fn of(device: PointerDevice) -> Self {
        match device {
            PointerDevice::Touch | PointerDevice::Pen => Controls::Touch,
            PointerDevice::Mouse => Controls::Pointer,
        }
    }

    fn toolbar(self) -> bool {
        match self {
            Controls::Touch | Controls::SecondaryPointer => true,
            Controls::Pointer => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visibility {
    Hidden,
    Shown,
}

/// A pointer pressed on the field: the tap's details, kept for its drag.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Press {
    origin: Point<Pixels>,
    device: PointerDevice,
    /// The effective tap count (`_getEffectiveConsecutiveTapCount`).
    tap_count: usize,
    shift: bool,
}

/// The gesture in progress: one per field, its data on the variant that owns it.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Gesture {
    Idle,
    /// Down, not yet past the drag slop: an `Up` is a tap.
    Pressed(Press),
    /// `onDragSelection*`; `start_selection` is `_dragStartSelection`.
    Dragging {
        press: Press,
        start_selection: Selection,
    },
    /// `onSingleLongTap*`; `origin` is where it started, `without_focus` is `_longPressStartedWithoutFocus`.
    LongPress {
        origin: Point<Pixels>,
        without_focus: bool,
    },
    /// `TextSelectionOverlay._handleSelection{Start,End}HandleDrag*`: `drag_position` and `drag_target` are its
    /// `_*HandleDragPosition` and `_*HandleDragTarget`, `start_selection` its `_dragStartSelection`.
    HandleDrag {
        handle: Handle,
        drag_position: Pixels,
        drag_target: Pixels,
        start_selection: Selection,
    },
}

/// One field's selection gestures and overlay. Feed it every [`Input`] with the field and its layout; apply the
/// [`Effects`]; draw [`TextSelection::overlay`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextSelection {
    rules: SelectionRules,
    handle_haptic: Option<Haptic>,
    gesture: Gesture,
    controls: Controls,
    handles: Visibility,
    toolbar: Visibility,
    /// Where the magnifier is, while it shows.
    magnifier: Option<MagnifierGeometry>,
}

impl TextSelection {
    /// The engine for a behaviour row, `None` where the platform selects natively (iOS).
    pub fn for_behaviour(behaviour: &Behaviour) -> Option<Self> {
        match behaviour.text_selection {
            TextSelectionControls::Native => None,
            TextSelectionControls::Flutter(rules) => Some(Self {
                rules,
                handle_haptic: behaviour.haptics.selection_handle_drag,
                gesture: Gesture::Idle,
                controls: Controls::Touch,
                handles: Visibility::Hidden,
                toolbar: Visibility::Hidden,
                magnifier: None,
            }),
        }
    }

    /// Runs one input: `state × input → next`, every pair in this one match.
    pub fn handle(&mut self, input: Input, field: &Field, layout: &impl TextLayout) -> Effects {
        let mut step = Step {
            engine: self,
            field: *field,
            layout,
            effects: Effects::default(),
        };
        let next = match (step.engine.gesture, input) {
            (
                Gesture::Idle | Gesture::Pressed(_),
                Input::Down {
                    position,
                    device,
                    tap_count,
                    shift,
                },
            ) => {
                let press = Press {
                    origin: position,
                    device,
                    tap_count: step.effective_tap_count(tap_count),
                    shift,
                };
                step.tap_down(press);
                Gesture::Pressed(press)
            }
            (Gesture::Pressed(press), Input::Move { position }) => {
                if step.past_drag_slop(press, position) {
                    let start_selection = step.field.selection;
                    step.drag_start(press);
                    step.drag_update(press, start_selection, position);
                    Gesture::Dragging {
                        press,
                        start_selection,
                    }
                } else {
                    Gesture::Pressed(press)
                }
            }
            (
                Gesture::Dragging {
                    press,
                    start_selection,
                },
                Input::Move { position },
            ) => {
                step.drag_update(press, start_selection, position);
                Gesture::Dragging {
                    press,
                    start_selection,
                }
            }
            (Gesture::Pressed(press), Input::Up) => {
                step.tap_up(press);
                Gesture::Idle
            }
            (Gesture::Dragging { press, .. }, Input::Up) => {
                step.drag_end(press);
                Gesture::Idle
            }
            (Gesture::Idle | Gesture::Pressed(_), Input::SecondaryTap { position, device }) => {
                step.secondary_tap(position, device);
                Gesture::Idle
            }
            // GPUI recognizes a long press from a touch that never became a tap, so it starts from any state but a
            // drag of the field or a handle.
            (Gesture::Idle | Gesture::Pressed(_), Input::LongPressStart { position }) => {
                step.long_press_start(position)
            }
            (
                Gesture::LongPress {
                    origin,
                    without_focus,
                },
                Input::LongPressMove { position },
            ) => {
                step.long_press_move(origin, without_focus, position);
                Gesture::LongPress {
                    origin,
                    without_focus,
                }
            }
            (Gesture::LongPress { .. }, Input::LongPressEnd) => {
                step.long_press_end_or_cancel();
                if step.engine.controls.toolbar() {
                    step.show_toolbar();
                }
                Gesture::Idle
            }
            (Gesture::LongPress { .. }, Input::LongPressCancel) => {
                step.long_press_end_or_cancel();
                Gesture::Idle
            }
            (Gesture::Idle | Gesture::Pressed(_), Input::HandleDragStart { handle, position }) => {
                step.handle_drag_start(handle, position)
            }
            (
                Gesture::HandleDrag {
                    handle,
                    drag_position,
                    drag_target,
                    start_selection,
                },
                Input::HandleDragUpdate { position },
            ) => {
                let drag_position = step.handle_drag_update(
                    handle,
                    drag_position,
                    drag_target,
                    start_selection,
                    position,
                );
                Gesture::HandleDrag {
                    handle,
                    drag_position,
                    drag_target,
                    start_selection,
                }
            }
            (Gesture::HandleDrag { .. }, Input::HandleDragEnd) => {
                step.handle_drag_end();
                Gesture::Idle
            }
            (_, Input::Typed | Input::Blurred) => {
                step.hide_toolbar(HideHandles::Yes);
                Gesture::Idle
            }
            // Rejected: input that doesn't belong to the gesture in progress (a stray move, a second long press,
            // a handle update with no handle drag) changes nothing.
            (
                gesture @ (Gesture::Idle
                | Gesture::Pressed(_)
                | Gesture::Dragging { .. }
                | Gesture::LongPress { .. }
                | Gesture::HandleDrag { .. }),
                Input::Down { .. }
                | Input::Move { .. }
                | Input::Up
                | Input::SecondaryTap { .. }
                | Input::LongPressStart { .. }
                | Input::LongPressMove { .. }
                | Input::LongPressEnd
                | Input::LongPressCancel
                | Input::HandleDragStart { .. }
                | Input::HandleDragUpdate { .. }
                | Input::HandleDragEnd,
            ) => gesture,
        };
        let effects = step.effects;
        self.gesture = next;
        effects
    }

    /// The overlay to draw for `field` now: handles while focused and allowed, the toolbar's anchors, the magnifier.
    pub fn overlay(&self, field: &Field, layout: &impl TextLayout) -> Overlay {
        let shown = |visibility| field.focused && visibility == Visibility::Shown;
        Overlay {
            handles: if shown(self.handles) {
                geometry::handles(layout, field.selection, self.rules.handle_size)
            } else {
                Vec::new()
            },
            toolbar: shown(self.toolbar).then(|| toolbar_anchors(layout, field.selection)),
            magnifier: self.magnifier,
        }
    }
}

/// Whether `hideToolbar` hides the handles too (`EditableTextState.hideToolbar(hideHandles)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HideHandles {
    Yes,
    No,
}

/// One input's run: the engine, the field as the app passed it (its selection updated as the step changes it),
/// and the effects so far. The methods are Flutter's builder handlers, named after them.
struct Step<'a, L: TextLayout> {
    engine: &'a mut TextSelection,
    field: Field,
    layout: &'a L,
    effects: Effects,
}

impl<L: TextLayout> Step<'_, L> {
    /// `_getEffectiveConsecutiveTapCount`: the raw count folded to 1–3 as the platform does.
    fn effective_tap_count(&self, raw: usize) -> usize {
        match self.engine.rules.tap_count {
            // A fourth tap starts again at one.
            TapCount::Cycle if raw <= 3 => raw,
            TapCount::Cycle if raw.is_multiple_of(3) => 3,
            TapCount::Cycle => raw % 3,
            // The count holds at three.
            TapCount::Hold => raw.min(3),
            // Double and triple alternate.
            TapCount::Alternate if raw < 2 => raw,
            TapCount::Alternate => 2 + raw % 2,
        }
    }

    /// `_hasSufficientGlobalDistanceToAccept` of the recognizer the platform uses.
    fn past_drag_slop(&self, press: Press, position: Point<Pixels>) -> bool {
        let slop = slops(press.device, None);
        let moved = position - press.origin;
        match self.engine.rules.drag_recognizer {
            DragRecognizer::Horizontal => moved.x.abs() > slop.hit,
            DragRecognizer::Pan => moved.x.as_f32().hypot(moved.y.as_f32()) > slop.pan.as_f32(),
        }
    }

    /// `_TextSelectionGestureDetectorState._handleTapDown`: `onTapDown`, then `onDoubleTapDown` or
    /// `onTripleTapDown` by the count.
    fn tap_down(&mut self, press: Press) {
        self.on_tap_down(press);
        match press.tap_count {
            2 => self.on_double_tap_down(press.origin),
            3 => self.on_triple_tap_down(press.origin),
            _ => {}
        }
    }

    fn on_tap_down(&mut self, press: Press) {
        // The overlay shows only for a finger or stylus, never a mouse.
        self.engine.controls = Controls::of(press.device);
        match self.engine.rules.tap_selects_on {
            // On mobile platforms the selection is set on tap up.
            TapEdge::Up => {}
            TapEdge::Down => {
                self.hide_toolbar(HideHandles::Yes);
                if press.shift {
                    return self.shift_select(press.origin, Cause::Tap);
                }
                // A tap or click places the caret precisely.
                self.select_position_at(press.origin, None, Cause::Tap);
            }
        }
    }

    /// Shift + tap: Apple's `_expandSelection` (an unfocused field expands from 0) or `_extendSelection`.
    fn shift_select(&mut self, position: Point<Pixels>, cause: Cause) {
        match self.engine.rules.shift_select {
            ShiftSelect::AppleExpand => {
                let from = (!self.field.focused).then(|| Selection::collapsed(0));
                self.expand_selection(position, cause, from);
            }
            ShiftSelect::Extend => self.extend_selection(position, cause),
        }
    }

    /// `_TextSelectionGestureDetectorState._handleTapUp`: `onSingleTapUp` for a first tap.
    fn tap_up(&mut self, press: Press) {
        if press.tap_count != 1 {
            return;
        }
        match self.engine.rules.tap_selects_on {
            // On desktop platforms the selection is set on tap down.
            TapEdge::Down => {}
            TapEdge::Up => {
                self.hide_toolbar(HideHandles::No);
                if press.shift {
                    return self.extend_selection(press.origin, Cause::Tap);
                }
                self.select_position_at(press.origin, None, Cause::Tap);
            }
        }
        self.effects.request_keyboard = true;
    }

    fn on_double_tap_down(&mut self, position: Point<Pixels>) {
        self.select_words_in_range(position, None, Cause::DoubleTap);
        if self.engine.controls.toolbar() {
            self.show_toolbar();
        }
    }

    fn on_triple_tap_down(&mut self, position: Point<Pixels>) {
        match self.field.lines {
            Lines::Single => self.select(
                Selection {
                    base: 0,
                    extent: self.layout.len(),
                },
                Cause::Tap,
            ),
            Lines::Multi => self.select_text_boundaries_in_range(position, None, Cause::Tap),
        }
        if self.engine.controls.toolbar() {
            self.show_toolbar();
        }
    }

    fn secondary_tap(&mut self, position: Point<Pixels>, device: PointerDevice) {
        // onSecondaryTapDown: the toolbar always, the handles for a finger or stylus.
        self.engine.controls = match Controls::of(device) {
            Controls::Touch => Controls::Touch,
            Controls::Pointer | Controls::SecondaryPointer => Controls::SecondaryPointer,
        };
        match self.engine.rules.secondary_tap {
            SecondaryTap::AppleSelectWord => {
                let tapped = self.layout.position_for_point(position);
                let selection = self.field.selection;
                let on_selection = selection.start() <= tapped && tapped <= selection.end();
                if !on_selection || !self.field.focused {
                    self.select_words_in_range(position, None, Cause::Tap);
                }
                self.hide_toolbar(HideHandles::Yes);
                self.show_toolbar();
            }
            SecondaryTap::CaretWhenUnfocused => {
                if !self.field.focused {
                    self.select_position_at(position, None, Cause::Tap);
                }
                self.toggle_toolbar();
            }
        }
    }

    /// `onSingleLongTapStart`, after the `onTapDown` Flutter's recognizer runs first for the touch.
    fn long_press_start(&mut self, position: Point<Pixels>) -> Gesture {
        self.on_tap_down(Press {
            origin: position,
            device: PointerDevice::Touch,
            tap_count: 1,
            shift: false,
        });
        let without_focus = match self.engine.rules.long_press {
            LongPressSelects::AppleCaretWhenFocused if self.field.focused => {
                self.select_position_at(position, None, Cause::LongPress);
                false
            }
            LongPressSelects::AppleCaretWhenFocused => {
                self.select_words_in_range(position, None, Cause::LongPress);
                true
            }
            LongPressSelects::Word => {
                self.select_words_in_range(position, None, Cause::LongPress);
                false
            }
        };
        self.show_magnifier_if_supported(position);
        Gesture::LongPress {
            origin: position,
            without_focus,
        }
    }

    fn long_press_move(
        &mut self,
        origin: Point<Pixels>,
        without_focus: bool,
        position: Point<Pixels>,
    ) {
        match self.engine.rules.long_press {
            LongPressSelects::AppleCaretWhenFocused if !without_focus => {
                self.select_position_at(position, None, Cause::LongPress)
            }
            LongPressSelects::AppleCaretWhenFocused | LongPressSelects::Word => {
                self.select_words_in_range(origin, Some(position), Cause::LongPress)
            }
        }
        self.show_magnifier_if_supported(position);
    }

    fn long_press_end_or_cancel(&mut self) {
        self.engine.magnifier = None;
    }

    /// `onDragSelectionStart`, at the press's origin (`DragStartBehavior.down`).
    fn drag_start(&mut self, press: Press) {
        self.engine.controls = Controls::of(press.device);
        // A consecutive tap and drag keeps the double or triple tap's selection.
        if press.tap_count > 1 {
            return;
        }
        if press.shift {
            return match self.engine.rules.shift_select {
                ShiftSelect::AppleExpand => self.expand_selection(press.origin, Cause::Drag, None),
                ShiftSelect::Extend => self.extend_selection(press.origin, Cause::Drag),
            };
        }
        match (self.engine.rules.touch_drag, press.device) {
            (TouchDragSelects::CaretWhenFocused, PointerDevice::Mouse)
            | (TouchDragSelects::Range, _) => {
                self.select_position_at(press.origin, None, Cause::Drag)
            }
            // A touch drag does not start unless the field has focus.
            (TouchDragSelects::CaretWhenFocused, PointerDevice::Touch | PointerDevice::Pen) => {
                if self.field.focused {
                    self.select_position_at(press.origin, None, Cause::Drag);
                    self.show_magnifier_if_supported(press.origin);
                }
            }
        }
    }

    /// `onDragSelectionUpdate`.
    fn drag_update(&mut self, press: Press, start_selection: Selection, position: Point<Pixels>) {
        let origin = press.origin;
        let finger = matches!(press.device, PointerDevice::Touch | PointerDevice::Pen);
        if !press.shift {
            return match press.tap_count {
                // Word by word.
                2 => {
                    self.select_words_in_range(origin, Some(position), Cause::Drag);
                    if finger {
                        self.show_magnifier_if_supported(position);
                    }
                }
                // Paragraph (or line) by paragraph; mobile platforms have no triple tap drag for a finger.
                3 => match (self.engine.rules.touch_drag, press.device) {
                    (
                        TouchDragSelects::CaretWhenFocused,
                        PointerDevice::Touch | PointerDevice::Pen,
                    ) => {}
                    (TouchDragSelects::CaretWhenFocused, PointerDevice::Mouse)
                    | (TouchDragSelects::Range, _) => {
                        self.select_text_boundaries_in_range(origin, Some(position), Cause::Drag)
                    }
                },
                _ => match (self.engine.rules.touch_drag, press.device) {
                    // A precise pointer selects from the drag's origin; a finger moves the caret.
                    (
                        TouchDragSelects::CaretWhenFocused,
                        PointerDevice::Mouse | PointerDevice::Pen,
                    )
                    | (TouchDragSelects::Range, _) => {
                        self.select_position_at(origin, Some(position), Cause::Drag)
                    }
                    (TouchDragSelects::CaretWhenFocused, PointerDevice::Touch) => {
                        if self.field.focused {
                            self.select_position_at(position, None, Cause::Drag);
                            self.show_magnifier_if_supported(position);
                        }
                    }
                },
            };
        }
        if start_selection.is_collapsed() || self.engine.rules.shift_select == ShiftSelect::Extend {
            return self.extend_selection(position, Cause::Drag);
        }
        // If the drag inverts the selection, Apple platforms revert to the initial selection.
        let selection = self.field.selection;
        let next_extent = self.layout.position_for_point(position);
        let is_forward = start_selection.base < start_selection.extent;
        let is_inverted = if is_forward {
            next_extent < start_selection.base
        } else {
            next_extent > start_selection.base
        };
        if is_inverted && selection.base == start_selection.base {
            self.select(
                Selection {
                    base: start_selection.extent,
                    extent: next_extent,
                },
                Cause::Drag,
            );
        } else if !is_inverted
            && next_extent != start_selection.base
            && selection.base != start_selection.base
        {
            self.select(
                Selection {
                    base: start_selection.base,
                    extent: next_extent,
                },
                Cause::Drag,
            );
        } else {
            self.extend_selection(position, Cause::Drag);
        }
    }

    /// `onDragSelectionEnd`.
    fn drag_end(&mut self, press: Press) {
        if self.engine.controls.toolbar() && press.tap_count == 2 {
            self.show_toolbar();
        }
        self.engine.magnifier = None;
    }

    /// `TextSelectionOverlay._handleSelection{Start,End}HandleDragStart`.
    fn handle_drag_start(&mut self, handle: Handle, position: Point<Pixels>) -> Gesture {
        let [start, end] = selection_endpoints(self.layout, self.field.selection);
        let endpoint = match handle {
            Handle::Start => start,
            Handle::End => end,
        };
        // Instead of finding the position at the handle's location directly, use the vertical centre of the line it
        // points to: handles hang below the line.
        let center_of_line = endpoint.y - self.layout.preferred_line_height() / 2.;
        let text_position = self
            .layout
            .position_for_point(point(position.x, center_of_line));
        self.show_magnifier(position, text_position);
        Gesture::HandleDrag {
            handle,
            drag_position: position.y,
            drag_target: center_of_line - position.y,
            start_selection: self.field.selection,
        }
    }

    /// `TextSelectionOverlay._handleSelection{Start,End}HandleDragUpdate`; returns the new `drag_position`.
    fn handle_drag_update(
        &mut self,
        handle: Handle,
        drag_position: Pixels,
        drag_target: Pixels,
        start_selection: Selection,
        position: Point<Pixels>,
    ) -> Pixels {
        let Some(drag_position) = handle_dy(
            position.y,
            drag_position,
            self.layout.preferred_line_height(),
        ) else {
            return drag_position;
        };
        let text_position = self
            .layout
            .position_for_point(point(position.x, drag_position + drag_target));
        let selection = self.field.selection;
        let collapsed = match self.engine.rules.handle_drag {
            HandleDrag::AppleExtent => start_selection.is_collapsed(),
            HandleDrag::Ordered => selection.is_collapsed(),
        };
        if collapsed {
            self.update_magnifier(position, text_position);
            self.handle_changed(Selection::collapsed(text_position));
            return drag_position;
        }
        let new_selection = match (self.engine.rules.handle_drag, handle) {
            // On Apple platforms the dragged handle becomes the extent.
            (HandleDrag::AppleExtent, _) => {
                let normalized = start_selection.extent >= start_selection.base;
                let other_end = match (handle, normalized) {
                    (Handle::End, true) | (Handle::Start, false) => start_selection.base,
                    (Handle::End, false) | (Handle::Start, true) => start_selection.extent,
                };
                Selection {
                    base: other_end,
                    extent: text_position,
                }
            }
            (HandleDrag::Ordered, Handle::End) => Selection {
                base: selection.base,
                extent: text_position,
            },
            (HandleDrag::Ordered, Handle::Start) => Selection {
                base: text_position,
                extent: selection.extent,
            },
        };
        // Don't allow order swapping.
        if self.engine.rules.handle_drag == HandleDrag::Ordered
            && new_selection.base >= new_selection.extent
        {
            return drag_position;
        }
        self.handle_changed(new_selection);
        let magnified = match handle {
            Handle::End => new_selection.extent,
            Handle::Start => new_selection.start(),
        };
        self.update_magnifier(position, magnified);
        drag_position
    }

    /// `_handleSelectionHandleChanged`, with `SelectionOverlay.selectionEndpoints`' click while a handle drags.
    fn handle_changed(&mut self, selection: Selection) {
        if selection != self.field.selection {
            self.effects.haptic = self.engine.handle_haptic;
        }
        self.select(selection, Cause::Drag);
    }

    /// `TextSelectionOverlay._handleAnyDragEnd`.
    fn handle_drag_end(&mut self) {
        self.engine.magnifier = None;
        if !self.field.selection.is_collapsed() {
            self.show_toolbar();
        }
    }

    /// `_showMagnifierIfSupportedByPlatform`: only where the row has a magnifier.
    fn show_magnifier_if_supported(&mut self, position: Point<Pixels>) {
        match self.engine.rules.magnifier {
            MagnifierKind::None => {}
            MagnifierKind::Material => {
                let offset = self.layout.position_for_point(position);
                match self.engine.magnifier {
                    None => self.show_magnifier(position, offset),
                    Some(_) => self.update_magnifier(position, offset),
                }
            }
        }
    }

    /// `SelectionOverlay.showMagnifier`: hides the toolbar, then shows the magnifier if the platform has one.
    fn show_magnifier(&mut self, position: Point<Pixels>, offset: usize) {
        if self.engine.magnifier.is_some() {
            return;
        }
        if self.engine.toolbar == Visibility::Shown {
            self.engine.toolbar = Visibility::Hidden;
        }
        self.update_magnifier(position, offset);
    }

    /// `SelectionOverlay.updateMagnifier`.
    fn update_magnifier(&mut self, position: Point<Pixels>, offset: usize) {
        match self.engine.rules.magnifier {
            MagnifierKind::None => {}
            MagnifierKind::Material => {
                let info = magnifier_info(self.layout, position, offset);
                self.engine.magnifier = Some(text_magnifier(
                    info,
                    self.field.screen,
                    self.engine.magnifier,
                ));
            }
        }
    }

    fn show_toolbar(&mut self) {
        self.engine.toolbar = Visibility::Shown;
    }

    fn hide_toolbar(&mut self, hide_handles: HideHandles) {
        self.engine.toolbar = Visibility::Hidden;
        match hide_handles {
            HideHandles::Yes => {
                self.engine.handles = Visibility::Hidden;
                self.engine.magnifier = None;
            }
            HideHandles::No => {}
        }
    }

    fn toggle_toolbar(&mut self) {
        match self.engine.toolbar {
            Visibility::Shown => self.hide_toolbar(HideHandles::Yes),
            Visibility::Hidden => self.show_toolbar(),
        }
    }

    /// `RenderEditable.selectPositionAt`.
    fn select_position_at(&mut self, from: Point<Pixels>, to: Option<Point<Pixels>>, cause: Cause) {
        let base = self.layout.position_for_point(from);
        let extent = to.map_or(base, |to| self.layout.position_for_point(to));
        self.select(Selection { base, extent }, cause);
    }

    /// `RenderEditable.selectWordsInRange` (and `selectWord`, with no `to`): the first and last endpoints are always
    /// at the beginning and end of a word.
    fn select_words_in_range(
        &mut self,
        from: Point<Pixels>,
        to: Option<Point<Pixels>>,
        cause: Cause,
    ) {
        let from_position = self.layout.position_for_point(from);
        let from_word = self.word_at_offset(from_position);
        let to_position = to.map_or(from_position, |to| self.layout.position_for_point(to));
        let to_word = if to_position == from_position {
            from_word
        } else {
            self.word_at_offset(to_position)
        };
        let is_from_word_before_to_word = from_word.start() < to_word.end();
        self.select(
            if is_from_word_before_to_word {
                Selection {
                    base: from_word.base,
                    extent: to_word.extent,
                }
            } else {
                Selection {
                    base: from_word.extent,
                    extent: to_word.base,
                }
            },
            cause,
        );
    }

    /// `RenderEditable.getWordAtOffset`: past the end of the text, a collapsed caret.
    fn word_at_offset(&self, offset: usize) -> Selection {
        let len = self.layout.len();
        if offset >= len {
            return Selection::collapsed(len);
        }
        let word = self.layout.word_boundary(offset);
        Selection {
            base: word.start,
            extent: word.end,
        }
    }

    /// `_selectTextBoundariesInRange` with the row's boundary (`_selectParagraphsInRange`, `_selectLinesInRange`).
    fn select_text_boundaries_in_range(
        &mut self,
        from: Point<Pixels>,
        to: Option<Point<Pixels>>,
        cause: Cause,
    ) {
        let from_position = self.layout.position_for_point(from);
        let from_range = self.move_to_text_boundary(from_position);
        let to_position = to.map_or(from_position, |to| self.layout.position_for_point(to));
        let to_range = if to_position == from_position {
            from_range.clone()
        } else {
            self.move_to_text_boundary(to_position)
        };
        let selection = if from_range.start < to_range.end {
            Selection {
                base: from_range.start,
                extent: to_range.end,
            }
        } else {
            Selection {
                base: from_range.end,
                extent: to_range.start,
            }
        };
        self.select(selection, cause);
    }

    /// `_moveToTextBoundary`: at the end of the text, the boundary before it.
    fn move_to_text_boundary(&self, extent: usize) -> Range<usize> {
        let at = if extent == self.layout.len() {
            extent.saturating_sub(1)
        } else {
            extent
        };
        match self.engine.rules.triple_tap {
            TextBoundaryKind::Paragraph => self.layout.paragraph_at(at),
            TextBoundaryKind::Line => self.layout.line_at(at),
        }
    }

    /// `_expandSelection`: the nearer end moves to the tap; the selection never shrinks or pivots.
    fn expand_selection(&mut self, position: Point<Pixels>, cause: Cause, from: Option<Selection>) {
        let tapped = self.layout.position_for_point(position);
        let selection = from.unwrap_or(self.field.selection);
        let base_is_closer = tapped.abs_diff(selection.base) < tapped.abs_diff(selection.extent);
        self.select(
            Selection {
                base: if base_is_closer {
                    selection.extent
                } else {
                    selection.base
                },
                extent: tapped,
            },
            cause,
        );
    }

    /// `_extendSelection`: the base stays, the extent moves to the tap.
    fn extend_selection(&mut self, position: Point<Pixels>, cause: Cause) {
        let extent = self.layout.position_for_point(position);
        self.select(
            Selection {
                extent,
                ..self.field.selection
            },
            cause,
        );
    }

    /// `userUpdateTextEditingValue` → `EditableText._handleSelectionChanged` (requests the keyboard) →
    /// `TextField._handleSelectionChanged` (shows the handles by `_shouldShowSelectionHandles`; a desktop drag hides
    /// the toolbar). It runs when the selection changed, or for any long press.
    fn select(&mut self, selection: Selection, cause: Cause) {
        if selection == self.field.selection && cause != Cause::LongPress {
            return;
        }
        if selection != self.field.selection {
            self.effects.selection = Some(selection);
            self.field.selection = selection;
        }
        self.effects.request_keyboard = true;
        let show_handles = self.engine.controls == Controls::Touch
            && (cause == Cause::LongPress || !self.layout.is_empty());
        self.engine.handles = if show_handles {
            Visibility::Shown
        } else {
            Visibility::Hidden
        };
        if self.engine.rules.drag_hides_toolbar && cause == Cause::Drag {
            self.engine.toolbar = Visibility::Hidden;
        }
    }
}
