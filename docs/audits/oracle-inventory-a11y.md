# Oracle inventory: accessibility (Phase 6)

Part of the [oracle inventory](oracle-inventory.md). Flutter tag `3.50.1`; upstream `gpui-pre` 0.3.14.

GPUI builds one AccessKit tree in core ([window/a11y.rs](../../upstream/gpui-pre/src/window/a11y.rs),
[_accessibility.rs](../../upstream/gpui-pre/src/_accessibility.rs)). Upstream adapters: Windows, macOS
(`accesskit_macos`) and Linux (`accesskit_unix`), per their `Cargo.toml`s. None on iOS, web or Android.
Shared mapping: Flutter's `SemanticsNode` flags/actions/label → an AccessKit `Node` with the same role, actions and
label; Flutter's expected platform object (`AccessibilityNodeInfo`, `UIAccessibilityElement` traits, ARIA DOM) is the
oracle for what the adapter produces from that node.

| Flutter test | Size / cases | Tests | Checks | Verdict |
|---|---|---|---|---|
| [AccessibilityBridgeTest.java](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/android/test/io/flutter/view/AccessibilityBridgeTest.java) | 164 KB / 99 | node info text/hint/tooltip, importance, locale spans, cursor granularity, collection info, focus events, platform views, high-contrast flags | `accesskit_android` in `parallax-android` | MAPPED: ~45/99. Robolectric there; `accesskit_android` is Rust over JNI, so these become emulator tests. Platform views, route announcements, channel and settings observers drop |
| [SemanticsObjectTest.mm](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/darwin/ios/framework/Source/SemanticsObjectTest.mm) | 69 KB / 60 | traits, hit test, scroll-to-visible, scrollables, locale, identifier, containers | none upstream on iOS (gap for the Phase 0 audit) | MAPPED: ~30/60 once an iOS adapter exists; bridge lifetime and child-array internals drop |
| [accessibility_bridge_test.mm](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/shell/platform/darwin/ios/framework/Source/accessibility_bridge_test.mm) | 124 KB / 44 | bridge updates, platform views, announcements, focus restoration | none upstream on iOS | HARD: ~8/44 (announcements, focus); the rest is Flutter's bridge |
| [web_ui semantics_test.dart](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/lib/web_ui/test/engine/semantics/semantics_test.dart) | 228 KB / 187 | ARIA roles and attributes per semantics role, DOM tree shape, scrolling, tappables | `parallax-web` semantics DOM (no upstream code) | MAPPED: ~90/187; expected DOM per role is the spec. Flutter's DOM-update internals drop |
| [web_ui text_field_test.dart](https://github.com/flutter/flutter/blob/3.50.1/engine/src/flutter/lib/web_ui/test/engine/semantics/text_field_test.dart) | 47 KB / 36 | semantic text field ↔ editing element focus | `parallax-web` DOM + [ime_mirror.rs](../../upstream/gpui-pre-web/src/ime_mirror.rs) | HARD: ~10/36; tied to Flutter's element-per-field editing |
| [framework semantics tests](https://github.com/flutter/flutter/tree/3.50.1/packages/flutter/test/semantics) and `widgets/semantics_*_test.dart` | 133 KB / 97 (`semantics_test.dart` ×2) | Flutter's semantics tree from widgets | none: GPUI builds its tree itself | HARD: 0 |

Not counted: the other `web_ui/test/engine/semantics/*` files (60 KB: tappable, scrollable, selectable, announcements),
which are MAPPED in the same way as `semantics_test.dart`; read them when `parallax-web` starts.

Area total: 6 rows, 523 cases; EASY 0, MAPPED 3, HARD 3 (rows); ~183 cases salvageable, none until an adapter exists.
