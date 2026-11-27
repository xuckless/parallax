//! Desktop and iOS entry point. Android and the web start from `lib.rs`.

/// `parallax::run` applies the platform behaviour table (Flutter's gesture feel, docs/behaviour.md) before `launch`.
fn main() -> Result<(), parallax::parallax_platform::Error> {
    parallax::run(hello::launch)
}
