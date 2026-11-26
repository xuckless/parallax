// Parallax's own view (not ported). Surface and inset handling read from Flutter's FlutterSurfaceView and
// FlutterView as reference. How it fits together: docs/android.md.
package io.github.xuckless.parallax;

import android.content.Context;
import android.graphics.Insets;
import android.os.Build;
import android.view.Choreographer;
import android.view.DisplayCutout;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.WindowInsets;
import java.nio.ByteBuffer;

/**
 * The surface GPUI draws into. Forwards the surface lifecycle, vsync frames and system-bar insets
 * to Rust (parallax-android, crates/android/src/host.rs); everything runs on the main thread.
 *
 * <p>Touch: onTouchEvent → the ported AndroidTouchProcessor → nativeDispatchPointerDataPacket (see
 * docs/android-touch.md).
 */
final class ParallaxSurfaceView extends SurfaceView
    implements SurfaceHolder.Callback, Choreographer.FrameCallback {
  // Bound by JNI_OnLoad (RegisterNatives, the `natives` table in host.rs). Sizes and insets in pixels.
  private static native void nativeSurfaceCreated(Surface surface, float density, float refreshRate);

  private static native void nativeSurfaceChanged(int width, int height, float density);

  private static native void nativeSurfaceDestroyed();

  private static native void nativeFrame();

  private static native void nativeInsets(
      int safeLeft,
      int safeTop,
      int safeRight,
      int safeBottom,
      int imeLeft,
      int imeTop,
      int imeRight,
      int imeBottom);

  // Flutter's pointer data packet (AndroidTouchProcessor): `position` bytes of a direct buffer,
  // read before the call returns. Named after FlutterJNI.nativeDispatchPointerDataPacket.
  static native void nativeDispatchPointerDataPacket(ByteBuffer buffer, int position);

  // The app's locales, BCP-47 tags joined by commas, preference order (LocaleList.toLanguageTags()).
  // ParallaxActivity sends them before the first surface; the fonts read them when the app starts.
  static native void nativeLocales(String languageTags);

  // As FlutterView: no platform views, so motion events are not tracked.
  private final AndroidTouchProcessor touchProcessor =
      new AndroidTouchProcessor(/* trackMotionEvents= */ false);

  /** Whether a Choreographer callback is posted: between onResume and onPause. */
  private boolean framesRunning;

  ParallaxSurfaceView(Context context) {
    super(context);
    getHolder().addCallback(this);
  }

  void startFrames() {
    if (!framesRunning) {
      framesRunning = true;
      Choreographer.getInstance().postFrameCallback(this);
    }
  }

  void stopFrames() {
    framesRunning = false;
    Choreographer.getInstance().removeFrameCallback(this);
  }

  @Override
  public void doFrame(long frameTimeNanos) {
    nativeFrame();
    if (framesRunning) {
      Choreographer.getInstance().postFrameCallback(this);
    }
  }

  @Override
  public void surfaceCreated(SurfaceHolder holder) {
    nativeSurfaceCreated(holder.getSurface(), density(), getDisplay().getRefreshRate());
  }

  @Override
  public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
    nativeSurfaceChanged(width, height, density());
  }

  @Override
  public void surfaceDestroyed(SurfaceHolder holder) {
    // Rust releases its ANativeWindow before this returns, as Android requires.
    nativeSurfaceDestroyed();
  }

  @Override
  public boolean onTouchEvent(MotionEvent event) {
    // As FlutterView.onTouchEvent: deliver moves as they arrive, not batched to vsync.
    requestUnbufferedDispatch(event);
    return touchProcessor.onTouchEvent(event);
  }

  @Override
  public WindowInsets onApplyWindowInsets(WindowInsets insets) {
    if (Build.VERSION.SDK_INT >= 30) {
      Insets safe =
          insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
      Insets ime = insets.getInsets(WindowInsets.Type.ime());
      nativeInsets(
          safe.left, safe.top, safe.right, safe.bottom, ime.left, ime.top, ime.right, ime.bottom);
    } else {
      // API 28-29: system window insets cover the bars (and the keyboard, which is not split out
      // before API 30); the cutout is reported separately.
      DisplayCutout cutout = insets.getDisplayCutout();
      nativeInsets(
          Math.max(insets.getSystemWindowInsetLeft(), cutout == null ? 0 : cutout.getSafeInsetLeft()),
          Math.max(insets.getSystemWindowInsetTop(), cutout == null ? 0 : cutout.getSafeInsetTop()),
          Math.max(
              insets.getSystemWindowInsetRight(), cutout == null ? 0 : cutout.getSafeInsetRight()),
          Math.max(
              insets.getSystemWindowInsetBottom(), cutout == null ? 0 : cutout.getSafeInsetBottom()),
          0,
          0,
          0,
          0);
    }
    return super.onApplyWindowInsets(insets);
  }

  private float density() {
    return getResources().getDisplayMetrics().density;
  }
}
