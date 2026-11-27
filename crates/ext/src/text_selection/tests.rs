// Ported from flutter/flutter @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7): test cases of
// packages/flutter/test/widgets/text_selection_test.dart, packages/flutter/test/material/text_field_test.dart,
// packages/flutter/test/material/magnifier_test.dart and packages/flutter/test/widgets/magnifier_test.dart, one row
// each, named after the Flutter test. Copyright 2014 The Flutter Authors. BSD-3-Clause, see
// third_party/flutter/LICENSE.

use std::ops::Range;

use gpui::{Bounds, Haptic, Pixels, Point, PointerDevice, point, px, size};
use parallax_platform::Target;
use parallax_platform::behaviour::behaviour;

use super::*;

/// A monospace single-line layout standing in for Flutter's test `RenderEditable`: each character 10 px wide, a
/// 20 px line at the window's top left, words split at spaces (ICU splits punctuation too; no row needs it).
struct Mono(&'static str);

const CHAR: f32 = 10.;
const LINE: f32 = 20.;

impl TextLayout for Mono {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn position_for_point(&self, point: Point<Pixels>) -> usize {
        ((point.x.as_f32() / CHAR).round().max(0.) as usize).min(self.0.len())
    }

    fn word_boundary(&self, offset: usize) -> Range<usize> {
        let bytes = self.0.as_bytes();
        let space = |index: usize| bytes[index] == b' ';
        let kind = space(offset);
        let start = (0..offset)
            .rev()
            .find(|&index| space(index) != kind)
            .map_or(0, |index| index + 1);
        let end = (offset..bytes.len())
            .find(|&index| space(index) != kind)
            .unwrap_or(bytes.len());
        start..end
    }

    fn line_at(&self, _offset: usize) -> Range<usize> {
        0..self.0.len()
    }

    fn paragraph_at(&self, _offset: usize) -> Range<usize> {
        0..self.0.len()
    }

    fn caret_rect(&self, offset: usize) -> Bounds<Pixels> {
        Bounds::new(
            point(px(offset as f32 * CHAR), px(0.)),
            size(px(2.), px(LINE)),
        )
    }

    fn preferred_line_height(&self) -> Pixels {
        px(LINE)
    }

    fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(point(px(0.), px(0.)), size(px(400.), px(LINE)))
    }
}

/// The point Flutter's `textOffsetToPosition` gives: on the caret, mid-line.
fn at(offset: usize) -> Point<Pixels> {
    point(px(offset as f32 * CHAR), px(LINE / 2.))
}

use PointerDevice::{Mouse, Pen, Touch};

fn down(offset: usize, device: PointerDevice, tap_count: usize) -> Input {
    Input::Down {
        position: at(offset),
        device,
        tap_count,
        shift: false,
    }
}

fn shift_tap(offset: usize, device: PointerDevice) -> [Input; 2] {
    [
        Input::Down {
            position: at(offset),
            device,
            tap_count: 1,
            shift: true,
        },
        Input::Up,
    ]
}

fn tap(offset: usize, device: PointerDevice, tap_count: usize) -> [Input; 2] {
    [down(offset, device, tap_count), Input::Up]
}

fn to(offset: usize) -> Input {
    Input::Move {
        position: at(offset),
    }
}

fn long_press(offset: usize) -> Input {
    Input::LongPressStart {
        position: at(offset),
    }
}

fn long_move(offset: usize) -> Input {
    Input::LongPressMove {
        position: at(offset),
    }
}

/// A handle drag from just off the handle's endpoint (the bottom of the line at `offset`) to `offset_to`.
fn drag_handle(handle: Handle, from: usize, to: usize) -> [Input; 3] {
    [
        Input::HandleDragStart {
            handle,
            position: point(px(from as f32 * CHAR + 1.), px(LINE + 1.)),
        },
        Input::HandleDragUpdate { position: at(to) },
        Input::HandleDragEnd,
    ]
}

/// What a row checks once its inputs ran.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Expected {
    selection: (usize, usize),
    handles: usize,
    toolbar: bool,
    magnifier: bool,
    haptic: Option<Haptic>,
}

const CARET_SHOWN: Expected = Expected {
    selection: (0, 0),
    handles: 1,
    toolbar: false,
    magnifier: false,
    haptic: None,
};

fn caret(offset: usize) -> Expected {
    Expected {
        selection: (offset, offset),
        ..CARET_SHOWN
    }
}

fn range(base: usize, extent: usize) -> Expected {
    Expected {
        selection: (base, extent),
        handles: 2,
        ..CARET_SHOWN
    }
}

fn no_overlay(expected: Expected) -> Expected {
    Expected {
        handles: 0,
        toolbar: false,
        magnifier: false,
        ..expected
    }
}

fn with_toolbar(expected: Expected) -> Expected {
    Expected {
        toolbar: true,
        ..expected
    }
}

fn with_magnifier(expected: Expected) -> Expected {
    Expected {
        magnifier: true,
        ..expected
    }
}

/// How the field starts: its text, selection and focus.
struct Start {
    text: &'static str,
    selection: (usize, usize),
    focused: bool,
}

const ABC: Start = Start {
    text: "abc def ghi",
    selection: (0, 0),
    focused: false,
};

const ATWATER: Start = Start {
    text: "Atwater Peel Sherbrooke Bonaventure",
    selection: (0, 0),
    focused: false,
};

/// Runs `inputs` on `target`'s engine the way an app does: it applies each selection, and a keyboard request
/// focuses the field (`EditableText.requestKeyboard`).
fn run(target: Target, start: &Start, inputs: &[Input]) -> Expected {
    let row = behaviour(Some(target)).expect("every engine target has a row");
    let mut engine = TextSelection::for_behaviour(row).expect("the row uses Flutter's selection");
    let layout = Mono(start.text);
    let mut field = Field {
        selection: Selection {
            base: start.selection.0,
            extent: start.selection.1,
        },
        focused: start.focused,
        lines: Lines::Single,
        screen: Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.))),
    };
    let mut haptic = None;
    for &input in inputs {
        let effects = engine.handle(input, &field, &layout);
        field.selection = effects.selection.unwrap_or(field.selection);
        field.focused |= effects.request_keyboard;
        field.focused &= input != Input::Blurred;
        haptic = haptic.or(effects.haptic);
    }
    let overlay = engine.overlay(&field, &layout);
    Expected {
        selection: (field.selection.base, field.selection.extent),
        handles: overlay.handles.len(),
        toolbar: overlay.toolbar.is_some(),
        magnifier: overlay.magnifier.is_some(),
        haptic,
    }
}

fn steps<const N: usize>(groups: [&[Input]; N]) -> Vec<Input> {
    groups.concat()
}

#[test]
fn flutter_selection_gesture_tests_pass() {
    use Target::{Android, Linux, MacOs, Windows};
    let focused_at = |text, base, extent| Start {
        text,
        selection: (base, extent),
        focused: true,
    };
    let unfocused_at = |text, offset| Start {
        text,
        selection: (offset, offset),
        focused: false,
    };
    // (Flutter test name, target, start, inputs, expected)
    let rows: Vec<(&str, Target, Start, Vec<Input>, Expected)> = vec![
        (
            "test TextSelectionGestureDetectorBuilder tap (android)",
            Android,
            ABC,
            tap(5, Touch, 1).to_vec(),
            caret(5),
        ),
        (
            "test TextSelectionGestureDetectorBuilder tap (macos)",
            MacOs,
            ABC,
            tap(5, Touch, 1).to_vec(),
            caret(5),
        ),
        (
            "test TextSelectionGestureDetectorBuilder tap (linux)",
            Linux,
            ABC,
            tap(5, Touch, 1).to_vec(),
            caret(5),
        ),
        (
            "test TextSelectionGestureDetectorBuilder tap (windows)",
            Windows,
            ABC,
            tap(5, Touch, 1).to_vec(),
            caret(5),
        ),
        (
            "test TextSelectionGestureDetectorBuilder double tap",
            Android,
            ABC,
            steps([&tap(5, Touch, 1), &tap(5, Touch, 2)]),
            with_toolbar(range(4, 7)),
        ),
        (
            "Mouse drag selects and cannot drag cursor",
            Android,
            Start {
                text: "I love flutter!",
                ..ABC
            },
            steps([
                &tap(4, Touch, 1),
                &[down(4, Mouse, 1), to(7), to(10), Input::Up],
            ]),
            no_overlay(range(4, 10)),
        ),
        (
            "Touch drag moves the cursor",
            Android,
            Start {
                text: "I love flutter!",
                ..ABC
            },
            steps([
                &tap(4, Touch, 1),
                &[down(4, Touch, 1), to(7), to(10), Input::Up],
            ]),
            caret(10),
        ),
        (
            "Stylus drag selects text (android)",
            Android,
            Start {
                text: "I love flutter!",
                ..ABC
            },
            steps([
                &tap(4, Touch, 1),
                &[down(4, Pen, 1), to(7), to(10), Input::Up],
            ]),
            range(4, 10),
        ),
        (
            "Stylus drag selects text (macos)",
            MacOs,
            Start {
                text: "I love flutter!",
                ..ABC
            },
            steps([
                &tap(4, Touch, 1),
                &[down(4, Pen, 1), to(7), to(10), Input::Up],
            ]),
            range(4, 10),
        ),
        (
            "Stylus drag selects text (linux)",
            Linux,
            Start {
                text: "I love flutter!",
                ..ABC
            },
            steps([
                &tap(4, Touch, 1),
                &[down(4, Pen, 1), to(7), to(10), Input::Up],
            ]),
            range(4, 10),
        ),
        (
            "Stylus drag selects text (windows)",
            Windows,
            Start {
                text: "I love flutter!",
                ..ABC
            },
            steps([
                &tap(4, Touch, 1),
                &[down(4, Pen, 1), to(7), to(10), Input::Up],
            ]),
            range(4, 10),
        ),
        (
            "Mouse drag does not show handles nor toolbar",
            Android,
            Start {
                text: "I love Flutter!",
                ..ABC
            },
            vec![down(0, Mouse, 1), to(5), Input::Up],
            no_overlay(range(0, 5)),
        ),
        (
            "Can double click + drag with a mouse to select word by word",
            Android,
            ABC,
            steps([&tap(4, Mouse, 1), &[down(4, Mouse, 2), to(8)]]),
            no_overlay(range(4, 11)),
        ),
        (
            "Can double tap + drag to select word by word: toolbar hidden during the drag",
            Android,
            ABC,
            steps([&tap(4, Touch, 1), &[down(4, Touch, 2), to(8)]]),
            with_magnifier(range(4, 11)),
        ),
        (
            "Can double tap + drag to select word by word: toolbar re-appears after the drag",
            Android,
            ABC,
            steps([&tap(4, Touch, 1), &[down(4, Touch, 2), to(8), Input::Up]]),
            with_toolbar(range(4, 11)),
        ),
        (
            "long press selects word and shows toolbar",
            Android,
            ABC,
            vec![long_press(5), Input::LongPressEnd],
            with_toolbar(range(4, 7)),
        ),
        (
            "long press drag extends the selection to the word under the drag and shows toolbar on lift on \
             non-Apple platforms: no toolbar while dragging",
            Android,
            ATWATER,
            vec![long_press(18), long_move(25), long_move(11)],
            with_magnifier(range(23, 8)),
        ),
        (
            "long press drag extends the selection to the word under the drag and shows toolbar on lift on \
             non-Apple platforms",
            Windows,
            ATWATER,
            vec![
                long_press(18),
                long_move(25),
                long_move(11),
                long_move(4),
                Input::LongPressEnd,
            ],
            with_toolbar(range(23, 0)),
        ),
        (
            "long press drag on a focused TextField moves the cursor under the drag and shows toolbar on lift",
            MacOs,
            focused_at(ATWATER.text, 3, 3),
            vec![long_press(18), long_move(25), Input::LongPressEnd],
            with_toolbar(caret(25)),
        ),
        (
            "long press drag on an unfocused TextField selects word-by-word and shows toolbar on lift",
            MacOs,
            ATWATER,
            vec![long_press(18), long_move(25), Input::LongPressEnd],
            with_toolbar(range(13, 35)),
        ),
        (
            "cancelling long press hides magnifier",
            Android,
            ABC,
            vec![long_press(5), Input::LongPressCancel],
            range(4, 7),
        ),
        (
            "Can drag handles to change selection on non-Apple platforms",
            Android,
            ABC,
            steps([
                &[long_press(5), Input::LongPressEnd],
                &drag_handle(Handle::End, 7, 11),
                &drag_handle(Handle::Start, 4, 2),
                &drag_handle(Handle::Start, 2, 0),
            ]),
            Expected {
                haptic: Some(Haptic::SelectionClick),
                ..with_toolbar(range(0, 11))
            },
        ),
        (
            "Can drag handles to change selection on Apple platforms",
            MacOs,
            focused_at(ABC.text, 4, 7),
            steps([
                &drag_handle(Handle::End, 7, 11),
                &drag_handle(Handle::Start, 4, 2),
                &drag_handle(Handle::Start, 2, 0),
            ]),
            with_toolbar(range(11, 0)),
        ),
        (
            "Cannot drag one handle past the other",
            Android,
            ABC,
            vec![
                long_press(5),
                Input::LongPressEnd,
                Input::HandleDragStart {
                    handle: Handle::End,
                    position: point(px(74.), px(LINE)),
                },
                Input::HandleDragUpdate { position: at(5) },
                Input::HandleDragUpdate { position: at(2) },
            ],
            Expected {
                haptic: Some(Haptic::SelectionClick),
                ..with_magnifier(range(4, 5))
            },
        ),
        (
            "Drag handles trigger feedback: none on desktop",
            Windows,
            ABC,
            steps([
                &[long_press(5), Input::LongPressEnd],
                &drag_handle(Handle::End, 7, 11),
            ]),
            with_toolbar(range(4, 11)),
        ),
        (
            "Dragging a collapsed handle should trigger feedback.",
            Android,
            ABC,
            steps([&tap(5, Touch, 1), &drag_handle(Handle::Start, 5, 9)]),
            Expected {
                haptic: Some(Haptic::SelectionClick),
                ..caret(9)
            },
        ),
        (
            "Can triple tap to select all on a single-line textfield on mobile platforms",
            Android,
            ABC,
            steps([&tap(5, Touch, 1), &tap(5, Touch, 2), &tap(5, Touch, 3)]),
            with_toolbar(range(0, 11)),
        ),
        (
            "Can triple click to select all on a single-line textfield on desktop platforms",
            Windows,
            ABC,
            steps([&tap(5, Mouse, 1), &tap(5, Mouse, 2), &tap(5, Mouse, 3)]),
            no_overlay(range(0, 11)),
        ),
        (
            "Tap count resets when going past a triple tap on Android, Fuchsia, and Linux: the fourth click places \
             the caret",
            Linux,
            ABC,
            steps([
                &tap(5, Mouse, 1),
                &tap(5, Mouse, 2),
                &tap(5, Mouse, 3),
                &tap(5, Mouse, 4),
            ]),
            no_overlay(caret(5)),
        ),
        (
            "Tap count resets when going past a triple tap on Android, Fuchsia, and Linux: the fifth selects the word",
            Linux,
            ABC,
            steps([&tap(5, Mouse, 4), &tap(5, Mouse, 5)]),
            no_overlay(range(4, 7)),
        ),
        (
            "Double click and triple click alternate on Windows",
            Windows,
            ABC,
            steps([&tap(5, Mouse, 3), &tap(5, Mouse, 4)]),
            no_overlay(range(4, 7)),
        ),
        (
            "Going past triple click retains the selection on Apple platforms",
            MacOs,
            ABC,
            steps([&tap(5, Mouse, 3), &tap(5, Mouse, 4)]),
            no_overlay(range(0, 11)),
        ),
        (
            "can shift + tap to select with a keyboard (non-Apple platforms)",
            Android,
            ATWATER,
            steps([
                &tap(13, Touch, 1),
                &shift_tap(20, Touch),
                &shift_tap(23, Touch),
                &shift_tap(4, Touch),
            ]),
            range(13, 4),
        ),
        (
            "can shift + tap to select with a keyboard (Apple platforms)",
            MacOs,
            ATWATER,
            steps([
                &tap(13, Touch, 1),
                &shift_tap(20, Touch),
                &shift_tap(23, Touch),
                &shift_tap(4, Touch),
            ]),
            range(23, 4),
        ),
        (
            "shift tapping an unfocused field (non-Apple)",
            Windows,
            unfocused_at(ATWATER.text, 35),
            shift_tap(20, Mouse).to_vec(),
            no_overlay(range(35, 20)),
        ),
        (
            "shift tapping an unfocused field (Apple platforms start from 0)",
            MacOs,
            unfocused_at(ATWATER.text, 35),
            shift_tap(20, Mouse).to_vec(),
            no_overlay(range(0, 20)),
        ),
        (
            "Tap shows handles but not toolbar",
            Android,
            ABC,
            tap(5, Touch, 1).to_vec(),
            caret(5),
        ),
        (
            "Tap in empty text field does not show handles nor toolbar",
            Android,
            Start { text: "", ..ABC },
            tap(0, Touch, 1).to_vec(),
            no_overlay(caret(0)),
        ),
        (
            "Long press in empty text field shows handles and toolbar",
            Android,
            Start { text: "", ..ABC },
            vec![long_press(0), Input::LongPressEnd],
            with_toolbar(caret(0)),
        ),
        (
            "Double tap in empty text field shows toolbar but not handles",
            Android,
            Start { text: "", ..ABC },
            steps([&tap(0, Touch, 1), &tap(0, Touch, 2)]),
            Expected {
                handles: 0,
                ..with_toolbar(caret(0))
            },
        ),
        (
            "Mouse tap does not show handles nor toolbar",
            Android,
            ABC,
            tap(5, Mouse, 1).to_vec(),
            no_overlay(caret(5)),
        ),
        (
            "Mouse double tap does not show handles nor toolbar",
            Android,
            ABC,
            steps([&tap(5, Mouse, 1), &tap(5, Mouse, 2)]),
            no_overlay(range(4, 7)),
        ),
        (
            "TextSelectionGestureDetectorBuilder right click non-Apple platforms: an unfocused field places the caret",
            Linux,
            Start {
                text: "one two three four five six seven",
                ..ABC
            },
            vec![Input::SecondaryTap {
                position: at(10),
                device: Mouse,
            }],
            Expected {
                handles: 0,
                ..with_toolbar(caret(10))
            },
        ),
        (
            "TextSelectionGestureDetectorBuilder right click non-Apple platforms: a focused selection stays",
            Windows,
            focused_at("one two three four five six seven", 20, 3),
            vec![Input::SecondaryTap {
                position: at(10),
                device: Mouse,
            }],
            Expected {
                handles: 0,
                ..with_toolbar(range(20, 3))
            },
        ),
        (
            "TextSelectionGestureDetectorBuilder right click Apple platforms: an unfocused field selects the word",
            MacOs,
            Start {
                text: "one two three four five six seven",
                ..ABC
            },
            vec![Input::SecondaryTap {
                position: at(10),
                device: Mouse,
            }],
            Expected {
                handles: 0,
                ..with_toolbar(range(8, 13))
            },
        ),
        (
            "TextSelectionGestureDetectorBuilder right click Apple platforms: a tap on the selection keeps it",
            MacOs,
            focused_at("one two three four five six seven", 3, 20),
            vec![Input::SecondaryTap {
                position: at(10),
                device: Mouse,
            }],
            Expected {
                handles: 0,
                ..with_toolbar(range(3, 20))
            },
        ),
        (
            "Can drag handles to show, unshow, and update magnifier: shown while dragging",
            Android,
            ABC,
            steps([
                &[long_press(5), Input::LongPressEnd],
                &drag_handle(Handle::End, 7, 11)[..2],
            ]),
            Expected {
                haptic: Some(Haptic::SelectionClick),
                ..with_magnifier(range(4, 11))
            },
        ),
        (
            "Entering text hides selection handle caret",
            Android,
            ABC,
            steps([&tap(5, Touch, 1), &[Input::Typed]]),
            no_overlay(caret(5)),
        ),
    ];
    for (name, target, start, inputs, expected) in rows {
        assert_eq!(run(target, &start, &inputs), expected, "{name}");
    }
}

#[test]
fn engine_is_not_used_on_ios() {
    let ios = behaviour(Some(Target::Ios)).expect("ios has a row");
    assert!(
        TextSelection::for_behaviour(ios).is_none(),
        "ios selects natively"
    );
}

#[test]
fn flutter_handle_geometry_tests_pass() {
    let layout = Mono("abc def ghi");
    let handle_size = parallax_platform::behaviour::kHandleSize;
    // (Flutter test or source, selection, expected (kind, rect origin) per handle)
    let rows = [
        (
            "only paints one collapsed handle: Material's getHandleAnchor collapsed (11, -4)",
            Selection::collapsed(5),
            vec![(HandleType::Collapsed, point(px(39.), px(24.)))],
        ),
        (
            "Android selection handles match endpoint direction: left anchored at its top right, right at its top left",
            Selection { base: 4, extent: 7 },
            vec![
                (HandleType::Left, point(px(18.), px(20.))),
                (HandleType::Right, point(px(70.), px(20.))),
            ],
        ),
    ];
    for (name, selection, expected) in rows {
        let handles = geometry::handles(&layout, selection, handle_size);
        let found: Vec<_> = handles
            .iter()
            .map(|handle| (handle.kind, handle.rect.origin))
            .collect();
        assert_eq!(found, expected, "{name}");
        for handle in handles {
            assert_eq!(handle.rect.size, size(handle_size, handle_size), "{name}");
            // selection handle's GestureDetector: kMinInteractiveDimension square around the handle, no larger.
            assert_eq!(handle.hit.size, size(px(48.), px(48.)), "{name}: hit area");
            assert_eq!(
                handle.hit.center(),
                handle.rect.center(),
                "{name}: hit area"
            );
        }
    }
}

#[test]
fn handle_dy_table() {
    // (Flutter test or rule, drag y, handle y, line height, expected)
    let rows = [
        (
            "less than a line: the handle stays",
            10.,
            21.,
            20.,
            Some(21.),
        ),
        (
            "a full line down: it jumps one line",
            45.,
            21.,
            20.,
            Some(41.),
        ),
        ("a full line up: it jumps one line", -1., 21., 20., Some(1.)),
        (
            "dragging selection handle does not crash when layout is degenerate (preferredLineHeight == 0)",
            50.,
            21.,
            0.,
            None,
        ),
        (
            "dragging selection handle does not crash when RenderObject is degenerate",
            f32::INFINITY,
            21.,
            20.,
            None,
        ),
    ];
    for (name, drag, handle, line, expected) in rows {
        assert_eq!(
            handle_dy(px(drag), px(handle), px(line)),
            expected.map(px),
            "{name}"
        );
    }
}

#[test]
fn toolbar_anchors_and_origin_table() {
    let layout = Mono("abc def ghi");
    let screen = Bounds::new(point(px(0.), px(0.)), size(px(400.), px(600.)));
    let toolbar = size(px(120.), px(44.));
    // (source, selection, field offset (moves the field down), expected anchors, expected toolbar origin)
    let rows = [
        (
            "TextSelectionToolbarAnchors.fromSelection: clamped into the field, so a toolbar at the top fits below",
            Selection { base: 4, extent: 7 },
            (point(px(55.), px(0.)), point(px(55.), px(20.))),
            point(px(8.), px(40.)),
        ),
        (
            "centerOn: a caret near the left edge puts the toolbar at the padding",
            Selection::collapsed(1),
            (point(px(10.), px(0.)), point(px(10.), px(20.))),
            point(px(8.), px(40.)),
        ),
    ];
    for (name, selection, (primary, secondary), origin) in rows {
        let anchors = toolbar_anchors(&layout, selection);
        assert_eq!(
            (anchors.primary, anchors.secondary),
            (primary, secondary),
            "{name}"
        );
        assert_eq!(
            toolbar_origin(anchors, toolbar, screen, px(0.)),
            origin,
            "{name}"
        );
    }
    let room_above = ToolbarAnchors {
        primary: point(px(200.), px(300.)),
        secondary: point(px(200.), px(320.)),
    };
    assert_eq!(
        toolbar_origin(room_above, toolbar, screen, px(0.)),
        point(px(140.), px(248.)),
        "TextSelectionToolbarLayoutDelegate: above the anchor, padded by _kToolbarContentDistance, when it fits"
    );
}

#[test]
fn flutter_magnifier_tests_pass() {
    // magnifier_test.dart's `reasonableTextField` and the test view (800 x 600).
    let field = Bounds::from_corners(point(px(50.), px(100.)), point(px(200.), px(100.)));
    let top_of_screen = Bounds::from_corners(point(px(0.), px(0.)), point(px(200.), px(0.)));
    let screen = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
    let info = |line: Bounds<Pixels>, field: Bounds<Pixels>, caret, gesture| MagnifierInfo {
        current_line_boundaries: line,
        field_bounds: field,
        caret_rect: caret,
        gesture_position: gesture,
    };
    let basic_offset = point(px(77.37 / 2.), px(22. + 37.9));
    let inflate = |rect: Bounds<Pixels>| rect.dilate(px(100.));
    // (Flutter test name, info, what the geometry must satisfy)
    type Check = fn(MagnifierGeometry, Point<Pixels>) -> bool;
    let rows: [(&str, MagnifierInfo, Check); 6] = [
        (
            "should never move outside the right bounds of the editing line",
            info(
                field,
                inflate(field),
                inflate(field),
                point(px(300.), px(0.)),
            ),
            |found, _| found.rect.left() <= px(200.),
        ),
        (
            "should never move outside the left bounds of the editing line",
            info(
                field,
                inflate(field),
                inflate(field),
                point(px(-50.), px(0.)),
            ),
            |found, basic| found.rect.left() + basic.x >= px(50.),
        ),
        (
            "should position vertically at the center of the line",
            info(field, field, field, field.center()),
            |found, basic| found.rect.top() == px(100.) - basic.y,
        ),
        (
            "should reposition vertically if mashed against the ceiling",
            info(
                top_of_screen,
                top_of_screen,
                top_of_screen,
                top_of_screen.top_center(),
            ),
            |found, _| found.rect.top() >= px(0.),
        ),
        (
            "should shift focal point so that the lens sees nothing out of bounds",
            info(field, field, field, field.origin),
            |found, _| found.focal_point_offset.x < px(50.) && found.focal_point_offset.x > px(0.),
        ),
        (
            "focal point should shift if mashed against the top to always point to text",
            info(
                top_of_screen,
                top_of_screen,
                top_of_screen,
                top_of_screen.top_center(),
            ),
            |found, _| found.focal_point_offset.y < px(0.),
        ),
    ];
    for (name, info, check) in rows {
        let found = text_magnifier(info, screen, None);
        assert!(check(found, basic_offset), "{name}: {found:?}");
        assert!(
            !found.jumped_line,
            "should not be animated on the initial state ({name})"
        );
    }
    let first = text_magnifier(info(field, field, field, field.center()), screen, None);
    let shifted = |dx: f32, dy: f32| {
        let shift =
            |rect: Bounds<Pixels>| Bounds::new(rect.origin + point(px(dx), px(dy)), rect.size);
        text_magnifier(
            info(
                shift(field),
                shift(field),
                shift(field),
                field.center() + point(px(dx), px(dy)),
            ),
            screen,
            Some(first),
        )
    };
    assert!(
        !shifted(200., 0.).jumped_line,
        "should not be animated on horizontal shifts"
    );
    assert!(
        shifted(0., 200.).jumped_line,
        "should be animated on vertical shifts"
    );
}

#[test]
fn shift_within_bounds_table() {
    // magnifier_test.dart's 'shift within bounds' group: (input, output), all in bounds 0, 0, 100, 100.
    let ltrb = |l: f32, t: f32, r: f32, b: f32| {
        Bounds::from_corners(point(px(l), px(t)), point(px(r), px(b)))
    };
    let bounds = ltrb(0., 0., 100., 100.);
    let rows = [
        (ltrb(-100., -100., -80., -80.), ltrb(0., 0., 20., 20.)),
        (ltrb(0., 0., 20., 20.), ltrb(0., 0., 20., 20.)),
        (ltrb(110., 0., 120., 10.), ltrb(90., 0., 100., 10.)),
        (ltrb(110., 110., 120., 120.), ltrb(90., 90., 100., 100.)),
    ];
    for (input, output) in rows {
        assert_eq!(
            shift_within_bounds(input, bounds),
            output,
            "should shift {input:?} to {output:?} for bounds {bounds:?}"
        );
    }
}
