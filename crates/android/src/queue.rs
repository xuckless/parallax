//! Copied from GPUI (zed-industries/zed) `crates/gpui/src/queue.rs` @ f126950f9d3b62666f97437ae5d4ccac2272c4db
//! (gpui-pre 0.3.14, `gpui-pre/src/queue.rs`). Copyright 2022 - 2025 Zed Industries, Inc. Apache-2.0.
//!
//! Why a copy: upstream exports `PriorityQueueSender`/`PriorityQueueReceiver` on Linux, Windows and wasm, not on
//! Android. Parallax's patch `patches/0001-export-priority-queue-android.patch` adds the Android cfg, but published
//! crates build on stock gpui-pre (AGENTS.md, patch rule 11). Delete this file and import the types from `gpui` once
//! patch 0001 lands upstream.
//!
//! Kept: only what the Android dispatcher uses (`new`, `send`, `clone`, `iter`, `try_iter`); the spin variants,
//! `is_empty`, `len` and `pop` are left out. Code and comments are otherwise upstream's, so upstream fixes diff in.
//!
//! Provides a thread-safe queue with weighted random selection between priorities.
//!
//! Items within each priority are received in FIFO order. Receivers can be cloned
//! to share work, and queued items remain available after the sender is dropped.

use std::{
    collections::VecDeque,
    fmt,
    iter::FusedIterator,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, atomic::AtomicUsize},
};

use gpui::Priority;
use rand::{Rng, SeedableRng, rngs::SmallRng};

struct PriorityQueues<T> {
    high_priority: VecDeque<T>,
    medium_priority: VecDeque<T>,
    low_priority: VecDeque<T>,
}

impl<T> PriorityQueues<T> {
    fn is_empty(&self) -> bool {
        self.high_priority.is_empty()
            && self.medium_priority.is_empty()
            && self.low_priority.is_empty()
    }
}

struct PriorityQueueState<T> {
    // std's Wasm atomic backend unlocks and notifies without acquiring another
    // lock. parking_lot can park during those operations, even after try_lock.
    queues: Mutex<PriorityQueues<T>>,
    condvar: Condvar,
    receiver_count: AtomicUsize,
    sender_count: AtomicUsize,
}

impl<T> PriorityQueueState<T> {
    fn send(&self, priority: Priority, item: T) -> Result<(), SendError<T>> {
        if self
            .receiver_count
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            return Err(SendError(item));
        }

        let mut queues = self.queues.lock().unwrap_or_else(PoisonError::into_inner);
        Self::push(&mut queues, priority, item);
        self.condvar.notify_one();
        Ok(())
    }

    fn push(queues: &mut PriorityQueues<T>, priority: Priority, item: T) {
        match priority {
            Priority::RealtimeAudio => unreachable!(
                "Realtime audio priority runs on a dedicated thread and is never queued"
            ),
            Priority::High => queues.high_priority.push_back(item),
            Priority::Medium => queues.medium_priority.push_back(item),
            Priority::Low => queues.low_priority.push_back(item),
        };
    }

    fn recv<'a>(&'a self) -> Result<MutexGuard<'a, PriorityQueues<T>>, RecvError> {
        let mut queues = self.queues.lock().unwrap_or_else(PoisonError::into_inner);

        // Re-checked after every wake: dropping the last sender wakes waiting receivers
        // so they can end instead of blocking forever on a queue nothing can fill.
        while queues.is_empty() {
            if self.sender_count.load(std::sync::atomic::Ordering::Acquire) == 0 {
                return Err(RecvError);
            }
            queues = self
                .condvar
                .wait(queues)
                .unwrap_or_else(PoisonError::into_inner);
        }

        Ok(queues)
    }

    fn try_recv<'a>(&'a self) -> Result<Option<MutexGuard<'a, PriorityQueues<T>>>, RecvError> {
        let queues = self.queues.lock().unwrap_or_else(PoisonError::into_inner);

        let sender_count = self.sender_count.load(std::sync::atomic::Ordering::Relaxed);
        if queues.is_empty() && sender_count == 0 {
            return Err(RecvError);
        }

        if queues.is_empty() {
            Ok(None)
        } else {
            Ok(Some(queues))
        }
    }
}

pub(crate) struct PriorityQueueSender<T> {
    state: Arc<PriorityQueueState<T>>,
}

impl<T> PriorityQueueSender<T> {
    fn new(state: Arc<PriorityQueueState<T>>) -> Self {
        Self { state }
    }

    /// Enqueues an item at the given priority, waiting for the queue lock if necessary.
    ///
    /// # Errors
    ///
    /// Returns the item in a [`SendError<T>`] if all receivers have been dropped.
    ///
    /// # Panics
    ///
    /// Panics if `priority` is [`Priority::RealtimeAudio`] and a receiver still exists.
    pub(crate) fn send(&self, priority: Priority, item: T) -> Result<(), SendError<T>> {
        self.state.send(priority, item)?;
        Ok(())
    }
}

impl<T> Drop for PriorityQueueSender<T> {
    fn drop(&mut self) {
        let previous = self
            .state
            .sender_count
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        if previous == 1 {
            // A receiver checks the sender count and starts waiting under the queue lock,
            // atomically. Acquiring that lock once, even with nothing to do under it,
            // means any receiver that saw a live sender is already waiting, so the
            // notification below reaches it; without this, a receiver between its check
            // and its wait would miss the wake-up and block forever.
            drop(
                self.state
                    .queues
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner),
            );
            self.state.condvar.notify_all();
        }
    }
}

pub(crate) struct PriorityQueueReceiver<T> {
    state: Arc<PriorityQueueState<T>>,
    rand: SmallRng,
    disconnected: bool,
}

impl<T> Clone for PriorityQueueReceiver<T> {
    fn clone(&self) -> Self {
        self.state
            .receiver_count
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Self {
            state: Arc::clone(&self.state),
            rand: SmallRng::seed_from_u64(0),
            disconnected: self.disconnected,
        }
    }
}

pub(crate) struct SendError<T>(pub T);

impl<T: fmt::Debug> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SendError").field(&self.0).finish()
    }
}

#[derive(Debug)]
pub(crate) struct RecvError;

impl<T> PriorityQueueReceiver<T> {
    /// Creates an empty priority queue and returns its sender and receiver.
    pub(crate) fn new() -> (PriorityQueueSender<T>, Self) {
        let state = PriorityQueueState {
            queues: Mutex::new(PriorityQueues {
                high_priority: VecDeque::new(),
                medium_priority: VecDeque::new(),
                low_priority: VecDeque::new(),
            }),
            condvar: Condvar::new(),
            receiver_count: AtomicUsize::new(1),
            sender_count: AtomicUsize::new(1),
        };
        let state = Arc::new(state);

        let sender = PriorityQueueSender::new(Arc::clone(&state));

        let receiver = PriorityQueueReceiver {
            state,
            rand: SmallRng::seed_from_u64(0),
            disconnected: false,
        };

        (sender, receiver)
    }

    /// Tries to pop one element from the priority queue without blocking.
    ///
    /// This will early return if there are no elements in the queue.
    ///
    /// # Errors
    ///
    /// If the sender was dropped
    fn try_pop(&mut self) -> Result<Option<T>, RecvError> {
        self.pop_inner(false)
    }

    /// Returns an iterator over the elements of the queue
    /// this iterator will end when all elements have been consumed and will not wait for new ones.
    pub(crate) fn try_iter(self) -> TryIter<T> {
        TryIter {
            receiver: self,
            ended: false,
        }
    }

    /// Returns an iterator over the elements of the queue
    /// this iterator will wait for new elements if the queue is empty.
    pub(crate) fn iter(self) -> Iter<T> {
        Iter(self)
    }

    #[inline(always)]
    // algorithm is the loaded die from biased coin from
    // https://www.keithschwarz.com/darts-dice-coins/
    fn pop_inner(&mut self, block: bool) -> Result<Option<T>, RecvError> {
        use Priority as P;

        let mut queues = if !block {
            let Some(queues) = self.state.try_recv()? else {
                return Ok(None);
            };
            queues
        } else {
            self.state.recv()?
        };

        let high = P::High.weight() * !queues.high_priority.is_empty() as u32;
        let medium = P::Medium.weight() * !queues.medium_priority.is_empty() as u32;
        let low = P::Low.weight() * !queues.low_priority.is_empty() as u32;
        let mut mass = high + medium + low; //%

        if !queues.high_priority.is_empty() {
            let flip = self.rand.random_ratio(P::High.weight(), mass);
            if flip {
                return Ok(queues.high_priority.pop_front());
            }
            mass -= P::High.weight();
        }

        if !queues.medium_priority.is_empty() {
            let flip = self.rand.random_ratio(P::Medium.weight(), mass);
            if flip {
                return Ok(queues.medium_priority.pop_front());
            }
            mass -= P::Medium.weight();
        }

        if !queues.low_priority.is_empty() {
            let flip = self.rand.random_ratio(P::Low.weight(), mass);
            if flip {
                return Ok(queues.low_priority.pop_front());
            }
        }

        Ok(None)
    }
}

impl<T> Drop for PriorityQueueReceiver<T> {
    fn drop(&mut self) {
        self.state
            .receiver_count
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

pub(crate) struct Iter<T>(PriorityQueueReceiver<T>);
impl<T> Iterator for Iter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // Upstream calls `pop()`, which unwraps this `Option`; a blocking pop always finds an item.
        self.0.pop_inner(true).ok().flatten()
    }
}
impl<T> FusedIterator for Iter<T> {}

pub(crate) struct TryIter<T> {
    receiver: PriorityQueueReceiver<T>,
    ended: bool,
}
impl<T> Iterator for TryIter<T> {
    type Item = Result<T, RecvError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }

        let res = self.receiver.try_pop();
        self.ended = res.is_err();

        res.transpose()
    }
}
impl<T> FusedIterator for TryIter<T> {}

// Upstream's tests, unchanged but for `pop()` → `iter().next()` (pop is not copied) and std's `HashSet`.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn blocked_receivers_end_when_the_sender_drops() {
        let (sender, receiver) = PriorityQueueReceiver::<u32>::new();
        let (done_sender, done_receiver) = std::sync::mpsc::channel();
        let workers = (0..2)
            .map(|_| {
                let receiver = receiver.clone();
                let done_sender = done_sender.clone();
                std::thread::spawn(move || {
                    let received: Vec<u32> = receiver.iter().collect();
                    done_sender.send(received).ok();
                })
            })
            .collect::<Vec<_>>();
        drop(receiver);

        sender.send(Priority::Medium, 7).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            done_receiver.try_recv().is_err(),
            "receivers keep waiting while the sender is alive"
        );

        drop(sender);
        let mut received = Vec::new();
        for _ in 0..2 {
            received.extend(
                done_receiver
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("a waiting receiver should end once the sender is dropped"),
            );
        }
        assert_eq!(received, [7]);
        for worker in workers {
            worker.join().expect("worker should exit cleanly");
        }
    }

    #[test]
    fn all_tasks_get_yielded() {
        let (tx, rx) = PriorityQueueReceiver::new();
        tx.send(Priority::Medium, 20).unwrap();
        tx.send(Priority::High, 30).unwrap();
        tx.send(Priority::Low, 10).unwrap();
        tx.send(Priority::Medium, 21).unwrap();
        tx.send(Priority::High, 31).unwrap();

        drop(tx);

        assert_eq!(
            rx.iter().collect::<HashSet<_>>(),
            [30, 31, 20, 21, 10].into_iter().collect::<HashSet<_>>()
        )
    }

    #[test]
    fn new_high_prio_task_get_scheduled_quickly() {
        let (tx, rx) = PriorityQueueReceiver::new();
        for _ in 0..100 {
            tx.send(Priority::Low, 1).unwrap();
        }

        let mut rx = rx.iter();
        assert_eq!(rx.next().unwrap(), 1);
        tx.send(Priority::High, 3).unwrap();
        assert_eq!(rx.next().unwrap(), 3);
        assert_eq!(rx.next().unwrap(), 1);
    }
}
