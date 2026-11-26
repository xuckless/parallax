// Ported from flutter/flutter engine/src/flutter/lib/ui/window/key_data.h @ 3.50.1
// (ae9b586f34a8443573f82ef57c3bf914a908cbf7): the KeyData field layout and the KeyEventType values. Logical key
// values from engine/src/flutter/shell/platform/android/io/flutter/embedding/android/KeyboardMap.java.
// Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.

//! Flutter's key data packet → GPUI key, modifier and back events. The ported `KeyEmbedderResponder` (Java) turns
//! each Android `KeyEvent` into Flutter's key data (synthesizing modifier syncs); this decodes it and decides what
//! GPUI gets. Pure, so it is tested on any host. How keys fit together: `docs/android-keys.md`.

use gpui::{
    BackGestureEvent, BackGesturePhase, Capslock, DispatchEventResult, KeyDownEvent, KeyUpEvent,
    Keystroke, Modifiers, ModifiersChangedEvent, Pixels, PlatformInput, Point,
};

use crate::Error;

/// The packet in order, as `KeyData.toBytes` writes it: the character's byte count, then Flutter's `KeyData`
/// (`lib/ui/window/key_data.h`), then the character's UTF-8 bytes. Every field is 8 bytes, little-endian.
/// To follow a Flutter change: edit this list in Flutter's order; offsets come from the position.
#[derive(Clone, Copy)]
#[allow(dead_code, reason = "the full layout; only some fields are read")]
enum Field {
    /// `charSize` in `KeyData.toBytes` (Flutter's `KeyDataPacket`), not part of `KeyData` itself.
    CharSize,
    Timestamp,
    Type,
    Physical,
    Logical,
    Synthesized,
    DeviceType,
}

/// Must match `KeyData.FIELD_COUNT` (Flutter's `kKeyDataFieldCount`): the fields after `CharSize`.
const KEY_DATA_FIELD_COUNT: usize = Field::DeviceType as usize;
const BYTES_PER_FIELD: usize = 8;
const HEADER_BYTES: usize = (1 + KEY_DATA_FIELD_COUNT) * BYTES_PER_FIELD;
const _: () = assert!(KEY_DATA_FIELD_COUNT == 6);

/// Flutter's `KeyEventType` (`key_data.h`), mirrored in `KeyData.Type`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum KeyEventType {
    Down,
    Up,
    Repeat,
}

/// Packet value → event type, indexed by Flutter's value. To follow Flutter: append new values in its order.
const KEY_EVENT_TYPES: [KeyEventType; 3] =
    [KeyEventType::Down, KeyEventType::Up, KeyEventType::Repeat];

/// The fields of one packet that GPUI uses; Flutter's names (`type` is `kind` here).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KeyData {
    pub(crate) kind: KeyEventType,
    /// Flutter's logical key: `KeyboardMap.keyCodeToLogical` of the Android key code, or the code in the Android
    /// plane. 0 for the empty event `KeyEmbedderResponder` sends when nothing else is.
    pub(crate) logical: u64,
    /// What the key types with the current meta state (`KeyEvent.getUnicodeChar`), dead keys combined.
    pub(crate) character: Option<String>,
}

/// Decodes one packet (`KeyData.toBytes` sends one event per packet).
pub(crate) fn decode(packet: &[u8]) -> Result<KeyData, Error> {
    let length = Error::KeyPacketLength(packet.len());
    let (fields, _) = packet.as_chunks::<BYTES_PER_FIELD>();
    let long = |field: Field| {
        fields
            .get(field as usize)
            .map(|bytes| i64::from_le_bytes(*bytes))
    };
    let character = long(Field::CharSize)
        .and_then(|size| usize::try_from(size).ok())
        .filter(|size| HEADER_BYTES.checked_add(*size) == Some(packet.len()))
        .and_then(|_| packet.get(HEADER_BYTES..))
        .ok_or(length)?;
    let kind = long(Field::Type).unwrap_or_default();
    Ok(KeyData {
        kind: usize::try_from(kind)
            .ok()
            .and_then(|index| KEY_EVENT_TYPES.get(index).copied())
            .ok_or(Error::KeyEventType(kind))?,
        logical: long(Field::Logical).unwrap_or_default() as u64,
        character: match character {
            [] => None,
            bytes => Some(String::from_utf8(bytes.to_vec()).map_err(|_| Error::KeyCharacter)?),
        },
    })
}

/// A modifier GPUI tracks in [`Modifiers`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Modifier {
    Control,
    Alt,
    Shift,
    Platform,
    Function,
}

/// What a logical key is to GPUI.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Key {
    /// A key event with GPUI's name for the key (`Keystroke::key`, as the other GPUI platforms name it).
    Named(&'static str),
    /// A printable key in Flutter's Unicode plane: its lowercase character is GPUI's name.
    Character(char),
    /// GPUI gets `ModifiersChanged`, never a key event, as on the other GPUI platforms.
    Modifier(Modifier),
    CapsLock,
    /// GPUI's `BackGestureEvent`; Android's own back (the Activity) when no element claims it.
    Back,
}

/// Flutter logical key (KeyboardMap.java names) → GPUI. Keys without a row are not GPUI's: Android gets them back
/// (volume, media, TV keys). Logical keys in Flutter's Unicode plane are [`Key::Character`] unless a row names them.
/// To add a key: add a row with its value and Flutter name from `KeyboardMap.keyCodeToLogical`.
const KEYS: &[(u64, Key)] = &[
    (0x0000000020, Key::Named("space")),               // space
    (0x0100000008, Key::Named("backspace")),           // backspace (KEYCODE_DEL)
    (0x0100000009, Key::Named("tab")),                 // tab
    (0x010000000d, Key::Named("enter")),               // enter
    (0x010000001b, Key::Named("escape")),              // escape
    (0x010000007f, Key::Named("delete")),              // delete (KEYCODE_FORWARD_DEL)
    (0x0100000104, Key::CapsLock),                     // capsLock
    (0x0100000106, Key::Modifier(Modifier::Function)), // fn
    (0x0100000301, Key::Named("down")),                // arrowDown
    (0x0100000302, Key::Named("left")),                // arrowLeft
    (0x0100000303, Key::Named("right")),               // arrowRight
    (0x0100000304, Key::Named("up")),                  // arrowUp
    (0x0100000305, Key::Named("end")),                 // end
    (0x0100000306, Key::Named("home")),                // home
    (0x0100000307, Key::Named("pagedown")),            // pageDown
    (0x0100000308, Key::Named("pageup")),              // pageUp
    (0x0100000407, Key::Named("insert")),              // insert
    (0x0100000801, Key::Named("f1")),                  // f1
    (0x0100000802, Key::Named("f2")),                  // f2
    (0x0100000803, Key::Named("f3")),                  // f3
    (0x0100000804, Key::Named("f4")),                  // f4
    (0x0100000805, Key::Named("f5")),                  // f5
    (0x0100000806, Key::Named("f6")),                  // f6
    (0x0100000807, Key::Named("f7")),                  // f7
    (0x0100000808, Key::Named("f8")),                  // f8
    (0x0100000809, Key::Named("f9")),                  // f9
    (0x010000080a, Key::Named("f10")),                 // f10
    (0x010000080b, Key::Named("f11")),                 // f11
    (0x010000080c, Key::Named("f12")),                 // f12
    (0x0100001005, Key::Back),                         // goBack (KEYCODE_BACK)
    (0x0200000100, Key::Modifier(Modifier::Control)),  // controlLeft
    (0x0200000101, Key::Modifier(Modifier::Control)),  // controlRight
    (0x0200000102, Key::Modifier(Modifier::Shift)),    // shiftLeft
    (0x0200000103, Key::Modifier(Modifier::Shift)),    // shiftRight
    (0x0200000104, Key::Modifier(Modifier::Alt)),      // altLeft
    (0x0200000105, Key::Modifier(Modifier::Alt)),      // altRight
    (0x0200000106, Key::Modifier(Modifier::Platform)), // metaLeft
    (0x0200000107, Key::Modifier(Modifier::Platform)), // metaRight
    (0x020000020d, Key::Named("enter")),               // numpadEnter
    (0x020000022a, Key::Named("*")),                   // numpadMultiply
    (0x020000022b, Key::Named("+")),                   // numpadAdd
    (0x020000022d, Key::Named("-")),                   // numpadSubtract
    (0x020000022e, Key::Named(".")),                   // numpadDecimal
    (0x020000022f, Key::Named("/")),                   // numpadDivide
    (0x0200000230, Key::Named("0")),                   // numpad0
    (0x0200000231, Key::Named("1")),                   // numpad1
    (0x0200000232, Key::Named("2")),                   // numpad2
    (0x0200000233, Key::Named("3")),                   // numpad3
    (0x0200000234, Key::Named("4")),                   // numpad4
    (0x0200000235, Key::Named("5")),                   // numpad5
    (0x0200000236, Key::Named("6")),                   // numpad6
    (0x0200000237, Key::Named("7")),                   // numpad7
    (0x0200000238, Key::Named("8")),                   // numpad8
    (0x0200000239, Key::Named("9")),                   // numpad9
    (0x020000022c, Key::Named(",")),                   // numpadComma
    (0x020000023d, Key::Named("=")),                   // numpadEqual
    (0x0200000228, Key::Named("(")),                   // numpadParenLeft
    (0x0200000229, Key::Named(")")),                   // numpadParenRight
];

/// Flutter's `KeyboardMap.kValueMask`: the bits of a logical key below its plane.
const VALUE_MASK: u64 = 0x000ffffffff;

/// Android's back key as GPUI's back gesture: one press is offered once and claimed (or not) as a whole.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Back {
    #[default]
    Idle,
    /// `Started` was sent; GPUI's answer to it decides.
    Offered,
    /// An element took the press: repeats are swallowed, the release commits.
    Claimed,
}

/// What happens to the back press: a back key event, or GPUI's answer to the last [`Step::Ask`] (for any key).
#[derive(Clone, Debug, PartialEq)]
enum BackEvent {
    Key(KeyEventType),
    /// The release came with `FLAG_CANCELED` (`KeyEvent.isCanceled()`): Android does not go back on it.
    Cancel,
    Answer(DispatchEventResult),
}

/// What one back event leads to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Outcome {
    /// Send GPUI this phase, then feed its answer back as [`BackEvent::Answer`].
    Ask(BackGesturePhase),
    /// Send GPUI `phase` if there is one, and answer Android with `handled`.
    Answer {
        phase: Option<BackGesturePhase>,
        handled: bool,
    },
}

impl Back {
    /// The only place the back press changes: state × event → next state and its outcome.
    /// To add an event: add the variant, and the compiler lists the arms to write here.
    fn next(self, event: BackEvent) -> (Back, Outcome) {
        use BackEvent::{Answer, Cancel, Key};
        use KeyEventType::{Down, Repeat, Up};
        let answer = |phase, handled| Outcome::Answer { phase, handled };
        match (self, event) {
            (Back::Idle | Back::Offered | Back::Claimed, Key(Down)) => {
                (Back::Offered, Outcome::Ask(BackGesturePhase::Started))
            }
            (Back::Claimed, Key(Repeat)) => (Back::Claimed, answer(None, true)),
            (Back::Claimed, Key(Up)) => {
                (Back::Idle, answer(Some(BackGesturePhase::Committed), true))
            }
            (Back::Claimed, Cancel) => {
                (Back::Idle, answer(Some(BackGesturePhase::Cancelled), true))
            }
            (Back::Idle | Back::Offered, Key(Repeat | Up) | Cancel) => {
                (Back::Idle, answer(None, false))
            }
            // An element with a back listener prevents default: it claims the press.
            (Back::Offered, Answer(result)) => match result.default_prevented {
                true => (Back::Claimed, answer(None, true)),
                false => (Back::Idle, answer(None, false)),
            },
            // Any other key's answer: handled when GPUI stopped its propagation.
            (state @ (Back::Idle | Back::Claimed), Answer(result)) => {
                (state, answer(None, !result.propagate))
            }
        }
    }
}

/// What the shell does with one packet.
#[derive(Debug)]
pub(crate) enum Step {
    /// Send this to GPUI, then pass its answer to [`Keyboard::reply`].
    Ask(PlatformInput),
    /// Send `input` to GPUI if there is one, and answer Android with `handled`.
    Answer {
        input: Option<PlatformInput>,
        handled: bool,
    },
}

/// The keyboard as GPUI sees it: held modifiers, caps lock and the back press. Flutter's responder already
/// synthesizes the events that bring held modifiers in line with Android's meta state, so following its events
/// keeps this true.
#[derive(Debug, Default)]
pub(crate) struct Keyboard {
    /// Held modifier keys by Flutter logical key; left and right are separate keys.
    held: Vec<(u64, Modifier)>,
    capslock: Capslock,
    back: Back,
}

impl Keyboard {
    pub(crate) fn modifiers(&self) -> Modifiers {
        let mut modifiers = Modifiers::default();
        for (_, modifier) in &self.held {
            match modifier {
                Modifier::Control => modifiers.control = true,
                Modifier::Alt => modifiers.alt = true,
                Modifier::Shift => modifiers.shift = true,
                Modifier::Platform => modifiers.platform = true,
                Modifier::Function => modifiers.function = true,
            }
        }
        modifiers
    }

    #[cfg_attr(
        not(target_os = "android"),
        allow(dead_code, reason = "read by the window, which is Android only")
    )]
    pub(crate) fn capslock(&self) -> Capslock {
        self.capslock
    }

    /// Decides what GPUI gets for one packet. `canceled` is the Android event's `FLAG_CANCELED`, which Flutter's
    /// key data does not carry. `back_at` is where a back press starts: GPUI hit-tests back gestures, and
    /// Android's back key has no position.
    pub(crate) fn input(&mut self, data: &KeyData, canceled: bool, back_at: Point<Pixels>) -> Step {
        let unhandled = Step::Answer {
            input: None,
            handled: false,
        };
        let key = KEYS
            .iter()
            .find(|(value, _)| *value == data.logical)
            .map(|(_, key)| *key)
            .or_else(|| {
                // Flutter's Unicode plane is plane 0; KeyboardMap only puts printable ASCII there.
                (data.logical & !VALUE_MASK == 0)
                    .then(|| char::from_u32(data.logical as u32))
                    .flatten()
                    .filter(|character| !character.is_control())
                    .map(Key::Character)
            });
        let Some(key) = key else {
            return unhandled;
        };
        match key {
            Key::Back => {
                let event = match (data.kind, canceled) {
                    (KeyEventType::Up, true) => BackEvent::Cancel,
                    (kind, _) => BackEvent::Key(kind),
                };
                let (back, outcome) = self.back.next(event);
                self.back = back;
                // A started press has made no progress; a committed one is complete.
                let gesture = |phase| {
                    PlatformInput::BackGesture(BackGestureEvent {
                        phase,
                        start_position: back_at,
                        progress: match phase {
                            BackGesturePhase::Committed => 1.,
                            BackGesturePhase::Started
                            | BackGesturePhase::Progressed
                            | BackGesturePhase::Cancelled => 0.,
                        },
                    })
                };
                match outcome {
                    Outcome::Ask(phase) => Step::Ask(gesture(phase)),
                    Outcome::Answer { phase, handled } => Step::Answer {
                        input: phase.map(gesture),
                        handled,
                    },
                }
            }
            Key::Modifier(modifier) => {
                match data.kind {
                    KeyEventType::Down => {
                        if !self.held.iter().any(|(held, _)| *held == data.logical) {
                            self.held.push((data.logical, modifier));
                        }
                    }
                    KeyEventType::Up => self.held.retain(|(held, _)| *held != data.logical),
                    KeyEventType::Repeat => return unhandled,
                }
                self.modifiers_changed()
            }
            Key::CapsLock => match data.kind {
                KeyEventType::Down => {
                    self.capslock.on = !self.capslock.on;
                    self.modifiers_changed()
                }
                KeyEventType::Up | KeyEventType::Repeat => unhandled,
            },
            Key::Named(name) => self.keystroke(name.to_string(), data),
            Key::Character(character) => self.keystroke(character.to_string(), data),
        }
    }

    /// GPUI's answer to an [`Step::Ask`] → whether Android counts the key as handled (Flutter's reply).
    pub(crate) fn reply(&mut self, result: DispatchEventResult) -> bool {
        let (back, outcome) = self.back.next(BackEvent::Answer(result));
        self.back = back;
        match outcome {
            Outcome::Answer { handled, .. } => handled,
            // `Back::next` never asks on an answer: its table has a row per state.
            Outcome::Ask(_) => false,
        }
    }

    fn modifiers_changed(&self) -> Step {
        Step::Ask(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
            modifiers: self.modifiers(),
            capslock: self.capslock,
        }))
    }

    fn keystroke(&self, key: String, data: &KeyData) -> Step {
        let keystroke = Keystroke {
            modifiers: self.modifiers(),
            key,
            // Enter, tab and backspace carry control characters; GPUI's key_char is text only.
            key_char: data
                .character
                .clone()
                .filter(|text| !text.chars().any(char::is_control)),
        };
        Step::Ask(match data.kind {
            KeyEventType::Down | KeyEventType::Repeat => PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held: data.kind == KeyEventType::Repeat,
                prefer_character_input: false,
            }),
            KeyEventType::Up => PlatformInput::KeyUp(KeyUpEvent { keystroke }),
        })
    }
}

#[cfg(test)]
mod tests {
    use gpui::{point, px};

    use super::*;

    // Flutter's values (KeyData.Type) and logical keys (KeyboardMap.keyCodeToLogical).
    const DOWN: i64 = 0;
    const UP: i64 = 1;
    const REPEAT: i64 = 2;
    const KEY_A: u64 = 0x61;
    const ARROW_LEFT: u64 = 0x0100000302;
    const ENTER: u64 = 0x010000000d;
    const CAPS_LOCK: u64 = 0x0100000104;
    const GO_BACK: u64 = 0x0100001005;
    const AUDIO_VOLUME_UP: u64 = 0x0100000a10;
    const CONTROL_LEFT: u64 = 0x0200000100;
    const SHIFT_LEFT: u64 = 0x0200000102;
    const SHIFT_RIGHT: u64 = 0x0200000103;
    const NUMPAD_1: u64 = 0x0200000231;
    /// A key code with no Flutter name lands in `KeyboardMap.kAndroidPlane`.
    const ANDROID_PLANE_KEY: u64 = 0x01100000000 | 300;

    // GPUI's answers: the app handled a key, nobody did, an element claimed the back press.
    const HANDLED: DispatchEventResult = DispatchEventResult {
        propagate: false,
        default_prevented: false,
    };
    const UNHANDLED: DispatchEventResult = DispatchEventResult {
        propagate: true,
        default_prevented: false,
    };
    const CLAIMED: DispatchEventResult = DispatchEventResult {
        propagate: false,
        default_prevented: true,
    };

    /// One packet as `KeyData.toBytes` writes it (other fields 0).
    fn packet(kind: i64, logical: u64, character: &str) -> Vec<u8> {
        let mut fields = [[0u8; BYTES_PER_FIELD]; 1 + KEY_DATA_FIELD_COUNT];
        fields[Field::CharSize as usize] = (character.len() as i64).to_le_bytes();
        fields[Field::Type as usize] = kind.to_le_bytes();
        fields[Field::Logical as usize] = logical.to_le_bytes();
        [fields.concat(), character.as_bytes().to_vec()].concat()
    }

    fn modifiers(modifiers: &Modifiers) -> String {
        [
            (modifiers.control, "ctrl-"),
            (modifiers.alt, "alt-"),
            (modifiers.shift, "shift-"),
            (modifiers.platform, "cmd-"),
            (modifiers.function, "fn-"),
        ]
        .iter()
        .filter_map(|(on, name)| on.then_some(*name))
        .collect()
    }

    fn describe(input: &PlatformInput) -> String {
        let keystroke = |keystroke: &Keystroke| {
            format!(
                "{}{} {:?}",
                modifiers(&keystroke.modifiers),
                keystroke.key,
                keystroke.key_char
            )
        };
        match input {
            PlatformInput::KeyDown(event) if event.is_held => {
                format!("held {}", keystroke(&event.keystroke))
            }
            PlatformInput::KeyDown(event) => format!("down {}", keystroke(&event.keystroke)),
            PlatformInput::KeyUp(event) => format!("up {}", keystroke(&event.keystroke)),
            PlatformInput::ModifiersChanged(event) => format!(
                "modifiers {}caps={}",
                modifiers(&event.modifiers),
                event.capslock.on
            ),
            PlatformInput::BackGesture(event) => format!("back {:?}", event.phase),
            other => format!("unexpected {other:?}"),
        }
    }

    #[test]
    fn keyboard() -> Result<(), Error> {
        // (row name, packets in order with GPUI's answer to each, then per packet: the event GPUI gets and
        // whether Android is told it was handled).
        type Row = (
            &'static str,
            Vec<(Vec<u8>, DispatchEventResult)>,
            Vec<(Option<&'static str>, bool)>,
        );
        let cases: Vec<Row> = vec![
            (
                "letter_down_up_unhandled",
                vec![
                    (packet(DOWN, KEY_A, "a"), UNHANDLED),
                    (packet(UP, KEY_A, ""), UNHANDLED),
                ],
                vec![
                    (Some(r#"down a Some("a")"#), false),
                    (Some("up a None"), false),
                ],
            ),
            (
                "app_handles_letter",
                vec![(packet(DOWN, KEY_A, "a"), HANDLED)],
                vec![(Some(r#"down a Some("a")"#), true)],
            ),
            (
                "repeat_is_held",
                vec![(packet(REPEAT, KEY_A, "a"), UNHANDLED)],
                vec![(Some(r#"held a Some("a")"#), false)],
            ),
            (
                "shift_letter",
                vec![
                    (packet(DOWN, SHIFT_LEFT, ""), UNHANDLED),
                    (packet(DOWN, KEY_A, "A"), UNHANDLED),
                    (packet(UP, SHIFT_LEFT, ""), UNHANDLED),
                ],
                vec![
                    (Some("modifiers shift-caps=false"), false),
                    (Some(r#"down shift-a Some("A")"#), false),
                    (Some("modifiers caps=false"), false),
                ],
            ),
            (
                "one_of_two_shifts_released",
                vec![
                    (packet(DOWN, SHIFT_LEFT, ""), UNHANDLED),
                    (packet(DOWN, SHIFT_RIGHT, ""), UNHANDLED),
                    (packet(UP, SHIFT_LEFT, ""), UNHANDLED),
                ],
                vec![
                    (Some("modifiers shift-caps=false"), false),
                    (Some("modifiers shift-caps=false"), false),
                    (Some("modifiers shift-caps=false"), false),
                ],
            ),
            (
                "modifier_repeat_sends_nothing",
                vec![
                    (packet(DOWN, CONTROL_LEFT, ""), UNHANDLED),
                    (packet(REPEAT, CONTROL_LEFT, ""), UNHANDLED),
                ],
                vec![(Some("modifiers ctrl-caps=false"), false), (None, false)],
            ),
            (
                "control_letter",
                vec![
                    (packet(DOWN, CONTROL_LEFT, ""), UNHANDLED),
                    (packet(DOWN, KEY_A, ""), HANDLED),
                ],
                vec![
                    (Some("modifiers ctrl-caps=false"), false),
                    (Some("down ctrl-a None"), true),
                ],
            ),
            (
                "caps_lock_toggles_on_down",
                vec![
                    (packet(DOWN, CAPS_LOCK, ""), UNHANDLED),
                    (packet(UP, CAPS_LOCK, ""), UNHANDLED),
                    (packet(DOWN, CAPS_LOCK, ""), UNHANDLED),
                ],
                vec![
                    (Some("modifiers caps=true"), false),
                    (None, false),
                    (Some("modifiers caps=false"), false),
                ],
            ),
            (
                "arrow_is_named",
                vec![(packet(DOWN, ARROW_LEFT, ""), UNHANDLED)],
                vec![(Some("down left None"), false)],
            ),
            (
                "enter_has_no_key_char",
                vec![(packet(DOWN, ENTER, "\n"), UNHANDLED)],
                vec![(Some("down enter None"), false)],
            ),
            (
                "numpad_digit",
                vec![(packet(DOWN, NUMPAD_1, "1"), UNHANDLED)],
                vec![(Some(r#"down 1 Some("1")"#), false)],
            ),
            (
                "volume_goes_to_android",
                vec![(packet(DOWN, AUDIO_VOLUME_UP, ""), HANDLED)],
                vec![(None, false)],
            ),
            (
                "android_plane_goes_to_android",
                vec![(packet(DOWN, ANDROID_PLANE_KEY, ""), HANDLED)],
                vec![(None, false)],
            ),
            (
                // KeyEmbedderResponder's empty event: logical and physical 0.
                "empty_event_ignored",
                vec![(packet(DOWN, 0, ""), HANDLED)],
                vec![(None, false)],
            ),
            (
                "back_claimed",
                vec![
                    (packet(DOWN, GO_BACK, ""), CLAIMED),
                    (packet(REPEAT, GO_BACK, ""), UNHANDLED),
                    (packet(UP, GO_BACK, ""), UNHANDLED),
                ],
                vec![
                    (Some("back Started"), true),
                    (None, true),
                    (Some("back Committed"), true),
                ],
            ),
            (
                "back_unclaimed_goes_to_android",
                vec![
                    (packet(DOWN, GO_BACK, ""), UNHANDLED),
                    (packet(REPEAT, GO_BACK, ""), UNHANDLED),
                    (packet(UP, GO_BACK, ""), UNHANDLED),
                ],
                vec![(Some("back Started"), false), (None, false), (None, false)],
            ),
            (
                // Propagation stopped without a back listener is not a claim.
                "back_handled_not_claimed",
                vec![
                    (packet(DOWN, GO_BACK, ""), HANDLED),
                    (packet(UP, GO_BACK, ""), UNHANDLED),
                ],
                vec![(Some("back Started"), false), (None, false)],
            ),
            (
                "back_claimed_after_unclaimed",
                vec![
                    (packet(DOWN, GO_BACK, ""), UNHANDLED),
                    (packet(UP, GO_BACK, ""), UNHANDLED),
                    (packet(DOWN, GO_BACK, ""), CLAIMED),
                    (packet(UP, GO_BACK, ""), UNHANDLED),
                ],
                vec![
                    (Some("back Started"), false),
                    (None, false),
                    (Some("back Started"), true),
                    (Some("back Committed"), true),
                ],
            ),
        ];
        let back_at = point(px(0.), px(400.));
        for (name, packets, expected) in cases {
            let mut keyboard = Keyboard::default();
            let mut got = Vec::new();
            for (bytes, answer) in packets {
                got.push(match keyboard.input(&decode(&bytes)?, false, back_at) {
                    Step::Ask(input) => (Some(describe(&input)), keyboard.reply(answer)),
                    Step::Answer { input, handled } => (input.as_ref().map(describe), handled),
                });
            }
            let expected: Vec<_> = expected
                .into_iter()
                .map(|(event, handled)| (event.map(str::to_string), handled))
                .collect();
            assert_eq!(got, expected, "{name}");
        }
        Ok(())
    }

    #[test]
    fn back_transitions() {
        use Back::{Claimed, Idle, Offered};
        use BackEvent::{Answer, Cancel, Key};
        use BackGesturePhase::{Cancelled, Committed, Started};
        use KeyEventType::{Down, Repeat, Up};
        let answer = |phase, handled| Outcome::Answer { phase, handled };
        // (row name, state, event, expected state, expected outcome). One row per (state, event); the answers are
        // GPUI's: handled, unhandled, and claimed by a back listener.
        #[rustfmt::skip]
        let cases: [(&str, Back, BackEvent, Back, Outcome); 21] = [
            ("idle_down_offers", Idle, Key(Down), Offered, Outcome::Ask(Started)),
            ("idle_repeat_unhandled", Idle, Key(Repeat), Idle, answer(None, false)),
            ("idle_up_unhandled", Idle, Key(Up), Idle, answer(None, false)),
            ("idle_cancel_unhandled", Idle, Cancel, Idle, answer(None, false)),
            ("idle_handled_answer", Idle, Answer(HANDLED), Idle, answer(None, true)),
            ("idle_unhandled_answer", Idle, Answer(UNHANDLED), Idle, answer(None, false)),
            ("idle_claimed_answer_is_handled", Idle, Answer(CLAIMED), Idle, answer(None, true)),
            ("offered_down_offers_again", Offered, Key(Down), Offered, Outcome::Ask(Started)),
            ("offered_repeat_unhandled", Offered, Key(Repeat), Idle, answer(None, false)),
            ("offered_up_unhandled", Offered, Key(Up), Idle, answer(None, false)),
            ("offered_cancel_unhandled", Offered, Cancel, Idle, answer(None, false)),
            ("offered_handled_is_no_claim", Offered, Answer(HANDLED), Idle, answer(None, false)),
            ("offered_unhandled", Offered, Answer(UNHANDLED), Idle, answer(None, false)),
            ("offered_claimed", Offered, Answer(CLAIMED), Claimed, answer(None, true)),
            ("claimed_down_offers_again", Claimed, Key(Down), Offered, Outcome::Ask(Started)),
            ("claimed_repeat_swallowed", Claimed, Key(Repeat), Claimed, answer(None, true)),
            ("claimed_up_commits", Claimed, Key(Up), Idle, answer(Some(Committed), true)),
            ("claimed_cancel_cancels", Claimed, Cancel, Idle, answer(Some(Cancelled), true)),
            ("claimed_handled_answer", Claimed, Answer(HANDLED), Claimed, answer(None, true)),
            ("claimed_unhandled_answer", Claimed, Answer(UNHANDLED), Claimed, answer(None, false)),
            ("claimed_claimed_answer", Claimed, Answer(CLAIMED), Claimed, answer(None, true)),
        ];
        for (name, state, event, expected_state, expected_outcome) in cases {
            assert_eq!(
                state.next(event),
                (expected_state, expected_outcome),
                "{name}"
            );
        }
    }

    #[test]
    fn decode_packets() {
        let mut negative_size = packet(DOWN, KEY_A, "");
        negative_size[..BYTES_PER_FIELD].copy_from_slice(&(-1i64).to_le_bytes());
        let mut bad_utf8 = packet(DOWN, KEY_A, "a");
        bad_utf8[HEADER_BYTES] = 0xff;
        let accented = packet(DOWN, KEY_A, "é");
        // (row name, packet, expected decode).
        let cases = [
            (
                "with_character",
                accented.clone(),
                Ok(KeyData {
                    kind: KeyEventType::Down,
                    logical: KEY_A,
                    character: Some("é".to_string()),
                }),
            ),
            (
                "without_character",
                packet(UP, KEY_A, ""),
                Ok(KeyData {
                    kind: KeyEventType::Up,
                    logical: KEY_A,
                    character: None,
                }),
            ),
            ("empty", vec![], Err(Error::KeyPacketLength(0))),
            (
                "short_header",
                accented[..HEADER_BYTES - 8].to_vec(),
                Err(Error::KeyPacketLength(HEADER_BYTES - 8)),
            ),
            (
                "character_cut_off",
                accented[..accented.len() - 1].to_vec(),
                Err(Error::KeyPacketLength(accented.len() - 1)),
            ),
            (
                "trailing_bytes",
                [accented.clone(), vec![0]].concat(),
                Err(Error::KeyPacketLength(accented.len() + 1)),
            ),
            (
                "negative_character_size",
                negative_size,
                Err(Error::KeyPacketLength(HEADER_BYTES)),
            ),
            (
                "unknown_type",
                packet(3, KEY_A, ""),
                Err(Error::KeyEventType(3)),
            ),
            ("bad_utf8", bad_utf8, Err(Error::KeyCharacter)),
        ];
        for (name, bytes, expected) in cases {
            assert_eq!(
                format!("{:?}", decode(&bytes)),
                format!("{expected:?}"),
                "{name}"
            );
        }
    }
}
