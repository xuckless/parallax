# Android fonts

fontdb finds no fonts on Android and cosmic-text has an empty Android fallback table
([audit](audits/android-fonts.md)), so [`parallax-android`](../crates/android/src/lib.rs) reads the system font list
itself. Rules: [AGENTS.md](../AGENTS.md) (Decisions: Android fonts; patch rule 11). Backend overview: [android.md](android.md).

## How fonts are found

1. **Locale.** [`ParallaxSurfaceView`](../crates/android/java/io/github/xuckless/parallax/ParallaxSurfaceView.java)
   sends `LocaleList.toLanguageTags()` with each surface (`nativeSurfaceCreated`, [host.rs](../crates/android/src/host.rs)):
   per-app locales on API 33+ included, which the `persist.sys.locale` property cosmic-text reads lacks. The first
   surface passes them straight into the platform start; later changes are not followed yet.
2. **List.** [text_system.rs](../crates/android/src/text_system.rs) reads the first readable of `FONT_LISTS`:
   `/system/etc/font_fallback.xml` (API 35+), then `/system/etc/fonts.xml`. On the API 35 emulator image the first is
   not readable by apps (SELinux), so `fonts.xml`, which API 35 still ships, is used.
3. **Parse.** [fonts.rs](../crates/android/src/fonts.rs) `parse`: both formats are `<familyset>` of `<family>`
   (`name`, `lang`, `ignore`) with `<font>` children (file name as text, `index`, `fallbackFor`) and `<alias>`.
   `lang` lists are comma-separated on API 35, space-separated on API 28. Unknown elements are skipped; malformed
   XML or a bad number is a typed `Error`.
4. **Plan.** `plan` (pure, tested on the host) turns that into files to load in XML order (once each, missing ones
   listed), the default face (the first family), generic names and the per-language order. A file that is on disk
   but cannot be opened counts as missing: it costs only itself, and the family's next font stands in.
5. **Load.** Files are memory-mapped (pages stay in the shared page cache; one that fails to map is skipped and
   logged) and handed to upstream's
   `CosmicTextSystem`. The log line `parallax: fonts: … in …` gives the count and the time, next to the renderer's.

Measured (release build, emulator): about 300 ms on the first launch after boot (API 35, cold page cache), 3–10 ms
after that (API 28: 231 files, API 35: 204 files; Android's zygote keeps the same files hot). Within budget: fonts
stay on the main thread.

## The fallback table

| Need | Where | Rule |
|---|---|---|
| `.SystemUIFont` | `plan.default` | First family's first font (`sans-serif`, Roboto) |
| `sans-serif`, `monospace`, `serif`, `arial`, `sans-serif-medium`… | `plan.generics` | Named families and `<alias>`es → the family name inside the file, plus the alias's weight |
| Han, kana, Hangul | `plan.by_locale` | `lang`-tagged families (not `und-`), the app's locales first (`LIKELY_SCRIPTS`: `zh-TW`/`HK`/`MO` → Hant, other `zh` → Hans), then XML order |
| Emoji | `SUBSTITUTES` | COLRv1 `NotoColorEmoji.ttf` → CBDT `NotoColorEmojiLegacy.ttf` when that file exists (swash draws CBDT, not COLRv1) |
| Everything else (Arabic, Devanagari…) | cosmic-text | Its last-resort scan tries faces in load order (XML order), emoji faces first |

To extend: a new locale → script rule is a row in `LIKELY_SCRIPTS`; a file swash can't draw is a row in
`SUBSTITUTES`; a new list location is a row in `FONT_LISTS`. Each gets a row in the tests in `fonts.rs`.

## Stock upstream workaround (patch 0002)

[Patch 0002](../patches/0002-cosmic-text-system-with-font-system.patch) adds `CosmicTextSystem::new_with_font_system`.
The published crate can't use it (patch rule 11, [patches.md](patches.md#published-crates)), so on stock `gpui-pre`:

- Fonts go in through `add_fonts` as `'static` bytes: each file is mapped and the mapping leaked for the process.
- The per-language order is applied as GPUI font fallbacks appended to every font (`AndroidTextSystem::font_id`),
  because stock cosmic-text keeps its empty Android `Fallback` and en-US locale. GPUI's fallback chain skips faces
  without a Latin `m`, which is why only the CJK faces go there.

Once the patch is upstream: build a `FontSystem` with `fontdb::Source::File` paths, the app's locale and an
`impl Fallback` from the same plan; the wrapper keeps only the generic names.

## Known gaps

- **Emoji on API 33+ images without `NotoColorEmojiLegacy.ttf`** (the API 35 `google_apis` emulator) are blank:
  only COLRv1 is left. The CBDT `NotoColorEmojiFlags.ttf` is not drawn either: upstream flags emoji fonts by the
  PostScript name `NotoColorEmoji` only (`check_is_known_emoji_font` in
  [cosmic_text_system.rs](../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs)).
- Han glyphs follow the app locale, not a per-run language (GPUI text has none).
- Not read: OEM `/product/etc/fonts_customization.xml`, updatable fonts in `/data/fonts`, locale changes after start.
