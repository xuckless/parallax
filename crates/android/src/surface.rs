//! The Android surface lifecycle: one state, one transition function. Pure, so it is tested on any host.
//! The shell in `host.rs` feeds it `SurfaceHolder.Callback` events and carries out the returned [`Action`].

use std::time::Instant;

/// Where the app's drawing surface is. `W` is the native window (`ANativeWindow` on Android, a plain value in tests).
#[derive(Debug, PartialEq)]
pub enum Surface<W> {
    /// No surface since launch: the `SurfaceView` has not created one yet.
    None,
    /// A native window the renderer draws into.
    Ready(W),
    /// Android destroyed the surface (Home, screen off, Activity recreated); drawing waits for the next one.
    Lost { since: Instant },
}

/// What Android or the GPU reported: one per `SurfaceHolder.Callback` method, plus GPU device loss.
#[derive(Debug)]
pub enum SurfaceEvent<W> {
    /// `surfaceCreated`, with the new native window.
    Created(W),
    /// `surfaceChanged`: new size (rotation, split screen) on the same window.
    Resized,
    /// `surfaceDestroyed`: the window must be released before the callback returns.
    Destroyed { at: Instant },
    /// The renderer reported its GPU device lost.
    DeviceLost,
}

/// What the shell does to the renderer after a transition.
#[derive(Debug, PartialEq)]
pub enum Action<W> {
    /// Point the renderer at the new window. `previous`, if any, is released once the renderer has let go of it.
    Attach { previous: Option<W> },
    /// Resize the renderer to the current surface size.
    Resize,
    /// Unconfigure the renderer, then release `window`.
    Detach { window: W },
    /// Recreate the GPU device on the current window.
    RecoverDevice,
    /// The event makes no sense in this state; nothing changes.
    Reject { why: &'static str },
}

impl<W> Surface<W> {
    /// The only place the surface changes: state × event → next state and the action that goes with it.
    /// To add an event: add the variant, and the compiler lists the arms to write here.
    pub fn next(self, event: SurfaceEvent<W>) -> (Self, Action<W>) {
        use Action as A;
        use Surface as S;
        use SurfaceEvent as E;
        match (self, event) {
            (S::None | S::Lost { .. }, E::Created(window)) => {
                (S::Ready(window), A::Attach { previous: None })
            }
            (S::Ready(previous), E::Created(window)) => (
                S::Ready(window),
                A::Attach {
                    previous: Some(previous),
                },
            ),
            (S::Ready(window), E::Resized) => (S::Ready(window), A::Resize),
            (S::Ready(window), E::Destroyed { at }) => {
                (S::Lost { since: at }, A::Detach { window })
            }
            (S::Ready(window), E::DeviceLost) => (S::Ready(window), A::RecoverDevice),
            (state @ (S::None | S::Lost { .. }), E::Resized) => (
                state,
                A::Reject {
                    why: "resized without a surface",
                },
            ),
            (state @ (S::None | S::Lost { .. }), E::Destroyed { .. }) => (
                state,
                A::Reject {
                    why: "destroyed without a surface",
                },
            ),
            // The renderer is checked again on the next frame after a surface arrives.
            (state @ (S::None | S::Lost { .. }), E::DeviceLost) => (
                state,
                A::Reject {
                    why: "device lost without a surface",
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn transitions() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_secs(1);
        let lost = || Surface::Lost { since: t0 };
        // (row name, state, event, expected state, expected action). Windows are plain numbers.
        type Row = (
            &'static str,
            Surface<u8>,
            SurfaceEvent<u8>,
            Surface<u8>,
            Action<u8>,
        );
        let cases: [Row; 12] = [
            (
                "none_created_attaches",
                Surface::None,
                SurfaceEvent::Created(1),
                Surface::Ready(1),
                Action::Attach { previous: None },
            ),
            (
                "ready_created_replaces",
                Surface::Ready(1),
                SurfaceEvent::Created(2),
                Surface::Ready(2),
                Action::Attach { previous: Some(1) },
            ),
            (
                "lost_created_reattaches",
                lost(),
                SurfaceEvent::Created(2),
                Surface::Ready(2),
                Action::Attach { previous: None },
            ),
            (
                "ready_resized_resizes",
                Surface::Ready(1),
                SurfaceEvent::Resized,
                Surface::Ready(1),
                Action::Resize,
            ),
            (
                "none_resized_rejected",
                Surface::None,
                SurfaceEvent::Resized,
                Surface::None,
                Action::Reject {
                    why: "resized without a surface",
                },
            ),
            (
                "lost_resized_rejected",
                lost(),
                SurfaceEvent::Resized,
                lost(),
                Action::Reject {
                    why: "resized without a surface",
                },
            ),
            (
                "ready_destroyed_detaches",
                Surface::Ready(1),
                SurfaceEvent::Destroyed { at: t1 },
                Surface::Lost { since: t1 },
                Action::Detach { window: 1 },
            ),
            (
                "none_destroyed_rejected",
                Surface::None,
                SurfaceEvent::Destroyed { at: t1 },
                Surface::None,
                Action::Reject {
                    why: "destroyed without a surface",
                },
            ),
            (
                "lost_destroyed_rejected_keeps_first_loss",
                lost(),
                SurfaceEvent::Destroyed { at: t1 },
                lost(),
                Action::Reject {
                    why: "destroyed without a surface",
                },
            ),
            (
                "ready_device_lost_recovers",
                Surface::Ready(1),
                SurfaceEvent::DeviceLost,
                Surface::Ready(1),
                Action::RecoverDevice,
            ),
            (
                "none_device_lost_rejected",
                Surface::None,
                SurfaceEvent::DeviceLost,
                Surface::None,
                Action::Reject {
                    why: "device lost without a surface",
                },
            ),
            (
                "lost_device_lost_rejected",
                lost(),
                SurfaceEvent::DeviceLost,
                lost(),
                Action::Reject {
                    why: "device lost without a surface",
                },
            ),
        ];
        for (name, state, event, expected_state, expected_action) in cases {
            assert_eq!(
                state.next(event),
                (expected_state, expected_action),
                "{name}"
            );
        }
    }
}
