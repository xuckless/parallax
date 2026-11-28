//! The native window and the `gpui_wgpu` renderer on it. The GPU backends, tried in order, come from the device's
//! row in the backend table ([`Device::backends`]).

use std::{ffi::CStr, ptr::NonNull, time::Instant};

use gpui::{DevicePixels, Size, size};
use gpui_wgpu::{GpuContext, WgpuContext, WgpuRenderer, WgpuSurfaceConfig, wgpu};
use raw_window_handle as rwh;

use crate::{
    Error,
    backend::{Backend, Device},
};

/// This device, from `ro.hardware` (Java's `Build.HARDWARE`); unreadable counts as real hardware. The FFI edge of the
/// pure [`Device::from_hardware`]: the shell reads it once, when it is made, and passes it to [`create`] (and its
/// backends to [`recover`]).
pub(crate) fn device() -> Device {
    let mut value = [0u8; libc::PROP_VALUE_MAX as usize];
    // SAFETY: the name is NUL-terminated and `value` holds PROP_VALUE_MAX bytes, the most the call writes
    // (NUL included).
    unsafe { libc::__system_property_get(c"ro.hardware".as_ptr(), value.as_mut_ptr().cast()) };
    let hardware = CStr::from_bytes_until_nul(&value)
        .ok()
        .and_then(|hardware| hardware.to_str().ok())
        .unwrap_or_default();
    Device::from_hardware(hardware)
}

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

/// The one place a [`Backend`] becomes wgpu's.
impl From<Backend> for wgpu::Backends {
    fn from(backend: Backend) -> Self {
        match backend {
            Backend::Vulkan => wgpu::Backends::VULKAN,
            Backend::Gl => wgpu::Backends::GL,
        }
    }
}

pub(crate) fn surface_config(size: Size<DevicePixels>) -> WgpuSurfaceConfig {
    WgpuSurfaceConfig {
        size,
        transparent: false,
        preferred_present_mode: None,
    }
}

/// This device's API level (`ro.build.version.sdk`, Java's `Build.VERSION.SDK_INT`); 0 when unreadable.
pub(crate) fn api_level() -> u32 {
    let mut value = [0u8; libc::PROP_VALUE_MAX as usize];
    // SAFETY: as in `device`.
    unsafe {
        libc::__system_property_get(c"ro.build.version.sdk".as_ptr(), value.as_mut_ptr().cast())
    };
    CStr::from_bytes_until_nul(&value)
        .ok()
        .and_then(|level| level.to_str().ok())
        .and_then(|level| level.parse().ok())
        .unwrap_or(0)
}

/// The backend of `gpu`'s context, if it holds one.
pub(crate) fn backend(gpu: &GpuContext) -> Option<Backend> {
    match gpu.borrow().as_ref()?.adapter.get_info().backend {
        wgpu::Backend::Vulkan => Some(Backend::Vulkan),
        wgpu::Backend::Gl => Some(Backend::Gl),
        wgpu::Backend::Noop
        | wgpu::Backend::Metal
        | wgpu::Backend::Dx12
        | wgpu::Backend::BrowserWebGpu => None,
    }
}

/// Seeds `gpu` with a context from the first of `backends` that can configure the surface, unless it already holds a
/// live one on the first (another window recovered or switched first: this one adopts it).
fn seed(
    gpu: &GpuContext,
    backends: [Backend; 2],
    window: &NativeWindow,
    size: Size<DevicePixels>,
) -> Result<(), Error> {
    let live = gpu
        .borrow()
        .as_ref()
        .is_some_and(|context| !context.device_lost());
    if live && backend(gpu) == Some(backends[0]) {
        return Ok(());
    }
    // The old context goes first, so its device is released before the next one is made (as upstream's recover).
    *gpu.borrow_mut() = None;
    let context = backends
        .into_iter()
        .find_map(|backend| {
            let attempt = || -> anyhow::Result<WgpuContext> {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: backend.into(),
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
                    log::warn!("parallax: {backend:?} cannot drive the surface: {error:#}")
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
    device: Device,
    window: &NativeWindow,
    size: Size<DevicePixels>,
) -> Result<WgpuRenderer, Error> {
    let started = Instant::now();
    seed(gpu, device.backends(), window, size)?;
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

/// Rebuilds `renderer` on the first of `backends` that drives the surface, keeping its atlas: after GPU device loss
/// (`Device::backends`; the surface state machine decides when, after upstream's 350 ms, backing off on error), or
/// when native views first show on GL (`backend::for_native_views`). Upstream's `WgpuRenderer::recover` would pick
/// the backend itself when the shared context is lost; seeding a live context first makes it adopt ours instead.
pub(crate) fn recover(
    gpu: &GpuContext,
    backends: [Backend; 2],
    renderer: &mut WgpuRenderer,
    window: &NativeWindow,
    size: Size<DevicePixels>,
) -> Result<(), Error> {
    let started = Instant::now();
    // Drops the renderer's surface and its hold on the lost device before a new device is made. Upstream's recover
    // builds its surface at the renderer's recorded size: record the current one, so a resize while recovering
    // needs no second surface.
    renderer.destroy();
    renderer.update_drawable_size(size);
    seed(gpu, backends, window, size)?;
    renderer
        .recover(window)
        .map_err(|error| Error::Renderer(format!("{error:#}")))?;
    log::info!(
        "parallax: renderer rebuilt in {:?}: {:?}",
        started.elapsed(),
        gpu.borrow()
            .as_ref()
            .map(|context| context.adapter.get_info().backend),
    );
    Ok(())
}
