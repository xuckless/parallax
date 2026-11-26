// Ported from flutter/flutter engine/src/flutter/lib/ui/window/pointer_data.h @ 3.50.1
// (ae9b586f34a8443573f82ef57c3bf914a908cbf7): the PointerData field layout and the Change and DeviceKind values.
// Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.

//! Flutter's pointer data packet → GPUI [`TouchEvent`]s. The ported `AndroidTouchProcessor` (Java) packs each
//! `MotionEvent` as Flutter's engine expects; this decodes it with Flutter's layout and names. Pure, so it is
//! tested on any host. How touch fits together: `docs/android-touch.md`.

use std::collections::HashMap;

use gpui::{Pixels, Point, PointerDevice, TouchEvent, TouchId, TouchPhase, point, px};

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

/// Packet value → device kind, indexed by Flutter's value. To follow Flutter: append new values in its order.
const POINTER_DEVICE_KINDS: [PointerDeviceKind; 6] = [
    PointerDeviceKind::Touch,
    PointerDeviceKind::Mouse,
    PointerDeviceKind::Stylus,
    PointerDeviceKind::InvertedStylus,
    PointerDeviceKind::Trackpad,
    PointerDeviceKind::Unknown,
];

/// Flutter's device kind → GPUI's `TouchEvent::device`. An inverted stylus is the eraser end of a pen; a trackpad
/// is an indirect pointer like a mouse; an unknown device is taken for a finger. To add a kind, add its row.
const DEVICES: [(PointerDeviceKind, PointerDevice); 6] = [
    (PointerDeviceKind::Touch, PointerDevice::Touch),
    (PointerDeviceKind::Mouse, PointerDevice::Mouse),
    (PointerDeviceKind::Stylus, PointerDevice::Pen),
    (PointerDeviceKind::InvertedStylus, PointerDevice::Pen),
    (PointerDeviceKind::Trackpad, PointerDevice::Mouse),
    (PointerDeviceKind::Unknown, PointerDevice::Touch),
];

/// What GPUI gets for each change; `None` sends nothing. Add and remove are Flutter's own bookkeeping (a remove
/// follows every touch up). Hover and trackpad pan/zoom have no `TouchEvent` meaning: they come with mouse support.
/// To route a change somewhere else, change its row.
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

/// The fields of one record that GPUI uses; Flutter's names. Positions are physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PointerData {
    pub(crate) change: PointerChange,
    pub(crate) kind: PointerDeviceKind,
    /// `(pointerId << 3) | toolType`: Android reuses pointer ids, so this names a pointer, not one touch.
    pub(crate) device: i64,
    pub(crate) physical_x: f64,
    pub(crate) physical_y: f64,
    pub(crate) pressure: f64,
    pub(crate) pressure_min: f64,
    pub(crate) pressure_max: f64,
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
            let change = long(Field::Change);
            let kind = long(Field::Kind);
            Ok(PointerData {
                change: usize::try_from(change)
                    .ok()
                    .and_then(|index| POINTER_CHANGES.get(index).copied())
                    .ok_or(Error::PointerChange(change))?,
                kind: usize::try_from(kind)
                    .ok()
                    .and_then(|index| POINTER_DEVICE_KINDS.get(index).copied())
                    .ok_or(Error::PointerKind(kind))?,
                device: long(Field::Device),
                physical_x: double(Field::PhysicalX),
                physical_y: double(Field::PhysicalY),
                pressure: double(Field::Pressure),
                pressure_min: double(Field::PressureMin),
                pressure_max: double(Field::PressureMax),
            })
        })
        .collect()
}

/// The touches in progress: Flutter's `device` → GPUI's [`TouchId`]. GPUI ids are never reused, Android's are.
#[derive(Debug, Default)]
pub(crate) struct Touches {
    next: u64,
    active: HashMap<i64, TouchId>,
    /// Where the last touch was, in logical pixels: GPUI's `mouse_position` on a touch screen.
    pub(crate) last_position: Point<Pixels>,
}

impl Touches {
    /// The GPUI events for one decoded packet. A move, up or cancel for a pointer that never went down is dropped.
    pub(crate) fn events(&mut self, packet: &[PointerData], density: f32) -> Vec<TouchEvent> {
        packet
            .iter()
            .filter_map(|data| {
                let phase = TOUCH_PHASES
                    .iter()
                    .find(|(change, _)| *change == data.change)
                    .and_then(|(_, phase)| *phase)?;
                let (_, device) = DEVICES.iter().find(|(kind, _)| *kind == data.kind)?;
                let id = match phase {
                    TouchPhase::Started => {
                        let id = TouchId(self.next);
                        // 2^64 touches never happen; wrapping keeps the counter panic-free.
                        self.next = self.next.wrapping_add(1);
                        self.active.insert(data.device, id);
                        id
                    }
                    TouchPhase::Moved => *self.active.get(&data.device)?,
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.active.remove(&data.device)?
                    }
                };
                let position = point(
                    px((data.physical_x / f64::from(density)) as f32),
                    px((data.physical_y / f64::from(density)) as f32),
                );
                self.last_position = position;
                let range = data.pressure_max - data.pressure_min;
                Some(TouchEvent {
                    id,
                    phase,
                    position,
                    predicted_position: None,
                    force: (range > 0.0).then(|| {
                        ((data.pressure - data.pressure_min) / range).clamp(0.0, 1.0) as f32
                    }),
                    device: *device,
                })
            })
            .collect()
    }
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

    // Flutter's values (AndroidTouchProcessor.PointerChange / PointerDeviceKind).
    const CANCEL: i64 = 0;
    const REMOVE: i64 = 2;
    const HOVER: i64 = 3;
    const DOWN: i64 = 4;
    const MOVE: i64 = 5;
    const UP: i64 = 6;
    const TOUCH: i64 = 0;
    const MOUSE: i64 = 1;
    const STYLUS: i64 = 2;
    // Device ids as uniquePointerIdByType makes them: (pointerId << 3) | TOOL_TYPE_FINGER (1).
    const FINGER_0: i64 = 1;
    const FINGER_1: i64 = (1 << 3) | 1;

    /// (id, phase, x, y) of one expected `TouchEvent`, in logical pixels at density 2.
    type Expected = (u64, TouchPhase, f32, f32);

    #[test]
    fn packets() {
        // (row name, packets sent in order, events of the last packet or its error).
        type Row = (&'static str, Vec<Vec<u8>>, Result<Vec<Expected>, Error>);
        let down = record(DOWN, TOUCH, FINGER_0, 20., 40.);
        let cases: Vec<Row> = vec![
            (
                "down_starts",
                vec![down.clone()],
                Ok(vec![(0, TouchPhase::Started, 10., 20.)]),
            ),
            (
                "move_moves",
                vec![down.clone(), record(MOVE, TOUCH, FINGER_0, 30., 40.)],
                Ok(vec![(0, TouchPhase::Moved, 15., 20.)]),
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
                Ok(vec![(0, TouchPhase::Ended, 15., 20.)]),
            ),
            (
                "cancel_cancels",
                vec![down.clone(), record(CANCEL, TOUCH, FINGER_0, 20., 40.)],
                Ok(vec![(0, TouchPhase::Cancelled, 10., 20.)]),
            ),
            (
                "next_touch_gets_new_id",
                vec![
                    down.clone(),
                    record(UP, TOUCH, FINGER_0, 20., 40.),
                    down.clone(),
                ],
                Ok(vec![(1, TouchPhase::Started, 10., 20.)]),
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
                Ok(vec![
                    (0, TouchPhase::Moved, 11., 20.),
                    (1, TouchPhase::Moved, 51., 50.),
                ]),
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
                Ok(vec![
                    (0, TouchPhase::Moved, 12., 20.),
                    (1, TouchPhase::Ended, 50., 50.),
                ]),
            ),
            (
                "move_without_down_dropped",
                vec![record(MOVE, TOUCH, FINGER_0, 20., 40.)],
                Ok(vec![]),
            ),
            (
                "hover_ignored",
                vec![record(HOVER, MOUSE, 2, 20., 40.)],
                Ok(vec![]),
            ),
            (
                "stylus_is_a_touch",
                vec![record(DOWN, STYLUS, 2, 20., 40.)],
                Ok(vec![(0, TouchPhase::Started, 10., 20.)]),
            ),
            (
                "mouse_is_a_touch",
                vec![record(DOWN, MOUSE, 3, 20., 40.)],
                Ok(vec![(0, TouchPhase::Started, 10., 20.)]),
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
            let mut touches = Touches::default();
            let mut last = Ok(vec![]);
            for packet in &packets {
                last = decode(packet).map(|data| touches.events(&data, 2.0));
            }
            let got = last.map(|events| {
                events
                    .iter()
                    .map(|event| {
                        (
                            event.id.0,
                            event.phase,
                            f32::from(event.position.x),
                            f32::from(event.position.y),
                        )
                    })
                    .collect::<Vec<_>>()
            });
            assert_eq!(format!("{got:?}"), format!("{expected:?}"), "{name}");
        }
    }

    #[test]
    fn devices() -> Result<(), Error> {
        // (row name, Flutter device kind, expected GPUI device).
        let cases = [
            ("finger", TOUCH, PointerDevice::Touch),
            ("mouse", MOUSE, PointerDevice::Mouse),
            ("stylus", STYLUS, PointerDevice::Pen),
            ("inverted_stylus", 3, PointerDevice::Pen),
            ("trackpad", 4, PointerDevice::Mouse),
            ("unknown", 5, PointerDevice::Touch),
        ];
        for (name, kind, device) in cases {
            let mut touches = Touches::default();
            let events = touches.events(&decode(&record(DOWN, kind, 2, 0., 0.))?, 2.0);
            assert_eq!(
                events.iter().map(|event| event.device).collect::<Vec<_>>(),
                [device],
                "{name}"
            );
        }
        Ok(())
    }

    #[test]
    fn force_and_last_position() -> Result<(), Error> {
        let mut no_range = record(DOWN, TOUCH, FINGER_1, 0., 0.);
        let max = Field::PressureMax as usize * BYTES_PER_FIELD;
        no_range[max..max + BYTES_PER_FIELD].copy_from_slice(&0f64.to_le_bytes());
        // (row name, packet, expected force, expected last position in logical pixels).
        let cases = [
            (
                "pressure_normalized",
                record(DOWN, TOUCH, FINGER_0, 20., 40.),
                Some(0.5),
                point(px(10.), px(20.)),
            ),
            (
                "no_pressure_range_no_force",
                no_range,
                None,
                point(px(0.), px(0.)),
            ),
        ];
        for (name, packet, force, position) in cases {
            let mut touches = Touches::default();
            let events = touches.events(&decode(&packet)?, 2.0);
            assert_eq!(
                events.first().and_then(|event| event.force),
                force,
                "{name}"
            );
            assert_eq!(touches.last_position, position, "{name}");
        }
        Ok(())
    }
}
