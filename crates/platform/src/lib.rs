//! Picks the GPUI platform for the current target (desktop, web, iOS, Android) from one table.
//! How it works and how to extend it: `docs/platform.md`. The target → behaviour table: [`behaviour`].

pub mod behaviour;

use std::rc::Rc;

use gpui::Platform;

/// Browser panic hook and console logger. A web app calls it first in its `#[wasm_bindgen(start)]` function,
/// as upstream's web examples do; Android's `android_main!` does the same for logcat.
#[cfg(target_family = "wasm")]
pub use gpui_platform::web_init;

/// A target Parallax builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    MacOs,
    Windows,
    Linux,
    Web,
    Ios,
    Android,
}

/// Who provides a target's backend today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Upstream `gpui_platform::current_platform` builds it.
    Upstream,
    /// A Parallax crate builds it (`parallax_android::current_platform`).
    Parallax,
    /// Planned, not built yet: [`current_platform`] returns [`Error::NotYet`].
    NotYet,
}

/// One row of [`TABLE`]: a target, the crate that provides its backend, and that backend's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub target: Target,
    pub backend: &'static str,
    pub status: Status,
}

/// Target → backend. To add a target: add a `Target` variant, a row here, and its arm in [`CURRENT_TARGET`].
pub const TABLE: [Row; 6] = [
    Row {
        target: Target::MacOs,
        backend: "gpui_macos",
        status: Status::Upstream,
    },
    Row {
        target: Target::Windows,
        backend: "gpui_windows",
        status: Status::Upstream,
    },
    Row {
        target: Target::Linux,
        backend: "gpui_linux",
        status: Status::Upstream,
    },
    Row {
        target: Target::Web,
        backend: "gpui_web",
        status: Status::Upstream,
    },
    Row {
        target: Target::Ios,
        backend: "gpui_ios",
        status: Status::Upstream,
    },
    Row {
        target: Target::Android,
        backend: "parallax-android",
        status: Status::Parallax,
    },
];

/// The target this build is for, `None` when [`TABLE`] has no row for it. The only place that picks a target by `cfg`.
pub const CURRENT_TARGET: Option<Target> = cfg_select! {
    target_os = "macos" => Some(Target::MacOs),
    target_os = "windows" => Some(Target::Windows),
    target_os = "linux" => Some(Target::Linux),
    target_family = "wasm" => Some(Target::Web),
    target_os = "ios" => Some(Target::Ios),
    target_os = "android" => Some(Target::Android),
    _ => None,
};

// Not comparable on Android: `parallax_android::Error` carries OS errors.
#[derive(Debug, thiserror::Error)]
#[cfg_attr(not(target_os = "android"), derive(Clone, Copy, PartialEq, Eq))]
pub enum Error {
    #[error("Parallax has no platform for this target (no row in parallax_platform::TABLE)")]
    UnsupportedTarget,
    #[error("the {target:?} backend ({backend}) is not built yet")]
    NotYet {
        target: Target,
        backend: &'static str,
    },
    #[cfg(target_os = "android")]
    #[error(transparent)]
    Android(#[from] parallax_android::Error),
}

/// Returns `target`'s row when its backend is available, or the reason it is not.
pub fn lookup(target: Option<Target>) -> Result<&'static Row, Error> {
    let row = TABLE
        .iter()
        .find(|row| Some(row.target) == target)
        .ok_or(Error::UnsupportedTarget)?;
    match row.status {
        Status::Upstream | Status::Parallax => Ok(row),
        Status::NotYet => Err(Error::NotYet {
            target: row.target,
            backend: row.backend,
        }),
    }
}

/// Returns the GPUI platform for the current target. Pass it to `gpui::Application::with_platform`.
pub fn current_platform(headless: bool) -> Result<Rc<dyn Platform>, Error> {
    #[cfg(target_os = "android")]
    let _ = headless;
    let row = lookup(CURRENT_TARGET)?;
    let not_built_here = Error::NotYet {
        target: row.target,
        backend: row.backend,
    };
    match row.status {
        // gpui_platform is not a dependency on Android (see Cargo.toml): an Upstream row there has nothing to call.
        Status::Upstream => cfg_select! {
            target_os = "android" => Err(not_built_here),
            _ => Ok(gpui_platform::current_platform(headless)),
        },
        // parallax-android is a dependency only on Android. It has no headless mode: the Activity owns the window.
        Status::Parallax => cfg_select! {
            target_os = "android" => Ok(parallax_android::current_platform()?),
            _ => Err(not_built_here),
        },
        Status::NotYet => Err(not_built_here),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_table() {
        // (row name, target, expected backend or error). Errors compare as text: on Android they are not `PartialEq`.
        let cases: [(&str, Option<Target>, Result<&str, Error>); 7] = [
            ("macos_upstream", Some(Target::MacOs), Ok("gpui_macos")),
            (
                "windows_upstream",
                Some(Target::Windows),
                Ok("gpui_windows"),
            ),
            ("linux_upstream", Some(Target::Linux), Ok("gpui_linux")),
            ("web_upstream", Some(Target::Web), Ok("gpui_web")),
            ("ios_upstream", Some(Target::Ios), Ok("gpui_ios")),
            (
                "android_parallax",
                Some(Target::Android),
                Ok("parallax-android"),
            ),
            ("unsupported_target", None, Err(Error::UnsupportedTarget)),
        ];
        for (name, target, expected) in cases {
            let actual = lookup(target).map(|row| row.backend);
            assert_eq!(
                actual.map_err(|error| error.to_string()),
                expected.map_err(|error| error.to_string()),
                "{name}"
            );
        }
    }
}
