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

  // Event codes for ParallaxSurfaceView.nativeActivityEvent; must match ActivityEvent::from_java in
  // crates/android/src/lifecycle.rs. To add a callback: a constant here, a row there.
  private static final int EVENT_START = 0;
  private static final int EVENT_RESUME = 1;
  private static final int EVENT_PAUSE = 2;
  private static final int EVENT_STOP = 3;
  private static final int EVENT_DESTROY = 4;
  private static final int EVENT_FOCUS = 5;
  private static final int EVENT_TRIM_MEMORY = 6;

  private ParallaxSurfaceView view;

  /** This instance's id from nativeActivityCreated: Rust ignores events from a replaced Activity. */
  private int activity;

  @Override
  protected void onCreate(Bundle savedInstanceState) {
    super.onCreate(savedInstanceState);
    // The app's JNI_OnLoad (android_main!) runs here, on the main thread, and binds the natives.
    System.loadLibrary(libName());
    activity = ParallaxSurfaceView.nativeActivityCreated();
    // Per-app locales (API 33+) are in the Activity's configuration, not the system property.
    ParallaxSurfaceView.nativeLocales(getResources().getConfiguration().getLocales().toLanguageTags());
    requestWindowFeature(Window.FEATURE_NO_TITLE);
    drawEdgeToEdge(getWindow());
    view = new ParallaxSurfaceView(this, activity);
    setContentView(view);
  }

  @Override
  protected void onStart() {
    super.onStart();
    ParallaxSurfaceView.nativeActivityEvent(activity, EVENT_START, 0);
  }

  @Override
  protected void onResume() {
    super.onResume();
    ParallaxSurfaceView.nativeActivityEvent(activity, EVENT_RESUME, 0);
    view.startFrames();
  }

  @Override
  protected void onPause() {
    view.stopFrames();
    ParallaxSurfaceView.nativeActivityEvent(activity, EVENT_PAUSE, 0);
    super.onPause();
  }

  @Override
  protected void onStop() {
    ParallaxSurfaceView.nativeActivityEvent(activity, EVENT_STOP, 0);
    super.onStop();
  }

  // Not quit: the process and GPUI may outlive this Activity, and a new one reattaches (docs/android-lifecycle.md).
  @Override
  protected void onDestroy() {
    ParallaxSurfaceView.nativeActivityEvent(activity, EVENT_DESTROY, 0);
    super.onDestroy();
  }

  @Override
  public void onWindowFocusChanged(boolean hasFocus) {
    super.onWindowFocusChanged(hasFocus);
    ParallaxSurfaceView.nativeActivityEvent(activity, EVENT_FOCUS, hasFocus ? 1 : 0);
  }

  @Override
  public void onTrimMemory(int level) {
    super.onTrimMemory(level);
    ParallaxSurfaceView.nativeActivityEvent(activity, EVENT_TRIM_MEMORY, level);
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
