# Behaviour table: how GPUI gets it

How the [behaviour table](behaviour.md)'s row reaches GPUI's gesture tuning, follows density changes, and how an app
overrides it.

## How GPUI gets it: automatically

Every Parallax entry point applies the current target's row before app code runs:
[`parallax::run(launch)`](../crates/parallax/src/lib.rs) on desktop, iOS and the web (hello's
[main.rs](../examples/hello/src/main.rs) and web `start`), the facade's `android_main!` on Android. Both call
`parallax::set_platform_gesture_tuning` (`Result<(), parallax_platform::Error>`; they log an error and keep the
platform's tuning). `Behaviour::gesture_tuning(reported)` fills GPUI's `GestureTuning` from the row (`touch_slop` ←
the hit slop of the resolved touch slop: Android passes `parallax_android::touch_slop()`, the others `None`;
`scroll_physics`, `min_fling_velocity` / `max_fling_velocity` ← its fling limits, `velocity_tracker`, `overscroll`,
`long_press_haptic` ← `haptics.long_press`, played on a claimed long press: [haptics.md](haptics.md)), the rest from
`GestureTuning::default()` (Flutter's constants), through `App::set_gesture_tuning` (patch 0014).
`parallax_android::android_main!` alone does not apply it (the table builds on that crate): use the facade's.

**Density changes (Android):** when an Activity's view reports a new logical slop while the app runs,
parallax-android runs the hook the facade's `android_main!` passes (`TouchSlopChanged`) on the next looper turn, with
that Activity's window ([android-multi-window.md](android-multi-window.md)), and `Behaviour::retune` sets the new hit
slop on the app's tuning and on that window's own (`Window::set_gesture_tuning`, patch 0019), while the tuning's owner
(`TuningSource`, recorded by the facade) is the platform. The facade applies the row before `launch`, which runs
once per process: a later Activity's window keeps whatever tuning is set. Logcat:
`parallax: Activity <n>: touch slop <px> px at density <d>` from parallax-android whenever the density or slop moves (the density
reaches it through `onApplyWindowInsets`), then `gesture tuning's touch slop now …` when the logical slop moved:
Android scales it with the density, so most changes keep it (`wm density 450` makes it 23 px / 2.8125 = 8.18).

**Override:** call `parallax::set_gesture_tuning(cx, tuning)` in `launch` with a changed copy, e.g.
`GestureTuning { touch_slop: px(12.), ..cx.gesture_tuning() }`. Windows opened later use it, and the app owns the
tuning from then on: density changes never touch it, even when it equals the platform's.

**Read by GPUI:** `overscroll` is `GestureTuning::overscroll`, the default every scroll container follows (a
container may set its own): [overscroll.md](overscroll.md). `haptics.long_press` plays on a claimed long press
(patch 0018, [haptics.md](haptics.md)). The touch pinch's scale recognizer takes its pan and scale slops from
`touch_slop` ([pinch.md](pinch.md)). `haptics.selection_handle_drag` is what `parallax_ext::text_selection` asks the app to play while a handle moves
([text-selection.md](text-selection.md)). The other `haptics` rows (`tap`, `picker_item`: no
GPUI widget plays them yet), `min_touch_target`, the pan and scale slops and `drag_start_distance_motion_threshold`
are data GPUI doesn't take yet; the widgets that later waves wire read them (ROADMAP Phase 2).
