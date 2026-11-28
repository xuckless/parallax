// Parallax's own glue (not ported). Lifecycle lessons read from Flutter's
// engine/src/flutter/shell/platform/android/io/flutter/plugin/platform/PlatformViewsController2.java
// @ 3.50.1 as reference: views laid out with FrameLayout params at the frame's bounds, hidden with
// GONE, removed from their parent eagerly on dispose; SurfaceControls checked with isValid() and
// reparented when the host surface is recreated; transactions applied with the frame. How it fits
// together: docs/native-view-android.md.
package io.github.xuckless.parallax;

import android.content.Context;
import android.graphics.PixelFormat;
import android.os.Build;
import android.util.Log;
import android.util.SparseArray;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.SurfaceControl;
import android.view.View;
import android.widget.FrameLayout;

/**
 * The app's native views of one Activity, shown below GPUI's {@link ParallaxSurfaceView} through
 * the holes GPUI cuts (parallax-android, crates/android/src/native_view.rs, decides where; this
 * class only acts). Main thread only.
 *
 * <p>A view is an {@link View} (a WebView, a MapView, the app's own SurfaceView), laid out in
 * {@link #underlay}, a FrameLayout under GPUI's view, or (API 29+) a {@link SurfaceControl} with a
 * buffer, made a child of GPUI's own surface, under its buffer, and placed in a transaction that
 * goes with GPUI's next frame (API 34+). While any view is attached GPUI's surface is translucent
 * and above the window, so the window's views show through its holes.
 */
final class NativeViews {
  private static final String TAG = "NativeViews";

  /** The views' parent, the Activity's content view's first child: under GPUI's surface view. */
  final FrameLayout underlay;

  private final ParallaxSurfaceView surfaceView;

  /** Attached views by the handle Rust gave them: a View or a SurfaceControl. */
  private final SparseArray<Object> views = new SparseArray<>();

  /** Whether GPUI's surface is translucent and above the window: while any view is attached. */
  private boolean translucent;

  NativeViews(Context context, ParallaxSurfaceView surfaceView) {
    this.surfaceView = surfaceView;
    underlay = new FrameLayout(context);
    // Insets are GPUI's: without this, a view that fits system windows could consume them before
    // GPUI's view, the next sibling, sees them (API 28-29 stop at the first consumer).
    underlay.setOnApplyWindowInsetsListener((view, insets) -> insets);
  }

  /** The Activity: the app builds its views with it (parallax_android::with_activity). */
  Context context() {
    return surfaceView.getContext();
  }

  /**
   * Adds the app's view, hidden until {@link #place} shows it. Returns false for an object that is
   * neither a View without a parent nor (API 29+) a SurfaceControl.
   */
  boolean attach(int handle, Object view) {
    if (view instanceof View && ((View) view).getParent() == null) {
      View child = (View) view;
      child.setVisibility(View.GONE);
      underlay.addView(child, new FrameLayout.LayoutParams(0, 0, Gravity.LEFT | Gravity.TOP));
    } else if (Build.VERSION.SDK_INT >= 29 && view instanceof SurfaceControl) {
      reparent((SurfaceControl) view, new SurfaceControl.Transaction()).apply();
    } else {
      Log.e(TAG, "not a native view: " + view);
      return false;
    }
    views.put(handle, view);
    // Not now: a surface format change recreates GPUI's surface, whose callbacks reach GPUI, which
    // is mid-update here (NativeView::new).
    surfaceView.post(this::followViews);
    return true;
  }

  /** Removes the view: the app's NativeView handle went. */
  void detach(int handle) {
    Object view = views.get(handle);
    views.remove(handle);
    if (view instanceof View) {
      underlay.removeView((View) view);
    } else if (Build.VERSION.SDK_INT >= 29 && view instanceof SurfaceControl) {
      SurfaceControl control = (SurfaceControl) view;
      if (control.isValid()) {
        new SurfaceControl.Transaction().reparent(control, null).apply();
      }
    }
    surfaceView.post(this::followViews);
  }

  /**
   * One frame's placements, in GPUI surface pixels: per view {handle, shown (1 or 0), x, y, width,
   * height}. Views GPUI did not move are not listed. A View is laid out in this vsync's traversal;
   * a SurfaceControl moves with GPUI's next buffer (API 34+) or at once (API 29-33). Its buffer
   * size goes to its producer at once: only apply() hands a size over, and the app's next buffer
   * has it. `background` (ARGB, 0 for none) shows wherever a view has not caught up with its hole
   * (NativeView::set_background).
   */
  void place(int background, int[] placements) {
    underlay.setBackgroundColor(background);
    SurfaceControl.Transaction transaction = null;
    SurfaceControl.Transaction sizes = new SurfaceControl.Transaction();
    for (int i = 0; i + 5 < placements.length; i += 6) {
      Object view = views.get(placements[i]);
      boolean shown = placements[i + 1] != 0;
      int x = placements[i + 2];
      int y = placements[i + 3];
      int width = placements[i + 4];
      int height = placements[i + 5];
      if (view instanceof View) {
        placeView((View) view, shown, x, y, width, height);
      } else if (Build.VERSION.SDK_INT >= 29 && view instanceof SurfaceControl) {
        SurfaceControl control = (SurfaceControl) view;
        if (!control.isValid()) {
          continue;
        }
        if (transaction == null) {
          transaction = new SurfaceControl.Transaction();
        }
        if (shown) {
          sizes.setBufferSize(control, width, height);
          transaction.setPosition(control, x, y);
        }
        transaction.setVisibility(control, shown);
      }
    }
    sizes.apply();
    if (transaction == null) {
      return;
    }
    if (Build.VERSION.SDK_INT >= 34) {
      surfaceView.applyTransactionToFrame(transaction);
    } else {
      transaction.apply();
    }
  }

  /**
   * GPUI's surface was recreated (a format change, back from the background): its SurfaceControl
   * is a new one, so the SurfaceControl views move to it, hidden until the next frame places them
   * (Rust forgets their placement with the old surface).
   */
  void surfaceCreated() {
    if (Build.VERSION.SDK_INT < 29) {
      return;
    }
    SurfaceControl.Transaction transaction = new SurfaceControl.Transaction();
    for (int i = 0; i < views.size(); i++) {
      if (views.valueAt(i) instanceof SurfaceControl) {
        reparent((SurfaceControl) views.valueAt(i), transaction);
      }
    }
    transaction.apply();
  }

  /** Sends a touch Rust's hit test gave to view `handle`, offset to its own coordinates. */
  void dispatchTouchEvent(int handle, MotionEvent event) {
    Object view = views.get(handle);
    if (!(view instanceof View)) {
      return;
    }
    View target = (View) view;
    MotionEvent local = MotionEvent.obtain(event);
    local.offsetLocation(-target.getLeft(), -target.getTop());
    target.dispatchTouchEvent(local);
    local.recycle();
  }

  private static void placeView(View view, boolean shown, int x, int y, int width, int height) {
    if (!shown) {
      view.setVisibility(View.GONE);
      return;
    }
    FrameLayout.LayoutParams params =
        new FrameLayout.LayoutParams(width, height, Gravity.LEFT | Gravity.TOP);
    params.leftMargin = x;
    params.topMargin = y;
    view.setLayoutParams(params);
    view.setVisibility(View.VISIBLE);
  }

  /** Under GPUI's buffer (a negative layer among its surface's children), hidden. */
  private SurfaceControl.Transaction reparent(
      SurfaceControl control, SurfaceControl.Transaction transaction) {
    SurfaceControl parent = Build.VERSION.SDK_INT >= 29 ? surfaceView.getSurfaceControl() : null;
    if (parent == null || !parent.isValid() || !control.isValid()) {
      return transaction;
    }
    return transaction
        .reparent(control, parent)
        .setLayer(control, -1)
        .setVisibility(control, false);
  }

  /**
   * GPUI's surface is translucent and above the window while any view is attached, opaque and
   * below it (SurfaceView's defaults) otherwise. Either change recreates the surface on API 28-29
   * and a format change does on every version: the renderer attaches to the new one as after Home.
   */
  private void followViews() {
    boolean wanted = views.size() > 0;
    if (wanted == translucent) {
      return;
    }
    translucent = wanted;
    // The layer first, so the surface the format change recreates is made at it (API 28-29 read it
    // only then).
    surfaceView.setZOrderOnTop(wanted);
    surfaceView.getHolder().setFormat(wanted ? PixelFormat.TRANSLUCENT : PixelFormat.RGB_565);
    Log.i(TAG, "GPUI's surface is " + (wanted ? "translucent, above the window" : "opaque"));
  }
}
