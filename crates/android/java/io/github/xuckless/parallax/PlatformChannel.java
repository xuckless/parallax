// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/io/flutter/embedding/engine/systemchannels/PlatformChannel.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. Only HapticFeedbackType is kept. The `flutter/platform`
// MethodChannel becomes JNI: Rust (GPUI) hands over the `HapticFeedback.vibrate` arguments as the
// return value of ParallaxSurfaceView.nativeHapticFeedbackCalls, and the view plays each as the
// channel's handler did. Dropped: the channel and its MethodCallHandler, PlatformMessageHandler
// and every other call (sounds, system chrome, orientation, clipboard, share, navigation) with
// their types; androidx annotations; io.flutter.Log. The bridge: docs/haptics.md. Ledger:
// third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

/**
 * System channel that receives requests for host platform behavior, e.g., haptic and sound effects,
 * system chrome configurations, and clipboard interaction.
 */
public class PlatformChannel {
  /** The types of haptic feedback that the Android OS can generate on behalf of an application. */
  public enum HapticFeedbackType {
    STANDARD(null),
    LIGHT_IMPACT("HapticFeedbackType.lightImpact"),
    MEDIUM_IMPACT("HapticFeedbackType.mediumImpact"),
    HEAVY_IMPACT("HapticFeedbackType.heavyImpact"),
    SELECTION_CLICK("HapticFeedbackType.selectionClick"),
    SUCCESS_NOTIFICATION("HapticFeedbackType.successNotification"),
    WARNING_NOTIFICATION("HapticFeedbackType.warningNotification"),
    ERROR_NOTIFICATION("HapticFeedbackType.errorNotification");

    static HapticFeedbackType fromValue(String encodedName) throws NoSuchFieldException {
      for (HapticFeedbackType feedbackType : HapticFeedbackType.values()) {
        if ((feedbackType.encodedName == null && encodedName == null)
            || (feedbackType.encodedName != null && feedbackType.encodedName.equals(encodedName))) {
          return feedbackType;
        }
      }
      throw new NoSuchFieldException("No such HapticFeedbackType: " + encodedName);
    }

    private final String encodedName;

    HapticFeedbackType(String encodedName) {
      this.encodedName = encodedName;
    }
  }
}
