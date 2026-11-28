# R8 keep rules for the Java in java/, applied by cargo-parallax to every release build (docs/cli-android-release.md).
# Rust finds these classes, and calls their methods and natives, by name over JNI: ParallaxSurfaceView's natives are
# registered by name (src/host.rs), EmojiRasterizer is found and called by name (src/text_system.rs), the platform
# plugin and native-view host methods are called by name and signature (src/platform.rs, src/native_view.rs), and
# signatures name inner classes (PlatformChannel$ClipboardContentFormat). R8 cannot see any of that, so it would
# rename or drop them. Keep the whole package: it is small, and a new JNI call can never miss a rule.
-keep class io.github.xuckless.parallax.** { *; }
