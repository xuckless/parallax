# Platform feel: the behaviour table

A guide for app code. Each target feels native because its gesture and scroll values are Flutter's per-platform
defaults, kept in one visible table ([behaviour.md](behaviour.md), `TABLE` in
[behaviour.rs](../crates/platform/src/behaviour.rs)). You do nothing to get them: `parallax::run` and the facade's
`android_main!` apply the current target's row before your `launch` runs.

## What changes per target

| Target | A tap turns into a pan after | Scrolling | Past the edge | Long-press haptic | Smallest touch target |
|---|---|---|---|---|---|
| Android | the device's touch slop (8 dp on most) | clamping | stretch | vibrate | 48 px |
| iOS | 18 px | bouncing | bounce | heavy impact | 44 px |
| macOS | 18 px | bouncing, fast flings | bounce | none | 36 px |
| Windows | 18 px | clamping | none | none | 48 px |
| Linux | 18 px | clamping | none | none | 48 px |

The web takes the row of the OS the browser runs on, with Flutter web's slops. A mouse always gets small slops (1 px
for a tap), so mouse drags start at once. Text selection follows the same split: Flutter's rules on Android and
touch desktops, UIKit's own on iOS ([text-selection.md](text-selection.md)). The table's full columns (velocity
tracker, fling limits, pan and scale slops) and their Flutter sources: [behaviour.md](behaviour.md).

The user's own OS settings (reduce motion, reduce transparency) are read at runtime, not from the table:
`cx.reduce_motion()`, `cx.reduce_transparency()` ([os-settings.md](os-settings.md)).

## Overriding it

Change a copy of the current tuning in `launch` and hand it back with `parallax::set_gesture_tuning`:

```rust
use parallax::gpui::{px, App, GestureTuning};

fn launch(cx: &mut App) {
    let tuning = GestureTuning { touch_slop: px(12.), ..cx.gesture_tuning() };
    parallax::set_gesture_tuning(cx, tuning);
}
```

Windows opened afterwards use it, and the app owns the tuning from then on: an Android density change, which
retunes the platform's slop, never overrides yours. Use `parallax::set_gesture_tuning`, not `cx.set_gesture_tuning`,
which the platform may retune. One scroll container can pick its own overscroll with `.overscroll(..)`
([overscroll.md](overscroll.md)). How the row reaches GPUI: [behaviour-tuning.md](behaviour-tuning.md).

## Why one table

Platform behaviour never hides in a `cfg` or a `Default`: every value is a row you can read, each with its Flutter
source ([AGENTS.md](../AGENTS.md), "No hidden behaviour"). To change or add a row: [behaviour.md](behaviour.md#extending).
