//! GPUI's `PlatformWindow` on Android: the app's one window, filling the Activity. State lives in the [`Shell`];
//! this type answers GPUI from it. Stubs are marked "Not yet on Android" and listed in `docs/android.md`.

use std::{rc::Rc, sync::Arc};

use futures::channel::oneshot;
use gpui::{
    Bounds, Capslock, DispatchEventResult, GpuSpecs, Modifiers, Pixels, PlatformAtlas,
    PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow, Point, PromptButton,
    PromptLevel, RequestFrameOptions, Scene, Size, WindowAppearance, WindowBackgroundAppearance,
    WindowBounds, WindowControlArea, WindowInsets, WindowVisibility,
};
use raw_window_handle as rwh;

use crate::{host::Shell, platform::AndroidDisplay};

pub(crate) struct AndroidWindow {
    pub(crate) shell: Rc<Shell>,
    /// The renderer's atlas. GPUI keeps the one it got at open; device recovery refills this same atlas.
    pub(crate) atlas: Arc<dyn PlatformAtlas>,
    pub(crate) input_handler: Option<PlatformInputHandler>,
}

impl rwh::HasWindowHandle for AndroidWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        // Not yet on Android: the native window changes with every surface, so no stable handle is handed out.
        Err(rwh::HandleError::Unavailable)
    }
}

impl rwh::HasDisplayHandle for AndroidWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::android())
    }
}

impl PlatformWindow for AndroidWindow {
    fn bounds(&self) -> Bounds<Pixels> {
        self.shell.metrics.get().bounds()
    }

    fn is_maximized(&self) -> bool {
        true // an Activity's window fills its task
    }

    fn window_bounds(&self) -> WindowBounds {
        WindowBounds::Fullscreen(self.bounds())
    }

    fn content_size(&self) -> Size<Pixels> {
        self.bounds().size
    }

    fn resize(&mut self, _size: Size<Pixels>) {
        // Android sizes the window; apps cannot.
    }

    fn scale_factor(&self) -> f32 {
        self.shell.metrics.get().density
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Light // Not yet on Android: dark mode (Configuration.uiMode).
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(AndroidDisplay::from(self.shell.metrics.get())))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.shell.touches.borrow().last_position
    }

    fn modifiers(&self) -> Modifiers {
        self.shell.keyboard.borrow().modifiers()
    }

    fn capslock(&self) -> Capslock {
        self.shell.keyboard.borrow().capslock()
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.input_handler = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.input_handler.take()
    }

    fn prompt(
        &self,
        _level: PromptLevel,
        _msg: &str,
        _detail: Option<&str>,
        _answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        None // Not yet on Android: native dialogs. `None` makes GPUI draw its own prompt.
    }

    fn activate(&self) {
        // The Activity in front is the active window; nothing to do.
    }

    fn is_active(&self) -> bool {
        self.shell.lifecycle.borrow().is_active()
    }

    fn visibility(&self) -> WindowVisibility {
        self.shell.lifecycle.borrow().visibility()
    }

    fn is_hovered(&self) -> bool {
        false // Not yet on Android: mouse and stylus hover (the ported onGenericMotionEvent, not wired yet)
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        WindowBackgroundAppearance::Opaque
    }

    fn set_title(&mut self, _title: &str) {
        // Android windows have no title bar.
    }

    fn set_background_appearance(&self, _background_appearance: WindowBackgroundAppearance) {
        // Not yet on Android: transparent surfaces.
    }

    fn minimize(&self) {
        // Android has no minimize; apps go to the background through Home.
    }

    fn zoom(&self) {
        // Android has no zoom (maximize); the window already fills its task.
    }

    fn toggle_fullscreen(&self) {
        // Not yet on Android: immersive mode (hiding the system bars).
    }

    fn is_fullscreen(&self) -> bool {
        true // drawn edge to edge, under the system bars
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        self.shell.callbacks.request_frame.set(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.shell.callbacks.input.set(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.shell.callbacks.active_status.set(callback);
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.shell.callbacks.visibility.set(callback);
    }

    fn on_hover_status_change(&self, _callback: Box<dyn FnMut(bool)>) {
        // No hover yet; comes with mouse and stylus hover (onGenericMotionEvent).
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.shell.callbacks.resize.set(callback);
    }

    fn on_moved(&self, _callback: Box<dyn FnMut()>) {
        // An Android window never moves.
    }

    fn on_display_changed(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: moving to another display (DisplayManager).
    }

    fn on_should_close(&self, _callback: Box<dyn FnMut() -> bool>) {
        // Not yet on Android: back and finish() (Full Android, keys and lifecycle).
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
        // Android draws no window controls.
    }

    fn on_close(&self, _callback: Box<dyn FnOnce()>) {
        // Not yet on Android: comes with `quit` (Activity.finish()); onDestroy is Disconnected, the window lives on.
    }

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: dark mode changes (onConfigurationChanged uiMode).
    }

    fn draw(&self, scene: &Scene) {
        if let Some(renderer) = self.shell.renderer.borrow_mut().as_mut() {
            // `false` means no surface or a lost device; the shell handles both on the next surface event or frame.
            renderer.draw(scene);
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false // grayscale text: Android panels have no fixed subpixel order (rotation)
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.shell.renderer.borrow().as_ref()?.gpu_specs()
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {
        // Not yet on Android: text input (InputConnectionAdaptor port).
    }

    fn insets(&self) -> WindowInsets {
        self.shell.metrics.get().insets()
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.shell.callbacks.insets_changed.set(callback);
    }
}
