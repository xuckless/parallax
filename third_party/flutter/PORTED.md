# Code ported from Flutter

Flutter is BSD-3-Clause (see `LICENSE` here). Every file that ports or adapts Flutter code gets one row.
Pinned Flutter stable tag: `3.50.1` (stable branch `ae9b586f34`, 2026-11-19). Track stable tags only, never master.

Sync rule: when a new stable tag ships, diff each source path between the old and new tag and port the fixes.

| Flutter path | Flutter tag | Parallax path | Kind (port / oracle) | Dropped parts |
|---|---|---|---|---|
| `engine/src/flutter/shell/platform/android/io/flutter/embedding/android/AndroidTouchProcessor.java` | `3.50.1` | `crates/android/java/io/github/xuckless/parallax/AndroidTouchProcessor.java` | port | `FlutterRenderer` (the packet goes to `ParallaxSurfaceView.nativeDispatchPointerDataPacket`); androidx annotations (`@IntDef`, `@NonNull`, `@VisibleForTesting`, `@RequiresApi`); `io.flutter.Build.API_LEVELS` (→ `Build.VERSION_CODES.O`). `onGenericMotionEvent` kept but not wired |
| `engine/src/flutter/shell/platform/android/io/flutter/embedding/android/MotionEventTracker.java` | `3.50.1` | `crates/android/java/io/github/xuckless/parallax/MotionEventTracker.java` | port | androidx annotations (`@NonNull`, `@Nullable`). Unused while `trackMotionEvents` is false (no platform views) |
| `engine/src/flutter/lib/ui/window/pointer_data.h` | `3.50.1` | `crates/android/src/pointer.rs` | port | everything but the `PointerData` field order and the `Change` / `DeviceKind` values (Rust decoder); button enums, `SignalKind` values, `Clear()` |
