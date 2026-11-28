# Credits

Parallax ports proven code instead of reinventing it ("port before write", [AGENTS.md](../AGENTS.md)). Every
ported file carries a header naming its source and commit; every port has a row in a ledger; every licence is in
[NOTICE](../NOTICE). This page is the map.

## GPUI (Zed Industries), the base

Parallax is built on [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (Apache-2.0), taken from the
`gpui-pre` snapshot crates ([upstream.md](upstream.md)) and shipped with Parallax's patches as the `parallax-gpui*`
crates ([gpui-fork-crates.md](gpui-fork-crates.md)). Not affiliated with or endorsed by Zed Industries.

## Ported code

| Source (licence) | What Parallax took | Ledger |
|---|---|---|
| [Flutter](https://github.com/flutter/flutter) (BSD-3-Clause), the second source of truth | The Android embedding's touch, keyboard, text input, autofill and clipboard (Java, near-verbatim); velocity trackers, scroll simulations and physics, gesture constants, scale (pinch) recognizer, overscroll; the per-platform behaviour table; haptics; iOS keyboard keys and inset; web text input and lifecycle mapping; text selection; touch time stamps; `flutter_tools`' iOS, macOS, web and Android app templates and build logic for `cargo parallax`; test cases as the spec for all of it | [third_party/flutter/PORTED.md](../third_party/flutter/PORTED.md), how: [flutter.md](flutter.md) |
| [Flutter's website](https://github.com/flutter/website) (code samples, BSD-3-Clause) | The Android release-signing sample (`signingConfigs`, `keytool -genkey`) | [third_party/flutter-website/PORTED.md](../third_party/flutter-website/PORTED.md) |
| [Chromium](https://chromium.googlesource.com/chromium/src) (BSD-3-Clause) | Wayland subsurfaces for native views; Windows DirectComposition overlay tree and underlay hole | [third_party/chromium/PORTED.md](../third_party/chromium/PORTED.md) |
| [React Native](https://github.com/facebook/react-native) (MIT) | The iOS keyboard's spring curve (via Flutter); reduce-motion and reduce-transparency reading on iOS and Android; Android native views' touch dispatch | [third_party/react-native/PORTED.md](../third_party/react-native/PORTED.md) |
| [winit](https://github.com/rust-windowing/winit) v0.30.13 (Apache-2.0) | Windows touch and pen (`WM_POINTER`), Wayland `wl_touch`, X11 XInput 2 touch | patches [0021](../patches/0021-touch-windows.patch), [0022](../patches/0022-touch-wayland.patch), [0023](../patches/0023-touch-x11.patch) and [NOTICE](../NOTICE) |
| [page-lifecycle](https://github.com/GoogleChromeLabs/page-lifecycle) 0.1.2 (Apache-2.0) | The web page lifecycle state machine | patch [0035](../patches/0035-web-lifecycle-and-keyboard-inset.patch), [web-lifecycle.md](web-lifecycle.md) and [NOTICE](../NOTICE) |
| [GPUI Kit](https://github.com/longbridge/gpui-kit) (Apache-2.0) | The staging step that builds `upstream/` from Zed's tree (`scripts/vendor-zed.py`, tooling only) | [third_party/gpui-kit/PORTED.md](../third_party/gpui-kit/PORTED.md) |

## Read as references

Read, not copied: [gpui-component](https://github.com/longbridge/gpui-component) (Apache-2.0) for touch selection
([text-selection-tests.md](text-selection-tests.md)) and its `gpui-webview` crate for native views
([ROADMAP.md](../ROADMAP.md)); its widgets are planned as ports (ROADMAP Wave 19). Flutter's `AccessibilityBridge.java`
and `SemanticsObject.mm` are the oracle for accessibility (v0.2.x). Copyleft code (Zed's editor and text crates,
GPL) is never copied.

## Bundled files

| File | From | Licence |
|---|---|---|
| `gradle-wrapper.jar` in cargo-parallax | [Gradle](https://github.com/gradle/gradle) 9.8.2 | Apache-2.0 |
| [examples/hello/fonts](../examples/hello/fonts/) | Noto Sans, Noto Sans Arabic (Google) | SIL OFL 1.1 |
| WebView2 loader in the native-view example's Windows build | Microsoft's WebView2 SDK, via [webview2-com](https://github.com/wravery/webview2-rs) (MIT) | [its README](../examples/native-view/README.md#credits) |
| [assets/brand](../assets/brand/) | the Parallax mark, the owner's own | not Apache-2.0: [README](../assets/brand/README.md) |

## Licence files

Crates with ported code are `Apache-2.0 AND BSD-3-Clause` (or name the MIT term) and ship the source's licence
beside their own: `LICENSE-FLUTTER`, `LICENSE-CHROMIUM`, `LICENSE-REACT-NATIVE`, `LICENSE-FLUTTER-WEBSITE`. Which crate
carries which is in [NOTICE](../NOTICE). "Flutter" and Google marks never appear in a crate or product name; Parallax
is not affiliated with or endorsed by Google or the Flutter project.
