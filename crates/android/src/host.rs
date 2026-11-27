//! The Rust end of `ParallaxActivity` and `ParallaxSurfaceView`: JNI entry points, the main-thread host and the
//! app's shell, which owns the Activities' lifecycle, the shared GPU context and each Activity's window
//! ([`ActivityWindow`]). How the windows fit together: docs/android-multi-window.md.

use std::{
    cell::{OnceCell, RefCell},
    collections::BTreeMap,
    ffi::c_void,
    rc::Rc,
    sync::Arc,
    time::Instant,
};

use gpui::{
    AnyWindowHandle, App, AppLifecyclePhase, Application, ApplicationHandle, AsyncApp,
    DevicePixels, Edges, Pixels, PlatformAtlas, size,
};
use gpui_wgpu::GpuContext;
use jni::{
    JNIEnv, JavaVM, NativeMethod,
    objects::{JByteBuffer, JClass, JFloatArray, JObject, JString},
    sys::{JNI_ERR, JNI_FALSE, JNI_TRUE, JNI_VERSION_1_6, jboolean, jfloat, jint, jstring},
};

use crate::{
    Error,
    activity_window::{ActivityWindow, Gpui, Slot},
    backend::Device,
    keys,
    lifecycle::{ActivityEvent, ActivityId, Callback, Effect, Lifecycle},
    renderer::{self, NativeWindow},
    surface::SurfaceEvent,
    text_input::EditingState,
    text_system::AndroidTextSystem,
};

/// The Java class whose static `native` methods [`natives`] binds.
const VIEW_CLASS: &str = "io/github/xuckless/parallax/ParallaxSurfaceView";

/// Java's native methods → Rust functions. To add one: declare it `static native` in `ParallaxSurfaceView.java`
/// and add a row here with its JNI signature.
fn natives() -> [NativeMethod; 14] {
    let row = |name: &str, sig: &str, fn_ptr: *mut c_void| NativeMethod {
        name: name.into(),
        sig: sig.into(),
        fn_ptr,
    };
    [
        row(
            "nativeSurfaceCreated",
            "(ILandroid/view/Surface;FIFLjava/lang/String;)V",
            surface_created as *mut c_void,
        ),
        row(
            "nativeSurfaceChanged",
            "(IIIFI)V",
            surface_changed as *mut c_void,
        ),
        row(
            "nativeSurfaceDestroyed",
            "(I)V",
            surface_destroyed as *mut c_void,
        ),
        row("nativeFrame", "(I)V", frame as *mut c_void),
        row(
            "nativeLocalesChanged",
            "(ILjava/lang/String;)V",
            locales_changed as *mut c_void,
        ),
        row(
            "nativeFrameworkCalls",
            "(I)Ljava/lang/String;",
            framework_calls as *mut c_void,
        ),
        row("nativeInsets", "(IFIIIIIIIII)V", insets as *mut c_void),
        row(
            "nativeDispatchPointerDataPacket",
            "(ILjava/nio/ByteBuffer;I)V",
            dispatch_pointer_data_packet as *mut c_void,
        ),
        row(
            "nativeActivityCreated",
            "()I",
            activity_created as *mut c_void,
        ),
        row(
            "nativeActivityEvent",
            "(III)V",
            activity_event as *mut c_void,
        ),
        row(
            "nativeDispatchKeyData",
            "(ILjava/nio/ByteBuffer;ZI)Z",
            dispatch_key_data as *mut c_void,
        ),
        row(
            "nativeUpdateEditingState",
            "(ILjava/lang/String;IIII)V",
            update_editing_state as *mut c_void,
        ),
        row(
            "nativePerformAction",
            "(ILjava/lang/String;)V",
            perform_action as *mut c_void,
        ),
        row(
            "nativeBoundsForRange",
            "(III[F)Z",
            bounds_for_range as *mut c_void,
        ),
    ]
}

/// The app's state on the Android main thread.
///
/// Why a thread-local: Java reaches Rust only through static JNI methods that carry no user pointer, and
/// [`crate::current_platform`] (the platform table's entry) takes no arguments; both must find the one host.
/// It lives on the main thread only: `JNI_OnLoad`, the `SurfaceHolder` callbacks and `Choreographer` all run there.
struct MainThreadHost {
    shell: Rc<Shell>,
    app: RefCell<AppState>,
    /// The app's launch ([`crate::android_main!`]'s first argument): run once, on the first Activity's first surface,
    /// where it starts GPUI and opens that Activity's window.
    launch: fn(&mut App),
    /// How the running app opens each later Activity's window ([`crate::on_activity_window`]); unset, a later
    /// Activity stays black (logged).
    open_activity_window: Slot<dyn FnMut(&mut App)>,
    /// The app's hook for a changed device touch slop ([`crate::android_main!`]'s second argument).
    touch_slop_changed: TouchSlopChanged,
}

/// Called on the main thread, in its own looper turn, when the device's touch slop moves to a new value (logical
/// pixels) in one Activity's view: the facade re-applies the behaviour table's row with it, to the app's tuning and
/// to that Activity's window (`None` before GPUI opens it).
pub type TouchSlopChanged = fn(&mut App, Option<AnyWindowHandle>, Pixels);

enum AppState {
    /// `JNI_OnLoad` ran; GPUI starts on the first Activity's first surface (it needs one to open a window).
    NotStarted,
    /// GPUI is running. The handle keeps it alive for the life of the process and reaches it when the locales
    /// change; `cx` schedules work on GPUI's next looper turn without entering it; `fonts` is `None` when the
    /// system fonts did not load (logged), then there is no order to change.
    Running {
        app: Rc<ApplicationHandle>,
        cx: AsyncApp,
        fonts: Option<Arc<AndroidTextSystem>>,
    },
}

impl MainThreadHost {
    /// Takes Java's density and touch slop (sent with every surface and insets report, [`crate::metrics::Metrics::rescaled`]) and logs
    /// when either moved. A moved logical slop goes to the app's [`TouchSlopChanged`] hook on the next looper turn,
    /// not inside this JNI call, which may be mid-report (the insets are not stored yet) or reached while GPUI runs;
    /// before launch, the launch reads it ([`crate::touch_slop`]). Returns whether the density moved. Each Activity's
    /// view reports its own: `window`'s metrics change.
    fn rescale_and_notify(&self, window: &ActivityWindow, density: f32, touch_slop: jint) -> bool {
        let rescaled = window
            .metrics
            .get()
            .rescaled(density, DevicePixels(touch_slop));
        window.metrics.set(rescaled.metrics);
        if rescaled.density_moved || rescaled.touch_slop_moved.is_some() {
            let logical = rescaled.metrics.touch_slop();
            log::info!(
                "parallax: Activity {}: touch slop {touch_slop} px at density {density}: {logical:?}",
                window.activity.0
            );
        }
        if let (Some(new), AppState::Running { cx, .. }) =
            (rescaled.touch_slop_moved, &*self.app.borrow())
        {
            let (hook, handle) = (self.touch_slop_changed, window.gpui_handle());
            cx.spawn(async move |cx| cx.update(|cx| hook(cx, handle, new)))
                .detach();
        }
        rescaled.density_moved
    }

    /// Asks the app for `window`'s Activity's GPUI window, on its first surface: the first Activity's starts GPUI,
    /// whose `launch` (once per process, as on desktop) opens it; each later one runs the app's
    /// [`crate::on_activity_window`] callback in the running app. [`Shell::open_window`] binds the window either opens.
    fn request_window(&self, window: &ActivityWindow, locales: &[String]) {
        *window.gpui.borrow_mut() = Gpui::Requested;
        // Cloned so the borrow ends before GPUI runs: the app's code may reach `self.app` again.
        let running = match &*self.app.borrow() {
            AppState::NotStarted => None,
            AppState::Running { app, .. } => Some(app.clone()),
        };
        match running {
            Some(app) => {
                let opener = app.update(|cx| self.open_activity_window.with(|open| open(cx)));
                if opener.is_none() {
                    log::warn!(
                        "parallax: the app set no on_activity_window to open a later Activity's window"
                    );
                }
            }
            None => self.start(locales),
        }
        if window.is_requested() {
            log::warn!(
                "parallax: the app opened no window for Activity {}: it stays black",
                window.activity.0
            );
        }
    }

    /// Starts GPUI, whose launch opens the first Activity's window.
    fn start(&self, locales: &[String]) {
        let fonts = crate::text_system::load(&self.shell.vm, locales);
        let platform =
            match crate::platform::AndroidPlatform::new(self.shell.clone(), fonts.clone()) {
                Ok(platform) => platform,
                Err(error) => return log::error!("parallax: cannot start the app: {error}"),
            };
        let platform: Rc<dyn gpui::Platform> = Rc::new(platform);
        let app = Application::with_platform(platform).run_embedded(self.launch);
        *self.app.borrow_mut() = AppState::Running {
            cx: app.to_async(),
            app: Rc::new(app),
            fonts,
        };
        // GPUI launched after onStart (and usually onResume): tell its observers what they missed.
        let phases = self.shell.lifecycle.borrow().launch_phases();
        for &phase in phases {
            self.shell
                .callbacks
                .app_lifecycle
                .with(|callback| callback(phase));
        }
    }

    /// The app's locales now (a configuration change, or a recreated Activity's surface): the per-language font
    /// order follows them. If it moved, GPUI drops the font ids and line layouts it cached with the old order and
    /// redraws. Before launch there is nothing to do: the first surface brings the locales.
    fn follow_locales(&self, locales: &[String]) {
        // Cloned so the borrow ends before GPUI runs: code inside `update` may reach `self.app` again.
        let (app, fonts) = match &*self.app.borrow() {
            AppState::NotStarted | AppState::Running { fonts: None, .. } => return,
            AppState::Running {
                app,
                fonts: Some(fonts),
                ..
            } => (app.clone(), fonts.clone()),
        };
        if fonts.set_locales(locales) {
            app.update(|cx| {
                cx.text_system().invalidate_fonts();
                cx.refresh_windows();
            });
        }
    }
}

/// The app's locales from Java: BCP-47 tags, comma-separated, preference order (`LocaleList.toLanguageTags()`),
/// per-app locales on API 33+ included, which the system property cosmic-text reads lacks. Unreadable: none (logged),
/// so the fonts keep fonts.xml order.
fn locales(env: &mut JNIEnv, tags: &JString) -> Vec<String> {
    match env.get_string(tags) {
        Ok(tags) => String::from(tags)
            .split(',')
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
        Err(error) => {
            log::error!("parallax: unreadable locales, fonts keep fonts.xml order: {error}");
            Vec::new()
        }
    }
}

thread_local! {
    // Clippy flags even the `const` form here (a false positive on this toolchain).
    #[allow(clippy::missing_const_for_thread_local)]
    static HOST: OnceCell<MainThreadHost> = const { OnceCell::new() };
}

/// The shell [`crate::current_platform`] binds to, or [`Error::NotLoaded`] off the main thread or before `on_load`.
pub(crate) fn shell() -> Result<Rc<Shell>, Error> {
    HOST.with(|host| host.get().map(|host| host.shell.clone()))
        .ok_or(Error::NotLoaded)
}

/// Called from the app's `JNI_OnLoad` (generated by [`crate::android_main!`]): logging, natives, host.
///
/// # Safety
/// `vm` must be the `JavaVM` pointer the JVM passed to `JNI_OnLoad`, on the thread that loaded the library.
pub unsafe fn on_load(
    vm: *mut jni::sys::JavaVM,
    launch: fn(&mut App),
    touch_slop_changed: TouchSlopChanged,
) -> jint {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("parallax"),
    );
    // A panic in a JNI callback aborts; without this hook its message goes to stderr, which Android discards.
    std::panic::set_hook(Box::new(|info| log::error!("parallax: panic: {info}")));
    // SAFETY: the caller passes the JVM's own pointer (see # Safety).
    let registered = unsafe { JavaVM::from_raw(vm) }
        .and_then(|vm| {
            vm.get_env()?
                .register_native_methods(VIEW_CLASS, &natives())?;
            Ok(vm)
        })
        .map_err(Error::Jni);
    match registered {
        Ok(vm) => {
            HOST.with(|host| {
                host.get_or_init(|| MainThreadHost {
                    shell: Rc::new(Shell::new(vm)),
                    app: RefCell::new(AppState::NotStarted),
                    launch,
                    open_activity_window: Slot::default(),
                    touch_slop_changed,
                });
            });
            JNI_VERSION_1_6
        }
        Err(error) => {
            log::error!("parallax: cannot load: {error}");
            JNI_ERR
        }
    }
}

/// Sets how the running app opens the GPUI window of each `ParallaxActivity` after the first (split screen, a deep
/// link, a relaunch after the last Activity was destroyed in the same process): `open` runs on the main thread, on
/// that Activity's first surface, and must open one window (`cx.open_window`), as a desktop app opens a window from
/// a reopen callback. Call it in `launch`, which runs once per process; a later call replaces the earlier `open`.
/// The facade's `parallax::android_main!` sets the app's `launch` as the default (docs/android-multi-window.md).
pub fn on_activity_window(open: impl FnMut(&mut App) + 'static) -> Result<(), Error> {
    HOST.with(|host| {
        let host = host.get().ok_or(Error::NotLoaded)?;
        host.open_activity_window.set(Box::new(open));
        Ok(())
    })
}

fn with_host(f: impl FnOnce(&MainThreadHost)) {
    HOST.with(|host| match host.get() {
        Some(host) => f(host),
        None => log::error!("parallax: JNI call before JNI_OnLoad or off the main thread"),
    });
}

/// [`with_host`] for an event from `activity`'s view, which goes to that Activity's window: the one rule for
/// surface, frame, inset, touch, key and text-input events. Dropped once the Activity is destroyed.
fn with_window(activity: jint, f: impl FnOnce(&MainThreadHost, &ActivityWindow)) {
    with_host(|host| match host.shell.window(ActivityId(activity)) {
        Some(window) => f(host, &window),
        None => log::warn!("parallax: event from destroyed Activity {activity} ignored"),
    });
}

/// `tags`: the app's locales ([`locales`]). The first surface starts the platform with them; a later one brings the
/// current ones ([`MainThreadHost::follow_locales`]). An Activity's first surface asks the app for its window
/// ([`MainThreadHost::request_window`]).
extern "system" fn surface_created(
    mut env: JNIEnv,
    _class: JClass,
    activity: jint,
    surface: JObject,
    density: jfloat,
    touch_slop: jint,
    refresh_rate: jfloat,
    tags: JString,
) {
    let locales = locales(&mut env, &tags);
    // SAFETY: `env` and `surface` are this call's live JNI arguments; fromSurface returns a new reference or null.
    let raw = unsafe {
        ndk_sys::ANativeWindow_fromSurface(env.get_raw().cast(), surface.as_raw().cast())
    };
    let Some(native) = NativeWindow::from_acquired(raw) else {
        log::error!("parallax: surfaceCreated without a native window");
        return;
    };
    // A destroyed Activity's window is released on return, unused.
    with_window(activity, |host, window| {
        let size = native.size();
        host.rescale_and_notify(window, density, touch_slop);
        window.set_display(size, refresh_rate);
        window.handle(SurfaceEvent::Created {
            window: native,
            size,
        });
        host.follow_locales(&locales);
        let waiting = matches!(*window.gpui.borrow(), Gpui::Waiting);
        if waiting {
            host.request_window(window, &locales);
        }
    });
}

extern "system" fn surface_changed(
    _env: JNIEnv,
    _class: JClass,
    activity: jint,
    width: jint,
    height: jint,
    density: jfloat,
    touch_slop: jint,
) {
    with_window(activity, |host, window| {
        let refresh_rate = window.metrics.get().refresh_rate;
        let size = size(DevicePixels(width), DevicePixels(height));
        host.rescale_and_notify(window, density, touch_slop);
        window.set_display(size, refresh_rate);
        window.handle(SurfaceEvent::Resized { size });
    });
}

extern "system" fn surface_destroyed(_env: JNIEnv, _class: JClass, activity: jint) {
    with_window(activity, |_, window| {
        window.handle(SurfaceEvent::Destroyed { at: Instant::now() })
    });
}

/// `ParallaxActivity.onCreate`: a new live Activity with its own window, which GPUI opens on its first surface;
/// Java tags its events with the returned id. Without a host there is no id to give: `onCreate` gets an `IllegalStateException`.
extern "system" fn activity_created(mut env: JNIEnv, _class: JClass) -> jint {
    let created = shell().map(|shell| {
        shell.lifecycle(ActivityEvent::Create);
        shell.lifecycle.borrow().newest()
    });
    let why = match created {
        Ok(Some(activity)) => return activity.0,
        Ok(None) => "the lifecycle handed out no id".to_string(),
        Err(error) => error.to_string(),
    };
    log::error!("parallax: cannot create the Activity: {why}");
    if let Err(error) = env.throw_new("java/lang/IllegalStateException", &why) {
        log::error!("parallax: cannot throw to onCreate: {error}");
    }
    0
}

/// `ParallaxActivity`'s lifecycle, focus and memory callbacks: its id, an `EVENT_*` code and its value. The
/// lifecycle itself decides what a callback from a covered or destroyed Activity does.
extern "system" fn activity_event(
    _env: JNIEnv,
    _class: JClass,
    activity: jint,
    code: jint,
    value: jint,
) {
    match Callback::from_java(code, value) {
        Some(callback) => with_host(|host| {
            host.shell
                .lifecycle(ActivityEvent::From(ActivityId(activity), callback))
        }),
        None => log::error!("parallax: unknown Activity event {code} ({value})"),
    }
}

/// The view's `onConfigurationChanged` (the manifest handles `locale|layoutDirection` itself, as Flutter's does):
/// the app's locales, which may not have changed. From any live Activity's view: locales are the app's, not one
/// Activity's, and an unchanged order costs nothing.
extern "system" fn locales_changed(
    mut env: JNIEnv,
    _class: JClass,
    _activity: jint,
    tags: JString,
) {
    let locales = locales(&mut env, &tags);
    with_host(|host| host.follow_locales(&locales));
}

/// Every vsync: GPUI draws if anything changed. Java then asks for GPUI's framework calls ([`framework_calls`]) and
/// runs them after this returns, outside GPUI's draw.
extern "system" fn frame(_env: JNIEnv, _class: JClass, activity: jint) {
    with_window(activity, |_, window| window.frame());
}

/// GPUI's pending calls to Java in Flutter's channel encoding, by channel ([`ActivityWindow::framework_calls`]), or null
/// when there are none. Java asks after each frame and right after `KeyboardManager` handled a key event: a key GPUI
/// edits the field with reaches Java's mirror before the IME's next call (Gboard sends DEL as a key, then may commit
/// text before the next vsync). A pull, not a call into Java: a haptic fires from a GPUI timer, where Rust holds no
/// Java frame to call through.
extern "system" fn framework_calls(env: JNIEnv, _class: JClass, activity: jint) -> jstring {
    let mut calls = None;
    with_window(activity, |_, window| calls = window.framework_calls());
    match calls.map(|calls| env.new_string(calls.to_string())) {
        Some(Ok(calls)) => calls.into_raw(),
        Some(Err(error)) => {
            log::error!("parallax: framework calls dropped: {error}");
            std::ptr::null_mut()
        }
        None => std::ptr::null_mut(),
    }
}

/// `TextInputChannel.updateEditingState`: Java's editing state after an IME edit (UTF-16 offsets, -1 for none).
extern "system" fn update_editing_state(
    mut env: JNIEnv,
    _class: JClass,
    activity: jint,
    text: JString,
    selection_start: jint,
    selection_end: jint,
    composing_start: jint,
    composing_end: jint,
) {
    let text = match env.get_string(&text) {
        Ok(text) => String::from(text),
        Err(error) => {
            log::error!("parallax: unreadable editing state: {error}");
            return;
        }
    };
    let state = EditingState::from_java(
        text,
        (selection_start, selection_end),
        (composing_start, composing_end),
    );
    with_window(activity, |_, window| window.update_editing_state(state));
}

/// `TextInputChannel`'s performAction family: the IME's editor action, by Flutter's name. One rule for every
/// action until GPUI has an action table: it is Enter ([`ActivityWindow::perform_action`]), so the name is not read.
extern "system" fn perform_action(_env: JNIEnv, _class: JClass, activity: jint, _action: JString) {
    with_window(activity, |_, window| window.perform_action());
}

/// `TextInputChannel.boundsForRange`: GPUI's bounds of a UTF-16 range in view pixels, into `ltrb`.
extern "system" fn bounds_for_range(
    env: JNIEnv,
    _class: JClass,
    activity: jint,
    start: jint,
    end: jint,
    ltrb: JFloatArray,
) -> jboolean {
    let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end)) else {
        return JNI_FALSE;
    };
    let mut bounds = None;
    with_window(activity, |_, window| {
        bounds = window.bounds_for_range(start..end)
    });
    let Some(bounds) = bounds else {
        return JNI_FALSE;
    };
    match env.set_float_array_region(&ltrb, 0, &bounds) {
        Ok(()) => JNI_TRUE,
        Err(error) => {
            log::error!("parallax: cannot write the range bounds: {error}");
            JNI_FALSE
        }
    }
}

/// The ported `AndroidTouchProcessor` hands over Flutter's pointer data packet: the first `position` bytes of
/// a direct `ByteBuffer`, from `activity`'s view.
extern "system" fn dispatch_pointer_data_packet(
    env: JNIEnv,
    _class: JClass,
    activity: jint,
    buffer: JByteBuffer,
    position: jint,
) {
    let address = env.get_direct_buffer_address(&buffer);
    let capacity = env.get_direct_buffer_capacity(&buffer);
    let (address, length) = match (address, capacity, usize::try_from(position)) {
        (Ok(address), Ok(capacity), Ok(length)) if length <= capacity => (address, length),
        (address, capacity, _) => {
            log::error!(
                "parallax: unreadable pointer data packet: position {position}, capacity {capacity:?}, \
                 address {:?}",
                address.map(|_| ())
            );
            return;
        }
    };
    // SAFETY: `buffer` is a live direct buffer for this call, `length` is within its capacity, and Java does
    // not touch it until this call returns (AndroidTouchProcessor allocates one per event).
    let packet = unsafe { std::slice::from_raw_parts(address.cast_const(), length) };
    with_window(activity, |_, window| window.pointer(packet));
}

/// The ported `KeyEmbedderResponder` hands over one Flutter key data packet from `activity`'s view: the whole
/// direct `ByteBuffer`, and the Android event's `isCanceled()` and `getMetaState()` (Parallax's addition: key data has
/// no field for them).
/// Returns whether GPUI handled it; Java then redispatches unhandled keys (and a destroyed Activity's) to Android.
extern "system" fn dispatch_key_data(
    env: JNIEnv,
    _class: JClass,
    activity: jint,
    packet: JByteBuffer,
    canceled: jboolean,
    meta_state: jint,
) -> jboolean {
    let extras = keys::KeyEventExtras {
        canceled: canceled != JNI_FALSE,
        meta_state,
    };
    let (address, length) = match (
        env.get_direct_buffer_address(&packet),
        env.get_direct_buffer_capacity(&packet),
    ) {
        (Ok(address), Ok(length)) => (address, length),
        (address, length) => {
            log::error!(
                "parallax: unreadable key data packet: capacity {length:?}, address {:?}",
                address.map(|_| ())
            );
            return JNI_FALSE;
        }
    };
    // SAFETY: `packet` is a live direct buffer for this call, `length` is its capacity, and Java does not touch
    // it until this call returns (KeyData.toBytes allocates one per event).
    let packet = unsafe { std::slice::from_raw_parts(address.cast_const(), length) };
    let mut handled = false;
    with_window(activity, |_, window| handled = window.key(packet, extras));
    if handled { JNI_TRUE } else { JNI_FALSE }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the JNI signature: the Activity, its scale and two edge sets"
)]
extern "system" fn insets(
    _env: JNIEnv,
    _class: JClass,
    activity: jint,
    density: jfloat,
    touch_slop: jint,
    safe_left: jint,
    safe_top: jint,
    safe_right: jint,
    safe_bottom: jint,
    ime_left: jint,
    ime_top: jint,
    ime_right: jint,
    ime_bottom: jint,
) {
    let edges = |left, top, right, bottom| Edges {
        top: DevicePixels(top),
        right: DevicePixels(right),
        bottom: DevicePixels(bottom),
        left: DevicePixels(left),
    };
    with_window(activity, |host, window| {
        // A density change keeps the surface's pixel size, so no surfaceChanged brings it: GPUI's scale changes here.
        let density_changed = host.rescale_and_notify(window, density, touch_slop);
        window.set_insets(
            edges(safe_left, safe_top, safe_right, safe_bottom),
            edges(ime_left, ime_top, ime_right, ime_bottom),
        );
        if density_changed {
            window.resized();
        }
    });
}

/// The platform's callbacks, not a window's: `Platform::on_app_lifecycle` and `on_memory_warning`.
#[derive(Default)]
pub(crate) struct AppCallbacks {
    pub(crate) app_lifecycle: Slot<dyn FnMut(AppLifecyclePhase)>,
    pub(crate) memory_warning: Slot<dyn FnMut()>,
}

/// The app as Android sees it: its Activities and their windows, one each. Main thread only.
pub(crate) struct Shell {
    /// The JVM that loaded the app: the text system calls Java through it to draw emoji.
    pub(crate) vm: JavaVM,
    pub(crate) lifecycle: RefCell<Lifecycle>,
    /// Shared by every window's renderer, as on GPUI's other wgpu platforms.
    gpu: GpuContext,
    /// Picks the renderer's backend order ([`crate::backend`]); `ro.hardware` never changes, so it is read once.
    device: Device,
    /// Each live Activity's window: added on `onCreate`, removed (and its GPUI window closed) on `onDestroy`. The
    /// lifecycle's live Activities and these keys change together, in [`Shell::lifecycle`] only.
    windows: RefCell<BTreeMap<ActivityId, Rc<ActivityWindow>>>,
    pub(crate) callbacks: AppCallbacks,
}

impl Shell {
    fn new(vm: JavaVM) -> Self {
        Self {
            vm,
            lifecycle: RefCell::new(Lifecycle::Loaded),
            gpu: Rc::new(RefCell::new(None)),
            device: renderer::device(),
            windows: RefCell::new(BTreeMap::new()),
            callbacks: AppCallbacks::default(),
        }
    }

    /// Activity `id`'s window; `None` once it is destroyed.
    pub(crate) fn window(&self, id: ActivityId) -> Option<Rc<ActivityWindow>> {
        self.windows.borrow().get(&id).cloned()
    }

    /// The most recently created Activity's window: the one whose metrics stand for the display.
    pub(crate) fn newest_window(&self) -> Option<Rc<ActivityWindow>> {
        self.windows.borrow().values().next_back().cloned()
    }

    /// GPUI's active window: the resumed, focused Activity's, once open.
    pub(crate) fn active_window(&self) -> Option<AnyWindowHandle> {
        let active = self.lifecycle.borrow().active()?;
        self.window(active)?.gpui_handle()
    }

    /// Opens the window the app was asked for: the Activity it is for ([`MainThreadHost::request_window`]), the newest
    /// waiting for one. Android opens no window without an Activity.
    pub(crate) fn open_window(
        &self,
        handle: AnyWindowHandle,
    ) -> Result<(Rc<ActivityWindow>, Arc<dyn PlatformAtlas>), Error> {
        let window = self
            .windows
            .borrow()
            .values()
            .rev()
            .find(|window| window.is_requested())
            .cloned()
            .ok_or(Error::NoActivityForWindow)?;
        let atlas = window.open(handle)?;
        log::info!("parallax: window opened for Activity {}", window.activity.0);
        Ok((window, atlas))
    }

    /// Feeds one Activity callback through the lifecycle and tells GPUI what changed: that Activity's window first,
    /// then the app's observers, in the order `gpui_ios` uses. A new Activity gets a window; a destroyed one's
    /// window closes.
    fn lifecycle(&self, event: ActivityEvent) {
        let before = self.lifecycle.replace(Lifecycle::Loaded);
        let id = match event {
            ActivityEvent::From(id, _) => Some(id),
            ActivityEvent::Create => None,
        };
        let was = id.map(|id| (before.is_active(id), before.visibility(id)));
        let (next, effect) = before.next(event);
        log::info!("parallax: lifecycle {event:?}: {next:?}, {effect:?}");
        let now = id.map(|id| (next.is_active(id), next.visibility(id)));
        let (created, gone) = match (event, id) {
            (ActivityEvent::Create, _) => (next.newest(), None),
            (ActivityEvent::From(..), Some(id)) => (None, (!next.contains(id)).then_some(id)),
            (ActivityEvent::From(..), None) => (None, None),
        };
        let runs_frames = id.is_some_and(|id| next.runs_frames(id));
        *self.lifecycle.borrow_mut() = next;
        if let Some(created) = created {
            let window = ActivityWindow::new(created, self.gpu.clone(), self.device);
            self.windows.borrow_mut().insert(created, Rc::new(window));
        }
        let window = id.and_then(|id| self.window(id));
        if let (Some(window), Some((was_active, was_visible)), Some((active, visibility))) =
            (&window, was, now)
        {
            if !runs_frames {
                // Paused: nobody pulls them now, and a haptic must not play late, on resume.
                window.haptics.borrow_mut().clear();
            }
            if was_active != active {
                window
                    .callbacks
                    .active_status
                    .with(|callback| callback(active));
            }
            if was_visible != visibility {
                window
                    .callbacks
                    .visibility
                    .with(|callback| callback(visibility));
            }
        }
        if let Some(gone) = gone {
            let closed = self.windows.borrow_mut().remove(&gone);
            if let Some(closed) = closed {
                log::info!("parallax: Activity {} destroyed: its window closes", gone.0);
                closed.close();
            }
        }
        match effect {
            Effect::Phases(phases) => {
                for &phase in phases {
                    self.callbacks
                        .app_lifecycle
                        .with(|callback| callback(phase));
                }
            }
            Effect::MemoryWarning => {
                self.callbacks.memory_warning.with(|callback| callback());
            }
            Effect::Nothing => {}
            Effect::Reject { why } => log::warn!("parallax: lifecycle event ignored: {why}"),
        }
    }
}
