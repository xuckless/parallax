//! The shell around [`crate::fonts`]: reads the system font list, memory-maps the planned files into a font
//! database for upstream's `CosmicTextSystem` (`new_with_font_system`, patch 0002), and wraps it to add Android's
//! generic names and per-language fallbacks. Main thread, once, when the platform starts; the time is logged.
//! Colour emoji glyphs are drawn by Android itself ([`crate::emoji`] decides, `EmojiRasterizer.java` draws).
//! How it fits together: `docs/android-fonts.md`, `docs/android-emoji.md`.

use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    path::Path,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::Instant,
};

use cosmic_text::{FontSystem, fontdb};

use gpui::{
    Bounds, DevicePixels, Font, FontFallbacks, FontId, FontMetrics, FontRun, FontWeight, GlyphId,
    Hsla, LineLayout, MissingGlyphSink, Pixels, PlatformTextSystem, RenderGlyphParams,
    SharedString, Size, TextRenderingMode, point, size,
};
use gpui_wgpu::CosmicTextSystem;
use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JClass, JIntArray, JValue},
};

use crate::{
    Error,
    emoji::{self, Clusters, Raster},
    fonts::{self, Face, FontPlan},
};

/// Where Android lists its fonts, newest format first; the first readable one is used. API 35+ ship both (the old
/// one kept for apps); `font_fallback.xml` may be unreadable to apps, then `fonts.xml` serves.
const FONT_LISTS: &[&str] = &["/system/etc/font_fallback.xml", "/system/etc/fonts.xml"];

/// The files both lists name.
const FONT_DIR: &str = "/system/fonts";

/// cosmic-text's locale when the surface sent none (an empty `LocaleList`): what upstream's
/// `new_without_system_fonts` uses, so behaviour matches upstream's default.
const FALLBACK_LOCALE: &str = "en-US";

/// Parallax's Java helper that draws emoji (`java/io/github/xuckless/parallax/EmojiRasterizer.java`).
const EMOJI_CLASS: &str = "io/github/xuckless/parallax/EmojiRasterizer";

/// GPUI's text system with Android's system fonts. Every method is upstream's but `font_id` and, for colour emoji,
/// `layout_line`, `glyph_raster_bounds` and `rasterize_glyph`.
pub(crate) struct AndroidTextSystem {
    cosmic: CosmicTextSystem,
    /// fonts.xml family and alias names (`sans-serif`, `monospace`, `arial`) → the family name inside the file,
    /// and the weight an alias implies (`sans-serif-medium`).
    generics: HashMap<String, (SharedString, Option<FontWeight>)>,
    /// The per-language faces' family names, the app's locales first: appended to every font's fallbacks.
    by_locale: Vec<String>,
    /// `None` when the Java helper could not be reached (logged): emoji stay on upstream's path.
    emoji: Option<Emoji>,
}

/// Android's half of colour emoji. Used on the main thread only (GPUI lays out and paints there), like the JNI calls.
struct Emoji {
    vm: JavaVM,
    /// `EmojiRasterizer`, looked up once while a Java frame is on the stack (so the app's class loader finds it).
    class: GlobalRef,
    /// Each system emoji face → its Android `Typeface`, made from the same file cosmic-text shapes with.
    faces: HashMap<fontdb::ID, GlobalRef>,
    /// The emoji clusters layouts produced, by the glyph id [`emoji::mark`] gave each.
    clusters: Mutex<Clusters>,
}

/// Loads the system fonts for `locales` (BCP-47, the app's preference order). On any failure the text system has
/// no system fonts, as before Phase 1, and the error is logged: text still renders with fonts the app adds.
pub(crate) fn load(vm: &JavaVM, locales: &[String]) -> Arc<dyn PlatformTextSystem> {
    let started = Instant::now();
    let planned = FONT_LISTS
        .iter()
        .find_map(|list| Some((*list, std::fs::read_to_string(list).ok()?)))
        .ok_or(Error::NoFontList(FONT_LISTS))
        .and_then(|(list, xml)| {
            let config = fonts::parse(&xml)?;
            Ok((
                list,
                fonts::plan(&config, Path::new(FONT_DIR), locales, |path| {
                    File::open(path).is_ok()
                })?,
            ))
        });
    match planned {
        Ok((list, plan)) => match AndroidTextSystem::new(vm, &plan, locales) {
            Ok(text_system) => {
                log::info!(
                    "parallax: fonts: {} files from {list} ({} listed but missing) in {:?}; default {}, \
                     per-language order {:?} for locales {locales:?}; {} of {} emoji faces drawn by Android",
                    plan.files.len(),
                    plan.missing.len(),
                    started.elapsed(),
                    plan.default.path.display(),
                    text_system.by_locale,
                    text_system
                        .emoji
                        .as_ref()
                        .map_or(0, |emoji| emoji.faces.len()),
                    plan.emoji.len(),
                );
                return Arc::new(text_system);
            }
            Err(error) => log::error!("parallax: fonts: cannot load {list}: {error}"),
        },
        Err(error) => log::error!("parallax: fonts: {error}"),
    }
    Arc::new(CosmicTextSystem::new_without_system_fonts("sans-serif"))
}

impl AndroidTextSystem {
    fn new(vm: &JavaVM, plan: &FontPlan, locales: &[String]) -> Result<Self, Error> {
        // Files load in plan order (cosmic-text's last-resort scan follows it). Each mapping is owned by the
        // database and lives as long as it; the pages stay in the shared page cache, not in the app's heap. The
        // plan opened each file once already; one that still fails here is skipped and logged, like a missing one.
        let mut db = fontdb::Database::new();
        let mut ids: HashMap<&PathBuf, Vec<fontdb::ID>> = HashMap::new();
        for path in &plan.files {
            let mapped = File::open(path)
                .map_err(|error| Error::Os("open font", error))
                .and_then(|file| {
                    // SAFETY: system fonts live on the read-only system partition; nothing truncates or
                    // rewrites them while mapped.
                    unsafe { memmap2::Mmap::map(&file) }
                        .map_err(|error| Error::Os("mmap font", error))
                });
            match mapped {
                Ok(mapped) => {
                    let loaded = db.load_font_source(fontdb::Source::Binary(Arc::new(mapped)));
                    ids.insert(path, loaded.to_vec());
                }
                Err(error) => log::warn!("parallax: fonts: skipped {}: {error}", path.display()),
            }
        }

        // Family names live in the files, not the XML: read them from the faces just loaded.
        let family = |face: &Face| -> Option<String> {
            ids.get(&face.path)?
                .iter()
                .filter_map(|id| db.face(*id))
                .find(|info| info.index == face.index)?
                .families
                .first()
                .map(|(name, _)| name.clone())
        };
        let default = family(&plan.default).ok_or(Error::NoSystemFont)?;
        let generics = plan
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
            .collect();
        let by_locale = plan.by_locale.iter().filter_map(family).collect();
        let emoji_faces = plan.emoji.iter().filter_map(|face| {
            let id = ids
                .get(&face.path)?
                .iter()
                .copied()
                .find(|id| db.face(*id).is_some_and(|info| info.index == face.index))?;
            Some((id, face))
        });
        let emoji = Emoji::new(vm, emoji_faces)
            .inspect_err(|error| {
                log::error!("parallax: fonts: emoji stay on swash (COLRv1 is blank): {error}")
            })
            .ok();

        // cosmic-text's Android fallback table is empty, so its locale changes nothing yet; the per-language order
        // stays GPUI fallbacks on every font (`font_id`). The app's locale is still the right one to hand over.
        let locale = locales
            .first()
            .map_or(FALLBACK_LOCALE, String::as_str)
            .to_string();
        Ok(Self {
            cosmic: CosmicTextSystem::new_with_font_system(
                FontSystem::new_with_locale_and_db(locale, db),
                &default,
            ),
            generics,
            by_locale,
            emoji,
        })
    }

    /// Android's drawing of `params`, or `None` for upstream's path ([`emoji::route`]).
    fn emoji_raster(&self, params: &RenderGlyphParams) -> Option<(&Emoji, &GlobalRef, String)> {
        let emoji = self.emoji.as_ref()?;
        let face = self
            .cosmic
            .font_database_id(params.font_id)
            .and_then(|id| emoji.faces.get(&id));
        let cluster = emoji
            .clusters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .text(params.glyph_id)
            .map(str::to_string);
        match emoji::route(params.is_emoji, face, cluster) {
            Raster::Android { face, cluster } => Some((emoji, face, cluster)),
            Raster::Upstream => None,
        }
    }
}

impl Emoji {
    fn new<'a>(
        vm: &JavaVM,
        faces: impl Iterator<Item = (fontdb::ID, &'a Face)>,
    ) -> Result<Self, Error> {
        let mut env = vm.get_env()?;
        let class = env.find_class(EMOJI_CLASS)?;
        let mut emoji = Self {
            vm: env.get_java_vm()?,
            class: env.new_global_ref(class)?,
            faces: HashMap::new(),
            clusters: Mutex::default(),
        };
        for (id, face) in faces {
            let typeface = emoji.call(|env, class| {
                let path = env.new_string(face.path.to_string_lossy())?;
                let typeface = env
                    .call_static_method(
                        class,
                        "typeface",
                        "(Ljava/lang/String;I)Landroid/graphics/Typeface;",
                        &[JValue::Object(&path), JValue::Int(face.index as i32)],
                    )?
                    .l()?;
                (!typeface.is_null())
                    .then(|| env.new_global_ref(typeface))
                    .transpose()
            })?;
            match typeface {
                Some(typeface) => {
                    emoji.faces.insert(id, typeface);
                }
                None => log::warn!(
                    "parallax: fonts: Android cannot load emoji face {}, left to swash",
                    face.path.display()
                ),
            }
        }
        Ok(emoji)
    }

    /// Runs `f` on the main thread's `JNIEnv` in its own local frame (a frame paints many glyphs inside one Java
    /// callback), with `EmojiRasterizer`. A Java exception becomes an error and is cleared.
    fn call<T>(
        &self,
        f: impl FnOnce(&mut JNIEnv, &JClass) -> jni::errors::Result<T>,
    ) -> Result<T, Error> {
        let mut env = self.vm.get_env()?;
        let class = <&JClass>::from(self.class.as_obj());
        let result = env.with_local_frame(8, |env| f(env, class));
        if result.is_err() && env.exception_check()? {
            env.exception_clear()?;
        }
        Ok(result?)
    }

    /// The pixels `cluster` covers at `size_px` device pixels, from the glyph origin, y down.
    fn bounds(
        &self,
        typeface: &GlobalRef,
        cluster: &str,
        size_px: f32,
    ) -> Result<Bounds<DevicePixels>, Error> {
        let [left, top, right, bottom] = self.call(|env, class| {
            let text = env.new_string(cluster)?;
            let bounds = JIntArray::from(
                env.call_static_method(
                    class,
                    "bounds",
                    "(Landroid/graphics/Typeface;Ljava/lang/String;F)[I",
                    &[
                        JValue::Object(typeface.as_obj()),
                        JValue::Object(&text),
                        JValue::Float(size_px),
                    ],
                )?
                .l()?,
            );
            let mut out = [0; 4];
            env.get_int_array_region(&bounds, 0, &mut out)?;
            Ok(out)
        })?;
        Ok(Bounds {
            origin: point(DevicePixels(left), DevicePixels(top)),
            size: size(DevicePixels(right - left), DevicePixels(bottom - top)),
        })
    }

    /// `cluster` drawn into `bounds` (from [`Self::bounds`]), as GPUI's colour atlas takes it: BGRA, unpremultiplied.
    fn draw(
        &self,
        typeface: &GlobalRef,
        cluster: &str,
        size_px: f32,
        bounds: Bounds<DevicePixels>,
    ) -> Result<Vec<u8>, Error> {
        let started = Instant::now();
        let pixels = self.call(|env, class| {
            let text = env.new_string(cluster)?;
            let pixels = JIntArray::from(
                env.call_static_method(
                    class,
                    "draw",
                    "(Landroid/graphics/Typeface;Ljava/lang/String;FIIII)[I",
                    &[
                        JValue::Object(typeface.as_obj()),
                        JValue::Object(&text),
                        JValue::Float(size_px),
                        JValue::Int(bounds.origin.x.0),
                        JValue::Int(bounds.origin.y.0),
                        JValue::Int(bounds.size.width.0),
                        JValue::Int(bounds.size.height.0),
                    ],
                )?
                .l()?,
            );
            let mut out = vec![0; env.get_array_length(&pixels)? as usize];
            env.get_int_array_region(&pixels, 0, &mut out)?;
            Ok(out)
        })?;
        log::debug!(
            "parallax: emoji: drew {cluster:?} at {size_px} px ({}x{}) in {:?}",
            bounds.size.width.0,
            bounds.size.height.0,
            started.elapsed()
        );
        // Java's ARGB ints as little-endian bytes are B, G, R, A.
        Ok(pixels
            .iter()
            .flat_map(|pixel| pixel.to_le_bytes())
            .collect())
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
        match self.emoji_raster(params) {
            Some((emoji, typeface, cluster)) => Ok(emoji.bounds(
                typeface,
                &cluster,
                f32::from(params.font_size) * params.scale_factor,
            )?),
            None => self.cosmic.glyph_raster_bounds(params),
        }
    }

    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> anyhow::Result<(Size<DevicePixels>, Vec<u8>)> {
        match self.emoji_raster(params) {
            Some((emoji, typeface, cluster)) => Ok((
                raster_bounds.size,
                emoji.draw(
                    typeface,
                    &cluster,
                    f32::from(params.font_size) * params.scale_factor,
                    raster_bounds,
                )?,
            )),
            None => self.cosmic.rasterize_glyph(params, raster_bounds),
        }
    }

    /// Upstream's layout, with the system emoji faces' runs rewritten for Android to draw ([`emoji::mark`]).
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout {
        let mut layout = self.cosmic.layout_line(text, font_size, runs);
        if let Some(emoji) = &self.emoji {
            emoji::mark(
                &mut layout,
                text,
                |font_id| {
                    self.cosmic
                        .font_database_id(font_id)
                        .is_some_and(|id| emoji.faces.contains_key(&id))
                },
                |cluster| {
                    emoji
                        .clusters
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .id(cluster)
                },
            );
        }
        layout
    }

    fn recommended_rendering_mode(&self, font_id: FontId, font_size: Pixels) -> TextRenderingMode {
        self.cosmic.recommended_rendering_mode(font_id, font_size)
    }

    fn glyph_dilation_for_color(&self, color: Hsla) -> u8 {
        self.cosmic.glyph_dilation_for_color(color)
    }
}
