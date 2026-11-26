//! GPUI's dispatcher on Android. Main-thread work goes into a priority queue drained by an `ALooper_addFd`
//! callback on the main thread's Looper, woken through an eventfd: no Java `Handler`, no JNI call per task.
//! Background work runs on a worker pool, timers on one timer thread (the same split as upstream's Linux dispatcher).

use std::{
    ffi::{c_int, c_void},
    mem,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use gpui::{PlatformDispatcher, Priority, RunnableVariant};

use crate::Error;
use crate::queue::{PriorityQueueReceiver, PriorityQueueSender};

const MIN_WORKERS: usize = 2;

pub(crate) struct AndroidDispatcher {
    main: PriorityQueueSender<RunnableVariant>,
    /// Written to wake the Looper. Never closed: the Looper callback reads it for the life of the process.
    main_wake: c_int,
    background: PriorityQueueSender<RunnableVariant>,
    timers: mpsc::Sender<Timer>,
    main_thread: thread::ThreadId,
}

/// The Looper callback's data. Leaked on purpose: Android gives no point after which the Looper cannot call back,
/// and GPUI on Android lives as long as the process (there is no terminate phase).
struct MainQueue {
    receiver: PriorityQueueReceiver<RunnableVariant>,
}

struct Timer {
    due: Instant,
    runnable: RunnableVariant,
}

impl AndroidDispatcher {
    /// Must run on the Android main thread: it registers with that thread's Looper.
    pub(crate) fn new() -> Result<Self, Error> {
        // SAFETY: ALooper_forThread only reads the calling thread's Looper; it returns null when there is none.
        let looper = unsafe { ndk_sys::ALooper_forThread() };
        if looper.is_null() {
            return Err(Error::NoLooper);
        }
        // SAFETY: eventfd takes no pointers; a negative result is checked below.
        let main_wake = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if main_wake < 0 {
            return Err(Error::Os("eventfd", std::io::Error::last_os_error()));
        }
        let (main, receiver) = PriorityQueueReceiver::new();
        let queue: &'static MainQueue = Box::leak(Box::new(MainQueue { receiver }));
        // SAFETY: `looper` is this thread's Looper, `main_wake` an open fd, and `queue` lives for the process,
        // so the callback's data pointer stays valid for every call.
        let added = unsafe {
            ndk_sys::ALooper_addFd(
                looper,
                main_wake,
                ndk_sys::ALOOPER_POLL_CALLBACK,
                ndk_sys::ALOOPER_EVENT_INPUT as c_int,
                Some(run_main_queue),
                (queue as *const MainQueue).cast_mut().cast(),
            )
        };
        if added != 1 {
            return Err(Error::Os("ALooper_addFd", std::io::Error::last_os_error()));
        }

        let (background, background_receiver) = PriorityQueueReceiver::new();
        let workers =
            thread::available_parallelism().map_or(MIN_WORKERS, |n| n.get().max(MIN_WORKERS));
        for index in 0..workers {
            let receiver: PriorityQueueReceiver<RunnableVariant> = background_receiver.clone();
            thread::Builder::new()
                .name(format!("parallax-worker-{index}"))
                .spawn(move || {
                    for runnable in receiver.iter() {
                        runnable.run();
                    }
                })
                .map_err(|error| Error::Os("spawn worker thread", error))?;
        }

        let (timers, timer_receiver) = mpsc::channel();
        thread::Builder::new()
            .name("parallax-timers".into())
            .spawn(move || run_timers(timer_receiver))
            .map_err(|error| Error::Os("spawn timer thread", error))?;

        Ok(Self {
            main,
            main_wake,
            background,
            timers,
            main_thread: thread::current().id(),
        })
    }
}

/// Runs on the main thread whenever `main_wake` is readable: drains the main queue.
unsafe extern "C" fn run_main_queue(fd: c_int, _events: c_int, data: *mut c_void) -> c_int {
    // SAFETY: `data` is the leaked `MainQueue` registered with this callback in `AndroidDispatcher::new`.
    let queue = unsafe { &*(data as *const MainQueue) };
    let mut count = 0u64;
    // SAFETY: `fd` is the eventfd; reading its 8-byte counter into a u64 resets it. EAGAIN (already reset) is harmless.
    unsafe { libc::read(fd, (&raw mut count).cast(), mem::size_of::<u64>()) };
    for runnable in queue.receiver.clone().try_iter() {
        match runnable {
            Ok(runnable) => {
                runnable.run();
            }
            Err(_disconnected) => break,
        }
    }
    1 // keep the fd registered
}

/// The timer thread: runs each timer's runnable when it is due, like upstream's Linux timer thread.
fn run_timers(receiver: mpsc::Receiver<Timer>) {
    let mut pending: Vec<Timer> = Vec::new();
    loop {
        let now = Instant::now();
        let (due, later): (Vec<Timer>, Vec<Timer>) =
            pending.into_iter().partition(|timer| timer.due <= now);
        pending = later;
        for timer in due {
            timer.runnable.run();
        }
        let received = match pending.iter().map(|timer| timer.due).min() {
            Some(next) => receiver.recv_timeout(next.saturating_duration_since(Instant::now())),
            None => receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        match received {
            Ok(timer) => pending.push(timer),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Shutting down: dropping a runnable cancels its task and panics its awaiter; leak instead.
                mem::forget(pending);
                return;
            }
        }
    }
}

impl PlatformDispatcher for AndroidDispatcher {
    fn is_main_thread(&self) -> bool {
        thread::current().id() == self.main_thread
    }

    fn dispatch(&self, runnable: RunnableVariant, priority: Priority) {
        if let Err(error) = self.background.send(priority, runnable) {
            // Every worker is gone (process shutting down): leak rather than drop a task off its thread.
            mem::forget(error);
        }
    }

    fn dispatch_on_main_thread(&self, runnable: RunnableVariant, priority: Priority) {
        if let Err(error) = self.main.send(priority, runnable) {
            // The runnable may wrap a !Send future and we may be off the main thread: never drop it here.
            mem::forget(error);
            return;
        }
        let one = 1u64;
        // SAFETY: `main_wake` is the open eventfd; writing an 8-byte u64 adds to its counter.
        unsafe {
            libc::write(
                self.main_wake,
                (&raw const one).cast(),
                mem::size_of::<u64>(),
            )
        };
    }

    fn dispatch_after(&self, duration: Duration, runnable: RunnableVariant) {
        let timer = Timer {
            due: Instant::now() + duration,
            runnable,
        };
        if let Err(error) = self.timers.send(timer) {
            // The timer thread is gone (shutting down): leak, as upstream's Linux dispatcher does.
            mem::forget(error);
        }
    }

    fn spawn_realtime(&self, f: Box<dyn FnOnce() + Send>) {
        // Not yet on Android: a plain thread, without a realtime scheduling priority.
        if let Err(error) = thread::Builder::new()
            .name("parallax-realtime".into())
            .spawn(f)
        {
            log::error!("parallax: cannot spawn a realtime thread: {error}");
        }
    }
}
