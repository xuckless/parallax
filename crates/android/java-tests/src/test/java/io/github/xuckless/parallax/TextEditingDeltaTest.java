// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/test/io/flutter/plugin/editing/TextEditingDeltaTest.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package. Nothing dropped. Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import static org.junit.Assert.assertEquals;

import androidx.test.ext.junit.runners.AndroidJUnit4;
import org.junit.Before;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.mockito.MockitoAnnotations;

@RunWith(AndroidJUnit4.class)
public class TextEditingDeltaTest {
  @Before
  public void setUp() {
    MockitoAnnotations.openMocks(this);
  }

  @Test
  public void testConstructorTextEditingDelta() {
    final CharSequence oldText = "hell";
    final CharSequence textAfterChange = "hello";

    final int oldComposingStart = 0;
    final int oldComposingEnd = 4;

    final int startOfReplacementText = 0;
    final int endOfReplacementText = textAfterChange.length();

    final int newSelectionStart = 5;
    final int newSelectionEnd = 5;
    final int newComposingStart = 0;
    final int newComposingEnd = 5;

    final TextEditingDelta delta =
        new TextEditingDelta(
            oldText,
            oldComposingStart,
            oldComposingEnd,
            textAfterChange,
            newSelectionStart,
            newSelectionEnd,
            newComposingStart,
            newComposingEnd);

    assertEquals(oldText, delta.getOldText());
    assertEquals(textAfterChange, delta.getDeltaText());
    assertEquals(oldComposingStart, delta.getDeltaStart());
    assertEquals(oldComposingEnd, delta.getDeltaEnd());
    assertEquals(newSelectionStart, delta.getNewSelectionStart());
    assertEquals(newSelectionEnd, delta.getNewSelectionEnd());
    assertEquals(newComposingStart, delta.getNewComposingStart());
    assertEquals(newComposingEnd, delta.getNewComposingEnd());
  }

  @Test
  public void testNonTextUpdateConstructorTextEditingDelta() {
    final CharSequence oldText = "hello";

    final int newSelectionStart = 3;
    final int newSelectionEnd = 3;
    final int newComposingStart = 0;
    final int newComposingEnd = 5;

    final TextEditingDelta delta =
        new TextEditingDelta(
            oldText, newSelectionStart, newSelectionEnd, newComposingStart, newComposingEnd);

    assertEquals(oldText, delta.getOldText());
    assertEquals("", delta.getDeltaText());
    assertEquals(-1, delta.getDeltaStart());
    assertEquals(-1, delta.getDeltaEnd());
    assertEquals(newSelectionStart, delta.getNewSelectionStart());
    assertEquals(newSelectionEnd, delta.getNewSelectionEnd());
    assertEquals(newComposingStart, delta.getNewComposingStart());
    assertEquals(newComposingEnd, delta.getNewComposingEnd());
  }
}
