// Copyright 2013 The Flutter Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Ported from flutter/flutter
// engine/src/flutter/shell/platform/android/io/flutter/embedding/android/MotionEventTracker.java
// @ 3.50.1 (ae9b586f34a8443573f82ef57c3bf914a908cbf7). Copyright 2014 The Flutter Authors.
// BSD-3-Clause, see third_party/flutter/LICENSE.
//
// Parallax changes: package; androidx annotations dropped (not on Parallax's classpath).
// Ledger: third_party/flutter/PORTED.md.

package io.github.xuckless.parallax;

import android.util.LongSparseArray;
import android.view.MotionEvent;
import java.util.PriorityQueue;
import java.util.concurrent.atomic.AtomicLong;

/** Tracks the motion events received by the FlutterView. */
public final class MotionEventTracker {
  private static final String TAG = "MotionEventTracker";
  /** Represents a unique identifier corresponding to a motion event. */
  public static class MotionEventId {
    private static final AtomicLong ID_COUNTER = new AtomicLong(0);
    private final long id;

    private MotionEventId(long id) {
      this.id = id;
    }

    public static MotionEventId from(long id) {
      return new MotionEventId(id);
    }

    public static MotionEventId createUnique() {
      return MotionEventId.from(ID_COUNTER.incrementAndGet());
    }

    public long getId() {
      return id;
    }
  }

  private final LongSparseArray<MotionEvent> eventById;
  private final PriorityQueue<Long> unusedEvents;
  private static MotionEventTracker INSTANCE;

  public static MotionEventTracker getInstance() {
    if (INSTANCE == null) {
      INSTANCE = new MotionEventTracker();
    }
    return INSTANCE;
  }

  private MotionEventTracker() {
    eventById = new LongSparseArray<>();
    unusedEvents = new PriorityQueue<>();
  }

  /** Tracks the event and returns a unique MotionEventId identifying the event. */
  public MotionEventId track(MotionEvent event) {
    MotionEventId eventId = MotionEventId.createUnique();
    // We copy event here because the original MotionEvent delivered to us
    // will be automatically recycled (`MotionEvent.recycle`) by the RootView and we need
    // access to it after the RootView code runs.
    // The return value of `MotionEvent.obtain(event)` is still verifiable if the input
    // event was verifiable. Other overloads of `MotionEvent.obtain` do not have this
    // guarantee and should be avoided when possible.
    MotionEvent eventCopy = MotionEvent.obtain(event);
    eventById.put(eventId.id, eventCopy);
    unusedEvents.add(eventId.id);
    return eventId;
  }

  /**
   * Returns the MotionEvent corresponding to the eventId while discarding all the motion events
   * that occurred prior to the event represented by the eventId. Returns null if this event was
   * popped or discarded.
   */
  public MotionEvent pop(MotionEventId eventId) {
    // remove all the older events.
    while (!unusedEvents.isEmpty() && unusedEvents.peek() < eventId.id) {
      eventById.remove(unusedEvents.poll());
    }

    // remove the current event from the heap if it exists.
    if (!unusedEvents.isEmpty() && unusedEvents.peek() == eventId.id) {
      unusedEvents.poll();
    }

    MotionEvent event = eventById.get(eventId.id);
    eventById.remove(eventId.id);
    return event;
  }

  /**
   * Returns the MotionEvent corresponding to the eventId without consuming it, or null if this
   * event was popped or discarded.
   */
  public MotionEvent peek(MotionEventId eventId) {
    return eventById.get(eventId.id);
  }
}
