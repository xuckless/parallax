// Parallax's own test (not ported). See docs/android-tests.md and docs/native-view-android.md.
package io.github.xuckless.parallax;

import static org.junit.Assert.assertEquals;
import static org.mockito.Mockito.mock;

import android.app.Activity;
import android.graphics.Rect;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.Robolectric;
import org.robolectric.annotation.Config;

/**
 * NativeViews' View hosting: Android focus follows GPUI's focused native view (a forwarded tap
 * never takes it), Views stack in GPUI's paint order, and a clip reaches the View. One row per
 * rule.
 */
@Config(shadows = {ShadowParallaxSurfaceView.class})
@RunWith(AndroidJUnit4.class)
public class NativeViewsTest {
  private static final int HANDLE = 7;

  @Before
  public void setUp() {
    // Attached to a window, GPUI's view sends its insets.
    ShadowParallaxSurfaceView.renderer = mock(ShadowParallaxSurfaceView.Renderer.class);
  }

  /** GPUI's surface view and its NativeViews in an Activity's window, as ParallaxActivity lays them out. */
  private static ParallaxSurfaceView inWindow() {
    Activity activity = Robolectric.buildActivity(Activity.class).setup().get();
    ParallaxSurfaceView surfaceView = new ParallaxSurfaceView(activity, 0, mock(PlatformPlugin.class));
    FrameLayout content = new FrameLayout(activity);
    content.addView(surfaceView.nativeViews.underlay);
    content.addView(surfaceView);
    activity.setContentView(content);
    surfaceView.requestFocus();
    return surfaceView;
  }

  private static View focusable(ParallaxSurfaceView surfaceView) {
    View view = new View(surfaceView.getContext());
    view.setFocusable(true);
    view.setFocusableInTouchMode(true);
    return view;
  }

  /** One row's actions on the NativeViews and its view. */
  private interface Steps {
    void run(NativeViews nativeViews, View view);
  }

  @Test
  public void androidFocusFollowsGpuiFocus() {
    // {name, steps, what has focus, the underlay's descendant focusability}
    Object[][] rows = {
      {
        "a view asking for focus (a WebView on a forwarded tap) does not get it",
        (Steps) (nativeViews, view) -> view.requestFocus(),
        "gpui",
        ViewGroup.FOCUS_BLOCK_DESCENDANTS,
      },
      {
        "GPUI focusing the view's handle gives it focus",
        (Steps) (nativeViews, view) -> nativeViews.focus(HANDLE),
        "view",
        ViewGroup.FOCUS_AFTER_DESCENDANTS,
      },
      {
        "GPUI focusing its own content takes focus back (keys and Back are GPUI's)",
        (Steps)
            (nativeViews, view) -> {
              nativeViews.focus(HANDLE);
              nativeViews.focus(-1);
            },
        "gpui",
        ViewGroup.FOCUS_BLOCK_DESCENDANTS,
      },
      {
        "after that the view cannot take focus itself",
        (Steps)
            (nativeViews, view) -> {
              nativeViews.focus(HANDLE);
              nativeViews.focus(-1);
              view.requestFocus();
            },
        "gpui",
        ViewGroup.FOCUS_BLOCK_DESCENDANTS,
      },
      {
        "a handle not attached keeps focus with GPUI",
        (Steps) (nativeViews, view) -> nativeViews.focus(99),
        "gpui",
        ViewGroup.FOCUS_BLOCK_DESCENDANTS,
      },
    };
    for (Object[] row : rows) {
      String name = (String) row[0];
      ParallaxSurfaceView surfaceView = inWindow();
      NativeViews nativeViews = surfaceView.nativeViews;
      View view = focusable(surfaceView);
      nativeViews.attach(HANDLE, view);
      nativeViews.place(0, new int[] {HANDLE, 1, 0, 0, 300, 200, -1, 0, 0, 300, 200});
      ((Steps) row[1]).run(nativeViews, view);
      String focused = view.isFocused() ? "view" : surfaceView.isFocused() ? "gpui" : "none";
      assertEquals(name, row[2], focused);
      assertEquals(name, row[3], nativeViews.underlay.getDescendantFocusability());
    }
  }

  @Test
  public void viewsStackInPaintOrderAndClip() {
    // {name, placement record of view 7 after view 8 at layer -1, view 7's Z, its clip bounds}
    Object[][] rows = {
      {
        "painted first: under the other view",
        new int[] {HANDLE, 1, 0, 0, 300, 200, -2, 0, 0, 300, 200},
        -2f,
        null,
      },
      {
        "painted last: over it",
        new int[] {HANDLE, 1, 0, 0, 300, 200, -1, 0, 0, 300, 200},
        -1f,
        null,
      },
      {
        "clipped by a GPUI content mask: only the visible part draws",
        new int[] {HANDLE, 1, 0, 0, 300, 200, -1, 0, 50, 300, 200},
        -1f,
        new Rect(0, 50, 300, 200),
      },
    };
    for (Object[] row : rows) {
      String name = (String) row[0];
      ParallaxSurfaceView surfaceView = inWindow();
      NativeViews nativeViews = surfaceView.nativeViews;
      View view = focusable(surfaceView);
      // Attached before view 8, so attach order alone would put it under.
      nativeViews.attach(HANDLE, view);
      nativeViews.attach(8, focusable(surfaceView));
      nativeViews.place(0, new int[] {8, 1, 100, 100, 300, 200, -1, 0, 0, 300, 200});
      nativeViews.place(0, (int[]) row[1]);
      assertEquals(name, (float) row[2], view.getZ(), 0f);
      assertEquals(name, row[3], view.getClipBounds());
    }
  }
}
