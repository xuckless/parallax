# Haptics

GPUI plays Flutter's haptics: `gpui::Haptic` is Flutter's `HapticFeedback` calls (vibrate, light / medium / heavy
impact, selection click, success / warning / error notification), added by
[patch 0018](../patches/0018-haptic-feedback.patch). Each backend plays every kind with the code Flutter's own
embedder uses for it, ported (ledger: [PORTED.md](../third_party/flutter/PORTED.md)).

## When a haptic plays

- **Long press, from the table.** The [behaviour table](behaviour.md)'s `haptics.long_press` row (Flutter's
  `Feedback.forLongPress`: Android `Vibrate`, iOS `HeavyImpact`, desktops none) reaches GPUI as
  `GestureTuning::long_press_haptic`, through the same `set_platform_gesture_tuning` call as the slops and physics.
  When an element claims a long press (prevents default on the started `LongPressEvent`, as GPUI's long-press
  tooltip does), the window plays it once. An unclaimed long press (a slow tap) plays nothing: Flutter's widgets
  call `Feedback.forLongPress` only when they handle the press.
- **From app code.** `window.perform_haptic(Haptic::SelectionClick)` plays any kind.
- The table's other rows (`tap`: none on every target, `selection_handle_drag`, `picker_item`) are data until GPUI
  has the widgets that play them (selection handles, pickers).

To change the long-press haptic, override the tuning as [behaviour.md](behaviour.md#how-gpui-gets-it-automatically)
shows, e.g. `GestureTuning { long_press_haptic: None, ..cx.gesture_tuning() }`.

## Per platform

`PlatformWindow::perform_haptic(Haptic)` defaults to nothing; each backend that has haptics implements it.

| Kind | Android ([PlatformPlugin.java](../crates/android/java/io/github/xuckless/parallax/PlatformPlugin.java)) | iOS (gpui_ios, `FlutterPlatformPlugin.mm`) | Web (gpui_web, `navigator.vibrate`) |
|---|---|---|---|
| `Vibrate` | `LONG_PRESS` | `AudioServicesPlaySystemSound(kSystemSoundID_Vibrate)` | 50 ms |
| `LightImpact` | `VIRTUAL_KEY` | `UIImpactFeedbackGenerator` light | 10 ms |
| `MediumImpact` | `KEYBOARD_TAP` | `UIImpactFeedbackGenerator` medium | 20 ms |
| `HeavyImpact` | `CONTEXT_CLICK` | `UIImpactFeedbackGenerator` heavy | 30 ms |
| `SelectionClick` | `CLOCK_TICK` | `UISelectionFeedbackGenerator` | 10 ms |
| `SuccessNotification` | `CONFIRM` (API 30+) | `UINotificationFeedbackGenerator` success | 20 ms |
| `WarningNotification` | `KEYBOARD_TAP` (API 30+) | `UINotificationFeedbackGenerator` warning | 20 ms |
| `ErrorNotification` | `REJECT` (API 30+) | `UINotificationFeedbackGenerator` error | 30 ms |

- **Android.** [window.rs](../crates/android/src/window.rs) queues the haptic in the shell; every vsync the view asks
  for the queue (`nativeHapticFeedbackCalls`, after the text-input calls) as Flutter's `HapticFeedback.vibrate`
  arguments ([haptics.rs](../crates/android/src/haptics.rs), the `haptic_feedback.dart` mapping), and plays each
  with the ported `PlatformChannel.HapticFeedbackType.fromValue` and `PlatformPlugin.vibrateHapticFeedback`
  (`View.performHapticFeedback` on the decor view). A pull, as for text input: a long press fires from a GPUI timer,
  where Rust holds no Java frame to call through. So it plays up to one frame late; Flutter's channel is async too.
  `performHapticFeedback` follows the system's touch-feedback setting.
- **iOS.** A fresh generator per call, as Flutter's. Compile-checked on the iOS target; run on a device to feel it
  (the simulator has no haptics).
- **Web.** Flutter web's durations; browsers without `navigator.vibrate` (Safari, iOS) play nothing, as in Flutter.
- **Desktop.** Nothing, as Flutter.

## Tests

| Where | Checks |
|---|---|
| `window::tests::claimed_long_press_plays_the_tuning_haptic_once` (patch 0018, [patch-test.sh](patch-tests.md)) | Through the test platform (`TestWindow::haptics`): claimed with vibrate / heavy impact plays it once over the whole press; unclaimed or `None` plays nothing |
| `haptics::tests::flutter_argument_table` (`cargo test -p parallax-android`) | Haptic → Flutter's `HapticFeedback.vibrate` argument, one row per kind |
| [PlatformPluginTest.java](../crates/android/java-tests/src/test/java/io/github/xuckless/parallax/PlatformPluginTest.java) ([android-tests.md](android-tests.md)) | Flutter's two cases: each type → its `HapticFeedbackConstants`, below and from API 30 |
| `behaviour_table` ([behaviour.rs](../crates/platform/src/behaviour.rs)) | Each row's `gesture_tuning(None).long_press_haptic` is its `Feedback.forLongPress` |

The iOS and web mappings are exhaustive matches, compile-checked on their targets; they have no host test.

## Try it

[hello](examples.md#hello): long-press the box. Its tooltip claims the press and the platform plays the row's haptic;
the app has no haptic code. On the Android emulator, `adb shell dumpsys vibrator_manager` lists it under the
vibrations played (`LONG_PRESS`, from the decor view).
