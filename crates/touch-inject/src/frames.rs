//! The frame engine on plain values, so it is tested on any host; `main.rs` reads the window and injects what it
//! returns. A gesture is frames in record-traces.sh's syntax (docs/traces.md): `ms finger…`, each finger `slot:x:y`
//! (in contact), `slot:x:y:hover` (a pen in range, not touching) or `slot:up`, in window logical px.
//! Each frame is injected, then held `ms`.

use std::collections::BTreeMap;

// POINTER_FLAG_* from WinUser.h, written out so this file builds on any host; `main.rs` asserts each against the
// windows crate.
pub const INRANGE: u32 = 0x2;
pub const INCONTACT: u32 = 0x4;
pub const DOWN: u32 = 0x1_0000;
pub const UPDATE: u32 = 0x2_0000;
pub const UP: u32 = 0x4_0000;

/// A point in the window, logical px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// A pointer the injection device has in range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InRange {
    Hover,
    Contact,
}

/// What a finger does in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Contact,
    Hover,
    Up,
}

/// (pointer's state, what it does) → its injection flags and its next state; any other pair is an error. These are
/// InjectTouchInput's documented flag sequences (its Remarks table; pens take the same): a down enters contact, an
/// update moves, an up leaves, a hover ends with a bare update. No cancel: Windows delivers an injected up marked
/// `POINTER_FLAG_CANCELED` as a plain up. To extend, add a row.
const TRANSITIONS: [(Option<InRange>, Verb, u32, Option<InRange>); 7] = [
    (
        None,
        Verb::Contact,
        INRANGE | INCONTACT | DOWN,
        Some(InRange::Contact),
    ),
    (
        Some(InRange::Hover),
        Verb::Contact,
        INRANGE | INCONTACT | DOWN,
        Some(InRange::Contact),
    ),
    (
        Some(InRange::Contact),
        Verb::Contact,
        INRANGE | INCONTACT | UPDATE,
        Some(InRange::Contact),
    ),
    (Some(InRange::Contact), Verb::Up, UP, None),
    (None, Verb::Hover, INRANGE | UPDATE, Some(InRange::Hover)),
    (
        Some(InRange::Hover),
        Verb::Hover,
        INRANGE | UPDATE,
        Some(InRange::Hover),
    ),
    (Some(InRange::Hover), Verb::Up, UPDATE, None),
];

/// One finger of a frame: its slot (the injected pointer id), what it does, and where (`None`: where it was).
type Finger = (u32, Verb, Option<Point>);

/// One frame: what the fingers do, then how long to hold.
#[derive(Debug, PartialEq)]
pub struct Frame {
    pub ms: u64,
    fingers: Vec<Finger>,
}

/// One pointer of one injection.
#[derive(Debug, PartialEq)]
pub struct Contact {
    pub slot: u32,
    pub flags: u32,
    pub at: Point,
    /// In contact after this injection (it carries pressure).
    pub touching: bool,
}

pub fn parse_frame(text: &str) -> Result<Frame, String> {
    let mut words = text.split_whitespace();
    let ms = words
        .next()
        .and_then(|ms| ms.parse().ok())
        .ok_or_else(|| format!("frame {text:?}: no hold time in ms first"))?;
    let fingers = words
        .map(|finger| {
            parse_finger(finger).ok_or_else(|| format!("frame {text:?}: bad finger {finger:?}"))
        })
        .collect::<Result<_, _>>()?;
    Ok(Frame { ms, fingers })
}

fn parse_finger(text: &str) -> Option<Finger> {
    let parts: Vec<&str> = text.split(':').collect();
    let slot = parts.first()?.parse().ok()?;
    let point = |x: &str, y: &str| {
        Some(Point {
            x: x.parse().ok()?,
            y: y.parse().ok()?,
        })
    };
    match parts[1..] {
        ["up"] => Some((slot, Verb::Up, None)),
        [x, y] => Some((slot, Verb::Contact, Some(point(x, y)?))),
        [x, y, "hover"] => Some((slot, Verb::Hover, Some(point(x, y)?))),
        _ => None,
    }
}

/// The pointers in range, by slot: every injection must list all of them (InjectTouchInput's `contacts` are "all
/// contacts on the desktop").
#[derive(Debug, Default)]
pub struct Pointers {
    in_range: BTreeMap<u32, (InRange, Point)>,
}

impl Pointers {
    /// The next injection: the frame's fingers, and every other pointer in range staying where it is. An empty
    /// frame repeats every pointer in place, which a held contact needs or Windows cancels it (InjectTouchInput:
    /// "For press and hold gestures, multiple frames must be sent").
    pub fn step(&mut self, frame: &Frame) -> Result<Vec<Contact>, String> {
        let staying = self
            .in_range
            .iter()
            .filter(|(slot, _)| !frame.fingers.iter().any(|(finger, _, _)| finger == *slot))
            .map(|(slot, (state, at))| {
                let verb = match state {
                    InRange::Hover => Verb::Hover,
                    InRange::Contact => Verb::Contact,
                };
                (*slot, verb, Some(*at))
            })
            .collect::<Vec<_>>();
        let mut fingers = [frame.fingers.as_slice(), &staying].concat();
        fingers.sort_by_key(|(slot, _, _)| *slot);
        fingers
            .into_iter()
            .map(|(slot, verb, at)| {
                let was = self.in_range.get(&slot).copied();
                let (_, _, flags, next) = TRANSITIONS
                    .iter()
                    .find(|(state, row_verb, _, _)| {
                        *state == was.map(|(s, _)| s) && *row_verb == verb
                    })
                    .ok_or_else(|| format!("slot {slot} cannot {verb:?} from {was:?}"))?;
                let at = at
                    .or(was.map(|(_, at)| at))
                    .ok_or_else(|| format!("slot {slot}: no position"))?;
                match next {
                    Some(state) => self.in_range.insert(slot, (*state, at)),
                    None => self.in_range.remove(&slot),
                };
                Ok(Contact {
                    slot,
                    flags: *flags,
                    at,
                    touching: *next == Some(InRange::Contact),
                })
            })
            .collect()
    }

    pub fn any_in_range(&self) -> bool {
        !self.in_range.is_empty()
    }
}

/// A window logical point as a physical screen pixel: the client area's screen origin plus the point at the
/// window's scale. Injection takes whole pixels.
pub fn to_screen(at: Point, origin: (i32, i32), scale: f64) -> (i32, i32) {
    (
        origin.0 + (at.x * scale).round() as i32,
        origin.1 + (at.y * scale).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    const CONTACT: u32 = INRANGE | INCONTACT | UPDATE;
    const PRESS: u32 = INRANGE | INCONTACT | DOWN;

    /// One row per (state, finger) pair the gestures use, and the rejected ones: frames in, each injection's
    /// (slot, flags, x, y) out, or the frame that fails.
    #[test]
    fn frames_to_injections() {
        type Injection = &'static [(u32, u32, f64, f64)];
        type Want = Result<&'static [Injection], usize>;
        let rows: [(&str, &[&str], Want); 10] = [
            (
                "tap",
                &["50 0:10:20", "0 0:up"],
                Ok(&[&[(0, PRESS, 10., 20.)], &[(0, UP, 10., 20.)]]),
            ),
            (
                "hold repeats in place",
                &["50 0:10:20", "16", "0 0:up"],
                Ok(&[
                    &[(0, PRESS, 10., 20.)],
                    &[(0, CONTACT, 10., 20.)],
                    &[(0, UP, 10., 20.)],
                ]),
            ),
            (
                "unlisted finger stays",
                &["16 0:1:1 1:5:5", "16 0:2:2", "0 0:up 1:up"],
                Ok(&[
                    &[(0, PRESS, 1., 1.), (1, PRESS, 5., 5.)],
                    &[(0, CONTACT, 2., 2.), (1, CONTACT, 5., 5.)],
                    &[(0, UP, 2., 2.), (1, UP, 5., 5.)],
                ]),
            ),
            (
                "hover, touch, lift, leave",
                &["16 0:1:1:hover", "16 0:2:2", "16 0:up"],
                Ok(&[
                    &[(0, INRANGE | UPDATE, 1., 1.)],
                    &[(0, PRESS, 2., 2.)],
                    &[(0, UP, 2., 2.)],
                ]),
            ),
            (
                "hover ends",
                &["16 0:1:1:hover", "0 0:up"],
                Ok(&[&[(0, INRANGE | UPDATE, 1., 1.)], &[(0, UPDATE, 1., 1.)]]),
            ),
            ("nothing in range injects nothing", &["100"], Ok(&[&[]])),
            ("up when away", &["0 0:up"], Err(0)),
            (
                "hover while touching",
                &["16 0:1:1", "16 0:2:2:hover"],
                Err(1),
            ),
            ("bad finger", &["16 0:x:1"], Err(0)),
            ("no hold time", &["0:1:1"], Err(0)),
        ];
        for (name, frames, want) in rows {
            let mut pointers = Pointers::default();
            let mut got = Vec::new();
            let mut failed = None;
            for (i, frame) in frames.iter().enumerate() {
                match parse_frame(frame).and_then(|frame| pointers.step(&frame)) {
                    Ok(contacts) => got.push(
                        contacts
                            .iter()
                            .map(|c| (c.slot, c.flags, c.at.x, c.at.y))
                            .collect::<Vec<_>>(),
                    ),
                    Err(_) => {
                        failed = Some(i);
                        break;
                    }
                }
            }
            match want {
                Ok(want) => {
                    assert_eq!(failed, None, "{name}");
                    let want: Vec<Vec<_>> = want.iter().map(|i| i.to_vec()).collect();
                    assert_eq!(got, want, "{name}");
                }
                Err(at) => assert_eq!(failed, Some(at), "{name}"),
            }
        }
    }

    #[test]
    fn logical_to_screen() {
        let rows = [
            ("100 %", p(200., 400.), (10, 20), 1., (210, 420)),
            ("150 %", p(200., 400.), (10, 20), 1.5, (310, 620)),
            ("rounds", p(0.4, 0.6), (0, 0), 1., (0, 1)),
            (
                "title bar above the client",
                p(10., -14.),
                (100, 100),
                1.5,
                (115, 79),
            ),
        ];
        for (name, at, origin, scale, want) in rows {
            assert_eq!(to_screen(at, origin, scale), want, "{name}");
        }
    }
}
