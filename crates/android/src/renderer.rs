//! The native window and the `gpui_wgpu` renderer on it. The GPU backend comes from [`BACKENDS`], tried in order.

use std::{ptr::NonNull, time::Instant};

use gpui::{DevicePixels, Size, size};
use gpui_wgpu::{GpuContext, WgpuContext, WgpuRenderer, WgpuSurfaceConfig, wgpu};
use raw_window_handle as rwh;

use crate::Error;

/// GPU backends in order of preference; the first that can drive the surface wins, at startup and after device
/// loss ([`recover`]). To change the order or add a backend, edit this table.
const BACKENDS: [wgpu::Backends; 2] = [wgpu::Backends::VULKAN, wgpu::Backends::GL];

/// An `ANativeWindow` reference. Each clone holds its own reference; drop releases it.
#[derive(Debug)]
pub(crate) struct NativeWindow(NonNull<ndk_sys::ANativeWindow>);

// SAFETY: `ANativeWindow` is reference counted, and acquire, release and size queries are thread-safe.
unsafe impl Send for NativeWindow {}
// SAFETY: as above; `NativeWindow` exposes no mutation of the window itself.
unsafe impl Sync for NativeWindow {}

impl NativeWindow {
    /// Takes ownership of one reference, e.g. from `ANativeWindow_fromSurface`. `None` for a null pointer.
    pub(crate) fn from_acquired(window: *mut ndk_sys::ANativeWindow) -> Option<Self> {
        NonNull::new(window).map(Self)
    }

    /// The window's buffer size in physical pixels.
    pub(crate) fn size(&self) -> Size<DevicePixels> {
        // SAFETY: `self.0` is a live reference to an ANativeWindow, held until drop.
        let (width, height) = unsafe {
            (
                ndk_sys::ANativeWindow_getWidth(self.0.as_ptr()),
                ndk_sys::ANativeWindow_getHeight(self.0.as_ptr()),
            )
        };
        size(DevicePixels(width), DevicePixels(height))
    }
}

impl Clone for NativeWindow {
    fn clone(&self) -> Self {
        // SAFETY: `self.0` is live; acquiring adds the reference the clone releases on drop.
        unsafe { ndk_sys::ANativeWindow_acquire(self.0.as_ptr()) };
        Self(self.0)
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        // SAFETY: this value owns exactly one reference, released once here.
        unsafe { ndk_sys::ANativeWindow_release(self.0.as_ptr()) };
    }
}

impl rwh::HasWindowHandle for NativeWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let handle = rwh::AndroidNdkWindowHandle::new(self.0.cast());
        // SAFETY: the ANativeWindow stays alive while `self` is borrowed.
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}

impl rwh::HasDisplayHandle for NativeWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::android())
    }
}

/// The display handle for a wgpu instance. It carries no data, so the instance holds no window reference.
#[derive(Debug)]
struct AndroidDisplay;

impl rwh::HasDisplayHandle for AndroidDisplay {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::android())
    }
}

pub(crate) fn surface_config(size: Size<DevicePixels>) -> WgpuSurfaceConfig {
    WgpuSurfaceConfig {
        size,
        transparent: false,
        preferred_present_mode: None,
    }
}

/// Seeds `gpu` with a context from the first backend in [`BACKENDS`] that can configure the surface, unless it
/// already holds a live one.
fn seed(gpu: &GpuContext, window: &NativeWindow, size: Size<DevicePixels>) -> Result<(), Error> {
    if gpu
        .borrow()
        .as_ref()
        .is_some_and(|context| !context.device_lost())
    {
        return Ok(());
    }
    // A lost context goes first, so its device is released before the next one is made (as upstream's recover).
    *gpu.borrow_mut() = None;
    let context = BACKENDS
        .iter()
        .find_map(|&backends| {
            let attempt = || -> anyhow::Result<WgpuContext> {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends,
                    flags: wgpu::InstanceFlags::default(),
                    backend_options: wgpu::BackendOptions::default(),
                    memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
                    display: Some(Box::new(AndroidDisplay)),
                });
                let target = wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: None,
                    raw_window_handle: rwh::HasWindowHandle::window_handle(window)?.as_raw(),
                };
                // SAFETY: `window` outlives this probe surface, dropped on return; the renderer makes its own.
                let surface = unsafe { instance.create_surface_unsafe(target)? };
                WgpuContext::new(instance, &surface, &surface_config(size), None)
            };
            attempt()
                .inspect_err(|error| {
                    log::warn!("parallax: {backends:?} cannot drive the surface: {error:#}")
                })
                .ok()
        })
        .ok_or(Error::NoBackend)?;
    *gpu.borrow_mut() = Some(context);
    Ok(())
}

/// Creates the renderer on `window`, on the calling (main) thread, on the context [`seed`] picked.
pub(crate) fn create(
    gpu: &GpuContext,
    window: &NativeWindow,
    size: Size<DevicePixels>,
) -> Result<WgpuRenderer, Error> {
    let started = Instant::now();
    seed(gpu, window, size)?;
    let renderer = WgpuRenderer::new(gpu.clone(), window, surface_config(size), None)
        .map_err(|error| Error::Renderer(format!("{error:#}")))?;
    log::info!(
        "parallax: renderer created on the main thread in {:?}: {:?}, {:?}",
        started.elapsed(),
        gpu.borrow()
            .as_ref()
            .map(|context| context.adapter.get_info().backend),
        renderer.gpu_specs().map(|specs| specs.device_name),
    );
    Ok(renderer)
}

/// Rebuilds `renderer` after GPU device loss, keeping its atlas. Upstream's `WgpuRenderer::recover` would pick the
/// backend itself when the shared context is lost; seeding a live context from [`BACKENDS`] first makes it adopt
/// ours instead. On error the renderer still reports its device lost, so the next frame tries again.
pub(crate) fn recover(
    gpu: &GpuContext,
    renderer: &mut WgpuRenderer,
    window: &NativeWindow,
    size: Size<DevicePixels>,
) -> Result<(), Error> {
    let started = Instant::now();
    // Drops the renderer's surface and its hold on the lost device before a new device is made.
    renderer.destroy();
    seed(gpu, window, size)?;
    renderer
        .recover(window)
        .map_err(|error| Error::Renderer(format!("{error:#}")))?;
    log::info!(
        "parallax: GPU device recovered in {:?}: {:?}",
        started.elapsed(),
        gpu.borrow()
            .as_ref()
            .map(|context| context.adapter.get_info().backend),
    );
    Ok(())
}
