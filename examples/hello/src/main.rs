//! Desktop and iOS entry point. Android and the web start from `lib.rs`.

/// `parallax::run` applies the platform behaviour table (Flutter's gesture feel, docs/behaviour.md) before `launch`.
fn main() -> Result<(), parallax::parallax_platform::Error> {
    // stderr logger: warnings from every crate, `info` from hello, unless RUST_LOG says otherwise.
    // Android and the web install theirs in lib.rs.
    #[cfg(not(any(target_os = "android", target_family = "wasm")))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,hello=info"))
        .init();
    parallax::run(|cx| {
        hello::launch(cx);
        hello::open_window(cx);
    })
}
