// Parallax's own view (not ported). Surface and inset handling read from Flutter's FlutterSurfaceView and
// FlutterView as reference. How it fits together: docs/android.md.
package io.github.xuckless.parallax;

import android.content.Context;
import android.graphics.Insets;
import android.os.Build;
import android.view.Choreographer;
import android.view.DisplayCutout;
import android.view.KeyEvent;
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
 * docs/android-touch.md). Keys: dispatchKeyEvent → the ported KeyboardManager →
 * nativeDispatchKeyData (see docs/android-keys.md).
 */
final class ParallaxSurfaceView extends SurfaceView
    implements SurfaceHolder.Callback, Choreographer.FrameCallback, KeyboardManager.ViewDelegate {
  // Bound by JNI_OnLoad (RegisterNatives, the `natives` table in host.rs). Sizes and insets in pixels.
  // The `activity` argument is the id nativeActivityCreated gave this view's Activity: Rust drops
  // events from a view whose Activity does not own the window (it is covered, or destroyed and its
  // callbacks arrive late).
  // `languageTags`: the app's locales, BCP-47 tags joined by commas, preference order
  // (LocaleList.toLanguageTags()); the fonts take them when the first surface starts the app.
  private static native void nativeSurfaceCreated(
      int activity, Surface surface, float density, float refreshRate, String languageTags);

  private static native void nativeSurfaceChanged(
      int activity, int width, int height, float density);

  private static native void nativeSurfaceDestroyed(int activity);

  private static native void nativeFrame(int activity);

  private static native void nativeInsets(
      int activity,
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
  static native void nativeDispatchPointerDataPacket(
      int activity, ByteBuffer buffer, int position);

  // ParallaxActivity.onCreate: a new Activity owns the window; returns its id for every later event.
  // Declared here so one class holds every native JNI_OnLoad binds.
  static native int nativeActivityCreated();

  // ParallaxActivity's lifecycle, focus and memory callbacks: its id, one of its EVENT_* codes and a
  // value (focus flag or trim level).
  static native void nativeActivityEvent(int activity, int event, int value);

  // Flutter's key data packet (KeyData.toBytes, the whole direct buffer), read before the call
  // returns, and the event's isCanceled() (key data has no field for it: a cancelled back release
  // must not go back). Returns whether GPUI handled it: the reply Flutter's framework sends
  // asynchronously.
  static native boolean nativeDispatchKeyData(int activity, ByteBuffer packet, boolean canceled);

  // Both tag what they send with `activity`, so they are built once it is set.
  private final AndroidTouchProcessor touchProcessor;

  private final KeyboardManager keyboardManager;

  /** Whether a Choreographer callback is posted: between onResume and onPause. */
  private boolean framesRunning;

  /** The id nativeActivityCreated gave the Activity showing this view; tags every event it sends. */
  private final int activity;

  ParallaxSurfaceView(Context context, int activity) {
    super(context);
    this.activity = activity;
    // As FlutterView: no platform views, so motion events are not tracked.
    touchProcessor = new AndroidTouchProcessor(activity, /* trackMotionEvents= */ false);
    keyboardManager = new KeyboardManager(this);
    getHolder().addCallback(this);
    // As FlutterView: focusable, so key events (and later the IME) reach this view.
    setFocusable(true);
    setFocusableInTouchMode(true);
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
    nativeFrame(activity);
    if (framesRunning) {
      Choreographer.getInstance().postFrameCallback(this);
    }
  }

  @Override
  public void surfaceCreated(SurfaceHolder holder) {
    // Per-app locales (API 33+) are in the Activity's configuration, not the system property.
    nativeSurfaceCreated(
        activity,
        holder.getSurface(),
        density(),
        getDisplay().getRefreshRate(),
        getResources().getConfiguration().getLocales().toLanguageTags());
  }

  @Override
  public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
    nativeSurfaceChanged(activity, width, height, density());
  }

  @Override
  public void surfaceDestroyed(SurfaceHolder holder) {
    // Rust releases its ANativeWindow before this returns, as Android requires.
    nativeSurfaceDestroyed(activity);
  }

  @Override
  public boolean onTouchEvent(MotionEvent event) {
    // As FlutterView.onTouchEvent: deliver moves as they arrive, not batched to vsync.
    requestUnbufferedDispatch(event);
    return touchProcessor.onTouchEvent(event);
  }

  // The app's only view takes focus as soon as it is in the window: without it, key events stop at
  // the DecorView. (FlutterView relies on Android's initial focus, which this view did not get on
  // the API 35 emulator.)
  @Override
  protected void onAttachedToWindow() {
    super.onAttachedToWindow();
    requestFocus();
  }

  // As FlutterView.dispatchKeyEvent: KeyboardManager answers every event it has not redispatched
  // itself; a redispatched one goes on to Android (back → the Activity, volume → the system).
  @Override
  public boolean dispatchKeyEvent(KeyEvent event) {
    if (event.getAction() == KeyEvent.ACTION_DOWN && event.getRepeatCount() == 0) {
      // Tell Android to start tracking this event.
      getKeyDispatcherState().startTracking(event, this);
    } else if (event.getAction() == KeyEvent.ACTION_UP) {
      // Stop tracking the event.
      getKeyDispatcherState().handleUpEvent(event);
    }
    return keyboardManager.handleEvent(event) || super.dispatchKeyEvent(event);
  }

  // The text-input seam: keys GPUI did not handle are offered here before Android gets them.
  // FlutterView calls TextInputPlugin.handleKeyEvent; the text-input port fills this in.
  @Override
  public boolean onTextInputKeyEvent(KeyEvent keyEvent) {
    return false;
  }

  @Override
  public int getActivityId() {
    return activity;
  }

  @Override
  public void redispatch(KeyEvent keyEvent) {
    getRootView().dispatchKeyEvent(keyEvent);
  }

  @Override
  public WindowInsets onApplyWindowInsets(WindowInsets insets) {
    if (Build.VERSION.SDK_INT >= 30) {
      Insets safe =
          insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
      Insets ime = insets.getInsets(WindowInsets.Type.ime());
      nativeInsets(
          activity,
          safe.left, safe.top, safe.right, safe.bottom, ime.left, ime.top, ime.right, ime.bottom);
    } else {
      // API 28-29: system window insets cover the bars (and the keyboard, which is not split out
      // before API 30); the cutout is reported separately.
      DisplayCutout cutout = insets.getDisplayCutout();
      nativeInsets(
          activity,
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
