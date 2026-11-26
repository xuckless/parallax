# Platform table

[`parallax-platform`](../crates/platform/src/lib.rs) picks the GPUI platform for the target an app is built for.
Rules it follows: [AGENTS.md](../AGENTS.md) (Caveman Code, errors).

## The table

`TABLE` in [lib.rs](../crates/platform/src/lib.rs) has one row per target: the crate that provides its backend and
that backend's `Status`.

| Target | Backend crate | Status |
|---|---|---|
| macOS | `gpui_macos` | `Upstream` |
| Windows | `gpui_windows` | `Upstream` |
| Linux | `gpui_linux` | `Upstream` |
| Web (wasm) | `gpui_web` | `Upstream` |
| iOS | `gpui_ios` | `Upstream` |
| Android | `parallax-android` | `Parallax` |

`Upstream`: upstream `gpui_platform` builds it. `Parallax`: a Parallax crate builds it. `NotYet`: planned, not built
(no row uses it today). `CURRENT_TARGET` is the only place that reads `cfg` to pick a target. `lookup(target)` is pure: it returns the row, or
a typed `Error`, so the table is tested on any host.

## How an app gets its platform

```rust
let platform = parallax::parallax_platform::current_platform(false)?;
gpui::Application::with_platform(platform).run(|cx| { /* ... */ });
```

`current_platform(headless)` looks up the current row. For `Upstream` rows it calls upstream
[`gpui_platform::current_platform`](../upstream/gpui-pre-platform/src/gpui_platform.rs), which builds the backend
itself. Parallax does not build upstream backends a second way. For the `Parallax` row (Android) it calls
[`parallax_android::current_platform`](../crates/android/src/lib.rs) (`headless` is ignored: the Activity owns the
window). On Android an app does not call it directly: `android_main!` starts GPUI ([android.md](android.md)).

## Unsupported and unbuilt targets

`current_platform` never panics. It returns:

- `Error::NotYet { target, backend }` for a row whose backend is not built yet, or not built for this target.
- `Error::Android(..)` on Android when the backend cannot start (e.g. called before `JNI_OnLoad` or off the main thread).
- `Error::UnsupportedTarget` when the target has no row (`CURRENT_TARGET` is `None`, e.g. FreeBSD, which upstream
  supports but Parallax does not list).

The crate compiles on every target in the table. Upstream `gpui_platform` has no Android arm and fails to compile
there, so [Cargo.toml](../crates/platform/Cargo.toml) depends on it only off Android, and on `parallax-android` only on
Android. On wasm it runs
single-threaded on stable Rust: the [workspace](../Cargo.toml) turns off `gpui_platform`'s default `multithreaded`
feature, which needs nightly (AGENTS.md "Web threading"). It turns on `wayland` and `x11`, which upstream leaves
off: without them Linux finds no display server, starts headless and draws nothing, with no error.

## Adding a target or backend

1. Add a `Target` variant, a row in `TABLE`, and its arm in `CURRENT_TARGET`.
2. Add a named row to the `lookup_table` test.
3. Update the table above.

A new Parallax backend: make its crate a target-only dependency, set its row to `Status::Parallax`, and add its
`cfg_select!` arm to `current_platform`'s `Status::Parallax` branch.
