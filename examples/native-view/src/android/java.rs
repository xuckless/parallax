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
    objects::{GlobalRef, JObject, JObjectArray, JValue},
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

/// What the surface is: `SurfaceControl` needs API 29, `InSurface` hosting API 34.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceKind {
    /// A `SurfaceControl` with a buffer, made a child of GPUI's surface (API 29+).
    Control,
    /// An ordinary `SurfaceView`, laid out under GPUI's view.
    View,
    /// A `SurfaceView` in a `SurfaceControlViewHost` of its own (`parallax_android::in_surface`, API 34+).
    Hosted,
}

/// The objects handed to `NativeView::new`, and the surface the producer fills.
pub struct Views {
    /// The kind made: below API 29 always `View`.
    pub kind: SurfaceKind,
    pub surface_view: GlobalRef,
    pub surface: GlobalRef,
    /// The WebView, or with `stack` the two tiles in attach order (the top one first), each with the surface its
    /// producer fills, if it has one.
    pub second: Second,
}

pub enum Second {
    Web(GlobalRef),
    Tiles([(GlobalRef, Option<GlobalRef>); 2]),
}

/// The tiles' names and colours (RGB), in attach order: the yellow one is attached first but painted last, so it is
/// on top.
pub const TILE_COLOURS: [(&str, u32); 2] = [("yellow", 0xe0c020), ("red", 0xe03030)];

/// The activity's intent extras, each as `String.valueOf` of its value (`true` for `--ez name true`).
pub fn extras(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<Vec<(String, String)>> {
    let intent = env
        .call_method(activity, "getIntent", "()Landroid/content/Intent;", &[])?
        .l()?;
    let bundle = env
        .call_method(&intent, "getExtras", "()Landroid/os/Bundle;", &[])?
        .l()?;
    if bundle.is_null() {
        return Ok(Vec::new());
    }
    let keys = env
        .call_method(&bundle, "keySet", "()Ljava/util/Set;", &[])?
        .l()?;
    let keys: JObjectArray = env
        .call_method(&keys, "toArray", "()[Ljava/lang/Object;", &[])?
        .l()?
        .into();
    let mut extras = Vec::new();
    for at in 0..env.get_array_length(&keys)? {
        let key = env.get_object_array_element(&keys, at)?;
        let value = env
            .call_method(
                &bundle,
                "get",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[JValue::Object(&key)],
            )?
            .l()?;
        let value = env
            .call_static_method(
                "java/lang/String",
                "valueOf",
                "(Ljava/lang/Object;)Ljava/lang/String;",
                &[JValue::Object(&value)],
            )?
            .l()?;
        let key: String = env.get_string(&key.into())?.into();
        let value: String = env.get_string(&value.into())?.into();
        extras.push((key, value));
    }
    Ok(extras)
}

pub fn views(
    env: &mut JNIEnv,
    activity: &JObject,
    kind: SurfaceKind,
    stack: bool,
) -> jni::errors::Result<Views> {
    let sdk = env
        .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()?;
    let kind = if sdk < 29 { SurfaceKind::View } else { kind };
    let (surface_view, surface) = native_surface(env, activity, kind)?;
    let second = if stack {
        // A SurfaceControl with a producer, or a plain coloured View: a SurfaceView's surface stacks by Android's own
        // SurfaceView rules, not by the View's Z (docs/native-view-android-hosting.md).
        let mut tile = |colour: u32| -> jni::errors::Result<(GlobalRef, Option<GlobalRef>)> {
            if kind == SurfaceKind::Control {
                let (object, surface) = native_surface(env, activity, SurfaceKind::Control)?;
                return Ok((
                    env.new_global_ref(object)?,
                    Some(env.new_global_ref(surface)?),
                ));
            }
            let view = env.new_object(
                "android/view/View",
                "(Landroid/content/Context;)V",
                &[JValue::Object(activity)],
            )?;
            let argb = (0xff00_0000 | colour) as i32;
            env.call_method(&view, "setBackgroundColor", "(I)V", &[JValue::Int(argb)])?;
            Ok((env.new_global_ref(view)?, None))
        };
        Second::Tiles([tile(TILE_COLOURS[0].1)?, tile(TILE_COLOURS[1].1)?])
    } else {
        let web_view = web_view(env, activity)?;
        Second::Web(env.new_global_ref(web_view)?)
    };
    Ok(Views {
        kind,
        surface_view: env.new_global_ref(surface_view)?,
        surface: env.new_global_ref(surface)?,
        second,
    })
}

/// A native view of `kind` and the `Surface` a producer fills: (what `NativeView::new` takes, its surface).
fn native_surface<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject,
    kind: SurfaceKind,
) -> jni::errors::Result<(JObject<'local>, JObject<'local>)> {
    Ok(match kind {
        SurfaceKind::Control => {
            let builder = env.new_object("android/view/SurfaceControl$Builder", "()V", &[])?;
            let name = env.new_string("native-view surface")?;
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
        SurfaceKind::View | SurfaceKind::Hosted => {
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
            if kind == SurfaceKind::Hosted {
                let hosted = parallax::parallax_android::in_surface(env, &view)
                    .map_err(|_| jni::errors::Error::JavaException)?;
                (hosted, surface)
            } else {
                (view, surface)
            }
        }
    })
}

/// The WebView with the local page.
fn web_view<'local>(
    env: &mut JNIEnv<'local>,
    activity: &JObject,
) -> jni::errors::Result<JObject<'local>> {
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
    Ok(web_view)
}

/// What a producer fills each frame with.
#[derive(Clone, Copy)]
pub enum Fill {
    /// The surface's: a new colour every frame (`rgba`), with a white bar that moves one step per frame.
    Changing,
    /// A tile's: one colour (RGB).
    Solid(u32),
}

/// Fills a surface about 60 times a second from its own thread, never through GPUI. Stops when dropped.
pub struct Producer {
    pub frames: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Producer {
    pub fn start(vm: Arc<JavaVM>, surface: GlobalRef, paint: Fill) -> Self {
        let frames = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (counted, stopped) = (frames.clone(), stop.clone());
        thread::spawn(move || {
            let mut env = match vm.attach_current_thread_permanently() {
                Ok(env) => env,
                Err(error) => return log::error!("producer: cannot attach to the VM: {error}"),
            };
            while !stopped.load(Ordering::Relaxed) {
                if fill(&mut env, &surface, paint, counted.load(Ordering::Relaxed)) {
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
fn fill(env: &mut JNIEnv, surface: &GlobalRef, paint: Fill, frame: u64) -> bool {
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
            let (colour, bar) = match paint {
                Fill::Changing => (rgba(frame), Some((frame as usize * 4) % width.max(1))),
                Fill::Solid(rgb) => (0xff00_0000 | rgb.swap_bytes() >> 8, None),
            };
            for row in pixels.chunks_exact_mut(stride) {
                row[..width].fill(colour);
                if let Some(bar) = bar {
                    row[bar..(bar + 12).min(width)].fill(0xffff_ffff);
                }
            }
            ndk_sys::ANativeWindow_unlockAndPost(window);
        }
        ndk_sys::ANativeWindow_release(window);
        locked
    }
}

/// Frame `frame`'s colour as RGBA bytes in memory order (little-endian `u32`): green `c0` and blue `40` with red
/// drifting 0..96 (the checks match it as `**c040`), never magenta.
fn rgba(frame: u64) -> u32 {
    let red = (frame % 96) as u32;
    0xff00_0000 | 0x40 << 16 | 0xc0 << 8 | red
}
