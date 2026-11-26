// Ported from flutter/flutter engine/src/flutter/lib/ui/window/pointer_data.h @ 3.50.1
// (ae9b586f34a8443573f82ef57c3bf914a908cbf7): the PointerData field layout and the Change, DeviceKind and SignalKind
// values. Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.

//! Flutter's pointer data packet → GPUI input. The ported `AndroidTouchProcessor` (Java) packs each `MotionEvent` as
//! Flutter's engine expects; this decodes it with Flutter's layout and names. Fingers and pens become
//! [`TouchEvent`]s, a mouse or trackpad becomes mouse and scroll events. Pure, so it is tested on any host. How
//! touch and mouse fit together: `docs/android-touch.md`, `docs/android-mouse.md`.

use std::collections::HashMap;

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent,
    NavigationDirection, Pixels, PlatformInput, Point, PointerDevice, ScrollDelta, ScrollEvent,
    ScrollSource, TouchEvent, TouchId, TouchPhase, point, px,
};

use crate::Error;

/// One pointer's record in packet order: Flutter's `PointerData` (`lib/ui/window/pointer_data.h`), written by
/// `AndroidTouchProcessor.addPointerForIndex`. Every field is 8 bytes, little-endian: `i64` or `f64`.
/// To follow a Flutter change: edit this list in Flutter's order; offsets come from the position.
#[derive(Clone, Copy)]
#[allow(dead_code, reason = "the full layout; only some fields are read")]
enum Field {
    /// `motionEventId` in AndroidTouchProcessor.
    EmbedderId,
    TimeStamp,
    Change,
    Kind,
    SignalKind,
    Device,
    PointerIdentifier,
    PhysicalX,
    PhysicalY,
    PhysicalDeltaX,
    PhysicalDeltaY,
    Buttons,
    Obscured,
    Synthesized,
    Pressure,
    PressureMin,
    PressureMax,
    Distance,
    DistanceMax,
    Size,
    RadiusMajor,
    RadiusMinor,
    RadiusMin,
    RadiusMax,
    Orientation,
    Tilt,
    PlatformData,
    ScrollDeltaX,
    ScrollDeltaY,
    PanX,
    PanY,
    PanDeltaX,
    PanDeltaY,
    Scale,
    Rotation,
    ViewId,
}

/// Must match `AndroidTouchProcessor.POINTER_DATA_FIELD_COUNT` (Flutter's `kPointerDataFieldCount`).
const POINTER_DATA_FIELD_COUNT: usize = Field::ViewId as usize + 1;
const BYTES_PER_FIELD: usize = 8;
const RECORD_BYTES: usize = POINTER_DATA_FIELD_COUNT * BYTES_PER_FIELD;
const _: () = assert!(POINTER_DATA_FIELD_COUNT == 36);

/// Flutter's `PointerChange` (`pointer.dart`), mirrored in `AndroidTouchProcessor.PointerChange`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PointerChange {
    Cancel,
    Add,
    Remove,
    Hover,
    Down,
    Move,
    Up,
    PanZoomStart,
    PanZoomUpdate,
    PanZoomEnd,
}

/// Flutter's `PointerDeviceKind` (`pointer.dart`), mirrored in `AndroidTouchProcessor.PointerDeviceKind`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PointerDeviceKind {
    Touch,
    Mouse,
    Stylus,
    InvertedStylus,
    Trackpad,
    Unknown,
}

/// Flutter's `PointerSignalKind` (`pointer.dart`), mirrored in `AndroidTouchProcessor.PointerSignalKind`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PointerSignalKind {
    None,
    Scroll,
    ScrollInertiaCancel,
    Scale,
    Unknown,
}

/// Packet value → change, indexed by Flutter's value. To follow Flutter: append new values in its order.
const POINTER_CHANGES: [PointerChange; 10] = [
    PointerChange::Cancel,
    PointerChange::Add,
    PointerChange::Remove,
    PointerChange::Hover,
    PointerChange::Down,
    PointerChange::Move,
    PointerChange::Up,
    PointerChange::PanZoomStart,
    PointerChange::PanZoomUpdate,
    PointerChange::PanZoomEnd,
];

/// Where a device kind's records go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Route {
    /// GPUI `TouchEvent`s tagged with this device; upstream's recognizer turns taps into clicks.
    Touch(PointerDevice),
    /// GPUI mouse and scroll events (`MOUSE_ACTIONS`), as on desktop: clicks are `ClickEvent::Mouse`.
    Mouse,
}

/// Packet value → Flutter's device kind and where its records go, indexed by Flutter's value. An inverted stylus is
/// the eraser end of a pen; a trackpad (Flutter's kind for a pan/zoom) is an indirect pointer like a mouse; an
/// unknown device is taken for a finger. To follow Flutter: append new values in its order; to reroute a kind,
/// change its row.
const DEVICE_KINDS: [(PointerDeviceKind, Route); 6] = [
    (PointerDeviceKind::Touch, Route::Touch(PointerDevice::Touch)),
    (PointerDeviceKind::Mouse, Route::Mouse),
    (PointerDeviceKind::Stylus, Route::Touch(PointerDevice::Pen)),
    (
        PointerDeviceKind::InvertedStylus,
        Route::Touch(PointerDevice::Pen),
    ),
    (PointerDeviceKind::Trackpad, Route::Mouse),
    (
        PointerDeviceKind::Unknown,
        Route::Touch(PointerDevice::Touch),
    ),
];

/// Packet value → Flutter's signal kind and the mouse action it forces, indexed by Flutter's value. A signal with
/// an action wins over the record's device and change: Android sends a wheel turn (`ACTION_SCROLL`) as a hover with
/// the scroll signal, from whatever tool. Android sends no other signal. To follow Flutter: append new values in its
/// order.
const SIGNAL_KINDS: [(PointerSignalKind, Option<MouseAction>); 5] = [
    (PointerSignalKind::None, None),
    (PointerSignalKind::Scroll, Some(MouseAction::Wheel)),
    (PointerSignalKind::ScrollInertiaCancel, None),
    (PointerSignalKind::Scale, None),
    (PointerSignalKind::Unknown, None),
];

/// What a touch or pen gets for each change; `None` sends nothing. Add and remove are Flutter's own bookkeeping (a
/// remove follows every touch up). A pen's hover has no `TouchEvent` meaning, and pan/zoom comes only from a
/// trackpad. To route a change somewhere else, change its row.
const TOUCH_PHASES: [(PointerChange, Option<TouchPhase>); 10] = [
    (PointerChange::Cancel, Some(TouchPhase::Cancelled)),
    (PointerChange::Add, None),
    (PointerChange::Remove, None),
    (PointerChange::Hover, None),
    (PointerChange::Down, Some(TouchPhase::Started)),
    (PointerChange::Move, Some(TouchPhase::Moved)),
    (PointerChange::Up, Some(TouchPhase::Ended)),
    (PointerChange::PanZoomStart, None),
    (PointerChange::PanZoomUpdate, None),
    (PointerChange::PanZoomEnd, None),
];

/// What a mouse or trackpad record becomes in GPUI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MouseAction {
    /// `MouseDown` for the newly pressed button, with its click count.
    Press,
    /// `MouseMove`, with the held button if any (a hover has none).
    Move,
    /// `MouseUp` for the released button.
    Release,
    /// `MouseExited`: Android took the gesture (cancel). GPUI has no mouse cancel, and a release would click.
    Leave,
    /// A `ScrollEvent` from a wheel, in Android's own scroll pixels (`ViewConfiguration`'s scroll factor).
    Wheel,
    /// A `ScrollEvent` in this phase from a trackpad pan (Flutter's pan/zoom): the change in pan offset.
    Pan(TouchPhase),
    Nothing,
}

/// What a mouse or trackpad gets for each change. A hover is a move with no button held (Android's hover moves come
/// through `onGenericMotionEvent`; hover enter and exit are not sent). To route a change somewhere else, change its
/// row.
const MOUSE_ACTIONS: [(PointerChange, MouseAction); 10] = [
    (PointerChange::Cancel, MouseAction::Leave),
    (PointerChange::Add, MouseAction::Nothing),
    (PointerChange::Remove, MouseAction::Nothing),
    (PointerChange::Hover, MouseAction::Move),
    (PointerChange::Down, MouseAction::Press),
    (PointerChange::Move, MouseAction::Move),
    (PointerChange::Up, MouseAction::Release),
    (
        PointerChange::PanZoomStart,
        MouseAction::Pan(TouchPhase::Started),
    ),
    (
        PointerChange::PanZoomUpdate,
        MouseAction::Pan(TouchPhase::Moved),
    ),
    (
        PointerChange::PanZoomEnd,
        MouseAction::Pan(TouchPhase::Ended),
    ),
];

/// Flutter's button bits (`kPrimaryButton`, `kSecondaryButton`, ... in `events.dart`; AndroidTouchProcessor masks
/// Android's `MotionEvent.BUTTON_*` to these five) → GPUI's button, in priority order. To add a button: a row.
const BUTTONS: [(i64, MouseButton); 5] = [
    (1, MouseButton::Left),
    (2, MouseButton::Right),
    (4, MouseButton::Middle),
    (8, MouseButton::Navigate(NavigationDirection::Back)),
    (16, MouseButton::Navigate(NavigationDirection::Forward)),
];

/// The first button in `bits`, in `BUTTONS` order.
fn button(bits: i64) -> Option<MouseButton> {
    BUTTONS
        .iter()
        .find(|(bit, _)| bits & bit != 0)
        .map(|(_, button)| *button)
}

/// A press of the same button is the next click within Android's double-tap timeout
/// (`ViewConfiguration.getDoubleTapTimeout`) and gpui_web's 5-pixel radius (Android's double-tap slop is sized for
/// fingers). Microseconds, as the packet's time stamps; logical pixels.
const DOUBLE_CLICK_MICROS: i64 = 300_000;
const DOUBLE_CLICK_SLOP: f32 = 5.0;

/// The fields of one record that GPUI uses; Flutter's names. Positions are physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PointerData {
    pub(crate) change: PointerChange,
    /// Flutter's `kind`, as where it goes (`DEVICE_KINDS`).
    pub(crate) route: Route,
    /// Flutter's `signal_kind`, as the action it forces (`SIGNAL_KINDS`).
    pub(crate) signal: Option<MouseAction>,
    /// `(pointerId << 3) | toolType`: Android reuses pointer ids, so this names a pointer, not one touch.
    pub(crate) device: i64,
    /// Microseconds (`MotionEvent.getEventTime() * 1000`).
    pub(crate) time_stamp: i64,
    pub(crate) physical_x: f64,
    pub(crate) physical_y: f64,
    /// Flutter's button bits (`BUTTONS`); 0 for a finger.
    pub(crate) buttons: i64,
    pub(crate) pressure: f64,
    pub(crate) pressure_min: f64,
    pub(crate) pressure_max: f64,
    /// How far to scroll, Flutter's sign (positive scrolls down and right).
    pub(crate) scroll_delta: Point<f64>,
    /// A trackpad pan's offset from where it started.
    pub(crate) pan: Point<f64>,
}

/// Decodes a whole packet; one bad record rejects it all.
pub(crate) fn decode(packet: &[u8]) -> Result<Vec<PointerData>, Error> {
    let (records, rest) = packet.as_chunks::<RECORD_BYTES>();
    if !rest.is_empty() {
        return Err(Error::PacketLength(packet.len()));
    }
    records
        .iter()
        .map(|record| {
            // A record is exactly POINTER_DATA_FIELD_COUNT fields, so every `Field` indexes inside it.
            let (fields, _) = record.as_chunks::<BYTES_PER_FIELD>();
            let long = |field: Field| i64::from_le_bytes(fields[field as usize]);
            let double = |field: Field| f64::from_le_bytes(fields[field as usize]);
            // A Flutter enum value as an index into its table.
            let index = |field: Field| usize::try_from(long(field)).ok();
            Ok(PointerData {
                change: index(Field::Change)
                    .and_then(|index| POINTER_CHANGES.get(index).copied())
                    .ok_or(Error::PointerChange(long(Field::Change)))?,
                route: index(Field::Kind)
                    .and_then(|index| DEVICE_KINDS.get(index))
                    .map(|(_, route)| *route)
                    .ok_or(Error::PointerKind(long(Field::Kind)))?,
                signal: index(Field::SignalKind)
                    .and_then(|index| SIGNAL_KINDS.get(index))
                    .map(|(_, action)| *action)
                    .ok_or(Error::PointerSignal(long(Field::SignalKind)))?,
                device: long(Field::Device),
                time_stamp: long(Field::TimeStamp),
                physical_x: double(Field::PhysicalX),
                physical_y: double(Field::PhysicalY),
                buttons: long(Field::Buttons),
                pressure: double(Field::Pressure),
                pressure_min: double(Field::PressureMin),
                pressure_max: double(Field::PressureMax),
                scroll_delta: Point::new(double(Field::ScrollDeltaX), double(Field::ScrollDeltaY)),
                pan: Point::new(double(Field::PanX), double(Field::PanY)),
            })
        })
        .collect()
}

/// The last mouse press, for click counting.
#[derive(Clone, Copy, Debug)]
struct Press {
    button: MouseButton,
    time_stamp: i64,
    position: Point<Pixels>,
    click_count: usize,
}

/// The mouse (one cursor, however many mice) and trackpad, as the last records left them.
#[derive(Debug, Default)]
struct Mouse {
    /// Flutter's button bits held, as the last record said.
    buttons: i64,
    /// `None` before the first press.
    last_press: Option<Press>,
    /// The trackpad pan in progress: its offset at the last record, in logical pixels. `None` between pans.
    pan: Option<Point<Pixels>>,
}

/// The pointers in progress. Touches: Flutter's `device` → GPUI's [`TouchId`]; GPUI ids are never reused, Android's
/// are.
#[derive(Debug, Default)]
pub(crate) struct Pointers {
    next_touch: u64,
    touches: HashMap<i64, TouchId>,
    mouse: Mouse,
    /// Where the last pointer event was, in logical pixels: GPUI's `mouse_position`.
    pub(crate) last_position: Point<Pixels>,
}

impl Pointers {
    /// The GPUI input for one decoded packet, with the keyboard's `modifiers`. A touch move, up or cancel for a
    /// pointer that never went down is dropped.
    pub(crate) fn events(
        &mut self,
        packet: &[PointerData],
        density: f32,
        modifiers: Modifiers,
    ) -> Vec<PlatformInput> {
        let logical = |physical: Point<f64>| {
            point(
                px((physical.x / f64::from(density)) as f32),
                px((physical.y / f64::from(density)) as f32),
            )
        };
        packet
            .iter()
            .filter_map(|data| {
                let position = logical(Point::new(data.physical_x, data.physical_y));
                let mouse_action = |change: PointerChange| {
                    MOUSE_ACTIONS
                        .iter()
                        .find(|(row, _)| *row == change)
                        .map_or(MouseAction::Nothing, |(_, action)| *action)
                };
                let input = match (data.signal, data.route) {
                    (Some(action), _) => self.mouse(action, data, position, logical, modifiers),
                    (None, Route::Mouse) => self.mouse(
                        mouse_action(data.change),
                        data,
                        position,
                        logical,
                        modifiers,
                    ),
                    (None, Route::Touch(device)) => self.touch(data, device, position),
                }?;
                self.last_position = position;
                Some(input)
            })
            .collect()
    }

    fn touch(
        &mut self,
        data: &PointerData,
        device: PointerDevice,
        position: Point<Pixels>,
    ) -> Option<PlatformInput> {
        let phase = TOUCH_PHASES
            .iter()
            .find(|(change, _)| *change == data.change)
            .and_then(|(_, phase)| *phase)?;
        let id = match phase {
            TouchPhase::Started => {
                let id = TouchId(self.next_touch);
                // 2^64 touches never happen; wrapping keeps the counter panic-free.
                self.next_touch = self.next_touch.wrapping_add(1);
                self.touches.insert(data.device, id);
                id
            }
            TouchPhase::Moved => *self.touches.get(&data.device)?,
            TouchPhase::Ended | TouchPhase::Cancelled => self.touches.remove(&data.device)?,
        };
        let range = data.pressure_max - data.pressure_min;
        Some(PlatformInput::Touch(TouchEvent {
            id,
            phase,
            position,
            predicted_position: None,
            force: (range > 0.0)
                .then(|| ((data.pressure - data.pressure_min) / range).clamp(0.0, 1.0) as f32),
            device,
        }))
    }

    /// One mouse or trackpad record; `logical` converts the record's physical pixels.
    fn mouse(
        &mut self,
        action: MouseAction,
        data: &PointerData,
        position: Point<Pixels>,
        logical: impl Fn(Point<f64>) -> Point<Pixels>,
        modifiers: Modifiers,
    ) -> Option<PlatformInput> {
        let mouse = &mut self.mouse;
        let held = std::mem::replace(&mut mouse.buttons, data.buttons);
        Some(match action {
            MouseAction::Press => {
                // A press reporting no new button (some injected events) is the primary one: GPUI needs a button.
                let button = button(data.buttons & !held).unwrap_or(MouseButton::Left);
                let click_count = match mouse.last_press {
                    Some(last)
                        if last.button == button
                            && data.time_stamp - last.time_stamp <= DOUBLE_CLICK_MICROS
                            && distance(last.position, position) <= DOUBLE_CLICK_SLOP =>
                    {
                        last.click_count + 1
                    }
                    Some(_) | None => 1,
                };
                mouse.last_press = Some(Press {
                    button,
                    time_stamp: data.time_stamp,
                    position,
                    click_count,
                });
                PlatformInput::MouseDown(MouseDownEvent {
                    button,
                    position,
                    modifiers,
                    click_count,
                    first_mouse: false,
                })
            }
            MouseAction::Move => PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: button(data.buttons),
                modifiers,
            }),
            MouseAction::Release => PlatformInput::MouseUp(MouseUpEvent {
                // Android's up carries the buttons still held, usually none: the released one is the one that went.
                button: button(held & !data.buttons)
                    .or_else(|| button(held))
                    .unwrap_or(MouseButton::Left),
                position,
                modifiers,
                click_count: mouse.last_press.map_or(1, |press| press.click_count),
            }),
            MouseAction::Leave => {
                mouse.buttons = 0;
                PlatformInput::MouseExited(MouseExitEvent {
                    position,
                    pressed_button: None,
                    modifiers,
                })
            }
            // Flutter's delta is how far to scroll; GPUI's is how far the content moves (gpui_web negates the DOM's).
            MouseAction::Wheel => {
                let scroll = logical(data.scroll_delta);
                PlatformInput::Scroll(ScrollEvent {
                    position,
                    delta: ScrollDelta::Pixels(point(-scroll.x, -scroll.y)),
                    modifiers,
                    touch_phase: TouchPhase::Moved,
                    source: ScrollSource::Pointer,
                })
            }
            // The content follows the fingers, as in a touch pan; `position` is where the pan started.
            MouseAction::Pan(phase) => {
                let pan = logical(data.pan);
                let last = mouse.pan.unwrap_or_default();
                mouse.pan = match phase {
                    TouchPhase::Started | TouchPhase::Moved => Some(pan),
                    TouchPhase::Ended | TouchPhase::Cancelled => None,
                };
                PlatformInput::Scroll(ScrollEvent {
                    position,
                    delta: ScrollDelta::Pixels(point(pan.x - last.x, pan.y - last.y)),
                    modifiers,
                    touch_phase: phase,
                    source: ScrollSource::Pointer,
                })
            }
            MouseAction::Nothing => return None,
        })
    }
}

fn distance(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    f32::from(a.x - b.x).hypot(f32::from(a.y - b.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One record as `AndroidTouchProcessor.addPointerForIndex` writes it (pressure range 0..1, other fields 0).
    fn record(change: i64, kind: i64, device: i64, x: f64, y: f64) -> Vec<u8> {
        let mut fields = [[0u8; BYTES_PER_FIELD]; POINTER_DATA_FIELD_COUNT];
        fields[Field::Change as usize] = change.to_le_bytes();
        fields[Field::Kind as usize] = kind.to_le_bytes();
        fields[Field::Device as usize] = device.to_le_bytes();
        fields[Field::PhysicalX as usize] = x.to_le_bytes();
        fields[Field::PhysicalY as usize] = y.to_le_bytes();
        fields[Field::Pressure as usize] = 0.5f64.to_le_bytes();
        fields[Field::PressureMax as usize] = 1.0f64.to_le_bytes();
        fields.concat()
    }

    /// `record` with more fields set: (field, little-endian value), e.g. `(Field::Buttons, 1i64.to_le_bytes())`.
    fn with(mut record: Vec<u8>, fields: &[(Field, [u8; BYTES_PER_FIELD])]) -> Vec<u8> {
        for (field, value) in fields {
            let at = *field as usize * BYTES_PER_FIELD;
            record[at..at + BYTES_PER_FIELD].copy_from_slice(value);
        }
        record
    }

    // Flutter's values (AndroidTouchProcessor.PointerChange / PointerDeviceKind / PointerSignalKind).
    const CANCEL: i64 = 0;
    const ADD: i64 = 1;
    const REMOVE: i64 = 2;
    const HOVER: i64 = 3;
    const DOWN: i64 = 4;
    const MOVE: i64 = 5;
    const UP: i64 = 6;
    const PAN_ZOOM_START: i64 = 7;
    const PAN_ZOOM_UPDATE: i64 = 8;
    const PAN_ZOOM_END: i64 = 9;
    const TOUCH: i64 = 0;
    const MOUSE: i64 = 1;
    const STYLUS: i64 = 2;
    const INVERTED_STYLUS: i64 = 3;
    const TRACKPAD: i64 = 4;
    const UNKNOWN: i64 = 5;
    const SCROLL: i64 = 1;
    // Device ids as uniquePointerIdByType makes them: (pointerId << 3) | toolType (finger 1, mouse 3).
    const FINGER_0: i64 = 1;
    const FINGER_1: i64 = (1 << 3) | 1;
    const MOUSE_0: i64 = 3;

    /// One GPUI input in short, positions in logical pixels at density 2. Exhaustive for what pointers send.
    fn describe(input: &PlatformInput) -> String {
        let at = |position: Point<Pixels>| {
            format!("({}, {})", f32::from(position.x), f32::from(position.y))
        };
        match input {
            PlatformInput::Touch(event) => {
                format!(
                    "touch {} {:?} {}",
                    event.id.0,
                    event.phase,
                    at(event.position)
                )
            }
            PlatformInput::MouseDown(event) => format!(
                "down {:?} x{} {}",
                event.button,
                event.click_count,
                at(event.position)
            ),
            PlatformInput::MouseUp(event) => format!(
                "up {:?} x{} {}",
                event.button,
                event.click_count,
                at(event.position)
            ),
            PlatformInput::MouseMove(event) => {
                format!("move {:?} {}", event.pressed_button, at(event.position))
            }
            PlatformInput::MouseExited(event) => format!("exit {}", at(event.position)),
            PlatformInput::Scroll(event) => {
                let delta = event.delta.pixel_delta(px(1.));
                format!(
                    "scroll {:?} ({}, {}) {}",
                    event.touch_phase,
                    f32::from(delta.x),
                    f32::from(delta.y),
                    at(event.position)
                )
            }
            other => format!("unexpected {other:?}"),
        }
    }

    /// Sends `packets` in order; the last packet's inputs, described, or its error.
    fn run(packets: &[Vec<u8>]) -> Result<Vec<String>, Error> {
        let mut pointers = Pointers::default();
        let mut last = Ok(vec![]);
        for packet in packets {
            last = decode(packet).map(|data| {
                pointers
                    .events(&data, 2.0, Modifiers::default())
                    .iter()
                    .map(describe)
                    .collect()
            });
        }
        last
    }

    #[test]
    fn touches() {
        // (row name, packets sent in order, inputs of the last packet or its error).
        type Row = (&'static str, Vec<Vec<u8>>, Result<Vec<&'static str>, Error>);
        let down = record(DOWN, TOUCH, FINGER_0, 20., 40.);
        let cases: Vec<Row> = vec![
            (
                "down_starts",
                vec![down.clone()],
                Ok(vec!["touch 0 Started (10, 20)"]),
            ),
            (
                "move_moves",
                vec![down.clone(), record(MOVE, TOUCH, FINGER_0, 30., 40.)],
                Ok(vec!["touch 0 Moved (15, 20)"]),
            ),
            (
                // ACTION_UP: up, then the remove Flutter synthesizes for touch.
                "up_ends_remove_ignored",
                vec![
                    down.clone(),
                    [
                        record(UP, TOUCH, FINGER_0, 30., 40.),
                        record(REMOVE, TOUCH, FINGER_0, 30., 40.),
                    ]
                    .concat(),
                ],
                Ok(vec!["touch 0 Ended (15, 20)"]),
            ),
            (
                "cancel_cancels",
                vec![down.clone(), record(CANCEL, TOUCH, FINGER_0, 20., 40.)],
                Ok(vec!["touch 0 Cancelled (10, 20)"]),
            ),
            (
                "next_touch_gets_new_id",
                vec![
                    down.clone(),
                    record(UP, TOUCH, FINGER_0, 20., 40.),
                    down.clone(),
                ],
                Ok(vec!["touch 1 Started (10, 20)"]),
            ),
            (
                // ACTION_MOVE carries every pointer; each keeps its own id.
                "multi_pointer_move",
                vec![
                    down.clone(),
                    record(DOWN, TOUCH, FINGER_1, 100., 100.),
                    [
                        record(MOVE, TOUCH, FINGER_0, 22., 40.),
                        record(MOVE, TOUCH, FINGER_1, 102., 100.),
                    ]
                    .concat(),
                ],
                Ok(vec!["touch 0 Moved (11, 20)", "touch 1 Moved (51, 50)"]),
            ),
            (
                // ACTION_POINTER_UP: batched moves for the others, then the up, then the remove.
                "multi_pointer_up",
                vec![
                    down.clone(),
                    record(DOWN, TOUCH, FINGER_1, 100., 100.),
                    [
                        record(MOVE, TOUCH, FINGER_0, 24., 40.),
                        record(UP, TOUCH, FINGER_1, 100., 100.),
                        record(REMOVE, TOUCH, FINGER_1, 100., 100.),
                    ]
                    .concat(),
                ],
                Ok(vec!["touch 0 Moved (12, 20)", "touch 1 Ended (50, 50)"]),
            ),
            (
                "move_without_down_dropped",
                vec![record(MOVE, TOUCH, FINGER_0, 20., 40.)],
                Ok(vec![]),
            ),
            (
                "pen_hover_ignored",
                vec![record(HOVER, STYLUS, 2, 20., 40.)],
                Ok(vec![]),
            ),
            (
                "stylus_is_a_touch",
                vec![record(DOWN, STYLUS, 2, 20., 40.)],
                Ok(vec!["touch 0 Started (10, 20)"]),
            ),
            ("empty_packet", vec![vec![]], Ok(vec![])),
            (
                "unknown_kind",
                vec![record(DOWN, 6, FINGER_0, 0., 0.)],
                Err(Error::PointerKind(6)),
            ),
            (
                "unknown_change",
                vec![record(10, TOUCH, FINGER_0, 0., 0.)],
                Err(Error::PointerChange(10)),
            ),
            (
                "negative_change",
                vec![record(-1, TOUCH, FINGER_0, 0., 0.)],
                Err(Error::PointerChange(-1)),
            ),
            (
                "unknown_signal",
                vec![with(
                    down.clone(),
                    &[(Field::SignalKind, 5i64.to_le_bytes())],
                )],
                Err(Error::PointerSignal(5)),
            ),
            (
                "short_packet",
                vec![down[..RECORD_BYTES - 8].to_vec()],
                Err(Error::PacketLength(RECORD_BYTES - 8)),
            ),
            (
                "trailing_bytes",
                vec![[down.clone(), vec![0; 8]].concat()],
                Err(Error::PacketLength(RECORD_BYTES + 8)),
            ),
        ];
        for (name, packets, expected) in cases {
            let expected =
                expected.map(|inputs| inputs.into_iter().map(String::from).collect::<Vec<_>>());
            // `Error` has no `PartialEq` (it wraps foreign errors); its `Debug` names the variant and value.
            assert_eq!(
                format!("{:?}", run(&packets)),
                format!("{expected:?}"),
                "{name}"
            );
        }
    }

    #[test]
    fn mouse() {
        // A mouse record at a time (ms) with Flutter's button bits.
        let at = |change: i64, ms: i64, x: f64, buttons: i64| {
            with(
                record(change, MOUSE, MOUSE_0, x, 40.),
                &[
                    (Field::TimeStamp, (ms * 1000).to_le_bytes()),
                    (Field::Buttons, buttons.to_le_bytes()),
                ],
            )
        };
        // ACTION_SCROLL as AndroidTouchProcessor packs it: a hover with the scroll signal and Flutter's deltas.
        let wheel = |kind: i64, dx: f64, dy: f64| {
            with(
                record(HOVER, kind, MOUSE_0, 20., 40.),
                &[
                    (Field::SignalKind, SCROLL.to_le_bytes()),
                    (Field::ScrollDeltaX, dx.to_le_bytes()),
                    (Field::ScrollDeltaY, dy.to_le_bytes()),
                ],
            )
        };
        // A trackpad pan record: at the pan's start (20, 40), offset (x, y) from it.
        let pan = |change: i64, x: f64, y: f64| {
            with(
                record(change, TRACKPAD, MOUSE_0, 20., 40.),
                &[
                    (Field::PanX, x.to_le_bytes()),
                    (Field::PanY, y.to_le_bytes()),
                ],
            )
        };
        // (row name, packets sent in order, inputs of the last packet).
        type Row = (&'static str, Vec<Vec<u8>>, Vec<&'static str>);
        let cases: Vec<Row> = vec![
            (
                "press_is_a_mouse_down",
                vec![at(DOWN, 0, 20., 1)],
                vec!["down Left x1 (10, 20)"],
            ),
            (
                "release_is_a_mouse_up",
                vec![at(DOWN, 0, 20., 1), at(UP, 50, 20., 0)],
                vec!["up Left x1 (10, 20)"],
            ),
            (
                "drag_moves_with_the_button",
                vec![at(DOWN, 0, 20., 1), at(MOVE, 20, 30., 1)],
                vec!["move Some(Left) (15, 20)"],
            ),
            (
                "hover_moves_without_a_button",
                vec![at(HOVER, 0, 30., 0)],
                vec!["move None (15, 20)"],
            ),
            (
                "secondary_button",
                vec![at(DOWN, 0, 20., 2)],
                vec!["down Right x1 (10, 20)"],
            ),
            (
                "secondary_release",
                vec![at(DOWN, 0, 20., 2), at(UP, 50, 20., 0)],
                vec!["up Right x1 (10, 20)"],
            ),
            (
                "middle_button",
                vec![at(DOWN, 0, 20., 4)],
                vec!["down Middle x1 (10, 20)"],
            ),
            (
                "back_button",
                vec![at(DOWN, 0, 20., 8)],
                vec!["down Navigate(Back) x1 (10, 20)"],
            ),
            (
                "forward_button",
                vec![at(DOWN, 0, 20., 16)],
                vec!["down Navigate(Forward) x1 (10, 20)"],
            ),
            (
                // Injected events (`input mouse tap` on some images) report no button.
                "press_without_buttons_is_primary",
                vec![at(DOWN, 0, 20., 0)],
                vec!["down Left x1 (10, 20)"],
            ),
            (
                "second_press_soon_is_a_double_click",
                vec![
                    at(DOWN, 0, 20., 1),
                    at(UP, 50, 20., 0),
                    at(DOWN, 200, 24., 1),
                ],
                vec!["down Left x2 (12, 20)"],
            ),
            (
                "double_click_release_counts_two",
                vec![
                    at(DOWN, 0, 20., 1),
                    at(UP, 50, 20., 0),
                    at(DOWN, 200, 20., 1),
                    at(UP, 250, 20., 0),
                ],
                vec!["up Left x2 (10, 20)"],
            ),
            (
                "third_press_is_a_triple_click",
                vec![
                    at(DOWN, 0, 20., 1),
                    at(UP, 50, 20., 0),
                    at(DOWN, 200, 20., 1),
                    at(UP, 250, 20., 0),
                    at(DOWN, 400, 20., 1),
                ],
                vec!["down Left x3 (10, 20)"],
            ),
            (
                "press_after_the_timeout_counts_one",
                vec![
                    at(DOWN, 0, 20., 1),
                    at(UP, 50, 20., 0),
                    at(DOWN, 400, 20., 1),
                ],
                vec!["down Left x1 (10, 20)"],
            ),
            (
                "press_far_away_counts_one",
                vec![
                    at(DOWN, 0, 20., 1),
                    at(UP, 50, 20., 0),
                    at(DOWN, 100, 40., 1),
                ],
                vec!["down Left x1 (20, 20)"],
            ),
            (
                "other_button_counts_one",
                vec![
                    at(DOWN, 0, 20., 1),
                    at(UP, 50, 20., 0),
                    at(DOWN, 100, 20., 2),
                ],
                vec!["down Right x1 (10, 20)"],
            ),
            (
                "cancel_leaves",
                vec![at(DOWN, 0, 20., 1), at(CANCEL, 50, 20., 1)],
                vec!["exit (10, 20)"],
            ),
            ("add_sends_nothing", vec![at(ADD, 0, 20., 0)], vec![]),
            ("remove_sends_nothing", vec![at(REMOVE, 0, 20., 0)], vec![]),
            (
                // Flutter's +y scrolls down; GPUI's content moves up: -y.
                "wheel_scrolls_down",
                vec![wheel(MOUSE, 0., 128.)],
                vec!["scroll Moved (-0, -64) (10, 20)"],
            ),
            (
                "wheel_scrolls_left",
                vec![wheel(MOUSE, -96., 0.)],
                vec!["scroll Moved (48, -0) (10, 20)"],
            ),
            (
                "wheel_from_a_finger_tool_scrolls",
                vec![wheel(TOUCH, 0., 128.)],
                vec!["scroll Moved (-0, -64) (10, 20)"],
            ),
            (
                "pan_starts",
                vec![pan(PAN_ZOOM_START, 0., 0.)],
                vec!["scroll Started (0, 0) (10, 20)"],
            ),
            (
                // The content follows the fingers; the delta is the change since the last record.
                "pan_updates_by_the_change",
                vec![
                    pan(PAN_ZOOM_START, 0., 0.),
                    pan(PAN_ZOOM_UPDATE, 0., 20.),
                    pan(PAN_ZOOM_UPDATE, 0., 60.),
                ],
                vec!["scroll Moved (0, 20) (10, 20)"],
            ),
            (
                "pan_ends",
                vec![
                    pan(PAN_ZOOM_START, 0., 0.),
                    pan(PAN_ZOOM_UPDATE, 10., 20.),
                    pan(PAN_ZOOM_END, 10., 20.),
                ],
                vec!["scroll Ended (0, 0) (10, 20)"],
            ),
            (
                "next_pan_starts_from_zero",
                vec![
                    pan(PAN_ZOOM_START, 0., 0.),
                    pan(PAN_ZOOM_UPDATE, 0., 20.),
                    pan(PAN_ZOOM_END, 0., 20.),
                    pan(PAN_ZOOM_START, 0., 0.),
                    pan(PAN_ZOOM_UPDATE, 0., 8.),
                ],
                vec!["scroll Moved (0, 4) (10, 20)"],
            ),
            (
                "mouse_move_is_not_a_touch",
                vec![at(DOWN, 0, 20., 1), record(MOVE, TOUCH, FINGER_0, 20., 40.)],
                vec![],
            ),
        ];
        for (name, packets, expected) in cases {
            let got = run(&packets).map_err(|error| error.to_string());
            assert_eq!(
                got,
                Ok(expected.into_iter().map(String::from).collect::<Vec<_>>()),
                "{name}"
            );
        }
    }

    #[test]
    fn devices() -> Result<(), Error> {
        // (row name, Flutter device kind, expected route).
        let cases = [
            ("finger", TOUCH, Route::Touch(PointerDevice::Touch)),
            ("mouse", MOUSE, Route::Mouse),
            ("stylus", STYLUS, Route::Touch(PointerDevice::Pen)),
            (
                "inverted_stylus",
                INVERTED_STYLUS,
                Route::Touch(PointerDevice::Pen),
            ),
            ("trackpad", TRACKPAD, Route::Mouse),
            ("unknown", UNKNOWN, Route::Touch(PointerDevice::Touch)),
        ];
        for (name, kind, route) in cases {
            let data = decode(&record(DOWN, kind, 2, 0., 0.))?;
            assert_eq!(
                data.iter().map(|data| data.route).collect::<Vec<_>>(),
                [route],
                "{name}"
            );
        }
        Ok(())
    }

    #[test]
    fn touch_device_force_and_last_position() -> Result<(), Error> {
        let mut no_range = record(DOWN, TOUCH, FINGER_1, 0., 0.);
        let max = Field::PressureMax as usize * BYTES_PER_FIELD;
        no_range[max..max + BYTES_PER_FIELD].copy_from_slice(&0f64.to_le_bytes());
        // (row name, packet, expected device, force and last position in logical pixels).
        let cases = [
            (
                "pressure_normalized",
                record(DOWN, TOUCH, FINGER_0, 20., 40.),
                Some((PointerDevice::Touch, Some(0.5))),
                point(px(10.), px(20.)),
            ),
            (
                "no_pressure_range_no_force",
                no_range,
                Some((PointerDevice::Touch, None)),
                point(px(0.), px(0.)),
            ),
            (
                "pen_is_tagged",
                record(DOWN, INVERTED_STYLUS, 4, 20., 40.),
                Some((PointerDevice::Pen, Some(0.5))),
                point(px(10.), px(20.)),
            ),
            (
                "mouse_moves_the_position",
                record(HOVER, MOUSE, MOUSE_0, 60., 80.),
                None,
                point(px(30.), px(40.)),
            ),
        ];
        for (name, packet, touch, position) in cases {
            let mut pointers = Pointers::default();
            let inputs = pointers.events(&decode(&packet)?, 2.0, Modifiers::default());
            let got = inputs.first().and_then(|input| match input {
                PlatformInput::Touch(event) => Some((event.device, event.force)),
                _ => None,
            });
            assert_eq!(got, touch, "{name}");
            assert_eq!(pointers.last_position, position, "{name}");
        }
        Ok(())
    }
}
