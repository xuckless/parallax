# Android emoji

swash, which upstream's `CosmicTextSystem` draws glyphs with, paints COLRv0 and CBDT but not COLRv1, the only emoji
font on newer images (API 33+ without `NotoColorEmojiLegacy.ttf`). So Android draws emoji itself
([AGENTS.md](../AGENTS.md), Decisions: Android emoji). Fonts in general: [android-fonts.md](android-fonts.md).

## How it works

1. **Which faces.** [fonts.rs](../crates/android/src/fonts.rs) `plan` lists as `emoji` every face of a non-ignored
   fonts.xml family whose `lang` names the script `Zsye` (`EMOJI_SCRIPT`): `NotoColorEmoji.ttf` and, on newer images,
   `NotoColorEmojiFlags.ttf`. By font, not by name: upstream's `check_is_known_emoji_font` matches the PostScript
   name `NotoColorEmoji` only and misses the flags font.
2. **Typefaces.** [text_system.rs](../crates/android/src/text_system.rs) `Emoji::new` makes an Android `Typeface` of
   each face from the same file cosmic-text loaded (`EmojiRasterizer.typeface`, over JNI, once at startup).
3. **Layout.** `layout_line` is upstream's, then [emoji.rs](../crates/android/src/emoji.rs) `mark` rewrites each run
   whose font is an emoji face ([patch 0007](../patches/0007-cosmic-text-system-font-database-id.patch) maps a
   `FontId` to its face): each cluster (text from a glyph's index to the next glyph's) becomes one glyph, marked
   emoji, whose id names the cluster's text in `Clusters` (ids from `0x1_0000`, above any font's).
4. **Raster.** `glyph_raster_bounds` and `rasterize_glyph` ask `emoji::route`: a colour request for an emoji face's
   cluster id goes to [`EmojiRasterizer`](../crates/android/java/io/github/xuckless/parallax/EmojiRasterizer.java)
   (`Paint.getTextBounds`, then `Canvas.drawText` into a `Bitmap`, unpremultiplied ARGB → BGRA); everything else
   stays on upstream's swash path. GPUI caches bounds and pixels (its atlas), so each cluster and size is drawn once.
   Main thread only, where GPUI runs on Android: a call from another thread has no `JNIEnv` and returns an error.

Why one glyph per cluster: Android draws text, not glyph ids (`Canvas.drawGlyphs` is API 31+, the minimum is 28).
A ZWJ sequence newer than the font shapes to several glyphs on one index (API 28: 🧑🏾‍🦱 is person + hair), and its
first glyph also stands alone, so a glyph id cannot name the text. Drawing the cluster once from its first glyph
shows what Android itself shows.

## Metrics: cosmic-text's

Layout keeps cosmic-text's advances: one text system, and Android draws from the same file, so the shapes agree.
Measured on both emulators at 73.5 px: cosmic-text 91.52 px, `Paint.measureText` 91.0 px (Android rounds advances
to whole pixels). Half a pixel inside the em box changes nothing visible; the bitmap is placed at cosmic-text's
glyph origin with Android's own bounds.

## Cost

Per new cluster and size, on the main thread; once drawn a cluster costs nothing (GPUI's atlas). An emoji picker's
first screen is the worst case: hello's "Show emoji" ([examples.md](examples.md)) shows 100 distinct emoji at 28 px
and logs `emoji grid: frame …`, from the start of the frame that draws them to the start of the next (release build,
emulator at 60 Hz, 6 cold launches each):

| Image | Font | 100 new emoji | Same grid again (cached) |
|---|---|---|---|
| API 35 | COLRv1 | 25–44 ms, median 32 ms (1–2 frames late) | 16.6–18.3 ms (one frame) |
| API 28 | CBDT | 54–154 ms, median 92 ms (3–9 frames late) | 16.6–17.5 ms (one frame) |

Decision (AGENTS.md: measure, move only if over budget): API 35 is within budget, so nothing changes. API 28 drops a
few frames once, when a screen first shows many new emoji. No clean fix exists on the main thread alone: GPUI asks
for each glyph inside its paint and keeps what it gets, so a late (async) fill would leave blanks. Proposal, if an
Android 9–12 device shows it: when `layout_line` meets new clusters (before paint), draw them on worker threads
attached to the JVM (`Canvas` into a `Bitmap` works off the UI thread) and let `rasterize_glyph` take the pixels
from that cache. GPUI itself stays on the main thread.

## Verified

`examples/hello` with emoji lines (single, ZWJ, skin tones, flags, keycaps, Emoji 15, mixed with Latin) on the API 35
and API 28 emulators: all colour; API 28 shows tofu for emoji newer than its font, as Android 9 does.

## Known gaps

- A glyph no layout produced (no cluster id) falls back to swash: blank on COLRv1.
- App-added colour fonts are not routed (only fonts.xml emoji faces); swash draws them if it can.
- The cluster table grows with the distinct emoji shown and is never trimmed.
