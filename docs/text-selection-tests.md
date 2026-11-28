# Text selection: tests, live checks and gaps

How [text-selection.md](text-selection.md)'s engine is tested, checked live, and what it leaves out.

## Tests

[tests.rs](../crates/ext/src/text_selection/tests.rs): 57 gesture rows (Flutter test names from
`text_selection_test.dart` and `text_field_test.dart`, on a 10 px monospace layout), handle geometry, `_getHandleDy`,
toolbar anchors and placement, toolbar buttons (`editable_text_test.dart`), affinity at a soft wrap (a wrapped
layout), the magnifier's positioning tests and `shiftWithinBounds`. `text_selection_table` in
[behaviour.rs](../crates/platform/src/behaviour.rs) checks the rows. hello's `mouse_selects_by_the_desktop_row` drives
the field through GPUI's test platform with mouse events, `touch_selects_by_the_android_row` with touches on
Android's row (tap, long press, a horizontal finger drag moving the caret, a vertical drag left to the pan).

## Live checks

[weston-selection-checks.sh](../scripts/weston-selection-checks.sh) runs hello under Weston's headless backend
([traces-desktop.md](traces-desktop.md#linux-weston-headless), CI's `linux-touch` job), types `abc def ghi` into the
first field, finds its glyphs on screen (`locate-colour.py ffffff ink`), then sends real mouse and `wl_touch` input
through Weston's test plugin: click, click left of the text, double and triple click, a drag backwards, a double-click
drag (word by word), a shift-click, a touch long press, and a long press then an end-handle drag. Each row checks
hello's last `text field selection: base..extent` log line (offsets only, never the text). Run it with
`WESTON_RUN="podman exec <container>"` where Weston's build dependencies live in a container; add a check as a row.

## Not ported (yet)

- A finger drag right after a double tap (word by word): GPUI's won drag carries no tap count.
- Scrolling fields (drag offsets, edge scroll), read-only fields, right-to-left handles, spell check, force press,
  the floating cursor, Share / Look Up / Search Web / Live Text, the toolbar's styles (Material's).
- Web on an iOS host takes the iOS row, so it has no selection gestures.

## Other implementations

longbridge/gpui-component (`crates/base/src/input/base/touch.rs`, `crates/component/src/touch_selection/`, read at
`0d9850db`) has touch selection for its own `InputState`: long press selects a word, handles, an edit menu. It is
tied to its input widget, has no magnifier or per-platform rules, and lets handles cross (Flutter's Android forbids
that), so nothing was taken from it.
