// Ported from facebook/react-native
// packages/react-native/ReactAndroid/src/main/java/com/facebook/react/uimanager/JSTouchDispatcher.kt
// @ v0.88.1 (c30b7f86850d47422637a64c70508e7411dbd8ea), Kotlin to Java. Copyright (c) Meta Platforms, Inc. and
// affiliates. MIT, see third_party/react-native/LICENSE.
//
// Kept: handleTouchEvent's gesture tracking (the target is found on ACTION_DOWN and keeps every
// event until ACTION_UP or ACTION_CANCEL) and its log messages. The target is a native view's
// handle, found by GPUI's hit test, and each event goes to the view itself, offset into its
// coordinates (NativeViews.dispatchTouchEvent, Flutter's translateMotionEvent), not to JS.
// Parallax's own (not in RN): the view's touch mode (NativeViews.touches) decides at the down
// whether the gesture goes to it and whether GPUI gets it too; handleGenericMotionEvent sends wheel
// turns the same way. How it fits together: docs/native-view-android.md.
package io.github.xuckless.parallax;

import android.util.Log;
import android.view.MotionEvent;

/**
 * Forwards touch gestures that start on a native view the app asked to forward to (GPUI's hit
 * test says the view is topmost there, no GPUI element over it) to that Android View; GPUI gets
 * the same events too unless the view takes them exclusively. ParallaxSurfaceView calls {@link
 * #handleTouchEvent} for every touch, before GPUI's own processor.
 */
final class NativeViewTouchDispatcher {
  private static final String TAG = "NativeViewTouch";

  /** GPUI's answer for a touch down: the handle of the native view topmost there, or -1. */
  interface HitTest {
    int nativeViewAt(float x, float y);
  }

  private final HitTest hitTest;

  private final NativeViews nativeViews;

  /** The native view the current gesture goes to, or -1: none (RN's targetTag). */
  private int targetTag = -1;

  /** Whether the current gesture is the view's alone (TOUCHES_EXCLUSIVE). */
  private boolean exclusive;

  NativeViewTouchDispatcher(HitTest hitTest, NativeViews nativeViews) {
    this.hitTest = hitTest;
    this.nativeViews = nativeViews;
  }

  /** Returns whether the event is the view's alone, so GPUI must not get it. */
  boolean handleTouchEvent(MotionEvent ev) {
    int action = ev.getActionMasked();
    if (action == MotionEvent.ACTION_DOWN) {
      if (targetTag != -1) {
        Log.e(TAG, "Got DOWN touch before receiving UP or CANCEL from last gesture");
      }

      // First event for this gesture. We expect tag to be set to -1, and we use the hit test to
      // find the native view that will handle this gesture, if any.
      int hit = hitTest.nativeViewAt(ev.getX(), ev.getY());
      int mode = nativeViews.touches(hit);
      targetTag = mode == NativeViews.TOUCHES_OFF ? -1 : hit;
      exclusive = mode == NativeViews.TOUCHES_EXCLUSIVE;
      dispatch(ev);
      return exclusive;
    } else if (targetTag == -1) {
      // The gesture started on GPUI's own content, or on a view that is not forwarded to.
      return false;
    }
    boolean alone = exclusive;
    if (action == MotionEvent.ACTION_UP) {
      // End of the gesture. We reset target tag to -1 and expect no further event associated with
      // this gesture.
      dispatch(ev);
      targetTag = -1;
    } else if (action == MotionEvent.ACTION_MOVE
        || action == MotionEvent.ACTION_POINTER_DOWN
        || action == MotionEvent.ACTION_POINTER_UP) {
      dispatch(ev);
    } else if (action == MotionEvent.ACTION_CANCEL) {
      dispatch(ev);
      targetTag = -1;
    } else {
      Log.w(TAG, "Warning : touch event was ignored. Action=" + action + " Target=" + targetTag);
    }
    return alone;
  }

  /**
   * A wheel turn over a native view whose touches are forwarded goes to it as well (or alone,
   * TOUCHES_EXCLUSIVE). Returns whether it is the view's alone.
   */
  boolean handleGenericMotionEvent(MotionEvent ev) {
    if (ev.getActionMasked() != MotionEvent.ACTION_SCROLL) {
      return false;
    }
    int hit = hitTest.nativeViewAt(ev.getX(), ev.getY());
    int mode = nativeViews.touches(hit);
    if (mode != NativeViews.TOUCHES_OFF) {
      nativeViews.dispatchGenericMotionEvent(hit, ev);
    }
    return mode == NativeViews.TOUCHES_EXCLUSIVE;
  }

  private void dispatch(MotionEvent ev) {
    if (targetTag != -1) {
      nativeViews.dispatchTouchEvent(targetTag, ev);
    }
  }
}
