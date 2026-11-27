// Parallax's own view. Surface and inset handling read from Flutter's FlutterSurfaceView and
// FlutterView as reference. One line is ported: touchSlop() from
// engine/src/flutter/shell/platform/android/io/flutter/embedding/android/FlutterView.java @ 3.50.1
// (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors. BSD-3-Clause,
// see third_party/flutter/LICENSE. How it fits together: docs/android.md.
package io.github.xuckless.parallax;

import android.app.Activity;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Insets;
import android.os.Build;
import android.util.Log;
import android.util.SparseArray;
import android.view.Choreographer;
import android.view.DisplayCutout;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.ViewStructure;
import android.view.WindowInsets;
import android.view.autofill.AutofillValue;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import java.nio.ByteBuffer;

/**
 * The surface GPUI draws into. Forwards the surface lifecycle, vsync frames and system-bar insets
 * to Rust (parallax-android, crates/android/src/host.rs); everything runs on the main thread.
 *
 * <p>Touch and mouse: onTouchEvent and onGenericMotionEvent → the ported AndroidTouchProcessor →
 * nativeDispatchPointerDataPacket (see docs/android-touch.md, docs/android-mouse.md). Keys:
 * dispatchKeyEvent → the ported KeyboardManager → nativeDispatchKeyData (see docs/android-keys.md).
 * Text input: the ported TextInputPlugin, synced with GPUI over the natives below (see
 * docs/android-text-input.md).
 */
final class ParallaxSurfaceView extends SurfaceView
    implements SurfaceHolder.Callback, Choreographer.FrameCallback, KeyboardManager.ViewDelegate {
  // Bound by JNI_OnLoad (RegisterNatives, the `natives` table in host.rs). Sizes and insets in pixels.
  // Every call that sends the density also sends `touchSlop`, the device's touch slop in pixels, as
  // FlutterView.sendViewportMetricsToFlutter sends devicePixelRatio and physicalTouchSlop together.
  // The `activity` argument is the id nativeActivityCreated gave this view's Activity: Rust drops
  // events from a view whose Activity does not own the window (it is covered, or destroyed and its
  // callbacks arrive late). surfaceDestroyed is the exception: it always reaches the surface its
  // own view created.
  // `languageTags`: the app's locales, BCP-47 tags joined by commas, preference order
  // (LocaleList.toLanguageTags()); the fonts take them when the first surface starts the app.
  private static native void nativeSurfaceCreated(
      int activity,
      Surface surface,
      float density,
      int touchSlop,
      float refreshRate,
      String languageTags);

  // The app's locales after a configuration change (system or per-app language), as above; the
  // per-language font order follows them. Named after Flutter's LocalizationPlugin.sendLocalesToFlutter.
  private static native void nativeLocalesChanged(int activity, String languageTags);

  private static native void nativeSurfaceChanged(
      int activity, int width, int height, float density, int touchSlop);

  private static native void nativeSurfaceDestroyed(int activity);

  // Every vsync: GPUI draws if anything changed.
  private static native void nativeFrame(int activity);

  // GPUI's pending text-input calls (Flutter's `flutter/textinput` calls as a JSON array, for
  // TextInputChannel.handleFrameworkCalls), or null when there are none.
  private static native String nativeTextInputCalls(int activity);

  // GPUI's pending haptics, oldest first: Flutter's `HapticFeedback.vibrate` arguments (null for
  // HapticFeedback.vibrate()), for PlatformChannel.HapticFeedbackType.fromValue; null when there
  // are none.
  private static native String[] nativeHapticFeedbackCalls(int activity);

  // Also after a density change: ParallaxActivity.onConfigurationChanged re-applies the insets, and
  // the surface keeps its pixel size, so no surfaceChanged follows.
  private static native void nativeInsets(
      int activity,
      float density,
      int touchSlop,
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

  // Text input (TextInputChannel): the editing state after an IME edit, in UTF-16 offsets, -1 for
  // none (Flutter's TextInputClient.updateEditingState); GPUI applies it synchronously.
  static native void nativeUpdateEditingState(
      int activity,
      String text,
      int selectionStart,
      int selectionEnd,
      int composingStart,
      int composingEnd);

  // The IME's editor action, by Flutter's name ("TextInputAction.done", ...).
  static native void nativePerformAction(int activity, String action);

  // GPUI's bounds of the UTF-16 range [start, end) in view pixels into `ltrb`; false if none.
  static native boolean nativeBoundsForRange(int activity, int start, int end, float[] ltrb);

  // Both tag what they send with `activity`, so they are built once it is set.
  private final AndroidTouchProcessor touchProcessor;

  private final KeyboardManager keyboardManager;

  private final TextInputChannel textInputChannel;

  private final TextInputPlugin textInputPlugin;

  /** Whether a Choreographer callback is posted: between onResume and onPause. */
  private boolean framesRunning;

  /** The id nativeActivityCreated gave the Activity showing this view; tags every event it sends. */
  private final int activity;

  ParallaxSurfaceView(Context context, int activity) {
    super(context);
    this.activity = activity;
    // As FlutterView: no platform views, so motion events are not tracked.
    touchProcessor = new AndroidTouchProcessor(activity, /* trackMotionEvents= */ false);
    // A key GPUI handles may edit the field: the IME's mirror takes GPUI's state before the IME's
    // next call (Gboard sends DEL as a key event, then may commit text before the next vsync). Both
    // hardware keys and the IME's sendKeyEvent come through here.
    keyboardManager =
        new KeyboardManager(this) {
          @Override
          public boolean handleEvent(KeyEvent keyEvent) {
            boolean handled = super.handleEvent(keyEvent);
            runTextInputCalls();
            return handled;
          }
        };
    textInputChannel = new TextInputChannel(activity);
    textInputPlugin = new TextInputPlugin(this, textInputChannel);
    getHolder().addCallback(this);
    // No default focus highlight (API 26+): out of touch mode Android draws a translucent grey over
    // the focused view, which here is GPUI's whole frame (white read 224 of 255). GPUI draws its own
    // focus.
    setDefaultFocusHighlightEnabled(false);
    // As FlutterView: focusable, so key events (and later the IME) reach this view.
    setFocusable(true);
    setFocusableInTouchMode(true);
    // As FlutterView: the autofill service asks this view for its fields (TextInputPlugin's virtual
    // structure).
    setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_YES);
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
    runTextInputCalls();
    runHapticFeedbackCalls();
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
        touchSlop(),
        getDisplay().getRefreshRate(),
        getResources().getConfiguration().getLocales().toLanguageTags());
  }

  // As FlutterView.onConfigurationChanged: send the locales on every configuration change (the
  // manifest's configChanges includes locale|layoutDirection, so the Activity is not recreated);
  // Rust ignores an unchanged list.
  @Override
  protected void onConfigurationChanged(Configuration newConfig) {
    super.onConfigurationChanged(newConfig);
    nativeLocalesChanged(activity, newConfig.getLocales().toLanguageTags());
  }

  @Override
  public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
    nativeSurfaceChanged(activity, width, height, density(), touchSlop());
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

  // As FlutterView.onGenericMotionEvent: mouse hover moves and wheel turns (the processor takes
  // pointer-class hover moves and scrolls, and leaves the rest to Android). Hover events reach here
  // because this view's onHoverEvent does not take them (it is not clickable), as in FlutterView
  // without accessibility.
  @Override
  public boolean onGenericMotionEvent(MotionEvent event) {
    boolean handled = touchProcessor.onGenericMotionEvent(event, getContext());
    return handled || super.onGenericMotionEvent(event);
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

  // As FlutterView: keys GPUI did not handle go to the IME (TextInputPlugin) before Android.
  @Override
  public boolean onTextInputKeyEvent(KeyEvent keyEvent) {
    return textInputPlugin.handleKeyEvent(keyEvent);
  }

  // As FlutterView.onCreateInputConnection.
  @Override
  public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
    return textInputPlugin.createInputConnection(this, keyboardManager, outAttrs);
  }

  // As FlutterView: the focused field, if it has an autofill hint, as a virtual child.
  @Override
  public void onProvideAutofillVirtualStructure(ViewStructure structure, int flags) {
    super.onProvideAutofillVirtualStructure(structure, flags);
    textInputPlugin.onProvideAutofillVirtualStructure(structure, flags);
  }

  // As FlutterView: values the user picked from the autofill service. The focused field's value
  // goes to GPUI as an IME edit would (TextInputPlugin.autofill → nativeUpdateEditingState).
  @Override
  public void autofill(SparseArray<AutofillValue> values) {
    textInputPlugin.autofill(values);
  }

  // GPUI's pending text-input calls, run outside GPUI's draw and key dispatch, so the IME's
  // synchronous callbacks (restartInput → onCreateInputConnection) reach Rust with GPUI free.
  private void runTextInputCalls() {
    String calls = nativeTextInputCalls(activity);
    if (calls != null) {
      textInputChannel.handleFrameworkCalls(calls);
    }
  }

  // GPUI's pending haptics, played as PlatformChannel's handler plays `HapticFeedback.vibrate`
  // (an unknown type is logged, as its result.error). Every vsync: a long press GPUI recognizes on a
  // timer plays within a frame. The view's context is the ParallaxActivity that made it.
  private void runHapticFeedbackCalls() {
    String[] calls = nativeHapticFeedbackCalls(activity);
    if (calls == null) {
      return;
    }
    PlatformPlugin platformPlugin = new PlatformPlugin((Activity) getContext());
    for (String feedbackType : calls) {
      try {
        platformPlugin.vibrateHapticFeedback(
            PlatformChannel.HapticFeedbackType.fromValue(feedbackType));
      } catch (NoSuchFieldException exception) {
        Log.e("ParallaxSurfaceView", exception.getMessage());
      }
    }
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
          density(),
          touchSlop(),
          safe.left, safe.top, safe.right, safe.bottom, ime.left, ime.top, ime.right, ime.bottom);
    } else {
      // API 28-29: system window insets cover the bars (and the keyboard, which is not split out
      // before API 30); the cutout is reported separately.
      DisplayCutout cutout = insets.getDisplayCutout();
      nativeInsets(
          activity,
          density(),
          touchSlop(),
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

  // As FlutterView.sendViewportMetricsToFlutter's physicalTouchSlop. ViewConfiguration.get caches one
  // configuration per density, so this follows a density change.
  private int touchSlop() {
    return ViewConfiguration.get(getContext()).getScaledTouchSlop();
  }
}
