//! Desktop entry point (macOS, Windows, Linux). Android and the web start from `lib.rs`.

use parallax::gpui::Application;
use parallax::parallax_platform::{Error, current_platform};

fn main() -> Result<(), Error> {
    // stderr logger: warnings from every crate, `info` from hello, unless RUST_LOG says otherwise.
    // Android and the web install theirs in lib.rs.
    #[cfg(not(any(target_os = "android", target_family = "wasm")))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,hello=info"))
        .init();
    Application::with_platform(current_platform(false)?).run(hello::launch);
    Ok(())
}
