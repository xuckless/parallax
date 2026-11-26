// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/test/io/flutter/plugin/editing/TextInputPluginTest.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package; io.flutter.Build.API_LEVELS → Build.VERSION_CODES. The port's
// TextInputPlugin takes (view, channel): no ScribeChannel or platform-view controllers. Its
// TextInputChannel takes an Activity id (0) in place of the DartExecutor, and Configuration has no
// enableDeltaModel, autofill, contentCommitMimeTypes or fields arguments. The channel's messages are
// ShadowParallaxSurfaceView's natives: what Flutter verified on DartExecutor.send is verified on
// mockTextInput (ShadowParallaxSurfaceView.TextInput). FlutterView + FlutterEngine + FlutterRenderer
// become a spied ParallaxSurfaceView whose nativeInsets reach a mocked
// ShadowParallaxSurfaceView.Renderer as ViewportMetrics (no engine to attach, so
// clearTextInputClient_restartsImmWhenIMEHidden counts one restart less). verifyInputConnection has
// no requestExistingInputState message. textEditingDelta_TestUpdateEditingValueWithDeltas-
// IsNotInvokedWhenDeltaModelDisabled keeps its updateEditingState checks (the port's only path)
// without the updateEditingStateWithDeltas ones. inputConnection_hintLocalesIsSetInEditorInfo runs
// on API 28 (was 24). editorInfoForNumberConfig has no autofill parameter.
// Dropped (21 of 53): textInputPlugin_RequestsReattachOnCreation (requestExistingInputState);
// setTextInputEditingState_doesNotInvokeUpdateEditingStateWithDeltas,
// textEditingDelta_TestUpdateEditingValueIsNotInvokedWhenDeltaModelEnabled and the three
// textEditingDelta_TestDeltaIsCreatedWhenComposingTextSetIs* cases (delta model);
// destroy_clearTextInputMethodHandler (destroy); inputConnection_setsStylusHandwritingAvailable and
// inputConnection_doesNotCallSetsStylusHandwritingAvailableWhenAPILevelUnsupported (stylus
// handwriting); sendAppPrivateCommand_dataIsEmpty and _hasData (private commands); pending the
// autofill port: the nine autofill_* cases, respondsToInputChannelMessages (requestAutofill,
// finishAutofillContext) and TestAfm. Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.Mockito.any;
import static org.mockito.Mockito.atLeast;
import static org.mockito.Mockito.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.spy;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.verifyNoMoreInteractions;
import static org.mockito.Mockito.when;

import android.annotation.TargetApi;
import android.app.Activity;
import android.content.Context;
import android.graphics.Insets;
import android.os.Build;
import android.os.Bundle;
import android.os.LocaleList;
import android.provider.Settings;
import android.text.InputType;
import android.text.Selection;
import android.util.SparseIntArray;
import android.view.KeyEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.inputmethod.CursorAnchorInfo;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.view.inputmethod.InputMethodSubtype;
import androidx.test.core.app.ActivityScenario;
import androidx.test.core.app.ApplicationProvider;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import io.github.xuckless.parallax.TextInputChannel.TextEditState;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.json.JSONException;
import org.json.JSONObject;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.mockito.ArgumentCaptor;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;
import org.robolectric.annotation.Config;
import org.robolectric.annotation.Implementation;
import org.robolectric.annotation.Implements;
import org.robolectric.shadow.api.Shadow;
import org.robolectric.shadows.ShadowBuild;
import org.robolectric.shadows.ShadowInputMethodManager;

// Parallax: TestAfm (autofill) is gone, ShadowParallaxSurfaceView added.
@Config(shadows = {TextInputPluginTest.TestImm.class, ShadowParallaxSurfaceView.class})
@RunWith(AndroidJUnit4.class)
public class TextInputPluginTest {
  private final Context ctx = ApplicationProvider.getApplicationContext();
  // Parallax: in place of the mocked FlutterJNI and FlutterLoader, the natives' recorder.
  @Mock ShadowParallaxSurfaceView.TextInput mockTextInput;

  @Before
  public void setUp() {
    MockitoAnnotations.openMocks(this);
    ShadowParallaxSurfaceView.textInput = mockTextInput;
  }

  // Parallax: verifyMethodCall and sendToBinaryMessageHandler are gone with the channel.
  // Parallax: textInputPlugin_RequestsReattachOnCreation is dropped (requestExistingInputState is dropped).

  @Test
  public void setTextInputEditingState_doesNotInvokeUpdateEditingState() {
    // Initialize a general TextInputPlugin.
    InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    TextInputChannel textInputChannel = spy(new TextInputChannel(0));
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            null,
            null,
            null,
            null));

    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("initial input from framework", 0, 0, -1, -1));
    assertEquals("initial input from framework", textInputPlugin.getEditable().toString());

    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    textInputPlugin.setTextInputEditingState(
        testView,
        new TextInputChannel.TextEditState("more update from the framework", 1, 2, -1, -1));

    assertEquals("more update from the framework", textInputPlugin.getEditable().toString());
    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());
  }

  @Test
  public void setTextInputEditingState_willNotThrowWithoutSetTextInputClient() {
    // Initialize a general TextInputPlugin.
    InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    TextInputChannel textInputChannel = spy(new TextInputChannel(0));
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);

    // Here's no textInputPlugin.setTextInputClient()
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("initial input from framework", 0, 0, -1, -1));
    assertEquals("initial input from framework", textInputPlugin.getEditable().toString());
  }

  // Parallax: setTextInputEditingState_doesNotInvokeUpdateEditingStateWithDeltas is dropped (delta model).

  @Test
  public void textEditingDelta_TestUpdateEditingValueWithDeltasIsNotInvokedWhenDeltaModelDisabled()
      throws NullPointerException {
    // Initialize a general TextInputPlugin.
    InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    EditorInfo outAttrs = new EditorInfo();
    outAttrs.inputType = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE;
    TextInputChannel textInputChannel = spy(new TextInputChannel(0));
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    CharSequence newText = "I do not fear computers. I fear the lack of them.";

    // Change InputTarget to FRAMEWORK_CLIENT.
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true, // Delta model is disabled.
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.TEXT, false, false, false),
            null,
            null,
            null));

    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));
    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());
    assertEquals(
        0,
        ((ListenableEditingState) textInputPlugin.getEditable())
            .extractBatchTextEditingDeltas()
            .size());

    InputConnection inputConnection =
        textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), outAttrs);

    inputConnection.beginBatchEdit();
    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());
    inputConnection.setComposingText(newText, newText.length());
    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());
    inputConnection.endBatchEdit();

    assertEquals(
        0,
        ((ListenableEditingState) textInputPlugin.getEditable())
            .extractBatchTextEditingDeltas()
            .size());

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnection.beginBatchEdit();

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnection.endBatchEdit();

    assertEquals(
        0,
        ((ListenableEditingState) textInputPlugin.getEditable())
            .extractBatchTextEditingDeltas()
            .size());

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnection.beginBatchEdit();

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    // Selection changes so this will trigger an update to the framework through
    // updateEditingStateWithDeltas after the batch edit has completed and notified all listeners
    // of the editing state.
    inputConnection.setSelection(3, 4);
    assertEquals(3, Selection.getSelectionStart(textInputPlugin.getEditable()));
    assertEquals(4, Selection.getSelectionEnd(textInputPlugin.getEditable()));

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnection.endBatchEdit();

    verify(textInputChannel, times(2))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());
  }

  // Parallax: textEditingDelta_TestUpdateEditingValueIsNotInvokedWhenDeltaModelEnabled is dropped (delta model).

  // Parallax: textEditingDelta_TestDeltaIsCreatedWhenComposingTextSetIsInserting is dropped (delta model).

  // Parallax: textEditingDelta_TestDeltaIsCreatedWhenComposingTextSetIsDeleting is dropped (delta model).

  // Parallax: textEditingDelta_TestDeltaIsCreatedWhenComposingTextSetIsReplacing is dropped (delta model).

  @Test
  public void inputConnectionAdaptor_RepeatFilter() throws NullPointerException {
    // Initialize a general TextInputPlugin.
    InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    EditorInfo outAttrs = new EditorInfo();
    outAttrs.inputType = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE;
    TextInputChannel textInputChannel = spy(new TextInputChannel(0));
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);

    // Change InputTarget to FRAMEWORK_CLIENT.
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.TEXT, false, false, false),
            null,
            null,
            null));

    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));
    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    InputConnectionAdaptor inputConnectionAdaptor =
        (InputConnectionAdaptor)
            textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), outAttrs);

    inputConnectionAdaptor.beginBatchEdit();
    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());
    inputConnectionAdaptor.setComposingText("I do not fear computers. I fear the lack of them.", 1);
    verify(textInputChannel, times(0))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());
    inputConnectionAdaptor.endBatchEdit();
    verify(textInputChannel, times(1))
        .updateEditingState(
            anyInt(),
            eq("I do not fear computers. I fear the lack of them."),
            eq(49),
            eq(49),
            eq(0),
            eq(49));

    inputConnectionAdaptor.beginBatchEdit();

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnectionAdaptor.endBatchEdit();

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnectionAdaptor.beginBatchEdit();

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnectionAdaptor.setSelection(3, 4);
    assertEquals(3, Selection.getSelectionStart(textInputPlugin.getEditable()));
    assertEquals(4, Selection.getSelectionEnd(textInputPlugin.getEditable()));

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    verify(textInputChannel, times(1))
        .updateEditingState(anyInt(), any(), anyInt(), anyInt(), anyInt(), anyInt());

    inputConnectionAdaptor.endBatchEdit();

    verify(textInputChannel, times(1))
        .updateEditingState(
            anyInt(),
            eq("I do not fear computers. I fear the lack of them."),
            eq(3),
            eq(4),
            eq(0),
            eq(49));
  }

  @Test
  public void setTextInputEditingState_doesNotRestartWhenTextIsIdentical() {
    // Initialize a general TextInputPlugin.
    InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            null,
            null,
            null,
            null));
    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));

    // Move the cursor.
    assertEquals(1, testImm.getRestartCount(testView));
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));

    // Verify that we haven't restarted the input.
    assertEquals(1, testImm.getRestartCount(testView));
  }

  @Test
  public void setTextInputEditingState_alwaysSetEditableWhenDifferent() {
    // Initialize a general TextInputPlugin.
    InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            null,
            null,
            null,
            null));
    // There's a pending restart since we initialized the text input client. Flush that now. With
    // changed text, we should
    // always set the Editable contents.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("hello", 0, 0, -1, -1));
    assertEquals(1, testImm.getRestartCount(testView));
    assertEquals("hello", textInputPlugin.getEditable().toString());

    // No pending restart, set Editable contents anyways.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("Shibuyawoo", 0, 0, -1, -1));
    assertEquals(1, testImm.getRestartCount(testView));
    assertEquals("Shibuyawoo", textInputPlugin.getEditable().toString());
  }

  // See https://github.com/flutter/flutter/issues/29341 and
  // https://github.com/flutter/flutter/issues/31512
  // All modern Samsung keyboards are affected including non-korean languages and thus
  // need the restart.
  // Update: many other keyboards need this too:
  // https://github.com/flutter/flutter/issues/78827
  @SuppressWarnings("deprecation") // InputMethodSubtype
  @Test
  public void setTextInputEditingState_restartsIMEOnlyWhenFrameworkChangesComposingRegion() {
    // Initialize a TextInputPlugin that needs to be always restarted.
    InputMethodSubtype inputMethodSubtype =
        new InputMethodSubtype(0, 0, /*locale=*/ "en", "", "", false, false);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.TEXT, false, false, false),
            null,
            null,
            null));
    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));
    assertEquals(1, testImm.getRestartCount(testView));
    InputConnection connection =
        textInputPlugin.createInputConnection(
            testView, mock(KeyboardManager.class), new EditorInfo());
    connection.setComposingText("POWERRRRR", 1);

    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("UNLIMITED POWERRRRR", 0, 0, 10, 19));
    // Does not restart since the composing text is not changed.
    assertEquals(1, testImm.getRestartCount(testView));

    connection.finishComposingText();
    // Does not restart since the composing text is committed by the IME.
    assertEquals(1, testImm.getRestartCount(testView));

    // Does not restart since the composing text is changed by the IME.
    connection.setComposingText("POWERRRRR", 1);
    assertEquals(1, testImm.getRestartCount(testView));

    // The framework tries to commit the composing region.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("POWERRRRR", 0, 0, -1, -1));

    // Verify that we've restarted the input.
    assertEquals(2, testImm.getRestartCount(testView));
  }

  @Test
  public void TextEditState_throwsOnInvalidStatesReceived() {
    // Index OOB:
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("", 0, -9, -1, -1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("", -9, 0, -1, -1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("", 0, 1, -1, -1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("", 1, 0, -1, -1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("Text", 0, 0, 1, 5));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("Text", 0, 0, 5, 1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("Text", 0, 0, 5, 5));

    // Invalid Selections:
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("", -1, -2, -1, -1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("", -2, -1, -1, -1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("", -9, -9, -1, -1));

    // Invalid Composing Ranges:
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("Text", 0, 0, -9, -1));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("Text", 0, 0, -1, -9));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("Text", 0, 0, -9, -9));
    assertThrows(IndexOutOfBoundsException.class, () -> new TextEditState("Text", 0, 0, 2, 1));

    // Valid values (does not throw):
    // Nothing selected/composing:
    TextEditState state = new TextEditState("", -1, -1, -1, -1);
    assertEquals("", state.text);
    assertEquals(-1, state.selectionStart);
    assertEquals(-1, state.selectionEnd);
    assertEquals(-1, state.composingStart);
    assertEquals(-1, state.composingEnd);
    // Collapsed selection.
    state = new TextEditState("x", 0, 0, 0, 1);
    assertEquals(0, state.selectionStart);
    assertEquals(0, state.selectionEnd);
    // Reversed Selection.
    state = new TextEditState("REEEE", 4, 2, -1, -1);
    assertEquals(4, state.selectionStart);
    assertEquals(2, state.selectionEnd);
    // A collapsed selection and composing range.
    state = new TextEditState("text", 0, 0, 0, 0);
    assertEquals("text", state.text);
    assertEquals(0, state.selectionStart);
    assertEquals(0, state.selectionEnd);
    assertEquals(0, state.composingStart);
    assertEquals(0, state.composingEnd);
  }

  @Test
  public void setTextInputEditingState_nullInputMethodSubtype() {
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(null);

    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            null,
            null,
            null,
            null));
    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));
    assertEquals(1, testImm.getRestartCount(testView));
  }

  @Test
  public void imeVisibilityListener_restartsImmWhenIMEHidden() {
    // Initialize a general TextInputPlugin.
    InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            null,
            null,
            null,
            null));
    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));
    assertEquals(1, testImm.getRestartCount(testView));

    // Imm restarts when IME is hidden.
    ImeSyncDeferringInsetsCallback imeSyncCallback = textInputPlugin.getImeSyncCallback();
    imeSyncCallback.getImeVisibilityListener().onImeVisibilityChanged(false);
    assertEquals(2, testImm.getRestartCount(testView));
  }

  @Test
  public void clearTextInputClient_restartsImmWhenIMEHidden() {
    try (ActivityScenario<Activity> scenario = ActivityScenario.launch(Activity.class)) {
      scenario.onActivity(
          activity -> {
            ParallaxSurfaceView testView = spy(new ParallaxSurfaceView(activity, 0));

            InputMethodSubtype inputMethodSubtype = mock(InputMethodSubtype.class);
            TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
            testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
            TextInputChannel textInputChannel = new TextInputChannel(0);
            TextInputPlugin textInputPlugin =
                new TextInputPlugin(
                    testView,
                    textInputChannel);
            textInputPlugin.setTextInputClient(
                0,
                new TextInputChannel.Configuration(
                    false,
                    false,
                    true,
                    true,
                    TextInputChannel.TextCapitalization.NONE,
                    null,
                    null,
                    null,
                    null));
            ImeSyncDeferringInsetsCallback imeSyncCallback = textInputPlugin.getImeSyncCallback();
            // Parallax: the view reports insets through the shadowed nativeInsets, no engine.
            ShadowParallaxSurfaceView.Renderer flutterRenderer =
                mock(ShadowParallaxSurfaceView.Renderer.class);
            ShadowParallaxSurfaceView.renderer = flutterRenderer;
            assertEquals(0, testImm.getRestartCount(testView));
            // Parallax: no engine to attach, so no restart for it; the counts below are one less
            // than Flutter's.

            ArgumentCaptor<ShadowParallaxSurfaceView.ViewportMetrics> viewportMetricsCaptor =
                ArgumentCaptor.forClass(ShadowParallaxSurfaceView.ViewportMetrics.class);

            WindowInsets.Builder builder = new WindowInsets.Builder();

            // There's a pending restart since we initialized the text input client. Flush that now.
            textInputPlugin.setTextInputEditingState(
                testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));
            assertEquals(1, testImm.getRestartCount(testView));

            // Set the initial insets and verify that they were set and the bottom view inset is
            // correct.
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 0));
            when(testView.getRootWindowInsets()).thenReturn(builder.build());
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());
            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Imm restarts when clearTextInputClient is called while the IME is hidden.
            textInputPlugin.clearTextInputClient();
            assertEquals(2, testImm.getRestartCount(testView));
          });
    }
  }

  // Parallax: destroy_clearTextInputMethodHandler is dropped (destroy is dropped).

  private void verifyInputConnection(TextInputChannel.TextInputType textInputType)
      throws JSONException {
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    View testView = new View(ctx);
    // Parallax: the channel's messages are the shadowed natives, recorded by mockTextInput.
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(textInputType, false, false, false),
            null,
            null,
            null));
    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("", 0, 0, -1, -1));

    // Parallax: Flutter's first message, TextInputClient.requestExistingInputState, is dropped.
    InputConnectionAdaptor connection =
        (InputConnectionAdaptor)
            textInputPlugin.createInputConnection(
                testView, mock(KeyboardManager.class), new EditorInfo());

    connection.handleKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER));
    verify(mockTextInput, times(1)).performAction("TextInputAction.done");
    connection.handleKeyEvent(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER));

    connection.handleKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_NUMPAD_ENTER));
    verify(mockTextInput, times(2)).performAction("TextInputAction.done");
    // Flutter counts every message sent; nothing else was.
    verifyNoMoreInteractions(mockTextInput);
  }

  @Test
  public void inputConnection_createsActionFromEnter() throws JSONException {
    verifyInputConnection(TextInputChannel.TextInputType.TEXT);
  }

  @Test
  public void inputConnection_respondsToKeyEvents_textInputTypeNone() throws JSONException {
    verifyInputConnection(TextInputChannel.TextInputType.NONE);
  }

  @SuppressWarnings("deprecation") // InputMethodSubtype
  @Test
  public void inputConnection_finishComposingTextUpdatesIMM() throws JSONException {
    ShadowBuild.setManufacturer("samsung");
    InputMethodSubtype inputMethodSubtype =
        new InputMethodSubtype(0, 0, /*locale=*/ "en", "", "", false, false);
    Settings.Secure.putString(
        ctx.getContentResolver(),
        Settings.Secure.DEFAULT_INPUT_METHOD,
        "com.sec.android.inputmethod/.SamsungKeypad");
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    testImm.setCurrentInputMethodSubtype(inputMethodSubtype);
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.TEXT, false, false, false),
            null,
            null,
            null));
    // There's a pending restart since we initialized the text input client. Flush that now.
    textInputPlugin.setTextInputEditingState(
        testView, new TextInputChannel.TextEditState("text", 0, 0, -1, -1));
    InputConnection connection =
        textInputPlugin.createInputConnection(
            testView, mock(KeyboardManager.class), new EditorInfo());

    connection.requestCursorUpdates(
        InputConnection.CURSOR_UPDATE_MONITOR | InputConnection.CURSOR_UPDATE_IMMEDIATE);

    connection.finishComposingText();

    assertEquals(-1, testImm.getLastCursorAnchorInfo().getComposingTextStart());
    assertEquals(0, testImm.getLastCursorAnchorInfo().getComposingText().length());
  }

  @Test
  public void inputConnection_textInputTypeNone() {
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.NONE, false, false, false),
            null,
            null,
            null));

    InputConnection connection =
        textInputPlugin.createInputConnection(
            testView, mock(KeyboardManager.class), new EditorInfo());
    assertNotNull(connection);
  }

  @Test
  public void showTextInput_textInputTypeNone() {
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.NONE, false, false, false),
            null,
            null,
            null));

    textInputPlugin.showTextInput(testView);
    assertFalse(testImm.isSoftInputVisible());
  }

  @Test
  public void showTextInput_textInputTypeWebSearch() {
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.WEB_SEARCH, false, false, false),
            null,
            null,
            null));

    EditorInfo editorInfo = new EditorInfo();
    InputConnection connection =
        textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), editorInfo);

    assertEquals(
        InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI, editorInfo.inputType);
  }

  @Test
  public void showTextInput_textInputTypeTwitter() {
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.TWITTER, false, false, false),
            null,
            null,
            null));

    EditorInfo editorInfo = new EditorInfo();
    InputConnection connection =
        textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), editorInfo);

    assertEquals(
        InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
        editorInfo.inputType);
  }

  @Test
  public void inputConnection_textInputTypeMultilineAndSuggestionsDisabled() {
    // Regression test for https://github.com/flutter/flutter/issues/71679.
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            false, // Disable suggestions.
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.MULTILINE, false, false, false),
            null,
            null,
            null));

    EditorInfo editorInfo = new EditorInfo();
    InputConnection connection =
        textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), editorInfo);

    assertEquals(
        InputType.TYPE_CLASS_TEXT
            | InputType.TYPE_TEXT_FLAG_MULTI_LINE
            | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            | InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
        editorInfo.inputType);
  }

  // Parallax: inputConnection_setsStylusHandwritingAvailable is dropped (stylus handwriting).

  // Parallax: inputConnection_doesNotCallSetsStylusHandwritingAvailableWhenAPILevelUnsupported is dropped (stylus handwriting).

  // Parallax: autofill_enabledByDefault is dropped (pending autofill port).

  // Parallax: autofill_canBeDisabled is dropped (pending autofill port).

  // Parallax: autofill_hintText is dropped (pending autofill port).

  // Parallax: autofill_onProvideVirtualViewStructure is dropped (pending autofill port).

  // Parallax: autofill_onProvideVirtualViewStructure_singular_textfield is dropped (pending autofill port).

  // Parallax: autofill_testLifeCycle is dropped (pending autofill port).

  // Parallax: autofill_testAutofillUpdatesTheFramework is dropped (pending autofill port).

  // Parallax: autofill_doesNotCrashAfterClearClientCall is dropped (pending autofill port).

  // Parallax: autofill_testSetTextInputClientUpdatesSideFields is dropped (pending autofill port).

  // -------- End: Autofill Tests -------

  // Parallax: respondsToInputChannelMessages is dropped (pending autofill port: requestAutofill, finishAutofillContext).

  // Parallax: sendAppPrivateCommand_dataIsEmpty is dropped (private commands).

  // Parallax: sendAppPrivateCommand_hasData is dropped (private commands).

  @Test
  @TargetApi(Build.VERSION_CODES.R)
  @Config(sdk = Build.VERSION_CODES.R)
  @SuppressWarnings("deprecation")
  // getWindowSystemUiVisibility, SYSTEM_UI_FLAG_LAYOUT_STABLE.
  // flutter#133074 tracks migration work.
  public void ime_windowInsetsSync_notLaidOutBehindNavigation_excludesNavigationBars() {
    try (ActivityScenario<Activity> scenario = ActivityScenario.launch(Activity.class)) {
      scenario.onActivity(
          activity -> {
            ParallaxSurfaceView testView = spy(new ParallaxSurfaceView(activity, 0));
            when(testView.getWindowSystemUiVisibility())
                .thenReturn(View.SYSTEM_UI_FLAG_LAYOUT_STABLE);

            TextInputChannel textInputChannel = new TextInputChannel(0);
            TextInputPlugin textInputPlugin =
                new TextInputPlugin(
                    testView,
                    textInputChannel);
            ImeSyncDeferringInsetsCallback imeSyncCallback = textInputPlugin.getImeSyncCallback();
            // Parallax: the view reports insets through the shadowed nativeInsets, no engine.
            ShadowParallaxSurfaceView.Renderer flutterRenderer =
                mock(ShadowParallaxSurfaceView.Renderer.class);
            ShadowParallaxSurfaceView.renderer = flutterRenderer;

            WindowInsetsAnimation animation = mock(WindowInsetsAnimation.class);
            when(animation.getTypeMask()).thenReturn(WindowInsets.Type.ime());

            List<WindowInsetsAnimation> animationList = new ArrayList();
            animationList.add(animation);

            ArgumentCaptor<ShadowParallaxSurfaceView.ViewportMetrics> viewportMetricsCaptor =
                ArgumentCaptor.forClass(ShadowParallaxSurfaceView.ViewportMetrics.class);

            WindowInsets.Builder builder = new WindowInsets.Builder();

            // Set the initial insets and verify that they were set and the bottom view inset is
            // correct
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Call onPrepare and set the lastWindowInsets - these should be stored for the end of
            // the
            // animation instead of being applied immediately
            imeSyncCallback.getAnimationCallback().onPrepare(animation);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 100));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 0));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Call onStart and apply new insets - these should be ignored completely
            imeSyncCallback.getAnimationCallback().onStart(animation, null);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 50));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Progress the animation and ensure that the navigation bar insets have been subtracted
            // from the IME insets
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 25));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 50));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(10, viewportMetricsCaptor.getValue().viewInsetBottom);

            // End the animation and ensure that the bottom insets match the lastWindowInsets that
            // we set
            // during onPrepare
            imeSyncCallback.getAnimationCallback().onEnd(animation);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(100, viewportMetricsCaptor.getValue().viewInsetBottom);
          });
    }
  }

  @Test
  @TargetApi(Build.VERSION_CODES.VANILLA_ICE_CREAM)
  @Config(minSdk = Build.VERSION_CODES.VANILLA_ICE_CREAM)
  @SuppressWarnings("deprecation")
  // getWindowSystemUiVisibility, SYSTEM_UI_FLAG_LAYOUT_STABLE.
  // flutter#133074 tracks migration work.
  public void ime_windowInsetsSync_notLaidOutBehindNavigation_post15_includesNavigationBars() {
    try (ActivityScenario<Activity> scenario = ActivityScenario.launch(Activity.class)) {
      scenario.onActivity(
          activity -> {
            ParallaxSurfaceView testView = spy(new ParallaxSurfaceView(activity, 0));
            when(testView.getWindowSystemUiVisibility())
                .thenReturn(View.SYSTEM_UI_FLAG_LAYOUT_STABLE);

            TextInputChannel textInputChannel = new TextInputChannel(0);
            TextInputPlugin textInputPlugin =
                new TextInputPlugin(
                    testView,
                    textInputChannel);
            ImeSyncDeferringInsetsCallback imeSyncCallback = textInputPlugin.getImeSyncCallback();
            // Parallax: the view reports insets through the shadowed nativeInsets, no engine.
            ShadowParallaxSurfaceView.Renderer flutterRenderer =
                mock(ShadowParallaxSurfaceView.Renderer.class);
            ShadowParallaxSurfaceView.renderer = flutterRenderer;

            WindowInsetsAnimation animation = mock(WindowInsetsAnimation.class);
            when(animation.getTypeMask()).thenReturn(WindowInsets.Type.ime());

            List<WindowInsetsAnimation> animationList = new ArrayList();
            animationList.add(animation);

            ArgumentCaptor<ShadowParallaxSurfaceView.ViewportMetrics> viewportMetricsCaptor =
                ArgumentCaptor.forClass(ShadowParallaxSurfaceView.ViewportMetrics.class);

            WindowInsets.Builder builder = new WindowInsets.Builder();

            // Set the initial insets and verify that they were set and the bottom view inset is
            // correct
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Call onPrepare and set the lastWindowInsets - these should be stored for the end of
            // the
            // animation instead of being applied immediately
            imeSyncCallback.getAnimationCallback().onPrepare(animation);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 100));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 0));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Call onStart and apply new insets - these should be ignored completely
            imeSyncCallback.getAnimationCallback().onStart(animation, null);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 50));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Progress the animation and ensure that the navigation bar insets have not been
            // subtracted from the IME insets
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 25));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(25, viewportMetricsCaptor.getValue().viewInsetBottom);

            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 50));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(50, viewportMetricsCaptor.getValue().viewInsetBottom);

            // End the animation and ensure that the bottom insets match the lastWindowInsets that
            // we set
            // during onPrepare
            imeSyncCallback.getAnimationCallback().onEnd(animation);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(100, viewportMetricsCaptor.getValue().viewInsetBottom);
          });
    }
  }

  @Test
  @TargetApi(Build.VERSION_CODES.R)
  @Config(sdk = Build.VERSION_CODES.R)
  @SuppressWarnings("deprecation")
  // getWindowSystemUiVisibility
  // flutter#133074 tracks migration work.
  public void ime_windowInsetsSync_laidOutBehindNavigation_includesNavigationBars() {
    try (ActivityScenario<Activity> scenario = ActivityScenario.launch(Activity.class)) {
      scenario.onActivity(
          activity -> {
            ParallaxSurfaceView testView = spy(new ParallaxSurfaceView(activity, 0));
            when(testView.getWindowSystemUiVisibility())
                .thenReturn(
                    View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION);

            TextInputChannel textInputChannel = new TextInputChannel(0);
            TextInputPlugin textInputPlugin =
                new TextInputPlugin(
                    testView,
                    textInputChannel);
            ImeSyncDeferringInsetsCallback imeSyncCallback = textInputPlugin.getImeSyncCallback();
            // Parallax: the view reports insets through the shadowed nativeInsets, no engine.
            ShadowParallaxSurfaceView.Renderer flutterRenderer =
                mock(ShadowParallaxSurfaceView.Renderer.class);
            ShadowParallaxSurfaceView.renderer = flutterRenderer;

            WindowInsetsAnimation animation = mock(WindowInsetsAnimation.class);
            when(animation.getTypeMask()).thenReturn(WindowInsets.Type.ime());

            List<WindowInsetsAnimation> animationList = new ArrayList();
            animationList.add(animation);

            ArgumentCaptor<ShadowParallaxSurfaceView.ViewportMetrics> viewportMetricsCaptor =
                ArgumentCaptor.forClass(ShadowParallaxSurfaceView.ViewportMetrics.class);

            WindowInsets.Builder builder = new WindowInsets.Builder();

            // Set the initial insets and verify that they were set and the bottom view inset is
            // correct
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Call onPrepare and set the lastWindowInsets - these should be stored for the end of
            // the
            // animation instead of being applied immediately
            imeSyncCallback.getAnimationCallback().onPrepare(animation);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 100));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 0));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Call onStart and apply new insets - these should be ignored completely
            imeSyncCallback.getAnimationCallback().onStart(animation, null);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 50));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewInsetBottom);

            // Progress the animation and ensure that the navigation bar insets have not been
            // subtracted from the IME insets
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 25));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(25, viewportMetricsCaptor.getValue().viewInsetBottom);

            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 50));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 40));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(50, viewportMetricsCaptor.getValue().viewInsetBottom);

            // End the animation and ensure that the bottom insets match the lastWindowInsets that
            // we set
            // during onPrepare
            imeSyncCallback.getAnimationCallback().onEnd(animation);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(100, viewportMetricsCaptor.getValue().viewInsetBottom);
          });
    }
  }

  @Test
  @TargetApi(Build.VERSION_CODES.R)
  @Config(sdk = Build.VERSION_CODES.R)
  @SuppressWarnings("deprecation")
  // getWindowSystemUiVisibility, SYSTEM_UI_FLAG_LAYOUT_STABLE
  // flutter#133074 tracks migration work.
  public void lastWindowInsets_updatedOnSecondOnProgressCall() {
    try (ActivityScenario<Activity> scenario = ActivityScenario.launch(Activity.class)) {
      scenario.onActivity(
          activity -> {
            ParallaxSurfaceView testView = spy(new ParallaxSurfaceView(activity, 0));
            when(testView.getWindowSystemUiVisibility())
                .thenReturn(View.SYSTEM_UI_FLAG_LAYOUT_STABLE);

            TextInputChannel textInputChannel = new TextInputChannel(0);
            TextInputPlugin textInputPlugin =
                new TextInputPlugin(
                    testView,
                    textInputChannel);
            ImeSyncDeferringInsetsCallback imeSyncCallback = textInputPlugin.getImeSyncCallback();
            // Parallax: the view reports insets through the shadowed nativeInsets, no engine.
            ShadowParallaxSurfaceView.Renderer flutterRenderer =
                mock(ShadowParallaxSurfaceView.Renderer.class);
            ShadowParallaxSurfaceView.renderer = flutterRenderer;

            WindowInsetsAnimation imeAnimation = mock(WindowInsetsAnimation.class);
            when(imeAnimation.getTypeMask()).thenReturn(WindowInsets.Type.ime());
            WindowInsetsAnimation navigationBarAnimation = mock(WindowInsetsAnimation.class);
            when(navigationBarAnimation.getTypeMask())
                .thenReturn(WindowInsets.Type.navigationBars());

            List<WindowInsetsAnimation> animationList = new ArrayList();
            animationList.add(imeAnimation);
            animationList.add(navigationBarAnimation);

            ArgumentCaptor<ShadowParallaxSurfaceView.ViewportMetrics> viewportMetricsCaptor =
                ArgumentCaptor.forClass(ShadowParallaxSurfaceView.ViewportMetrics.class);

            WindowInsets.Builder builder = new WindowInsets.Builder();

            // Set the initial insets and verify that they were set and the bottom view padding is
            // correct
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 1000));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 100));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(100, viewportMetricsCaptor.getValue().viewPaddingBottom);

            // Call onPrepare and set the lastWindowInsets - these should be stored for the end of
            // the
            // animation instead of being applied immediately
            imeSyncCallback.getAnimationCallback().onPrepare(imeAnimation);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 0));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 100));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(100, viewportMetricsCaptor.getValue().viewPaddingBottom);

            // Call onPrepare again and apply new insets - these should overrite lastWindowInsets
            imeSyncCallback.getAnimationCallback().onPrepare(navigationBarAnimation);
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 0));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 0));
            imeSyncCallback.getInsetsListener().onApplyWindowInsets(testView, builder.build());

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(100, viewportMetricsCaptor.getValue().viewPaddingBottom);

            // Progress the animation and ensure that the navigation bar insets have not been
            // subtracted from the IME insets
            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 500));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 0));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewPaddingBottom);

            builder.setInsets(WindowInsets.Type.ime(), Insets.of(0, 0, 0, 250));
            builder.setInsets(WindowInsets.Type.navigationBars(), Insets.of(0, 0, 0, 0));
            imeSyncCallback.getAnimationCallback().onProgress(builder.build(), animationList);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewPaddingBottom);

            // End the animation and ensure that the bottom insets match the lastWindowInsets that
            // we set
            // during onPrepare
            imeSyncCallback.getAnimationCallback().onEnd(imeAnimation);

            verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());
            assertEquals(0, viewportMetricsCaptor.getValue().viewPaddingBottom);
          });
    }
  }

  @Test
  // Parallax: pinned to API 28 (was 24, when hintLocales arrived), Parallax's minimum.
  @TargetApi(Build.VERSION_CODES.P)
  @Config(sdk = Build.VERSION_CODES.P)
  public void inputConnection_hintLocalesIsSetInEditorInfo() {
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    final Locale[] hintLocales = {new Locale.Builder().setLanguage("en").build()};
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            false,
            false,
            true,
            true,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.MULTILINE, false, false, false),
            null,
            null,
            hintLocales));

    EditorInfo editorInfo = new EditorInfo();
    InputConnection connection =
        textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), editorInfo);

    assertEquals(editorInfo.hintLocales, new LocaleList(hintLocales));
  }

  /**
   * Helper that constructs a minimal {@link TextInputChannel.Configuration} with the given number
   * variation flags and captures the resulting {@link EditorInfo} from {@link
   * TextInputPlugin#createInputConnection}.
   */
  private EditorInfo editorInfoForNumberConfig(
      boolean isSigned,
      boolean isDecimal,
      boolean isPassword,
      boolean obscureText,
      boolean enableIMEPersonalizedLearning) {
    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(
        0,
        new TextInputChannel.Configuration(
            obscureText,
            false,
            true,
            enableIMEPersonalizedLearning,
            TextInputChannel.TextCapitalization.NONE,
            new TextInputChannel.InputType(
                TextInputChannel.TextInputType.NUMBER, isSigned, isDecimal, isPassword),
            null,
            null,
            null));

    EditorInfo editorInfo = new EditorInfo();
    textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), editorInfo);
    return editorInfo;
  }

  @Test
  public void inputType_number() {
    EditorInfo editorInfo = editorInfoForNumberConfig(false, false, false, false, true);
    assertEquals(InputType.TYPE_CLASS_NUMBER, editorInfo.inputType);
  }

  @Test
  public void inputType_numberWithPassword() {
    EditorInfo editorInfo = editorInfoForNumberConfig(false, false, true, false, true);
    assertEquals(
        InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD,
        editorInfo.inputType);
  }

  @Test
  public void inputType_numberWithSigned() {
    EditorInfo editorInfo = editorInfoForNumberConfig(true, false, false, false, true);
    assertEquals(
        InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_SIGNED, editorInfo.inputType);
  }

  @Test
  public void inputType_numberWithDecimal() {
    EditorInfo editorInfo = editorInfoForNumberConfig(false, true, false, false, true);
    assertEquals(
        InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_FLAG_DECIMAL, editorInfo.inputType);
  }

  @Test
  public void inputType_numberWithSignedAndPassword() {
    EditorInfo editorInfo = editorInfoForNumberConfig(true, false, true, false, true);
    assertEquals(
        InputType.TYPE_CLASS_NUMBER
            | InputType.TYPE_NUMBER_FLAG_SIGNED
            | InputType.TYPE_NUMBER_VARIATION_PASSWORD,
        editorInfo.inputType);
  }

  @Test
  public void inputType_numberWithDecimalAndPassword() {
    EditorInfo editorInfo = editorInfoForNumberConfig(false, true, true, false, true);
    assertEquals(
        InputType.TYPE_CLASS_NUMBER
            | InputType.TYPE_NUMBER_FLAG_DECIMAL
            | InputType.TYPE_NUMBER_VARIATION_PASSWORD,
        editorInfo.inputType);
  }

  @Test
  public void inputType_numberObscureTextDoesNotSetPasswordVariation() {
    // obscureText is orthogonal to the numeric password variation; it must not implicitly set
    // TYPE_NUMBER_VARIATION_PASSWORD on a NUMBER field. This test is load-bearing because the
    // obscureText branch lives in the same inputTypeFromTextInputType() function as the NUMBER
    // branch — the two are the most plausible place for accidental coupling.
    EditorInfo editorInfo = editorInfoForNumberConfig(false, false, false, true, true);
    assertEquals(InputType.TYPE_CLASS_NUMBER, editorInfo.inputType);
    assertEquals(0, editorInfo.inputType & InputType.TYPE_NUMBER_VARIATION_PASSWORD);
  }

  @Test
  public void inputType_numberWithPasswordJsonRoundTrip()
      throws JSONException, NoSuchFieldException {
    // End-to-end coverage from JSON through Configuration.fromJson -> InputType.fromJson ->
    // setTextInputClient -> createInputConnection. Catches typos in the "password" JSON key,
    // gaps in Configuration.fromJson's recursive inputType handling, and any break in the
    // JSON -> field -> plugin chain that the matrix tests bypass by calling new InputType(...)
    // directly.
    final JSONObject inputType = new JSONObject();
    inputType.put("name", "TextInputType.number");
    inputType.put("password", true);

    final JSONObject arguments = new JSONObject();
    arguments.put("inputAction", "TextInputAction.done");
    arguments.put("textCapitalization", "TextCapitalization.none");
    arguments.put("inputType", inputType);

    final TextInputChannel.Configuration configuration =
        TextInputChannel.Configuration.fromJson(arguments);

    View testView = new View(ctx);
    TextInputChannel textInputChannel = new TextInputChannel(0);
    TextInputPlugin textInputPlugin =
        new TextInputPlugin(
            testView,
            textInputChannel);
    textInputPlugin.setTextInputClient(0, configuration);

    EditorInfo editorInfo = new EditorInfo();
    textInputPlugin.createInputConnection(testView, mock(KeyboardManager.class), editorInfo);

    assertEquals(
        InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD,
        editorInfo.inputType);
  }

  interface EventHandler {
    void sendAppPrivateCommand(View view, String action, Bundle data);
  }

  @Implements(InputMethodManager.class)
  public static class TestImm extends ShadowInputMethodManager {
    private InputMethodSubtype currentInputMethodSubtype;
    private SparseIntArray restartCounter = new SparseIntArray();
    private CursorAnchorInfo cursorAnchorInfo;
    private ArrayList<Integer> selectionUpdateValues;
    private boolean trackSelection = false;
    private EventHandler handler;

    public TestImm() {
      selectionUpdateValues = new ArrayList<Integer>();
    }

    @Implementation
    public InputMethodSubtype getCurrentInputMethodSubtype() {
      return currentInputMethodSubtype;
    }

    @Implementation
    public void restartInput(View view) {
      int count = restartCounter.get(view.hashCode(), /*defaultValue=*/ 0) + 1;
      restartCounter.put(view.hashCode(), count);
    }

    public void setCurrentInputMethodSubtype(InputMethodSubtype inputMethodSubtype) {
      this.currentInputMethodSubtype = inputMethodSubtype;
    }

    public int getRestartCount(View view) {
      return restartCounter.get(view.hashCode(), /*defaultValue=*/ 0);
    }

    public void setEventHandler(EventHandler eventHandler) {
      handler = eventHandler;
    }

    @Implementation
    public void sendAppPrivateCommand(View view, String action, Bundle data) {
      handler.sendAppPrivateCommand(view, action, data);
    }

    @Implementation
    public void updateCursorAnchorInfo(View view, CursorAnchorInfo cursorAnchorInfo) {
      this.cursorAnchorInfo = cursorAnchorInfo;
    }

    // We simply store the values to verify later.
    @Implementation
    public void updateSelection(
        View view, int selStart, int selEnd, int candidatesStart, int candidatesEnd) {
      if (trackSelection) {
        this.selectionUpdateValues.add(selStart);
        this.selectionUpdateValues.add(selEnd);
        this.selectionUpdateValues.add(candidatesStart);
        this.selectionUpdateValues.add(candidatesEnd);
      }
    }

    // only track values when enabled via this.
    public void setTrackSelection(boolean val) {
      trackSelection = val;
    }

    // Returns true if the last updateSelection call passed the following values.
    public ArrayList<Integer> getSelectionUpdateValues() {
      return selectionUpdateValues;
    }

    public CursorAnchorInfo getLastCursorAnchorInfo() {
      return cursorAnchorInfo;
    }
  }

  // Parallax: TestAfm is dropped (pending autofill port).
}
