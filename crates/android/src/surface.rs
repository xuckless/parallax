//! The Android surface lifecycle: one state, one transition function. Pure, so it is tested on any host.
//! The shell in `host.rs` feeds it `SurfaceHolder.Callback` events, vsync frames and the outcome of GPU device
//! recovery, and carries out the returned [`Action`].

use std::time::{Duration, Instant};

use crate::lifecycle::ActivityId;

/// How long to wait after device loss (or after a failed rebuild) before rebuilding: index = failed attempts so far,
/// the last row repeats. The first row is upstream's 350 ms (`WgpuRenderer::recover` sleeps that long for the GPU
/// driver to settle); each failure doubles it, so a GPU that stays gone is not retried every frame. To tune the
/// backoff, edit the rows.
const RECOVERY_DELAYS: [Duration; 5] = [
    Duration::from_millis(350),
    Duration::from_millis(700),
    Duration::from_millis(1400),
    Duration::from_millis(2800),
    Duration::from_millis(5600),
];

/// Where the app's drawing surface is. `W` is the native window (`ANativeWindow` on Android, a plain value in tests);
/// `activity` is the Activity whose view created it, the only one whose `surfaceDestroyed` releases it.
#[derive(Debug, PartialEq)]
pub enum Surface<W> {
    /// No surface since launch: the `SurfaceView` has not created one yet.
    None,
    /// A native window the renderer draws into.
    Ready { window: W, activity: ActivityId },
    /// The GPU device was lost at `since`, a new surface could not be attached then, or the last rebuild failed then
    /// (after `failures` failed rebuilds). The renderer is rebuilt on `window` once the delay for `failures` has
    /// passed.
    Recovering {
        window: W,
        activity: ActivityId,
        since: Instant,
        failures: usize,
    },
    /// Android destroyed the surface (Home, screen off, Activity recreated); drawing waits for the next one.
    Lost { since: Instant },
}

/// What Android, the vsync or the GPU reported.
#[derive(Debug)]
pub enum SurfaceEvent<W> {
    /// `surfaceCreated` in the window owner's view, with the new native window.
    Created { window: W, activity: ActivityId },
    /// `surfaceChanged`: new size (rotation, split screen) on the same window.
    Resized,
    /// `surfaceDestroyed` in `activity`'s view, owner or not: its window must be released before the callback
    /// returns, and never drawn into again.
    Destroyed { activity: ActivityId, at: Instant },
    /// A `Choreographer` vsync, with whether the renderer reports its GPU device lost.
    Frame { at: Instant, device_lost: bool },
    /// The renderer was rebuilt on a new device.
    Recovered,
    /// Rebuilding the renderer failed at `at`.
    RecoveryFailed { at: Instant },
    /// The renderer could not make a surface on the window at `at` (after `Attach`); it draws nothing until rebuilt.
    AttachFailed { at: Instant },
}

/// What the shell does to the renderer after a transition.
#[derive(Debug, PartialEq)]
pub enum Action<W> {
    /// Give the renderer a new surface on the current window at its current size, and redraw. `previous`, if any,
    /// is released once the renderer has let go of it. A resize makes a new surface too: an EGL window surface
    /// keeps buffers of its old size (docs/android-surface.md#resize).
    Attach { previous: Option<W> },
    /// Unconfigure the renderer, then release `window`.
    Detach { window: W },
    /// Let GPUI draw if anything changed.
    Draw,
    /// Rebuild the renderer on a new GPU device (the window is the one in `Recovering`).
    RecoverDevice,
    /// Nothing to do yet.
    Wait,
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
            (S::None | S::Lost { .. }, E::Created { window, activity }) => {
                (S::Ready { window, activity }, A::Attach { previous: None })
            }
            // A new view's surface before the old one's surfaceDestroyed: the old window is released here, so
            // that late surfaceDestroyed finds nothing of its own.
            (
                S::Ready {
                    window: previous, ..
                },
                E::Created { window, activity },
            ) => (
                S::Ready { window, activity },
                A::Attach {
                    previous: Some(previous),
                },
            ),
            // The torn-down renderer holds no surface; the rebuild uses the new window.
            (
                S::Recovering {
                    window: previous,
                    since,
                    failures,
                    ..
                },
                E::Created { window, activity },
            ) => (
                S::Recovering {
                    window,
                    activity,
                    since,
                    failures,
                },
                A::Detach { window: previous },
            ),
            (S::Ready { window, activity }, E::Resized) => {
                (S::Ready { window, activity }, A::Attach { previous: None })
            }
            // The size is applied when recovery ends (`Recovered` → `Attach`).
            (state @ S::Recovering { .. }, E::Resized) => (state, A::Wait),
            // Its own view's surfaceDestroyed always releases the window, even once another Activity owns the
            // lifecycle (the old surface destroyed before the new one is created).
            (
                S::Ready { window, activity }
                | S::Recovering {
                    window, activity, ..
                },
                E::Destroyed { activity: from, at },
            ) if from == activity => (S::Lost { since: at }, A::Detach { window }),
            (state @ (S::Ready { .. } | S::Recovering { .. }), E::Destroyed { .. }) => (
                state,
                A::Reject {
                    why: "destroyed another view's surface",
                },
            ),
            (
                S::Ready { window, activity },
                E::Frame {
                    device_lost: false, ..
                },
            ) => (S::Ready { window, activity }, A::Draw),
            (
                S::Ready { window, activity },
                E::Frame {
                    at,
                    device_lost: true,
                },
            ) => (
                S::Recovering {
                    window,
                    activity,
                    since: at,
                    failures: 0,
                },
                A::Wait,
            ),
            (
                S::Recovering {
                    window,
                    activity,
                    since,
                    failures,
                },
                E::Frame { at, .. },
            ) => {
                let delay = RECOVERY_DELAYS[failures.min(RECOVERY_DELAYS.len() - 1)];
                let action = if at >= since + delay {
                    A::RecoverDevice
                } else {
                    A::Wait
                };
                let state = S::Recovering {
                    window,
                    activity,
                    since,
                    failures,
                };
                (state, action)
            }
            // Frames run from onResume to onPause, with or without a surface.
            (state @ (S::None | S::Lost { .. }), E::Frame { .. }) => (state, A::Wait),
            (
                S::Recovering {
                    window, activity, ..
                },
                E::Recovered,
            ) => (S::Ready { window, activity }, A::Attach { previous: None }),
            (
                S::Recovering {
                    window,
                    activity,
                    failures,
                    ..
                },
                E::RecoveryFailed { at },
            ) => (
                S::Recovering {
                    window,
                    activity,
                    since: at,
                    failures: failures.saturating_add(1),
                },
                A::Wait,
            ),
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
            (state @ (S::None | S::Ready { .. } | S::Lost { .. }), E::Recovered) => (
                state,
                A::Reject {
                    why: "recovered while not recovering",
                },
            ),
            (state @ (S::None | S::Ready { .. } | S::Lost { .. }), E::RecoveryFailed { .. }) => (
                state,
                A::Reject {
                    why: "recovery failed while not recovering",
                },
            ),
            // Without a surface the renderer draws black; a rebuild after the backoff retries, like device loss.
            (S::Ready { window, activity }, E::AttachFailed { at }) => (
                S::Recovering {
                    window,
                    activity,
                    since: at,
                    failures: 0,
                },
                A::Wait,
            ),
            (state @ (S::None | S::Recovering { .. } | S::Lost { .. }), E::AttachFailed { .. }) => {
                (
                    state,
                    A::Reject {
                        why: "attach failed while not attached",
                    },
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Activity whose view made the surface in the rows, and `B`, another one's view.
    const A: ActivityId = ActivityId(1);
    const B: ActivityId = ActivityId(2);

    #[test]
    fn transitions() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let lost = || Surface::Lost { since: t0 };
        let recovering = |window, since, failures| Surface::Recovering {
            window,
            activity: A,
            since,
            failures,
        };
        let ready = |window| Surface::Ready {
            window,
            activity: A,
        };
        let created = |window| SurfaceEvent::Created {
            window,
            activity: A,
        };
        let destroyed = |at| SurfaceEvent::Destroyed { activity: A, at };
        let frame = |at, device_lost| SurfaceEvent::Frame { at, device_lost };
        let no = |why| Action::Reject { why };
        // (row name, state, event, expected state, expected action). Windows are plain numbers; times are ms after t0.
        // One row per (state, event), rejected included, plus the recovery delay's edges.
        type Row = (
            &'static str,
            Surface<u8>,
            SurfaceEvent<u8>,
            Surface<u8>,
            Action<u8>,
        );
        #[rustfmt::skip]
        let cases: [Row; 40] = [
            ("none_created_attaches", Surface::None, created(1), ready(1), Action::Attach { previous: None }),
            ("ready_created_replaces", ready(1), created(2), ready(2), Action::Attach { previous: Some(1) }),
            ("recovering_created_swaps_window", recovering(1, t0, 2), created(2), recovering(2, t0, 2), Action::Detach { window: 1 }),
            ("lost_created_reattaches", lost(), created(2), ready(2), Action::Attach { previous: None }),
            ("none_resized_rejected", Surface::None, SurfaceEvent::Resized, Surface::None, no("resized without a surface")),
            ("ready_resized_makes_new_surface", ready(1), SurfaceEvent::Resized, ready(1), Action::Attach { previous: None }),
            ("recovering_resized_waits", recovering(1, t0, 0), SurfaceEvent::Resized, recovering(1, t0, 0), Action::Wait),
            ("lost_resized_rejected", lost(), SurfaceEvent::Resized, lost(), no("resized without a surface")),
            ("none_destroyed_rejected", Surface::None, destroyed(ms(1)), Surface::None, no("destroyed without a surface")),
            ("ready_destroyed_detaches", ready(1), destroyed(ms(1)), Surface::Lost { since: ms(1) }, Action::Detach { window: 1 }),
            ("recovering_destroyed_detaches", recovering(1, t0, 1), destroyed(ms(1)), Surface::Lost { since: ms(1) }, Action::Detach { window: 1 }),
            ("lost_destroyed_rejected_keeps_first_loss", lost(), destroyed(ms(1)), lost(), no("destroyed without a surface")),
            ("none_frame_waits", Surface::None, frame(ms(1), false), Surface::None, Action::Wait),
            ("ready_frame_draws", ready(1), frame(ms(1), false), ready(1), Action::Draw),
            ("ready_frame_device_lost_starts_recovering", ready(1), frame(ms(1), true), recovering(1, ms(1), 0), Action::Wait),
            ("recovering_frame_before_350ms_waits", recovering(1, t0, 0), frame(ms(349), true), recovering(1, t0, 0), Action::Wait),
            ("recovering_frame_at_350ms_recovers", recovering(1, t0, 0), frame(ms(350), true), recovering(1, t0, 0), Action::RecoverDevice),
            ("recovering_after_1_failure_waits_700ms", recovering(1, t0, 1), frame(ms(699), true), recovering(1, t0, 1), Action::Wait),
            ("recovering_after_1_failure_recovers_at_700ms", recovering(1, t0, 1), frame(ms(700), true), recovering(1, t0, 1), Action::RecoverDevice),
            ("recovering_after_9_failures_caps_at_5600ms", recovering(1, t0, 9), frame(ms(5599), true), recovering(1, t0, 9), Action::Wait),
            ("recovering_after_9_failures_recovers_at_5600ms", recovering(1, t0, 9), frame(ms(5600), true), recovering(1, t0, 9), Action::RecoverDevice),
            ("lost_frame_waits", lost(), frame(ms(1), true), lost(), Action::Wait),
            ("none_recovered_rejected", Surface::None, SurfaceEvent::Recovered, Surface::None, no("recovered while not recovering")),
            ("ready_recovered_rejected", ready(1), SurfaceEvent::Recovered, ready(1), no("recovered while not recovering")),
            ("recovering_recovered_makes_new_surface", recovering(1, t0, 3), SurfaceEvent::Recovered, ready(1), Action::Attach { previous: None }),
            ("lost_recovered_rejected", lost(), SurfaceEvent::Recovered, lost(), no("recovered while not recovering")),
            ("none_recovery_failed_rejected", Surface::None, SurfaceEvent::RecoveryFailed { at: ms(1) }, Surface::None, no("recovery failed while not recovering")),
            ("ready_recovery_failed_rejected", ready(1), SurfaceEvent::RecoveryFailed { at: ms(1) }, ready(1), no("recovery failed while not recovering")),
            ("recovering_recovery_failed_backs_off", recovering(1, t0, 0), SurfaceEvent::RecoveryFailed { at: ms(400) }, recovering(1, ms(400), 1), Action::Wait),
            ("recovering_recovery_failed_counts_on", recovering(1, t0, 4), SurfaceEvent::RecoveryFailed { at: ms(400) }, recovering(1, ms(400), 5), Action::Wait),
            // Two Activities' views (a deep link stacked B on A, then back), in both orders. Its own view's
            // surfaceDestroyed always releases the window; a late one from a replaced view finds nothing of its own.
            ("other_view_created_replaces_before_destroyed", ready(1), SurfaceEvent::Created { window: 2, activity: B }, Surface::Ready { window: 2, activity: B }, Action::Attach { previous: Some(1) }),
            ("replaced_view_destroyed_late_rejected", Surface::Ready { window: 2, activity: B }, destroyed(ms(1)), Surface::Ready { window: 2, activity: B }, no("destroyed another view's surface")),
            ("own_view_destroyed_before_other_created", ready(1), destroyed(ms(1)), Surface::Lost { since: ms(1) }, Action::Detach { window: 1 }),
            ("other_view_created_after_destroyed", Surface::Lost { since: ms(1) }, SurfaceEvent::Created { window: 2, activity: B }, Surface::Ready { window: 2, activity: B }, Action::Attach { previous: None }),
            ("recovering_other_view_destroyed_rejected", recovering(1, t0, 0), SurfaceEvent::Destroyed { activity: B, at: ms(1) }, recovering(1, t0, 0), no("destroyed another view's surface")),
            ("lost_recovery_failed_rejected", lost(), SurfaceEvent::RecoveryFailed { at: ms(1) }, lost(), no("recovery failed while not recovering")),
            ("none_attach_failed_rejected", Surface::None, SurfaceEvent::AttachFailed { at: ms(1) }, Surface::None, no("attach failed while not attached")),
            ("ready_attach_failed_retries_after_backoff", ready(1), SurfaceEvent::AttachFailed { at: ms(5) }, recovering(1, ms(5), 0), Action::Wait),
            ("recovering_attach_failed_rejected", recovering(1, t0, 2), SurfaceEvent::AttachFailed { at: ms(1) }, recovering(1, t0, 2), no("attach failed while not attached")),
            ("lost_attach_failed_rejected", lost(), SurfaceEvent::AttachFailed { at: ms(1) }, lost(), no("attach failed while not attached")),
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
