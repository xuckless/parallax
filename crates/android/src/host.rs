//! The Rust end of `ParallaxSurfaceView`: JNI entry points, the main-thread host and the shell that owns the
//! surface state machine, the renderer and GPUI's window callbacks.

use std::{
    cell::{Cell, OnceCell, RefCell},
    ffi::c_void,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    App, AppLifecyclePhase, Application, ApplicationHandle, Bounds, DevicePixels,
    DispatchEventResult, Edges, Pixels, PlatformAtlas, PlatformInput, PlatformInputHandler,
    RequestFrameOptions, Size, WindowInsets, WindowVisibility, point, px, size,
};
use gpui_wgpu::{GpuContext, WgpuRenderer};
use jni::{
    JNIEnv, JavaVM, NativeMethod,
    objects::{JByteBuffer, JClass, JFloatArray, JObject, JString},
    sys::{JNI_ERR, JNI_FALSE, JNI_TRUE, JNI_VERSION_1_6, jboolean, jfloat, jint, jstring},
};

use crate::{
    Error, keys,
    lifecycle::{ActivityEvent, ActivityId, Callback, Effect, Lifecycle},
    pointer,
    renderer::{self, NativeWindow},
    surface::{Action, Surface, SurfaceEvent},
    text_input::{self, Edit, EditingState},
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
            "(ILandroid/view/Surface;FFLjava/lang/String;)V",
            surface_created as *mut c_void,
        ),
        row(
            "nativeSurfaceChanged",
            "(IIIF)V",
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
            "nativeTextInputCalls",
            "(I)Ljava/lang/String;",
            text_input_calls as *mut c_void,
        ),
        row("nativeInsets", "(IIIIIIIII)V", insets as *mut c_void),
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
            "(ILjava/nio/ByteBuffer;Z)Z",
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
}

enum AppState {
    /// `JNI_OnLoad` ran; the app launches on the first surface (GPUI needs one to open its window).
    Loaded { launch: fn(&mut App) },
    /// GPUI is running. The handle keeps it alive for the life of the process and reaches it when the locales
    /// change; `fonts` is `None` when the system fonts did not load (logged), then there is no order to change.
    Running {
        app: Rc<ApplicationHandle>,
        fonts: Option<Arc<AndroidTextSystem>>,
    },
}

impl MainThreadHost {
    /// The app's locales now (a configuration change, or a recreated Activity's surface): the per-language font
    /// order follows them. If it moved, GPUI drops the font ids and line layouts it cached with the old order and
    /// redraws. Before launch there is nothing to do: the first surface brings the locales.
    fn follow_locales(&self, locales: &[String]) {
        // Cloned so the borrow ends before GPUI runs: code inside `update` may reach `self.app` again.
        let (app, fonts) = match &*self.app.borrow() {
            AppState::Loaded { .. } | AppState::Running { fonts: None, .. } => return,
            AppState::Running {
                app,
                fonts: Some(fonts),
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
pub unsafe fn on_load(vm: *mut jni::sys::JavaVM, launch: fn(&mut App)) -> jint {
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
                    app: RefCell::new(AppState::Loaded { launch }),
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

fn with_host(f: impl FnOnce(&MainThreadHost)) {
    HOST.with(|host| match host.get() {
        Some(host) => f(host),
        None => log::error!("parallax: JNI call before JNI_OnLoad or off the main thread"),
    });
}

/// [`with_host`] for an event from `activity`'s view: dropped unless that Activity owns the window
/// ([`Lifecycle::accepts`], the one rule for surface, frame, inset, touch and key events).
fn with_owner(activity: jint, f: impl FnOnce(&MainThreadHost)) {
    with_host(|host| {
        if host.shell.lifecycle.borrow().accepts(ActivityId(activity)) {
            f(host)
        } else {
            log::warn!(
                "parallax: event from Activity {activity}, which does not own the window, ignored"
            );
        }
    });
}

/// `tags`: the app's locales ([`locales`]). The first surface starts the platform with them; a later one (a recreated
/// Activity) brings the current ones ([`MainThreadHost::follow_locales`]).
extern "system" fn surface_created(
    mut env: JNIEnv,
    _class: JClass,
    activity: jint,
    surface: JObject,
    density: jfloat,
    refresh_rate: jfloat,
    tags: JString,
) {
    let locales = locales(&mut env, &tags);
    // SAFETY: `env` and `surface` are this call's live JNI arguments; fromSurface returns a new reference or null.
    let raw = unsafe {
        ndk_sys::ANativeWindow_fromSurface(env.get_raw().cast(), surface.as_raw().cast())
    };
    let Some(window) = NativeWindow::from_acquired(raw) else {
        log::error!("parallax: surfaceCreated without a native window");
        return;
    };
    // A stale view's window is released on return, unused.
    with_owner(activity, |host| {
        host.shell.set_display(window.size(), density, refresh_rate);
        host.shell.handle(SurfaceEvent::Created {
            window,
            activity: ActivityId(activity),
        });
        // Read first, so the borrow ends before `follow_locales` runs GPUI.
        let loaded = match &*host.app.borrow() {
            AppState::Loaded { launch } => Some(*launch),
            AppState::Running { .. } => None,
        };
        let Some(launch) = loaded else {
            return host.follow_locales(&locales);
        };
        let fonts = crate::text_system::load(&host.shell.vm, &locales);
        match crate::platform::AndroidPlatform::new(host.shell.clone(), fonts.clone()) {
            Ok(platform) => {
                let platform: Rc<dyn gpui::Platform> = Rc::new(platform);
                let app = Application::with_platform(platform).run_embedded(launch);
                *host.app.borrow_mut() = AppState::Running {
                    app: Rc::new(app),
                    fonts,
                };
                // GPUI launched after onStart (and usually onResume): tell its observers what they missed.
                let phases = host.shell.lifecycle.borrow().launch_phases();
                for &phase in phases {
                    host.shell
                        .callbacks
                        .app_lifecycle
                        .with(|callback| callback(phase));
                }
            }
            Err(error) => log::error!("parallax: cannot start the app: {error}"),
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
) {
    with_owner(activity, |host| {
        let refresh_rate = host.shell.metrics.get().refresh_rate;
        host.shell.set_display(
            size(DevicePixels(width), DevicePixels(height)),
            density,
            refresh_rate,
        );
        host.shell.handle(SurfaceEvent::Resized);
    });
}

/// Not [`with_owner`]: a view whose Activity no longer owns the window may still hold the current surface, and
/// the surface state machine releases it only for that view.
extern "system" fn surface_destroyed(_env: JNIEnv, _class: JClass, activity: jint) {
    with_host(|host| {
        host.shell.handle(SurfaceEvent::Destroyed {
            activity: ActivityId(activity),
            at: Instant::now(),
        })
    });
}

/// `ParallaxActivity.onCreate`: a new live Activity, which owns the window once it starts; Java tags its events
/// with the returned id. Without a host there is no id to give: `onCreate` gets an `IllegalStateException`.
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

/// Every vsync: GPUI draws if anything changed. Java then asks for the text-input calls ([`text_input_calls`]) and
/// runs them after this returns, outside GPUI's draw.
extern "system" fn frame(_env: JNIEnv, _class: JClass, activity: jint) {
    with_owner(activity, |host| host.shell.frame());
}

/// GPUI's pending text-input calls for `TextInputChannel.handleFrameworkCalls` (Flutter's JSON), or null. Java asks
/// after each frame and right after `KeyboardManager` handled a key event: a key GPUI edits the field with reaches
/// Java's mirror before the IME's next call (Gboard sends DEL as a key, then may commit text before the next vsync).
extern "system" fn text_input_calls(env: JNIEnv, _class: JClass, activity: jint) -> jstring {
    let mut calls = None;
    with_owner(activity, |host| calls = host.shell.text_input_calls());
    match calls.map(|calls| env.new_string(calls)) {
        Some(Ok(calls)) => calls.into_raw(),
        Some(Err(error)) => {
            log::error!("parallax: text input calls dropped: {error}");
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
    with_owner(activity, |host| host.shell.update_editing_state(state));
}

/// `TextInputChannel`'s performAction family: the IME's editor action, by Flutter's name. One rule for every
/// action until GPUI has an action table: it is Enter ([`Shell::perform_action`]), so the name is not read.
extern "system" fn perform_action(_env: JNIEnv, _class: JClass, activity: jint, _action: JString) {
    with_owner(activity, |host| host.shell.perform_action());
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
    with_owner(activity, |host| {
        bounds = host.shell.bounds_for_range(start..end)
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
    with_owner(activity, |host| host.shell.pointer(packet));
}

/// The ported `KeyEmbedderResponder` hands over one Flutter key data packet from `activity`'s view: the whole
/// direct `ByteBuffer`, and the Android event's `isCanceled()` (Parallax's addition: key data has no field for it).
/// Returns whether GPUI handled it; Java then redispatches unhandled keys (and a non-owner's) to Android.
extern "system" fn dispatch_key_data(
    env: JNIEnv,
    _class: JClass,
    activity: jint,
    packet: JByteBuffer,
    canceled: jboolean,
) -> jboolean {
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
    with_owner(activity, |host| {
        handled = host.shell.key(packet, canceled != JNI_FALSE)
    });
    if handled { JNI_TRUE } else { JNI_FALSE }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the JNI signature: the Activity and two edge sets"
)]
extern "system" fn insets(
    _env: JNIEnv,
    _class: JClass,
    activity: jint,
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
    with_owner(activity, |host| {
        host.shell.set_insets(
            edges(safe_left, safe_top, safe_right, safe_bottom),
            edges(ime_left, ime_top, ime_right, ime_bottom),
        )
    });
}

/// What Java last reported about the display and system bars, in physical pixels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Metrics {
    pub(crate) size: Size<DevicePixels>,
    pub(crate) density: f32,
    pub(crate) refresh_rate: f32,
    safe_area: Edges<DevicePixels>,
    ime: Edges<DevicePixels>,
}

impl Metrics {
    /// Before the first surface: nothing drawn yet, so any density works; 1.0 keeps conversions finite.
    const BEFORE_FIRST_SURFACE: Metrics = Metrics {
        size: Size {
            width: DevicePixels(0),
            height: DevicePixels(0),
        },
        density: 1.0,
        refresh_rate: 0.0,
        safe_area: Edges {
            top: DevicePixels(0),
            right: DevicePixels(0),
            bottom: DevicePixels(0),
            left: DevicePixels(0),
        },
        ime: Edges {
            top: DevicePixels(0),
            right: DevicePixels(0),
            bottom: DevicePixels(0),
            left: DevicePixels(0),
        },
    };

    fn logical(&self, edges: Edges<DevicePixels>) -> Edges<Pixels> {
        edges.map(|edge| px(edge.0 as f32 / self.density))
    }

    /// The window (and display) bounds in logical pixels.
    pub(crate) fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(0.), px(0.)),
            size(
                px(self.size.width.0 as f32 / self.density),
                px(self.size.height.0 as f32 / self.density),
            ),
        )
    }

    pub(crate) fn refresh_interval(&self) -> Option<Duration> {
        (self.refresh_rate > 0.0).then(|| Duration::from_secs_f32(1.0 / self.refresh_rate))
    }

    pub(crate) fn insets(&self) -> WindowInsets {
        WindowInsets {
            safe_area: self.logical(self.safe_area),
            ime: self.logical(self.ime),
            // Not yet on Android: rounded display corners (`RoundedCorner`, API 31).
            corner_radii: Default::default(),
        }
    }
}

/// A GPUI callback the window registered, taken out while it runs so it can re-enter the shell.
pub(crate) struct Slot<F: ?Sized>(RefCell<Option<Box<F>>>);

impl<F: ?Sized> Slot<F> {
    pub(crate) fn set(&self, callback: Box<F>) {
        *self.0.borrow_mut() = Some(callback);
    }

    /// Runs `call` on the callback; `None` when GPUI has not registered one.
    fn with<R>(&self, call: impl FnOnce(&mut F) -> R) -> Option<R> {
        let mut callback = self.0.borrow_mut().take()?;
        let result = call(&mut callback);
        self.0.borrow_mut().get_or_insert(callback);
        Some(result)
    }
}

impl<F: ?Sized> Default for Slot<F> {
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}

#[derive(Default)]
pub(crate) struct Callbacks {
    pub(crate) request_frame: Slot<dyn FnMut(RequestFrameOptions)>,
    /// Pointers arrive here as touch, mouse and scroll input ([`Shell::pointer`]), keys as key, modifier and back events
    /// ([`Shell::key`]).
    pub(crate) input: Slot<dyn FnMut(PlatformInput) -> DispatchEventResult>,
    pub(crate) resize: Slot<dyn FnMut(Size<Pixels>, f32)>,
    pub(crate) insets_changed: Slot<dyn FnMut(WindowInsets)>,
    pub(crate) active_status: Slot<dyn FnMut(bool)>,
    pub(crate) visibility: Slot<dyn FnMut(WindowVisibility)>,
    /// The platform's, not the window's: `Platform::on_app_lifecycle` and `on_memory_warning`.
    pub(crate) app_lifecycle: Slot<dyn FnMut(AppLifecyclePhase)>,
    pub(crate) memory_warning: Slot<dyn FnMut()>,
}

/// The app's one window as Android sees it. Main thread only.
pub(crate) struct Shell {
    /// The JVM that loaded the app: the text system calls Java through it to draw emoji.
    pub(crate) vm: JavaVM,
    surface: RefCell<Surface<NativeWindow>>,
    pub(crate) lifecycle: RefCell<Lifecycle>,
    pub(crate) metrics: Cell<Metrics>,
    gpu: GpuContext,
    /// `None` until GPUI opens the window; then the renderer for the rest of the process.
    pub(crate) renderer: RefCell<Option<WgpuRenderer>>,
    pub(crate) callbacks: Callbacks,
    pub(crate) pointers: RefCell<pointer::Pointers>,
    pub(crate) keyboard: RefCell<keys::Keyboard>,
    /// The focused field's handler: GPUI installs it every frame ([`crate::window`]), the IME queries it.
    pub(crate) input_handler: RefCell<Option<PlatformInputHandler>>,
    pub(crate) text_input: RefCell<text_input::Session>,
}

impl Shell {
    fn new(vm: JavaVM) -> Self {
        Self {
            vm,
            surface: RefCell::new(Surface::None),
            lifecycle: RefCell::new(Lifecycle::Loaded),
            metrics: Cell::new(Metrics::BEFORE_FIRST_SURFACE),
            gpu: Rc::new(RefCell::new(None)),
            renderer: RefCell::new(None),
            callbacks: Callbacks::default(),
            pointers: RefCell::new(pointer::Pointers::default()),
            keyboard: RefCell::new(keys::Keyboard::default()),
            input_handler: RefCell::new(None),
            text_input: RefCell::new(text_input::Session::default()),
        }
    }

    fn set_display(&self, size: Size<DevicePixels>, density: f32, refresh_rate: f32) {
        self.metrics.set(Metrics {
            size,
            density,
            refresh_rate,
            ..self.metrics.get()
        });
    }

    fn set_insets(&self, safe_area: Edges<DevicePixels>, ime: Edges<DevicePixels>) {
        let metrics = Metrics {
            safe_area,
            ime,
            ..self.metrics.get()
        };
        self.metrics.set(metrics);
        log::info!("parallax: insets in pixels: safe area {safe_area:?}, ime {ime:?}");
        self.callbacks
            .insets_changed
            .with(|callback| callback(metrics.insets()));
    }

    /// The current native window: ready, or waiting for the renderer to be rebuilt on it.
    fn current_window(&self) -> Option<NativeWindow> {
        match &*self.surface.borrow() {
            Surface::Ready { window, .. } | Surface::Recovering { window, .. } => {
                Some(window.clone())
            }
            Surface::None | Surface::Lost { .. } => None,
        }
    }

    /// Opens the app's one window: creates the renderer on the ready surface and returns its atlas.
    pub(crate) fn open_window(&self) -> Result<Arc<dyn PlatformAtlas>, Error> {
        if self.renderer.borrow().is_some() {
            return Err(Error::OneWindow);
        }
        let window = self.current_window().ok_or(Error::NoSurface)?;
        let renderer = renderer::create(&self.gpu, &window, self.metrics.get().size)?;
        let atlas: Arc<dyn PlatformAtlas> = renderer.sprite_atlas().clone();
        *self.renderer.borrow_mut() = Some(renderer);
        Ok(atlas)
    }

    /// Feeds one event through the surface state machine and carries out its action on the renderer.
    fn handle(&self, event: SurfaceEvent<NativeWindow>) {
        let state = self.surface.replace(Surface::None);
        let (next, action) = state.next(event);
        // Not every vsync: Draw and Wait are the quiet steady states.
        if !matches!(action, Action::Draw | Action::Wait) {
            log::info!("parallax: surface {next:?}, {action:?}");
        }
        *self.surface.borrow_mut() = next;
        let size = self.metrics.get().size;
        match action {
            Action::Attach { previous } => {
                if let (Some(renderer), Some(window)) =
                    (self.renderer.borrow_mut().as_mut(), self.current_window())
                {
                    let instance = self
                        .gpu
                        .borrow()
                        .as_ref()
                        .map(|context| context.instance.clone());
                    let replaced = instance.ok_or(Error::NoBackend).and_then(|instance| {
                        renderer
                            .replace_surface(&window, renderer::surface_config(size), &instance)
                            .map_err(|error| Error::Renderer(format!("{error:#}")))
                    });
                    if let Err(error) = replaced {
                        log::error!("parallax: cannot attach the new surface: {error}");
                    }
                }
                // Released only now: the renderer has dropped its surface on the previous window.
                drop(previous);
                self.resized();
            }
            Action::Resize => {
                if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
                    renderer.update_drawable_size(size);
                }
                self.resized();
            }
            Action::Detach { window } => {
                if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
                    renderer.unconfigure_surface();
                }
                // Android requires the window released before surfaceDestroyed returns.
                drop(window);
            }
            Action::RecoverDevice => {
                let recovered = match (self.renderer.borrow_mut().as_mut(), self.current_window()) {
                    (Some(renderer), Some(window)) => {
                        renderer::recover(&self.gpu, renderer, &window, size)
                    }
                    (None, _) | (_, None) => Err(Error::NoSurface),
                };
                match recovered {
                    Ok(()) => self.handle(SurfaceEvent::Recovered),
                    Err(error) => {
                        // The state machine waits longer before the next try.
                        log::error!("parallax: GPU device recovery failed: {error}");
                        self.handle(SurfaceEvent::RecoveryFailed { at: Instant::now() });
                    }
                }
            }
            Action::Draw => {
                self.callbacks
                    .request_frame
                    .with(|callback| callback(RequestFrameOptions::default()));
            }
            Action::Wait => {}
            Action::Reject { why } => log::warn!("parallax: surface event ignored: {why}"),
        }
    }

    /// Feeds one Activity callback through the lifecycle and tells GPUI what changed: the window first, then the
    /// app's observers, in the order `gpui_ios` uses.
    fn lifecycle(&self, event: ActivityEvent) {
        let before = self.lifecycle.replace(Lifecycle::Loaded);
        let (was_active, was_visible, was_owner) =
            (before.is_active(), before.visibility(), before.owner());
        let (next, effect) = before.next(event);
        log::info!("parallax: lifecycle {event:?}: {next:?}, {effect:?}");
        let (active, visibility, owner) = (next.is_active(), next.visibility(), next.owner());
        *self.lifecycle.borrow_mut() = next;
        if owner.is_some() && owner != was_owner {
            self.text_input.borrow_mut().owner_changed();
        }
        if was_active != active {
            self.callbacks
                .active_status
                .with(|callback| callback(active));
        }
        if was_visible != visibility {
            self.callbacks
                .visibility
                .with(|callback| callback(visibility));
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

    /// Tells GPUI the window size or scale may have changed, then redraws the new surface.
    fn resized(&self) {
        let metrics = self.metrics.get();
        self.callbacks
            .resize
            .with(|callback| callback(metrics.bounds().size, metrics.density));
        self.redraw();
    }

    /// A full frame: after a new surface, size or device the old content is gone.
    fn redraw(&self) {
        self.callbacks.request_frame.with(|callback| {
            callback(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            })
        });
    }

    /// One pointer data packet → GPUI touch, mouse and scroll events, dispatched in packet order. Upstream turns a
    /// tap into a click; a mouse clicks itself. The keyboard is read before GPUI runs (it reads modifiers too).
    fn pointer(&self, packet: &[u8]) {
        let modifiers = self.keyboard.borrow().modifiers();
        let events = match pointer::decode(packet) {
            Ok(data) => {
                self.pointers
                    .borrow_mut()
                    .events(&data, self.metrics.get().density, modifiers)
            }
            Err(error) => {
                log::error!("parallax: pointer packet dropped: {error}");
                return;
            }
        };
        for event in events {
            log::debug!("parallax: {event:?}");
            self.callbacks.input.with(|callback| {
                callback(event);
            });
        }
    }

    /// One key data packet → GPUI; returns whether GPUI handled it. The keyboard is not borrowed while GPUI runs,
    /// so the window can read modifiers during dispatch.
    fn key(&self, packet: &[u8], canceled: bool) -> bool {
        let data = match keys::decode(packet) {
            Ok(data) => data,
            Err(error) => {
                log::error!("parallax: key dropped: {error}");
                return false;
            }
        };
        // GPUI hit-tests back gestures; Android's back key has none, so it starts at the left edge, mid-height,
        // where an edge swipe would (iOS's back gesture comes from the leading edge).
        let back_at = point(px(0.), self.metrics.get().bounds().size.height / 2.);
        let step = self.keyboard.borrow_mut().input(&data, canceled, back_at);
        log::debug!("parallax: {data:?} → {step:?}");
        match step {
            keys::Step::Answer { input, handled } => {
                if let Some(input) = input {
                    self.callbacks.input.with(|callback| callback(input));
                }
                handled
            }
            keys::Step::Ask(input) => {
                // No window yet: nothing handled it, so Android gets the key.
                let result = self
                    .callbacks
                    .input
                    .with(|callback| callback(input))
                    .unwrap_or(DispatchEventResult {
                        propagate: true,
                        default_prevented: false,
                    });
                self.keyboard.borrow_mut().reply(result)
            }
        }
    }

    /// Runs `call` on the focused field's handler, taken out so GPUI can re-enter the window; `None` without one.
    /// Re-entrancy: if GPUI installs a new handler during `call` (`set_input_handler`), the newer one is kept and
    /// this one dropped.
    fn with_input_handler<R>(
        &self,
        call: impl FnOnce(&mut PlatformInputHandler) -> R,
    ) -> Option<R> {
        let mut handler = self.input_handler.borrow_mut().take()?;
        let result = call(&mut handler);
        self.input_handler.borrow_mut().get_or_insert(handler);
        Some(result)
    }

    /// The pending text-input calls for Java, as Flutter's JSON; empties them. Never logs the text (password
    /// fields included): the calls carry it.
    fn text_input_calls(&self) -> Option<String> {
        let pending = self.text_input.borrow_mut().take()?;
        let state = if pending.sends_state() {
            self.with_input_handler(|handler| {
                let state = read_state(handler)?;
                match pending.commit(&state) {
                    Some(edit) => {
                        log::debug!("parallax: composition committed for a new client");
                        apply(handler, edit);
                        read_state(handler)
                    }
                    None => Some(state),
                }
            })
            .flatten()
        } else {
            None
        };
        if pending.sets_client() && state.is_none() {
            // Once per focus: each focus sets a new client.
            log::error!(
                "parallax: the focused field's text is unreadable (no text_length_utf16 or text_for_range): \
                 the IME sees it empty"
            );
        }
        // The field's text bounds, for autofill's rect: the caret when the field is empty.
        let reads_bounds = self.text_input.borrow().reads_bounds(&pending);
        let bounds = if reads_bounds {
            self.with_input_handler(|handler| {
                let length = match &state {
                    Some(state) => state.text.encode_utf16().count(),
                    None => handler.text_length_utf16().unwrap_or(0),
                };
                handler.bounds_for_range(0..length)
            })
            .flatten()
        } else {
            None
        };
        let described = state.as_ref().map_or("none".to_string(), describe);
        let calls = self.text_input.borrow_mut().send(pending, state, bounds)?;
        log::debug!("parallax: text input {pending:?}, state: {described}");
        Some(calls.to_string())
    }

    /// Java's editing state after an IME edit → GPUI, synchronously: merged onto GPUI's state when GPUI changed
    /// since Java's mirror was last current ([`text_input::Session::java_edit`]).
    fn update_editing_state(&self, java: EditingState) {
        self.with_input_handler(|handler| {
            let Some(gpui) = read_state(handler) else {
                log::debug!("parallax: IME edit on an unreadable field dropped");
                return;
            };
            let Some(target) = self.text_input.borrow_mut().java_edit(&java, &gpui) else {
                log::debug!("parallax: IME edit for a client GPUI is replacing dropped");
                return;
            };
            let edits = text_input::edits(&gpui, &target);
            log::debug!(
                "parallax: IME {} over {} → {} calls",
                describe(&java),
                describe(&gpui),
                edits.len()
            );
            for edit in edits {
                apply(handler, edit);
            }
            // The app may refuse or change the edit (a filter, a length limit): the session decides whether Java
            // must take GPUI's state.
            let after = read_state(handler).unwrap_or(target);
            self.text_input.borrow_mut().java_edited(java, &after);
        });
    }

    /// The IME's editor action: GPUI gets Enter, as a hardware key (see [`keys::Keyboard::press`]).
    fn perform_action(&self) {
        let inputs = self.keyboard.borrow().press("enter");
        for input in inputs {
            self.callbacks.input.with(|callback| callback(input));
        }
    }

    /// GPUI's bounds of a UTF-16 range as `[left, top, right, bottom]` in view pixels.
    fn bounds_for_range(&self, range: std::ops::Range<usize>) -> Option<[f32; 4]> {
        let bounds = self
            .with_input_handler(|handler| handler.bounds_for_range(range))
            .flatten()?;
        let density = self.metrics.get().density;
        Some([
            f32::from(bounds.left()) * density,
            f32::from(bounds.top()) * density,
            f32::from(bounds.right()) * density,
            f32::from(bounds.bottom()) * density,
        ])
    }

    /// Every vsync (Choreographer): GPUI draws if anything changed, or device recovery moves on.
    fn frame(&self) {
        let device_lost = self
            .renderer
            .borrow()
            .as_ref()
            .is_some_and(WgpuRenderer::device_lost);
        self.handle(SurfaceEvent::Frame {
            at: Instant::now(),
            device_lost,
        });
    }
}

/// One [`Edit`] as GPUI's `InputHandler` call.
fn apply(handler: &mut PlatformInputHandler, edit: Edit) {
    match edit {
        Edit::Replace { range, text } => handler.replace_text_in_range(Some(range), &text),
        Edit::Mark {
            range,
            text,
            selected,
        } => handler.replace_and_mark_text_in_range(Some(range), &text, selected),
        Edit::Unmark => handler.unmark_text(),
        Edit::Select(range) => handler.set_selected_text_range(range),
    }
}

/// The focused field's whole editing state, in UTF-16, clamped to its text ([`EditingState::clamped`]). The
/// editable range is not honoured yet: Java mirrors the whole document. A handler without `text_length_utf16`
/// (GPUI's default is `None`) is asked for everything; the range it adjusts to must start at 0.
fn read_state(handler: &mut PlatformInputHandler) -> Option<EditingState> {
    let mut adjusted = None;
    let text = match handler.text_length_utf16() {
        Some(length) => handler.text_for_range(0..length, &mut adjusted),
        None => handler.text_for_range(0..usize::MAX, &mut adjusted),
    }
    .filter(|_| adjusted.as_ref().is_none_or(|range| range.start == 0))?;
    let selection = handler
        .selected_text_range(false)
        .map(|selection| match selection.reversed {
            true => (selection.range.end, selection.range.start),
            false => (selection.range.start, selection.range.end),
        });
    let composing = handler.marked_text_range();
    Some(EditingState::clamped(text, selection, composing))
}

/// An editing state for the log: its length and ranges, never its text (it may be a password).
fn describe(state: &EditingState) -> String {
    format!(
        "{} UTF-16 units, selection {:?}, composing {:?}",
        state.text.encode_utf16().count(),
        state.selection,
        state.composing
    )
}
