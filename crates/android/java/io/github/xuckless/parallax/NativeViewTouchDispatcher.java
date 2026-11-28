// Ported from facebook/react-native
// packages/react-native/ReactAndroid/src/main/java/com/facebook/react/uimanager/JSTouchDispatcher.kt
// @ v0.88.1 (c30b7f86850d47422637a64c70508e7411dbd8ea), Kotlin to Java. Copyright (c) Meta Platforms, Inc. and
// affiliates. MIT, see third_party/react-native/LICENSE.
//
// Kept: handleTouchEvent's gesture tracking (the target is found on ACTION_DOWN and keeps every
// event until ACTION_UP or ACTION_CANCEL) and its log messages. The target is a native view's
// handle, found by GPUI's hit test, and each event goes to the view itself, offset into its
// coordinates (NativeViews.dispatchTouchEvent, Flutter's translateMotionEvent), not to JS. How it
// fits together: docs/native-view-android.md.
package io.github.xuckless.parallax;

import android.util.Log;
import android.view.MotionEvent;

/**
 * Forwards touch gestures that start on a native view the app asked to forward to (GPUI's hit
 * test says the view is topmost there, no GPUI element over it) to that Android View, while GPUI
 * gets the same events as before. ParallaxSurfaceView calls {@link #handleTouchEvent} for every
 * touch, before GPUI's own processor.
 */
final class NativeViewTouchDispatcher {
  private static final String TAG = "NativeViewTouch";

  /** GPUI's answer for a touch down: the forwarding native view's handle there, or -1. */
  interface HitTest {
    int nativeViewAt(float x, float y);
  }

  private final HitTest hitTest;

  private final NativeViews nativeViews;

  /** The native view the current gesture goes to, or -1: none (RN's targetTag). */
  private int targetTag = -1;

  NativeViewTouchDispatcher(HitTest hitTest, NativeViews nativeViews) {
    this.hitTest = hitTest;
    this.nativeViews = nativeViews;
  }

  void handleTouchEvent(MotionEvent ev) {
    int action = ev.getActionMasked();
    if (action == MotionEvent.ACTION_DOWN) {
      if (targetTag != -1) {
        Log.e(TAG, "Got DOWN touch before receiving UP or CANCEL from last gesture");
      }

      // First event for this gesture. We expect tag to be set to -1, and we use the hit test to
      // find the native view that will handle this gesture, if any.
      targetTag = hitTest.nativeViewAt(ev.getX(), ev.getY());
      dispatch(ev);
    } else if (targetTag == -1) {
      // The gesture started on GPUI's own content, or on a view that is not forwarded to.
      return;
    } else if (action == MotionEvent.ACTION_UP) {
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
  }

  private void dispatch(MotionEvent ev) {
    if (targetTag != -1) {
      nativeViews.dispatchTouchEvent(targetTag, ev);
    }
  }
}
