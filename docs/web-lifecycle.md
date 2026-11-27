# Web lifecycle and keyboard inset

How the web reports GPUI's app lifecycle and the on-screen keyboard: [patch 0035](../patches/0035-web-lifecycle-and-keyboard-inset.patch).
Upstream gpui_web had neither: `on_app_lifecycle` was the trait's no-op and `WindowInsets::ime` was always zero
([gap audit](audits/gap-audit.md)).

## Lifecycle

`page_lifecycle.rs` is Chrome's [Page Lifecycle](https://developer.chrome.com/docs/web-platform/page-lifecycle-api)
state machine, ported from [GoogleChromeLabs/page-lifecycle](https://github.com/GoogleChromeLabs/page-lifecycle/blob/a32563d198a7e4f5c2338fe6a53a02fd5857992b/src/Lifecycle.mjs)
(Apache-2.0): five states, the event rules, and the legal paths, so a state a browser skips is still reported.
Flutter's web engine ([app_lifecycle_state.dart](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/lib/web_ui/lib/src/engine/platform_dispatcher/app_lifecycle_state.dart))
hears only focus, blur and visibility; its mapping names the phases.

| Page state | Means | GPUI hears on the way in |
|---|---|---|
| active | visible, focused | `Active` (from passive), `Foreground`, `Active` (from hidden or frozen) |
| passive | visible, not focused | `Inactive` (from active), `Foreground` (from hidden or frozen) |
| hidden | tab in the background, window minimised | `Background` |
| frozen | `freeze`, or `pagehide` into the back/forward cache | nothing more (still hidden to GPUI) |
| terminated | `pagehide` of an unloading page | `Disconnected` |

So a tab hidden from active hears `Inactive`, `Background`; a bfcache restore (`pageshow`) hears `Foreground`,
`Active`; closing an active tab hears `Inactive`, `Background`, `Disconnected`. `WebPlatform` listens to `focus`,
`blur`, `pageshow` and `pagehide` on the window and `visibilitychange`, `freeze` and `resume` on the document, and
calls the observer synchronously: `pagehide` is a closing page's last chance to save. The page's own focus moves
(the IME mirror's focus cycling) change no state, so they never call the observer from inside a GPUI dispatch.

## Keyboard inset

Browsers shrink the visual viewport, not the layout viewport, for the keyboard (`interactive-widget=resizes-visual`).
`keyboard_inset.rs` ports Flutter's `computeKeyboardInsets`
([full_page_dimensions_provider.dart](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/lib/web_ui/lib/src/engine/view_embedder/dimensions_provider/full_page_dimensions_provider.dart)):
the inset is the canvas's bottom edge minus the visual viewport's (Flutter takes the page height; a GPUI canvas
need not fill the page). None without a visual viewport, none on iOS while not editing (Flutter's `_iOSHeight`
rule: a viewport iOS hasn't resized back is not a keyboard), none while pinch-zoomed (gpui_web's own rule). It
fills `WindowInsets::ime.bottom`; `on_insets_changed` fires from the visual viewport's `resize`/`scroll` listeners
when the insets change, never from `prepare_frame` (GPUI forbids callbacks there). The VirtualKeyboard API is not
used: Flutter doesn't, it is Chromium-only, and it reports a size only when the page opts into `overlaysContent`.

## Tests and checks

Both modules are pure and run on the host as the `web-pure` crate of [patch-test.sh](../scripts/patch-test.sh)
([patch-tests.md](patch-tests.md)), so CI's `patch-tests` job runs them: `transitions` (17 rows: Flutter's four
listeners, then freeze, resume, bfcache, unload and rejected events), `current_state`, `every_legal_step_has_phases`,
and `insets` (8 rows, 3 of them Flutter's tests). Flutter's one lifecycle test (views registered and disposed) has
no counterpart: the web has one window.

In a browser, [hello](examples.md) logs `lifecycle <phase>` and `keyboard inset <px>`.
[web-checks.mjs](../scripts/web-checks.mjs) checks both (and the text-input checks of
[web-text-input.md](web-text-input.md)). In headless Chromium it hides the page with a tab opened in front (blur,
then `visibilitychange`) and shows it with `Target.activateTarget`, freezes and resumes it with
`Page.setWebLifecycleState`, and makes the visual viewport 300 px shorter. Turning focus emulation off is no way to
hide it: a fresh headless Chromium's only tab stays visible and focused, and the hidden phases arrived only at the
next lifecycle step, which made the old check flaky. Synthetic `pagehide`/`pageshow` cover the back/forward cache
and unloading.
