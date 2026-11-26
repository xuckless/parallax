//! The shell around [`crate::fonts`]: reads the system font list, memory-maps the planned files into upstream's
//! `CosmicTextSystem`, and wraps it to add Android's generic names and per-language fallbacks. Main thread, once,
//! when the platform starts; the time is logged. How it fits together: `docs/android-fonts.md`.
//!
//! Local workaround for patch 0002 (AGENTS.md, patch rule 11): stock `CosmicTextSystem` takes fonts only as bytes
//! and keeps cosmic-text's empty Android fallback table, so files are mapped and leaked, and the per-language order
//! is applied as GPUI fallbacks on every font. Once `new_with_font_system` is upstream, this builds a `FontSystem`
//! (fonts by path, the app's locale, an `impl Fallback` from the same plan) and the wrapper shrinks to the names.

use std::{
    borrow::Cow, collections::HashMap, fs::File, path::Path, path::PathBuf, sync::Arc,
    time::Instant,
};

use gpui::{
    Bounds, DevicePixels, Font, FontFallbacks, FontId, FontMetrics, FontRun, FontWeight, GlyphId,
    Hsla, LineLayout, MissingGlyphSink, Pixels, PlatformTextSystem, RenderGlyphParams,
    SharedString, Size, TextRenderingMode,
};
use gpui_wgpu::CosmicTextSystem;

use crate::{
    Error,
    fonts::{self, Face, FontPlan},
};

/// Where Android lists its fonts, newest format first; the first readable one is used. API 35+ ship both (the old
/// one kept for apps); `font_fallback.xml` may be unreadable to apps, then `fonts.xml` serves.
const FONT_LISTS: &[&str] = &["/system/etc/font_fallback.xml", "/system/etc/fonts.xml"];

/// The files both lists name.
const FONT_DIR: &str = "/system/fonts";

/// GPUI's text system with Android's system fonts. Every method but `font_id` is upstream's.
pub(crate) struct AndroidTextSystem {
    cosmic: CosmicTextSystem,
    /// fonts.xml family and alias names (`sans-serif`, `monospace`, `arial`) → the family name inside the file,
    /// and the weight an alias implies (`sans-serif-medium`).
    generics: HashMap<String, (SharedString, Option<FontWeight>)>,
    /// The per-language faces' family names, the app's locales first: appended to every font's fallbacks.
    by_locale: Vec<String>,
}

/// Loads the system fonts for `locales` (BCP-47, the app's preference order). On any failure the text system has
/// no system fonts, as before Phase 1, and the error is logged: text still renders with fonts the app adds.
pub(crate) fn load(locales: &[String]) -> Arc<dyn PlatformTextSystem> {
    let started = Instant::now();
    match load_plan(locales) {
        Ok((list, plan)) => match AndroidTextSystem::new(&plan) {
            Ok(text_system) => {
                log::info!(
                    "parallax: fonts: {} files from {list} ({} listed but missing) in {:?}; default {}, \
                     per-language order {:?} for locales {locales:?}",
                    plan.files.len(),
                    plan.missing.len(),
                    started.elapsed(),
                    plan.default.path.display(),
                    text_system.by_locale,
                );
                return Arc::new(text_system);
            }
            Err(error) => log::error!("parallax: fonts: cannot load {list}: {error}"),
        },
        Err(error) => log::error!("parallax: fonts: {error}"),
    }
    Arc::new(CosmicTextSystem::new_without_system_fonts("sans-serif"))
}

fn load_plan(locales: &[String]) -> Result<(&'static str, FontPlan), Error> {
    let (list, xml) = FONT_LISTS
        .iter()
        .find_map(|list| Some((*list, std::fs::read_to_string(list).ok()?)))
        .ok_or(Error::NoFontList(FONT_LISTS))?;
    let config = fonts::parse(&xml)?;
    let plan = fonts::plan(&config, Path::new(FONT_DIR), locales, Path::exists)?;
    Ok((list, plan))
}

/// Maps a font file for the life of the process: fonts are never unloaded, and stock `add_fonts` needs `'static`
/// bytes. The pages stay in the shared page cache, not in the app's heap.
fn map(path: &Path) -> Result<&'static [u8], Error> {
    let file = File::open(path).map_err(|error| Error::Os("open font", error))?;
    // SAFETY: system fonts live on the read-only system partition; nothing truncates or rewrites them while mapped.
    let mapped =
        unsafe { memmap2::Mmap::map(&file) }.map_err(|error| Error::Os("mmap font", error))?;
    Ok(Box::leak(Box::new(mapped)))
}

impl AndroidTextSystem {
    fn new(plan: &FontPlan) -> Result<Self, Error> {
        let mapped: HashMap<&PathBuf, &'static [u8]> = plan
            .files
            .iter()
            .map(|path| Ok((path, map(path)?)))
            .collect::<Result<_, Error>>()?;

        // Family names live in the files, not the XML: read them for the few faces the plan names.
        let mut names = fontdb::Database::new();
        let mut ids: HashMap<&PathBuf, Vec<fontdb::ID>> = HashMap::new();
        let named = std::iter::once(&plan.default)
            .chain(plan.generics.iter().map(|generic| &generic.face))
            .chain(&plan.by_locale);
        for face in named {
            if let (false, Some(&bytes)) = (ids.contains_key(&face.path), mapped.get(&face.path)) {
                let loaded = names.load_font_source(fontdb::Source::Binary(Arc::new(bytes)));
                ids.insert(&face.path, loaded.to_vec());
            }
        }
        let family = |face: &Face| -> Option<String> {
            ids.get(&face.path)?
                .iter()
                .filter_map(|id| names.face(*id))
                .find(|info| info.index == face.index)?
                .families
                .first()
                .map(|(name, _)| name.clone())
        };

        let default = family(&plan.default).ok_or(Error::NoSystemFont)?;
        let cosmic = CosmicTextSystem::new_without_system_fonts(&default);
        cosmic
            .add_fonts(
                plan.files
                    .iter()
                    .filter_map(|path| mapped.get(path))
                    .map(|bytes| Cow::Borrowed(*bytes))
                    .collect(),
            )
            .map_err(|error| Error::AddFonts(format!("{error:#}")))?;
        Ok(Self {
            cosmic,
            generics: plan
                .generics
                .iter()
                .filter_map(|generic| {
                    Some((
                        generic.name.clone(),
                        (
                            family(&generic.face)?.into(),
                            generic.weight.map(|weight| FontWeight(weight.into())),
                        ),
                    ))
                })
                .collect(),
            by_locale: plan.by_locale.iter().filter_map(family).collect(),
        })
    }
}

impl PlatformTextSystem for AndroidTextSystem {
    /// Resolves Android's generic names, then appends the per-language fallbacks to the font's own.
    fn font_id(&self, descriptor: &Font) -> anyhow::Result<FontId> {
        let mut font = descriptor.clone();
        if let Some((family, weight)) = self.generics.get(font.family.as_ref()) {
            font.family = family.clone();
            font.weight = weight.unwrap_or(font.weight);
        }
        let own = font
            .fallbacks
            .as_ref()
            .map(FontFallbacks::fallback_list)
            .unwrap_or_default();
        font.fallbacks = Some(FontFallbacks::from_fonts(
            own.iter().chain(&self.by_locale).cloned().collect(),
        ));
        self.cosmic.font_id(&font)
    }

    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> anyhow::Result<()> {
        self.cosmic.add_fonts(fonts)
    }

    fn set_missing_glyph_sink(&self, sink: Option<Arc<dyn MissingGlyphSink>>) {
        self.cosmic.set_missing_glyph_sink(sink)
    }

    fn all_font_names(&self) -> Vec<String> {
        self.cosmic.all_font_names()
    }

    fn prewarm_fonts(&self, font_ids: &[FontId]) {
        self.cosmic.prewarm_fonts(font_ids)
    }

    fn font_metrics(&self, font_id: FontId) -> FontMetrics {
        self.cosmic.font_metrics(font_id)
    }

    fn typographic_bounds(
        &self,
        font_id: FontId,
        glyph_id: GlyphId,
    ) -> anyhow::Result<Bounds<f32>> {
        self.cosmic.typographic_bounds(font_id, glyph_id)
    }

    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> anyhow::Result<Size<f32>> {
        self.cosmic.advance(font_id, glyph_id)
    }

    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId> {
        self.cosmic.glyph_for_char(font_id, ch)
    }

    fn glyph_raster_bounds(
        &self,
        params: &RenderGlyphParams,
    ) -> anyhow::Result<Bounds<DevicePixels>> {
        self.cosmic.glyph_raster_bounds(params)
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> anyhow::Result<(Size<DevicePixels>, Vec<u8>)> {
        self.cosmic.rasterize_glyph(params, raster_bounds)
    }

    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        self.cosmic.layout_line(text, font_size, runs)
    }

    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.cosmic.recommended_rendering_mode(font_id, font_size)
    }

    fn glyph_dilation_for_color(&self, color: Hsla) -> u8 {
        self.cosmic.glyph_dilation_for_color(color)
    }
}
