// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/io/flutter/plugin/platform/PlatformPlugin.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. Only the Activity, vibrateHapticFeedback and popSystemNavigator are
// kept. The constructor takes the Activity alone (no PlatformChannel: GPUI's haptics and its
// window closing arrive through ParallaxSurfaceView.nativeFrameworkCalls). popSystemNavigator
// keeps only its no-delegate, plain-Activity branch (finish). Dropped: PlatformChannel,
// PlatformPluginDelegate and mPlatformMessageHandler; destroy; system sounds, system chrome and
// overlays, orientation, app switcher description, clipboard, share, the
// OnBackPressedDispatcherOwner branch; androidx annotations; io.flutter.Log;
// io.flutter.Build.API_LEVELS (→ Build.VERSION_CODES). The bridge: docs/haptics.md,
// docs/android-multi-window.md. Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.app.Activity;
import android.os.Build;
import android.view.HapticFeedbackConstants;
import android.view.View;

/** Android implementation of the platform plugin. */
public class PlatformPlugin {
  private final Activity activity;

  public PlatformPlugin(Activity activity) {
    this.activity = activity;
  }

  /* package */ void vibrateHapticFeedback(PlatformChannel.HapticFeedbackType feedbackType) {
    View view = activity.getWindow().getDecorView();
    switch (feedbackType) {
      case STANDARD:
        view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS);
        break;
      case LIGHT_IMPACT:
        view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY);
        break;
      case MEDIUM_IMPACT:
        view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
        break;
      case HEAVY_IMPACT:
        view.performHapticFeedback(HapticFeedbackConstants.CONTEXT_CLICK);
        break;
      case SELECTION_CLICK:
        view.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK);
        break;
      case SUCCESS_NOTIFICATION:
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
          view.performHapticFeedback(HapticFeedbackConstants.CONFIRM);
        }
        break;
      case WARNING_NOTIFICATION:
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
          view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
        }
        break;
      case ERROR_NOTIFICATION:
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
          view.performHapticFeedback(HapticFeedbackConstants.REJECT);
        }
        break;
    }
  }

  /* package */ void popSystemNavigator() {
    activity.finish();
  }
}
