//! The Activity lifecycle as GPUI sees it: one state, one transition function. Pure, so it is tested on any host.
//! The shell in `host.rs` feeds it `ParallaxActivity` callbacks and carries out the returned [`Effect`].
//! Phases follow Flutter's `LifecycleChannel`: the app is `Active` only while resumed *and* focused, so the
//! notification shade or a system dialog makes it `Inactive` without an `onPause`. How it maps: docs/android-lifecycle.md.

use gpui::{AppLifecyclePhase, WindowVisibility};

/// Which `ParallaxActivity` instance an event comes from. Each `onCreate` gets the next one, never reused, so a
/// destroyed Activity's late callbacks match nothing. Several can be live at once: a deep link stacks a second
/// one on the first, a clear-task relaunch overlaps the old and the new, multi-window shows two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivityId(pub i32);

/// Where one live Activity is. `focused` is its last `onWindowFocusChanged`: kept in every stage, because focus
/// and lifecycle callbacks arrive in no fixed order (Flutter's `LifecycleChannel` says so).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// After `onCreate` or `onStop`: not visible. Its surface is gone or about to go.
    Created { focused: bool },
    /// After `onStart` or `onPause`: visible, not resumed.
    Started { focused: bool },
    /// After `onResume`.
    Resumed { focused: bool },
}

/// One live Activity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Activity {
    pub id: ActivityId,
    pub stage: Stage,
}

/// The app's Activities. GPUI has one window; the Activity that most recently ran `onStart` owns it, and only
/// the owner's stage reaches GPUI.
#[derive(Clone, Debug, PartialEq)]
pub enum Lifecycle {
    /// Before the first `onCreate`: JNI_OnLoad runs inside it, and `Create` follows.
    Loaded,
    /// At least one Activity is live. `below` are the others, the most recently started last (one never started
    /// sits below every started one); when the owner is destroyed, the last of them takes the window.
    /// `newest` is the last id handed out.
    Live {
        owner: Activity,
        below: Vec<Activity>,
        newest: ActivityId,
    },
    /// Every Activity was destroyed. The process and GPUI live on; a new Activity in this process starts again
    /// from here, with the id after `newest`.
    Disconnected { newest: ActivityId },
}

/// What happened, and to which Activity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivityEvent {
    /// `onCreate`: a new live Activity, which owns the window once it starts (or when it is the only one).
    Create,
    /// A callback from Activity `from`.
    From(ActivityId, Callback),
}

/// One per `ParallaxActivity` callback after `onCreate`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Callback {
    Start,
    Resume,
    Pause,
    Stop,
    Destroy,
    /// `onWindowFocusChanged(hasFocus)`.
    Focus(bool),
    /// `onTrimMemory(level)`. `onLowMemory` is not forwarded: Flutter ignores it too.
    TrimMemory(i32),
}

/// What the shell does after a transition. Window activity and visibility follow from the state itself
/// ([`Lifecycle::is_active`], [`Lifecycle::visibility`]).
#[derive(Debug, PartialEq)]
pub enum Effect {
    /// Tell GPUI's `on_app_lifecycle` observers these phases, in order.
    Phases(&'static [AppLifecyclePhase]),
    /// Tell GPUI's `on_memory_warning` observers.
    MemoryWarning,
    /// A legal event that changes nothing GPUI sees.
    Nothing,
    /// The event makes no sense in this state; nothing changes.
    Reject { why: &'static str },
}

/// `ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW`. Levels at or above it warn, the rule of Flutter 3.50.1's
/// `FlutterActivityAndFragmentDelegate.onTrimMemory` (`level >= TRIM_MEMORY_RUNNING_LOW`), so `UI_HIDDEN` (every
/// Home press) and the background levels warn too; the `trim_levels` test has a row per level. Flutter also skips
/// warnings before its first frame; here GPUI has no observers before launch, and launch is one vsync before it.
const TRIM_MEMORY_RUNNING_LOW: i32 = 10;

/// What GPUI sees of the owner, lowest first: no Activity, hidden (`Created`), visible (`Started`, or resumed
/// without focus), active (resumed and focused).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Gone,
    Hidden,
    Visible,
    Active,
}

/// The phases heard climbing into `Visible` and `Active`, and falling out of `Active`, `Visible` and `Hidden`.
/// A move of several levels hears each step's phase, so the owner changing (another Activity starts, or takes
/// over when the owner is destroyed) reports what GPUI missed. To add a level: a row in each, in level order.
const UP: [AppLifecyclePhase; 2] = [AppLifecyclePhase::Foreground, AppLifecyclePhase::Active];
const DOWN: [AppLifecyclePhase; 3] = [
    AppLifecyclePhase::Inactive,
    AppLifecyclePhase::Background,
    AppLifecyclePhase::Disconnected,
];

/// The phases from one level to another: a slice of [`UP`] or [`DOWN`].
fn phases(from: Level, to: Level) -> &'static [AppLifecyclePhase] {
    // Index into UP: Gone and Hidden climb from 0. Index into DOWN: Active falls from 0.
    let up = |level: Level| (level as usize).max(1) - 1;
    let down = |level: Level| Level::Active as usize - level as usize;
    if to >= from {
        &UP[up(from)..up(to)]
    } else {
        &DOWN[down(from)..down(to)]
    }
}

/// `ParallaxActivity`'s event codes (its `EVENT_*` constants) → callbacks; `value` is the focus flag or trim level.
/// To add a callback: add a constant in `ParallaxActivity.java`, a row here, and the variant above.
impl Callback {
    pub fn from_java(code: i32, value: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Start),
            1 => Some(Self::Resume),
            2 => Some(Self::Pause),
            3 => Some(Self::Stop),
            4 => Some(Self::Destroy),
            5 => Some(Self::Focus(value != 0)),
            6 => Some(Self::TrimMemory(value)),
            _ => None,
        }
    }
}

impl Lifecycle {
    /// The last id handed out; `None` before the first `onCreate`.
    pub fn newest(&self) -> Option<ActivityId> {
        match self {
            Lifecycle::Loaded => None,
            Lifecycle::Live { newest, .. } | Lifecycle::Disconnected { newest } => Some(*newest),
        }
    }

    /// The Activity that owns the window, if any.
    pub fn owner(&self) -> Option<ActivityId> {
        match self {
            Lifecycle::Live { owner, .. } => Some(owner.id),
            Lifecycle::Loaded | Lifecycle::Disconnected { .. } => None,
        }
    }

    /// The one rule for events from a view (surface, frame, insets, touch, keys): only the owner's count.
    pub fn accepts(&self, from: ActivityId) -> bool {
        self.owner() == Some(from)
    }

    fn level(&self) -> Level {
        match self {
            Lifecycle::Loaded | Lifecycle::Disconnected { .. } => Level::Gone,
            Lifecycle::Live { owner, .. } => match owner.stage {
                Stage::Created { .. } => Level::Hidden,
                Stage::Started { .. } | Stage::Resumed { focused: false } => Level::Visible,
                Stage::Resumed { focused: true } => Level::Active,
            },
        }
    }

    /// The only place the lifecycle changes: state × event → next state and the effect that goes with it.
    /// Each Activity moves through its own stages; the window follows the most recently started one, and GPUI
    /// hears the owner's level change. To add a callback: add the variant, and the compiler lists the arms.
    pub fn next(self, event: ActivityEvent) -> (Self, Effect) {
        use Callback as C;
        use Lifecycle as L;
        use Stage as S;
        let before = self.level();
        let (from, callback, owner, mut below, newest) = match (self, event) {
            (state, ActivityEvent::Create) => {
                let newest = ActivityId(state.newest().map_or(0, |id| id.0).wrapping_add(1));
                let created = Activity {
                    id: newest,
                    stage: S::Created { focused: false },
                };
                let next = match state {
                    L::Loaded | L::Disconnected { .. } => L::Live {
                        owner: created,
                        below: Vec::new(),
                        newest,
                    },
                    L::Live {
                        owner, mut below, ..
                    } => {
                        below.insert(0, created);
                        L::Live {
                            owner,
                            below,
                            newest,
                        }
                    }
                };
                return (next, Effect::Nothing);
            }
            (state @ (L::Loaded | L::Disconnected { .. }), ActivityEvent::From(..)) => {
                let why = "event without a live Activity";
                return (state, Effect::Reject { why });
            }
            (
                L::Live {
                    owner,
                    below,
                    newest,
                },
                ActivityEvent::From(from, callback),
            ) => (from, callback, owner, below, newest),
        };
        let found = std::iter::once(&owner)
            .chain(&below)
            .find(|activity| activity.id == from)
            .map(|activity| activity.stage);
        // One Activity's own step: its next stage, `None` once destroyed.
        let step = match found {
            None => Err("event from a destroyed Activity"),
            Some(stage) => match (stage, callback) {
                (stage, C::TrimMemory(_)) => Ok(Some(stage)),
                (S::Created { focused }, C::Start) => Ok(Some(S::Started { focused })),
                (S::Started { focused }, C::Resume) => Ok(Some(S::Resumed { focused })),
                (S::Resumed { focused }, C::Pause) => Ok(Some(S::Started { focused })),
                (S::Started { focused }, C::Stop) => Ok(Some(S::Created { focused })),
                (S::Created { .. }, C::Destroy) => Ok(None),
                (S::Created { .. }, C::Focus(focused)) => Ok(Some(S::Created { focused })),
                (S::Started { .. }, C::Focus(focused)) => Ok(Some(S::Started { focused })),
                (S::Resumed { .. }, C::Focus(focused)) => Ok(Some(S::Resumed { focused })),
                (S::Started { .. } | S::Resumed { .. }, C::Start) => Err("start while started"),
                (S::Created { .. }, C::Resume) => Err("resume before start"),
                (S::Resumed { .. }, C::Resume) => Err("resume while resumed"),
                (S::Created { .. } | S::Started { .. }, C::Pause) => Err("pause while not resumed"),
                (S::Created { .. }, C::Stop) => Err("stop while not started"),
                (S::Resumed { .. }, C::Stop) => Err("stop before pause"),
                (S::Started { .. } | S::Resumed { .. }, C::Destroy) => Err("destroy before stop"),
            },
        };
        let stage = match step {
            Ok(stage) => stage,
            Err(why) => {
                let state = L::Live {
                    owner,
                    below,
                    newest,
                };
                return (state, Effect::Reject { why });
            }
        };
        let is_owner = from == owner.id;
        let next = match (stage, is_owner) {
            // The owner is gone: the most recently started live Activity takes the window.
            (None, true) => match below.pop() {
                Some(owner) => L::Live {
                    owner,
                    below,
                    newest,
                },
                None => L::Disconnected { newest },
            },
            (None, false) => {
                below.retain(|activity| activity.id != from);
                L::Live {
                    owner,
                    below,
                    newest,
                }
            }
            (Some(stage), true) => L::Live {
                owner: Activity { id: from, stage },
                below,
                newest,
            },
            // The window follows the Activity that most recently ran onStart (back from the back stack, or the
            // newer of two in multi-window).
            (Some(stage), false) if callback == C::Start => {
                below.retain(|activity| activity.id != from);
                below.push(owner);
                L::Live {
                    owner: Activity { id: from, stage },
                    below,
                    newest,
                }
            }
            (Some(stage), false) => {
                for activity in below.iter_mut().filter(|activity| activity.id == from) {
                    activity.stage = stage;
                }
                L::Live {
                    owner,
                    below,
                    newest,
                }
            }
        };
        let effect = match callback {
            // Every live Activity gets onTrimMemory; the owner's alone warns.
            C::TrimMemory(level) if is_owner && level >= TRIM_MEMORY_RUNNING_LOW => {
                Effect::MemoryWarning
            }
            C::TrimMemory(_) => Effect::Nothing,
            C::Start | C::Resume | C::Pause | C::Stop | C::Destroy | C::Focus(_) => {
                match phases(before, next.level()) {
                    [] => Effect::Nothing,
                    phases => Effect::Phases(phases),
                }
            }
        };
        (next, effect)
    }

    /// GPUI's `PlatformWindow::is_active`: the owner is resumed and focused.
    pub fn is_active(&self) -> bool {
        self.level() == Level::Active
    }

    /// GPUI's `PlatformWindow::visibility`: the owner is between `onStart` and `onStop`.
    pub fn visibility(&self) -> WindowVisibility {
        match self.level() {
            Level::Visible | Level::Active => WindowVisibility::Visible,
            Level::Gone | Level::Hidden => WindowVisibility::Hidden,
        }
    }

    /// The phases an observer registered at launch has missed: GPUI launches on the first surface, after
    /// `onStart` and usually `onResume`. Replayed once after launch, as iOS reports them after its scene connects.
    pub fn launch_phases(&self) -> &'static [AppLifecyclePhase] {
        phases(Level::Gone, self.level())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AppLifecyclePhase as P;
    use Callback as C;
    use Effect::{MemoryWarning, Nothing, Phases, Reject};

    /// `A` is the first live Activity in the rows, `B` the one stacked on it, `C3` a third; `OLD` was destroyed.
    const OLD: ActivityId = ActivityId(1);
    const A: ActivityId = ActivityId(2);
    const B: ActivityId = ActivityId(3);
    const C3: ActivityId = ActivityId(4);

    const fn c(focused: bool) -> Stage {
        Stage::Created { focused }
    }
    const fn s(focused: bool) -> Stage {
        Stage::Started { focused }
    }
    const fn r(focused: bool) -> Stage {
        Stage::Resumed { focused }
    }
    const fn of(id: ActivityId, stage: Stage) -> Activity {
        Activity { id, stage }
    }
    fn live(owner: Activity, below: &[Activity], newest: ActivityId) -> Lifecycle {
        Lifecycle::Live {
            owner,
            below: below.to_vec(),
            newest,
        }
    }
    /// A alone.
    fn one(stage: Stage) -> Lifecycle {
        live(of(A, stage), &[], A)
    }
    /// B owns the window with A below it.
    fn on_a(b: Stage, a: Stage) -> Lifecycle {
        live(of(B, b), &[of(A, a)], B)
    }
    /// A owns the window with B below it.
    fn under_b(a: Stage, b: Stage) -> Lifecycle {
        live(of(A, a), &[of(B, b)], B)
    }
    const D: Lifecycle = Lifecycle::Disconnected { newest: A };
    const fn on(callback: Callback) -> ActivityEvent {
        ActivityEvent::From(A, callback)
    }
    const fn by(id: ActivityId, callback: Callback) -> ActivityEvent {
        ActivityEvent::From(id, callback)
    }
    const fn no(why: &'static str) -> Effect {
        Reject { why }
    }

    #[test]
    fn trim_levels() {
        // (ComponentCallbacks2 level name, value, warns?) as Flutter 3.50.1 decides: level >= RUNNING_LOW.
        // The owner's stage does not matter for memory; every stage is checked against every level.
        let levels = [
            ("RUNNING_MODERATE", 5, false),
            ("RUNNING_LOW", 10, true),
            ("RUNNING_CRITICAL", 15, true),
            ("UI_HIDDEN", 20, true),
            ("BACKGROUND", 40, true),
            ("MODERATE", 60, true),
            ("COMPLETE", 80, true),
        ];
        for state in [one(c(false)), one(s(true)), one(r(true))] {
            for (name, level, warns) in levels {
                let effect = if warns { MemoryWarning } else { Nothing };
                assert_eq!(
                    state.clone().next(on(C::TrimMemory(level))),
                    (state.clone(), effect),
                    "{name} in {state:?}"
                );
            }
        }
    }

    #[test]
    fn transitions() {
        // (row name, state, event, expected state, expected effect). One row per (stage, callback) of a lone
        // Activity A, rejected included; then the rows with several live Activities. c/s/r are A's (or B's)
        // Created/Started/Resumed stage with the focus flag; D is every Activity destroyed.
        // Kept one row per line (rustfmt::skip) so the table reads as a table.
        #[rustfmt::skip]
        let cases: Vec<(&str, Lifecycle, ActivityEvent, Lifecycle, Effect)> = vec![
            ("loaded_create_owns_activity_1", Lifecycle::Loaded, ActivityEvent::Create, live(of(ActivityId(1), c(false)), &[], ActivityId(1)), Nothing),
            ("loaded_event_rejected", Lifecycle::Loaded, on(C::Start), Lifecycle::Loaded, no("event without a live Activity")),
            ("disconnected_create_takes_next_id", D, ActivityEvent::Create, live(of(B, c(false)), &[], B), Nothing),
            ("created_start_foregrounds", one(c(false)), on(C::Start), one(s(false)), Phases(&[P::Foreground])),
            ("created_start_keeps_early_focus", one(c(true)), on(C::Start), one(s(true)), Phases(&[P::Foreground])),
            ("created_resume_rejected", one(c(false)), on(C::Resume), one(c(false)), no("resume before start")),
            ("created_pause_rejected", one(c(false)), on(C::Pause), one(c(false)), no("pause while not resumed")),
            ("created_stop_rejected", one(c(false)), on(C::Stop), one(c(false)), no("stop while not started")),
            ("created_destroy_disconnects", one(c(false)), on(C::Destroy), D, Phases(&[P::Disconnected])),
            ("created_focus_remembered", one(c(false)), on(C::Focus(true)), one(c(true)), Nothing),
            ("created_unfocus_remembered", one(c(true)), on(C::Focus(false)), one(c(false)), Nothing),
            ("started_start_rejected", one(s(false)), on(C::Start), one(s(false)), no("start while started")),
            ("started_resume_unfocused_waits", one(s(false)), on(C::Resume), one(r(false)), Nothing),
            ("started_resume_focused_activates", one(s(true)), on(C::Resume), one(r(true)), Phases(&[P::Active])),
            ("started_pause_rejected", one(s(true)), on(C::Pause), one(s(true)), no("pause while not resumed")),
            ("started_stop_backgrounds", one(s(false)), on(C::Stop), one(c(false)), Phases(&[P::Background])),
            ("started_stop_keeps_focus", one(s(true)), on(C::Stop), one(c(true)), Phases(&[P::Background])),
            ("started_destroy_rejected", one(s(false)), on(C::Destroy), one(s(false)), no("destroy before stop")),
            ("started_focus_quiet", one(s(false)), on(C::Focus(true)), one(s(true)), Nothing),
            ("started_unfocus_quiet", one(s(true)), on(C::Focus(false)), one(s(false)), Nothing),
            ("resumed_start_rejected", one(r(true)), on(C::Start), one(r(true)), no("start while started")),
            ("resumed_resume_rejected", one(r(true)), on(C::Resume), one(r(true)), no("resume while resumed")),
            ("resumed_focused_pause_deactivates", one(r(true)), on(C::Pause), one(s(true)), Phases(&[P::Inactive])),
            ("resumed_unfocused_pause_quiet", one(r(false)), on(C::Pause), one(s(false)), Nothing),
            ("resumed_stop_rejected", one(r(true)), on(C::Stop), one(r(true)), no("stop before pause")),
            ("resumed_destroy_rejected", one(r(true)), on(C::Destroy), one(r(true)), no("destroy before stop")),
            ("resumed_focus_activates", one(r(false)), on(C::Focus(true)), one(r(true)), Phases(&[P::Active])),
            ("resumed_unfocus_deactivates", one(r(true)), on(C::Focus(false)), one(r(false)), Phases(&[P::Inactive])),
            ("resumed_focus_again_quiet", one(r(true)), on(C::Focus(true)), one(r(true)), Nothing),
            ("resumed_unfocus_again_quiet", one(r(false)), on(C::Focus(false)), one(r(false)), Nothing),
            ("disconnected_start_rejected", D, on(C::Start), D, no("event without a live Activity")),
            ("disconnected_destroy_rejected", D, on(C::Destroy), D, no("event without a live Activity")),
            ("disconnected_focus_rejected", D, on(C::Focus(true)), D, no("event without a live Activity")),
            // A destroyed Activity's late callbacks (the clear-task relaunch) change nothing; nor do unknown ids.
            ("destroyed_start_rejected", one(c(false)), by(OLD, C::Start), one(c(false)), no("event from a destroyed Activity")),
            ("destroyed_stop_rejected", one(r(true)), by(OLD, C::Stop), one(r(true)), no("event from a destroyed Activity")),
            ("destroyed_destroy_rejected", one(r(true)), by(OLD, C::Destroy), one(r(true)), no("event from a destroyed Activity")),
            ("destroyed_trim_rejected", one(r(true)), by(OLD, C::TrimMemory(80)), one(r(true)), no("event from a destroyed Activity")),
            ("never_created_rejected", one(r(true)), by(B, C::Pause), one(r(true)), no("event from a destroyed Activity")),
            // Several live: a deep link stacks B on A, then back.
            ("stacked_create_waits_below", one(s(true)), ActivityEvent::Create, under_b(s(true), c(false)), Nothing),
            ("stacked_start_takes_window", under_b(s(true), c(false)), by(B, C::Start), on_a(s(false), s(true)), Nothing),
            ("stacked_resume_unfocused_waits", on_a(s(false), s(true)), by(B, C::Resume), on_a(r(false), s(true)), Nothing),
            ("covered_unfocus_quiet", on_a(r(true), s(true)), on(C::Focus(false)), on_a(r(true), s(false)), Nothing),
            ("covered_stop_quiet", on_a(r(true), s(false)), on(C::Stop), on_a(r(true), c(false)), Nothing),
            ("covered_trim_quiet", on_a(r(true), c(false)), on(C::TrimMemory(80)), on_a(r(true), c(false)), Nothing),
            ("covered_rejected_event_rejected", on_a(r(true), c(false)), on(C::Resume), on_a(r(true), c(false)), no("resume before start")),
            ("back_start_takes_window_back", on_a(s(true), c(false)), on(C::Start), under_b(s(false), s(true)), Nothing),
            ("back_resume_activates", under_b(s(true), s(false)), on(C::Resume), under_b(r(true), s(false)), Phases(&[P::Active])),
            ("back_covering_stop_quiet", under_b(r(true), s(false)), by(B, C::Stop), under_b(r(true), c(false)), Nothing),
            ("destroy_of_non_owner_quiet", under_b(r(true), c(false)), by(B, C::Destroy), live(of(A, r(true)), &[], B), Nothing),
            ("create_with_two_live_goes_to_bottom", on_a(r(true), c(false)), ActivityEvent::Create, live(of(B, r(true)), &[of(C3, c(false)), of(A, c(false))], C3), Nothing),
            // Two live in multi-window: the newer start owns the window; the older one's callbacks are quiet.
            ("multi_window_newer_start_deactivates", under_b(r(true), c(false)), by(B, C::Start), on_a(s(false), r(true)), Phases(&[P::Inactive])),
            ("multi_window_newer_focus_activates", on_a(r(false), r(false)), by(B, C::Focus(true)), on_a(r(true), r(false)), Phases(&[P::Active])),
            ("multi_window_older_focus_quiet", on_a(r(true), r(false)), on(C::Focus(true)), on_a(r(true), r(true)), Nothing),
            // The owner destroyed while another is live: the most recently started one takes the window.
            ("destroy_of_owner_falls_back_hidden", on_a(c(false), c(false)), by(B, C::Destroy), live(of(A, c(false)), &[], B), Nothing),
            ("destroy_of_owner_falls_back_active", on_a(c(false), r(true)), by(B, C::Destroy), live(of(A, r(true)), &[], B), Phases(&[P::Foreground, P::Active])),
            ("destroy_of_owner_skips_never_started", live(of(B, c(false)), &[of(C3, c(false)), of(A, s(false))], C3), by(B, C::Destroy), live(of(A, s(false)), &[of(C3, c(false))], C3), Phases(&[P::Foreground])),
        ];
        for (name, state, event, expected_state, expected_effect) in cases {
            assert_eq!(
                state.next(event),
                (expected_state, expected_effect),
                "{name}"
            );
        }
    }

    #[test]
    fn accepts_only_the_owner() {
        // (row name, state, event's Activity, accepted?). Every view event goes through the same rule.
        #[rustfmt::skip]
        let cases: [(&str, Lifecycle, ActivityId, bool); 6] = [
            ("loaded_rejects_any_view", Lifecycle::Loaded, A, false),
            ("owner_accepted", one(r(true)), A, true),
            ("destroyed_rejected", one(r(true)), OLD, false),
            ("covered_rejected", on_a(r(true), c(false)), A, false),
            ("newer_owner_accepted", on_a(r(true), c(false)), B, true),
            ("disconnected_rejects_its_last", D, A, false),
        ];
        for (name, state, from, accepted) in cases {
            assert_eq!(state.accepts(from), accepted, "{name}");
        }
    }

    #[test]
    fn window_and_launch_phases() {
        use WindowVisibility::{Hidden, Visible};
        // (row name, state, is_active, visibility, phases replayed at launch)
        #[rustfmt::skip]
        let cases: [(&str, Lifecycle, bool, WindowVisibility, &[AppLifecyclePhase]); 9] = [
            ("loaded", Lifecycle::Loaded, false, Hidden, &[]),
            ("created", one(c(false)), false, Hidden, &[]),
            ("created_focused", one(c(true)), false, Hidden, &[]),
            ("started", one(s(false)), false, Visible, &[P::Foreground]),
            ("started_focused", one(s(true)), false, Visible, &[P::Foreground]),
            ("resumed_unfocused", one(r(false)), false, Visible, &[P::Foreground]),
            ("resumed_focused", one(r(true)), true, Visible, &[P::Foreground, P::Active]),
            ("owner_counts_not_below", on_a(c(false), r(true)), false, Hidden, &[]),
            ("disconnected", D, false, Hidden, &[]),
        ];
        for (name, state, active, visibility, phases) in cases {
            assert_eq!(
                (state.is_active(), state.visibility(), state.launch_phases()),
                (active, visibility, phases),
                "{name}"
            );
        }
    }

    #[test]
    fn java_codes() {
        // (row name, code, value, callback). Codes must match ParallaxActivity's EVENT_* constants.
        #[rustfmt::skip]
        let cases: [(&str, i32, i32, Option<Callback>); 9] = [
            ("start", 0, 0, Some(C::Start)),
            ("resume", 1, 0, Some(C::Resume)),
            ("pause", 2, 0, Some(C::Pause)),
            ("stop", 3, 0, Some(C::Stop)),
            ("destroy", 4, 0, Some(C::Destroy)),
            ("focus", 5, 1, Some(C::Focus(true))),
            ("unfocus", 5, 0, Some(C::Focus(false))),
            ("trim_memory", 6, 15, Some(C::TrimMemory(15))),
            ("unknown", 7, 0, None),
        ];
        for (name, code, value, callback) in cases {
            assert_eq!(Callback::from_java(code, value), callback, "{name}");
        }
    }
}
