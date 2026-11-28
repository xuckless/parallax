//! cargo parallax: build and run GPUI apps on iOS, Android and web from parallax.toml. Reference: docs/cli.md.

mod android;
mod config;
mod error;
mod icon;

use std::path::Path;

use config::Config;
use error::CliError;

/// A platform `cargo parallax` builds for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Android,
}

/// Target name on the command line → target. To add a target: add its variant and a row.
const TARGETS: [(&str, Target); 1] = [("android", Target::Android)];

/// One `cargo parallax <verb> <target>` command; the handler parses the flags after the target.
struct Command {
    verb: &'static str,
    target: Target,
    handler: fn(&Path, &Config, &[String]) -> Result<(), CliError>,
}

/// (verb, target) → handler. To add a command: add a row.
const COMMANDS: [Command; 2] = [
    Command {
        verb: "run",
        target: Target::Android,
        handler: android::run,
    },
    Command {
        verb: "build",
        target: Target::Android,
        handler: android::build,
    },
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, flags) = parse_command(&args)?;
    let app_dir = std::env::current_dir()?;
    let path = app_dir.join("parallax.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| CliError::ConfigMissing { path, source })?;
    let config = Config::parse(&text)?;
    (command.handler)(&app_dir, &config, flags)?;
    Ok(())
}

/// `[parallax] <verb> <target> <flags…>`; cargo passes the leading `parallax` when run as `cargo parallax`.
fn parse_command(args: &[String]) -> Result<(&'static Command, &[String]), CliError> {
    let args = match args.first().map(String::as_str) {
        Some("parallax") => &args[1..],
        _ => args,
    };
    let [verb, target, flags @ ..] = args else {
        return Err(CliError::Usage("expected a command and a target".into()));
    };
    let target = TARGETS
        .iter()
        .find(|(name, _)| name == target)
        .map(|(_, t)| *t)
        .ok_or_else(|| CliError::Usage(format!("unknown target {target:?}")))?;
    let command = COMMANDS
        .iter()
        .find(|c| c.verb == verb && c.target == target)
        .ok_or_else(|| CliError::Usage(format!("unknown command {verb:?}")))?;
    Ok((command, flags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_table() {
        // (row name, argv after the binary, expected (verb, target, flags) or a usage error)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            Result<(&'a str, Target, &'a [&'a str]), ()>,
        );
        let cases: [Case; 8] = [
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
                "flags go to the handler",
                &["parallax", "build", "android", "--release", "--apk"],
                Ok(("build", Target::Android, &["--release", "--apk"])),
            ),
            (
                "build",
                &["build", "android"],
                Ok(("build", Target::Android, &[])),
            ),
            ("unknown verb", &["install", "android"], Err(())),
            ("unknown target", &["run", "symbian"], Err(())),
            ("no target", &["parallax", "run"], Err(())),
            ("no command", &["parallax"], Err(())),
        ];
        for (name, args, want) in cases {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let got = parse_command(&args)
                .map(|(c, flags)| (c.verb, c.target, flags.to_vec()))
                .map_err(|e| e.variant());
            let want = want
                .map(|(verb, target, flags)| {
                    (verb, target, flags.iter().map(|s| s.to_string()).collect())
                })
                .map_err(|()| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }
}
