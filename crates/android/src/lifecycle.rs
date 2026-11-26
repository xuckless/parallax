//! The Activity lifecycle as GPUI sees it: one state, one transition function. Pure, so it is tested on any host.
//! The shell in `host.rs` feeds it `ParallaxActivity` callbacks and carries out the returned [`Effect`].
//! Phases follow Flutter's `LifecycleChannel`: the app is `Active` only while resumed *and* focused, so the
//! notification shade or a system dialog makes it `Inactive` without an `onPause`. How it maps: docs/android-lifecycle.md.

use gpui::{AppLifecyclePhase, WindowVisibility};

/// Which `ParallaxActivity` instance an event comes from. Each `onCreate` gets the next one; only the newest owns the
/// window. Two instances overlap when Android replaces one (a clear-task relaunch): the old one's `onStop`,
/// `onDestroy` and `surfaceDestroyed` arrive after the new one started, and must not touch the new one's window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivityId(pub i32);

/// Where the owning Activity is. `focused` is the last `onWindowFocusChanged`: kept in every state of a live
/// Activity, because focus and lifecycle callbacks arrive in no fixed order (Flutter's `LifecycleChannel` says so).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lifecycle {
    /// After `onCreate` or `onStop`: not visible. The surface is gone or about to go.
    Created { activity: ActivityId, focused: bool },
    /// After `onStart` or `onPause`: visible, not resumed.
    Started { activity: ActivityId, focused: bool },
    /// After `onResume`.
    Resumed { activity: ActivityId, focused: bool },
    /// Before the first `onCreate` (`ActivityId(0)`), or after `onDestroy` of `activity`. The process and GPUI live
    /// on; a new Activity in this process starts again from here.
    Disconnected { activity: ActivityId },
}

/// What happened, and to which Activity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivityEvent {
    /// `onCreate`: a new Activity owns the window from now on.
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
    /// Tell GPUI's `on_app_lifecycle` observers.
    Phase(AppLifecyclePhase),
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
    /// Before the first `onCreate`: JNI_OnLoad runs inside it, and `Create` follows.
    pub const LOADED: Lifecycle = Lifecycle::Disconnected {
        activity: ActivityId(0),
    };

    /// The Activity that owns the window: the newest one created.
    pub fn activity(self) -> ActivityId {
        match self {
            Lifecycle::Created { activity, .. }
            | Lifecycle::Started { activity, .. }
            | Lifecycle::Resumed { activity, .. }
            | Lifecycle::Disconnected { activity } => activity,
        }
    }

    /// The one rule for stale events, lifecycle and surface alike: only the owning Activity's events count.
    pub fn accepts(self, from: ActivityId) -> bool {
        from == self.activity()
    }

    /// The only place the lifecycle changes: state × event → next state and the effect that goes with it.
    /// To add an event: add the variant, and the compiler lists the arms to write here.
    pub fn next(self, event: ActivityEvent) -> (Self, Effect) {
        use AppLifecyclePhase as P;
        use Callback as C;
        use Effect::{MemoryWarning, Nothing, Phase, Reject};
        use Lifecycle as S;
        let reject = |why| (self, Reject { why });
        let (from, callback) = match event {
            ActivityEvent::Create => {
                let activity = ActivityId(self.activity().0.wrapping_add(1));
                let created = S::Created {
                    activity,
                    focused: false,
                };
                // A replaced Activity that is still visible leaves GPUI's window hidden until the new one starts.
                let effect = match self {
                    S::Started { .. } | S::Resumed { .. } => Phase(P::Background),
                    S::Created { .. } | S::Disconnected { .. } => Nothing,
                };
                return (created, effect);
            }
            ActivityEvent::From(from, _) if !self.accepts(from) => {
                return reject("event from a replaced Activity");
            }
            ActivityEvent::From(from, callback) => (from, callback),
        };
        let activity = from;
        match (self, callback) {
            // Memory pressure changes no state, in any state.
            (state, C::TrimMemory(level)) if level >= TRIM_MEMORY_RUNNING_LOW => {
                (state, MemoryWarning)
            }
            (state, C::TrimMemory(_)) => (state, Nothing),
            (S::Created { focused, .. }, C::Start) => {
                (S::Started { activity, focused }, Phase(P::Foreground))
            }
            (S::Started { focused, .. }, C::Resume) => (
                S::Resumed { activity, focused },
                if focused { Phase(P::Active) } else { Nothing },
            ),
            (S::Resumed { focused, .. }, C::Pause) => (
                S::Started { activity, focused },
                if focused { Phase(P::Inactive) } else { Nothing },
            ),
            (S::Started { focused, .. }, C::Stop) => {
                (S::Created { activity, focused }, Phase(P::Background))
            }
            (S::Created { .. }, C::Destroy) => {
                (S::Disconnected { activity }, Phase(P::Disconnected))
            }
            (S::Created { .. }, C::Focus(focused)) => (S::Created { activity, focused }, Nothing),
            (S::Started { .. }, C::Focus(focused)) => (S::Started { activity, focused }, Nothing),
            (S::Resumed { focused: was, .. }, C::Focus(focused)) => (
                S::Resumed { activity, focused },
                match (was, focused) {
                    (false, true) => Phase(P::Active),
                    (true, false) => Phase(P::Inactive),
                    (false, false) | (true, true) => Nothing,
                },
            ),
            (S::Disconnected { .. }, C::Start) => reject("start without onCreate"),
            (S::Started { .. } | S::Resumed { .. }, C::Start) => reject("start while started"),
            (S::Created { .. } | S::Disconnected { .. }, C::Resume) => {
                reject("resume before start")
            }
            (S::Resumed { .. }, C::Resume) => reject("resume while resumed"),
            (S::Created { .. } | S::Started { .. } | S::Disconnected { .. }, C::Pause) => {
                reject("pause while not resumed")
            }
            (S::Created { .. } | S::Disconnected { .. }, C::Stop) => {
                reject("stop while not started")
            }
            (S::Resumed { .. }, C::Stop) => reject("stop before pause"),
            (S::Started { .. } | S::Resumed { .. }, C::Destroy) => reject("destroy before stop"),
            (S::Disconnected { .. }, C::Destroy) => reject("destroy while disconnected"),
            (S::Disconnected { .. }, C::Focus(_)) => reject("focus without an Activity"),
        }
    }

    /// GPUI's `PlatformWindow::is_active`: resumed and focused.
    pub fn is_active(self) -> bool {
        matches!(self, Lifecycle::Resumed { focused: true, .. })
    }

    /// GPUI's `PlatformWindow::visibility`: between `onStart` and `onStop`.
    pub fn visibility(self) -> WindowVisibility {
        match self {
            Lifecycle::Started { .. } | Lifecycle::Resumed { .. } => WindowVisibility::Visible,
            Lifecycle::Created { .. } | Lifecycle::Disconnected { .. } => WindowVisibility::Hidden,
        }
    }

    /// The phases an observer registered at launch has missed: GPUI launches on the first surface, after
    /// `onStart` and usually `onResume`. Replayed once after launch, as iOS reports them after its scene connects.
    pub fn launch_phases(self) -> &'static [AppLifecyclePhase] {
        use AppLifecyclePhase as P;
        match self {
            Lifecycle::Created { .. } | Lifecycle::Disconnected { .. } => &[],
            Lifecycle::Started { .. } | Lifecycle::Resumed { focused: false, .. } => {
                &[P::Foreground]
            }
            Lifecycle::Resumed { focused: true, .. } => &[P::Foreground, P::Active],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AppLifecyclePhase as P;
    use Callback as C;
    use Effect::{MemoryWarning, Nothing, Phase, Reject};

    /// The owning Activity in the rows; `OLD` is the one it replaced.
    const A: ActivityId = ActivityId(2);
    const OLD: ActivityId = ActivityId(1);
    const D: Lifecycle = Lifecycle::Disconnected { activity: A };

    const fn c(focused: bool) -> Lifecycle {
        Lifecycle::Created {
            activity: A,
            focused,
        }
    }
    const fn s(focused: bool) -> Lifecycle {
        Lifecycle::Started {
            activity: A,
            focused,
        }
    }
    const fn r(focused: bool) -> Lifecycle {
        Lifecycle::Resumed {
            activity: A,
            focused,
        }
    }
    const fn on(callback: Callback) -> ActivityEvent {
        ActivityEvent::From(A, callback)
    }
    const fn old(callback: Callback) -> ActivityEvent {
        ActivityEvent::From(OLD, callback)
    }
    const fn no(why: &'static str) -> Effect {
        Reject { why }
    }

    #[test]
    fn trim_levels() {
        // (ComponentCallbacks2 level name, value, warns?) as Flutter 3.50.1 decides: level >= RUNNING_LOW.
        // The state does not matter for memory; every state is checked against every level.
        let levels = [
            ("RUNNING_MODERATE", 5, false),
            ("RUNNING_LOW", 10, true),
            ("RUNNING_CRITICAL", 15, true),
            ("UI_HIDDEN", 20, true),
            ("BACKGROUND", 40, true),
            ("MODERATE", 60, true),
            ("COMPLETE", 80, true),
        ];
        for state in [c(false), s(true), r(true), D] {
            for (name, level, warns) in levels {
                let effect = if warns { MemoryWarning } else { Nothing };
                assert_eq!(
                    state.next(on(C::TrimMemory(level))),
                    (state, effect),
                    "{name} in {state:?}"
                );
            }
        }
    }

    #[test]
    fn transitions() {
        let created_by = |activity| Lifecycle::Created {
            activity,
            focused: false,
        };
        // (row name, state, event, expected state, expected effect). One row per (state, event), rejected
        // included; c/s/r are Created/Started/Resumed of Activity A with the focus flag, D is A disconnected.
        // Kept one row per line (rustfmt::skip) so the table reads as a table.
        #[rustfmt::skip]
        let cases: [(&str, Lifecycle, ActivityEvent, Lifecycle, Effect); 48] = [
            ("loaded_create_owns_activity_1", Lifecycle::LOADED, ActivityEvent::Create, created_by(ActivityId(1)), Nothing),
            ("created_create_replaces_quietly", c(true), ActivityEvent::Create, created_by(ActivityId(3)), Nothing),
            ("started_create_replaces_and_hides", s(true), ActivityEvent::Create, created_by(ActivityId(3)), Phase(P::Background)),
            ("resumed_create_replaces_and_hides", r(true), ActivityEvent::Create, created_by(ActivityId(3)), Phase(P::Background)),
            ("disconnected_create_reconnects", D, ActivityEvent::Create, created_by(ActivityId(3)), Nothing),
            ("created_start_foregrounds", c(false), on(C::Start), s(false), Phase(P::Foreground)),
            ("created_start_keeps_early_focus", c(true), on(C::Start), s(true), Phase(P::Foreground)),
            ("created_resume_rejected", c(false), on(C::Resume), c(false), no("resume before start")),
            ("created_pause_rejected", c(false), on(C::Pause), c(false), no("pause while not resumed")),
            ("created_stop_rejected", c(false), on(C::Stop), c(false), no("stop while not started")),
            ("created_destroy_disconnects", c(false), on(C::Destroy), D, Phase(P::Disconnected)),
            ("created_focus_remembered", c(false), on(C::Focus(true)), c(true), Nothing),
            ("created_unfocus_remembered", c(true), on(C::Focus(false)), c(false), Nothing),
            ("started_start_rejected", s(false), on(C::Start), s(false), no("start while started")),
            ("started_resume_unfocused_waits", s(false), on(C::Resume), r(false), Nothing),
            ("started_resume_focused_activates", s(true), on(C::Resume), r(true), Phase(P::Active)),
            ("started_pause_rejected", s(true), on(C::Pause), s(true), no("pause while not resumed")),
            ("started_stop_backgrounds", s(false), on(C::Stop), c(false), Phase(P::Background)),
            ("started_stop_keeps_focus", s(true), on(C::Stop), c(true), Phase(P::Background)),
            ("started_destroy_rejected", s(false), on(C::Destroy), s(false), no("destroy before stop")),
            ("started_focus_quiet", s(false), on(C::Focus(true)), s(true), Nothing),
            ("started_unfocus_quiet", s(true), on(C::Focus(false)), s(false), Nothing),
            ("resumed_start_rejected", r(true), on(C::Start), r(true), no("start while started")),
            ("resumed_resume_rejected", r(true), on(C::Resume), r(true), no("resume while resumed")),
            ("resumed_focused_pause_deactivates", r(true), on(C::Pause), s(true), Phase(P::Inactive)),
            ("resumed_unfocused_pause_quiet", r(false), on(C::Pause), s(false), Nothing),
            ("resumed_stop_rejected", r(true), on(C::Stop), r(true), no("stop before pause")),
            ("resumed_destroy_rejected", r(true), on(C::Destroy), r(true), no("destroy before stop")),
            ("resumed_focus_activates", r(false), on(C::Focus(true)), r(true), Phase(P::Active)),
            ("resumed_unfocus_deactivates", r(true), on(C::Focus(false)), r(false), Phase(P::Inactive)),
            ("resumed_focus_again_quiet", r(true), on(C::Focus(true)), r(true), Nothing),
            ("resumed_unfocus_again_quiet", r(false), on(C::Focus(false)), r(false), Nothing),
            ("disconnected_start_rejected", D, on(C::Start), D, no("start without onCreate")),
            ("disconnected_resume_rejected", D, on(C::Resume), D, no("resume before start")),
            ("disconnected_pause_rejected", D, on(C::Pause), D, no("pause while not resumed")),
            ("disconnected_stop_rejected", D, on(C::Stop), D, no("stop while not started")),
            ("disconnected_destroy_rejected", D, on(C::Destroy), D, no("destroy while disconnected")),
            ("disconnected_focus_rejected", D, on(C::Focus(true)), D, no("focus without an Activity")),
            ("disconnected_unfocus_rejected", D, on(C::Focus(false)), D, no("focus without an Activity")),
            // The replaced Activity's late callbacks (the clear-task relaunch) change nothing.
            ("stale_start_rejected", c(false), old(C::Start), c(false), no("event from a replaced Activity")),
            ("stale_resume_rejected", s(false), old(C::Resume), s(false), no("event from a replaced Activity")),
            ("stale_pause_rejected", r(true), old(C::Pause), r(true), no("event from a replaced Activity")),
            ("stale_stop_rejected", r(true), old(C::Stop), r(true), no("event from a replaced Activity")),
            ("stale_destroy_rejected", r(true), old(C::Destroy), r(true), no("event from a replaced Activity")),
            ("stale_unfocus_rejected", r(true), old(C::Focus(false)), r(true), no("event from a replaced Activity")),
            ("stale_trim_rejected", r(true), old(C::TrimMemory(80)), r(true), no("event from a replaced Activity")),
            ("stale_destroy_after_disconnect_rejected", D, old(C::Destroy), D, no("event from a replaced Activity")),
            ("newer_than_owner_rejected", r(true), ActivityEvent::From(ActivityId(3), C::Pause), r(true), no("event from a replaced Activity")),
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
        // (row name, state, event's Activity, accepted?). Surface events go through the same rule.
        #[rustfmt::skip]
        let cases: [(&str, Lifecycle, ActivityId, bool); 6] = [
            ("loaded_rejects_any_view", Lifecycle::LOADED, A, false),
            ("resumed_accepts_owner", r(true), A, true),
            ("resumed_rejects_replaced", r(true), OLD, false),
            ("created_rejects_replaced", c(false), OLD, false),
            ("disconnected_accepts_its_last_surface_destroyed", D, A, true),
            ("disconnected_rejects_older", D, OLD, false),
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
        let cases: [(&str, Lifecycle, bool, WindowVisibility, &[AppLifecyclePhase]); 7] = [
            ("created", c(false), false, Hidden, &[]),
            ("created_focused", c(true), false, Hidden, &[]),
            ("started", s(false), false, Visible, &[P::Foreground]),
            ("started_focused", s(true), false, Visible, &[P::Foreground]),
            ("resumed_unfocused", r(false), false, Visible, &[P::Foreground]),
            ("resumed_focused", r(true), true, Visible, &[P::Foreground, P::Active]),
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
