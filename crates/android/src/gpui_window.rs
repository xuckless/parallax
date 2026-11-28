//! Where one Activity's GPUI window is: one state, one transition function. Pure, so it is tested on any host.
//! `activity_window.rs` holds one per Activity and carries out the returned [`GpuiEffect`]; how the windows fit
//! together: docs/android-multi-window.md.

/// Where the Activity's GPUI window is. `W` is the open window (its GPUI handle and renderer on Android, a plain
/// value in tests): it lives only in `Open`.
#[derive(Debug, PartialEq)]
pub enum Gpui<W> {
    /// No GPUI window yet: the Activity's first surface asks the app for one.
    Waiting,
    /// The app was asked for this Activity's window (its `open_window`, `android_main!`); the window it opens
    /// (`Platform::open_window`) binds here.
    Requested,
    /// GPUI's window for this Activity.
    Open(W),
    /// The app closed its window (GPUI dropped it, and its renderer with it) and Java was told to finish the
    /// Activity (Flutter's `SystemNavigator.pop`); its `onDestroy` follows.
    Finishing,
}

/// What happened to the Activity's window.
#[derive(Debug)]
pub enum GpuiEvent<W> {
    /// The Activity's view has its first surface.
    FirstSurface,
    /// GPUI opened a window for this Activity (`Platform::open_window`).
    Opened(W),
    /// GPUI dropped the window: the app closed it.
    ClosedByApp,
}

/// What the shell does after a transition.
#[derive(Clone, Debug, PartialEq)]
pub enum GpuiEffect {
    /// Ask the app for this Activity's window.
    RequestWindow,
    /// Call Java's `PlatformPlugin.popSystemNavigator` now, whatever the Activity's stage, so it finishes.
    FinishActivity,
    Nothing,
    /// The event makes no sense in this state; nothing changes.
    Reject {
        why: &'static str,
    },
}

impl<W> Gpui<W> {
    /// The only place the window's state changes: state × event → next state and the effect that goes with it.
    /// To add an event: add the variant, and the compiler lists the arms to write here.
    pub fn next(self, event: GpuiEvent<W>) -> (Self, GpuiEffect) {
        use Gpui as G;
        use GpuiEffect as E;
        use GpuiEvent as V;
        match (self, event) {
            (G::Waiting, V::FirstSurface) => (G::Requested, E::RequestWindow),
            (G::Requested, V::Opened(window)) => (G::Open(window), E::Nothing),
            // Dropping the open window drops its renderer.
            (G::Open(_), V::ClosedByApp) => (G::Finishing, E::FinishActivity),
            (state @ (G::Requested | G::Open(_) | G::Finishing), V::FirstSurface) => (
                state,
                E::Reject {
                    why: "a later surface asks for no window",
                },
            ),
            (state @ (G::Waiting | G::Open(_) | G::Finishing), V::Opened(_)) => (
                state,
                E::Reject {
                    why: "no window was requested for this Activity",
                },
            ),
            // The Activity's own destruction closes its window too, once it is no longer the shell's.
            (state @ (G::Waiting | G::Requested | G::Finishing), V::ClosedByApp) => (
                state,
                E::Reject {
                    why: "no open window to close",
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Gpui, GpuiEffect, GpuiEvent};

    #[test]
    fn transition_table() {
        use Gpui::{Finishing, Open, Requested, Waiting};
        use GpuiEvent::{ClosedByApp, FirstSurface, Opened};
        let reject = |why| GpuiEffect::Reject { why };
        let no_window = reject("no window was requested for this Activity");
        let later_surface = reject("a later surface asks for no window");
        let nothing_open = reject("no open window to close");
        // (state, event, next state, effect): every pair, rejected ones included.
        type Row = (Gpui<u8>, GpuiEvent<u8>, Gpui<u8>, GpuiEffect);
        let rows: [Row; 12] = [
            (Waiting, FirstSurface, Requested, GpuiEffect::RequestWindow),
            (Waiting, Opened(1), Waiting, no_window.clone()),
            (Waiting, ClosedByApp, Waiting, nothing_open.clone()),
            (Requested, FirstSurface, Requested, later_surface.clone()),
            (Requested, Opened(1), Open(1), GpuiEffect::Nothing),
            (Requested, ClosedByApp, Requested, nothing_open.clone()),
            (Open(1), FirstSurface, Open(1), later_surface.clone()),
            (Open(1), Opened(2), Open(1), no_window.clone()),
            (Open(1), ClosedByApp, Finishing, GpuiEffect::FinishActivity),
            (Finishing, FirstSurface, Finishing, later_surface),
            (Finishing, Opened(1), Finishing, no_window),
            (Finishing, ClosedByApp, Finishing, nothing_open),
        ];
        for (state, event, next, effect) in rows {
            let name = format!("{state:?} × {event:?}");
            assert_eq!(state.next(event), (next, effect), "{name}");
        }
    }
}
