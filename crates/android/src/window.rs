//! GPUI's `PlatformWindow` on Android: one Activity's window, filling it. State lives in its [`ActivityWindow`] (and
//! the Activity's stage in the app's [`Shell`]); this type answers GPUI from them. Stubs are marked "Not yet on Android" and listed in `docs/android.md`.

use std::{rc::Rc, sync::Arc};

use futures::channel::oneshot;
use gpui::{
    Bounds, Capslock, DispatchEventResult, GpuSpecs, Haptic, Modifiers, Pixels, PlatformAtlas,
    PlatformDisplay, PlatformInput, PlatformInputHandler, PlatformWindow, Point, PromptButton,
    PromptLevel, RequestFrameOptions, Scene, Size, TextInputConfiguration, TextInputStateChange,
    WindowAppearance, WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowInsets,
    WindowVisibility,
};
use raw_window_handle as rwh;

use crate::{
    activity_window::{ActivityWindow, FirstFrame},
    gpui_window::GpuiEffect,
    host::Shell,
    platform::AndroidDisplay,
};

pub(crate) struct AndroidWindow {
    pub(crate) shell: Rc<Shell>,
    pub(crate) window: Rc<ActivityWindow>,
    /// The renderer's atlas. GPUI keeps the one it got at open; device recovery refills this same atlas.
    pub(crate) atlas: Arc<dyn PlatformAtlas>,
}

/// GPUI drops a window it closed, and dropping a platform window closes the native one, as upstream's macOS and iOS
/// windows do: here the Activity's window, so the Activity finishes rather than stay on screen empty
/// ([`ActivityWindow::closed_by_app`]). This is the one `Drop` in Parallax that does more than free a resource
/// (AGENTS.md, "No hidden behaviour"): it keeps upstream's contract that closing a window closes it on screen, so an
/// app's `window.remove_window()` works here as on desktop. It finishes the Activity at once, even when it is paused
/// or stopped (docs/android-multi-window.md#closing-a-window).
impl Drop for AndroidWindow {
    fn drop(&mut self) {
        match self.window.closed_by_app() {
            GpuiEffect::FinishActivity => self.shell.finish_activity(&self.window),
            GpuiEffect::Nothing | GpuiEffect::RequestWindow | GpuiEffect::Reject { .. } => {}
        }
    }
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
        self.window.metrics.get().bounds()
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
        self.window.metrics.get().density
    }

    fn appearance(&self) -> WindowAppearance {
        WindowAppearance::Light // Not yet on Android: dark mode (Configuration.uiMode).
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(Rc::new(AndroidDisplay::from(self.window.metrics.get())))
    }

    fn mouse_position(&self) -> Point<Pixels> {
        self.window.pointers.borrow().last_position
    }

    fn modifiers(&self) -> Modifiers {
        self.window.keyboard.borrow().modifiers()
    }

    fn capslock(&self) -> Capslock {
        self.window.keyboard.borrow().capslock()
    }

    // The handler lives in the shell, where the IME's JNI calls reach it (docs/android-text-input.md).
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        self.window
            .text_input
            .borrow_mut()
            .installed(input_handler.focus_id());
        *self.window.input_handler.borrow_mut() = Some(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        self.window.input_handler.borrow_mut().take()
    }

    fn set_text_input_configuration(&mut self, configuration: TextInputConfiguration) {
        self.window.text_input.borrow_mut().configure(configuration);
    }

    fn show_soft_keyboard(&self) {
        self.window
            .text_input
            .borrow_mut()
            .notice(crate::text_input::Notice::Show);
    }

    fn hide_soft_keyboard(&self) {
        self.window
            .text_input
            .borrow_mut()
            .notice(crate::text_input::Notice::Hide);
    }

    fn text_input_state_changed(&self, change: TextInputStateChange) {
        self.window.text_input.borrow_mut().state_changed(change);
    }

    // Queued for Java, which plays it after the next vsync's frame: GPUI may be mid-dispatch here, and a long press
    // fires from a timer, where no JNI call from Java is on the stack to call back through. Dropped while frames stop
    // (`Lifecycle::runs_frames`).
    fn perform_haptic(&self, haptic: Haptic) {
        if self
            .shell
            .lifecycle
            .borrow()
            .runs_frames(self.window.activity)
        {
            self.window.haptics.borrow_mut().push(haptic);
        }
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
        self.shell
            .lifecycle
            .borrow()
            .is_active(self.window.activity)
    }

    fn visibility(&self) -> WindowVisibility {
        self.shell
            .lifecycle
            .borrow()
            .visibility(self.window.activity)
    }

    fn is_hovered(&self) -> bool {
        // Not yet on Android: Flutter's processor sends hover moves (mouse moves in GPUI), not hover enter or exit.
        false
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
        self.window.callbacks.request_frame.set(callback);
    }

    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>) {
        self.window.callbacks.input.set(callback);
    }

    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>) {
        self.window.callbacks.active_status.set(callback);
    }

    fn on_visibility_change(&self, callback: Box<dyn FnMut(WindowVisibility)>) {
        self.window.callbacks.visibility.set(callback);
    }

    fn on_hover_status_change(&self, _callback: Box<dyn FnMut(bool)>) {
        // Not yet on Android: no hover enter or exit (see is_hovered).
    }

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        self.window.callbacks.resize.set(callback);
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

    // Called when the Activity is destroyed (`Shell::lifecycle`).
    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        *self.window.callbacks.close.borrow_mut() = Some(callback);
    }

    fn on_appearance_changed(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: dark mode changes (onConfigurationChanged uiMode).
    }

    fn draw(&self, scene: &Scene) {
        // `false` means no surface or a lost device; the shell handles both on the next surface event or frame.
        let drawn = self
            .window
            .with_renderer(|renderer| renderer.draw(scene))
            .unwrap_or(false);
        if drawn && self.window.first_frame.replace(FirstFrame::Drawn) == FirstFrame::Pending {
            let size = self.window.metrics.get().size;
            let orientation = if size.width > size.height {
                "landscape"
            } else {
                "portrait"
            };
            // scripts/android-smoke.sh waits for this line, orientation included: keep its wording.
            log::info!(
                "parallax: first frame drawn on a {}x{} {orientation} surface (Activity {})",
                size.width.0,
                size.height.0,
                self.window.activity.0
            );
        }
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.atlas.clone()
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false // grayscale text: Android panels have no fixed subpixel order (rotation)
    }

    fn gpu_specs(&self) -> Option<GpuSpecs> {
        self.window
            .with_renderer(|renderer| renderer.gpu_specs())
            .flatten()
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {
        // Android pulls the caret instead: CursorAnchorInfo asks `bounds_for_range` (docs/android-text-input.md).
    }

    fn insets(&self) -> WindowInsets {
        self.window.metrics.get().insets()
    }

    fn on_insets_changed(&self, callback: Box<dyn FnMut(WindowInsets)>) {
        self.window.callbacks.insets_changed.set(callback);
    }
}
