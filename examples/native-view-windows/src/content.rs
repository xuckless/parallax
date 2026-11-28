//! The view's content, as an app such as a CEF browser makes it: a composition swap chain on the app's own D3D11
//! device, presented from the app's own thread. GPUI only holds the swap chain in its visual tree; DWM shows each
//! present without GPUI drawing or committing anything.

use std::{
    ffi::c_void,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

use windows::{
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
            },
            Dxgi::{
                Common::{
                    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN,
                    DXGI_SAMPLE_DESC,
                },
                CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
                DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIFactory2, IDXGISwapChain1,
            },
        },
    },
    core::{IUnknown, Interface},
};

/// The view's content, owned by its thread; the main thread asks for sizes and counts presents.
pub struct Content {
    /// The swap chain's size the view wants, in device pixels, `width << 32 | height`.
    size: Arc<AtomicU64>,
    /// Presents so far.
    pub presents: Arc<AtomicU64>,
}

/// A reference to the swap chain, for `NativeView::new`, which takes its own.
pub struct SwapChain(IUnknown);

impl SwapChain {
    pub fn as_ptr(&self) -> NonNull<c_void> {
        // SAFETY: a live COM interface's pointer is never null.
        unsafe { NonNull::new_unchecked(self.0.as_raw()) }
    }
}

/// The colour of frame `n`: green that changes its blue every frame. The checks look for green with little red.
pub fn colour(n: u64) -> [f32; 4] {
    [16. / 255., 192. / 255., (n % 64) as f32 / 255., 1.]
}

impl Content {
    /// Starts the content thread with a swap chain of `width` × `height` device pixels.
    pub fn start(width: u32, height: u32) -> anyhow::Result<(Self, SwapChain)> {
        let size = Arc::new(AtomicU64::new(pack(width, height)));
        let presents = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = mpsc::channel();
        let thread_size = size.clone();
        let thread_presents = presents.clone();
        std::thread::spawn(move || {
            let started = SwapChainThread::new(width, height);
            let thread = match started {
                Ok(thread) => {
                    // The interface is free-threaded, so its reference may cross to the main thread as a raw pointer.
                    let raw = thread
                        .swap_chain
                        .cast::<IUnknown>()
                        .map(|unknown| unknown.into_raw() as usize);
                    sender.send(raw).ok();
                    thread
                }
                Err(error) => {
                    sender.send(Err(error)).ok();
                    return;
                }
            };
            if let Err(error) = thread.run(&thread_size, &thread_presents) {
                log::error!("view content thread stopped: {error}");
            }
        });
        let raw = receiver.recv()??;
        // SAFETY: `raw` is the reference `into_raw` gave up above, taken back once here.
        let swap_chain = SwapChain(unsafe { IUnknown::from_raw(raw as *mut c_void) });
        Ok((Self { size, presents }, swap_chain))
    }

    /// Asks the thread to resize the swap chain; it does so before its next present.
    pub fn resize(&self, width: u32, height: u32) {
        self.size.store(pack(width, height), Ordering::Relaxed);
    }
}

fn pack(width: u32, height: u32) -> u64 {
    (u64::from(width) << 32) | u64::from(height)
}

struct SwapChainThread {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
}

impl SwapChainThread {
    fn new(width: u32, height: u32) -> Result<Self, windows::core::Error> {
        // A hardware device where there is one, else WARP, as on CI's runners.
        let (device, context) = [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP]
            .into_iter()
            .map(create_device)
            .find_map(Result::ok)
            .ok_or_else(windows::core::Error::empty)?;
        // SAFETY: plain factory creation.
        let factory: IDXGIFactory2 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }?;
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            // The content is opaque; GPUI's own swap chain is what blends over it.
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            ..Default::default()
        };
        // SAFETY: `device` and `desc` are live for the call.
        let swap_chain = unsafe { factory.CreateSwapChainForComposition(&device, &desc, None) }?;
        Ok(Self {
            device,
            context,
            swap_chain,
        })
    }

    /// Clears the back buffer to the next colour and presents, every vsync, resizing when asked.
    fn run(&self, size: &AtomicU64, presents: &AtomicU64) -> Result<(), windows::core::Error> {
        let mut current = size.load(Ordering::Relaxed);
        for n in 0.. {
            let wanted = size.load(Ordering::Relaxed);
            if wanted != current {
                current = wanted;
                // SAFETY: no view of the buffers is alive here; they are made per frame below.
                unsafe {
                    self.swap_chain.ResizeBuffers(
                        0,
                        (wanted >> 32) as u32,
                        wanted as u32,
                        DXGI_FORMAT_UNKNOWN,
                        DXGI_SWAP_CHAIN_FLAG(0),
                    )
                }?;
                log::info!(
                    "view content: resized to {}x{}",
                    wanted >> 32,
                    wanted as u32
                );
            }
            // SAFETY: the back buffer and its view live only for this frame, on this thread's device and context.
            unsafe {
                let buffer: ID3D11Texture2D = self.swap_chain.GetBuffer(0)?;
                let mut view: Option<ID3D11RenderTargetView> = None;
                self.device
                    .CreateRenderTargetView(&buffer, None, Some(&mut view))?;
                let view = view.ok_or_else(windows::core::Error::empty)?;
                self.context.ClearRenderTargetView(&view, &colour(n));
                self.swap_chain.Present(1, DXGI_PRESENT(0)).ok()?;
            }
            presents.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

fn create_device(
    driver: D3D_DRIVER_TYPE,
) -> Result<(ID3D11Device, ID3D11DeviceContext), windows::core::Error> {
    let mut device = None;
    let mut context = None;
    // SAFETY: the out pointers outlive the call.
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }?;
    Ok((
        device.ok_or_else(windows::core::Error::empty)?,
        context.ok_or_else(windows::core::Error::empty)?,
    ))
}
