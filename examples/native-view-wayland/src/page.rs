//! The "page": what a browser engine would be. It owns one `wl_surface` on GPUI's own Wayland connection, on its
//! own event queue and thread, and commits a new shm buffer to it about `fps` times a second, without GPUI.

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

/// The buffer the page draws, in pixels. The compositor scales it to the view's size (wp_viewport).
pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 400;
/// Enough buffers that one is free while the compositor holds the others.
const BUFFER_COUNT: usize = 4;
const STRIDE: i32 = WIDTH * 4;

pub struct Page {
    /// The surface the app hands to `NativeView::new`.
    pub surface: wl_surface::WlSurface,
    /// Buffers committed so far, read by the example's frame counter.
    pub frames: Arc<AtomicU64>,
    /// The thread uses GPUI's connection, so it must stop before GPUI disconnects: dropping the page stops it.
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Page {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Which of the page's buffers the compositor still holds.
struct PageState {
    busy: [bool; BUFFER_COUNT],
}

impl Page {
    /// # Safety
    ///
    /// `display` must be the live `wl_display` of GPUI's connection (the window's display handle).
    pub unsafe fn new(display: NonNull<c_void>, fps: u32) -> Result<Self, String> {
        // SAFETY: the caller passes GPUI's live wl_display, which outlives the app.
        let backend = unsafe {
            wayland_backend::client::Backend::from_foreign_display(display.as_ptr().cast())
        };
        let connection = Connection::from_backend(backend);
        let (globals, mut queue): (_, EventQueue<PageState>) =
            registry_queue_init(&connection).map_err(|error| format!("registry: {error}"))?;
        let qh = queue.handle();
        let compositor: wl_compositor::WlCompositor = globals
            .bind(&qh, 1..=4, ())
            .map_err(|error| format!("wl_compositor: {error}"))?;
        let shm: wl_shm::WlShm = globals
            .bind(&qh, 1..=1, ())
            .map_err(|error| format!("wl_shm: {error}"))?;
        let surface = compositor.create_surface(&qh, ());

        let size = STRIDE as usize * HEIGHT as usize;
        let pool_size = size * BUFFER_COUNT;
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
        let buffers: Vec<wl_buffer::WlBuffer> = (0..BUFFER_COUNT)
            .map(|index| {
                pool.create_buffer(
                    (index * size) as i32,
                    WIDTH,
                    HEIGHT,
                    STRIDE,
                    wl_shm::Format::Xrgb8888,
                    &qh,
                    index,
                )
            })
            .collect();

        let frames = Arc::new(AtomicU64::new(0));
        let running = Arc::new(AtomicBool::new(true));
        let page_surface = surface.clone();
        let (page_frames, page_running) = (frames.clone(), running.clone());
        let memory = memory as usize;
        let period = Duration::from_secs_f64(1. / f64::from(fps.max(1)));
        let thread = thread::Builder::new()
            .name("page".into())
            .spawn(move || {
                let (surface, frames) = (page_surface, page_frames);
                let mut state = PageState {
                    busy: [false; BUFFER_COUNT],
                };
                let mut next = Instant::now();
                for frame in 0u64.. {
                    if !page_running.load(Ordering::Relaxed) {
                        break;
                    }
                    // Reads buffer releases; a round trip also keeps the page from outrunning the compositor.
                    if queue.roundtrip(&mut state).is_err() {
                        return;
                    }
                    if let Some(index) = state.busy.iter().position(|busy| !busy) {
                        // SAFETY: each buffer's slice of the mapping is only written while the compositor has
                        // released it.
                        let pixels = unsafe {
                            std::slice::from_raw_parts_mut(
                                (memory + index * size) as *mut u32,
                                size / 4,
                            )
                        };
                        draw(pixels, frame);
                        surface.attach(Some(&buffers[index]), 0, 0);
                        surface.damage_buffer(0, 0, WIDTH, HEIGHT);
                        surface.commit();
                        state.busy[index] = true;
                        frames.fetch_add(1, Ordering::Relaxed);
                    }
                    let _ = connection.flush();
                    next += period;
                    thread::sleep(next.saturating_duration_since(Instant::now()));
                }
                drop(fd);
            })
            .map_err(|error| format!("page thread: {error}"))?;
        Ok(Self {
            surface,
            frames,
            running,
            thread: Some(thread),
        })
    }

    /// The `wl_proxy` GPUI's `NativeView::new` takes.
    pub fn proxy(&self) -> Option<NonNull<c_void>> {
        NonNull::new(self.surface.id().as_ptr().cast())
    }
}

/// A frame that differs from the last everywhere: a hue that drifts, a white bar that sweeps across, and a
/// one-pixel yellow frame on the buffer's edges, so a seam or a crop is easy to spot.
fn draw(pixels: &mut [u32], frame: u64) {
    let phase = (frame % 240) as u32;
    let background = 0x0020_4080 | ((phase * 255 / 240) << 16);
    let bar = (frame as i32 * 4) % WIDTH;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let edge = x == 0 || y == 0 || x == WIDTH - 1 || y == HEIGHT - 1;
            let color = if edge {
                0x00ff_ff00
            } else if (x - bar).abs() < 8 {
                0x00ff_ffff
            } else {
                background
            };
            pixels[(y * WIDTH + x) as usize] = color;
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, usize> for PageState {
    fn event(
        state: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.busy[*index] = false;
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
