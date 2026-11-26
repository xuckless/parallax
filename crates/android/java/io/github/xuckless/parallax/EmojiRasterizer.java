// Parallax's own (not ported). How it fits together: docs/android-emoji.md.
package io.github.xuckless.parallax;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.Typeface;

/**
 * Draws colour emoji for the Rust text system (parallax-android, crates/android/src/text_system.rs).
 * cosmic-text shapes and lays out the text, but swash cannot paint COLRv1 (the only emoji font on
 * newer images), so Android's own renderer draws each emoji cluster from the same font file. Rust
 * calls these over JNI on the main thread; nothing here keeps state.
 */
final class EmojiRasterizer {
  private EmojiRasterizer() {}

  /** Face {@code index} of the font file at {@code path}, or null if Android cannot load it. */
  static Typeface typeface(String path, int index) {
    return new Typeface.Builder(path).setTtcIndex(index).build();
  }

  /**
   * {left, top, right, bottom}: the pixels {@code text} covers when drawn at the origin, y down
   * from the baseline.
   */
  static int[] bounds(Typeface typeface, String text, float size) {
    Rect bounds = new Rect();
    paint(typeface, size).getTextBounds(text, 0, text.length(), bounds);
    return new int[] {bounds.left, bounds.top, bounds.right, bounds.bottom};
  }

  /**
   * {@code text} drawn at the origin, the {@code width} x {@code height} box whose top-left corner
   * is ({@code left}, {@code top}) from it: unpremultiplied ARGB, rows top to bottom.
   */
  static int[] draw(
      Typeface typeface, String text, float size, int left, int top, int width, int height) {
    Bitmap bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
    new Canvas(bitmap).drawText(text, -left, -top, paint(typeface, size));
    int[] pixels = new int[width * height];
    bitmap.getPixels(pixels, 0, width, 0, 0, width, height);
    bitmap.recycle();
    return pixels;
  }

  private static Paint paint(Typeface typeface, float size) {
    Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    paint.setTypeface(typeface);
    paint.setTextSize(size);
    return paint;
  }
}
