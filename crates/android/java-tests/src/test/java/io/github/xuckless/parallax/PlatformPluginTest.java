// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/test/io/flutter/plugin/platform/PlatformPluginTest.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. 3 of 30 cases: the two vibrateHapticFeedback cases and
// performsDefaultBehaviorWhenNoDelegateProvided (the port keeps only the haptic part of
// PlatformPlugin and popSystemNavigator's no-delegate branch). Dropped: the other 27 (clipboard,
// system chrome, overlays, delegate and fragment navigation, share, sounds) with their setup
// (ClipboardManager, PlatformPluginDelegate); the PlatformChannel mock (the port's constructor
// takes the Activity alone); mPlatformMessageHandler (vibrateHapticFeedback and
// popSystemNavigator are called directly); io.flutter.Build.API_LEVELS (-> Build.VERSION_CODES).
// Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import static org.mockito.Mockito.clearInvocations;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import android.app.Activity;
import android.os.Build;
import android.view.HapticFeedbackConstants;
import android.view.View;
import android.view.Window;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
public class PlatformPluginTest {
  @Config(sdk = Build.VERSION_CODES.Q)
  @Test
  public void vibrateHapticFeedbackWhenApiLevelIsLessThan30() {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.STANDARD);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.LONG_PRESS);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.LIGHT_IMPACT);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.MEDIUM_IMPACT);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.HEAVY_IMPACT);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.CONTEXT_CLICK);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.SELECTION_CLICK);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.SUCCESS_NOTIFICATION);
    verify(fakeDecorView, never()).performHapticFeedback(HapticFeedbackConstants.CONFIRM);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.WARNING_NOTIFICATION);
    verify(fakeDecorView, never()).performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.ERROR_NOTIFICATION);
    verify(fakeDecorView, never()).performHapticFeedback(HapticFeedbackConstants.REJECT);
    clearInvocations(fakeDecorView);
  }

  @Config(minSdk = Build.VERSION_CODES.R)
  @Test
  public void vibrateHapticFeedbackWhenApiLevelIsHigherOrEquals30() {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.STANDARD);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.LONG_PRESS);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.LIGHT_IMPACT);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.MEDIUM_IMPACT);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.HEAVY_IMPACT);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.CONTEXT_CLICK);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.SELECTION_CLICK);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.SUCCESS_NOTIFICATION);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.CONFIRM);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.WARNING_NOTIFICATION);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP);
    clearInvocations(fakeDecorView);

    platformPlugin.vibrateHapticFeedback(PlatformChannel.HapticFeedbackType.ERROR_NOTIFICATION);
    verify(fakeDecorView).performHapticFeedback(HapticFeedbackConstants.REJECT);
    clearInvocations(fakeDecorView);
  }

  @Test
  public void performsDefaultBehaviorWhenNoDelegateProvided() {
    Activity mockActivity = mock(Activity.class);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);

    platformPlugin.popSystemNavigator();

    verify(mockActivity, times(1)).finish();
  }
}
