# Patches

Parallax changes upstream GPUI only through numbered patches in [`patches/`](../patches/). The rules (when a patch
is allowed, size, `cfg` gating, header, filing upstream) are in [AGENTS.md, "Upstream and patch rules"](../AGENTS.md#upstream-and-patch-rules).
This page is the how-to.

## Writing a patch

- **Format:** a git-format diff (`git diff` or `git format-patch`), paths relative to `upstream/`:
  `a/gpui-pre-ios/src/ios.rs`. That is what [`apply-patches.sh`](../scripts/apply-patches.sh) feeds to
  `git apply --directory=build/gpui-patched`.
- **Name:** `NNNN-short-concern.patch`, four digits from `0001`, applied in sorted order. Never renumber a filed patch.
- **Header:** the free text above the first `diff --git` line (`git apply` ignores it). Contents per AGENTS.md patch rule 5.

## Making one from a change in `build/gpui-patched/`

`build/` is regenerated on every run, so edit it only to produce a diff, then move the diff into `patches/`:

```sh
scripts/apply-patches.sh                    # fresh build/gpui-patched/ with all current patches
cd build/gpui-patched && git init -q && git add -A && git commit -qm base
# ...edit files, then from the repo root: scripts/fork-crates.sh && cargo check (the workspace builds build/parallax-gpui/)...
git diff > ../../patches/0004-my-concern.patch   # paths come out as a/gpui-pre-*/..., as required
cd ../.. && rm -rf build/gpui-patched/.git
scripts/apply-patches.sh                    # must apply cleanly on top of the earlier patches
```

Add the header to the top of the file before committing it.

## Translating for filing upstream

Patches are written against the vendored layout and translated to Zed's `crates/` layout with
[`patch-paths.sh`](../scripts/patch-paths.sh): [patch-translation.md](patch-translation.md).

## Our Zed fork

[`scripts/sync-fork.sh`](../scripts/sync-fork.sh) rebuilds the `parallax` branch of
[`xuckless/zed`](https://github.com/xuckless/zed/tree/parallax): the UPSTREAM commit plus every patch, translated as
in [patch-translation.md](patch-translation.md), one commit each. Run it whenever a patch is added or changed.
Apps don't use that branch; they get every patch through the published `parallax-gpui*` crates. Both are in [zed-fork.md](zed-fork.md).

## Published crates

Every patch reaches crates.io through our fork crates ([AGENTS.md](../AGENTS.md#upstream-and-patch-rules), patch rule 11):
[`apply-patches.sh`](../scripts/apply-patches.sh) ends by running [`fork-crates.sh`](../scripts/fork-crates.sh), which
renames the patched `gpui-pre*` crates to `parallax-gpui*` in `build/parallax-gpui/`, and every Parallax crate depends
on those. Changing a patch between syncs means bumping [`patches/FORK_REVISION`](../patches/FORK_REVISION) and
publishing a new fork version: [gpui-fork-crates.md](gpui-fork-crates.md).

## The patches

One row per file in [`patches/`](../patches/); the header of each says what and why.

| Patch | Concern |
|---|---|
| [0001](../patches/0001-export-priority-queue-android.patch) | export `PriorityQueueSender`/`Receiver` on Android |
| [0002](../patches/0002-cosmic-text-system-with-font-system.patch) | `CosmicTextSystem::new_with_font_system` |
| [0003](../patches/0003-pointer-source.patch) | `TouchEvent::device`, `Window::pointer_source()` ([core-patches.md](core-patches.md)) |
| [0004](../patches/0004-touch-opt-out.patch) | `on_touch` opts an element out of touch → mouse, `Window::leave_touch_to_gestures`; taps click as `ClickEvent::Touch` ([core-patches.md](core-patches.md)) |
| [0005](../patches/0005-text-input-keyboard-type.patch) | keyboard type, secure entry, autofill hint in `TextInputConfiguration` ([core-patches.md](core-patches.md)) |
| [0006](../patches/0006-claim-back-gesture-after-key-input.patch) | `on_back_gesture` claims after key input ([android-keys.md](android-keys.md#back)) |
| [0007](../patches/0007-cosmic-text-system-font-database-id.patch) | `CosmicTextSystem::font_database_id`, so Android can draw emoji ([android-emoji.md](android-emoji.md)) |
| [0008](../patches/0008-back-gesture-source.patch) | `BackGestureEvent::source`: edge swipe or key ([core-patches.md](core-patches.md)) |
| [0009](../patches/0009-text-system-invalidate-fonts.patch) | `TextSystem::invalidate_fonts`, for Android's locale font order ([android-fonts.md](android-fonts.md)) |
| [0010](../patches/0010-end-composition-on-focus-loss.patch) | a field that focus leaves ends its IME composition ([core-patches.md](core-patches.md)) |
| [0011](../patches/0011-flutter-velocity-tracker.patch) | Flutter's velocity trackers and least-squares solver, `GestureTuning::velocity_tracker`, with Flutter's tests ([velocity-tracking.md](velocity-tracking.md)) |
| [0012](../patches/0012-flutter-scroll-simulations.patch) | `ScrollPhysics` is Flutter's (`Bouncing { deceleration_rate }`, `Clamping`), with its simulations and Flutter's physics module on GPUI's springs ([scroll-physics.md](scroll-physics.md)) |
| [0013](../patches/0013-gesture-recognizer-flutter-oracle.patch) | Flutter's gesture constants, slops, per-target double-tap counting and fling rule in the touch recognizer, executor clock, Flutter recognizer test rows, `gpui::play_trace` ([gesture-recognizers.md](gesture-recognizers.md)) |
| [0014](../patches/0014-app-gesture-tuning-override.patch) | `App::set_gesture_tuning` overrides the platform's gesture tuning; `GestureTuning::max_fling_velocity` ([gesture-tuning.md](gesture-tuning.md)) |
| [0015](../patches/0015-scale-and-overscroll-ports.patch) | Flutter's scale recognizer and overscroll indicators, new modules only ([scale-and-overscroll.md](scale-and-overscroll.md)) |
| [0016](../patches/0016-touch-pinch.patch) | touch pinch: the scale recognizer in touch dispatch, `PinchEvent::touch`, `on_pinch` for fingers, `Window::join_pinch` ([pinch.md](pinch.md)) |
| [0017](../patches/0017-overscroll-in-scroll-containers.patch) | overscroll (bounce, stretch, glow) in `div` scroll containers, `GestureTuning::overscroll`, `.overscroll(..)` ([overscroll.md](overscroll.md)) |
| [0018](../patches/0018-haptic-feedback.patch) | `gpui::Haptic`, `PlatformWindow::perform_haptic` / `Window::perform_haptic`, `GestureTuning::long_press_haptic` on a claimed long press; iOS and web backends ([haptics.md](haptics.md)) |
| [0019](../patches/0019-window-set-gesture-tuning.patch) | `Window::set_gesture_tuning` retunes an open window, for Android's device touch slop ([core-patches.md](gesture-tuning.md#0019-retune-an-open-window)) |
| [0020](../patches/0020-touch-event-timestamp.patch) | `TouchEvent::time_stamp`, the platform's event time on GPUI's clock, filled by iOS and the web, read by velocity tracking ([touch-time-stamps.md](touch-time-stamps.md)) |
