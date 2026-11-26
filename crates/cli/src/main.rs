//! cargo parallax: build and run GPUI apps on iOS, Android and web from parallax.toml. Reference: docs/cli.md.

mod android;
mod config;
mod error;
mod icon;

use android::{Profile, RunArgs};
use config::Config;
use error::CliError;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run_args = parse_args(&args)?;
    let app_dir = std::env::current_dir()?;
    let path = app_dir.join("parallax.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|source| CliError::ConfigMissing { path, source })?;
    let config = Config::parse(&text)?;
    android::run(&app_dir, &config, &run_args)?;
    Ok(())
}

/// `[parallax] run android [--release] [--device <serial>]`; cargo passes the leading `parallax` when run as
/// `cargo parallax`.
fn parse_args(args: &[String]) -> Result<RunArgs, CliError> {
    let args = match args.first().map(String::as_str) {
        Some("parallax") => &args[1..],
        _ => args,
    };
    let mut rest = match args {
        [run, target, rest @ ..] if run == "run" && target == "android" => rest.iter(),
        _ => return Err(CliError::Usage("expected `run android`".into())),
    };
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

    #[test]
    fn parse_args_table() {
        // (row name, argv after the binary, expected (profile dir, --device) or a usage error)
        type Case<'a> = (
            &'a str,
            &'a [&'a str],
            Result<(&'a str, Option<&'a str>), ()>,
        );
        let cases: [Case; 8] = [
            (
                "as cargo subcommand",
                &["parallax", "run", "android"],
                Ok(("debug", None)),
            ),
            ("as binary", &["run", "android"], Ok(("debug", None))),
            (
                "release",
                &["parallax", "run", "android", "--release"],
                Ok(("release", None)),
            ),
            (
                "device",
                &["run", "android", "--device", "emulator-5556"],
                Ok(("debug", Some("emulator-5556"))),
            ),
            (
                "device without serial",
                &["run", "android", "--device"],
                Err(()),
            ),
            ("unknown flag", &["run", "android", "--fast"], Err(())),
            ("unknown target", &["run", "ios"], Err(())),
            ("no command", &["parallax"], Err(())),
        ];
        for (name, args, want) in cases {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let got = parse_args(&args)
                .map(|a| (a.profile.dir(), a.device))
                .map_err(|e| e.variant());
            let want = want
                .map(|(profile, device)| (profile, device.map(String::from)))
                .map_err(|()| "Usage".to_string());
            assert_eq!(got, want, "{name}");
        }
    }
}
