//! cargo parallax: build and run GPUI apps on iOS, Android and web from parallax.toml. Reference: docs/cli.md.

mod android;
mod config;
mod error;
mod icon;
mod ios;

use std::path::Path;

use android::{Profile, RunArgs};
use config::Config;
use error::CliError;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Android,
    Ios,
}

/// Target name on the command line → target. To add a target: add a row and its `COMMANDS` rows.
const TARGETS: [(&str, Target); 2] = [("android", Target::Android), ("ios", Target::Ios)];

/// A command's handler: the app directory, its parsed `parallax.toml`, the flags after `<verb> <target>`.
type Handler = fn(&Path, &Config, &[String]) -> Result<(), CliError>;

/// One command: verb, target, handler.
type Command = (&'static str, Target, Handler);

/// (verb, target) → handler. To add a command: add a row.
const COMMANDS: [Command; 3] = [
    ("run", Target::Android, run_android),
    ("run", Target::Ios, ios::run),
    ("build", Target::Ios, ios::build),
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ((_, _, handler), flags) = command(&args)?;
    let app_dir = std::env::current_dir()?;
    let path = app_dir.join("parallax.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| CliError::ConfigMissing { path, source })?;
    let config = Config::parse(&text)?;
    handler(&app_dir, &config, flags)?;
    Ok(())
}

/// `[parallax] <verb> <target> <flags>…`: cargo passes the leading `parallax` when run as `cargo parallax`.
fn command(args: &[String]) -> Result<(&'static Command, &[String]), CliError> {
    let args = match args.first().map(String::as_str) {
        Some("parallax") => &args[1..],
        _ => args,
    };
    let [verb, name, flags @ ..] = args else {
        return Err(CliError::Usage("expected <verb> <target>".into()));
    };
    let target = TARGETS
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, t)| *t)
        .ok_or_else(|| CliError::Usage(format!("unknown target {name:?}")))?;
    COMMANDS
        .iter()
        .find(|(v, t, _)| v == verb && *t == target)
        .map(|row| (row, flags))
        .ok_or_else(|| CliError::Usage(format!("no command `{verb} {name}`")))
}

fn run_android(app_dir: &Path, config: &Config, flags: &[String]) -> Result<(), CliError> {
    android::run(app_dir, config, &parse_android(flags)?)
}

/// `run android` flags: `[--release] [--device <serial>]`.
fn parse_android(flags: &[String]) -> Result<RunArgs, CliError> {
    let mut rest = flags.iter();
    let mut run_args = RunArgs {
        profile: Profile::Debug,
        device: None,
    };
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--release" => run_args.profile = Profile::Release,
            "--device" => {
                let serial = rest
                    .next()
                    .ok_or_else(|| CliError::Usage("--device needs a serial".into()))?;
                run_args.device = Some(serial.clone());
            }
            other => return Err(CliError::Usage(format!("unknown argument {other:?}"))),
        }
    }
    Ok(run_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn command_table() {
        // (row name, argv after the binary, expected (verb, target, flags) or a usage error)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            Result<(&'a str, Target, &'a [&'a str]), ()>,
        );
        let cases: [Case; 9] = [
            (
                "as cargo subcommand",
                &["parallax", "run", "android"],
                Ok(("run", Target::Android, &[])),
            ),
            (
                "as binary",
                &["run", "android"],
                Ok(("run", Target::Android, &[])),
            ),
            (
                "flags pass through",
                &["parallax", "run", "ios", "--simulator", "iPhone 17"],
                Ok(("run", Target::Ios, &["--simulator", "iPhone 17"])),
            ),
            (
                "build ios",
                &["build", "ios", "--release"],
                Ok(("build", Target::Ios, &["--release"])),
            ),
            ("no build android here", &["build", "android"], Err(())),
            ("unknown target", &["run", "tvos"], Err(())),
            ("unknown verb", &["test", "ios"], Err(())),
            ("target missing", &["parallax", "run"], Err(())),
            ("no command", &["parallax"], Err(())),
        ];
        for (name, args, want) in cases {
            let args = strings(args);
            let got = command(&args)
                .map(|((verb, target, _), flags)| (*verb, *target, flags.to_vec()))
                .map_err(|e| e.variant());
            let want = want
                .map(|(verb, target, flags)| (verb, target, strings(flags)))
                .map_err(|()| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }

    #[test]
    fn parse_android_table() {
        // (row name, flags after `run android`, expected (profile dir, --device) or a usage error)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            Result<(&'a str, Option<&'a str>), ()>,
        );
        let cases: [Case; 5] = [
            ("no flags", &[], Ok(("debug", None))),
            ("release", &["--release"], Ok(("release", None))),
            (
                "device",
                &["--device", "emulator-5556"],
                Ok(("debug", Some("emulator-5556"))),
            ),
            ("device without serial", &["--device"], Err(())),
            ("unknown flag", &["--fast"], Err(())),
        ];
        for (name, flags, want) in cases {
            let got = parse_android(&strings(flags))
                .map(|a| (a.profile.dir(), a.device))
                .map_err(|e| e.variant());
            let want = want
                .map(|(profile, device)| (profile, device.map(String::from)))
                .map_err(|()| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }
}
