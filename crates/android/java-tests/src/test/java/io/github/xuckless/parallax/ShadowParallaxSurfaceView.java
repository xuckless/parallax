// Parallax's own test seam (not ported). See docs/android-tests.md.
package io.github.xuckless.parallax;

import java.nio.ByteBuffer;
import org.robolectric.annotation.Implementation;
import org.robolectric.annotation.Implements;
import org.robolectric.shadows.ShadowSurfaceView;

/**
 * Replaces ParallaxSurfaceView's static natives on Robolectric, which cannot load the Rust .so.
 * A test opts in with {@code @Config(shadows = ShadowParallaxSurfaceView.class)}; Robolectric then
 * instruments ParallaxSurfaceView and routes the natives here. Add a method per native a test needs.
 * It extends ShadowSurfaceView so a test can also construct the view (TextInputPluginTest).
 */
@Implements(ParallaxSurfaceView.class)
public class ShadowParallaxSurfaceView extends ShadowSurfaceView {
  /**
   * Stands in for Flutter's FlutterRenderer, whose dispatchPointerDataPacket and
   * setViewportMetrics the ported tests mock and verify. Same names and arguments, so the Flutter
   * test bodies stay unchanged.
   */
  public interface Renderer {
    void dispatchPointerDataPacket(ByteBuffer buffer, int position);

    void setViewportMetrics(ViewportMetrics viewportMetrics);
  }

  /**
   * The insets nativeInsets carries, under FlutterRenderer.ViewportMetrics' names: the safe area
   * (system bars and cutout) is the view padding, the keyboard the view inset; the density and
   * touch slop sent with them are devicePixelRatio and physicalTouchSlop (-1 until set, as Flutter's).
   */
  public static class ViewportMetrics {
    public float devicePixelRatio = 1.0f;
    public int physicalTouchSlop = -1;
    public int viewPaddingLeft;
    public int viewPaddingTop;
    public int viewPaddingRight;
    public int viewPaddingBottom;
    public int viewInsetLeft;
    public int viewInsetTop;
    public int viewInsetRight;
    public int viewInsetBottom;
  }

  // Static because the native it replaces is static: there is no instance to hang it on. Each
  // test sets it in @Before; Robolectric runs a class's tests one after another.
  static Renderer renderer;

  @Implementation
  protected static void nativeDispatchPointerDataPacket(
      int activity, ByteBuffer buffer, int position) {
    renderer.dispatchPointerDataPacket(buffer, position);
  }

  @Implementation
  protected static void nativeInsets(
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
      int imeBottom) {
    final ViewportMetrics metrics = new ViewportMetrics();
    metrics.devicePixelRatio = density;
    metrics.physicalTouchSlop = touchSlop;
    metrics.viewPaddingLeft = safeLeft;
    metrics.viewPaddingTop = safeTop;
    metrics.viewPaddingRight = safeRight;
    metrics.viewPaddingBottom = safeBottom;
    metrics.viewInsetLeft = imeLeft;
    metrics.viewInsetTop = imeTop;
    metrics.viewInsetRight = imeRight;
    metrics.viewInsetBottom = imeBottom;
    renderer.setViewportMetrics(metrics);
  }

  /**
   * Stands in for the "flutter/keydata" BinaryMessenger the ported KeyboardManagerTest mocked.
   * Returns the reply (whether the key was handled), as the native does.
   */
  public interface KeyDataSink {
    boolean dispatchKeyData(ByteBuffer packet, boolean canceled);
  }

  // Static for the same reason as `renderer`; KeyboardManagerTest's KeyboardTester sets it.
  static KeyDataSink keyData;

  @Implementation
  protected static boolean nativeDispatchKeyData(
      int activity, ByteBuffer packet, boolean canceled, int metaState) {
    return keyData.dispatchKeyData(packet, canceled);
  }

  /**
   * Stands in for the "flutter/textinput" messages TextInputChannel sent, which the ported tests
   * verified on the DartExecutor. boundsForRange is GPUI's layout; a mock answers false (none).
   */
  public interface TextInput {
    void updateEditingState(
        String text, int selectionStart, int selectionEnd, int composingStart, int composingEnd);

    void performAction(String action);

    boolean boundsForRange(int start, int end, float[] ltrb);
  }

  // Static for the same reason as `renderer`; TextInputPluginTest sets it in @Before.
  static TextInput textInput;

  @Implementation
  protected static void nativeUpdateEditingState(
      int activity,
      String text,
      int selectionStart,
      int selectionEnd,
      int composingStart,
      int composingEnd) {
    textInput.updateEditingState(text, selectionStart, selectionEnd, composingStart, composingEnd);
  }

  @Implementation
  protected static void nativePerformAction(int activity, String action) {
    textInput.performAction(action);
  }

  @Implementation
  protected static boolean nativeBoundsForRange(int activity, int start, int end, float[] ltrb) {
    return textInput.boundsForRange(start, end, ltrb);
  }
}
