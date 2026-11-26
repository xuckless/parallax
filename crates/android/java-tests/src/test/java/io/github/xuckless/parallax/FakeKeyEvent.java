// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/test/io/flutter/util/FakeKeyEvent.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package (one test package for the whole port). Nothing dropped.
// Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.view.KeyEvent;

// In the test environment, keyEvent.getUnicodeChar throws an exception. This
// class works around the exception by hardcoding the returned value.
public class FakeKeyEvent extends KeyEvent {
  public FakeKeyEvent(int action, int keyCode) {
    super(action, keyCode);
  }

  public FakeKeyEvent(int action, int keyCode, char character) {
    super(action, keyCode);
    this.character = character;
  }

  public FakeKeyEvent(
      int action, int scancode, int code, int repeat, char character, int metaState) {
    super(0, 0, action, code, repeat, metaState, 0, scancode);
    this.character = character;
  }

  public FakeKeyEvent(
      int action, int scancode, int code, int repeat, char character, int metaState, int source) {
    super(0, 0, action, code, repeat, metaState, 0, scancode, 0, source);
    this.character = character;
  }

  public FakeKeyEvent(
      int action,
      int scancode,
      int code,
      int repeat,
      char character,
      int metaState,
      int deviceId,
      int source) {
    super(0, 0, action, code, repeat, metaState, deviceId, scancode, 0, source);
    this.character = character;
  }

  private char character = 0;

  public final int getUnicodeChar() {
    if (getKeyCode() == KeyEvent.KEYCODE_BACK) {
      return 0;
    }
    return character;
  }
}
