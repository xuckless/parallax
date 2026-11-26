// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/io/flutter/plugin/editing/TextEditingDelta.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package; androidx annotations and io.flutter.Log (→ android.util.Log) dropped
// (not on Parallax's classpath).
// Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.util.Log;
import org.json.JSONException;
import org.json.JSONObject;

/// A representation of the change that occured to an editing state, along with the resulting
/// composing and selection regions.
public final class TextEditingDelta {
  private CharSequence oldText;
  private CharSequence deltaText;
  private int deltaStart;
  private int deltaEnd;
  private int newSelectionStart;
  private int newSelectionEnd;
  private int newComposingStart;
  private int newComposingEnd;

  private static final String TAG = "TextEditingDelta";

  public TextEditingDelta(
      CharSequence oldEditable,
      int replacementDestinationStart,
      int replacementDestinationEnd,
      CharSequence replacementSource,
      int selectionStart,
      int selectionEnd,
      int composingStart,
      int composingEnd) {
    newSelectionStart = selectionStart;
    newSelectionEnd = selectionEnd;
    newComposingStart = composingStart;
    newComposingEnd = composingEnd;

    setDeltas(
        oldEditable,
        replacementSource.toString(),
        replacementDestinationStart,
        replacementDestinationEnd);
  }

  // Non text update delta constructor.
  public TextEditingDelta(
      CharSequence oldText,
      int selectionStart,
      int selectionEnd,
      int composingStart,
      int composingEnd) {
    newSelectionStart = selectionStart;
    newSelectionEnd = selectionEnd;
    newComposingStart = composingStart;
    newComposingEnd = composingEnd;

    setDeltas(oldText, "", -1, -1);
  }

  public CharSequence getOldText() {
    return oldText;
  }

  public CharSequence getDeltaText() {
    return deltaText;
  }

  public int getDeltaStart() {
    return deltaStart;
  }

  public int getDeltaEnd() {
    return deltaEnd;
  }

  public int getNewSelectionStart() {
    return newSelectionStart;
  }

  public int getNewSelectionEnd() {
    return newSelectionEnd;
  }

  public int getNewComposingStart() {
    return newComposingStart;
  }

  public int getNewComposingEnd() {
    return newComposingEnd;
  }

  private void setDeltas(
      CharSequence oldText, CharSequence newText, int newStart, int newExtent) {
    this.oldText = oldText;
    deltaText = newText;
    deltaStart = newStart;
    deltaEnd = newExtent;
  }

  public JSONObject toJSON() {
    JSONObject delta = new JSONObject();

    try {
      delta.put("oldText", oldText.toString());
      delta.put("deltaText", deltaText.toString());
      delta.put("deltaStart", deltaStart);
      delta.put("deltaEnd", deltaEnd);
      delta.put("selectionBase", newSelectionStart);
      delta.put("selectionExtent", newSelectionEnd);
      delta.put("composingBase", newComposingStart);
      delta.put("composingExtent", newComposingEnd);
    } catch (JSONException e) {
      Log.e(TAG, "unable to create JSONObject: " + e);
    }

    return delta;
  }
}
