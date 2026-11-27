// Ported from facebook/react-native
// packages/react-native/ReactAndroid/src/main/java/com/facebook/react/modules/accessibilityinfo/AccessibilityInfoModule.kt
// @ v0.88.1 (c30b7f86850d47422637a64c70508e7411dbd8ea), Kotlin to Java. Copyright (c) Meta Platforms, Inc. and
// affiliates. MIT, see third_party/react-native/LICENSE.
//
// Kept: the reduce-motion parts (animationScaleObserver, updateAndSendReduceMotionChangeEvent,
// onHostResume, onHostPause, initialize). How it fits together: docs/os-settings.md.
package io.github.xuckless.parallax;

import android.content.ContentResolver;
import android.database.ContentObserver;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;

/**
 * Module that monitors and provides information about the state of the system's animation scales
 * ("Remove animations" and the developer options' scales), which GPUI reads as reduce motion.
 */
final class AccessibilityInfoModule {
  // Listener that is notified when the global TRANSITION_ANIMATION_SCALE.
  private final ContentObserver animationScaleObserver =
      new ContentObserver(new Handler(Looper.getMainLooper())) {
        @Override
        public void onChange(boolean selfChange) {
          this.onChange(selfChange, null);
        }

        @Override
        public void onChange(boolean selfChange, Uri uri) {
          updateAndSendReduceMotionChangeEvent();
        }
      };

  private final ContentResolver contentResolver;

  AccessibilityInfoModule(ContentResolver contentResolver) {
    this.contentResolver = contentResolver;
  }

  // isReduceMotionEnabledValue's parsing is in Rust (accessibility.rs), table-tested on the host;
  // Java sends the raw strings, and Rust sends GPUI only a change (the reduceMotionEnabled check).
  // Also sent: ANIMATOR_DURATION_SCALE, which Android's own animators obey.
  private void updateAndSendReduceMotionChangeEvent() {
    ParallaxSurfaceView.nativeAccessibilitySettings(
        Settings.Global.getString(contentResolver, Settings.Global.TRANSITION_ANIMATION_SCALE),
        Settings.Global.getString(contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE));
  }

  void onHostResume() {
    Uri transitionUri = Settings.Global.getUriFor(Settings.Global.TRANSITION_ANIMATION_SCALE);
    contentResolver.registerContentObserver(transitionUri, false, animationScaleObserver);
    Uri animatorUri = Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE);
    contentResolver.registerContentObserver(animatorUri, false, animationScaleObserver);
    updateAndSendReduceMotionChangeEvent();
  }

  void onHostPause() {
    contentResolver.unregisterContentObserver(animationScaleObserver);
  }

  void initialize() {
    updateAndSendReduceMotionChangeEvent();
  }
}
