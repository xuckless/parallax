//! GPUI's `Platform` on Android. Started by `Application::run_embedded`: the Android main Looper owns the loop.
//! Stubs are marked "Not yet on Android" (a no-op or [`Error::NotYet`]) and listed in `docs/android.md`.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use futures::channel::oneshot;
use gpui::{
    Action, ActivityGuard, AnyWindowHandle, AppLifecyclePhase, BackgroundExecutor, Bounds,
    ClipboardItem, CursorStyle, DisplayId, DummyKeyboardMapper, ForegroundExecutor, Keymap, Menu,
    MenuItem, OperatingSystem, PathPromptOptions, Pixels, Platform, PlatformDisplay, PlatformInfo,
    PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTarget, PlatformTextSystem,
    PlatformWindow, Task, ThermalState, WindowAppearance, WindowParams,
};

use gpui_wgpu::CosmicTextSystem;
use jni::{
    JNIEnv,
    objects::{JObject, JString, JValue},
};

use crate::{
    Error, dispatcher::AndroidDispatcher, host::Shell, metrics::Metrics,
    text_system::AndroidTextSystem, window::AndroidWindow,
};

pub(crate) struct AndroidPlatform {
    shell: Rc<Shell>,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
}

impl AndroidPlatform {
    /// `fonts`: the system fonts ([`crate::text_system::load`]); the host keeps a handle to follow locale changes.
    /// `None` (they did not load, logged): upstream's text system without system fonts, so text still renders with
    /// fonts the app adds.
    pub(crate) fn new(
        shell: Rc<Shell>,
        fonts: Option<Arc<AndroidTextSystem>>,
    ) -> Result<Self, Error> {
        let dispatcher = Arc::new(AndroidDispatcher::new()?);
        let text_system: Arc<dyn PlatformTextSystem> = match fonts {
            Some(fonts) => fonts,
            None => Arc::new(CosmicTextSystem::new_without_system_fonts("sans-serif")),
        };
        Ok(Self {
            shell,
            background_executor: BackgroundExecutor::new(dispatcher.clone()),
            foreground_executor: ForegroundExecutor::new(dispatcher),
            text_system,
        })
    }

    /// Runs `f` with the newest Activity's Java `PlatformPlugin` (the clipboard is the system's, not one Activity's,
    /// so any live one serves) in its own local frame; `None` with no live Activity. A Java exception becomes an
    /// error and is cleared.
    fn platform_plugin<T>(
        &self,
        f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>,
    ) -> Result<Option<T>, Error> {
        let Some(window) = self.shell.newest_window() else {
            return Ok(None);
        };
        let mut env = self.shell.vm.get_env()?;
        let result = env.with_local_frame(8, |env| f(env, window.platform_plugin.as_obj()));
        if result.is_err() && env.exception_check()? {
            env.exception_clear()?;
        }
        Ok(Some(result?))
    }
}

/// The one display: an Activity's window as Android last reported it.
#[derive(Debug)]
pub(crate) struct AndroidDisplay {
    bounds: Bounds<Pixels>,
    refresh_interval: Option<Duration>,
}

impl From<Metrics> for AndroidDisplay {
    fn from(metrics: Metrics) -> Self {
        Self {
            bounds: metrics.bounds(),
            refresh_interval: metrics.refresh_interval(),
        }
    }
}

impl PlatformDisplay for AndroidDisplay {
    fn id(&self) -> DisplayId {
        DisplayId::new(0) // Android's default display
    }

    fn uuid(&self) -> anyhow::Result<uuid::Uuid> {
        Err(Error::NotYet("display uuid").into())
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    fn refresh_interval(&self) -> Option<Duration> {
        self.refresh_interval
    }
}

struct AndroidKeyboardLayout;

impl PlatformKeyboardLayout for AndroidKeyboardLayout {
    fn id(&self) -> &str {
        "android" // Not yet on Android: the IME's layout (KeyboardManager port).
    }

    fn name(&self) -> &str {
        "android"
    }
}

impl Platform for AndroidPlatform {
    fn platform_info(&self) -> PlatformInfo {
        PlatformInfo {
            os: OperatingSystem::Android,
            target: PlatformTarget::Native,
        }
    }

    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        // Embedded: the Looper already runs; launch and return (`Application::run_embedded`).
        on_finish_launching();
    }

    fn quit(&self) {
        // Not yet on Android: Activity.finish() over JNI (Full Android lifecycle).
    }

    fn restart(&self, _binary_path: Option<PathBuf>, _arguments: Vec<OsString>) {
        // Android apps cannot restart themselves.
    }

    fn activate(&self, _ignoring_other_apps: bool) {
        // Android brings the app forward; apps cannot.
    }

    fn hide(&self) {
        // Not yet on Android: moveTaskToBack over JNI.
    }

    fn hide_other_apps(&self) {
        // Android apps cannot hide others.
    }

    fn unhide_other_apps(&self) {
        // Android apps cannot hide others.
    }

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        self.primary_display().into_iter().collect()
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        let metrics = self
            .shell
            .newest_window()
            .map_or(Metrics::BEFORE_FIRST_SURFACE, |window| window.metrics.get());
        Some(Rc::new(AndroidDisplay::from(metrics)))
    }

    fn on_displays_changed(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: DisplayManager listener.
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        self.shell.active_window()
    }

    // One window per Activity: the app's `open_window` (`android_main!`), run on that Activity's first surface,
    // opens it.
    fn open_window(
        &self,
        handle: AnyWindowHandle,
        _options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let (window, atlas) = self.shell.open_window(handle)?;
        Ok(Box::new(AndroidWindow {
            shell: self.shell.clone(),
            window,
            atlas,
        }))
    }

    fn window_appearance(&self) -> WindowAppearance {
        WindowAppearance::Light // Not yet on Android: dark mode (Configuration.uiMode).
    }

    fn open_url(&self, _url: &str) {
        // Not yet on Android: an ACTION_VIEW intent over JNI.
    }

    fn on_open_urls(&self, _callback: Box<dyn FnMut(Vec<String>)>) {
        // Not yet on Android: deep links (onNewIntent).
    }

    fn register_url_scheme(&self, _url: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Err(Error::NotYet("register_url_scheme").into()))
    }

    fn prompt_for_paths(
        &self,
        _options: PathPromptOptions,
    ) -> oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
        not_yet_receiver("prompt_for_paths")
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<anyhow::Result<Option<PathBuf>>> {
        not_yet_receiver("prompt_for_new_path")
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false // Not yet on Android: the storage access framework.
    }

    fn reveal_path(&self, _path: &Path) {
        // Not yet on Android: there is no file manager to reveal in.
    }

    fn open_with_system(&self, _path: &Path) {
        // Not yet on Android: an ACTION_VIEW intent with a content URI.
    }

    fn on_quit(&self, _callback: Box<dyn FnMut() -> bool>) {
        // Android never announces process death, and onDestroy is `Disconnected`, not quit (docs/android-lifecycle.md).
    }

    fn on_reopen(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: onRestart (Full Android lifecycle).
    }

    fn on_system_sleep(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: screen off (Full Android lifecycle).
    }

    fn on_system_wake(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: screen on (Full Android lifecycle).
    }

    fn on_app_lifecycle(&self, callback: Box<dyn FnMut(AppLifecyclePhase)>) {
        self.shell.callbacks.app_lifecycle.set(callback);
    }

    fn on_memory_warning(&self, callback: Box<dyn FnMut()>) {
        self.shell.callbacks.memory_warning.set(callback);
    }

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {
        // Android has no menu bar.
    }

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {
        // Android has no dock.
    }

    fn on_app_menu_action(&self, _callback: Box<dyn FnMut(&dyn Action)>) {
        // Android has no menu bar.
    }

    fn on_will_open_app_menu(&self, _callback: Box<dyn FnMut()>) {
        // Android has no menu bar.
    }

    fn on_validate_app_menu_command(&self, _callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        // Android has no menu bar.
    }

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal // Not yet on Android: PowerManager thermal status.
    }

    fn on_thermal_state_change(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: PowerManager thermal listener.
    }

    fn prevent_idle_sleep(&self, _reason: &str) -> Task<anyhow::Result<ActivityGuard>> {
        Task::ready(Err(Error::NotYet("prevent_idle_sleep").into()))
    }

    fn app_path(&self) -> anyhow::Result<PathBuf> {
        Err(Error::NotYet("app_path").into())
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> anyhow::Result<PathBuf> {
        Err(Error::NotYet("path_for_auxiliary_executable").into())
    }

    fn set_cursor_style(&self, _style: CursorStyle) {
        // Not yet on Android: PointerIcon for mouse and stylus.
    }

    fn hide_cursor_until_mouse_moves(&self) {
        // Not yet on Android: PointerIcon for mouse and stylus.
    }

    fn is_cursor_visible(&self) -> bool {
        false // touch first; no pointer icon yet
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        true // Android hides scrollbars when idle
    }

    // Plain text, as Flutter's Android clipboard: `PlatformPlugin.getClipboardData` with no format (its
    // channel's default, plain text) (docs/android-clipboard.md).
    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        let read = self.platform_plugin(|env, plugin| {
            let format = JObject::null();
            let text = env
                .call_method(
                    plugin,
                    "getClipboardData",
                    "(Lio/github/xuckless/parallax/PlatformChannel$ClipboardContentFormat;)Ljava/lang/CharSequence;",
                    &[JValue::Object(&format)],
                )?
                .l()?;
            if text.is_null() {
                return Ok(None);
            }
            let text = JString::from(
                env.call_method(&text, "toString", "()Ljava/lang/String;", &[])?
                    .l()?,
            );
            Ok(Some(String::from(env.get_string(&text)?)))
        });
        match read {
            Ok(text) => text.flatten().map(ClipboardItem::new_string),
            Err(error) => {
                log::error!("parallax: cannot read the clipboard: {error}");
                None
            }
        }
    }

    // `PlatformPlugin.setClipboardData`: the item's text; GPUI's metadata and images do not reach Android's
    // clipboard (docs/android-clipboard.md).
    fn write_to_clipboard(&self, item: ClipboardItem) {
        let Some(text) = item.text() else {
            return log::warn!("parallax: only text reaches the Android clipboard");
        };
        let written = self.platform_plugin(|env, plugin| {
            let text = env.new_string(text)?;
            env.call_method(
                plugin,
                "setClipboardData",
                "(Ljava/lang/String;)V",
                &[JValue::Object(&text)],
            )?;
            Ok(())
        });
        if let Err(error) = written {
            log::error!("parallax: cannot write the clipboard: {error}");
        }
    }

    fn write_credentials(
        &self,
        _url: &str,
        _username: &str,
        _password: &[u8],
    ) -> Task<anyhow::Result<()>> {
        Task::ready(Err(Error::NotYet("write_credentials").into()))
    }

    fn read_credentials(&self, _url: &str) -> Task<anyhow::Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Err(Error::NotYet("read_credentials").into()))
    }

    fn delete_credentials(&self, _url: &str) -> Task<anyhow::Result<()>> {
        Task::ready(Err(Error::NotYet("delete_credentials").into()))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(AndroidKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper) // Not yet on Android: key mapping (KeyboardManager port).
    }

    fn on_keyboard_layout_change(&self, _callback: Box<dyn FnMut()>) {
        // Not yet on Android: IME subtype changes.
    }
}

/// A receiver already holding the typed "not yet on Android" error, for the file prompts.
fn not_yet_receiver<T>(what: &'static str) -> oneshot::Receiver<anyhow::Result<T>> {
    let (sender, receiver) = oneshot::channel();
    // The receiver is alive right here, so this send cannot fail.
    let _ = sender.send(Err(Error::NotYet(what).into()));
    receiver
}
