// Parallax's own glue (not ported). How it fits together: docs/native-view-android.md.
package io.github.xuckless.parallax;

import android.view.View;

/**
 * A View the app asks to have drawn in a surface of its own (SurfaceControlViewHost, API 34+), so
 * it moves in step with GPUI's frame: for views without text input, such as video or a map. The
 * app passes this to NativeView::new instead of the View (parallax_android::in_surface); below API
 * 34 the View is hosted as an ordinary one.
 */
final class InSurface {
  final View view;

  InSurface(View view) {
    this.view = view;
  }
}
