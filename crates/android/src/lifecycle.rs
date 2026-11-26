//! The Activity lifecycle as GPUI sees it: one state, one transition function. Pure, so it is tested on any host.
//! The shell in `host.rs` feeds it `ParallaxActivity` callbacks and carries out the returned [`Effect`].
//! Phases follow Flutter's `LifecycleChannel`: the app is `Active` only while resumed *and* focused, so the
//! notification shade or a system dialog makes it `Inactive` without an `onPause`. How it maps: docs/android-lifecycle.md.

use gpui::{AppLifecyclePhase, WindowVisibility};

/// Where the Activity is. `focused` is the last `onWindowFocusChanged`: kept in every state of a live Activity,
/// because focus and lifecycle callbacks arrive in no fixed order (Flutter's `LifecycleChannel` says so too).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lifecycle {
    /// After `onCreate` or `onStop`: not visible. The surface is gone or about to go.
    Created { focused: bool },
    /// After `onStart` or `onPause`: visible, not resumed.
    Started { focused: bool },
    /// After `onResume`.
    Resumed { focused: bool },
    /// After `onDestroy`. The process and GPUI live on; a new Activity in this process starts again from here.
    Disconnected,
}

/// One per `ParallaxActivity` callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActivityEvent {
    Start,
    Resume,
    Pause,
    Stop,
    Destroy,
    /// `onWindowFocusChanged(hasFocus)`.
    Focus(bool),
    /// `onTrimMemory(level)`.
    TrimMemory(i32),
    /// `onLowMemory()`.
    LowMemory,
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

/// `ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW`. Levels at or above it warn, as Flutter's
/// `FlutterActivityAndFragmentDelegate.onTrimMemory` does (it includes `UI_HIDDEN` and the background levels;
/// Flutter also waits for its first frame, to keep a GC off startup; GPUI has no GC, so that part is dropped).
const TRIM_MEMORY_RUNNING_LOW: i32 = 10;

/// `ParallaxActivity`'s event codes (its `EVENT_*` constants) → events; `value` is the focus flag or trim level.
/// To add a callback: add a constant in `ParallaxActivity.java`, a row here, and the variant above.
impl ActivityEvent {
    pub fn from_java(code: i32, value: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Start),
            1 => Some(Self::Resume),
            2 => Some(Self::Pause),
            3 => Some(Self::Stop),
            4 => Some(Self::Destroy),
            5 => Some(Self::Focus(value != 0)),
            6 => Some(Self::TrimMemory(value)),
            7 => Some(Self::LowMemory),
            _ => None,
        }
    }
}

impl Lifecycle {
    /// The only place the lifecycle changes: state × event → next state and the effect that goes with it.
    /// To add an event: add the variant, and the compiler lists the arms to write here.
    pub fn next(self, event: ActivityEvent) -> (Self, Effect) {
        use ActivityEvent as E;
        use AppLifecyclePhase as P;
        use Effect::{MemoryWarning, Nothing, Phase, Reject};
        use Lifecycle as S;
        let reject = |why| (self, Reject { why });
        match (self, event) {
            // Memory pressure changes no state, in any state.
            (state, E::TrimMemory(level)) if level >= TRIM_MEMORY_RUNNING_LOW => {
                (state, MemoryWarning)
            }
            (state, E::TrimMemory(_)) => (state, Nothing),
            (state, E::LowMemory) => (state, MemoryWarning),
            (S::Created { focused }, E::Start) => (S::Started { focused }, Phase(P::Foreground)),
            // A new Activity in the same process: its window has not been focused yet.
            (S::Disconnected, E::Start) => (S::Started { focused: false }, Phase(P::Foreground)),
            (S::Started { focused }, E::Resume) => (
                S::Resumed { focused },
                if focused { Phase(P::Active) } else { Nothing },
            ),
            (S::Resumed { focused }, E::Pause) => (
                S::Started { focused },
                if focused { Phase(P::Inactive) } else { Nothing },
            ),
            (S::Started { focused }, E::Stop) => (S::Created { focused }, Phase(P::Background)),
            (S::Created { .. }, E::Destroy) => (S::Disconnected, Phase(P::Disconnected)),
            (S::Created { .. }, E::Focus(focused)) => (S::Created { focused }, Nothing),
            (S::Started { .. }, E::Focus(focused)) => (S::Started { focused }, Nothing),
            (S::Resumed { focused: was }, E::Focus(focused)) => (
                S::Resumed { focused },
                match (was, focused) {
                    (false, true) => Phase(P::Active),
                    (true, false) => Phase(P::Inactive),
                    (false, false) | (true, true) => Nothing,
                },
            ),
            (S::Started { .. } | S::Resumed { .. }, E::Start) => reject("start while started"),
            (S::Created { .. } | S::Disconnected, E::Resume) => reject("resume before start"),
            (S::Resumed { .. }, E::Resume) => reject("resume while resumed"),
            (S::Created { .. } | S::Started { .. } | S::Disconnected, E::Pause) => {
                reject("pause while not resumed")
            }
            (S::Created { .. } | S::Disconnected, E::Stop) => reject("stop while not started"),
            (S::Resumed { .. }, E::Stop) => reject("stop before pause"),
            (S::Started { .. } | S::Resumed { .. }, E::Destroy) => reject("destroy before stop"),
            (S::Disconnected, E::Destroy) => reject("destroy while disconnected"),
            (S::Disconnected, E::Focus(_)) => reject("focus without an Activity"),
        }
    }

    /// GPUI's `PlatformWindow::is_active`: resumed and focused.
    pub fn is_active(self) -> bool {
        matches!(self, Lifecycle::Resumed { focused: true })
    }

    /// GPUI's `PlatformWindow::visibility`: between `onStart` and `onStop`.
    pub fn visibility(self) -> WindowVisibility {
        match self {
            Lifecycle::Started { .. } | Lifecycle::Resumed { .. } => WindowVisibility::Visible,
            Lifecycle::Created { .. } | Lifecycle::Disconnected => WindowVisibility::Hidden,
        }
    }

    /// The phases an observer registered at launch has missed: GPUI launches on the first surface, after
    /// `onStart` and usually `onResume`. Replayed once after launch, as iOS reports them after its scene connects.
    pub fn launch_phases(self) -> &'static [AppLifecyclePhase] {
        use AppLifecyclePhase as P;
        match self {
            Lifecycle::Created { .. } | Lifecycle::Disconnected => &[],
            Lifecycle::Started { .. } | Lifecycle::Resumed { focused: false } => &[P::Foreground],
            Lifecycle::Resumed { focused: true } => &[P::Foreground, P::Active],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ActivityEvent as E;
    use AppLifecyclePhase as P;
    use Effect::{MemoryWarning, Nothing, Phase, Reject};
    use Lifecycle::Disconnected as D;

    const fn c(focused: bool) -> Lifecycle {
        Lifecycle::Created { focused }
    }
    const fn s(focused: bool) -> Lifecycle {
        Lifecycle::Started { focused }
    }
    const fn r(focused: bool) -> Lifecycle {
        Lifecycle::Resumed { focused }
    }
    const fn no(why: &'static str) -> Effect {
        Reject { why }
    }

    #[test]
    fn transitions() {
        // (row name, state, event, expected state, expected effect). One row per (state, event), rejected
        // included; c/s/r are Created/Started/Resumed with the focus flag, D is Disconnected. Kept one row per
        // line (rustfmt::skip) so the table reads as a table.
        #[rustfmt::skip]
        let cases: [(&str, Lifecycle, ActivityEvent, Lifecycle, Effect); 47] = [
            ("created_start_foregrounds", c(false), E::Start, s(false), Phase(P::Foreground)),
            ("created_start_keeps_early_focus", c(true), E::Start, s(true), Phase(P::Foreground)),
            ("created_resume_rejected", c(false), E::Resume, c(false), no("resume before start")),
            ("created_pause_rejected", c(false), E::Pause, c(false), no("pause while not resumed")),
            ("created_stop_rejected", c(false), E::Stop, c(false), no("stop while not started")),
            ("created_destroy_disconnects", c(false), E::Destroy, D, Phase(P::Disconnected)),
            ("created_focus_remembered", c(false), E::Focus(true), c(true), Nothing),
            ("created_unfocus_remembered", c(true), E::Focus(false), c(false), Nothing),
            ("created_trim_running_low_warns", c(false), E::TrimMemory(10), c(false), MemoryWarning),
            ("created_trim_complete_warns", c(false), E::TrimMemory(80), c(false), MemoryWarning),
            ("created_trim_below_running_low_quiet", c(true), E::TrimMemory(9), c(true), Nothing),
            ("created_low_memory_warns", c(false), E::LowMemory, c(false), MemoryWarning),
            ("started_start_rejected", s(false), E::Start, s(false), no("start while started")),
            ("started_resume_unfocused_waits", s(false), E::Resume, r(false), Nothing),
            ("started_resume_focused_activates", s(true), E::Resume, r(true), Phase(P::Active)),
            ("started_pause_rejected", s(true), E::Pause, s(true), no("pause while not resumed")),
            ("started_stop_backgrounds", s(false), E::Stop, c(false), Phase(P::Background)),
            ("started_stop_keeps_focus", s(true), E::Stop, c(true), Phase(P::Background)),
            ("started_destroy_rejected", s(false), E::Destroy, s(false), no("destroy before stop")),
            ("started_focus_quiet", s(false), E::Focus(true), s(true), Nothing),
            ("started_unfocus_quiet", s(true), E::Focus(false), s(false), Nothing),
            ("started_trim_ui_hidden_warns", s(true), E::TrimMemory(20), s(true), MemoryWarning),
            ("started_trim_moderate_quiet", s(true), E::TrimMemory(5), s(true), Nothing),
            ("started_low_memory_warns", s(true), E::LowMemory, s(true), MemoryWarning),
            ("resumed_start_rejected", r(true), E::Start, r(true), no("start while started")),
            ("resumed_resume_rejected", r(true), E::Resume, r(true), no("resume while resumed")),
            ("resumed_focused_pause_deactivates", r(true), E::Pause, s(true), Phase(P::Inactive)),
            ("resumed_unfocused_pause_quiet", r(false), E::Pause, s(false), Nothing),
            ("resumed_stop_rejected", r(true), E::Stop, r(true), no("stop before pause")),
            ("resumed_destroy_rejected", r(true), E::Destroy, r(true), no("destroy before stop")),
            ("resumed_focus_activates", r(false), E::Focus(true), r(true), Phase(P::Active)),
            ("resumed_unfocus_deactivates", r(true), E::Focus(false), r(false), Phase(P::Inactive)),
            ("resumed_focus_again_quiet", r(true), E::Focus(true), r(true), Nothing),
            ("resumed_unfocus_again_quiet", r(false), E::Focus(false), r(false), Nothing),
            ("resumed_trim_running_low_warns", r(true), E::TrimMemory(10), r(true), MemoryWarning),
            ("resumed_trim_moderate_quiet", r(true), E::TrimMemory(5), r(true), Nothing),
            ("resumed_low_memory_warns", r(true), E::LowMemory, r(true), MemoryWarning),
            ("disconnected_start_reconnects", D, E::Start, s(false), Phase(P::Foreground)),
            ("disconnected_resume_rejected", D, E::Resume, D, no("resume before start")),
            ("disconnected_pause_rejected", D, E::Pause, D, no("pause while not resumed")),
            ("disconnected_stop_rejected", D, E::Stop, D, no("stop while not started")),
            ("disconnected_destroy_rejected", D, E::Destroy, D, no("destroy while disconnected")),
            ("disconnected_focus_rejected", D, E::Focus(true), D, no("focus without an Activity")),
            ("disconnected_unfocus_rejected", D, E::Focus(false), D, no("focus without an Activity")),
            ("disconnected_trim_warns", D, E::TrimMemory(80), D, MemoryWarning),
            ("disconnected_trim_quiet", D, E::TrimMemory(5), D, Nothing),
            ("disconnected_low_memory_warns", D, E::LowMemory, D, MemoryWarning),
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
        // (row name, code, value, event). Codes must match ParallaxActivity's EVENT_* constants.
        #[rustfmt::skip]
        let cases: [(&str, i32, i32, Option<ActivityEvent>); 10] = [
            ("start", 0, 0, Some(E::Start)),
            ("resume", 1, 0, Some(E::Resume)),
            ("pause", 2, 0, Some(E::Pause)),
            ("stop", 3, 0, Some(E::Stop)),
            ("destroy", 4, 0, Some(E::Destroy)),
            ("focus", 5, 1, Some(E::Focus(true))),
            ("unfocus", 5, 0, Some(E::Focus(false))),
            ("trim_memory", 6, 15, Some(E::TrimMemory(15))),
            ("low_memory", 7, 0, Some(E::LowMemory)),
            ("unknown", 8, 0, None),
        ];
        for (name, code, value, event) in cases {
            assert_eq!(ActivityEvent::from_java(code, value), event, "{name}");
        }
    }
}
