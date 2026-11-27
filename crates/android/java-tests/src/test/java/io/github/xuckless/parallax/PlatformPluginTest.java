// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/test/io/flutter/plugin/platform/PlatformPluginTest.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. 9 of 30 cases: the six getClipboardData cases, the two
// vibrateHapticFeedback cases and performsDefaultBehaviorWhenNoDelegateProvided (the port keeps
// only the clipboard reads and writes, the haptic part of PlatformPlugin and popSystemNavigator's
// no-delegate branch). Dropped: the other 21 (platformPlugin_hasStrings: the port has no
// clipboardHasStrings; system chrome, overlays, delegate and fragment navigation, share, sounds) with their
// setup (PlatformPluginDelegate); the PlatformChannel mock (the port's constructor takes the
// Activity alone); mPlatformMessageHandler (getClipboardData, vibrateHapticFeedback and
// popSystemNavigator are called directly); io.flutter.Build.API_LEVELS (-> Build.VERSION_CODES).
// Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.mockito.Mockito.any;
import static org.mockito.Mockito.clearInvocations;
import static org.mockito.Mockito.doThrow;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.spy;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.ContentResolver;
import android.content.Context;
import android.content.res.AssetFileDescriptor;
import android.net.Uri;
import android.os.Build;
import android.view.HapticFeedbackConstants;
import android.view.View;
import android.view.Window;
import androidx.test.core.app.ApplicationProvider;
import io.github.xuckless.parallax.PlatformChannel.ClipboardContentFormat;
import java.io.IOException;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.robolectric.RobolectricTestRunner;
import org.robolectric.annotation.Config;

@RunWith(RobolectricTestRunner.class)
public class PlatformPluginTest {
  private final Context ctx = ApplicationProvider.getApplicationContext();

  private ClipboardManager clipboardManager;
  private ClipboardContentFormat clipboardFormat;

  public void setUpForTextClipboardTests(Activity mockActivity) {
    clipboardManager = spy(ctx.getSystemService(ClipboardManager.class));
    when(mockActivity.getSystemService(Context.CLIPBOARD_SERVICE)).thenReturn(clipboardManager);
    clipboardFormat = ClipboardContentFormat.PLAIN_TEXT;
  }

  @Test
  public void platformPlugin_getClipboardDataIsNonNullWhenPlainTextCopied() throws IOException {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);
    setUpForTextClipboardTests(mockActivity);

    // Primary clip contains non-text media.
    ClipData clip = ClipData.newPlainText("label", "Text");
    assertNull(platformPlugin.getClipboardData(clipboardFormat));
    clipboardManager.setPrimaryClip(clip);
    assertNotNull(platformPlugin.getClipboardData(clipboardFormat));
  }

  @Test
  public void platformPlugin_getClipboardDataIsNonNullWhenIOExceptionThrown() throws IOException {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);
    setUpForTextClipboardTests(mockActivity);
    ContentResolver contentResolver = mock(ContentResolver.class);
    ClipData.Item mockItem = mock(ClipData.Item.class);
    Uri mockUri = mock(Uri.class);
    AssetFileDescriptor mockAssetFileDescriptor = mock(AssetFileDescriptor.class);

    when(mockActivity.getContentResolver()).thenReturn(contentResolver);
    when(mockUri.getScheme()).thenReturn("content");
    when(mockItem.getText()).thenReturn(null);
    when(mockItem.getUri()).thenReturn(mockUri);
    when(contentResolver.openTypedAssetFileDescriptor(any(Uri.class), any(), any()))
        .thenReturn(mockAssetFileDescriptor);
    when(mockItem.coerceToText(mockActivity)).thenReturn("something non-null");
    doThrow(new IOException()).when(mockAssetFileDescriptor).close();

    ClipData clip = new ClipData("label", new String[0], mockItem);

    assertNull(platformPlugin.getClipboardData(clipboardFormat));
    clipboardManager.setPrimaryClip(clip);
    assertNotNull(platformPlugin.getClipboardData(clipboardFormat));
  }

  @Test
  public void platformPlugin_getClipboardDataIsNonNullWhenContentUriWithTextProvided()
      throws IOException {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);
    setUpForTextClipboardTests(mockActivity);
    ContentResolver contentResolver = mock(ContentResolver.class);
    ClipData.Item mockItem = mock(ClipData.Item.class);
    Uri mockUri = mock(Uri.class);

    when(mockActivity.getContentResolver()).thenReturn(contentResolver);
    when(mockUri.getScheme()).thenReturn("content");
    when(mockItem.getText()).thenReturn(null);
    when(mockItem.getUri()).thenReturn(mockUri);
    when(contentResolver.openTypedAssetFileDescriptor(any(Uri.class), any(), any()))
        .thenReturn(mock(AssetFileDescriptor.class));
    when(mockItem.coerceToText(mockActivity)).thenReturn("something non-null");

    ClipData clip = new ClipData("label", new String[0], mockItem);

    assertNull(platformPlugin.getClipboardData(clipboardFormat));
    clipboardManager.setPrimaryClip(clip);
    assertNotNull(platformPlugin.getClipboardData(clipboardFormat));
  }

  @Test
  public void platformPlugin_getClipboardDataIsNullWhenContentUriProvidedContainsNoText()
      throws IOException {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);
    setUpForTextClipboardTests(mockActivity);
    ContentResolver contentResolver = ctx.getContentResolver();

    when(mockActivity.getContentResolver()).thenReturn(contentResolver);

    Uri uri = Uri.parse("content://media/external_primary/images/media/");
    ClipData clip = ClipData.newUri(contentResolver, "URI", uri);

    clipboardManager.setPrimaryClip(clip);
    assertNull(platformPlugin.getClipboardData(clipboardFormat));
  }

  @Test
  public void platformPlugin_getClipboardDataIsNullWhenNonContentUriProvided() throws IOException {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);
    setUpForTextClipboardTests(mockActivity);
    ContentResolver contentResolver = ctx.getContentResolver();

    when(mockActivity.getContentResolver()).thenReturn(contentResolver);

    Uri uri = Uri.parse("file:///");
    ClipData clip = ClipData.newUri(contentResolver, "URI", uri);

    clipboardManager.setPrimaryClip(clip);
    assertNull(platformPlugin.getClipboardData(clipboardFormat));
  }

  @Test
  public void platformPlugin_getClipboardDataIsNullWhenItemHasNoTextNorUri() throws IOException {
    View fakeDecorView = mock(View.class);
    Window fakeWindow = mock(Window.class);
    Activity mockActivity = mock(Activity.class);
    when(fakeWindow.getDecorView()).thenReturn(fakeDecorView);
    when(mockActivity.getWindow()).thenReturn(fakeWindow);
    PlatformPlugin platformPlugin = new PlatformPlugin(mockActivity);
    setUpForTextClipboardTests(mockActivity);
    ClipData.Item mockItem = mock(ClipData.Item.class);

    when(mockItem.getText()).thenReturn(null);
    when(mockItem.getUri()).thenReturn(null);

    ClipData clip = new ClipData("label", new String[0], mockItem);

    clipboardManager.setPrimaryClip(clip);
    assertNull(platformPlugin.getClipboardData(clipboardFormat));
  }

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
