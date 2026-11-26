// Parallax's own Activity (not ported). Lifecycle and edge-to-edge handling read from Flutter's FlutterActivity
// and FlutterView as reference. How it fits together: docs/android.md.
package io.github.xuckless.parallax;

import android.app.Activity;
import android.content.pm.PackageManager;
import android.content.res.Configuration;
import android.graphics.Color;
import android.os.Build;
import android.os.Bundle;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;

/**
 * Hosts a Parallax app: loads the app's native library, then shows one {@link ParallaxSurfaceView}
 * drawn edge to edge. The library is named by {@code <meta-data android:name="parallax.lib_name">}
 * under {@code <application>} in the manifest.
 */
public class ParallaxActivity extends Activity {
  private static final String LIB_NAME_KEY = "parallax.lib_name";

  private ParallaxSurfaceView view;

  @Override
  protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    // The app's JNI_OnLoad (android_main!) runs here, on the main thread, and binds the natives.
    System.loadLibrary(libName());
    // Per-app locales (API 33+) are in the Activity's configuration, not the system property.
    ParallaxSurfaceView.nativeLocales(getResources().getConfiguration().getLocales().toLanguageTags());
    requestWindowFeature(Window.FEATURE_NO_TITLE);
    drawEdgeToEdge(getWindow());
    view = new ParallaxSurfaceView(this);
    setContentView(view);
  }

  @Override
  protected void onResume() {
    super.onResume();
    view.startFrames();
  }

  @Override
  protected void onPause() {
    view.stopFrames();
    super.onPause();
  }

  // The manifest declares configChanges, so rotation and density changes arrive here instead of
  // recreating the Activity. The surface reports the new size; insets are re-applied.
  @Override
  public void onConfigurationChanged(Configuration newConfig) {
    super.onConfigurationChanged(newConfig);
    view.requestApplyInsets();
  }

  private String libName() {
    try {
      Bundle metaData =
          getPackageManager()
              .getApplicationInfo(getPackageName(), PackageManager.GET_META_DATA)
              .metaData;
      String name = metaData == null ? null : metaData.getString(LIB_NAME_KEY);
      if (name == null) {
        throw new IllegalStateException(
            "AndroidManifest.xml <application> needs <meta-data android:name=\""
                + LIB_NAME_KEY
                + "\" android:value=\"<native library name>\"/>");
      }
      return name;
    } catch (PackageManager.NameNotFoundException e) {
      throw new IllegalStateException("cannot read this app's own manifest", e);
    }
  }

  // targetSdk 35 forces edge to edge on Android 15+; do the same on API 28-34 so every version draws
  // under the system bars and the view reports them as insets.
  private static void drawEdgeToEdge(Window window) {
    if (Build.VERSION.SDK_INT >= 30) {
      window.setDecorFitsSystemWindows(false);
    } else {
      window
          .getDecorView()
          .setSystemUiVisibility(
              View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                  | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                  | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
    }
    window.getAttributes().layoutInDisplayCutoutMode =
        WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
    window.setStatusBarColor(Color.TRANSPARENT);
    window.setNavigationBarColor(Color.TRANSPARENT);
  }
}
