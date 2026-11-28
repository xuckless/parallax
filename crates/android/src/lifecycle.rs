//! The Activity lifecycle as GPUI sees it: one state, one transition function. Pure, so it is tested on any host.
//! The shell in `host.rs` feeds it `ParallaxActivity` callbacks and carries out the returned [`Effect`].
//! Each live Activity owns one GPUI window; its window's activity and visibility follow its own stage, and the app's
//! phase follows the most foreground of them. Phases follow Flutter's `LifecycleChannel`: an Activity is `Active`
//! only while resumed *and* focused, so the notification shade or a system dialog makes it `Inactive` without an
//! `onPause`. How it maps: docs/android-lifecycle.md.

use gpui::{AppLifecyclePhase, WindowVisibility};

/// Which `ParallaxActivity` instance an event comes from. Each `onCreate` gets the next one, never reused, so a
/// destroyed Activity's late callbacks match nothing. Several can be live at once: a deep link stacks a second
/// one on the first, a clear-task relaunch overlaps the old and the new, multi-window shows two.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

/// The app's Activities, each with its own GPUI window.
#[derive(Clone, Debug, PartialEq)]
pub enum Lifecycle {
    /// Before the first `onCreate`: JNI_OnLoad runs inside it, and `Create` follows.
    Loaded,
    /// At least one Activity is live: `activities` in creation order, never empty. `newest` is the last id handed
    /// out.
    Live {
        activities: Vec<Activity>,
        newest: ActivityId,
    },
    /// Every Activity was destroyed. The process and GPUI live on; a new Activity in this process starts again
    /// from here, with the id after `newest`.
    Disconnected { newest: ActivityId },
}

/// What happened, and to which Activity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivityEvent {
    /// `onCreate`: a new live Activity, whose window GPUI opens on its first surface.
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

/// What the shell does for the app after a transition. Each window's activity and visibility follow from the state
/// itself ([`Lifecycle::is_active`], [`Lifecycle::visibility`]); a destroyed Activity's window closes
/// ([`Lifecycle::contains`]).
#[derive(Debug, PartialEq)]
pub enum Effect {
    /// Tell GPUI's `on_app_lifecycle` observers these phases, in order.
    Phases(&'static [AppLifecyclePhase]),
    /// Tell GPUI's `on_memory_warning` observers.
    MemoryWarning,
    /// A legal event that changes nothing the app sees.
    Nothing,
    /// The event makes no sense in this state; nothing changes.
    Reject { why: &'static str },
}

/// `ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW`. Levels at or above it warn, the rule of Flutter 3.50.1's
/// `FlutterActivityAndFragmentDelegate.onTrimMemory` (`level >= TRIM_MEMORY_RUNNING_LOW`), so `UI_HIDDEN` (every
/// Home press) and the background levels warn too; the `trim_levels` test has a row per level. Flutter also skips
/// warnings before its first frame; here GPUI has no observers before launch, and launch is one vsync before it.
const TRIM_MEMORY_RUNNING_LOW: i32 = 10;

/// What GPUI sees of an Activity, lowest first: no Activity, hidden (`Created`), visible (`Started`, or resumed
/// without focus), active (resumed and focused). The app's level is its highest Activity's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Gone,
    Hidden,
    Visible,
    Active,
}

/// The phases heard climbing into `Visible` and `Active`, and falling out of `Active`, `Visible` and `Hidden`.
/// A move of several levels hears each step's phase, so a fallback (the foreground Activity destroyed while a
/// resumed one lives) reports what GPUI missed. To add a level: a row in each, in level order.
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

impl Stage {
    fn level(self) -> Level {
        match self {
            Stage::Created { .. } => Level::Hidden,
            Stage::Started { .. } | Stage::Resumed { focused: false } => Level::Visible,
            Stage::Resumed { focused: true } => Level::Active,
        }
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

    fn activities(&self) -> &[Activity] {
        match self {
            Lifecycle::Live { activities, .. } => activities,
            Lifecycle::Loaded | Lifecycle::Disconnected { .. } => &[],
        }
    }

    fn stage(&self, id: ActivityId) -> Option<Stage> {
        self.activities()
            .iter()
            .find(|activity| activity.id == id)
            .map(|activity| activity.stage)
    }

    /// Whether Activity `id` is live: the one rule for events from a view (surface, frame, insets, touch, keys),
    /// which reach that Activity's window.
    pub fn contains(&self, id: ActivityId) -> bool {
        self.stage(id).is_some()
    }

    fn level_of(&self, id: ActivityId) -> Level {
        self.stage(id).map_or(Level::Gone, Stage::level)
    }

    /// The app's level: its most foreground Activity's.
    fn level(&self) -> Level {
        self.activities()
            .iter()
            .map(|activity| activity.stage.level())
            .max()
            .unwrap_or(Level::Gone)
    }

    /// The only place the lifecycle changes: state × event → next state and the effect that goes with it.
    /// Each Activity moves through its own stages; the app hears its level change. To add a callback: add the
    /// variant, and the compiler lists the arms.
    pub fn next(self, event: ActivityEvent) -> (Self, Effect) {
        use Callback as C;
        use Lifecycle as L;
        use Stage as S;
        let before = self.level();
        let (from, callback, mut activities, newest) = match (self, event) {
            (state, ActivityEvent::Create) => {
                let newest = ActivityId(state.newest().map_or(0, |id| id.0).wrapping_add(1));
                let mut activities = state.activities().to_vec();
                activities.push(Activity {
                    id: newest,
                    stage: S::Created { focused: false },
                });
                return (L::Live { activities, newest }, Effect::Nothing);
            }
            (state @ (L::Loaded | L::Disconnected { .. }), ActivityEvent::From(..)) => {
                let why = "event without a live Activity";
                return (state, Effect::Reject { why });
            }
            (L::Live { activities, newest }, ActivityEvent::From(from, callback)) => {
                (from, callback, activities, newest)
            }
        };
        let found = activities.iter().position(|activity| activity.id == from);
        // One Activity's own step: where it is in `activities` and its next stage, `None` once destroyed.
        let step = match found.map(|index| (index, activities[index].stage)) {
            None => Err("event from a destroyed Activity"),
            Some((index, stage)) => match (stage, callback) {
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
            }
            .map(|stage| (index, stage)),
        };
        let (index, stage) = match step {
            Ok(step) => step,
            Err(why) => return (L::Live { activities, newest }, Effect::Reject { why }),
        };
        let next = match stage {
            Some(stage) => {
                activities[index].stage = stage;
                L::Live { activities, newest }
            }
            None => {
                activities.remove(index);
                match activities.is_empty() {
                    true => L::Disconnected { newest },
                    false => L::Live { activities, newest },
                }
            }
        };
        // Android sends every live Activity the same onTrimMemory; the oldest one's alone warns, once.
        let is_oldest = index == 0;
        let effect = match callback {
            C::TrimMemory(level) if is_oldest && level >= TRIM_MEMORY_RUNNING_LOW => {
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

    /// GPUI's `PlatformWindow::is_active` for Activity `id`'s window: it is resumed and focused.
    pub fn is_active(&self, id: ActivityId) -> bool {
        self.level_of(id) == Level::Active
    }

    /// The Activity whose window is active, if any (GPUI's `Platform::active_window`).
    pub fn active(&self) -> Option<ActivityId> {
        self.activities()
            .iter()
            .find(|activity| activity.stage.level() == Level::Active)
            .map(|activity| activity.id)
    }

    /// GPUI's `PlatformWindow::visibility` for Activity `id`'s window: it is between `onStart` and `onStop`.
    pub fn visibility(&self, id: ActivityId) -> WindowVisibility {
        match self.level_of(id) {
            Level::Visible | Level::Active => WindowVisibility::Visible,
            Level::Gone | Level::Hidden => WindowVisibility::Hidden,
        }
    }

    /// Whether Activity `id`'s view runs frames: Java posts them between `onResume` and `onPause`, and pulls GPUI's
    /// framework calls with each. Haptics are kept only then: Flutter plays none while its app is not in front.
    pub fn runs_frames(&self, id: ActivityId) -> bool {
        match self.stage(id) {
            Some(Stage::Resumed { .. }) => true,
            Some(Stage::Created { .. } | Stage::Started { .. }) | None => false,
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

    /// `A` is the first live Activity in the rows, `B` the second (stacked on it, or beside it in multi-window),
    /// `C3` a third; `OLD` was destroyed.
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
    fn live(activities: &[Activity], newest: ActivityId) -> Lifecycle {
        Lifecycle::Live {
            activities: activities.to_vec(),
            newest,
        }
    }
    /// A alone.
    fn one(stage: Stage) -> Lifecycle {
        live(&[of(A, stage)], A)
    }
    /// A, then B.
    fn two(a: Stage, b: Stage) -> Lifecycle {
        live(&[of(A, a), of(B, b)], B)
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
    fn runs_frames_table() {
        // (row name, lifecycle, Activity, whether its view runs frames and so keeps GPUI's haptics)
        let rows = [
            ("loaded: no view", Lifecycle::Loaded, A, false),
            ("created: stopped", one(c(true)), A, false),
            ("started, or paused: stopFrames ran", one(s(true)), A, false),
            ("resumed without focus: frames run", one(r(false)), A, true),
            ("resumed and focused", one(r(true)), A, true),
            (
                "each Activity its own: A paused",
                two(s(true), r(true)),
                A,
                false,
            ),
            (
                "each Activity its own: B resumed",
                two(s(true), r(true)),
                B,
                true,
            ),
            ("destroyed: no view", one(r(true)), OLD, false),
            ("disconnected: no view", D, A, false),
        ];
        for (name, lifecycle, id, runs) in rows {
            assert_eq!(lifecycle.runs_frames(id), runs, "{name}");
        }
    }

    #[test]
    fn trim_levels() {
        // (ComponentCallbacks2 level name, value, warns?) as Flutter 3.50.1 decides: level >= RUNNING_LOW.
        // The Activity's stage does not matter for memory; every stage is checked against every level.
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
        // Activity A, rejected included; then the rows with several live Activities. c/s/r are an Activity's
        // Created/Started/Resumed stage with the focus flag; D is every Activity destroyed.
        // Kept one row per line (rustfmt::skip) so the table reads as a table.
        #[rustfmt::skip]
        let cases: Vec<(&str, Lifecycle, ActivityEvent, Lifecycle, Effect)> = vec![
            ("loaded_create_makes_activity_1", Lifecycle::Loaded, ActivityEvent::Create, live(&[of(ActivityId(1), c(false))], ActivityId(1)), Nothing),
            ("loaded_event_rejected", Lifecycle::Loaded, on(C::Start), Lifecycle::Loaded, no("event without a live Activity")),
            ("disconnected_create_takes_next_id", D, ActivityEvent::Create, live(&[of(B, c(false))], B), Nothing),
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
            // A deep link stacks B on A: A pauses, B starts and resumes and takes focus, A stops; back reverses it.
            ("second_create_goes_last", one(r(true)), ActivityEvent::Create, two(r(true), c(false)), Nothing),
            ("covered_pause_deactivates", two(r(true), c(false)), on(C::Pause), two(s(true), c(false)), Phases(&[P::Inactive])),
            ("covered_unfocus_quiet", two(s(true), c(false)), on(C::Focus(false)), two(s(false), c(false)), Nothing),
            ("covering_start_quiet", two(s(false), c(false)), by(B, C::Start), two(s(false), s(false)), Nothing),
            ("covering_resume_unfocused_quiet", two(s(false), s(false)), by(B, C::Resume), two(s(false), r(false)), Nothing),
            ("covering_focus_activates", two(s(false), r(false)), by(B, C::Focus(true)), two(s(false), r(true)), Phases(&[P::Active])),
            ("covered_stop_quiet", two(s(false), r(true)), on(C::Stop), two(c(false), r(true)), Nothing),
            ("covered_trim_quiet_for_second", two(c(false), r(true)), by(B, C::TrimMemory(80)), two(c(false), r(true)), Nothing),
            ("covered_trim_warns_for_oldest", two(c(false), r(true)), on(C::TrimMemory(80)), two(c(false), r(true)), MemoryWarning),
            ("covered_rejected_event_rejected", two(c(false), r(true)), on(C::Resume), two(c(false), r(true)), no("resume before start")),
            ("back_covering_pause_deactivates", two(c(false), r(true)), by(B, C::Pause), two(c(false), s(true)), Phases(&[P::Inactive])),
            ("back_covered_start_quiet", two(c(false), s(true)), on(C::Start), two(s(false), s(true)), Nothing),
            ("back_covered_resume_focused_activates", two(s(true), s(false)), on(C::Resume), two(r(true), s(false)), Phases(&[P::Active])),
            ("back_covering_stop_quiet", two(r(true), s(false)), by(B, C::Stop), two(r(true), c(false)), Nothing),
            ("back_covering_destroy_quiet", two(r(true), c(false)), by(B, C::Destroy), live(&[of(A, r(true))], B), Nothing),
            ("third_create_goes_last", two(r(true), c(false)), ActivityEvent::Create, live(&[of(A, r(true)), of(B, c(false)), of(C3, c(false))], C3), Nothing),
            // Split screen: both resumed and visible, focus moves between them; the app stays Foreground.
            ("split_focus_leaves_a", two(r(true), r(false)), on(C::Focus(false)), two(r(false), r(false)), Phases(&[P::Inactive])),
            ("split_focus_reaches_b", two(r(false), r(false)), by(B, C::Focus(true)), two(r(false), r(true)), Phases(&[P::Active])),
            ("split_unfocused_pause_quiet", two(r(false), r(true)), on(C::Pause), two(s(false), r(true)), Nothing),
            ("split_focused_pause_deactivates", two(r(false), r(true)), by(B, C::Pause), two(r(false), s(true)), Phases(&[P::Inactive])),
            ("split_other_stop_keeps_foreground", two(r(false), s(true)), by(B, C::Stop), two(r(false), c(true)), Nothing),
            ("split_other_destroy_quiet", two(r(false), c(true)), by(B, C::Destroy), live(&[of(A, r(false))], B), Nothing),
            ("last_of_two_stop_backgrounds", two(c(false), s(false)), by(B, C::Stop), two(c(false), c(false)), Phases(&[P::Background])),
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
    fn contains_live_activities() {
        // (row name, state, event's Activity, live?). Every view event goes through the same rule.
        #[rustfmt::skip]
        let cases: [(&str, Lifecycle, ActivityId, bool); 6] = [
            ("loaded_has_none", Lifecycle::Loaded, A, false),
            ("lone_activity", one(r(true)), A, true),
            ("destroyed", one(r(true)), OLD, false),
            ("covered_still_live", two(c(false), r(true)), A, true),
            ("covering", two(c(false), r(true)), B, true),
            ("disconnected_has_none", D, A, false),
        ];
        for (name, state, from, live) in cases {
            assert_eq!(state.contains(from), live, "{name}");
        }
    }

    #[test]
    fn windows_and_launch_phases() {
        use WindowVisibility::{Hidden, Visible};
        // (row name, state, Activity, its window's is_active and visibility, the active Activity, the phases
        // replayed at launch)
        type Row = (
            &'static str,
            Lifecycle,
            ActivityId,
            bool,
            WindowVisibility,
            Option<ActivityId>,
            &'static [AppLifecyclePhase],
        );
        #[rustfmt::skip]
        let cases: [Row; 13] = [
            ("loaded", Lifecycle::Loaded, A, false, Hidden, None, &[]),
            ("created", one(c(false)), A, false, Hidden, None, &[]),
            ("created_focused", one(c(true)), A, false, Hidden, None, &[]),
            ("started", one(s(false)), A, false, Visible, None, &[P::Foreground]),
            ("started_focused", one(s(true)), A, false, Visible, None, &[P::Foreground]),
            ("resumed_unfocused", one(r(false)), A, false, Visible, None, &[P::Foreground]),
            ("resumed_focused", one(r(true)), A, true, Visible, Some(A), &[P::Foreground, P::Active]),
            ("covered_window_hidden", two(c(false), r(true)), A, false, Hidden, Some(B), &[P::Foreground, P::Active]),
            ("covering_window_active", two(c(false), r(true)), B, true, Visible, Some(B), &[P::Foreground, P::Active]),
            ("split_unfocused_window_visible", two(r(false), r(true)), A, false, Visible, Some(B), &[P::Foreground, P::Active]),
            ("split_focused_window_active", two(r(false), r(true)), B, true, Visible, Some(B), &[P::Foreground, P::Active]),
            ("destroyed_window_hidden", one(r(true)), OLD, false, Hidden, Some(A), &[P::Foreground, P::Active]),
            ("disconnected", D, A, false, Hidden, None, &[]),
        ];
        for (name, state, id, active, visibility, foreground, phases) in cases {
            assert_eq!(
                (
                    state.is_active(id),
                    state.visibility(id),
                    state.active(),
                    state.launch_phases()
                ),
                (active, visibility, foreground, phases),
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
