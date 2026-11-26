// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/io/flutter/plugin/editing/TextInputPlugin.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. Input parts only: show/hide, setTextInputClient,
// setTextInputEditingState (with Flutter's restart rules), createInputConnection (EditorInfo from
// the configuration), handleKeyEvent, didChangeEditingState and the
// ImeSyncDeferringInsetsCallback setup. Dropped: autofill (AutofillManager, notifyView*,
// onProvideAutofillVirtualStructure, autofill), platform views (PlatformViewsController(2), the
// platform-view InputTarget types, lock/unlock, clearPlatformViewClient,
// setPlatformViewTextInputClient), ScribeChannel, saveEditableSizeAndTransform (lastClientRect
// fed autofill only), sendTextInputAppPrivateCommand, requestExistingInputState, destroy (the
// view lives as long as its Activity), the delta-model branch (GPUI takes full states),
// contentCommitMimeTypes and stylus handwriting (EditorInfoCompat); androidx annotations;
// ViewCompat/WindowInsetsCompat → View.getRootWindowInsets (API 30+, else no restart);
// io.flutter.Log → android.util.Log; io.flutter.Build.API_LEVELS → Build.VERSION_CODES;
// didChangeEditingState's log gives the text's length, not the text (it may be a password). The sync
// with GPUI: docs/android-text-input.md.
// Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.content.Context;
import android.os.Build;
import android.os.LocaleList;
import android.text.Editable;
import android.text.InputType;
import android.util.Log;
import android.view.KeyEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import io.github.xuckless.parallax.TextInputChannel.TextEditState;
import java.util.ArrayList;

/** Android implementation of the text input plugin. */
public class TextInputPlugin implements ListenableEditingState.EditingStateWatcher {
  private static final String TAG = "TextInputPlugin";

  private final View mView;
  private final InputMethodManager mImm;
  private final TextInputChannel textInputChannel;
  private InputTarget inputTarget = new InputTarget(InputTarget.Type.NO_TARGET, 0);
  private TextInputChannel.Configuration configuration;
  private ListenableEditingState mEditable;
  private boolean mRestartInputPending;
  private InputConnection lastInputConnection;
  private ImeSyncDeferringInsetsCallback imeSyncCallback;

  // Initialize the "last seen" text editing values to a non-null value.
  private TextEditState mLastKnownFrameworkTextEditingState;

  public TextInputPlugin(View view, TextInputChannel textInputChannel) {
    mView = view;
    // Create a default object.
    mEditable = new ListenableEditingState(null, mView);
    mImm = (InputMethodManager) view.getContext().getSystemService(Context.INPUT_METHOD_SERVICE);

    // Sets up syncing ime insets with the framework, allowing
    // the Flutter view to grow and shrink to accommodate Android
    // controlled keyboard animations.
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      imeSyncCallback = new ImeSyncDeferringInsetsCallback(view);
      imeSyncCallback.install();

      // When the IME is hidden, we need to restart the input method manager to accomodate
      // some keyboards like the Samsung keyboard that may be caching old state.
      imeSyncCallback.setImeVisibilityListener(
          new ImeSyncDeferringInsetsCallback.ImeVisibilityListener() {
            @Override
            public void onImeVisibilityChanged(boolean visible) {
              if (!visible) {
                mImm.restartInput(mView);
              }
            }
          });
    }

    this.textInputChannel = textInputChannel;
    textInputChannel.setTextInputMethodHandler(
        new TextInputChannel.TextInputMethodHandler() {
          @Override
          public void show() {
            showTextInput(mView);
          }

          @Override
          public void hide() {
            hideTextInput(mView);
          }

          @Override
          public void setClient(
              int textInputClientId, TextInputChannel.Configuration configuration) {
            setTextInputClient(textInputClientId, configuration);
          }

          @Override
          public void setEditingState(TextInputChannel.TextEditState editingState) {
            setTextInputEditingState(mView, editingState);
          }

          @Override
          public void clearClient() {
            clearTextInputClient();
          }
        });
  }

  public InputMethodManager getInputMethodManager() {
    return mImm;
  }

  Editable getEditable() {
    return mEditable;
  }

  ImeSyncDeferringInsetsCallback getImeSyncCallback() {
    return imeSyncCallback;
  }

  private static int inputTypeFromTextInputType(
      TextInputChannel.InputType type,
      boolean obscureText,
      boolean autocorrect,
      boolean enableSuggestions,
      boolean enableIMEPersonalizedLearning,
      TextInputChannel.TextCapitalization textCapitalization) {
    if (type.type == TextInputChannel.TextInputType.DATETIME) {
      return InputType.TYPE_CLASS_DATETIME;
    } else if (type.type == TextInputChannel.TextInputType.NUMBER) {
      int textType = InputType.TYPE_CLASS_NUMBER;
      if (type.isSigned) {
        textType |= InputType.TYPE_NUMBER_FLAG_SIGNED;
      }
      if (type.isDecimal) {
        textType |= InputType.TYPE_NUMBER_FLAG_DECIMAL;
      }
      if (type.isPassword) {
        textType |= InputType.TYPE_NUMBER_VARIATION_PASSWORD;
      }
      return textType;
    } else if (type.type == TextInputChannel.TextInputType.PHONE) {
      return InputType.TYPE_CLASS_PHONE;
    } else if (type.type == TextInputChannel.TextInputType.NONE) {
      return InputType.TYPE_NULL;
    }

    int textType = InputType.TYPE_CLASS_TEXT;
    if (type.type == TextInputChannel.TextInputType.MULTILINE) {
      textType |= InputType.TYPE_TEXT_FLAG_MULTI_LINE;
    } else if (type.type == TextInputChannel.TextInputType.EMAIL_ADDRESS
        || type.type == TextInputChannel.TextInputType.TWITTER) {
      textType |= InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS;
    } else if (type.type == TextInputChannel.TextInputType.URL
        || type.type == TextInputChannel.TextInputType.WEB_SEARCH) {
      textType |= InputType.TYPE_TEXT_VARIATION_URI;
    } else if (type.type == TextInputChannel.TextInputType.VISIBLE_PASSWORD) {
      textType |= InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD;
    } else if (type.type == TextInputChannel.TextInputType.NAME) {
      textType |= InputType.TYPE_TEXT_VARIATION_PERSON_NAME;
    } else if (type.type == TextInputChannel.TextInputType.POSTAL_ADDRESS) {
      textType |= InputType.TYPE_TEXT_VARIATION_POSTAL_ADDRESS;
    }

    if (obscureText) {
      // Note: both required. Some devices ignore TYPE_TEXT_FLAG_NO_SUGGESTIONS.
      textType |= InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
      textType |= InputType.TYPE_TEXT_VARIATION_PASSWORD;
    } else {
      if (autocorrect) textType |= InputType.TYPE_TEXT_FLAG_AUTO_CORRECT;
      if (!enableSuggestions) {
        // Note: both required. Some devices ignore TYPE_TEXT_FLAG_NO_SUGGESTIONS.
        textType |= InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS;
        textType |= InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD;
      }
    }

    if (textCapitalization == TextInputChannel.TextCapitalization.CHARACTERS) {
      textType |= InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS;
    } else if (textCapitalization == TextInputChannel.TextCapitalization.WORDS) {
      textType |= InputType.TYPE_TEXT_FLAG_CAP_WORDS;
    } else if (textCapitalization == TextInputChannel.TextCapitalization.SENTENCES) {
      textType |= InputType.TYPE_TEXT_FLAG_CAP_SENTENCES;
    }

    return textType;
  }

  public InputConnection createInputConnection(
      View view, KeyboardManager keyboardManager, EditorInfo outAttrs) {
    if (inputTarget.type == InputTarget.Type.NO_TARGET) {
      lastInputConnection = null;
      return null;
    }

    outAttrs.inputType =
        inputTypeFromTextInputType(
            configuration.inputType,
            configuration.obscureText,
            configuration.autocorrect,
            configuration.enableSuggestions,
            configuration.enableIMEPersonalizedLearning,
            configuration.textCapitalization);
    outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN;

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O
        && !configuration.enableIMEPersonalizedLearning) {
      outAttrs.imeOptions |= EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
    }

    int enterAction;
    if (configuration.inputAction == null) {
      // If an explicit input action isn't set, then default to none for multi-line fields
      // and done for single line fields.
      enterAction =
          (InputType.TYPE_TEXT_FLAG_MULTI_LINE & outAttrs.inputType) != 0
              ? EditorInfo.IME_ACTION_NONE
              : EditorInfo.IME_ACTION_DONE;
    } else {
      enterAction = configuration.inputAction;
    }
    if (configuration.actionLabel != null) {
      outAttrs.actionLabel = configuration.actionLabel;
      outAttrs.actionId = enterAction;
    }
    outAttrs.imeOptions |= enterAction;

    if (configuration.hintLocales != null) {
      outAttrs.hintLocales = new LocaleList(configuration.hintLocales);
    }

    InputConnectionAdaptor connection =
        new InputConnectionAdaptor(
            view, inputTarget.id, textInputChannel, keyboardManager, mEditable, outAttrs);
    outAttrs.initialSelStart = mEditable.getSelectionStart();
    outAttrs.initialSelEnd = mEditable.getSelectionEnd();

    lastInputConnection = connection;
    return lastInputConnection;
  }

  public InputConnection getLastInputConnection() {
    return lastInputConnection;
  }

  void showTextInput(View view) {
    if (configuration == null
        || configuration.inputType == null
        || configuration.inputType.type != TextInputChannel.TextInputType.NONE) {
      view.requestFocus();
      mImm.showSoftInput(view, 0);
    } else {
      hideTextInput(view);
    }
  }

  private void hideTextInput(View view) {
    // Note: when a virtual display is used, a race condition may lead to us hiding the keyboard
    // here just after a platform view has shown it.
    // This can only potentially happen when switching focus from a Flutter text field to a platform
    // view's text
    // field(by text field here I mean anything that keeps the keyboard open).
    // See: https://github.com/flutter/flutter/issues/34169
    mImm.hideSoftInputFromWindow(view.getApplicationWindowToken(), 0);
  }

  void setTextInputClient(int client, TextInputChannel.Configuration configuration) {
    this.configuration = configuration;
    inputTarget = new InputTarget(InputTarget.Type.FRAMEWORK_CLIENT, client);

    mEditable.removeEditingStateListener(this);
    mEditable = new ListenableEditingState(null, mView);

    // setTextInputClient will be followed by a call to setTextInputEditingState.
    // Do a restartInput at that time.
    mRestartInputPending = true;
    mEditable.addEditingStateListener(this);
  }

  private static boolean composingChanged(
      TextInputChannel.TextEditState before, TextInputChannel.TextEditState after) {
    final int composingRegionLength = before.composingEnd - before.composingStart;
    if (composingRegionLength != after.composingEnd - after.composingStart) {
      return true;
    }
    for (int index = 0; index < composingRegionLength; index++) {
      if (before.text.charAt(index + before.composingStart)
          != after.text.charAt(index + after.composingStart)) {
        return true;
      }
    }
    return false;
  }

  // Called by the text input channel to update the text input plugin with the
  // latest TextEditState from the framework.
  void setTextInputEditingState(View view, TextInputChannel.TextEditState state) {
    if (!mRestartInputPending
        && mLastKnownFrameworkTextEditingState != null
        && mLastKnownFrameworkTextEditingState.hasComposing()) {
      // Also restart input if the framework (or the developer) decides to
      // change the composing region by itself (which is discouraged). Many IMEs
      // don't expect editors to commit composing text, so a restart is needed
      // to reset their internal states.
      mRestartInputPending = composingChanged(mLastKnownFrameworkTextEditingState, state);
      if (mRestartInputPending) {
        Log.i(TAG, "Composing region changed by the framework. Restarting the input method.");
      }
    }

    mLastKnownFrameworkTextEditingState = state;
    mEditable.setEditingState(state);

    // Restart if needed. Restarting will also update the selection.
    if (mRestartInputPending) {
      mImm.restartInput(view);
      mRestartInputPending = false;
    }
  }

  void clearTextInputClient() {
    mEditable.removeEditingStateListener(this);
    configuration = null;
    inputTarget = new InputTarget(InputTarget.Type.NO_TARGET, 0);
    // When the IME is hidden, we need to restart the input method manager to accomodate
    // some keyboards like the Samsung keyboard that may be caching old state.
    // Parallax: WindowInsets.isVisible is API 30+ (Flutter's WindowInsetsCompat is androidx).
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      WindowInsets insets = mView.getRootWindowInsets();
      if (insets != null && !insets.isVisible(WindowInsets.Type.ime())) {
        mImm.restartInput(mView);
      }
    }
  }

  private static class InputTarget {
    enum Type {
      NO_TARGET,
      // InputConnection is managed by the TextInputPlugin, and events are forwarded to the Flutter
      // framework.
      FRAMEWORK_CLIENT,
    }

    public InputTarget(Type type, int id) {
      this.type = type;
      this.id = id;
    }

    Type type;
    // The ID of the input target.
    //
    // For framework clients this is the framework input connection client ID.
    int id;
  }

  // -------- Start: KeyboardManager Synchronous Responder -------
  public boolean handleKeyEvent(KeyEvent keyEvent) {
    if (!getInputMethodManager().isAcceptingText() || lastInputConnection == null) {
      return false;
    }

    // Send the KeyEvent as an IME KeyEvent. If the input connection is an
    // InputConnectionAdaptor then call its handleKeyEvent method (because
    // this method will be called by the keyboard manager, and
    // InputConnectionAdaptor#sendKeyEvent forwards the key event back to the
    // keyboard manager).
    return (lastInputConnection instanceof InputConnectionAdaptor)
        ? ((InputConnectionAdaptor) lastInputConnection).handleKeyEvent(keyEvent)
        : lastInputConnection.sendKeyEvent(keyEvent);
  }
  // -------- End: KeyboardManager Synchronous Responder -------

  // -------- Start: ListenableEditingState watcher implementation -------

  @Override
  public void didChangeEditingState(
      boolean textChanged, boolean selectionChanged, boolean composingRegionChanged) {
    final int selectionStart = mEditable.getSelectionStart();
    final int selectionEnd = mEditable.getSelectionEnd();
    final int composingStart = mEditable.getComposingStart();
    final int composingEnd = mEditable.getComposingEnd();

    final ArrayList<TextEditingDelta> batchTextEditingDeltas =
        mEditable.extractBatchTextEditingDeltas();
    final boolean skipFrameworkUpdate =
        // The framework needs to send its editing state first.
        mLastKnownFrameworkTextEditingState == null
            || (mEditable.toString().equals(mLastKnownFrameworkTextEditingState.text)
                && selectionStart == mLastKnownFrameworkTextEditingState.selectionStart
                && selectionEnd == mLastKnownFrameworkTextEditingState.selectionEnd
                && composingStart == mLastKnownFrameworkTextEditingState.composingStart
                && composingEnd == mLastKnownFrameworkTextEditingState.composingEnd);
    if (!skipFrameworkUpdate) {
      Log.v(TAG, "send EditingState to flutter: " + mEditable.length() + " chars");

      textInputChannel.updateEditingState(
          inputTarget.id,
          mEditable.toString(),
          selectionStart,
          selectionEnd,
          composingStart,
          composingEnd);
      mLastKnownFrameworkTextEditingState =
          new TextEditState(
              mEditable.toString(), selectionStart, selectionEnd, composingStart, composingEnd);
    } else {
      // Don't accumulate deltas if they are not sent to the framework.
      mEditable.clearBatchDeltas();
    }
  }

  // -------- End: ListenableEditingState watcher implementation -------
}
