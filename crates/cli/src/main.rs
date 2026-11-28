//! cargo parallax: build and run GPUI apps on iOS, Android and web from parallax.toml. Reference: docs/cli.md.

mod android;
mod config;
mod error;
mod icon;
mod web;

use std::path::Path;

use config::Config;
use error::CliError;

/// One command's work: the app directory, its parsed `parallax.toml`, the flags after `<verb> <target>`.
type Handler = fn(&Path, &Config, &[String]) -> Result<(), CliError>;

/// The targets `cargo parallax` builds for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Android,
    Web,
}

/// (verb, target name, target, handler): every command. Each handler parses its own flags. To add one: add a row
/// (and its usage line to `USAGE` in error.rs).
const COMMANDS: [(&str, &str, Target, Handler); 3] = [
    ("run", "android", Target::Android, android::run),
    ("run", "web", Target::Web, web::run),
    ("build", "web", Target::Web, web::build),
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (_, handler, flags) = command(&args)?;
    let app_dir = std::env::current_dir()?;
    let path = app_dir.join("parallax.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| CliError::ConfigMissing { path, source })?;
    let config = Config::parse(&text)?;
    handler(&app_dir, &config, flags)?;
    Ok(())
}

/// `[parallax] <verb> <target> <flags…>`; cargo passes the leading `parallax` when run as `cargo parallax`.
fn command(args: &[String]) -> Result<(Target, Handler, &[String]), CliError> {
    let args = match args.first().map(String::as_str) {
        Some("parallax") => &args[1..],
        _ => args,
    };
    let [verb, target, flags @ ..] = args else {
        return Err(CliError::Usage("expected a command".into()));
    };
    COMMANDS
        .iter()
        .find(|(v, t, _, _)| v == verb && t == target)
        .map(|&(_, _, target, handler)| (target, handler, flags))
        .ok_or_else(|| CliError::Usage(format!("unknown command `{verb} {target}`")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_table() {
        // (row name, argv after the binary, expected (target, flags) or a usage error)
        type Case<'a> = (&'a str, &'a [&'a str], Option<(Target, &'a [&'a str])>);
        let cases: [Case; 8] = [
            (
                "as cargo subcommand",
                &["parallax", "run", "android"],
                Some((Target::Android, &[])),
            ),
            (
                "as binary",
                &["run", "android"],
                Some((Target::Android, &[])),
            ),
            (
                "flags pass through",
                &["parallax", "run", "android", "--release"],
                Some((Target::Android, &["--release"])),
            ),
            (
                "run web",
                &["run", "web", "--port", "9000"],
                Some((Target::Web, &["--port", "9000"])),
            ),
            ("build web", &["build", "web"], Some((Target::Web, &[]))),
            ("unknown target", &["run", "plan9"], None),
            ("unknown verb", &["serve", "web"], None),
            ("no command", &["parallax"], None),
        ];
        for (name, args, want) in cases {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let got = command(&args)
                .map(|(target, _, flags)| (target, flags.to_vec()))
                .map_err(|e| e.variant());
            let want = want
                .map(|(target, flags)| (target, flags.iter().map(|s| s.to_string()).collect()))
                .ok_or_else(|| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }
}
