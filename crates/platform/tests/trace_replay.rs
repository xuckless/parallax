//! Gates 2 and 3's trace replay (docs/traces.md): touch traces recorded on each platform by `scripts/record-traces.sh` go
//! through GPUI's recognizer and scroll physics, under every target's row of the behaviour table, and must come out
//! as the same gestures and the same scroll everywhere. Runs on GPUI's test platform (host only).

#![cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test: a bad fixture or a failed replay should stop it (AGENTS.md, Errors)"
)]

use std::{cell::RefCell, path::Path, rc::Rc};

use gpui::{
    AnyWindowHandle, App, Context, DispatchPhase, GestureTuning, InputEvent as _, IntoElement,
    LongPressEvent, MouseDownEvent, PinchEvent, Pixels, PlatformInput, Point, PointerDevice,
    Render, ScrollDelta, ScrollEvent, TestAppContext, TouchEvent, TouchId, TouchPhase, TouchPinch,
    TraceStep, Window, canvas, div, play_trace, point, prelude::*, px,
};
use parallax_platform::{
    Target,
    behaviour::{TABLE, web_behaviour},
};

/// Platforms with a trace folder under tests/traces/, the desktops' (Gate 3) read when their harnesses have
/// recorded them. Each gesture must have a trace from at least two of them.
const PLATFORMS: [&str; 7] = [
    "android", "web", "ios", "windows", "wayland", "x11", "macos",
];

/// The gestures of record-traces.sh's table, with what every platform's trace must replay to under every row.
const GESTURES: [(&str, Gesture); 6] = [
    ("tap", Gesture::Taps(&[1])),
    ("double-tap", Gesture::Taps(&[1, 2])),
    ("long-press", Gesture::LongPress),
    ("slow-drag", Gesture::Drag),
    ("fling", Gesture::Fling { ms: 150 }),
    ("pinch", Gesture::Pinch),
];

/// What a trace must be recognized as.
#[derive(Clone, Copy, Debug)]
enum Gesture {
    /// Taps with these click counts, nothing else.
    Taps(&'static [usize]),
    /// A long press, nothing else.
    LongPress,
    /// A pan that stops with the finger: no scroll after the release.
    Drag,
    /// A pan that flings on: momentum scrolls on after the release. `ms` is the script's swipe time: every trace
    /// is compared as if its finger took that long ([`at_scripted_speed`]).
    Fling { ms: u64 },
    /// A touch pinch out (scale above 1) that ends, with no tap, long press or scroll.
    Pinch,
}

/// Scroll totals of the same gesture on two platforms may differ by this fraction of the larger one. The drivers move
/// between the same points, so pans differ only by the last move a platform reports; a fling's momentum is its
/// release velocity's integral, and each driver samples the finger at its own rate (iOS ~20 ms, headless Chromium
/// ~33 ms, Android 16 ms), each sample timed by its platform time stamp. Flings agree within 5 % under every row.
const SCROLL_TOLERANCE: f32 = 0.10;

/// A pinch's final scale (Flutter's: the span now over the span at the start) on two platforms may differ by this
/// fraction of the larger one, and its last focal point by `PINCH_FOCAL_TOLERANCE` logical px. Every driver spreads
/// the same two fingers to the same points; the Android emulator's touchscreen axes (0-32767 over the display) round
/// them by under 0.1 px.
const PINCH_SCALE_TOLERANCE: f32 = 0.01;
const PINCH_FOCAL_TOLERANCE: f32 = 1.;

/// One replay's result: the window's gestures, summed.
#[derive(Debug, Default, PartialEq)]
struct Outcome {
    /// Click count of each tap.
    taps: Vec<usize>,
    long_press: bool,
    /// Vertical scroll while a finger was down (a pan), and after the last finger lifted (fling momentum).
    pan: f32,
    momentum: f32,
    /// The touch pinch, from its start to its last event.
    pinch: Option<Pinch>,
}

/// A touch pinch's last report: Flutter's scale since it started, its focal point (window px), and whether it ended.
#[derive(Debug, PartialEq)]
struct Pinch {
    scale: f32,
    focal: Point<Pixels>,
    ended: bool,
}

/// Records what the window dispatches, as an element that takes long presses would see it.
struct Recorder {
    seen: Rc<RefCell<Seen>>,
}

#[derive(Default)]
struct Seen {
    outcome: Outcome,
    released: bool,
}

impl Render for Recorder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        let on_pinch = self.seen.clone();
        // Touch pinches need an element id (docs/pinch.md); the canvas inside sees the rest window-wide.
        div()
            .id("trace-replay")
            .size_full()
            .on_pinch(move |event: &PinchEvent, _, _| {
                let pinch = &mut on_pinch.borrow_mut().outcome.pinch;
                match (&event.touch, pinch.as_mut()) {
                    (Some(TouchPinch::Start(start)), _) => {
                        *pinch = Some(Pinch {
                            scale: 1.,
                            focal: start.focal_point,
                            ended: false,
                        });
                    }
                    (Some(TouchPinch::Update(update)), Some(pinch)) => {
                        pinch.scale = update.scale;
                        pinch.focal = update.focal_point;
                    }
                    (Some(TouchPinch::End(_)), Some(pinch)) => pinch.ended = true,
                    (Some(TouchPinch::Update(_) | TouchPinch::End(_)), None) | (None, _) => {}
                }
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let on_tap = seen.clone();
                        window.on_mouse_event(move |event: &MouseDownEvent, phase, _, _| {
                            if phase == DispatchPhase::Bubble {
                                on_tap.borrow_mut().outcome.taps.push(event.click_count);
                            }
                        });
                        let on_scroll = seen.clone();
                        window.on_mouse_event(move |event: &ScrollEvent, phase, _, _| {
                            if let (DispatchPhase::Bubble, ScrollDelta::Pixels(delta)) =
                                (phase, event.delta)
                            {
                                let mut seen = on_scroll.borrow_mut();
                                let dy = f32::from(delta.y);
                                match seen.released {
                                    false => seen.outcome.pan += dy,
                                    true => seen.outcome.momentum += dy,
                                }
                            }
                        });
                        window.on_mouse_event(move |_: &LongPressEvent, phase, window, _| {
                            if phase == DispatchPhase::Bubble {
                                seen.borrow_mut().outcome.long_press = true;
                                window.prevent_default();
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}

/// One `touch(at_ms, id, phase, x, y)` or `touch(at_ms, id, phase, x, y, device)` line of a trace file (no device:
/// `Touch`, as every trace before Gate 3); `#` lines are comments.
fn parse(trace: &str) -> Vec<(u64, TouchEvent)> {
    let phase = |name: &str| match name {
        "Started" => TouchPhase::Started,
        "Moved" => TouchPhase::Moved,
        "Ended" => TouchPhase::Ended,
        "Cancelled" => TouchPhase::Cancelled,
        other => panic!("unknown touch phase {other}"),
    };
    // `PointerDevice`'s debug names, as hello logs them. A mouse never sends touches.
    let device = |name: Option<&&str>| match name.copied() {
        None | Some("Touch") => PointerDevice::Touch,
        Some("Pen") => PointerDevice::Pen,
        Some(other) => panic!("unknown touch device {other}"),
    };
    trace
        .lines()
        .filter(|line| line.starts_with("touch("))
        .map(|line| {
            let fields: Vec<&str> = line["touch(".len()..line.len() - 1].split(", ").collect();
            let number = |i: usize| fields[i].parse::<f32>().unwrap();
            let event = TouchEvent {
                id: TouchId(fields[1].parse().unwrap()),
                phase: phase(fields[2]),
                position: point(px(number(3)), px(number(4))),
                predicted_position: None,
                force: None,
                device: device(fields.get(5)),
                // Each touch is dispatched at its `at_ms` (the platform's event time, docs/traces.md), so GPUI's
                // dispatch time is its time stamp.
                time_stamp: None,
            };
            (fields[0].parse().unwrap(), event)
        })
        .collect()
}

/// `trace` with its time line stretched so its last move comes `ms` after its first touch, as the script asked. The
/// drivers swipe at their own speed (idb's 150 ms swipe takes ~200 ms) and a fling's momentum grows with the finger's
/// speed, so only the same speed can fling as far. The spacing of the platform's own samples, which its time stamps
/// carry, stays.
fn at_scripted_speed(trace: &[(u64, TouchEvent)], ms: u64) -> Vec<(u64, TouchEvent)> {
    let first = trace.first().map_or(0, |(at_ms, _)| *at_ms);
    let last_move = trace
        .iter()
        .rev()
        .find(|(_, event)| event.phase == TouchPhase::Moved)
        .map_or(first + ms, |(at_ms, _)| *at_ms);
    let stretch = ms as f64 / (last_move - first).max(1) as f64;
    trace
        .iter()
        .map(|(at_ms, event)| {
            let at = first + ((at_ms - first) as f64 * stretch).round() as u64;
            (at, event.clone())
        })
        .collect()
}

/// Frames come every 16 ms, as at 60 Hz: they drive fling momentum.
const FRAME_MS: u64 = 16;
/// The clock runs on this long after the last event, for a long press's timer and a fling's momentum to finish.
const SETTLE_MS: u64 = 5_000;

/// Replays `trace` into a fresh window under `tuning` with `gpui::play_trace`, a frame every `FRAME_MS`.
fn replay(trace: &[(u64, TouchEvent)], tuning: GestureTuning) -> Outcome {
    let cx = &mut TestAppContext::single();
    cx.update(|cx: &mut App| cx.set_gesture_tuning(tuning));
    let seen = Rc::new(RefCell::new(Seen::default()));
    let window: AnyWindowHandle = cx
        .add_window({
            let seen = seen.clone();
            move |_, _| Recorder { seen }
        })
        .into();
    let end = trace.last().map_or(0, |(at_ms, _)| *at_ms);
    let inputs = trace
        .iter()
        .map(|(at_ms, event)| (*at_ms, event.clone().to_platform_input()));
    let mut down = 0;
    play_trace(
        cx,
        window,
        inputs,
        Some(FRAME_MS),
        end + SETTLE_MS,
        |step, _, _| match step {
            // After the dispatch: a release's own move is the finger's, not momentum.
            TraceStep::Input(PlatformInput::Touch(event)) => {
                down = match event.phase {
                    TouchPhase::Started => down + 1,
                    TouchPhase::Moved => down,
                    TouchPhase::Ended | TouchPhase::Cancelled => down - 1,
                };
                seen.borrow_mut().released = down == 0;
            }
            TraceStep::Input(_) | TraceStep::Frame(_) => {}
        },
    );
    seen.take().outcome
}

/// Checks `outcome` is `gesture`, or says how it is not.
fn recognized(gesture: Gesture, outcome: &Outcome) -> Result<(), String> {
    let still = outcome.pan == 0. && outcome.momentum == 0.;
    let no_pinch = outcome.pinch.is_none();
    let pinched_out = outcome
        .pinch
        .as_ref()
        .is_some_and(|pinch| pinch.ended && pinch.scale > 1.);
    let ok = match gesture {
        Gesture::Taps(counts) => outcome.taps == counts && !outcome.long_press && still && no_pinch,
        Gesture::LongPress => outcome.taps.is_empty() && outcome.long_press && still && no_pinch,
        Gesture::Drag => {
            outcome.taps.is_empty() && outcome.pan != 0. && outcome.momentum == 0. && no_pinch
        }
        Gesture::Fling { .. } => {
            outcome.taps.is_empty() && outcome.pan != 0. && outcome.momentum != 0. && no_pinch
        }
        Gesture::Pinch => outcome.taps.is_empty() && !outcome.long_press && still && pinched_out,
    };
    ok.then_some(()).ok_or(format!("expected {gesture:?}"))
}

fn within(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance * a.abs().max(b.abs())
}

/// Whether two platforms' outcomes of one gesture agree within the tolerances above.
fn same(a: &Outcome, b: &Outcome) -> bool {
    let pinch = match (&a.pinch, &b.pinch) {
        (Some(a), Some(b)) => {
            within(a.scale, b.scale, PINCH_SCALE_TOLERANCE)
                && (a.focal - b.focal).magnitude() <= f64::from(PINCH_FOCAL_TOLERANCE)
        }
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    };
    within(a.pan, b.pan, SCROLL_TOLERANCE)
        && within(a.momentum, b.momentum, SCROLL_TOLERANCE)
        && pinch
}

#[test]
fn trace_line_table() {
    // (row name, fixture line, the device it replays as)
    let rows = [
        (
            "five fields, as Gate 2's traces: a finger",
            "touch(0, 1, Started, 1.00, 2.00)",
            PointerDevice::Touch,
        ),
        (
            "a sixth field names the device",
            "touch(0, 1, Started, 1.00, 2.00, Touch)",
            PointerDevice::Touch,
        ),
        (
            "a pen",
            "touch(0, 1, Started, 1.00, 2.00, Pen)",
            PointerDevice::Pen,
        ),
    ];
    for (name, line, device) in rows {
        let events = parse(line);
        assert_eq!(events.len(), 1, "{name}");
        let (at_ms, event) = &events[0];
        assert_eq!(
            (*at_ms, event.id, event.phase, event.position, event.device),
            (
                0,
                TouchId(1),
                TouchPhase::Started,
                point(px(1.), px(2.)),
                device
            ),
            "{name}"
        );
    }
}

#[test]
fn recorded_traces_replay_the_same_everywhere() {
    let traces = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traces");
    // Every target's row, and the web's (Flutter's slops on an Android host).
    let mut rows: Vec<(String, GestureTuning)> = TABLE
        .iter()
        .map(|row| (format!("{:?}", row.target), row.gesture_tuning(None)))
        .collect();
    let web = web_behaviour(Target::Android).unwrap();
    rows.push(("Web on Android".into(), web.gesture_tuning(None)));

    let mut failures = Vec::new();
    for (gesture, expected) in GESTURES {
        let recorded: Vec<(&str, Vec<(u64, TouchEvent)>)> = PLATFORMS
            .iter()
            .filter_map(|platform| {
                let file = traces.join(platform).join(format!("{gesture}.trace"));
                let trace = parse(&std::fs::read_to_string(file).ok()?);
                let trace = match expected {
                    Gesture::Fling { ms } => at_scripted_speed(&trace, ms),
                    Gesture::Taps(_) | Gesture::LongPress | Gesture::Drag | Gesture::Pinch => trace,
                };
                Some((*platform, trace))
            })
            .collect();
        assert!(
            recorded.len() >= 2,
            "{gesture}: traces from fewer than two platforms"
        );
        for (row, tuning) in &rows {
            let outcomes: Vec<(&str, Outcome)> = recorded
                .iter()
                .map(|(platform, trace)| (*platform, replay(trace, *tuning)))
                .collect();
            println!("{gesture} under {row}: {outcomes:?}");
            let (first, reference) = &outcomes[0];
            for (platform, outcome) in &outcomes {
                if let Err(why) = recognized(expected, outcome) {
                    failures.push(format!(
                        "{gesture} from {platform} under {row}: {why}, got {outcome:?}"
                    ));
                }
                if !same(outcome, reference) {
                    failures.push(format!(
                        "{gesture} under {row}: {platform} scrolls {outcome:?}, {first} {reference:?}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
