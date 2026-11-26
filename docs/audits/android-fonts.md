# Android font audit

Phase 0 audit ([ROADMAP.md](../../ROADMAP.md)): what cosmic-text needs on Android at `gpui-pre` 0.3.14
(cosmic-text 0.19.0, fontdb 0.23.0, swash 0.2.10 per [Cargo.lock](../../Cargo.lock)). Registry sources link to docs.rs.

## 1. Where the text system lives

- `CosmicTextSystem` is in **gpui-pre-wgpu** ([cosmic_text_system.rs](../../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs)),
  exported with no `cfg` gate ([gpui_wgpu.rs:1-6](../../upstream/gpui-pre-wgpu/src/gpui_wgpu.rs)); its cosmic-text and swash
  deps are not target-gated ([Cargo.toml](../../upstream/gpui-pre-wgpu/Cargo.toml)). Android uses gpui_wgpu anyway: reuse as-is.
- Linux and web only construct it: [platform.rs:130](../../upstream/gpui-pre-linux/src/linux/platform.rs) (`new("IBM Plex Sans")`,
  Linux-gated dep at [Cargo.toml:139](../../upstream/gpui-pre-linux/Cargo.toml)); web uses `new_without_system_fonts`
  ([text_system.rs:62](../../upstream/gpui-pre-web/src/text_system.rs)).
- **Missing:** a constructor taking a caller-built `FontSystem`. Today there is `new()` (system fonts + `PlatformFallback`) or an
  empty db plus `add_fonts` as bytes ([L131](../../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs#L131),
  [L329](../../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs#L329)), which would put all of `/system/fonts` in RAM.
  Fix: one upstreamable patch, `new_with_font_system(FontSystem, &str)`; non-font-kit matching ([L994](../../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs#L994)) suffices.

## 2. Discovery: fontdb finds nothing on Android

- [`load_system_fonts`](https://docs.rs/crate/fontdb/0.23.0/source/src/lib.rs#400) has no Android branch
  ([L461](https://docs.rs/crate/fontdb/0.23.0/source/src/lib.rs#461) excludes it): empty db, no warning, via `FontSystem::new`
  ([system.rs:438](https://docs.rs/crate/cosmic-text/0.19.0/source/src/font/system.rs#438)).
- **Readable:** `/system/fonts` and `/system/etc/fonts.xml` are world-readable; Skia and Firefox
  ([bug 1760567](https://bugzilla.mozilla.org/show_bug.cgi?id=1760567)) read them in app processes. API 31+ updated fonts sit in
  world-readable `/data/fonts` ([AOSP: custom fonts](https://source.android.com/docs/core/fonts/custom-font-fallback)).
- **Parse the XML:** a dir scan loses aliases (`sans-serif`→Roboto), fallback order (unnamed families, file order), `lang=`
  (Han unification), `variant` and `ignore`. Formats: `fonts.xml` (API 28–34), `font_fallback.xml` (API 35+, adds
  `supportedAxes`), OEM `/product/etc/fonts_customization.xml` (API 29+).
- `sys-locale` reads `persist.sys.locale` ([android.rs](https://docs.rs/crate/sys-locale/0.3.2/source/src/android.rs)), not the per-app locale (API 33+): pass it over JNI.

## 3. Fallback and emoji

- GPUI: user `FontFallbacks` chains ([L339](../../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs#L339)), then cosmic-text's
  script, common, then a scan of **every face** ([fallback/mod.rs:470](https://docs.rs/crate/cosmic-text/0.19.0/source/src/font/fallback/mod.rs#470)).
- Android gets the empty `other.rs` table ([mod.rs:13](https://docs.rs/crate/cosmic-text/0.19.0/source/src/font/fallback/mod.rs#13),
  [other.rs:30](https://docs.rs/crate/cosmic-text/0.19.0/source/src/font/fallback/other.rs#30)): arbitrary picks (wrong CJK
  variant), linear scan per miss. Fix: our own `impl Fallback` from fonts.xml via the public
  [`new_with_locale_and_db_and_fallback`](https://docs.rs/crate/cosmic-text/0.19.0/source/src/font/system.rs#213), shaped like
  [unix.rs](https://docs.rs/crate/cosmic-text/0.19.0/source/src/font/fallback/unix.rs#56). Its `&'static str` names mean
  leaking or interning parsed names once.
- **Emoji:** swash renders COLR **v0** only ([color.rs:24](https://docs.rs/crate/swash/0.2.10/source/src/scale/color.rs#24))
  and CBDT/sbix ([strike.rs:33](https://docs.rs/crate/swash/0.2.10/source/src/strike.rs#33)); GPUI tries ColorOutline,
  ColorBitmap(BestFit), Outline ([L532](../../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs#L532)). API 28–32
  `NotoColorEmoji.ttf` is CBDT: works. API 33+ it is **COLRv1**
  ([Android 13 features](https://developer.android.com/about/versions/13/features)): blank. Load the CBDT
  `NotoColorEmojiLegacy.ttf` (`ignore="true"` in fonts.xml, Mozilla bug above) instead.
- Emoji are flagged by PostScript name `"NotoColorEmoji"` ([L1204](../../upstream/gpui-pre-wgpu/src/cosmic_text_system.rs#L1204));
  if Legacy's name differs, `Bitmap(ExactSize)` misses CBDT's one strike. Verify on device; else patch detection by color tables.

## 4. Alternatives

- NDK `ASystemFontIterator`/`AFontMatcher` ([docs](https://developer.android.com/ndk/reference/group/font)) and Java `SystemFonts`: API 29+ (min 28), no fallback order.
- Skia (Flutter) parses fonts.xml itself ([SkFontMgr_android_parser.cpp](https://github.com/google/skia/blob/main/src/ports/SkFontMgr_android_parser.cpp)): parser oracle; port rules, not C++.

## 5. Size: ~650 LOC incl. tests, 1–2 small patches, 4–6 days

| Piece | Size |
|---|---|
| Patch: `CosmicTextSystem::new_with_font_system` (filed upstream) | ~25 LOC |
| fonts.xml / font_fallback.xml parser, pure text → families (`roxmltree`, in lock) | ~200 LOC + tests on AOSP fixtures (9, 10, 13, 14, 15) |
| `impl Fallback` from families (common order, BCP47 `lang` → script/locale table) | ~120 LOC + table tests |
| Shell: load listed files by path into `fontdb::Database`, generic families, CBDT emoji | ~80 LOC |
| Locale from the Activity over JNI | ~30 LOC |
| Maybe patch: emoji detection by color tables | ~15 LOC |
| Smoke: Latin, ja vs zh, Arabic, Devanagari, emoji on API 28, 33, 35 | example screen |

**Risks:** (1) blank emoji if the CBDT Legacy file goes, until swash paints COLRv1. (2) ~200 font headers parsed on the UI
thread at startup: measure, then lead decides caching/deferral. (3) OEM XML quirks. (4) `/data/fonts` updates not in the XML.

## Proposed Phase 1 tasks

- [ ] Patch: `CosmicTextSystem::new_with_font_system(FontSystem, &str)` in gpui-pre-wgpu; file upstream.
- [ ] `fonts.xml` + `font_fallback.xml` parser in parallax-android (pure; table tests on AOSP fixtures, API 28–35).
- [ ] Android `Fallback` table from parsed families (common order, `lang` → Han/locale); table tests.
- [ ] Build `fontdb::Database` from listed files (by path), generic families from aliases, CBDT emoji preferred on API 33+.
- [ ] Pass app locale from the Activity over JNI into `FontSystem`.
- [ ] Verify emoji detection for `NotoColorEmojiLegacy`; patch `check_is_known_emoji_font` if needed.
- [ ] Measure font-db startup time on a low-end device; record in ROADMAP "Upstream notes".
- [ ] Example screen: Latin, CJK (ja/zh), RTL, Indic, emoji; smoke on API 28, 33, 35 emulators.
