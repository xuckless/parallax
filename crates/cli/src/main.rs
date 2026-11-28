//! cargo parallax: build and run GPUI apps on Android, macOS, Linux and Windows from parallax.toml. Reference:
//! docs/cli.md.

mod android;
mod config;
mod desktop;
mod error;
mod icon;
mod macos;

use std::path::Path;

use android::{Profile, RunArgs};
use config::Config;
use error::CliError;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Verb {
    Run,
    Build,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Android,
    Macos,
    Linux,
    Windows,
}

/// Verb name → verb. To add a verb: add a row and its `DISPATCH` rows.
const VERBS: [(&str, Verb); 2] = [("run", Verb::Run), ("build", Verb::Build)];

struct TargetRow {
    name: &'static str,
    target: Target,
    /// `std::env::consts::OS` the target builds on, for desktop targets (no cross builds); `None`: any host.
    /// `run` with no target picks the row whose `host` is this machine's OS.
    host: Option<&'static str>,
    /// Whether `--device <serial>` means anything for this target.
    device: bool,
}

/// Target name → target. To add a target: add a row and its `DISPATCH` rows.
const TARGETS: [TargetRow; 4] = [
    TargetRow {
        name: "android",
        target: Target::Android,
        host: None,
        device: true,
    },
    TargetRow {
        name: "macos",
        target: Target::Macos,
        host: Some("macos"),
        device: false,
    },
    TargetRow {
        name: "linux",
        target: Target::Linux,
        host: Some("linux"),
        device: false,
    },
    TargetRow {
        name: "windows",
        target: Target::Windows,
        host: Some("windows"),
        device: false,
    },
];

type Handler = fn(&Path, &Config, &RunArgs) -> Result<(), CliError>;
type Dispatch = (Verb, Target, Handler);

/// (verb, target) → what runs. A pair with no row is a usage error.
const DISPATCH: [Dispatch; 7] = [
    (Verb::Run, Target::Android, android::run),
    (Verb::Run, Target::Macos, macos::run),
    (Verb::Build, Target::Macos, macos::build),
    (Verb::Run, Target::Linux, desktop::run),
    (Verb::Build, Target::Linux, desktop::build),
    (Verb::Run, Target::Windows, desktop::run),
    (Verb::Build, Target::Windows, desktop::build),
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ((_, _, handler), run_args) = parse_args(&args, std::env::consts::OS)?;
    let app_dir = std::env::current_dir()?;
    let path = app_dir.join("parallax.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| CliError::ConfigMissing { path, source })?;
    let config = Config::parse(&text)?;
    handler(&app_dir, &config, &run_args)?;
    Ok(())
}

/// `[parallax] <verb> [<target>] [--release] [--device <serial>]` on host OS `host`; cargo passes the leading
/// `parallax` when run as `cargo parallax`.
fn parse_args(args: &[String], host: &str) -> Result<(&'static Dispatch, RunArgs), CliError> {
    let args = match args.first().map(String::as_str) {
        Some("parallax") => &args[1..],
        _ => args,
    };
    let (verb, rest) = match args {
        [verb, rest @ ..] => (verb, rest),
        [] => return Err(CliError::Usage("no command".into())),
    };
    let verb = VERBS
        .iter()
        .find(|(name, _)| name == verb)
        .map(|(_, v)| *v)
        .ok_or_else(|| CliError::Usage(format!("unknown command {verb:?}")))?;
    let (row, rest) = match rest {
        [target, rest @ ..] if !target.starts_with("--") => (
            TARGETS
                .iter()
                .find(|t| t.name == target)
                .ok_or_else(|| CliError::Usage(format!("unknown target {target:?}")))?,
            rest,
        ),
        _ => (
            TARGETS
                .iter()
                .find(|t| t.host == Some(host))
                .ok_or(CliError::NoHostDesktop(host.to_string()))?,
            rest,
        ),
    };
    if let Some(needed) = row.host.filter(|needed| *needed != host) {
        return Err(CliError::WrongHost {
            target: row.name,
            needed,
            host: host.to_string(),
        });
    }
    let dispatch = DISPATCH
        .iter()
        .find(|(v, t, _)| *v == verb && *t == row.target)
        .ok_or_else(|| CliError::Usage(format!("no {verb:?} for {}", row.name)))?;
    let mut run_args = RunArgs {
        profile: Profile::Debug,
        device: None,
    };
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--release" => run_args.profile = Profile::Release,
            "--device" if row.device => {
                let serial = rest
                    .next()
                    .ok_or_else(|| CliError::Usage("--device needs a serial".into()))?;
                run_args.device = Some(serial.clone());
            }
            other => {
                return Err(CliError::Usage(format!(
                    "unknown argument {other:?} for {}",
                    row.name
                )));
            }
        }
    }
    Ok((dispatch, run_args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_table() {
        // (row name, argv after the binary, host OS, expected (DISPATCH row, profile dir, --device) or an error variant)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            &'a str,
            Result<((Verb, Target), &'a str, Option<&'a str>), &'a str>,
        );
        let cases: [Case; 22] = [
            (
                "as cargo subcommand",
                &["parallax", "run", "android"],
                "linux",
                Ok(((Verb::Run, Target::Android), "debug", None)),
            ),
            (
                "as binary",
                &["run", "android"],
                "linux",
                Ok(((Verb::Run, Target::Android), "debug", None)),
            ),
            (
                "release",
                &["parallax", "run", "android", "--release"],
                "linux",
                Ok(((Verb::Run, Target::Android), "release", None)),
            ),
            (
                "device",
                &["run", "android", "--device", "emulator-5556"],
                "macos",
                Ok(((Verb::Run, Target::Android), "debug", Some("emulator-5556"))),
            ),
            (
                "device without serial",
                &["run", "android", "--device"],
                "linux",
                Err("Usage"),
            ),
            (
                "unknown flag",
                &["run", "android", "--fast"],
                "linux",
                Err("Usage"),
            ),
            ("unknown target", &["run", "tvos"], "linux", Err("Usage")),
            (
                "unknown command",
                &["serve", "macos"],
                "macos",
                Err("Usage"),
            ),
            ("no command", &["parallax"], "linux", Err("Usage")),
            (
                "build android has no row",
                &["build", "android"],
                "linux",
                Err("Usage"),
            ),
            (
                "run macos",
                &["parallax", "run", "macos"],
                "macos",
                Ok(((Verb::Run, Target::Macos), "debug", None)),
            ),
            (
                "build macos --release",
                &["build", "macos", "--release"],
                "macos",
                Ok(((Verb::Build, Target::Macos), "release", None)),
            ),
            (
                "macos off a Mac",
                &["build", "macos"],
                "linux",
                Err("WrongHost"),
            ),
            (
                "--device is Android only",
                &["run", "macos", "--device", "x"],
                "macos",
                Err("Usage"),
            ),
            (
                "run linux",
                &["run", "linux", "--release"],
                "linux",
                Ok(((Verb::Run, Target::Linux), "release", None)),
            ),
            (
                "build linux",
                &["build", "linux"],
                "linux",
                Ok(((Verb::Build, Target::Linux), "debug", None)),
            ),
            (
                "run windows",
                &["run", "windows"],
                "windows",
                Ok(((Verb::Run, Target::Windows), "debug", None)),
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
                Ok(((Verb::Run, Target::Macos), "debug", None)),
            ),
            (
                "no target, flags only: host desktop on Linux",
                &["run", "--release"],
                "linux",
                Ok(((Verb::Run, Target::Linux), "release", None)),
            ),
            (
                "no target: host desktop on Windows",
                &["build"],
                "windows",
                Ok(((Verb::Build, Target::Windows), "debug", None)),
            ),
            (
                "no target on a host with no desktop row",
                &["run"],
                "freebsd",
                Err("NoHostDesktop"),
            ),
        ];
        for (name, args, host, want) in cases {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let got = parse_args(&args, host)
                .map(|((v, t, _), a)| ((*v, *t), a.profile.dir(), a.device))
                .map_err(|e| e.variant());
            let want = want
                .map(|(row, profile, device)| (row, profile, device.map(String::from)))
                .map_err(String::from);
            assert_eq!(got, want, "{name}");
        }
    }
}
