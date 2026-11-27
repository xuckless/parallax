// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/test/io/flutter/embedding/android/FlutterViewTest.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package; one case of the file, ViewportMetrics_initializedPhysicalTouchSlop
// (Flutter's lacks @Test, so it never runs there; here it does). FlutterView + FlutterEngine +
// FlutterRenderer → a ParallaxSurfaceView whose nativeInsets reach a mocked
// ShadowParallaxSurfaceView.Renderer as ViewportMetrics; attachToFlutterEngine (which sends the
// metrics) → onApplyWindowInsets, the view's call that sends them, verified at least once. Also
// checks the slop is ViewConfiguration's scaled one and the density the display's. Ledger:
// third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.mockito.Mockito.atLeast;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;

import android.content.Context;
import android.view.ViewConfiguration;
import android.view.WindowInsets;
import androidx.test.core.app.ApplicationProvider;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.mockito.ArgumentCaptor;
import org.robolectric.annotation.Config;

@Config(shadows = {ShadowParallaxSurfaceView.class})
@RunWith(AndroidJUnit4.class)
public class FlutterViewTest {
  private final Context ctx = ApplicationProvider.getApplicationContext();

  @Test
  public void ViewportMetrics_initializedPhysicalTouchSlop() {
    ParallaxSurfaceView flutterView = new ParallaxSurfaceView(ctx, 0);
    ShadowParallaxSurfaceView.Renderer flutterRenderer =
        mock(ShadowParallaxSurfaceView.Renderer.class);
    ShadowParallaxSurfaceView.renderer = flutterRenderer;

    flutterView.onApplyWindowInsets(new WindowInsets.Builder().build());
    ArgumentCaptor<ShadowParallaxSurfaceView.ViewportMetrics> viewportMetricsCaptor =
        ArgumentCaptor.forClass(ShadowParallaxSurfaceView.ViewportMetrics.class);
    // Parallax: at least once (Robolectric's View.onApplyWindowInsets dispatch sends them again).
    verify(flutterRenderer, atLeast(1)).setViewportMetrics(viewportMetricsCaptor.capture());

    assertFalse(-1 == viewportMetricsCaptor.getValue().physicalTouchSlop);
    // Parallax: the value is the device's, in physical pixels, sent with the density.
    assertEquals(
        ViewConfiguration.get(ctx).getScaledTouchSlop(),
        viewportMetricsCaptor.getValue().physicalTouchSlop);
    assertEquals(
        ctx.getResources().getDisplayMetrics().density,
        viewportMetricsCaptor.getValue().devicePixelRatio,
        0f);
  }
}
