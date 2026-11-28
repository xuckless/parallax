# Desktop touch: macOS emulation and `NSIsTouchNative`

How [desktop-touch-macos.md](desktop-touch-macos.md)'s touch path runs without a touch Mac.

## Emulation and `NSIsTouchNative`

Without the key AppKit emulates: tap → `mouseDown`/`mouseUp`, pan → trackpad scroll, long press → right click,
pinch / rotate → magnify / rotate, hold and move → drag. An app with its own touch pipeline opts out:

```xml
<key>NSIsTouchNative</key><true/>
```

in `Contents/Info.plist` of its `.app`: `cargo parallax run macos` writes it from `[macos] touch_native = true` in
`parallax.toml` ([cli-desktop.md](cli-desktop.md); another bundler: its plist extension, e.g. cargo-bundle's
`osx_info_plist_exts`). Then touches arrive only as `TouchEvent`s and GPUI's touch →
mouse ([pointer-source.md](pointer-source.md)) makes the clicks. Without the key, the owner's Sidecar check (Wave
13, macOS 27.3) saw one `touch click` per tap and no double scroll in both bundles. Its two `pinch end` lines 9 ms
apart were not AppKit's magnify (a trackpad pinch has no `TouchPinch`) but Flutter's scale: a lift ends it, the
finger still down restarts it once it moves, and its lift ends it again. The zero velocities fit
Flutter's fling rule (`_isFlingGesture`): an end slower than `kMinFlingVelocity` has none, and the touch-native
end with one was a two-finger flick (zoom 1.00). `hello` logs a
`trackpad pinch end` line, so a rerun shows if magnify copies ever leak past the drop.
