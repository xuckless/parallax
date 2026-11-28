//! The "page": what a browser engine would be. It owns one `wl_surface` on GPUI's own Wayland connection, on its
//! own event queue and thread, and commits a new shm buffer to it about `fps` times a second, without GPUI, each
//! through the view's `NativeViewBuffers`, which crops it to the view's clip.

use gpui_linux::{Committed, NativeViewBuffers};
use parallax::gpui::size;
use std::{
    ffi::c_void,
    os::fd::{AsFd, FromRawFd, OwnedFd},
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface},
};

/// The buffer sizes the page draws, in pixels; the compositor scales each to the view's size (wp_viewport). The
/// first is the page's size; `--resize` switches between both, like a browser following its view's size.
pub const SIZES: [(i32, i32); 2] = [(640, 400), (320, 200)];
/// Enough buffers that one is free while the compositor holds the others.
const BUFFER_COUNT: usize = 4;
/// Bytes of one buffer slot of the pool: room for the largest size.
const SLOT: usize = SIZES[0].0 as usize * 4 * SIZES[0].1 as usize;
/// `--resize`: frames between size switches, and how many frames of the old size follow a switch (a browser's
/// frames already in flight when its size changes).
const RESIZE_EVERY: u64 = 45;
const STALE_FRAMES: u64 = 2;

pub struct Page {
    /// The surface the app hands to `NativeView::new`.
    pub surface: wl_surface::WlSurface,
    /// Buffers committed so far, read by the example's frame counter.
    pub frames: Arc<AtomicU64>,
    /// The thread uses GPUI's connection, so it must stop before GPUI disconnects: dropping the page stops it.
    running: Arc<AtomicBool>,
    thread: Thread,
}

/// The page's thread, which starts once the view's buffers exist.
enum Thread {
    Ready(Box<Setup>),
    Running(JoinHandle<()>),
    /// Stopped, or it never started.
    Stopped,
}

/// What the thread takes: the page's queue and its shm buffers.
struct Setup {
    connection: Connection,
    queue: EventQueue<PageState>,
    /// One wl_buffer per size and slot, `buffers[size][slot]`, sharing the slot's memory.
    buffers: [Vec<wl_buffer::WlBuffer>; 2],
    /// The pool's mapping, as an address (the thread owns it).
    memory: usize,
    fd: OwnedFd,
    period: Duration,
    resize: bool,
}

impl Drop for Page {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Thread::Running(thread) = std::mem::replace(&mut self.thread, Thread::Stopped) {
            let _ = thread.join();
        }
    }
}

/// Which slots the compositor still holds.
struct PageState {
    busy: [bool; BUFFER_COUNT],
}

impl Page {
    /// # Safety
    ///
    /// `display` must be the live `wl_display` of GPUI's connection (the window's display handle).
    pub unsafe fn new(display: NonNull<c_void>, fps: u32, resize: bool) -> Result<Self, String> {
        // SAFETY: the caller passes GPUI's live wl_display, which outlives the app.
        let backend = unsafe {
            wayland_backend::client::Backend::from_foreign_display(display.as_ptr().cast())
        };
        let connection = Connection::from_backend(backend);
        let (globals, queue): (_, EventQueue<PageState>) =
            registry_queue_init(&connection).map_err(|error| format!("registry: {error}"))?;
        let qh = queue.handle();
        let compositor: wl_compositor::WlCompositor = globals
            .bind(&qh, 1..=4, ())
            .map_err(|error| format!("wl_compositor: {error}"))?;
        let shm: wl_shm::WlShm = globals
            .bind(&qh, 1..=1, ())
            .map_err(|error| format!("wl_shm: {error}"))?;
        let surface = compositor.create_surface(&qh, ());

        let pool_size = SLOT * BUFFER_COUNT;
        // SAFETY: memfd_create with a valid C string; a negative result is checked.
        let raw = unsafe { libc::memfd_create(c"native-view-page".as_ptr(), libc::MFD_CLOEXEC) };
        if raw < 0 {
            return Err("memfd_create failed".into());
        }
        // SAFETY: raw is a fresh fd this function owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: fd is a valid memfd; the size is positive.
        if unsafe { libc::ftruncate(raw, pool_size as libc::off_t) } != 0 {
            return Err("ftruncate failed".into());
        }
        // SAFETY: maps the whole memfd shared; MAP_FAILED is checked. The mapping lives as long as the thread.
        let memory = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                pool_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                raw,
                0,
            )
        };
        if memory == libc::MAP_FAILED {
            return Err("mmap failed".into());
        }
        let pool = shm.create_pool(fd.as_fd(), pool_size as i32, &qh, ());
        let buffers = SIZES.map(|(width, height)| {
            (0..BUFFER_COUNT)
                .map(|slot| {
                    pool.create_buffer(
                        (slot * SLOT) as i32,
                        width,
                        height,
                        width * 4,
                        wl_shm::Format::Xrgb8888,
                        &qh,
                        slot,
                    )
                })
                .collect()
        });
        let setup = Setup {
            connection,
            queue,
            buffers,
            memory: memory as usize,
            fd,
            period: Duration::from_secs_f64(1. / f64::from(fps.max(1))),
            resize,
        };
        Ok(Self {
            surface,
            frames: Arc::new(AtomicU64::new(0)),
            running: Arc::new(AtomicBool::new(true)),
            thread: Thread::Ready(Box::new(setup)),
        })
    }

    /// The `wl_proxy` GPUI's `NativeView::new` takes.
    pub fn proxy(&self) -> Option<NonNull<c_void>> {
        NonNull::new(self.surface.id().as_ptr().cast())
    }

    /// Starts committing frames to the view through its buffers.
    pub fn start(&mut self, view: NativeViewBuffers) -> Result<(), String> {
        let Thread::Ready(setup) = std::mem::replace(&mut self.thread, Thread::Stopped) else {
            return Err("the page has started already".into());
        };
        let (surface, frames, running) = (self.surface.clone(), self.frames.clone(), self.running.clone());
        let thread = thread::Builder::new()
            .name("page".into())
            .spawn(move || run(*setup, surface, view, frames, running))
            .map_err(|error| format!("page thread: {error}"))?;
        self.thread = Thread::Running(thread);
        Ok(())
    }
}

/// The page thread: a frame every `period`, drawn into a free slot and committed through the view's buffers.
fn run(
    setup: Setup,
    surface: wl_surface::WlSurface,
    view: NativeViewBuffers,
    frames: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
) {
    let Setup {
        connection,
        mut queue,
        buffers,
        memory,
        fd,
        period,
        resize,
    } = setup;
    let mut state = PageState {
        busy: [false; BUFFER_COUNT],
    };
    let mut next = Instant::now();
    for frame in 0u64.. {
        if !running.load(Ordering::Relaxed) {
            break;
        }
        // Reads buffer releases; a round trip also keeps the page from outrunning the compositor.
        if queue.roundtrip(&mut state).is_err() {
            log::error!("page: the Wayland connection is gone");
            return;
        }
        if let Some(slot) = state.busy.iter().position(|busy| !busy) {
            let which = size_index(frame, resize);
            let (width, height) = SIZES[which];
            // SAFETY: each slot of the mapping is only written while the compositor has released it.
            let pixels = unsafe {
                std::slice::from_raw_parts_mut((memory + slot * SLOT) as *mut u32, (width * height) as usize)
            };
            draw(pixels, frame, width, height);
            match view.commit_buffer(&surface, &buffers[which][slot], size(width, height)) {
                Committed::Attached => {
                    state.busy[slot] = true;
                    frames.fetch_add(1, Ordering::Relaxed);
                }
                Committed::NotShown => {}
                Committed::OtherSurface => {
                    log::error!("page: the view's buffers belong to another surface");
                    return;
                }
            }
        }
        let _ = connection.flush();
        next += period;
        thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    drop(fd);
}

/// Which of `SIZES` frame `frame` draws: the first, or with `--resize` the size switched every `RESIZE_EVERY`
/// frames, a switch's first `STALE_FRAMES` frames still of the size before.
fn size_index(frame: u64, resize: bool) -> usize {
    if !resize {
        return 0;
    }
    let lagged = frame.saturating_sub(STALE_FRAMES);
    ((lagged / RESIZE_EVERY) % 2) as usize
}

/// A frame that differs from the last everywhere: a hue that drifts, a white bar that sweeps across, a one-pixel
/// yellow frame on the buffer's edges, so a seam or a crop is easy to spot, and a yellow band across the top half
/// (rows 24 % to 26 % down), which a view cropped to its lower half must not show.
fn draw(pixels: &mut [u32], frame: u64, width: i32, height: i32) {
    let phase = (frame % 240) as u32;
    let background = 0x0020_4080 | ((phase * 255 / 240) << 16);
    let bar = (frame as i32 * 4 * width / SIZES[0].0) % width;
    let mark = height * 24 / 100..height * 26 / 100;
    for y in 0..height {
        for x in 0..width {
            let edge = x == 0 || y == 0 || x == width - 1 || y == height - 1 || mark.contains(&y);
            let color = if edge {
                0x00ff_ff00
            } else if (x - bar).abs() < 8 * width / SIZES[0].0 {
                0x00ff_ffff
            } else {
                background
            };
            pixels[(y * width + x) as usize] = color;
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, usize> for PageState {
    fn event(
        state: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        slot: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.busy[*slot] = false;
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for PageState {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

wayland_client::delegate_noop!(PageState: ignore wl_compositor::WlCompositor);
wayland_client::delegate_noop!(PageState: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(PageState: ignore wl_shm_pool::WlShmPool);
wayland_client::delegate_noop!(PageState: ignore wl_surface::WlSurface);
