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
// ParallaxSurfaceView.nativeFrameworkCalls (handleFrameworkCalls below, in place of the channel's
// MethodCallHandler), and the calls to the framework become static natives on
// ParallaxSurfaceView, tagged with the view's Activity id (which replaces the DartExecutor).
// Kept: show, hide, setClient, setEditingState, clearClient, requestAutofill,
// setEditableSizeAndTransform, finishAutofillContext; updateEditingState; the performAction family;
// Configuration (fromJson, with Autofill and fields), InputType, TextInputType, TextCapitalization,
// TextEditState. updateEditingStateWithTag only logs: GPUI announces the focused field alone (no
// `fields`), so autofill never fills another field. Dropped: the delta model (enableDeltaModel,
// updateEditingStateWithDeltas), platform views (setPlatformViewClient), app private commands (sendAppPrivateCommand, performPrivateCommand), commitContent
// (contentCommitMimeTypes), requestExistingInputState; androidx annotations; io.flutter.Log
// (→ android.util.Log). handleFrameworkCalls catches a call's RuntimeException and logs it, as the
// MethodChannel's handler did. updateEditingState's log gives the text's length, not the text (it
// may be a password). Added: boundsForRange (GPUI's caret bounds for CursorAnchorInfo). The sync
// protocol: docs/android-text-input.md. Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.os.Build;
import android.util.Log;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import java.util.HashMap;
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
  // objects in Flutter's `flutter/textinput` encoding, in order (host.rs builds it; the view takes
  // it from nativeFrameworkCalls).
  void handleFrameworkCalls(JSONArray callList) {
    if (textInputMethodHandler == null) {
      // If no explicit TextInputMethodHandler has been registered then we don't
      // need to forward this call to an API. Return.
      return;
    }
    try {
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
      case "TextInput.requestAutofill":
        textInputMethodHandler.requestAutofill();
        break;
      case "TextInput.setEditingState":
        try {
          final JSONObject editingState = (JSONObject) args;
          textInputMethodHandler.setEditingState(TextEditState.fromJson(editingState));
        } catch (JSONException exception) {
          Log.e(TAG, "error: " + exception.getMessage());
        }
        break;
      case "TextInput.setEditableSizeAndTransform":
        try {
          final JSONObject arguments = (JSONObject) args;
          final double width = arguments.getDouble("width");
          final double height = arguments.getDouble("height");
          final JSONArray jsonMatrix = arguments.getJSONArray("transform");
          final double[] matrix = new double[16];
          for (int i = 0; i < 16; i++) {
            matrix[i] = jsonMatrix.getDouble(i);
          }

          textInputMethodHandler.setEditableSizeAndTransform(width, height, matrix);
        } catch (JSONException exception) {
          Log.e(TAG, "error: " + exception.getMessage());
        }
        break;
      case "TextInput.clearClient":
        textInputMethodHandler.clearClient();
        break;
      case "TextInput.finishAutofillContext":
        textInputMethodHandler.finishAutofillContext((boolean) args);
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

  public void updateEditingStateWithTag(
      int inputClientId, HashMap<String, TextEditState> editStates) {
    Log.v(TAG, "Sending message to update editing state for " + editStates.size() + " field(s).");
    // Parallax: GPUI announces only the focused field (setClient carries no `fields`), so the map is
    // always empty: the focused field's autofilled value reaches GPUI through updateEditingState.
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
     * Requests that the autofill dropdown menu appear for the current client.
     *
     * <p>Has no effect if the current client does not support autofill.
     */
    void requestAutofill();

    /**
     * Requests that the {@link android.view.autofill.AutofillManager} cancel or commit the current
     * autofill context.
     *
     * <p>The method calls {@link android.view.autofill.AutofillManager#commit()} when {@code
     * shouldSave} is true, and calls {@link android.view.autofill.AutofillManager#cancel()}
     * otherwise.
     *
     * @param shouldSave whether the active autofill service should save the current user input for
     *     future use.
     */
    void finishAutofillContext(boolean shouldSave);

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
     * Sets the size and the transform matrix of the current text input client.
     *
     * @param width the width of text input client. Must be finite.
     * @param height the height of text input client. Must be finite.
     * @param transform a 4x4 matrix that maps the local paint coordinate system to coordinate
     *     system of the FlutterView that owns the current client.
     */
    void setEditableSizeAndTransform(double width, double height, double[] transform);

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
      Configuration[] fields = null;
      if (!json.isNull("fields")) {
        final JSONArray jsonFields = json.getJSONArray("fields");
        fields = new Configuration[jsonFields.length()];
        for (int i = 0; i < fields.length; i++) {
          fields[i] = Configuration.fromJson(jsonFields.getJSONObject(i));
        }
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
          json.isNull("autofill") ? null : Autofill.fromJson(json.getJSONObject("autofill")),
          fields,
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

    public static class Autofill {
      public static Autofill fromJson(JSONObject json)
          throws JSONException, NoSuchFieldException {
        final String uniqueIdentifier = json.getString("uniqueIdentifier");
        final JSONArray hints = json.getJSONArray("hints");
        final String hintText = json.isNull("hintText") ? null : json.getString("hintText");
        final JSONObject editingState = json.getJSONObject("editingValue");
        final String[] autofillHints = new String[hints.length()];

        for (int i = 0; i < hints.length(); i++) {
          autofillHints[i] = translateAutofillHint(hints.getString(i));
        }
        return new Autofill(
            uniqueIdentifier, autofillHints, hintText, TextEditState.fromJson(editingState));
      }

      public final String uniqueIdentifier;
      public final String[] hints;
      public final TextEditState editState;
      public final String hintText;

      private static String translateAutofillHint(String hint) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
          return hint;
        }
        switch (hint) {
          case "addressCity":
            return "addressLocality";
          case "addressState":
            return "addressRegion";
          case "birthday":
            return "birthDateFull";
          case "birthdayDay":
            return "birthDateDay";
          case "birthdayMonth":
            return "birthDateMonth";
          case "birthdayYear":
            return "birthDateYear";
          case "countryName":
            return "addressCountry";
          case "creditCardExpirationDate":
            return View.AUTOFILL_HINT_CREDIT_CARD_EXPIRATION_DATE;
          case "creditCardExpirationDay":
            return View.AUTOFILL_HINT_CREDIT_CARD_EXPIRATION_DAY;
          case "creditCardExpirationMonth":
            return View.AUTOFILL_HINT_CREDIT_CARD_EXPIRATION_MONTH;
          case "creditCardExpirationYear":
            return View.AUTOFILL_HINT_CREDIT_CARD_EXPIRATION_YEAR;
          case "creditCardNumber":
            return View.AUTOFILL_HINT_CREDIT_CARD_NUMBER;
          case "creditCardSecurityCode":
            return View.AUTOFILL_HINT_CREDIT_CARD_SECURITY_CODE;
          case "email":
            return View.AUTOFILL_HINT_EMAIL_ADDRESS;
          case "emailOTPCode":
            return "emailOTPCode";
          case "familyName":
            return "personFamilyName";
          case "fullStreetAddress":
            return "streetAddress";
          case "gender":
            return "gender";
          case "givenName":
            return "personGivenName";
          case "middleInitial":
            return "personMiddleInitial";
          case "middleName":
            return "personMiddleName";
          case "name":
            return "personName";
          case "namePrefix":
            return "personNamePrefix";
          case "nameSuffix":
            return "personNameSuffix";
          case "newPassword":
            return "newPassword";
          case "newUsername":
            return "newUsername";
          case "oneTimeCode":
            return "smsOTPCode";
          case "password":
            return View.AUTOFILL_HINT_PASSWORD;
          case "postalAddress":
            return View.AUTOFILL_HINT_POSTAL_ADDRESS;
          case "postalAddressExtended":
            return "extendedAddress";
          case "postalAddressExtendedPostalCode":
            return "extendedPostalCode";
          case "postalCode":
            return View.AUTOFILL_HINT_POSTAL_CODE;
          case "telephoneNumber":
            return "phoneNumber";
          case "telephoneNumberCountryCode":
            return "phoneCountryCode";
          case "telephoneNumberDevice":
            return "phoneNumberDevice";
          case "telephoneNumberNational":
            return "phoneNational";
          case "username":
            return View.AUTOFILL_HINT_USERNAME;
          default:
            return hint;
        }
      }

      public Autofill(
          String uniqueIdentifier,
          String[] hints,
          String hintText,
          TextEditState editingState) {
        this.uniqueIdentifier = uniqueIdentifier;
        this.hints = hints;
        this.hintText = hintText;
        this.editState = editingState;
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
    public final Autofill autofill;
    public final Configuration[] fields;
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
        Autofill autofill,
        Configuration[] fields,
        Locale[] hintLocales) {
      this.obscureText = obscureText;
      this.autocorrect = autocorrect;
      this.enableSuggestions = enableSuggestions;
      this.enableIMEPersonalizedLearning = enableIMEPersonalizedLearning;
      this.textCapitalization = textCapitalization;
      this.inputType = inputType;
      this.inputAction = inputAction;
      this.actionLabel = actionLabel;
      this.autofill = autofill;
      this.fields = fields;
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
