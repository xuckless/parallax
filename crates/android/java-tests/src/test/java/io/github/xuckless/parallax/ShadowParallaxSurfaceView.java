// Parallax's own test seam (not ported). See docs/android-tests.md.
package io.github.xuckless.parallax;

import java.nio.ByteBuffer;
import org.robolectric.annotation.Implementation;
import org.robolectric.annotation.Implements;

/**
 * Replaces ParallaxSurfaceView's static natives on Robolectric, which cannot load the Rust .so.
 * A test opts in with {@code @Config(shadows = ShadowParallaxSurfaceView.class)}; Robolectric then
 * instruments ParallaxSurfaceView and routes the natives here. Add a method per native a test needs.
 */
@Implements(ParallaxSurfaceView.class)
public class ShadowParallaxSurfaceView {
  /**
   * Stands in for Flutter's FlutterRenderer, whose dispatchPointerDataPacket the ported tests
   * mock and verify. Same name and arguments, so the Flutter test bodies stay unchanged.
   */
  public interface Renderer {
    void dispatchPointerDataPacket(ByteBuffer buffer, int position);
  }

  // Static because the native it replaces is static: there is no instance to hang it on. Each
  // test sets it in @Before; Robolectric runs a class's tests one after another.
  static Renderer renderer;

  @Implementation
  protected static void nativeDispatchPointerDataPacket(
      int activity, ByteBuffer buffer, int position) {
    renderer.dispatchPointerDataPacket(buffer, position);
  }
}
