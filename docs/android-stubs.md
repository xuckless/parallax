# Android stubs (not yet on Android)

What the [Android backend](android.md) leaves out for now, and what GPUI does instead.

Each is marked `Not yet on Android` in the source with where it will come from: a no-op, a fixed value, or a typed
`Error::NotYet`. 42 in all (the clipboard is done: [android-clipboard.md](android-clipboard.md)):

| Area | Stubs | Comes with |
|---|---|---|
| Lifecycle | `quit`, `on_reopen`, `on_system_sleep/wake`, `on_should_close` | `Activity.finish()`, back (keys), screen off/on (`on_close` fires on `onDestroy`: [android-multi-window.md](android-multi-window.md)) |
| Input | `is_hovered`, `keyboard_layout`, `keyboard_mapper`, `on_keyboard_layout_change` | hover enter and exit, keyboard layout |
| Appearance | `appearance`, `window_appearance`, `on_appearance_changed`, `set_background_appearance`, `toggle_fullscreen`, corner radii | dark mode, immersive mode |
| System services | `open_url`, `on_open_urls`, `register_url_scheme`, `open_with_system`, `reveal_path`, `hide`, file prompts (3), credentials (3), `prevent_idle_sleep`, thermal (2), cursor (3), `app_path`, `path_for_auxiliary_executable` | JNI services, later |
| Display | `on_displays_changed`, `on_display_changed`, display `uuid`, window raw handle | multi-display |
| Other | `spawn_realtime` without realtime priority; `prompt` (GPUI draws its own) | later |

Methods with no Android meaning (`restart`, `minimize`, `zoom`, `set_title`, menus, dock, `on_moved`, window
controls, `on_quit`: Android announces no process death) are no-ops with a one-line reason, not stubs. Density changes without a resize are not tracked yet.
