// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/io/flutter/embedding/engine/systemchannels/TextInputChannel.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. The `flutter/textinput` MethodChannel becomes JNI: Rust (GPUI) sends
// the framework's calls in Flutter's JSON form as the return value of
// ParallaxSurfaceView.nativeFrame (handleFrameworkCalls below, in place of the channel's
// MethodCallHandler), and the calls to the framework become static natives on
// ParallaxSurfaceView, tagged with the view's Activity id (which replaces the DartExecutor).
// Kept: show, hide, setClient, setEditingState, clearClient; updateEditingState; the
// performAction family; Configuration (fromJson), InputType, TextInputType, TextCapitalization,
// TextEditState. Dropped: autofill (Configuration.Autofill, fields, requestAutofill,
// finishAutofillContext, updateEditingStateWithTag), the delta model (enableDeltaModel,
// updateEditingStateWithDeltas), platform views (setPlatformViewClient), setEditableSizeAndTransform,
// app private commands (sendAppPrivateCommand, performPrivateCommand), commitContent
// (contentCommitMimeTypes), requestExistingInputState; androidx annotations; io.flutter.Log
// (→ android.util.Log). handleFrameworkCalls catches a call's RuntimeException and logs it, as the
// MethodChannel's handler did. updateEditingState's log gives the text's length, not the text (it
// may be a password). Added: boundsForRange (GPUI's caret bounds for CursorAnchorInfo). The sync
// protocol: docs/android-text-input.md. Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.util.Log;
import android.view.inputmethod.EditorInfo;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * {@link TextInputChannel} is a platform channel between Android and Flutter that is used to
 * communicate information about the user's text input.
 *
 * <p>When the user presses an action button like "done" or "next", that action is sent from Android
 * to Flutter through this {@link TextInputChannel}.
 *
 * <p>When an input system in the Flutter app wants to show the keyboard, or hide it, or configure
 * editing state, etc. a message is sent from Flutter to Android through this {@link
 * TextInputChannel}.
 */
public class TextInputChannel {
  private static final String TAG = "TextInputChannel";

  // Parallax: in place of the DartExecutor, the id of the view's Activity, which tags every call
  // to Rust.
  private final int activity;
  private TextInputMethodHandler textInputMethodHandler;

  // Parallax: the channel's MethodCallHandler. `calls` is a JSON array of {"method", "args"}
  // objects in Flutter's `flutter/textinput` encoding, in order (host.rs builds it).
  void handleFrameworkCalls(String calls) {
    if (textInputMethodHandler == null) {
      // If no explicit TextInputMethodHandler has been registered then we don't
      // need to forward this call to an API. Return.
      return;
    }
    try {
      final JSONArray callList = new JSONArray(calls);
      for (int i = 0; i < callList.length(); i++) {
        final JSONObject call = callList.getJSONObject(i);
        final String method = call.getString("method");
        try {
          onMethodCall(method, call.opt("args"));
        } catch (RuntimeException e) {
          // As MethodChannel's IncomingMethodCallHandler: a failing call is logged, not thrown into
          // the caller (here doFrame, where it would end the app).
          Log.e(TAG, "Failed to handle method call " + method, e);
        }
      }
    } catch (JSONException exception) {
      Log.e(TAG, "unreadable text input calls: " + exception.getMessage());
    }
  }

  private void onMethodCall(String method, Object args) {
    Log.v(TAG, "Received '" + method + "' message.");
    switch (method) {
      case "TextInput.show":
        textInputMethodHandler.show();
        break;
      case "TextInput.hide":
        textInputMethodHandler.hide();
        break;
      case "TextInput.setClient":
        try {
          final JSONArray argumentList = (JSONArray) args;
          final int textInputClientId = argumentList.getInt(0);
          final JSONObject jsonConfiguration = argumentList.getJSONObject(1);
          textInputMethodHandler.setClient(
              textInputClientId, Configuration.fromJson(jsonConfiguration));
        } catch (JSONException | NoSuchFieldException exception) {
          // JSONException: missing keys or bad value types.
          // NoSuchFieldException: one or more values were invalid.
          Log.e(TAG, "error: " + exception.getMessage());
        }
        break;
      case "TextInput.setEditingState":
        try {
          final JSONObject editingState = (JSONObject) args;
          textInputMethodHandler.setEditingState(TextEditState.fromJson(editingState));
        } catch (JSONException exception) {
          Log.e(TAG, "error: " + exception.getMessage());
        }
        break;
      case "TextInput.clearClient":
        textInputMethodHandler.clearClient();
        break;
      default:
        Log.e(TAG, "not implemented: " + method);
        break;
    }
  }

  /** Constructs a {@code TextInputChannel} that connects Android to GPUI in Rust. */
  public TextInputChannel(int activity) {
    this.activity = activity;
  }

  /**
   * Instructs Flutter to update its text input editing state to reflect the given configuration.
   */
  public void updateEditingState(
      int inputClientId,
      String text,
      int selectionStart,
      int selectionEnd,
      int composingStart,
      int composingEnd) {
    Log.v(
        TAG,
        "Sending message to update editing state: \n"
            + "Text length: "
            + text.length()
            + "\n"
            + "Selection start: "
            + selectionStart
            + "\n"
            + "Selection end: "
            + selectionEnd
            + "\n"
            + "Composing start: "
            + composingStart
            + "\n"
            + "Composing end: "
            + composingEnd);

    ParallaxSurfaceView.nativeUpdateEditingState(
        activity, text, selectionStart, selectionEnd, composingStart, composingEnd);
  }

  // Parallax: the performAction family below goes through one native, with Flutter's action name.
  private void performAction(String action) {
    ParallaxSurfaceView.nativePerformAction(activity, action);
  }

  /** Instructs Flutter to execute a "newline" action. */
  public void newline(int inputClientId) {
    Log.v(TAG, "Sending 'newline' message.");
    performAction("TextInputAction.newline");
  }

  /** Instructs Flutter to execute a "go" action. */
  public void go(int inputClientId) {
    Log.v(TAG, "Sending 'go' message.");
    performAction("TextInputAction.go");
  }

  /** Instructs Flutter to execute a "search" action. */
  public void search(int inputClientId) {
    Log.v(TAG, "Sending 'search' message.");
    performAction("TextInputAction.search");
  }

  /** Instructs Flutter to execute a "send" action. */
  public void send(int inputClientId) {
    Log.v(TAG, "Sending 'send' message.");
    performAction("TextInputAction.send");
  }

  /** Instructs Flutter to execute a "done" action. */
  public void done(int inputClientId) {
    Log.v(TAG, "Sending 'done' message.");
    performAction("TextInputAction.done");
  }

  /** Instructs Flutter to execute a "next" action. */
  public void next(int inputClientId) {
    Log.v(TAG, "Sending 'next' message.");
    performAction("TextInputAction.next");
  }

  /** Instructs Flutter to execute a "previous" action. */
  public void previous(int inputClientId) {
    Log.v(TAG, "Sending 'previous' message.");
    performAction("TextInputAction.previous");
  }

  /** Instructs Flutter to execute an "unspecified" action. */
  public void unspecifiedAction(int inputClientId) {
    Log.v(TAG, "Sending 'unspecified' message.");
    performAction("TextInputAction.unspecified");
  }

  // Parallax: GPUI's bounds of the UTF-16 range [start, end) in view pixels, written to
  // `ltrb` (left, top, right, bottom); false when GPUI has none. Flutter's framework pushes its
  // geometry instead (setEditableSizeAndTransform, dropped).
  boolean boundsForRange(int start, int end, float[] ltrb) {
    return ParallaxSurfaceView.nativeBoundsForRange(activity, start, end, ltrb);
  }

  /**
   * Sets the {@link TextInputMethodHandler} which receives all events and requests that are parsed
   * from the underlying platform channel.
   */
  public void setTextInputMethodHandler(TextInputMethodHandler textInputMethodHandler) {
    this.textInputMethodHandler = textInputMethodHandler;
  }

  public interface TextInputMethodHandler {
    /**
     * Requests that the software keyboard (IME) be displayed on screen for the active text input
     * client.
     */
    void show();

    /**
     * Requests that the software keyboard (IME) be hidden from screen, dismissing the current text
     * input view.
     */
    void hide();

    /**
     * Sets the active text input client identifier along with its input text configurations (e.g.,
     * text capitalization rules, obscure text flags, and input actions such as done, go, or next).
     *
     * @param textInputClientId The unique ID of the text input client sent by the Dart framework
     *     via {@code TextInput.setClient}.
     * @param configuration The {@link Configuration} text editing configuration containing layout
     *     and behavior options.
     */
    void setClient(int textInputClientId, Configuration configuration);

    /**
     * Sets the current text selection, composing ranges, and raw string contents for the active
     * text input client.
     *
     * @param editingState The text state model containing the text, selections, and composing
     *     offsets.
     */
    void setEditingState(TextEditState editingState);

    /** Clears the active text input client and tears down the text editing connection. */
    void clearClient();
  }

  /** A text editing configuration. */
  public static class Configuration {
    public static Configuration fromJson(JSONObject json)
        throws JSONException, NoSuchFieldException {
      final String inputActionName = json.getString("inputAction");
      if (inputActionName == null) {
        throw new JSONException("Configuration JSON missing 'inputAction' property.");
      }
      final Integer inputAction = inputActionFromTextInputAction(inputActionName);

      // Build an array of hint locales from the data in the JSON list.
      Locale[] hintLocales = null;
      if (!json.isNull("hintLocales")) {
        JSONArray hintLocalesJson = json.getJSONArray("hintLocales");
        hintLocales = new Locale[hintLocalesJson.length()];
        for (int i = 0; i < hintLocalesJson.length(); i++) {
          hintLocales[i] = Locale.forLanguageTag(hintLocalesJson.optString(i));
        }
      }

      return new Configuration(
          json.optBoolean("obscureText"),
          json.optBoolean("autocorrect", true),
          json.optBoolean("enableSuggestions"),
          json.optBoolean("enableIMEPersonalizedLearning"),
          TextCapitalization.fromValue(json.getString("textCapitalization")),
          InputType.fromJson(json.getJSONObject("inputType")),
          inputAction,
          json.isNull("actionLabel") ? null : json.getString("actionLabel"),
          hintLocales);
    }

    private static Integer inputActionFromTextInputAction(String inputAction) {
      switch (inputAction) {
        case "TextInputAction.newline":
        case "TextInputAction.none":
          return EditorInfo.IME_ACTION_NONE;
        case "TextInputAction.unspecified":
          return EditorInfo.IME_ACTION_UNSPECIFIED;
        case "TextInputAction.done":
          return EditorInfo.IME_ACTION_DONE;
        case "TextInputAction.go":
          return EditorInfo.IME_ACTION_GO;
        case "TextInputAction.search":
          return EditorInfo.IME_ACTION_SEARCH;
        case "TextInputAction.send":
          return EditorInfo.IME_ACTION_SEND;
        case "TextInputAction.next":
          return EditorInfo.IME_ACTION_NEXT;
        case "TextInputAction.previous":
          return EditorInfo.IME_ACTION_PREVIOUS;
        default:
          // Present default key if bad input type is given.
          return EditorInfo.IME_ACTION_UNSPECIFIED;
      }
    }

    public final boolean obscureText;
    public final boolean autocorrect;
    public final boolean enableSuggestions;
    public final boolean enableIMEPersonalizedLearning;
    public final TextCapitalization textCapitalization;
    public final InputType inputType;
    public final Integer inputAction;
    public final String actionLabel;
    public final Locale[] hintLocales;

    public Configuration(
        boolean obscureText,
        boolean autocorrect,
        boolean enableSuggestions,
        boolean enableIMEPersonalizedLearning,
        TextCapitalization textCapitalization,
        InputType inputType,
        Integer inputAction,
        String actionLabel,
        Locale[] hintLocales) {
      this.obscureText = obscureText;
      this.autocorrect = autocorrect;
      this.enableSuggestions = enableSuggestions;
      this.enableIMEPersonalizedLearning = enableIMEPersonalizedLearning;
      this.textCapitalization = textCapitalization;
      this.inputType = inputType;
      this.inputAction = inputAction;
      this.actionLabel = actionLabel;
      this.hintLocales = hintLocales;
    }
  }

  /**
   * A text input type.
   *
   * <p>If the {@link #type} is {@link TextInputType#NUMBER}, this {@code InputType} also reports
   * whether that number {@link #isSigned}, {@link #isDecimal}, and {@link #isPassword}.
   */
  public static class InputType {
    public static InputType fromJson(JSONObject json) throws JSONException, NoSuchFieldException {
      return new InputType(
          TextInputType.fromValue(json.getString("name")),
          json.optBoolean("signed", false),
          json.optBoolean("decimal", false),
          json.optBoolean("password", false));
    }

    public final TextInputType type;
    public final boolean isSigned;
    public final boolean isDecimal;
    public final boolean isPassword;

    /** Constructs an {@code InputType} for {@code type} with the given NUMBER-variation flags. */
    public InputType(TextInputType type, boolean isSigned, boolean isDecimal, boolean isPassword) {
      this.type = type;
      this.isSigned = isSigned;
      this.isDecimal = isDecimal;
      this.isPassword = isPassword;
    }
  }

  /** Types of text input. */
  public enum TextInputType {
    TEXT("TextInputType.text"),
    DATETIME("TextInputType.datetime"),
    NAME("TextInputType.name"),
    POSTAL_ADDRESS("TextInputType.address"),
    NUMBER("TextInputType.number"),
    PHONE("TextInputType.phone"),
    MULTILINE("TextInputType.multiline"),
    EMAIL_ADDRESS("TextInputType.emailAddress"),
    URL("TextInputType.url"),
    VISIBLE_PASSWORD("TextInputType.visiblePassword"),
    NONE("TextInputType.none"),
    WEB_SEARCH("TextInputType.webSearch"),
    TWITTER("TextInputType.twitter");

    static TextInputType fromValue(String encodedName) throws NoSuchFieldException {
      for (TextInputType textInputType : TextInputType.values()) {
        if (textInputType.encodedName.equals(encodedName)) {
          return textInputType;
        }
      }
      throw new NoSuchFieldException("No such TextInputType: " + encodedName);
    }

    private final String encodedName;

    TextInputType(String encodedName) {
      this.encodedName = encodedName;
    }
  }

  /** Text capitalization schemes. */
  public enum TextCapitalization {
    CHARACTERS("TextCapitalization.characters"),
    WORDS("TextCapitalization.words"),
    SENTENCES("TextCapitalization.sentences"),
    NONE("TextCapitalization.none");

    static TextCapitalization fromValue(String encodedName) throws NoSuchFieldException {
      for (TextCapitalization textCapitalization : TextCapitalization.values()) {
        if (textCapitalization.encodedName.equals(encodedName)) {
          return textCapitalization;
        }
      }
      throw new NoSuchFieldException("No such TextCapitalization: " + encodedName);
    }

    private final String encodedName;

    TextCapitalization(String encodedName) {
      this.encodedName = encodedName;
    }
  }

  /** State of an on-going text editing session. */
  public static class TextEditState {
    public static TextEditState fromJson(JSONObject textEditState) throws JSONException {
      return new TextEditState(
          textEditState.getString("text"),
          textEditState.getInt("selectionBase"),
          textEditState.getInt("selectionExtent"),
          textEditState.getInt("composingBase"),
          textEditState.getInt("composingExtent"));
    }

    public final String text;
    public final int selectionStart;
    public final int selectionEnd;
    public final int composingStart;
    public final int composingEnd;

    public TextEditState(
        String text, int selectionStart, int selectionEnd, int composingStart, int composingEnd)
        throws IndexOutOfBoundsException {

      if ((selectionStart != -1 || selectionEnd != -1)
          && (selectionStart < 0 || selectionEnd < 0)) {
        throw new IndexOutOfBoundsException(
            "invalid selection: (" + selectionStart + ", " + selectionEnd + ")");
      }

      if ((composingStart != -1 || composingEnd != -1)
          && (composingStart < 0 || composingStart > composingEnd)) {
        throw new IndexOutOfBoundsException(
            "invalid composing range: (" + composingStart + ", " + composingEnd + ")");
      }

      if (composingEnd > text.length()) {
        throw new IndexOutOfBoundsException("invalid composing start: " + composingStart);
      }

      if (selectionStart > text.length()) {
        throw new IndexOutOfBoundsException("invalid selection start: " + selectionStart);
      }

      if (selectionEnd > text.length()) {
        throw new IndexOutOfBoundsException("invalid selection end: " + selectionEnd);
      }

      this.text = text;
      this.selectionStart = selectionStart;
      this.selectionEnd = selectionEnd;
      this.composingStart = composingStart;
      this.composingEnd = composingEnd;
    }

    public boolean hasSelection() {
      // When selectionStart == -1, it's guaranteed that selectionEnd will also
      // be -1.
      return selectionStart >= 0;
    }

    public boolean hasComposing() {
      return composingStart >= 0 && composingEnd > composingStart;
    }
  }
}
