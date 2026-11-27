//! The Android surface lifecycle: one state, one transition function. Pure, so it is tested on any host.
//! The shell in `host.rs` feeds it `SurfaceHolder.Callback` events, vsync frames and the outcome of GPU device
//! recovery, and carries out the returned [`Action`].

use std::time::{Duration, Instant};

use gpui::{DevicePixels, Size};

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

/// Where one Activity's drawing surface is (each Activity's window has its own). `W` is the native window
/// (`ANativeWindow` on Android, a plain value in tests); `size` is the window's size in pixels as Android last reported
/// it: the size the renderer's surface is (or is rebuilt) at.
#[derive(Debug, PartialEq)]
pub enum Surface<W> {
    /// No surface since launch: the `SurfaceView` has not created one yet.
    None,
    /// A native window the renderer draws into.
    Ready { window: W, size: Size<DevicePixels> },
    /// The GPU device was lost at `since`, a new surface could not be attached then, or the last rebuild failed then
    /// (after `failures` failed rebuilds). The renderer is rebuilt on `window` at `size` once the delay for
    /// `failures` has passed.
    Recovering {
        window: W,
        size: Size<DevicePixels>,
        since: Instant,
        failures: usize,
    },
    /// Android destroyed the surface (Home, screen off, Activity recreated); drawing waits for the next one.
    Lost { since: Instant },
}

/// What Android, the vsync or the GPU reported.
#[derive(Debug)]
pub enum SurfaceEvent<W> {
    /// `surfaceCreated` in the Activity's view, with the new native window and its size.
    Created { window: W, size: Size<DevicePixels> },
    /// `surfaceChanged` on the same window: Android sends one right after every `surfaceCreated`, usually at the
    /// same size, and one per real change (rotation, split screen).
    Resized { size: Size<DevicePixels> },
    /// `surfaceDestroyed`: the window must be released before the callback
    /// returns, and never drawn into again.
    Destroyed { at: Instant },
    /// A `Choreographer` vsync, with whether the renderer reports its GPU device lost.
    Frame { at: Instant, device_lost: bool },
    /// The renderer was rebuilt, with its own surface on the window at the state's size.
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
    /// The rebuilt renderer already has a surface at the current size: tell GPUI the size and draw a full frame.
    Redraw,
    /// Rebuild the renderer on a new GPU device (the window and size are the ones in `Recovering`).
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
            (S::None | S::Lost { .. }, E::Created { window, size }) => {
                (S::Ready { window, size }, A::Attach { previous: None })
            }
            // A new surface before the old one's surfaceDestroyed (SurfaceHolder does not promise the order): the old
            // window is released here, once the renderer has let go of it.
            (
                S::Ready {
                    window: previous, ..
                },
                E::Created { window, size },
            ) => (
                S::Ready { window, size },
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
                E::Created { window, size },
            ) => (
                S::Recovering {
                    window,
                    size,
                    since,
                    failures,
                },
                A::Detach { window: previous },
            ),
            // The surfaceChanged that follows every surfaceCreated: the surface just attached already has this size.
            (state @ S::Ready { size, .. }, E::Resized { size: new }) if new == size => {
                (state, A::Draw)
            }
            // A real size change makes a new surface: API 28's GL keeps buffers of the old size otherwise.
            (S::Ready { window, .. }, E::Resized { size }) => {
                (S::Ready { window, size }, A::Attach { previous: None })
            }
            // The rebuild makes its surface at the newest size.
            (
                S::Recovering {
                    window,
                    since,
                    failures,
                    ..
                },
                E::Resized { size },
            ) => (
                S::Recovering {
                    window,
                    size,
                    since,
                    failures,
                },
                A::Wait,
            ),
            (S::Ready { window, .. } | S::Recovering { window, .. }, E::Destroyed { at }) => {
                (S::Lost { since: at }, A::Detach { window })
            }
            (
                state @ S::Ready { .. },
                E::Frame {
                    device_lost: false, ..
                },
            ) => (state, A::Draw),
            // A lost device, or no surface after a failed attach (the renderer draws black): rebuild after the
            // backoff.
            (
                S::Ready { window, size },
                E::Frame {
                    at,
                    device_lost: true,
                }
                | E::AttachFailed { at },
            ) => (
                S::Recovering {
                    window,
                    size,
                    since: at,
                    failures: 0,
                },
                A::Wait,
            ),
            (
                state @ S::Recovering {
                    since, failures, ..
                },
                E::Frame { at, .. },
            ) => {
                let delay = RECOVERY_DELAYS[failures.min(RECOVERY_DELAYS.len() - 1)];
                let action = if at >= since + delay {
                    A::RecoverDevice
                } else {
                    A::Wait
                };
                (state, action)
            }
            // Frames run from onResume to onPause, with or without a surface.
            (state @ (S::None | S::Lost { .. }), E::Frame { .. }) => (state, A::Wait),
            // `renderer::recover` built its surface at `size`, a resize while recovering included: no second one.
            (S::Recovering { window, size, .. }, E::Recovered) => {
                (S::Ready { window, size }, A::Redraw)
            }
            (
                S::Recovering {
                    window,
                    size,
                    failures,
                    ..
                },
                E::RecoveryFailed { at },
            ) => (
                S::Recovering {
                    window,
                    size,
                    since: at,
                    failures: failures.saturating_add(1),
                },
                A::Wait,
            ),
            (state @ (S::None | S::Lost { .. }), E::Resized { .. }) => (
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
    use gpui::size;

    use super::*;

    #[test]
    fn transitions() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        // A portrait window and the same window rotated.
        let p = size(DevicePixels(1080), DevicePixels(2400));
        let l = size(DevicePixels(2400), DevicePixels(1080));
        let lost = || Surface::Lost { since: t0 };
        let recovering = |window, size, since, failures| Surface::Recovering {
            window,
            size,
            since,
            failures,
        };
        let ready = |window, size| Surface::Ready { window, size };
        let created = |window, size| SurfaceEvent::Created { window, size };
        let resized = |size| SurfaceEvent::Resized { size };
        let destroyed = |at| SurfaceEvent::Destroyed { at };
        let frame = |at, device_lost| SurfaceEvent::Frame { at, device_lost };
        let no = |why| Action::Reject { why };
        // (row name, state, event, expected state, expected action). Windows are plain numbers; times are ms after t0;
        // sizes are portrait `p` or landscape `l`. One row per (state, event), rejected included, plus the recovery
        // delay's edges and the same-size resize.
        type Row = (
            &'static str,
            Surface<u8>,
            SurfaceEvent<u8>,
            Surface<u8>,
            Action<u8>,
        );
        #[rustfmt::skip]
        let cases: [Row; 38] = [
            ("none_created_attaches", Surface::None, created(1, p), ready(1, p), Action::Attach { previous: None }),
            ("ready_created_replaces", ready(1, p), created(2, l), ready(2, l), Action::Attach { previous: Some(1) }),
            ("recovering_created_swaps_window", recovering(1, p, t0, 2), created(2, l), recovering(2, l, t0, 2), Action::Detach { window: 1 }),
            ("lost_created_reattaches", lost(), created(2, p), ready(2, p), Action::Attach { previous: None }),
            ("none_resized_rejected", Surface::None, resized(p), Surface::None, no("resized without a surface")),
            ("ready_resized_to_same_size_draws_without_new_surface", ready(1, p), resized(p), ready(1, p), Action::Draw),
            ("ready_resized_to_new_size_makes_new_surface", ready(1, p), resized(l), ready(1, l), Action::Attach { previous: None }),
            ("ready_resized_back_makes_new_surface", ready(1, l), resized(p), ready(1, p), Action::Attach { previous: None }),
            ("recovering_resized_waits_with_new_size", recovering(1, p, t0, 0), resized(l), recovering(1, l, t0, 0), Action::Wait),
            ("lost_resized_rejected", lost(), resized(p), lost(), no("resized without a surface")),
            ("none_destroyed_rejected", Surface::None, destroyed(ms(1)), Surface::None, no("destroyed without a surface")),
            ("ready_destroyed_detaches", ready(1, p), destroyed(ms(1)), Surface::Lost { since: ms(1) }, Action::Detach { window: 1 }),
            ("recovering_destroyed_detaches", recovering(1, p, t0, 1), destroyed(ms(1)), Surface::Lost { since: ms(1) }, Action::Detach { window: 1 }),
            ("lost_destroyed_rejected_keeps_first_loss", lost(), destroyed(ms(1)), lost(), no("destroyed without a surface")),
            ("none_frame_waits", Surface::None, frame(ms(1), false), Surface::None, Action::Wait),
            ("ready_frame_draws", ready(1, p), frame(ms(1), false), ready(1, p), Action::Draw),
            ("ready_frame_device_lost_starts_recovering", ready(1, p), frame(ms(1), true), recovering(1, p, ms(1), 0), Action::Wait),
            ("recovering_frame_before_350ms_waits", recovering(1, p, t0, 0), frame(ms(349), true), recovering(1, p, t0, 0), Action::Wait),
            ("recovering_frame_at_350ms_recovers", recovering(1, p, t0, 0), frame(ms(350), true), recovering(1, p, t0, 0), Action::RecoverDevice),
            ("recovering_after_1_failure_waits_700ms", recovering(1, p, t0, 1), frame(ms(699), true), recovering(1, p, t0, 1), Action::Wait),
            ("recovering_after_1_failure_recovers_at_700ms", recovering(1, p, t0, 1), frame(ms(700), true), recovering(1, p, t0, 1), Action::RecoverDevice),
            ("recovering_after_9_failures_caps_at_5600ms", recovering(1, p, t0, 9), frame(ms(5599), true), recovering(1, p, t0, 9), Action::Wait),
            ("recovering_after_9_failures_recovers_at_5600ms", recovering(1, p, t0, 9), frame(ms(5600), true), recovering(1, p, t0, 9), Action::RecoverDevice),
            ("lost_frame_waits", lost(), frame(ms(1), true), lost(), Action::Wait),
            ("none_recovered_rejected", Surface::None, SurfaceEvent::Recovered, Surface::None, no("recovered while not recovering")),
            ("ready_recovered_rejected", ready(1, p), SurfaceEvent::Recovered, ready(1, p), no("recovered while not recovering")),
            ("recovering_recovered_redraws_without_second_surface", recovering(1, p, t0, 3), SurfaceEvent::Recovered, ready(1, p), Action::Redraw),
            ("recovering_resized_then_recovered_keeps_new_size", recovering(1, l, t0, 0), SurfaceEvent::Recovered, ready(1, l), Action::Redraw),
            ("lost_recovered_rejected", lost(), SurfaceEvent::Recovered, lost(), no("recovered while not recovering")),
            ("none_recovery_failed_rejected", Surface::None, SurfaceEvent::RecoveryFailed { at: ms(1) }, Surface::None, no("recovery failed while not recovering")),
            ("ready_recovery_failed_rejected", ready(1, p), SurfaceEvent::RecoveryFailed { at: ms(1) }, ready(1, p), no("recovery failed while not recovering")),
            ("recovering_recovery_failed_backs_off", recovering(1, p, t0, 0), SurfaceEvent::RecoveryFailed { at: ms(400) }, recovering(1, p, ms(400), 1), Action::Wait),
            ("recovering_recovery_failed_counts_on", recovering(1, p, t0, 4), SurfaceEvent::RecoveryFailed { at: ms(400) }, recovering(1, p, ms(400), 5), Action::Wait),
            ("lost_recovery_failed_rejected", lost(), SurfaceEvent::RecoveryFailed { at: ms(1) }, lost(), no("recovery failed while not recovering")),
            ("none_attach_failed_rejected", Surface::None, SurfaceEvent::AttachFailed { at: ms(1) }, Surface::None, no("attach failed while not attached")),
            ("ready_attach_failed_retries_after_backoff", ready(1, l), SurfaceEvent::AttachFailed { at: ms(5) }, recovering(1, l, ms(5), 0), Action::Wait),
            ("recovering_attach_failed_rejected", recovering(1, p, t0, 2), SurfaceEvent::AttachFailed { at: ms(1) }, recovering(1, p, t0, 2), no("attach failed while not attached")),
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
