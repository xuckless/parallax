// Parallax's own test (not ported). See docs/android-tests.md and docs/native-view-android.md.
package io.github.xuckless.parallax;

import static org.junit.Assert.assertEquals;
import static org.mockito.Mockito.mock;

import android.content.Context;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.View;
import androidx.test.core.app.ApplicationProvider;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.annotation.Config;

/**
 * NativeViewTouchDispatcher (React Native's JSTouchDispatcher, ported) with NativeViews: a gesture
 * goes to the native view GPUI's hit test found on its touch down, every event offset into the
 * view's coordinates, until its up or cancel, if the view's touches are forwarded; GPUI gets it
 * too unless they are forwarded exclusively. Wheel turns follow the same modes. One row per rule.
 */
@Config(shadows = {ShadowParallaxSurfaceView.class})
@RunWith(AndroidJUnit4.class)
public class NativeViewTouchDispatcherTest {
  private static final int HANDLE = 7;

  /** The view, laid out at (100, 50), 300 × 200, by NativeViews.place. */
  private static final int LEFT = 100;

  private static final int TOP = 50;

  private final Context ctx = ApplicationProvider.getApplicationContext();

  /** A view that records what reaches it: "action x y". */
  private static final class RecordingView extends View {
    final List<String> events = new ArrayList<>();

    RecordingView(Context context) {
      super(context);
    }

    @Override
    public boolean dispatchGenericMotionEvent(MotionEvent event) {
      return dispatchTouchEvent(event);
    }

    @Override
    public boolean dispatchTouchEvent(MotionEvent event) {
      events.add(
          MotionEvent.actionToString(event.getActionMasked())
              + " "
              + (int) event.getX()
              + " "
              + (int) event.getY());
      return true;
    }
  }

  /** {action, x, y} in GPUI view pixels; the hit test finds the view inside its bounds. */
  private static int[] at(int action, int x, int y) {
    return new int[] {action, x, y};
  }

  /** A dispatcher over a NativeViews holding `view` at (LEFT, TOP) with touch mode `mode`. */
  private NativeViewTouchDispatcher dispatcher(RecordingView view, int mode) {
    ParallaxSurfaceView surfaceView = new ParallaxSurfaceView(ctx, 0, mock(PlatformPlugin.class));
    NativeViews nativeViews = surfaceView.nativeViews;
    nativeViews.attach(HANDLE, view);
    nativeViews.forwardTouches(view, mode);
    nativeViews.place(0, new int[] {HANDLE, 1, LEFT, TOP, 300, 200, -1, 0, 0, 300, 200});
    nativeViews.underlay.measure(
        View.MeasureSpec.makeMeasureSpec(1080, View.MeasureSpec.EXACTLY),
        View.MeasureSpec.makeMeasureSpec(2400, View.MeasureSpec.EXACTLY));
    nativeViews.underlay.layout(0, 0, 1080, 2400);
    return new NativeViewTouchDispatcher(
        (x, y) -> x >= LEFT && x < LEFT + 300 && y >= TOP && y < TOP + 200 ? HANDLE : -1,
        nativeViews);
  }

  @Test
  public void gesturesGoToTheViewTheirDownLandedOn() {
    final int down = MotionEvent.ACTION_DOWN;
    final int move = MotionEvent.ACTION_MOVE;
    final int up = MotionEvent.ACTION_UP;
    final int cancel = MotionEvent.ACTION_CANCEL;
    final int pointerDown = MotionEvent.ACTION_POINTER_DOWN;
    final int pointerUp = MotionEvent.ACTION_POINTER_UP;
    final int on = NativeViews.TOUCHES_ON;
    final int off = NativeViews.TOUCHES_OFF;
    final int exclusive = NativeViews.TOUCHES_EXCLUSIVE;
    // {name, touch mode, events, what the view gets, how many of the events GPUI gets}
    Object[][] rows = {
      {
        "a gesture on the view reaches it, offset into its coordinates, and GPUI",
        on,
        new int[][] {at(down, 150, 80), at(move, 160, 90), at(up, 160, 90)},
        new String[] {"ACTION_DOWN 50 30", "ACTION_MOVE 60 40", "ACTION_UP 60 40"},
        3,
      },
      {
        "a gesture that starts off the view stays GPUI's, even over the view",
        on,
        new int[][] {at(down, 10, 10), at(move, 150, 80), at(up, 150, 80)},
        new String[] {},
        3,
      },
      {
        "a gesture keeps its view after leaving it",
        on,
        new int[][] {at(down, 150, 80), at(move, 10, 10), at(up, 10, 10)},
        new String[] {"ACTION_DOWN 50 30", "ACTION_MOVE -90 -40", "ACTION_UP -90 -40"},
        3,
      },
      {
        "a cancel ends the gesture",
        on,
        new int[][] {at(down, 150, 80), at(cancel, 150, 80), at(move, 160, 90)},
        new String[] {"ACTION_DOWN 50 30", "ACTION_CANCEL 50 30"},
        3,
      },
      {
        "a second finger goes with the gesture",
        on,
        new int[][] {
          at(down, 150, 80), at(pointerDown, 150, 80), at(pointerUp, 150, 80), at(up, 150, 80)
        },
        new String[] {
          "ACTION_DOWN 50 30", "ACTION_POINTER_DOWN(0) 50 30", "ACTION_POINTER_UP(0) 50 30",
          "ACTION_UP 50 30"
        },
        4,
      },
      {
        "a view whose touches are not forwarded gets none",
        off,
        new int[][] {at(down, 150, 80), at(move, 160, 90), at(up, 160, 90)},
        new String[] {},
        3,
      },
      {
        "an exclusive view's gesture is its alone (no double scroll in a GPUI scroller)",
        exclusive,
        new int[][] {at(down, 150, 80), at(move, 160, 90), at(up, 160, 90)},
        new String[] {"ACTION_DOWN 50 30", "ACTION_MOVE 60 40", "ACTION_UP 60 40"},
        0,
      },
      {
        "a gesture off an exclusive view stays GPUI's",
        exclusive,
        new int[][] {at(down, 10, 10), at(move, 150, 80), at(up, 150, 80)},
        new String[] {},
        3,
      },
    };
    for (Object[] row : rows) {
      String name = (String) row[0];
      RecordingView view = new RecordingView(ctx);
      NativeViewTouchDispatcher dispatcher = dispatcher(view, (int) row[1]);
      long time = SystemClock.uptimeMillis();
      int gpui = 0;
      for (int[] event : (int[][]) row[2]) {
        MotionEvent motion = MotionEvent.obtain(time, time, event[0], event[1], event[2], 0);
        gpui += dispatcher.handleTouchEvent(motion) ? 0 : 1;
        motion.recycle();
      }
      assertEquals(name, Arrays.asList((String[]) row[3]), view.events);
      assertEquals(name + ": events GPUI gets", (int) row[4], gpui);
    }
  }

  @Test
  public void wheelTurnsFollowTheTouchMode() {
    // {name, touch mode, x, y, what the view gets, whether GPUI gets it}
    Object[][] rows = {
      {"not forwarded: GPUI's", NativeViews.TOUCHES_OFF, 150, 80, new String[] {}, true},
      {
        "forwarded: the view's too",
        NativeViews.TOUCHES_ON,
        150,
        80,
        new String[] {"ACTION_SCROLL 50 30"},
        true
      },
      {
        "exclusive: the view's alone",
        NativeViews.TOUCHES_EXCLUSIVE,
        150,
        80,
        new String[] {"ACTION_SCROLL 50 30"},
        false
      },
      {"off the view: GPUI's", NativeViews.TOUCHES_EXCLUSIVE, 10, 10, new String[] {}, true},
    };
    for (Object[] row : rows) {
      String name = (String) row[0];
      RecordingView view = new RecordingView(ctx);
      NativeViewTouchDispatcher dispatcher = dispatcher(view, (int) row[1]);
      long time = SystemClock.uptimeMillis();
      MotionEvent wheel =
          MotionEvent.obtain(time, time, MotionEvent.ACTION_SCROLL, (int) row[2], (int) row[3], 0);
      boolean gpui = !dispatcher.handleGenericMotionEvent(wheel);
      wheel.recycle();
      assertEquals(name, Arrays.asList((String[]) row[4]), view.events);
      assertEquals(name + ": GPUI gets it", row[5], gpui);
    }
  }
}
