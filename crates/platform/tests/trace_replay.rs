//! Gate 2's trace replay (docs/traces.md): touch traces recorded on each platform by `scripts/record-traces.sh` go
//! through GPUI's recognizer and scroll physics, under every target's row of the behaviour table, and must come out
//! as the same gestures and the same scroll everywhere. Runs on GPUI's test platform (host only).

#![cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test: a bad fixture or a failed replay should stop it (AGENTS.md, Errors)"
)]

use std::{cell::RefCell, path::Path, rc::Rc, time::Duration};

use gpui::{
    App, Context, DispatchPhase, GestureTuning, InputEvent as _, IntoElement, LongPressEvent,
    MouseDownEvent, PointerDevice, Render, ScrollDelta, ScrollEvent, TestAppContext, TouchEvent,
    TouchId, TouchPhase, VelocityTrackerKind, Window, canvas, point, px,
};
use parallax_platform::{
    Target,
    behaviour::{TABLE, web_behaviour},
};

/// Platforms with a trace folder under tests/traces/. Each gesture must have a trace from at least two of them.
const PLATFORMS: [&str; 3] = ["android", "web", "ios"];

/// The gestures of record-traces.sh's table, with what every platform's trace must replay to under every row.
/// `pinch` is recorded but not replayed: GPUI ignores a second finger until the scale recognizer is wired into touch
/// dispatch (ROADMAP Phase 2), and until then each platform's first finger, a different one, pans.
const GESTURES: [(&str, Gesture); 5] = [
    ("tap", Gesture::Taps(&[1])),
    ("double-tap", Gesture::Taps(&[1, 2])),
    ("long-press", Gesture::LongPress),
    ("slow-drag", Gesture::Drag),
    ("fling", Gesture::Fling),
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
    /// A pan that flings on: momentum scrolls on after the release.
    Fling,
}

/// Scroll totals of the same gesture on two platforms may differ by this fraction of the larger one. The drivers move
/// between the same points, so pans differ only by the last move a platform reports (iOS lifts 10 px past it); a
/// fling's momentum is its release velocity's integral, and each platform samples the finger at its own rate (iOS
/// every 16 ms, headless Chromium every ~33 ms, Android per input batch), so the velocity, and the distance, differ.
const SCROLL_TOLERANCE: f32 = 0.35;

/// Differences the replay found and their open fix: (gesture, the rows' velocity tracker, why). Their scroll is
/// printed, not compared; recognition is still checked. Remove a row once its fix lands.
const KNOWN_DIFFERENCES: [(&str, VelocityTrackerKind, &str); 1] = [(
    "fling",
    VelocityTrackerKind::LeastSquares,
    "touches are timed when GPUI receives them, not by the platform's event time, and least squares \
     extrapolates the web's jittered ~33 ms samples to a lower release velocity than iOS's even 16 ms ones \
     (ROADMAP Phase 2, Touch timestamps)",
)];

/// One replay's result: the window's gestures, summed.
#[derive(Debug, Default, PartialEq)]
struct Outcome {
    /// Click count of each tap.
    taps: Vec<usize>,
    long_press: bool,
    /// Vertical scroll while a finger was down (a pan), and after the last finger lifted (fling momentum).
    pan: f32,
    momentum: f32,
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
    }
}

/// One `touch(at_ms, id, phase, x, y)` line of a trace file; `#` lines are comments.
fn parse(trace: &str) -> Vec<(u64, TouchEvent)> {
    let phase = |name: &str| match name {
        "Started" => TouchPhase::Started,
        "Moved" => TouchPhase::Moved,
        "Ended" => TouchPhase::Ended,
        "Cancelled" => TouchPhase::Cancelled,
        other => panic!("unknown touch phase {other}"),
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
                device: PointerDevice::Touch,
            };
            (fields[0].parse().unwrap(), event)
        })
        .collect()
}

/// Frames come every 16 ms, as at 60 Hz: they drive fling momentum.
const FRAME_MS: u64 = 16;
/// The clock runs on this long after the last event, for a long press's timer and a fling's momentum to finish.
const SETTLE_MS: u64 = 5_000;

/// Replays `trace` into a fresh window under `tuning`, a frame every `FRAME_MS`.
fn replay(trace: &[(u64, TouchEvent)], tuning: GestureTuning) -> Outcome {
    let cx = &mut TestAppContext::single();
    cx.update(|cx: &mut App| cx.set_gesture_tuning(tuning));
    let seen = Rc::new(RefCell::new(Seen::default()));
    let window = cx.add_window({
        let seen = seen.clone();
        move |_, _| Recorder { seen }
    });
    let mut now = 0;
    let mut run_until = |cx: &mut TestAppContext, until: u64| {
        while now < until {
            let step = (FRAME_MS - now % FRAME_MS).min(until - now);
            cx.executor().advance_clock(Duration::from_millis(step));
            cx.run_until_parked();
            now += step;
            if now % FRAME_MS == 0 {
                window
                    .update(cx, |_, window, cx| window.simulate_next_frame(cx))
                    .unwrap();
            }
        }
    };
    let mut down = 0;
    for (at_ms, event) in trace {
        run_until(cx, *at_ms);
        down = match event.phase {
            TouchPhase::Started => down + 1,
            TouchPhase::Moved => down,
            TouchPhase::Ended | TouchPhase::Cancelled => down - 1,
        };
        seen.borrow_mut().released = down == 0;
        window
            .update(cx, |_, window, cx| {
                window.dispatch_event(event.clone().to_platform_input(), cx);
            })
            .unwrap();
    }
    let end = trace.last().map_or(0, |(at_ms, _)| *at_ms);
    run_until(cx, end + SETTLE_MS);
    seen.take().outcome
}

/// Checks `outcome` is `gesture`, or says how it is not.
fn recognized(gesture: Gesture, outcome: &Outcome) -> Result<(), String> {
    let still = outcome.pan == 0. && outcome.momentum == 0.;
    let ok = match gesture {
        Gesture::Taps(counts) => outcome.taps == counts && !outcome.long_press && still,
        Gesture::LongPress => outcome.taps.is_empty() && outcome.long_press && still,
        Gesture::Drag => outcome.taps.is_empty() && outcome.pan != 0. && outcome.momentum == 0.,
        Gesture::Fling => outcome.taps.is_empty() && outcome.pan != 0. && outcome.momentum != 0.,
    };
    ok.then_some(()).ok_or(format!("expected {gesture:?}"))
}

fn within(a: f32, b: f32) -> bool {
    (a - b).abs() <= SCROLL_TOLERANCE * a.abs().max(b.abs())
}

#[test]
fn recorded_traces_replay_the_same_everywhere() {
    let traces = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traces");
    // Every target's row, and the web's (Flutter's slops on an Android host).
    let mut rows: Vec<(String, GestureTuning)> = TABLE
        .iter()
        .map(|row| (format!("{:?}", row.target), row.gesture_tuning()))
        .collect();
    let web = web_behaviour(Target::Android).unwrap();
    rows.push(("Web on Android".into(), web.gesture_tuning()));

    let mut failures = Vec::new();
    for (gesture, expected) in GESTURES {
        let recorded: Vec<(&str, Vec<(u64, TouchEvent)>)> = PLATFORMS
            .iter()
            .filter_map(|platform| {
                let file = traces.join(platform).join(format!("{gesture}.trace"));
                let text = std::fs::read_to_string(file).ok()?;
                Some((*platform, parse(&text)))
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
            let known = KNOWN_DIFFERENCES
                .iter()
                .find(|(name, tracker, _)| *name == gesture && *tracker == tuning.velocity_tracker);
            let (first, reference) = &outcomes[0];
            for (platform, outcome) in &outcomes {
                if let Err(why) = recognized(expected, outcome) {
                    failures.push(format!(
                        "{gesture} from {platform} under {row}: {why}, got {outcome:?}"
                    ));
                }
                let same = within(outcome.pan, reference.pan)
                    && within(outcome.momentum, reference.momentum);
                if let (false, Some((_, _, why))) = (same, known) {
                    println!(
                        "known difference, {gesture} under {row}: {platform} {outcome:?}, {first} {reference:?}: {why}"
                    );
                } else if !same {
                    failures.push(format!(
                        "{gesture} under {row}: {platform} scrolls {outcome:?}, {first} {reference:?}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
