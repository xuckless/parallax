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
| Android | `parallax-android` | `NotYet` |

`CURRENT_TARGET` is the only place that reads `cfg` to pick a target. `lookup(target)` is pure: it returns the row, or
a typed `Error`, so the table is tested on any host.

## How an app gets its platform

```rust
let platform = parallax::parallax_platform::current_platform(false)?;
gpui::Application::with_platform(platform).run(|cx| { /* ... */ });
```

`current_platform(headless)` looks up the current row. For `Upstream` rows it calls upstream
[`gpui_platform::current_platform`](../upstream/gpui-pre-platform/src/gpui_platform.rs), which builds the backend
itself. Parallax does not build upstream backends a second way.

## Unsupported and unbuilt targets

`current_platform` never panics. It returns:

- `Error::NotYet { target, backend }` for a row whose backend is not built yet (Android today).
- `Error::UnsupportedTarget` when the target has no row (`CURRENT_TARGET` is `None`, e.g. FreeBSD, which upstream
  supports but Parallax does not list).

The crate compiles on every target in the table. Upstream `gpui_platform` has no Android arm and fails to compile
there, so [Cargo.toml](../crates/platform/Cargo.toml) depends on it only off Android. On wasm it runs
single-threaded on stable Rust: the [workspace](../Cargo.toml) turns off `gpui_platform`'s default `multithreaded`
feature, which needs nightly (AGENTS.md "Web threading").

## Adding a target or backend

1. Add a `Target` variant, a row in `TABLE`, and its arm in `CURRENT_TARGET`.
2. Add a named row to the `lookup_table` test.
3. Update the table above.

When Android lands: add `parallax-android` as an Android-only dependency, add a `Status::Parallax` variant, set the
Android row to it, and give `current_platform` a `Status::Parallax` arm that calls `parallax-android`. The
compiler then points at every match that must handle it.
