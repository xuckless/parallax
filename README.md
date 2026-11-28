# Parallax

One GPUI app, every platform. Write a [GPUI](https://gpui.rs) app once and run it on Android, iOS, the web, macOS,
Windows and Linux, with each platform's touch, scroll and text-input feel ported from Flutter. Parallax adds an
Android backend, desktop touch, native views and the `cargo parallax` build tool to GPUI, shipped with our patched
GPUI fork (`parallax-gpui*`). Built on GPUI; not affiliated with Zed Industries or Google.

| Target | Runs with | Status |
|---|---|---|
| Android 9+ | `cargo parallax run android` | Checked on emulators (API 28–35): touch, IME, emoji, autofill, native views |
| iOS 17+ | `cargo parallax run ios` | Checked on the simulator; device runs owed |
| Web | `cargo parallax run web` | Checked in Chromium and Firefox (WebGPU, WebGL2) |
| macOS 12+ | `cargo parallax run macos` | Real `.app`; Sidecar touch checked by the owner |
| Windows 10+ | `cargo parallax run windows` | Touch and pen checked in CI |
| Linux | `cargo parallax run linux` | Wayland and X11, touch checked in CI |

Pre-alpha: checked on emulators, simulators and CI, not yet on phones or touch PCs. Plan and progress: [ROADMAP.md](ROADMAP.md); changes:
[CHANGELOG.md](CHANGELOG.md).

## Quickstart

```sh
git clone https://github.com/xuckless/parallax && cd parallax
cargo install --path crates/cli      # cargo-parallax (or cargo install cargo-parallax)
scripts/apply-patches.sh             # builds the patched GPUI fork this checkout uses
cd examples/hello && cargo parallax run  # this machine's desktop; add android, ios or web
```

Per-target setup, from SDKs to common errors: [docs/getting-started.md](docs/getting-started.md). The example app:
[examples/hello](examples/hello/) ([what it shows](docs/examples.md)).

## Learn more

- [Touch and mouse](docs/touch-and-mouse.md): touch acts as a mouse by default; `.on_touch` opts out.
- [Platform feel](docs/platform-feel.md): Flutter's per-platform gesture values, and `parallax::set_gesture_tuning`.
- [Native views](docs/native-views-guide.md): a browser page, video or map inside a GPUI window.
- [Credits](docs/credits.md): what Parallax ports from Flutter, Chromium, React Native, winit and others.
- [How the code works](docs/README.md). Contributors and agents: read [AGENTS.md](AGENTS.md) first.

Copyright 2026 Syed Ali (xuckless). Licensed under Apache 2.0, with ported code under its own licences. See
[LICENSE](LICENSE) and [NOTICE](NOTICE).
