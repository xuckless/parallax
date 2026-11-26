//! GPUI `Platform` and `PlatformWindow` for Android: GPUI on the main Looper, `gpui_wgpu` on the
//! `ParallaxSurfaceView` surface, Java glue in `java/`. How it fits together: `docs/android.md`.
//!
//! Off Android this crate holds only [`android_main!`], which expands to nothing there.

#[cfg(target_os = "android")]
mod dispatcher;
#[cfg(any(target_os = "android", test))]
mod fonts;
#[cfg(target_os = "android")]
mod host;
#[cfg(any(target_os = "android", test))]
mod keys;
#[cfg(any(target_os = "android", test))]
mod lifecycle;
#[cfg(target_os = "android")]
mod platform;
#[cfg(any(target_os = "android", test))]
mod pointer;
#[cfg(target_os = "android")]
mod renderer;
#[cfg(any(target_os = "android", test))]
mod surface;
#[cfg(target_os = "android")]
mod text_system;
#[cfg(target_os = "android")]
mod window;

#[cfg(target_os = "android")]
pub use host::on_load;
#[doc(hidden)]
#[cfg(target_os = "android")]
pub use jni;

/// Declares the app's entry point on Android. Write it once in the app's `cdylib` crate:
///
/// ```ignore
/// fn launch(cx: &mut parallax::gpui::App) { /* cx.open_window(...) */ }
/// parallax::android_main!(launch);
/// ```
///
/// On Android it defines `JNI_OnLoad`, which `ParallaxActivity`'s `System.loadLibrary` calls; GPUI then runs
/// `launch` on the main thread when the first surface exists. On other targets it expands to nothing.
#[macro_export]
macro_rules! android_main {
    ($launch:expr) => {
        #[cfg(target_os = "android")]
        #[unsafe(no_mangle)]
        /// # Safety
        ///
        /// Only the JVM calls this, with its own valid `JavaVM` pointer, on the thread loading the library.
        pub unsafe extern "system" fn JNI_OnLoad(
            vm: *mut $crate::jni::sys::JavaVM,
            _reserved: *mut ::core::ffi::c_void,
        ) -> $crate::jni::sys::jint {
            // SAFETY: the JVM calls JNI_OnLoad with its own JavaVM pointer, on the thread loading the library.
            unsafe { $crate::on_load(vm, $launch) }
        }
    };
}

#[cfg(any(target_os = "android", test))]
#[cfg_attr(
    not(target_os = "android"),
    allow(dead_code, reason = "host tests build only the pure modules")
)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "parallax-android is not loaded on this thread: start the app through ParallaxActivity and android_main!"
    )]
    NotLoaded,
    #[error("the calling thread has no Android Looper (GPUI must run on the main thread)")]
    NoLooper,
    #[error("{0} failed: {1}")]
    Os(&'static str, #[source] std::io::Error),
    #[cfg(target_os = "android")]
    #[error("JNI: {0}")]
    Jni(#[from] jni::errors::Error),
    #[error("no surface yet: GPUI opens its window once ParallaxSurfaceView has one")]
    NoSurface,
    #[error("Android has one window per app; it is already open")]
    OneWindow,
    #[error("no GPU backend in the renderer table can drive this surface")]
    NoBackend,
    #[error("renderer: {0}")]
    Renderer(String),
    #[error("{0} is not yet on Android")]
    NotYet(&'static str),
    #[error("pointer data packet of {0} bytes is not whole records (36 fields of 8 bytes each)")]
    PacketLength(usize),
    #[error("pointer data packet has an unknown PointerChange {0}")]
    PointerChange(i64),
    #[error("pointer data packet has an unknown PointerDeviceKind {0}")]
    PointerKind(i64),
    #[error("fonts.xml: {0}")]
    FontsXml(#[source] roxmltree::Error),
    #[error("fonts.xml: root element is <{0}>, not <familyset>")]
    FontsXmlRoot(String),
    #[error("fonts.xml: <{element} {attribute}> is missing or invalid: {value:?}")]
    FontsXmlAttribute {
        element: String,
        attribute: &'static str,
        value: Option<String>,
    },
    #[error("no system font: the font list names no font file that is on disk")]
    NoSystemFont,
    /// Kept so 0.1.x stays semver-compatible; removed when the enum turns `#[non_exhaustive]` on the next breaking line.
    #[deprecated(
        since = "0.1.3",
        note = "never constructed: fonts no longer go through add_fonts (patch 0002)"
    )]
    #[error("cannot add the system fonts: {0}")]
    AddFonts(String),
    #[error("no readable font list (tried {0:?})")]
    NoFontList(&'static [&'static str]),
    #[error(
        "key data packet of {0} bytes does not match its header (7 fields of 8 bytes, then the character)"
    )]
    KeyPacketLength(usize),
    #[error("key data packet has an unknown KeyEventType {0}")]
    KeyEventType(i64),
    #[error("key data packet's character is not UTF-8")]
    KeyCharacter,
}

/// The Android platform for GPUI, bound to the Activity that loaded the app. Call it on the main thread, once,
/// after `JNI_OnLoad`. Apps pick platforms through `parallax_platform::current_platform`, which routes Android here.
/// parallax-android starts its own platform on the first surface, with the app's locales from the view; this entry
/// has no view to ask, so its CJK fonts keep fonts.xml order (docs/android-fonts.md).
#[cfg(target_os = "android")]
pub fn current_platform() -> Result<std::rc::Rc<dyn gpui::Platform>, Error> {
    Ok(std::rc::Rc::new(platform::AndroidPlatform::new(
        host::shell()?,
        &[],
    )?))
}
