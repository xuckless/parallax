//! Native views on Android: the app's `View` or `SurfaceControl` under GPUI's surface, showing through the hole the
//! wgpu renderer cuts (patch 0047). This side decides; Java's `NativeViews` acts (lays a View out, moves a
//! SurfaceControl with GPUI's frame) and its `NativeViewTouchDispatcher` forwards touches. How it fits together:
//! docs/native-view-android.md.

use std::{
    any::Any,
    cell::{Cell, OnceCell, RefCell},
    ffi::c_void,
    ptr::NonNull,
    rc::{Rc, Weak},
};

use gpui::{NativeViewId, Scene, Window};
use jni::{
    JNIEnv,
    objects::{GlobalRef, JObject, JValue},
};

use crate::{
    Error,
    activity_window::ActivityWindow,
    native_view_placement::{self, Placement},
};

/// Whether a touch gesture that starts on a native view, with no GPUI element over it there, also goes to the
/// Android View (offset into its coordinates), so a WebView scrolls and takes taps. GPUI gets the touch either way
/// (`NativeViewInput::Window`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchForwarding {
    Off,
    On,
}

/// One attached view. Kept alive by the `NativeView`'s attachment; dropping it removes the view from the Activity.
struct AttachedView {
    id: NativeViewId,
    /// The view's key in Java's `NativeViews`.
    handle: i32,
    /// The app's `View` or `SurfaceControl`.
    object: GlobalRef,
    /// The Activity's `NativeViews`, which removes the view on drop.
    host: GlobalRef,
    placement: Cell<Placement>,
    touches: Cell<TouchForwarding>,
}

/// Removing the view from the Activity's hierarchy is freeing what `attach` added.
impl Drop for AttachedView {
    fn drop(&mut self) {
        let detached = with_env(|env| {
            env.call_method(
                self.host.as_obj(),
                "detach",
                "(I)V",
                &[JValue::Int(self.handle)],
            )?
            .v()
        });
        if let Err(error) = detached {
            log::error!(
                "parallax: cannot remove native view {}: {error}",
                self.handle
            );
        }
    }
}

/// One Activity's native views. Main thread only.
#[derive(Default)]
pub(crate) struct NativeViews {
    /// Java's `NativeViews`, sent once the Activity's view exists (`nativeViewCreated`).
    host: OnceCell<GlobalRef>,
    views: RefCell<Vec<Weak<AttachedView>>>,
    next_handle: Cell<i32>,
    /// The underlay's colour as Java last set it ([`native_view_placement::background`]).
    background: Cell<i32>,
}

impl NativeViews {
    pub(crate) fn created(&self, host: GlobalRef) {
        if self.host.set(host).is_err() {
            log::error!("parallax: a second NativeViews for one Activity ignored");
        }
    }

    /// `PlatformWindow::attach_native_view`: hands the app's object to Java, hidden until a frame places it.
    ///
    /// # Safety
    /// `view` is a live JNI reference (local or global) to an `android.view.View` without a parent or an
    /// `android.view.SurfaceControl`, valid for this call.
    pub(crate) unsafe fn attach(
        &self,
        id: NativeViewId,
        view: NonNull<c_void>,
    ) -> Option<Box<dyn Any>> {
        let host = self.host.get()?.clone();
        // SAFETY: the caller passes a live JNI reference; `JObject` frees nothing on drop.
        let object = unsafe { JObject::from_raw(view.as_ptr().cast()) };
        let attached = with_env(|env| {
            let object = env.new_global_ref(&object)?;
            let handle = self.next_handle.replace(self.next_handle.get() + 1);
            let args = [JValue::Int(handle), JValue::Object(object.as_obj())];
            let attached = env
                .call_method(host.as_obj(), "attach", "(ILjava/lang/Object;)Z", &args)?
                .z()?;
            Ok(attached.then_some((handle, object)))
        });
        let (handle, object) = match attached {
            Ok(Some(attached)) => attached,
            Ok(None) => return None,
            Err(error) => {
                log::error!("parallax: cannot attach a native view: {error}");
                return None;
            }
        };
        log::info!("parallax: native view {handle} attached");
        let view = Rc::new(AttachedView {
            id,
            handle,
            object,
            host,
            placement: Cell::new(Placement::Hidden),
            touches: Cell::new(TouchForwarding::Off),
        });
        self.views.borrow_mut().push(Rc::downgrade(&view));
        Some(Box::new(view))
    }

    /// Before the renderer draws `scene`: every view the scene moves, resizes, shows or hides goes to Java in one
    /// call, which places it with this frame, with the colour under the views. A frame that changes neither costs
    /// nothing.
    pub(crate) fn place(&self, scene: &Scene) {
        self.views
            .borrow_mut()
            .retain(|view| view.strong_count() > 0);
        let records: Vec<i32> = self
            .views
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .filter_map(|view| {
                let shown_at = scene
                    .native_views
                    .iter()
                    .find(|painted| painted.id == view.id)
                    .map(|painted| painted.bounds);
                let next = Placement::of(shown_at);
                (view.placement.replace(next) != next).then(|| next.record(view.handle))
            })
            .flatten()
            .collect();
        let background = native_view_placement::background(
            scene.native_views.iter().map(|painted| painted.background),
        );
        let recoloured = self.background.replace(background) != background;
        let Some(host) = self
            .host
            .get()
            .filter(|_| recoloured || !records.is_empty())
        else {
            return;
        };
        let placed = with_env(|env| {
            let array = env.new_int_array(records.len() as i32)?;
            env.set_int_array_region(&array, 0, &records)?;
            let args = [JValue::Int(background), JValue::Object(&array)];
            env.call_method(host.as_obj(), "place", "(I[I)V", &args)?
                .v()
        });
        if let Err(error) = placed {
            log::error!("parallax: cannot place native views: {error}");
        }
    }

    /// GPUI's surface was recreated: Java moved the SurfaceControl views to it, hidden, so the next frame places
    /// every view again.
    pub(crate) fn surface_replaced(&self) {
        for view in self.views.borrow().iter().filter_map(Weak::upgrade) {
            view.placement.set(Placement::Hidden);
        }
    }

    /// The handle Java forwards a touch down to: the view GPUI's hit test found (`hit`), if its touches are
    /// forwarded; -1 otherwise.
    pub(crate) fn forwarded(&self, hit: Option<NativeViewId>) -> i32 {
        self.views
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .find(|view| Some(view.id) == hit && view.touches.get() == TouchForwarding::On)
            .map_or(-1, |view| view.handle)
    }

    fn set_touches(&self, env: &mut JNIEnv, object: &JObject, touches: TouchForwarding) -> bool {
        let views = self.views.borrow();
        let view = views.iter().filter_map(Weak::upgrade).find(|view| {
            env.is_same_object(view.object.as_obj(), object)
                .unwrap_or(false)
        });
        view.map(|view| view.touches.set(touches)).is_some()
    }
}

/// Runs `f` with the JNI env of the main thread, where every caller runs. A Java exception becomes the error, cleared.
fn with_env<R>(f: impl FnOnce(&mut JNIEnv) -> Result<R, jni::errors::Error>) -> Result<R, Error> {
    let shell = crate::host::shell()?;
    let mut env = shell.vm.get_env()?;
    let result = env.with_local_frame(8, |env| f(env));
    if result.is_err() && env.exception_check()? {
        env.exception_describe()?;
        env.exception_clear()?;
    }
    Ok(result?)
}

/// The Activity showing `window`, and its native views.
fn activity_window(window: &Window) -> Result<Rc<ActivityWindow>, Error> {
    crate::host::shell()?
        .window_for(window.window_handle())
        .ok_or(Error::NoActivityForWindow)
}

/// Runs `f` with the main thread's JNI env and the Android `Activity` showing `window`: the `Context` an app builds
/// its native views with (a `WebView`, a `SurfaceView`). Call it on the main thread, where GPUI runs.
///
/// # Errors
/// [`Error::NoActivityForWindow`] for a window no Activity shows, [`Error::Jni`] when Java fails.
pub fn with_activity<R>(
    window: &Window,
    f: impl FnOnce(&mut JNIEnv, &JObject) -> R,
) -> Result<R, Error> {
    let activity_window = activity_window(window)?;
    let host = activity_window
        .native_views
        .host
        .get()
        .ok_or(Error::NoActivityForWindow)?;
    let shell = crate::host::shell()?;
    let mut env = shell.vm.get_env()?;
    let context = env
        .call_method(host.as_obj(), "context", "()Landroid/content/Context;", &[])?
        .l()?;
    let result = f(&mut env, &context);
    env.delete_local_ref(context)?;
    Ok(result)
}

/// Sets whether touch gestures that start on `view` (a native view of `window`, as passed to `NativeView::new`) go
/// to the Android View too ([`TouchForwarding`]). A gesture starting under a GPUI element painted over the view
/// stays GPUI's alone.
///
/// # Errors
/// [`Error::NotANativeView`] when `view` is not one of `window`'s native views.
pub fn forward_touches(
    window: &Window,
    view: &JObject,
    touches: TouchForwarding,
) -> Result<(), Error> {
    let activity_window = activity_window(window)?;
    let shell = crate::host::shell()?;
    let mut env = shell.vm.get_env()?;
    if activity_window
        .native_views
        .set_touches(&mut env, view, touches)
    {
        Ok(())
    } else {
        Err(Error::NotANativeView)
    }
}
