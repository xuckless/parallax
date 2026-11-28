# Touch drags: claimed at down, or won past the slop

How an element takes a finger drag from GPUI's touch recognizer (`TouchGestureRecognizer` in `gestures.rs`).
[Patch 0044](../patches/0044-touch-drag-offer.patch) adds the second way, ported from Flutter's
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

GPUI's pan stands in for the drag recognizers of the scroll containers under the touch
([patch 0045](../patches/0045-pan-follows-scroll-axes.patch)): each scrolling div reports
its axes while the touch's `Started` is dispatched (`Window::scroll_container_under_touch`), and the pan accepts by them:

| Containers under the touch | Flutter recognizer | The pan takes the touch once | Locked to |
|---|---|---|---|
| only vertical ones | `VerticalDragGestureRecognizer` | the travel along y passes `touch_slop` | y |
| only horizontal ones | `HorizontalDragGestureRecognizer` | the travel along x passes `touch_slop` | x |
| none, a two-axis one, or both kinds | `PanGestureRecognizer`'s place | the travel in any direction passes `touch_slop` | the dominant axis |

So in a vertical list a horizontal drag wins sideways moves, and a `Pan` drag (a desktop field) wins a mostly
horizontal one past 36 px; outside a one-axis container a `Pan` drag only wins a move that crosses both slops at once.
`list()` reports y, its one axis, the same way. To add a kind: one arm in each of `TouchDragKind`'s two methods.
A touch the pan has not taken yet is no tap or long press once it strays past `touch_slop` from where it went down
(`PressSlop`, Flutter's `preAcceptSlopTolerance`): a sideways swipe in a vertical list never clicks its row.

## Who uses it

The behaviour table's `SelectionRules::drag_recognizer` is a `TouchDragKind` ([behaviour.md](behaviour.md)); parallax-ext's
`TextSelection::touch_drag_kind` returns it, and [hello's text field](../examples/hello/src/text_field.rs) competes
with it for every touch on the field that isn't on a handle or the toolbar, then feeds the won drag to the selection
engine as a touch press at the start and its moves ([text-selection.md](text-selection.md)).

## Tests

`gestures::recognizer_tests::flutter_recognizer_tests_pass`: 6 rows from `tap_and_drag_test.dart` (their
`VerticalDragGestureRecognizer` as a vertical container), 2 rows for monodrag.dart's one-axis acceptance, 3 press-slop rows and 5 GPUI rows
(a text field's pair: a claimed long press plus a competing drag, one in a vertical list); `pan_axes_join_…` tables
how containers combine. Run `scripts/patch-test.sh recognizer`.
hello's `touch_selects_by_the_android_row` drives the field with touches through GPUI's test platform.

## Not ported

A drag after a double tap (Flutter's consecutive tap count on the drag), `eagerVictoryOnDrag: false`, winning by
default when no pan competes, and drag-update throttling.
