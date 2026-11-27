# Touch drags: claimed at down, or won past the slop

How an element takes a finger drag from GPUI's touch recognizer (`TouchGestureRecognizer` in `gestures.rs`).
[Patch 0042](../patches/0042-touch-drag-past-slop.patch) adds the second way, ported from Flutter's
`tap_and_drag.dart` @ 3.50.1 (ledger: [PORTED.md](../third_party/flutter/PORTED.md)). The recognizer's tests and
trace format: [gesture-recognizers.md](gesture-recognizers.md).

## The two ways

At touch down the window offers the drag: listeners get a `Started` `TouchDragEvent` with `start: TouchDragStart::Down`.

| The listener calls | Flutter's arena | The touch then |
|---|---|---|
| nothing | no drag recognizer | stays a tap, long press or pan |
| `window.prevent_default()` | a drag recognizer alone in its arena: wins at the down | is the drag's at once (a selection handle) |
| `window.compete_for_touch_drag(kind)` | a tap-and-drag recognizer competing with the tap, long press and pan | stays a tap or long press to be; the drag wins once the touch passes `kind`'s slop |

A competing drag that wins gets a second `Started` event, `start: TouchDragStart::PastSlop(kind)`, at the touch's
position (`start_position` is still where it went down), then `Moved` events and `Ended` or `Cancelled`. One that
loses (a tap, a claimed long press, the pan, a held scroll) gets nothing more. A claim beats any competitor; the
deepest element to compete gets the drag.

## When the drag wins

On every move of a pending touch the competing drag runs first (Flutter routes the deeper recognizer first, so on a
move both accept, the drag wins), then the pan. The drag adds Flutter's `_globalDistanceMoved` per move and wins
when it passes the slop (`gpui::slops` from the window's `GestureTuning::touch_slop`):

| `TouchDragKind` | Flutter recognizer | Travel counted | Wins past |
|---|---|---|---|
| `Horizontal` | `TapAndHorizontalDragGestureRecognizer` | the signed horizontal travel | the hit slop (18 px for a finger) |
| `Pan` | `TapAndPanGestureRecognizer` | the path length | the pan slop (36 px) |

GPUI's pan stands in for a scroll view's drag recognizer: it takes the touch once it passes `touch_slop` in any
direction. So a horizontal drag wins mostly horizontal moves (a diagonal one past 18 px but under 18 px along x is
the pan's), and a `Pan` drag only wins a move that crosses both slops at once. To add a kind: one arm in each of
`TouchDragKind`'s two methods.

## Who uses it

The behaviour table's `SelectionRules::drag_recognizer` is a `TouchDragKind` ([behaviour.md](behaviour.md)); parallax-ext's
`TextSelection::touch_drag_kind` returns it, and [hello's text field](../examples/hello/src/text_field.rs) competes
with it for every touch on the field that isn't on a handle or the toolbar, then feeds the won drag to the selection
engine as a touch press at the start and its moves ([text-selection.md](text-selection.md)).

## Tests

`gestures::recognizer_tests::flutter_recognizer_tests_pass`: 6 rows from `tap_and_drag_test.dart` and 4 GPUI rows (a
text field's pair: a claimed long press plus a competing horizontal drag). Run `scripts/patch-test.sh recognizer`.
hello's `touch_selects_by_the_android_row` drives the field with touches through GPUI's test platform.

## Not ported

A drag after a double tap (Flutter's consecutive tap count on the drag), `eagerVictoryOnDrag: false`, winning by
default when no pan competes, and drag-update throttling.
