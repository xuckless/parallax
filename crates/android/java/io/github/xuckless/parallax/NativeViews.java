// Parallax's own glue (not ported). Lifecycle lessons read from Flutter's
// engine/src/flutter/shell/platform/android/io/flutter/plugin/platform/PlatformViewsController2.java
// @ 3.50.1 as reference: views laid out with FrameLayout params at the frame's bounds, hidden with
// GONE, removed from their parent eagerly on dispose; SurfaceControls checked with isValid() and
// reparented when the host surface is recreated; transactions applied with the frame. How it fits
// together: docs/native-view-android.md.
package io.github.xuckless.parallax;

import android.content.Context;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.os.Build;
import android.util.Log;
import android.util.SparseArray;
import android.util.SparseIntArray;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.SurfaceControl;
import android.view.SurfaceControlViewHost;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;

/**
 * The app's native views of one Activity, shown below GPUI's {@link ParallaxSurfaceView} through
 * the holes GPUI cuts (parallax-android, crates/android/src/native_view.rs, decides where; this
 * class only acts). Main thread only.
 *
 * <p>Each view is {@link Hosted} one of three ways, picked by what the app passed: a {@link View}
 * laid out in {@link #underlay}, a FrameLayout under GPUI's view ({@link ViewHosted}); a {@link
 * SurfaceControl} with a buffer (API 29+), made a child of GPUI's own surface under its buffer
 * ({@link ControlHosted}); or a View wrapped in {@link InSurface} (API 34+), drawn by a {@link
 * SurfaceControlViewHost} whose surface is placed like a SurfaceControl ({@link PackageHosted}).
 * While any view is attached GPUI's surface is translucent and above the window, so the window's
 * views show through its holes.
 */
final class NativeViews {
  private static final String TAG = "NativeViews";

  /** Ints per view in {@link #place}'s array; Placement::record in native_view_placement.rs. */
  static final int RECORD = 11;

  // How a view takes the touches that start on it: TouchForwarding's discriminants in
  // native_view.rs. GPUI's alone; the View's too; the View's alone.
  static final int TOUCHES_OFF = 0;
  static final int TOUCHES_ON = 1;
  static final int TOUCHES_EXCLUSIVE = 2;

  /** One frame's placement of one view, in GPUI surface pixels. */
  static final class Placement {
    final boolean shown;
    final int x;
    final int y;
    final int width;
    final int height;

    /** Paint order among the frame's views, -n (bottom) to -1 (top). */
    final int layer;

    /** The part of the view GPUI's content mask lets show, in the view's own pixels. */
    final Rect clip;

    Placement(int[] record, int i) {
      shown = record[i + 1] != 0;
      x = record[i + 2];
      y = record[i + 3];
      width = record[i + 4];
      height = record[i + 5];
      layer = record[i + 6];
      clip = new Rect(record[i + 7], record[i + 8], record[i + 9], record[i + 10]);
    }

    /** Whether the clip leaves the whole view showing. */
    boolean unclipped() {
      return clip.left <= 0 && clip.top <= 0 && clip.right >= width && clip.bottom >= height;
    }
  }

  /**
   * What one frame's placements collect: SurfaceControl changes that go with GPUI's next buffer,
   * and sizes that go to producers now (only apply() hands a size over, and the producer's next
   * buffer has it). Both made on first use: API 28 has no SurfaceControl.
   */
  final class Frame {
    private SurfaceControl.Transaction withFrame;
    private SurfaceControl.Transaction now;

    SurfaceControl.Transaction withFrame() {
      if (withFrame == null) {
        withFrame = new SurfaceControl.Transaction();
      }
      return withFrame;
    }

    SurfaceControl.Transaction now() {
      if (now == null) {
        now = new SurfaceControl.Transaction();
      }
      return now;
    }

    /** Sizes now; positions with GPUI's next buffer (API 34+) or at once (API 29-33). */
    void apply() {
      if (now != null) {
        now.apply();
      }
      if (withFrame == null) {
        return;
      }
      if (Build.VERSION.SDK_INT >= 34) {
        surfaceView.applyTransactionToFrame(withFrame);
      } else {
        withFrame.apply();
      }
    }
  }

  /** How one attached view is shown. To host another kind, add an implementation and its row in {@link #attach}. */
  interface Hosted {
    /** What the app passed to attach. */
    Object source();

    /** The View touches, wheel turns and focus go to; null when it takes none from here. */
    View view();

    void place(Placement placement, Frame frame);

    /** GPUI's surface is new (or first exists): a child surface moves to it, hidden. */
    void reparent(SurfaceControl.Transaction transaction);

    void detach();
  }

  /** An ordinary View, laid out in the underlay: in this vsync's traversal, a frame behind GPUI's. */
  private final class ViewHosted implements Hosted {
    private final Object source;
    private final View view;

    ViewHosted(Object source, View view) {
      this.source = source;
      this.view = view;
      view.setVisibility(View.GONE);
      underlay.addView(view, new FrameLayout.LayoutParams(0, 0, Gravity.LEFT | Gravity.TOP));
    }

    @Override
    public Object source() {
      return source;
    }

    @Override
    public View view() {
      return view;
    }

    @Override
    public void place(Placement placement, Frame frame) {
      if (!placement.shown) {
        view.setVisibility(View.GONE);
        return;
      }
      FrameLayout.LayoutParams params =
          new FrameLayout.LayoutParams(
              placement.width, placement.height, Gravity.LEFT | Gravity.TOP);
      params.leftMargin = placement.x;
      params.topMargin = placement.y;
      view.setLayoutParams(params);
      view.setClipBounds(placement.unclipped() ? null : placement.clip);
      // The underlay draws (and hit-tests) its children by Z: GPUI's paint order. Negative, so no
      // elevation shadow.
      
      view.setVisibility(View.VISIBLE);
    }

    @Override
    public void reparent(SurfaceControl.Transaction transaction) {
      // In the window's own hierarchy: GPUI's surface is not its parent.
    }

    @Override
    public void detach() {
      underlay.removeView(view);
    }
  }

  /** A SurfaceControl under GPUI's buffer, moved with GPUI's frame (API 34+). API 29+. */
  private final class ControlHosted implements Hosted {
    private final Object source;
    private final SurfaceControl control;

    /** Whether its buffer size follows the slot: the app's own buffer, not a view host's. */
    private final boolean sizesBuffer;

    ControlHosted(Object source, SurfaceControl control, boolean sizesBuffer) {
      this.source = source;
      this.control = control;
      this.sizesBuffer = sizesBuffer;
      reparented(new SurfaceControl.Transaction()).apply();
    }

    @Override
    public Object source() {
      return source;
    }

    @Override
    public View view() {
      return null;
    }

    @Override
    public void place(Placement placement, Frame frame) {
      if (!control.isValid()) {
        return;
      }
      SurfaceControl.Transaction transaction = frame.withFrame();
      if (placement.shown) {
        if (sizesBuffer) {
          frame.now().setBufferSize(control, placement.width, placement.height);
        }
        transaction.setPosition(control, placement.x, placement.y);
        transaction.setLayer(control, placement.layer);
        // No crop before API 33 (setGeometry would scale a stale buffer): a clipped view shows
        // whole there.
        if (Build.VERSION.SDK_INT >= 33) {
          transaction.setCrop(control, placement.unclipped() ? null : placement.clip);
        }
      }
      transaction.setVisibility(control, placement.shown);
    }

    @Override
    public void reparent(SurfaceControl.Transaction transaction) {
      reparented(transaction);
    }

    /** Under GPUI's buffer (a negative layer among its surface's children), hidden. */
    private SurfaceControl.Transaction reparented(SurfaceControl.Transaction transaction) {
      SurfaceControl parent = surfaceView.getSurfaceControl();
      if (parent == null || !parent.isValid() || !control.isValid()) {
        return transaction;
      }
      return transaction
          .reparent(control, parent)
          .setLayer(control, -1)
          .setVisibility(control, false);
    }

    @Override
    public void detach() {
      if (control.isValid()) {
        new SurfaceControl.Transaction().reparent(control, null).apply();
      }
    }
  }

  /**
   * A View drawn by a SurfaceControlViewHost (API 34+: SurfacePackage.getSurfaceControl), its
   * surface placed like a SurfaceControl, so it moves in step with GPUI's frame. It is its own
   * window: its touches and focus are its own, not forwarded from here.
   */
  private final class PackageHosted implements Hosted {
    private final Object source;
    private final SurfaceControlViewHost host;
    private final ControlHosted control;
    private int width;
    private int height;

    PackageHosted(Object source, View view) {
      this.source = source;
      host =
          new SurfaceControlViewHost(
              context(), surfaceView.getDisplay(), surfaceView.getHostToken());
      // A size before the first placement: setView lays the view out at once.
      host.setView(view, 1, 1);
      control =
          new ControlHosted(
              source, host.getSurfacePackage().getSurfaceControl(), /* sizesBuffer= */ false);
    }

    @Override
    public Object source() {
      return source;
    }

    @Override
    public View view() {
      return null;
    }

    @Override
    public void place(Placement placement, Frame frame) {
      if (placement.shown && (placement.width != width || placement.height != height)) {
        width = placement.width;
        height = placement.height;
        // Laid out in the host's next traversal: its buffer at the new size may come a frame
        // after the move (the underlay's background shows meanwhile).
        host.relayout(width, height);
      }
      control.place(placement, frame);
    }

    @Override
    public void reparent(SurfaceControl.Transaction transaction) {
      control.reparent(transaction);
    }

    @Override
    public void detach() {
      control.detach();
      host.release();
    }
  }

  /** The views' parent, the Activity's content view's first child: under GPUI's surface view. */
  final FrameLayout underlay;

  private final ParallaxSurfaceView surfaceView;

  /** Attached views by the handle Rust gave them. */
  private final SparseArray<Hosted> views = new SparseArray<>();

  /** TOUCHES_* by handle; absent is TOUCHES_OFF. */
  private final SparseIntArray touches = new SparseIntArray();

  /** Whether GPUI's surface is translucent and above the window: while any view is attached. */
  private boolean translucent;

  NativeViews(Context context, ParallaxSurfaceView surfaceView) {
    this.surfaceView = surfaceView;
    underlay = new FrameLayout(context);
    // Insets are GPUI's: without this, a view that fits system windows could consume them before
    // GPUI's view, the next sibling, sees them (API 28-29 stop at the first consumer).
    underlay.setOnApplyWindowInsetsListener((view, insets) -> insets);
    // Keys and Back stay GPUI's: a forwarded tap must not move Android focus into a view (a
    // WebView takes it on touch). focus() lets it in while GPUI focuses the view's handle.
  }

  /** The Activity: the app builds its views with it (parallax_android::with_activity). */
  Context context() {
    return surfaceView.getContext();
  }

  /**
   * Adds the app's view, hidden until {@link #place} shows it. Returns false for an object no
   * {@link Hosted} takes. Rows in order: the first whose type and API level match hosts it.
   */
  boolean attach(int handle, Object view) {
    Hosted hosted;
    if (view instanceof InSurface && Build.VERSION.SDK_INT >= 34) {
      hosted = new PackageHosted(view, ((InSurface) view).view);
    } else if (view instanceof InSurface && ((InSurface) view).view.getParent() == null) {
      Log.w(TAG, "InSurface needs API 34: hosted as an ordinary View");
      hosted = new ViewHosted(view, ((InSurface) view).view);
    } else if (view instanceof View && ((View) view).getParent() == null) {
      hosted = new ViewHosted(view, (View) view);
    } else if (Build.VERSION.SDK_INT >= 29 && view instanceof SurfaceControl) {
      hosted = new ControlHosted(view, (SurfaceControl) view, /* sizesBuffer= */ true);
    } else {
      Log.e(TAG, "not a native view: " + view);
      return false;
    }
    views.put(handle, hosted);
    // Not now: a surface format change recreates GPUI's surface, whose callbacks reach GPUI, which
    // is mid-update here (NativeView::new).
    surfaceView.post(this::followViews);
    return true;
  }

  /** Removes the view: the app's NativeView handle went. */
  void detach(int handle) {
    Hosted hosted = views.get(handle);
    views.remove(handle);
    touches.delete(handle);
    if (hosted != null) {
      hosted.detach();
    }
    surfaceView.post(this::followViews);
  }

  /**
   * One frame's placements: per view {@link #RECORD} ints, {handle, shown (1 or 0), x, y, width,
   * height, layer, clip left, top, right, bottom}. Views GPUI did not move are not listed.
   * `background` (ARGB, 0 for none) shows wherever a view has not caught up with its hole
   * (NativeView::set_background).
   */
  void place(int background, int[] placements) {
    underlay.setBackgroundColor(background);
    Frame frame = new Frame();
    for (int i = 0; i + RECORD <= placements.length; i += RECORD) {
      Hosted hosted = views.get(placements[i]);
      if (hosted != null) {
        hosted.place(new Placement(placements, i), frame);
      }
    }
    frame.apply();
  }

  /**
   * GPUI's surface was recreated (a format change, back from the background): its SurfaceControl
   * is a new one, so child surfaces move to it, hidden until the next frame places them (Rust
   * forgets their placement with the old surface).
   */
  void surfaceCreated() {
    if (Build.VERSION.SDK_INT < 29) {
      return;
    }
    SurfaceControl.Transaction transaction = new SurfaceControl.Transaction();
    for (int i = 0; i < views.size(); i++) {
      views.valueAt(i).reparent(transaction);
    }
    transaction.apply();
  }

  /**
   * Sets how the touches that start on `view` (what the app attached) go: TOUCHES_*. False when
   * it is not attached here.
   */
  boolean forwardTouches(Object view, int mode) {
    for (int i = 0; i < views.size(); i++) {
      if (views.valueAt(i).source() == view) {
        touches.put(views.keyAt(i), mode);
        return true;
      }
    }
    return false;
  }

  /** How touches starting on view `handle` (-1: none) go: TOUCHES_*. */
  int touches(int handle) {
    return touches.get(handle, TOUCHES_OFF);
  }

  /**
   * GPUI focused view `handle`'s focus handle, or nothing of a view (-1): Android focus (keys,
   * Back, the IME) follows. A view without a View of its own leaves it with GPUI's view.
   */
  void focus(int handle) {
    Hosted hosted = views.get(handle);
    View target = hosted == null ? null : hosted.view();
    if (target == null) {
      underlay.setDescendantFocusability(ViewGroup.FOCUS_BLOCK_DESCENDANTS);
      surfaceView.requestFocus();
    } else {
      underlay.setDescendantFocusability(ViewGroup.FOCUS_AFTER_DESCENDANTS);
      target.requestFocus();
    }
  }

  /** Sends a touch to view `handle`, offset to its own coordinates. */
  void dispatchTouchEvent(int handle, MotionEvent event) {
    View target = target(handle);
    if (target == null) {
      return;
    }
    MotionEvent local = offset(event, target);
    target.dispatchTouchEvent(local);
    local.recycle();
  }

  /** Sends a wheel turn (or other generic motion) to view `handle`, offset to its coordinates. */
  void dispatchGenericMotionEvent(int handle, MotionEvent event) {
    View target = target(handle);
    if (target == null) {
      return;
    }
    MotionEvent local = offset(event, target);
    target.dispatchGenericMotionEvent(local);
    local.recycle();
  }

  private View target(int handle) {
    Hosted hosted = views.get(handle);
    return hosted == null ? null : hosted.view();
  }

  private static MotionEvent offset(MotionEvent event, View target) {
    MotionEvent local = MotionEvent.obtain(event);
    local.offsetLocation(-target.getLeft(), -target.getTop());
    return local;
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
