//! Desktop and iOS entry point. Android and the web start from `lib.rs`.

#[cfg(not(any(target_os = "android", target_family = "wasm")))]
fn main() -> Result<(), parallax::parallax_platform::Error> {
    native_view::main()
}

#[cfg(any(target_os = "android", target_family = "wasm"))]
fn main() {}
