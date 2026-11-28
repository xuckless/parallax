# iOS keyboard

Three patches to gpui_ios. Two are ported from Flutter's iOS embedder (ledger:
[PORTED.md](../third_party/flutter/PORTED.md)): a hardware keyboard reaches GPUI as key events
([0031](../patches/0031-ios-hardware-keyboard.patch)), and the software keyboard's inset follows the keyboard
([0032](../patches/0032-ios-keyboard-inset.patch)). The third reports layout changes
([0033](../patches/0033-ios-keyboard-layout-change.patch)). The decisions are plain-value modules, `keyboard.rs` and
`keyboard_inset.rs`, whose table tests run on the host ([patch-tests.md](patch-tests.md)); the UIKit glue is in
`window.rs`.

## Hardware keyboard (0031)

- **Where presses arrive.** The view controller overrides `pressesBegan:` … `pressesCancelled:`, as
  `FlutterViewController.mm`. Presses reach it with no first responder, and while a field is focused from the text
  input view through the responder chain (that view has no presses methods, so `text_input.rs` is untouched).
- **GPUI first, then UIKit.** Each press is GPUI's event first. One GPUI leaves unhandled (`propagate` still set)
  goes to `super`, and so to UIKit's text system, which types it into the focused field. A key is delivered once:
  `a` is a `KeyDown` nobody binds, then `insertText:`; a field's Backspace binding takes the `KeyDown` and UIKit
  never deletes. A key's up and cancel go to UIKit exactly when its down did (`KeyResponder` keeps the keys UIKit
  has down), so UIKit is never left holding a key. While the field composes (marked text) downs are UIKit's alone,
  as gpui_macos sends them to the input method first, so Enter commits instead of reaching a binding; the presses
  still run through `KeyResponder`, so a key whose down came before the composition and its up during it (Option-e,
  a dead key) is not left down for GPUI, and the next e is typed.
- **Which event.** `KeyResponder` is Flutter's `FlutterEmbedderKeyResponder` with GPUI's events: a non-modifier key
  is `KeyDown` / `KeyUp`, a modifier or Caps Lock press is `ModifiersChanged`, and Flutter's synthesized modifier
  events (iOS flags a modifier with no press for it, e.g. Alt released before its letter) are one
  `ModifiersChanged` before the key. A second down of a key that is down, or an up of one that is not, is Flutter's
  empty event: handled, nothing sent. Changed and stationary presses are UIKit's.
- **Key names.** Three tables, checked in order: `SPECIAL_KEYS` (UIKit's `UIKeyInput…` names, so Cmd-. is
  `cmd-escape`), `KEY_CODE_KEYS` (HID usage → GPUI key and `key_char`), else the key's label, lowercased. GPUI's
  rules as gpui_macos: no `key_char` with Control or Command; Shift is folded into a key that is not a letter
  (`!`, `shift-a`). A key GPUI has no name for (Eject, Fn) is UIKit's. To extend, add a row to a table.
- **Caps Lock** is the lock: a Caps Lock down toggles it, any other key takes iOS's flag.
  `PlatformWindow::capslock()` reports it.
- **Changed from Flutter.** A cancelled press releases its key (Flutter's TODO). `UIKeyInputEnd` is End (Flutter's
  generated table gives Enter's logical key).
- **Key repeat** (not Flutter's: its iOS embedder has none). UIKit sends no repeat presses and repeats only keys its
  text system got (a held `a` types `aaa` through `insertText:`). A held key GPUI took (Backspace, arrows,
  shortcuts) repeats on the window: an `NSTimer` sends its `KeyDown` again with `is_held` after `KEY_REPEAT_DELAY`
  (500 ms), then every `KEY_REPEAT_INTERVAL` (100 ms), until its up or cancel, another key's down, or the window
  resigning. iPadOS's own Key Repeat setting is not readable by apps; to retune, change the two constants in
  `keyboard.rs`.

## Keyboard inset (0032)

`WindowInsets::ime.bottom` is the part of the docked keyboard that covers the window, animated with the keyboard.

- **Notifications.** `UIKeyboardWillShow`, `WillChangeFrame` and `WillHide` go to `KeyboardInsetManager`, Flutter's
  `KeyboardInsetManager.swift`: another app's keyboard (`isLocal` false, iPad Split View) and a dragged keyboard's
  zero frame are ignored; the end frame's mode is docked, floating or hidden (undocked, split and floating iPad
  keyboards cover nothing); Slide Over's offset is added; the inset is the docked frame intersected with the view
  in the screen's coordinate space (Stage Manager windows, split view).
- **Animation.** A new target starts an animation of a hidden view's `y` from the current inset, with the
  notification's duration (UIKit applies the keyboard's curve inside its notification). When UIKit animates it with
  a `CASpringAnimation` (the keyboard's), its parameters become Flutter's spring (React Native's `SpringAnimation`,
  MIT, [spring_animation.rs](../patches/0032-ios-keyboard-inset.patch)), and each display-link frame sets the inset to
  the spring at the link's `targetTimestamp`, frame-exact with the keyboard; without a spring it reads the view's
  presentation layer. The completion of the animation still running sets the target.
  A same-direction change while one runs continues it (Flutter's compounding). A hide with no duration right after a
  show (the password manager's prompt) drops the inset to 0 at once.
- **Changed from Flutter.** Lengths are points; the display link replaces Flutter's vsync client. GPUI's own
  `SpringConfig` is no stand-in for the spring: with the keyboard's (mass 3, stiffness 1000, damping 500) it is
  overdamped and reaches only about 70 % in 0.6 s, where Flutter's curve (its critically damped branch) and Core
  Animation's keyboard land. The spring makes parallax-gpui-ios `Apache-2.0 AND BSD-3-Clause AND MIT`
  ([gpui-fork-crates.md](gpui-fork-crates.md)).

## Layout change (0033)

`App::on_keyboard_layout_change` observers run when UIKit posts `UITextInputCurrentInputModeDidChangeNotification`
(globe key, a hardware keyboard's layout shortcut, the emoji keyboard; also when a field first takes focus). The
callback runs on the foreground executor once UIKit's call unwinds, as gpui_ios's display notifications do.
`keyboard_layout()` still names one layout (`ios-default`).

## Checks

`scripts/patch-test.sh keyboard` runs the tables: one row per `FlutterEmbedderKeyResponderTest.mm` and keyboard
`FlutterViewControllerTest.mm` case, by test name, then GPUI's rows. On the iPhone and iPad simulators (Wave 14):
hello's field got typing once per key, Left moved the caret (bound, so UIKit did not), Backspace deleted once,
Enter submitted; keys reached GPUI with no field focused; the inset rose to the keyboard's height over about 20
frames and fell back on hide; the input-mode notification fired on focus and on switching to the emoji keyboard. idb sends no key chords, so modifier combinations are covered by the tables only.
