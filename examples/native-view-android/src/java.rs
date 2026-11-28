//! The example's Android side over JNI: the views it hands to `NativeView::new`, and the producer that fills the
//! surface from its own thread, as a video decoder or a browser engine would.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use parallax::parallax_android::jni::{
    self, JNIEnv, JavaVM,
    objects::{GlobalRef, JObject, JValue},
};

/// The page the WebView loads: tall enough to scroll, a button that turns colour on tap. Its console goes to logcat
/// (tag `chromium`).
const PAGE: &str = r#"<!doctype html><html><head><meta name="viewport" content="width=device-width">
<style>body{font:18px sans-serif;margin:0;background:#fffbe6}p{margin:0;padding:12px 16px;border-bottom:1px solid #ddd}
button{font-size:22px;margin:16px;padding:16px 28px;background:#2a7}</style></head><body>
<button id="b" onclick="this.style.background=this.style.background=='orange'?'#2a7':'orange';console.log('page: tapped')">Tap me</button>
<script>
for(let i=1;i<=60;i++){const p=document.createElement('p');p.textContent='Row '+i+' of a local page in a WebView';document.body.appendChild(p)}
let last=0;addEventListener('scroll',()=>{const y=Math.round(scrollY);if(Math.abs(y-last)>=100){last=y;console.log('page: scrollY '+y)}});
</script></body></html>"#;

/// What the surface is: `SurfaceControl` needs API 29.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceKind {
    /// A `SurfaceControl` with a buffer, made a child of GPUI's surface (API 29+).
    Control,
    /// An ordinary `SurfaceView`, laid out under GPUI's view.
    View,
}

/// The intent's extras: `surface` (`view` or `control`) and `animate`.
pub struct Options {
    pub surface: SurfaceKind,
    pub animate: bool,
}

/// The objects handed to `NativeView::new`, and the surface the producer fills.
pub struct Views {
    pub surface_view: GlobalRef,
    pub surface: GlobalRef,
    pub web_view: GlobalRef,
}

pub fn options(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<Options> {
    let sdk = env
        .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()?;
    let intent = env
        .call_method(activity, "getIntent", "()Landroid/content/Intent;", &[])?
        .l()?;
    let key = env.new_string("surface")?;
    let surface = env
        .call_method(
            &intent,
            "getStringExtra",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&key)],
        )?
        .l()?;
    let surface: Option<String> = if surface.is_null() {
        None
    } else {
        Some(env.get_string(&surface.into())?.into())
    };
    let key = env.new_string("animate")?;
    let animate = env
        .call_method(
            &intent,
            "getBooleanExtra",
            "(Ljava/lang/String;Z)Z",
            &[JValue::Object(&key), JValue::Bool(0)],
        )?
        .z()?;
    let surface = match (surface.as_deref(), sdk) {
        (Some("view"), _) | (_, ..29) => SurfaceKind::View,
        _ => SurfaceKind::Control,
    };
    Ok(Options { surface, animate })
}

pub fn views(
    env: &mut JNIEnv,
    activity: &JObject,
    kind: SurfaceKind,
) -> jni::errors::Result<Views> {
    let (surface_view, surface) = match kind {
        SurfaceKind::Control => {
            let builder = env.new_object("android/view/SurfaceControl$Builder", "()V", &[])?;
            let name = env.new_string("native-view-android surface")?;
            let returns_builder =
                |args: &str| format!("({args})Landroid/view/SurfaceControl$Builder;");
            env.call_method(
                &builder,
                "setName",
                returns_builder("Ljava/lang/String;"),
                &[JValue::Object(&name)],
            )?;
            // Any size: GPUI's frames give the buffer the slot's size (NativeViews.place).
            env.call_method(
                &builder,
                "setBufferSize",
                returns_builder("II"),
                &[JValue::Int(64), JValue::Int(64)],
            )?;
            let control = env
                .call_method(&builder, "build", "()Landroid/view/SurfaceControl;", &[])?
                .l()?;
            let surface = env.new_object(
                "android/view/Surface",
                "(Landroid/view/SurfaceControl;)V",
                &[JValue::Object(&control)],
            )?;
            (control, surface)
        }
        SurfaceKind::View => {
            let view = env.new_object(
                "android/view/SurfaceView",
                "(Landroid/content/Context;)V",
                &[JValue::Object(activity)],
            )?;
            let holder = env
                .call_method(&view, "getHolder", "()Landroid/view/SurfaceHolder;", &[])?
                .l()?;
            let surface = env
                .call_method(&holder, "getSurface", "()Landroid/view/Surface;", &[])?
                .l()?;
            (view, surface)
        }
    };
    let web_view = env.new_object(
        "android/webkit/WebView",
        "(Landroid/content/Context;)V",
        &[JValue::Object(activity)],
    )?;
    let settings = env
        .call_method(
            &web_view,
            "getSettings",
            "()Landroid/webkit/WebSettings;",
            &[],
        )?
        .l()?;
    env.call_method(
        &settings,
        "setJavaScriptEnabled",
        "(Z)V",
        &[JValue::Bool(1)],
    )?;
    let (page, mime, encoding) = (
        env.new_string(PAGE)?,
        env.new_string("text/html")?,
        env.new_string("utf-8")?,
    );
    let null = JObject::null();
    env.call_method(
        &web_view,
        "loadDataWithBaseURL",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&null),
            JValue::Object(&page),
            JValue::Object(&mime),
            JValue::Object(&encoding),
            JValue::Object(&null),
        ],
    )?;
    Ok(Views {
        surface_view: env.new_global_ref(surface_view)?,
        surface: env.new_global_ref(surface)?,
        web_view: env.new_global_ref(web_view)?,
    })
}

/// Fills the surface about 60 times a second with the next colour of a hue cycle and a white bar that moves one step
/// per frame, from its own thread, never through GPUI. Stops when dropped.
pub struct Producer {
    pub frames: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Producer {
    pub fn start(vm: JavaVM, surface: GlobalRef) -> Self {
        let frames = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (counted, stopped) = (frames.clone(), stop.clone());
        thread::spawn(move || {
            let mut env = match vm.attach_current_thread_permanently() {
                Ok(env) => env,
                Err(error) => return log::error!("producer: cannot attach to the VM: {error}"),
            };
            while !stopped.load(Ordering::Relaxed) {
                if fill(&mut env, &surface, counted.load(Ordering::Relaxed)) {
                    counted.fetch_add(1, Ordering::Relaxed);
                }
                thread::sleep(Duration::from_millis(16));
            }
        });
        Self { frames, stop }
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// One frame into `surface`, if it has a buffer now (a hidden SurfaceView has none). Returns whether it posted.
fn fill(env: &mut JNIEnv, surface: &GlobalRef, frame: u64) -> bool {
    let valid = env
        .call_method(surface.as_obj(), "isValid", "()Z", &[])
        .and_then(|valid| valid.z())
        .unwrap_or(false);
    if !valid {
        return false;
    }
    // SAFETY: `surface` is a live android.view.Surface; fromSurface returns a new reference or null.
    let window = unsafe {
        ndk_sys::ANativeWindow_fromSurface(env.get_raw().cast(), surface.as_obj().as_raw().cast())
    };
    if window.is_null() {
        return false;
    }
    let mut buffer = ndk_sys::ANativeWindow_Buffer {
        width: 0,
        height: 0,
        stride: 0,
        format: 0,
        bits: std::ptr::null_mut(),
        reserved: [0; 6],
    };
    // SAFETY: `window` is the reference acquired above, released below; a locked buffer's `bits` hold `stride ×
    // height` RGBA pixels until unlockAndPost.
    unsafe {
        ndk_sys::ANativeWindow_setBuffersGeometry(
            window,
            0,
            0,
            ndk_sys::ANativeWindow_LegacyFormat::WINDOW_FORMAT_RGBA_8888.0 as i32,
        );
        let locked = ndk_sys::ANativeWindow_lock(window, &mut buffer, std::ptr::null_mut()) == 0;
        if locked {
            let (width, height, stride) = (
                buffer.width.max(0) as usize,
                buffer.height.max(0) as usize,
                buffer.stride.max(0) as usize,
            );
            let pixels = std::slice::from_raw_parts_mut(buffer.bits.cast::<u32>(), stride * height);
            let colour = rgba(frame);
            let bar = (frame as usize * 4) % width.max(1);
            for row in pixels.chunks_exact_mut(stride) {
                row[..width].fill(colour);
                row[bar..(bar + 12).min(width)].fill(0xffff_ffff);
            }
            ndk_sys::ANativeWindow_unlockAndPost(window);
        }
        ndk_sys::ANativeWindow_release(window);
        locked
    }
}

/// Frame `frame`'s colour as RGBA bytes in memory order (little-endian `u32`): a hue cycle, never magenta.
fn rgba(frame: u64) -> u32 {
    let hue = 90. + (frame % 240) as f32 / 2.; // 90°..210°: greens, cyans, blues
    let x = 1. - ((hue / 60.) % 2. - 1.).abs();
    let (r, g, b) = match (hue / 60.) as u32 {
        1 => (x, 1., 0.),
        2 => (0., 1., x),
        _ => (0., x, 1.),
    };
    let byte = |value: f32| (value * 255.) as u32;
    0xff00_0000 | byte(b) << 16 | byte(g) << 8 | byte(r)
}
