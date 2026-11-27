# Web text input

How a GPUI text field talks to browser IMEs, and what [patch 0034](../patches/0034-web-text-input.patch) ports from
Flutter's web engine ([`text_editing/`](https://github.com/flutter/flutter/tree/3.50.1/engine/src/flutter/lib/web_ui/lib/src/engine/text_editing)).

## The mirror

Upstream gpui_web keeps one hidden `<textarea>`, the IME mirror ([ime_mirror.rs](../upstream/gpui-pre-web/src/ime_mirror.rs)),
focused while the window is: it mirrors 512 UTF-16 units around the caret, and `input` events are diffed against it
to recover each IME edit ([events.rs](../upstream/gpui-pre-web/src/events.rs) `register_input`). Flutter instead
gives each field its own `<input>` or `<textarea>`, so Flutter's rules port and its elements don't.

## What 0034 changes

| Flutter rule (3.50.1) | Upstream gpui_web | Now |
|---|---|---|
| `autocomplete`: the autofill hint's token (`BrowserAutofillHints`), else `off` | `on` when `suggestions` | Flutter's |
| `enterkeyhint` only on iOS and Android (`configureInputAction`) | set everywhere | Flutter's |
| `inputmode` per keyboard type (`EngineInputType.inputmodeAttribute`) | always `text` | Flutter's, written whenever touch input enables the keyboard; `Search` → `search` is GPUI's own |
| `autocorrect`, `autocapitalize` (`setAutocapitalizeAttribute`) | same | unchanged |
| `obscureText` → `<input type=password>` (`createDomElement`, `applyConfiguration`) | always a `<textarea>` | the mirror is swapped for an `<input type=password>` while the focused field is `secure`, and back |
| Enter in an `<input>` is a keydown (`maybeSendAction`) | touch leaves Enter to `insertLineBreak` | an `<input>`'s Enter is dispatched as a key on touch too |
| `placeElement`: the element sits on the field, so IME popups open there; not mid-composition (`updateElementPlacement`) | fixed at the page's top-left | `update_ime_position` moves the mirror to the caret, on desktop only (on touch, iOS would pan the page to a field near the keyboard) |

The rules are one pure module, `text_editing.rs` (`attributes`, `element_kind`, `input_mode`, `infer_edit`);
`ImeMirror::apply_configuration` swaps the element when its kind changes (placement, `inputmode`, `readonly` and focus
carry over; the next sync writes the text afresh) and writes the attributes in one loop. The IME listeners sit on a
`display: contents` container around the element, so a swap leaves them alone. GPUI has no multiline flag and an
`<input>` strips newlines, so only secure fields get one. Flutter's `AutofillInfo` resets a secure `<input>` to
`type=text` when its hint lacks "password"; GPUI keeps `password`. Apps get the placement by calling
`Window::invalidate_character_coordinates` after a paint that moved the caret, as [hello's text field](../examples/hello/src/text_field.rs) does: GPUI runs
the query at the next frame, before its layout, so asking earlier reads the old caret.

## The tests

Table rows named after Flutter's tests ([text_editing_test.dart](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/lib/web_ui/test/engine/text_editing_test.dart)),
run on the host by [patch-test.sh](../scripts/patch-test.sh) as the `web-pure` crate ([patch-tests.md](patch-tests.md)),
so CI's `patch-tests` job runs them:

- `element_kinds`: Flutter's "Knows how to create password fields" and "password autofill hint", plus 3 GPUI rows.
- `configuration_attributes`: 14 rows (autocorrect on/off, autofill off and hinted, password hint, four
  capitalizations, text actions on mobile and desktop, spellcheck). Against upstream, 3 failed: the desktop
  text-action row and the two hint rows.
- `input_modes`: Flutter's "sets correct input type" cases (Android, iOS, Firefox on Android). All failed upstream.
- `edits`: Flutter's 7 `TextEditingDeltaState` cases plus upstream's caret-anchored case, against upstream's own diff
  (moved, unchanged, into `infer_edit`). All passed upstream.

Not ported, by design: blur and refocus strategies per browser (GPUI's focus owns the connection, not the DOM),
autofill forms, the delta model, inverted selections (the import diffs offsets, so direction never matters),
Firefox's `keyup`/`select` selection polling (`FirefoxTextEditingStrategy`): Firefox 142 fires `selectionchange` for
a text control at the element, and it bubbles to the document listener gpui_web has.

## Checking it in a browser

Build and serve hello ([examples-run.md](examples-run.md)), start headless Chromium as
[record-traces.sh](../scripts/record-traces.sh)'s `web_setup` does, then run
`node scripts/web-checks.mjs <port> <url> desktop` and `… mobile` ([web-checks.mjs](../scripts/web-checks.mjs)):
it taps each field, reads the mirror's attributes and position (`Runtime.evaluate`), composes with
`Input.imeSetComposition`, and fakes an Android phone with touch emulation and its user agent. Verified on Chromium 158 (2026-11-27): desktop attributes, placement at the
caret and none mid-composition (`Input.imeSetComposition`); Android `inputmode=email`, `enterkeyhint=done`. Firefox
and Safari are left for the owner.
