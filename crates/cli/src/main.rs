//! cargo parallax: build and run GPUI apps on Android, iOS, the web, macOS, Linux and Windows from parallax.toml.
//! Reference: docs/cli.md.

mod android;
mod cargo;
mod config;
mod desktop;
mod error;
mod icon;
mod ios;
mod macos;
mod template;
mod version;
mod web;

use std::path::Path;

use config::Config;
use error::CliError;

/// A platform `cargo parallax` builds for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Android,
    Ios,
    Web,
    Macos,
    Linux,
    Windows,
}

struct TargetRow {
    name: &'static str,
    target: Target,
    /// `std::env::consts::OS` the target builds on (no cross builds); `None`: any host.
    host: Option<&'static str>,
    /// A desktop: `<verb>` with no target picks the desktop whose `host` is this machine's OS.
    desktop: bool,
}

/// Target name on the command line → target. To add a target: add its variant, a row and its `COMMANDS` rows.
const TARGETS: [TargetRow; 6] = [
    TargetRow {
        name: "android",
        target: Target::Android,
        host: None,
        desktop: false,
    },
    TargetRow {
        name: "ios",
        target: Target::Ios,
        host: Some("macos"),
        desktop: false,
    },
    TargetRow {
        name: "web",
        target: Target::Web,
        host: None,
        desktop: false,
    },
    TargetRow {
        name: "macos",
        target: Target::Macos,
        host: Some("macos"),
        desktop: true,
    },
    TargetRow {
        name: "linux",
        target: Target::Linux,
        host: Some("linux"),
        desktop: true,
    },
    TargetRow {
        name: "windows",
        target: Target::Windows,
        host: Some("windows"),
        desktop: true,
    },
];

/// A command's work: the app directory, its parsed `parallax.toml`, the flags after `<verb> <target>` and the app's
/// own arguments (after `--`, `run` only). Each handler parses its own flags.
type Handler = fn(&Path, &Config, &[String], &[String]) -> Result<(), CliError>;

/// One `cargo parallax <verb> <target>` command: verb, target, handler.
type Command = (&'static str, Target, Handler);

/// (verb, target) → handler. To add a command: add a row and its line to `USAGE` in error.rs.
const COMMANDS: [Command; 12] = [
    ("run", Target::Android, android::run),
    ("build", Target::Android, android::build),
    ("run", Target::Ios, ios::run),
    ("build", Target::Ios, ios::build),
    ("run", Target::Web, web::run),
    ("build", Target::Web, web::build),
    ("run", Target::Macos, macos::run),
    ("build", Target::Macos, macos::build),
    ("run", Target::Linux, desktop::run),
    ("build", Target::Linux, desktop::build),
    ("run", Target::Windows, desktop::run),
    ("build", Target::Windows, desktop::build),
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ((_, _, handler), flags, app_args) = command(&args, std::env::consts::OS)?;
    let app_dir = std::env::current_dir()?;
    let path = app_dir.join("parallax.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| CliError::ConfigMissing { path, source })?;
    let config = Config::parse(&text)?;
    handler(&app_dir, &config, flags, app_args)?;
    Ok(())
}

/// The arguments of a command: its flags, then the app's own arguments after `--`.
type Parsed<'a> = (&'static Command, &'a [String], &'a [String]);

/// `[parallax] <verb> [<target>] <flags…> [-- <app args…>]` on host OS `host`; cargo passes the leading `parallax`
/// when run as `cargo parallax`. No target (nothing, or a flag, after the verb): this machine's desktop.
fn command<'a>(args: &'a [String], host: &str) -> Result<Parsed<'a>, CliError> {
    let args = match args.first().map(String::as_str) {
        Some("parallax") => &args[1..],
        _ => args,
    };
    let [verb, rest @ ..] = args else {
        return Err(CliError::Usage("expected a command".into()));
    };
    let (row, flags) = match rest {
        [name, flags @ ..] if !name.starts_with("--") => (
            TARGETS
                .iter()
                .find(|t| t.name == name)
                .ok_or_else(|| CliError::Usage(format!("unknown target {name:?}")))?,
            flags,
        ),
        flags => (
            TARGETS
                .iter()
                .find(|t| t.desktop && t.host == Some(host))
                .ok_or(CliError::NoHostDesktop(host.to_string()))?,
            flags,
        ),
    };
    let command = COMMANDS
        .iter()
        .find(|(v, t, _)| v == verb && *t == row.target)
        .ok_or_else(|| CliError::Usage(format!("unknown command `{verb} {}`", row.name)))?;
    if let Some(needed) = row.host.filter(|needed| *needed != host) {
        return Err(CliError::WrongHost {
            target: row.name,
            needed,
            host: host.to_string(),
        });
    }
    let (flags, app_args) = match flags.iter().position(|a| a == "--") {
        Some(at) => (&flags[..at], &flags[at + 1..]),
        None => (flags, &flags[flags.len()..]),
    };
    if *verb != "run" && !app_args.is_empty() {
        return Err(CliError::Usage(
            "app arguments after `--` go with `run` only".into(),
        ));
    }
    Ok((command, flags, app_args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_table() {
        // (row name, argv after the binary, host OS, expected (verb, target, flags, then `--` and the app's
        // arguments when there are any) or the error variant)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            &'a str,
            Result<(&'a str, Target, &'a [&'a str]), &'a str>,
        );
        let cases: [Case; 26] = [
            (
                "as cargo subcommand",
                &["parallax", "run", "android"],
                "linux",
                Ok(("run", Target::Android, &[])),
            ),
            (
                "as binary",
                &["run", "android"],
                "linux",
                Ok(("run", Target::Android, &[])),
            ),
            (
                "flags go to the handler",
                &["parallax", "build", "android", "--release", "--apk"],
                "windows",
                Ok(("build", Target::Android, &["--release", "--apk"])),
            ),
            (
                "run ios on a Mac",
                &["run", "ios", "--simulator", "iPad Air"],
                "macos",
                Ok(("run", Target::Ios, &["--simulator", "iPad Air"])),
            ),
            (
                "build ios on a Mac",
                &["build", "ios"],
                "macos",
                Ok(("build", Target::Ios, &[])),
            ),
            ("ios off a Mac", &["run", "ios"], "linux", Err("WrongHost")),
            (
                "run web anywhere",
                &["run", "web", "--port", "9000"],
                "windows",
                Ok(("run", Target::Web, &["--port", "9000"])),
            ),
            (
                "build web",
                &["build", "web", "--release"],
                "linux",
                Ok(("build", Target::Web, &["--release"])),
            ),
            (
                "run macos",
                &["parallax", "run", "macos"],
                "macos",
                Ok(("run", Target::Macos, &[])),
            ),
            (
                "build macos --release",
                &["build", "macos", "--release"],
                "macos",
                Ok(("build", Target::Macos, &["--release"])),
            ),
            (
                "macos off a Mac",
                &["build", "macos"],
                "linux",
                Err("WrongHost"),
            ),
            (
                "run linux",
                &["run", "linux"],
                "linux",
                Ok(("run", Target::Linux, &[])),
            ),
            (
                "windows off Windows",
                &["run", "windows"],
                "linux",
                Err("WrongHost"),
            ),
            (
                "no target: host desktop on macOS",
                &["parallax", "run"],
                "macos",
                Ok(("run", Target::Macos, &[])),
            ),
            (
                "no target, flags only: host desktop on Linux",
                &["run", "--release"],
                "linux",
                Ok(("run", Target::Linux, &["--release"])),
            ),
            (
                "no target: host desktop on Windows",
                &["build"],
                "windows",
                Ok(("build", Target::Windows, &[])),
            ),
            (
                "no target on a host with no desktop row",
                &["run"],
                "freebsd",
                Err("NoHostDesktop"),
            ),
            (
                "unknown verb",
                &["install", "android"],
                "linux",
                Err("Usage"),
            ),
            (
                "unknown verb before a host check",
                &["serve", "ios"],
                "linux",
                Err("Usage"),
            ),
            ("unknown target", &["run", "symbian"], "linux", Err("Usage")),
            ("no command", &["parallax"], "linux", Err("Usage")),
            ("nothing", &[], "linux", Err("Usage")),
            (
                "app arguments after --",
                &["run", "windows", "--release", "--", "--view", "webview2"],
                "windows",
                Ok((
                    "run",
                    Target::Windows,
                    &["--release", "--", "--view", "webview2"],
                )),
            ),
            (
                "app arguments, no target",
                &["run", "--", "--view", "webview2"],
                "linux",
                Ok(("run", Target::Linux, &["--", "--view", "webview2"])),
            ),
            (
                "bare -- is no app argument",
                &["run", "android", "--"],
                "linux",
                Ok(("run", Target::Android, &[])),
            ),
            (
                "app arguments with build",
                &["build", "macos", "--", "x"],
                "macos",
                Err("Usage"),
            ),
        ];
        for (name, args, host, want) in cases {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let got = command(&args, host)
                .map(|((verb, target, _), flags, app)| {
                    let app = match app {
                        [] => Vec::new(),
                        app => [&["--".to_string()], app].concat(),
                    };
                    (*verb, *target, [flags, app.as_slice()].concat())
                })
                .map_err(|e| e.variant());
            let want = want
                .map(|(verb, target, flags)| {
                    (verb, target, flags.iter().map(|s| s.to_string()).collect())
                })
                .map_err(String::from);
            assert_eq!(got, want, "{name}");
        }
    }
}
