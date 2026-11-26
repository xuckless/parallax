// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/test/io/flutter/plugin/editing/InputConnectionAdaptorTest.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package; FakeKeyEvent is the ported test util in this package. The port's
// InputConnectionAdaptor has no ScribeChannel argument (Scribe is dropped), so those arguments are
// gone; the mocked FlutterJNI is a mocked FlutterTextUtils.CodePoints (the port's stand-in for the
// engine's ICU queries, same method names), answered by ICU4J as in Flutter; TextInputChannel
// takes an Activity id (0) in place of the DartExecutor. Dropped: testCommitContent and the nine
// testPerformPrivateCommand_* cases (commitContent and performPrivateCommand are dropped from the
// port, with their framework calls), and verifyMethodCall (used by those only).
// Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.Mockito.anyString;
import static org.mockito.Mockito.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.spy;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import android.content.ClipboardManager;
import android.content.ContentResolver;
import android.content.Context;
import android.text.InputType;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.CursorAnchorInfo;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import androidx.test.core.app.ApplicationProvider;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import com.ibm.icu.lang.UCharacter;
import com.ibm.icu.lang.UProperty;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.mockito.Mock;
import org.mockito.MockitoAnnotations;
import org.robolectric.annotation.Config;
import org.robolectric.annotation.Implementation;
import org.robolectric.annotation.Implements;
import org.robolectric.shadow.api.Shadow;
import org.robolectric.shadows.ShadowContentResolver;
import org.robolectric.shadows.ShadowInputMethodManager;

@Config(shadows = {InputConnectionAdaptorTest.TestImm.class})
@RunWith(AndroidJUnit4.class)
public class InputConnectionAdaptorTest {
  private final Context ctx = ApplicationProvider.getApplicationContext();
  private ContentResolver contentResolver;
  private ShadowContentResolver shadowContentResolver;

  @Mock KeyboardManager mockKeyboardManager;

  @Before
  public void setUp() {
    MockitoAnnotations.openMocks(this);
    contentResolver = ctx.getContentResolver();
    shadowContentResolver = Shadow.extract(contentResolver);
  }

  @Test
  public void inputConnectionAdaptor_ReceivesEnter() throws NullPointerException {
    View testView = new View(ctx);
    int inputTargetId = 0;
    // Parallax: the channel takes an Activity id in place of the DartExecutor.
    TextInputChannel textInputChannel = new TextInputChannel(0);
    ListenableEditingState mEditable = new ListenableEditingState(null, testView);
    Selection.setSelection(mEditable, 0, 0);
    ListenableEditingState spyEditable = spy(mEditable);
    EditorInfo outAttrs = new EditorInfo();
    outAttrs.inputType = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE;

    InputConnectionAdaptor inputConnectionAdaptor =
        new InputConnectionAdaptor(
            testView,
            inputTargetId,
            textInputChannel,
            mockKeyboardManager,
            spyEditable,
            outAttrs);

    // Send an enter key and make sure the Editable received it.
    FakeKeyEvent keyEvent = new FakeKeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER, '\n');
    inputConnectionAdaptor.handleKeyEvent(keyEvent);
    verify(spyEditable, times(1)).insert(eq(0), anyString());
  }

  @Test
  public void testPerformContextMenuAction_selectAll() {
    int selStart = 5;
    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    boolean didConsume = adaptor.performContextMenuAction(android.R.id.selectAll);

    assertTrue(didConsume);
    assertEquals(0, Selection.getSelectionStart(editable));
    assertEquals(editable.length(), Selection.getSelectionEnd(editable));
  }

  @SuppressWarnings("deprecation")
  // ClipboardManager.hasText is deprecated.
  @Test
  public void testPerformContextMenuAction_cut() {
    ClipboardManager clipboardManager = ctx.getSystemService(ClipboardManager.class);
    int selStart = 6;
    int selEnd = 11;
    ListenableEditingState editable = sampleEditable(selStart, selEnd);
    CharSequence textToBeCut = editable.subSequence(selStart, selEnd);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    boolean didConsume = adaptor.performContextMenuAction(android.R.id.cut);

    assertTrue(didConsume);
    assertTrue(clipboardManager.hasText());
    assertEquals(textToBeCut, clipboardManager.getPrimaryClip().getItemAt(0).getText());
    assertFalse(editable.toString().contains(textToBeCut));
  }

  @SuppressWarnings("deprecation")
  // ClipboardManager.hasText is deprecated.
  @Test
  public void testPerformContextMenuAction_copy() {
    ClipboardManager clipboardManager = ctx.getSystemService(ClipboardManager.class);
    int selStart = 6;
    int selEnd = 11;
    ListenableEditingState editable = sampleEditable(selStart, selEnd);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    assertFalse(clipboardManager.hasText());

    boolean didConsume = adaptor.performContextMenuAction(android.R.id.copy);

    assertTrue(didConsume);
    assertTrue(clipboardManager.hasText());
    assertEquals(
        editable.subSequence(selStart, selEnd),
        clipboardManager.getPrimaryClip().getItemAt(0).getText());
  }

  @SuppressWarnings("deprecation")
  // ClipboardManager.setText is deprecated.
  @Test
  public void testPerformContextMenuAction_paste() {
    ClipboardManager clipboardManager = ctx.getSystemService(ClipboardManager.class);
    String textToBePasted = "deadbeef";
    clipboardManager.setText(textToBePasted);
    ListenableEditingState editable = sampleEditable(0, 0);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    boolean didConsume = adaptor.performContextMenuAction(android.R.id.paste);

    assertTrue(didConsume);
    assertTrue(editable.toString().startsWith(textToBePasted));
  }

  // Parallax: testCommitContent and the nine testPerformPrivateCommand_* cases are dropped
  // (commitContent and performPrivateCommand are dropped from the port).

  @Test
  public void testSendKeyEvent_shiftKeyUpDoesNotCancelSelection() {
    // Regression test for https://github.com/flutter/flutter/issues/101569.
    int selStart = 5;
    int selEnd = 10;
    ListenableEditingState editable = sampleEditable(selStart, selEnd);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent shiftKeyUp = new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_SHIFT_LEFT);
    boolean didConsume = adaptor.handleKeyEvent(shiftKeyUp);

    assertFalse(didConsume);
    assertEquals(selStart, Selection.getSelectionStart(editable));
    assertEquals(selEnd, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testSendKeyEvent_leftKeyMovesCaretLeft() {
    int selStart = 5;
    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent leftKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_LEFT);
    boolean didConsume = adaptor.handleKeyEvent(leftKeyDown);

    assertTrue(didConsume);
    assertEquals(selStart - 1, Selection.getSelectionStart(editable));
    assertEquals(selStart - 1, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testSendKeyEvent_leftKeyMovesCaretLeftComplexEmoji() {
    int selStart = 75;
    ListenableEditingState editable = sampleEditable(selStart, selStart, SAMPLE_EMOJI_TEXT);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent downKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_LEFT);
    boolean didConsume;

    // Normal Character
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(74, Selection.getSelectionStart(editable));

    // Non-Spacing Mark
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(73, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(72, Selection.getSelectionStart(editable));

    // Keycap
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(69, Selection.getSelectionStart(editable));

    // Keycap with invalid base
    adaptor.setSelection(68, 68);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(66, Selection.getSelectionStart(editable));
    adaptor.setSelection(67, 67);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(66, Selection.getSelectionStart(editable));

    // Zero Width Joiner
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(55, Selection.getSelectionStart(editable));

    // Zero Width Joiner with invalid base
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(53, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(52, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(51, Selection.getSelectionStart(editable));

    // ----- Start Emoji Tag Sequence with invalid base testing ----
    // Delete base tag
    adaptor.setSelection(39, 39);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(37, Selection.getSelectionStart(editable));

    // Delete the sequence
    adaptor.setSelection(49, 49);
    for (int i = 0; i < 6; i++) {
      didConsume = adaptor.handleKeyEvent(downKeyDown);
      assertTrue(didConsume);
    }
    assertEquals(37, Selection.getSelectionStart(editable));
    // ----- End Emoji Tag Sequence with invalid base testing ----

    // Emoji Tag Sequence
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(23, Selection.getSelectionStart(editable));

    // Variation Selector with invalid base
    adaptor.setSelection(22, 22);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(21, Selection.getSelectionStart(editable));
    adaptor.setSelection(22, 22);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(21, Selection.getSelectionStart(editable));

    // Variation Selector
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(19, Selection.getSelectionStart(editable));

    // Emoji Modifier
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(16, Selection.getSelectionStart(editable));

    // Emoji Modifier with invalid base
    adaptor.setSelection(14, 14);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(13, Selection.getSelectionStart(editable));
    adaptor.setSelection(14, 14);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(13, Selection.getSelectionStart(editable));

    // Line Feed
    adaptor.setSelection(12, 12);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(11, Selection.getSelectionStart(editable));

    // Carriage Return
    adaptor.setSelection(12, 12);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(11, Selection.getSelectionStart(editable));

    // Carriage Return and Line Feed
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(9, Selection.getSelectionStart(editable));

    // Regional Indicator Symbol odd
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(7, Selection.getSelectionStart(editable));

    // Regional Indicator Symbol even
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(3, Selection.getSelectionStart(editable));

    // Simple Emoji
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(1, Selection.getSelectionStart(editable));

    // First CodePoint
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(0, Selection.getSelectionStart(editable));
  }

  @Test
  public void testSendKeyEvent_leftKeyExtendsSelectionLeft() {
    int selStart = 5;
    int selEnd = 40;
    ListenableEditingState editable = sampleEditable(selStart, selEnd);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent leftKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_LEFT);
    boolean didConsume = adaptor.handleKeyEvent(leftKeyDown);

    assertTrue(didConsume);
    assertEquals(selStart, Selection.getSelectionStart(editable));
    assertEquals(selEnd - 1, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testSendKeyEvent_shiftLeftKeyStartsSelectionLeft() {
    int selStart = 5;
    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent shiftLeftKeyDown =
        new KeyEvent(
            0, 0, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_LEFT, 0, KeyEvent.META_SHIFT_ON);
    boolean didConsume = adaptor.handleKeyEvent(shiftLeftKeyDown);

    assertTrue(didConsume);
    assertEquals(selStart, Selection.getSelectionStart(editable));
    assertEquals(selStart - 1, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testSendKeyEvent_rightKeyMovesCaretRight() {
    int selStart = 5;
    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent rightKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_RIGHT);
    boolean didConsume = adaptor.handleKeyEvent(rightKeyDown);

    assertTrue(didConsume);
    assertEquals(selStart + 1, Selection.getSelectionStart(editable));
    assertEquals(selStart + 1, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testSendKeyEvent_rightKeyMovesCaretRightComplexRegion() {
    int selStart = 0;
    // Seven region indicator characters. The first six should be considered as
    // three region indicators, and the final seventh character should be
    // considered to be on its own because it has no partner.
    String SAMPLE_REGION_TEXT = "🇷🇷🇷🇷🇷🇷🇷";
    ListenableEditingState editable = sampleEditable(selStart, selStart, SAMPLE_REGION_TEXT);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent downKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_RIGHT);
    boolean didConsume;

    // The cursor moves over two region indicators at a time.
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(4, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(8, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(12, Selection.getSelectionStart(editable));

    // When there is only one region indicator left with no pair, the cursor
    // moves over that single region indicator.
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(14, Selection.getSelectionStart(editable));

    // If the cursor is placed in the middle of a region indicator pair, it
    // moves over only the second half of the pair.
    adaptor.setSelection(6, 6);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(8, Selection.getSelectionStart(editable));
  }

  @Test
  public void testSendKeyEvent_rightKeyMovesCaretRightComplexEmoji() {
    int selStart = 0;
    ListenableEditingState editable = sampleEditable(selStart, selStart, SAMPLE_EMOJI_TEXT);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent downKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_RIGHT);
    boolean didConsume;

    // First CodePoint
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(1, Selection.getSelectionStart(editable));

    // Simple Emoji
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(3, Selection.getSelectionStart(editable));

    // Regional Indicator Symbol even
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(7, Selection.getSelectionStart(editable));

    // Regional Indicator Symbol odd
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(9, Selection.getSelectionStart(editable));

    // Carriage Return
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(10, Selection.getSelectionStart(editable));

    // Line Feed and Carriage Return
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(12, Selection.getSelectionStart(editable));

    // Line Feed
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(13, Selection.getSelectionStart(editable));

    // Modified Emoji
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(16, Selection.getSelectionStart(editable));

    // Emoji Modifier
    adaptor.setSelection(14, 14);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(16, Selection.getSelectionStart(editable));

    // Emoji Modifier with invalid base
    adaptor.setSelection(18, 18);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(19, Selection.getSelectionStart(editable));

    // Variation Selector
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(21, Selection.getSelectionStart(editable));

    // Variation Selector with invalid base
    adaptor.setSelection(22, 22);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(23, Selection.getSelectionStart(editable));

    // Emoji Tag Sequence
    for (int i = 0; i < 7; i++) {
      didConsume = adaptor.handleKeyEvent(downKeyDown);
      assertTrue(didConsume);
      assertEquals(Selection.getSelectionStart(editable), 25 + 2 * i);
    }
    assertEquals(37, Selection.getSelectionStart(editable));

    // ----- Start Emoji Tag Sequence with invalid base testing ----
    // Pass the sequence
    adaptor.setSelection(39, 39);
    for (int i = 0; i < 6; i++) {
      didConsume = adaptor.handleKeyEvent(downKeyDown);
      assertTrue(didConsume);
      assertEquals(Selection.getSelectionStart(editable), 41 + 2 * i);
    }
    assertEquals(51, Selection.getSelectionStart(editable));
    // ----- End Emoji Tag Sequence with invalid base testing ----

    // Zero Width Joiner with invalid base
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(52, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(53, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(55, Selection.getSelectionStart(editable));

    // Zero Width Joiner
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(66, Selection.getSelectionStart(editable));

    // Keycap with invalid base
    adaptor.setSelection(67, 67);
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(68, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(69, Selection.getSelectionStart(editable));

    // Keycap
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(72, Selection.getSelectionStart(editable));

    // Non-Spacing Mark
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(73, Selection.getSelectionStart(editable));
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(74, Selection.getSelectionStart(editable));

    // Normal Character
    didConsume = adaptor.handleKeyEvent(downKeyDown);
    assertTrue(didConsume);
    assertEquals(75, Selection.getSelectionStart(editable));
  }

  @Test
  public void testSendKeyEvent_rightKeyExtendsSelectionRight() {
    int selStart = 5;
    int selEnd = 40;
    ListenableEditingState editable = sampleEditable(selStart, selEnd);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent rightKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_RIGHT);
    boolean didConsume = adaptor.handleKeyEvent(rightKeyDown);

    assertTrue(didConsume);
    assertEquals(selStart, Selection.getSelectionStart(editable));
    assertEquals(selEnd + 1, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testSendKeyEvent_shiftRightKeyStartsSelectionRight() {
    int selStart = 5;
    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent shiftRightKeyDown =
        new KeyEvent(
            0, 0, KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_RIGHT, 0, KeyEvent.META_SHIFT_ON);
    boolean didConsume = adaptor.handleKeyEvent(shiftRightKeyDown);

    assertTrue(didConsume);
    assertEquals(selStart, Selection.getSelectionStart(editable));
    assertEquals(selStart + 1, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testSendKeyEvent_upKeyMovesCaretUp() {
    int selStart = SAMPLE_TEXT.indexOf('\n') + 4;
    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent upKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_UP);
    boolean didConsume = adaptor.handleKeyEvent(upKeyDown);

    assertTrue(didConsume);
    // Checks the caret moved left (to some previous character). Selection.moveUp() behaves
    // different in tests than on a real device, we can't verify the exact position.
    assertTrue(Selection.getSelectionStart(editable) < selStart);
  }

  @Test
  public void testSendKeyEvent_downKeyMovesCaretDown() {
    int selStart = 4;
    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent downKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_DOWN);
    boolean didConsume = adaptor.handleKeyEvent(downKeyDown);

    assertTrue(didConsume);
    // Checks the caret moved right (to some following character). Selection.moveDown() behaves
    // different in tests than on a real device, we can't verify the exact position.
    assertTrue(Selection.getSelectionStart(editable) > selStart);
  }

  @Test
  public void testSendKeyEvent_MovementKeysAreNopWhenNoSelection() {
    // Regression test for https://github.com/flutter/flutter/issues/76283.
    ListenableEditingState editable = sampleEditable(-1, -1);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent keyEvent = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_DOWN);
    boolean didConsume = adaptor.handleKeyEvent(keyEvent);
    assertFalse(didConsume);
    assertEquals(-1, Selection.getSelectionStart(editable));
    assertEquals(-1, Selection.getSelectionEnd(editable));

    keyEvent = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_UP);
    didConsume = adaptor.handleKeyEvent(keyEvent);
    assertFalse(didConsume);
    assertEquals(-1, Selection.getSelectionStart(editable));
    assertEquals(-1, Selection.getSelectionEnd(editable));

    keyEvent = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_LEFT);
    didConsume = adaptor.handleKeyEvent(keyEvent);
    assertFalse(didConsume);
    assertEquals(-1, Selection.getSelectionStart(editable));
    assertEquals(-1, Selection.getSelectionEnd(editable));

    keyEvent = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DPAD_RIGHT);
    didConsume = adaptor.handleKeyEvent(keyEvent);
    assertFalse(didConsume);
    assertEquals(-1, Selection.getSelectionStart(editable));
    assertEquals(-1, Selection.getSelectionEnd(editable));
  }

  @Test
  public void testMethod_getExtractedText() {
    int selStart = 5;

    ListenableEditingState editable = sampleEditable(selStart, selStart);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    ExtractedText extractedText = adaptor.getExtractedText(null, 0);

    assertEquals(SAMPLE_TEXT, extractedText.text);
    assertEquals(selStart, extractedText.selectionStart);
    assertEquals(selStart, extractedText.selectionEnd);
  }

  @Test
  public void testExtractedText_monitoring() {
    ListenableEditingState editable = sampleEditable(5, 5);
    View testView = new View(ctx);
    InputConnectionAdaptor adaptor =
        new InputConnectionAdaptor(
            testView,
            1,
            mock(TextInputChannel.class),
            mockKeyboardManager,
            editable,
            new EditorInfo());
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));

    testImm.resetStates();

    ExtractedTextRequest request = new ExtractedTextRequest();
    request.token = 123;

    ExtractedText extractedText = adaptor.getExtractedText(request, 0);
    assertEquals(5, extractedText.selectionStart);
    assertEquals(5, extractedText.selectionEnd);
    assertFalse(extractedText.text instanceof SpannableStringBuilder);

    // Move the cursor. Should not report extracted text.
    adaptor.setSelection(2, 3);
    assertNull(testImm.lastExtractedText);

    // Now request monitoring, and update the request text flag.
    request.flags = InputConnection.GET_TEXT_WITH_STYLES;
    extractedText = adaptor.getExtractedText(request, InputConnection.GET_EXTRACTED_TEXT_MONITOR);
    assertEquals(2, extractedText.selectionStart);
    assertEquals(3, extractedText.selectionEnd);
    assertTrue(extractedText.text instanceof SpannableStringBuilder);

    adaptor.setSelection(3, 5);
    assertEquals(3, testImm.lastExtractedText.selectionStart);
    assertEquals(5, testImm.lastExtractedText.selectionEnd);
    assertTrue(testImm.lastExtractedText.text instanceof SpannableStringBuilder);

    // Stop monitoring.
    testImm.resetStates();
    extractedText = adaptor.getExtractedText(request, 0);
    assertEquals(3, extractedText.selectionStart);
    assertEquals(5, extractedText.selectionEnd);
    assertTrue(extractedText.text instanceof SpannableStringBuilder);

    adaptor.setSelection(1, 3);
    assertNull(testImm.lastExtractedText);
  }

  @Test
  public void testCursorAnchorInfo() {
    ListenableEditingState editable = sampleEditable(5, 5);
    View testView = new View(ctx);
    InputConnectionAdaptor adaptor =
        new InputConnectionAdaptor(
            testView,
            1,
            mock(TextInputChannel.class),
            mockKeyboardManager,
            editable,
            new EditorInfo());
    TestImm testImm = Shadow.extract(ctx.getSystemService(Context.INPUT_METHOD_SERVICE));

    testImm.resetStates();

    // Monitoring only. Does not send update immediately.
    adaptor.requestCursorUpdates(InputConnection.CURSOR_UPDATE_MONITOR);
    assertNull(testImm.lastCursorAnchorInfo);

    // Monitor selection changes.
    adaptor.setSelection(0, 1);
    CursorAnchorInfo cursorAnchorInfo = testImm.lastCursorAnchorInfo;
    assertEquals(0, cursorAnchorInfo.getSelectionStart());
    assertEquals(1, cursorAnchorInfo.getSelectionEnd());

    // Turn monitoring off.
    testImm.resetStates();
    assertNull(testImm.lastCursorAnchorInfo);
    adaptor.requestCursorUpdates(InputConnection.CURSOR_UPDATE_IMMEDIATE);
    cursorAnchorInfo = testImm.lastCursorAnchorInfo;
    assertEquals(0, cursorAnchorInfo.getSelectionStart());
    assertEquals(1, cursorAnchorInfo.getSelectionEnd());

    // No more updates.
    testImm.resetStates();
    adaptor.setSelection(1, 3);
    assertNull(testImm.lastCursorAnchorInfo);
  }

  @Test
  public void testSendKeyEvent_sendSoftKeyEvents() {
    ListenableEditingState editable = sampleEditable(5, 5);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable, mockKeyboardManager);

    KeyEvent shiftKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_SHIFT_LEFT);

    boolean didConsume = adaptor.handleKeyEvent(shiftKeyDown);
    assertFalse(didConsume);
    verify(mockKeyboardManager, never()).handleEvent(shiftKeyDown);
  }

  @Test
  public void testSendKeyEvent_sendHardwareKeyEvents() {
    ListenableEditingState editable = sampleEditable(5, 5);
    when(mockKeyboardManager.handleEvent(any())).thenReturn(true);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable, mockKeyboardManager);

    KeyEvent shiftKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_SHIFT_LEFT);

    // Call sendKeyEvent instead of handleKeyEvent.
    boolean didConsume = adaptor.sendKeyEvent(shiftKeyDown);
    assertTrue(didConsume);
    verify(mockKeyboardManager, times(1)).handleEvent(shiftKeyDown);
  }

  @Test
  public void testSendKeyEvent_delKeyNotConsumed() {
    ListenableEditingState editable = sampleEditable(5, 5);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    KeyEvent downKeyDown = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DEL);

    for (int i = 0; i < 4; i++) {
      boolean didConsume = adaptor.handleKeyEvent(downKeyDown);
      assertFalse(didConsume);
    }
    assertEquals(5, Selection.getSelectionStart(editable));
  }

  @Test
  public void testDoesNotConsumeBackButton() {
    ListenableEditingState editable = sampleEditable(0, 0);
    InputConnectionAdaptor adaptor = sampleInputConnectionAdaptor(editable);

    FakeKeyEvent keyEvent = new FakeKeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK, '\b');
    boolean didConsume = adaptor.handleKeyEvent(keyEvent);

    assertFalse(didConsume);
  }

  @Test
  public void testCleanUpBatchEndsOnCloseConnection() {
    final ListenableEditingState editable = sampleEditable(0, 0);
    InputConnectionAdaptor adaptor = spy(sampleInputConnectionAdaptor(editable));
    for (int i = 0; i < 5; i++) {
      adaptor.beginBatchEdit();
    }
    adaptor.endBatchEdit();
    verify(adaptor, times(1)).endBatchEdit();
    adaptor.closeConnection();
    verify(adaptor, times(4)).endBatchEdit();
  }

  private static final String SAMPLE_TEXT =
      "Lorem ipsum dolor sit amet," + "\nconsectetur adipiscing elit.";

  private static final String SAMPLE_EMOJI_TEXT =
      "a" // First CodePoint
          + "😂" // Simple Emoji
          + "🇮🇷" // Regional Indicator Symbol even
          + "🇷" // Regional Indicator Symbol odd
          + "\r\n" // Carriage Return and Line Feed
          + "\r\n"
          + "✋🏿" // Emoji Modifier
          + "✋🏿"
          + "⚠️" // Variant Selector
          + "⚠️"
          + "🏴󠁧󠁢󠁥󠁮󠁧󠁿" // Emoji Tag Sequence
          + "🏴󠁧󠁢󠁥󠁮󠁧󠁿"
          + "a‍👨" // Zero Width Joiner
          + "👨‍👩‍👧‍👦"
          + "5️⃣" // Keycap
          + "5️⃣"
          + "عَ" // Non-Spacing Mark
          + "a"; // Normal Character

  private static final String SAMPLE_RTL_TEXT = "متن ساختگی" + "\nبرای تستfor test😊";

  private static ListenableEditingState sampleEditable(int selStart, int selEnd) {
    ListenableEditingState sample =
        new ListenableEditingState(null, new View(ApplicationProvider.getApplicationContext()));
    sample.replace(0, 0, SAMPLE_TEXT);
    Selection.setSelection(sample, selStart, selEnd);
    return sample;
  }

  private static ListenableEditingState sampleEditable(int selStart, int selEnd, String text) {
    ListenableEditingState sample =
        new ListenableEditingState(null, new View(ApplicationProvider.getApplicationContext()));
    sample.replace(0, 0, text);
    Selection.setSelection(sample, selStart, selEnd);
    return sample;
  }

  private static InputConnectionAdaptor sampleInputConnectionAdaptor(
      ListenableEditingState editable) {
    return sampleInputConnectionAdaptor(editable, mock(KeyboardManager.class));
  }

  private static InputConnectionAdaptor sampleInputConnectionAdaptor(
      ListenableEditingState editable, KeyboardManager mockKeyboardManager) {
    View testView = new View(ApplicationProvider.getApplicationContext());
    int client = 0;
    TextInputChannel textInputChannel = mock(TextInputChannel.class);
    FlutterTextUtils.CodePoints mockFlutterJNI = mock(FlutterTextUtils.CodePoints.class);
    when(mockFlutterJNI.isCodePointEmoji(anyInt()))
        .thenAnswer((invocation) -> Emoji.isEmoji((int) invocation.getArguments()[0]));
    when(mockFlutterJNI.isCodePointEmojiModifier(anyInt()))
        .thenAnswer((invocation) -> Emoji.isEmojiModifier((int) invocation.getArguments()[0]));
    when(mockFlutterJNI.isCodePointEmojiModifierBase(anyInt()))
        .thenAnswer((invocation) -> Emoji.isEmojiModifierBase((int) invocation.getArguments()[0]));
    when(mockFlutterJNI.isCodePointVariantSelector(anyInt()))
        .thenAnswer((invocation) -> Emoji.isVariationSelector((int) invocation.getArguments()[0]));
    when(mockFlutterJNI.isCodePointRegionalIndicator(anyInt()))
        .thenAnswer(
            (invocation) -> Emoji.isRegionalIndicatorSymbol((int) invocation.getArguments()[0]));
    return new InputConnectionAdaptor(
        testView,
        client,
        textInputChannel,
        mockKeyboardManager,
        editable,
        null,
        mockFlutterJNI);
  }

  private static class Emoji {
    public static boolean isEmoji(int codePoint) {
      return UCharacter.hasBinaryProperty(codePoint, UProperty.EMOJI);
    }

    public static boolean isEmojiModifier(int codePoint) {
      return UCharacter.hasBinaryProperty(codePoint, UProperty.EMOJI_MODIFIER);
    }

    public static boolean isEmojiModifierBase(int codePoint) {
      return UCharacter.hasBinaryProperty(codePoint, UProperty.EMOJI_MODIFIER_BASE);
    }

    public static boolean isRegionalIndicatorSymbol(int codePoint) {
      return UCharacter.hasBinaryProperty(codePoint, UProperty.REGIONAL_INDICATOR);
    }

    public static boolean isVariationSelector(int codePoint) {
      return UCharacter.hasBinaryProperty(codePoint, UProperty.VARIATION_SELECTOR);
    }
  }

  private class TestTextInputChannel extends TextInputChannel {
    public TestTextInputChannel(int activity) {
      super(activity);
    }

    public int inputClientId;
    public String text;
    public int selectionStart;
    public int selectionEnd;
    public int composingStart;
    public int composingEnd;
    public int updateEditingStateInvocations = 0;

    @Override
    public void updateEditingState(
        int inputClientId,
        String text,
        int selectionStart,
        int selectionEnd,
        int composingStart,
        int composingEnd) {
      this.inputClientId = inputClientId;
      this.text = text;
      this.selectionStart = selectionStart;
      this.selectionEnd = selectionEnd;
      this.composingStart = composingStart;
      this.composingEnd = composingEnd;
      updateEditingStateInvocations++;
    }
  }

  @Implements(InputMethodManager.class)
  public static class TestImm extends ShadowInputMethodManager {
    public static int empty = -999;
    CursorAnchorInfo lastCursorAnchorInfo;
    int lastExtractedTextToken = empty;
    ExtractedText lastExtractedText;

    int lastSelectionStart = empty;
    int lastSelectionEnd = empty;
    int lastCandidatesStart = empty;
    int lastCandidatesEnd = empty;

    public TestImm() {}

    @Implementation
    public void updateCursorAnchorInfo(View view, CursorAnchorInfo cursorAnchorInfo) {
      lastCursorAnchorInfo = cursorAnchorInfo;
    }

    @Implementation
    public void updateExtractedText(View view, int token, ExtractedText text) {
      lastExtractedTextToken = token;
      lastExtractedText = text;
    }

    @Implementation
    public void updateSelection(
        View view, int selStart, int selEnd, int candidatesStart, int candidatesEnd) {
      lastSelectionStart = selStart;
      lastSelectionEnd = selEnd;
      lastCandidatesStart = candidatesStart;
      lastCandidatesEnd = candidatesEnd;
    }

    public void resetStates() {
      lastExtractedText = null;
      lastExtractedTextToken = empty;

      lastSelectionStart = empty;
      lastSelectionEnd = empty;
      lastCandidatesStart = empty;
      lastCandidatesEnd = empty;

      lastCursorAnchorInfo = null;
    }
  }
}
