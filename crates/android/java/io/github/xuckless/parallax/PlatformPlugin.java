// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/io/flutter/plugin/platform/PlatformPlugin.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. Only the Activity, TAG, vibrateHapticFeedback, popSystemNavigator,
// getClipboardData and setClipboardData are kept. The constructor takes the Activity alone (no
// PlatformChannel: GPUI's haptics and its window closing arrive through
// ParallaxSurfaceView.nativeFrameworkCalls; Rust calls the two clipboard methods over JNI,
// synchronously, on the main thread, as GPUI's clipboard calls are). popSystemNavigator keeps only
// its no-delegate, plain-Activity branch (finish). Dropped: PlatformChannel,
// PlatformPluginDelegate and mPlatformMessageHandler; destroy; system sounds, system chrome and
// overlays, orientation, app switcher description, clipboardHasStrings (GPUI has no such query),
// share, the OnBackPressedDispatcherOwner branch; androidx annotations; io.flutter.Log
// (→ android.util.Log); io.flutter.Build.API_LEVELS (→ Build.VERSION_CODES). The bridge:
// docs/haptics.md, docs/android-clipboard.md, docs/android-multi-window.md. Ledger:
// third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.res.AssetFileDescriptor;
import android.net.Uri;
import android.os.Build;
import android.util.Log;
import android.view.HapticFeedbackConstants;
import android.view.View;
import java.io.FileNotFoundException;
import java.io.IOException;

/** Android implementation of the platform plugin. */
public class PlatformPlugin {
  private static final String TAG = "PlatformPlugin";

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

  // Called by Rust over JNI (parallax-android's Platform::read_from_clipboard), with a null format.
  /* package */ CharSequence getClipboardData(PlatformChannel.ClipboardContentFormat format) {
    ClipboardManager clipboard =
        (ClipboardManager) activity.getSystemService(Context.CLIPBOARD_SERVICE);

    if (!clipboard.hasPrimaryClip()) return null;

    CharSequence itemText = null;
    try {
      ClipData clip = clipboard.getPrimaryClip();
      if (clip == null) return null;
      if (format == null || format == PlatformChannel.ClipboardContentFormat.PLAIN_TEXT) {
        ClipData.Item item = clip.getItemAt(0);
        // First, try getting clipboard data as text; no further processing
        // required if so.
        itemText = item.getText();
        if (itemText == null) {
          // Clipboard data does not contain text, so check whether or not it
          // contains a URI to extract text from.
          Uri itemUri = item.getUri();

          if (itemUri == null) {
            Log.w(
                TAG, "Clipboard item contained no textual content nor a URI to retrieve it from.");
            return null;
          }

          // Will only try to extract text from URI if it has the content scheme.
          String uriScheme = itemUri.getScheme();

          if (!uriScheme.equals("content")) {
            Log.w(
                TAG,
                "Clipboard item contains a Uri with scheme '" + uriScheme + "'that is unhandled.");
            return null;
          }

          AssetFileDescriptor assetFileDescriptor =
              activity.getContentResolver().openTypedAssetFileDescriptor(itemUri, "text/*", null);

          // Safely return clipboard data coerced into text; will return either
          // itemText or text retrieved from its URI.
          itemText = item.coerceToText(activity);
          if (assetFileDescriptor != null) assetFileDescriptor.close();
        }

        return itemText;
      }
    } catch (SecurityException e) {
      Log.w(
          TAG,
          "Attempted to get clipboard data that requires additional permission(s).\n"
              + "See the exception details for which permission(s) are required, and consider adding them to your Android Manifest as described in:\n"
              + "https://developer.android.com/guide/topics/permissions/overview",
          e);
      return null;
    } catch (FileNotFoundException e) {
      Log.w(TAG, "Clipboard text was unable to be received from content URI.");
      return null;
    } catch (IOException e) {
      Log.w(TAG, "Failed to close AssetFileDescriptor while trying to read text from URI.", e);
      return itemText;
    }

    return null;
  }

  // Called by Rust over JNI (parallax-android's Platform::write_to_clipboard).
  /* package */ void setClipboardData(String text) {
    ClipboardManager clipboard =
        (ClipboardManager) activity.getSystemService(Context.CLIPBOARD_SERVICE);
    ClipData clip = ClipData.newPlainText("text label?", text);
    clipboard.setPrimaryClip(clip);
  }
}
