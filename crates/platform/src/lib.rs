//! Picks the GPUI platform for the current target (desktop, web, iOS, Android) from one table.
//! How it works and how to extend it: `docs/platform.md`.

use std::rc::Rc;

use gpui::Platform;

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
        status: Status::NotYet,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Parallax has no platform for this target (no row in parallax_platform::TABLE)")]
    UnsupportedTarget,
    #[error("the {target:?} backend ({backend}) is not built yet")]
    NotYet {
        target: Target,
        backend: &'static str,
    },
}

/// Returns `target`'s row when its backend is available, or the reason it is not.
pub fn lookup(target: Option<Target>) -> Result<&'static Row, Error> {
    let row = TABLE
        .iter()
        .find(|row| Some(row.target) == target)
        .ok_or(Error::UnsupportedTarget)?;
    match row.status {
        Status::Upstream => Ok(row),
        Status::NotYet => Err(Error::NotYet {
            target: row.target,
            backend: row.backend,
        }),
    }
}

/// Returns the GPUI platform for the current target. Pass it to `gpui::Application::with_platform`.
pub fn current_platform(headless: bool) -> Result<Rc<dyn Platform>, Error> {
    let row = lookup(CURRENT_TARGET)?;
    cfg_select! {
        // gpui_platform is not a dependency on Android (see Cargo.toml): an Upstream row there has nothing to call.
        target_os = "android" => {
            let _ = headless;
            Err(Error::NotYet { target: row.target, backend: "gpui_platform" })
        }
        _ => {
            let _ = row;
            Ok(gpui_platform::current_platform(headless))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_table() {
        // (row name, target, expected backend or error)
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
                "android_not_yet",
                Some(Target::Android),
                Err(Error::NotYet {
                    target: Target::Android,
                    backend: "parallax-android",
                }),
            ),
            ("unsupported_target", None, Err(Error::UnsupportedTarget)),
        ];
        for (name, target, expected) in cases {
            assert_eq!(lookup(target).map(|row| row.backend), expected, "{name}");
        }
    }
}
