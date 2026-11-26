//! Which glyphs Android draws: those of the system colour emoji faces (`fonts::plan`'s `emoji`), each drawn from the
//! cluster of text it was shaped from. Pure: the JNI shell is [`crate::text_system`]. Why: `docs/android-emoji.md`.

use std::collections::HashMap;

use gpui::{FontId, GlyphId, LineLayout};

/// Cluster ids start here, above every font's own glyph ids (OpenType's are 16-bit): such a `GlyphId` names a cluster
/// in [`Clusters`], never a glyph in a font.
const FIRST_CLUSTER_ID: u32 = 0x1_0000;

/// The emoji clusters layouts produced, each under the glyph id [`mark`] gave it. Grows with the distinct emoji shown,
/// never shrinks. Not a raster cache: GPUI's atlas keeps the pixels.
#[derive(Debug, Default)]
pub(crate) struct Clusters {
    texts: Vec<String>,
    ids: HashMap<String, GlyphId>,
}

impl Clusters {
    /// The id of `text`, assigned on first sight.
    pub(crate) fn id(&mut self, text: &str) -> GlyphId {
        if let Some(id) = self.ids.get(text) {
            return *id;
        }
        let id = GlyphId(FIRST_CLUSTER_ID + self.texts.len() as u32);
        self.texts.push(text.to_string());
        self.ids.insert(text.to_string(), id);
        id
    }

    /// The text `id` names, if it is a cluster id.
    pub(crate) fn text(&self, id: GlyphId) -> Option<&str> {
        let index = id.0.checked_sub(FIRST_CLUSTER_ID)?;
        self.texts.get(index as usize).map(String::as_str)
    }
}

/// Rewrites the runs of emoji faces for Android to draw. Each cluster (the text from a glyph's index up to the next
/// glyph's index in the line) becomes one glyph, marked emoji (GPUI paints it as a colour sprite), whose id is
/// `cluster_id` of the cluster's text. Android draws text, not glyph ids (`Canvas.drawGlyphs` is API 31+), so the id
/// must name the text: a cluster can shape to several glyphs on one index (a ZWJ sequence newer than the font), and
/// one glyph can stand alone or start such a cluster. The first glyph draws the whole cluster; the rest are dropped.
pub(crate) fn mark(
    layout: &mut LineLayout,
    text: &str,
    is_emoji_face: impl Fn(FontId) -> bool,
    mut cluster_id: impl FnMut(&str) -> GlyphId,
) {
    // Most lines have no emoji: an empty Vec does not allocate, so they cost one lookup per run.
    let emoji_runs: Vec<usize> = (0..layout.runs.len())
        .filter(|run| is_emoji_face(layout.runs[*run].font_id))
        .collect();
    if emoji_runs.is_empty() {
        return;
    }
    let mut starts: Vec<usize> = layout
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .map(|glyph| glyph.index)
        .collect();
    starts.sort_unstable();
    starts.dedup();
    let end = |start: usize| {
        starts
            .iter()
            .find(|next| **next > start)
            .copied()
            .unwrap_or(text.len())
    };
    for run in emoji_runs {
        let glyphs = &mut layout.runs[run].glyphs;
        glyphs.dedup_by_key(|glyph| glyph.index);
        for glyph in glyphs {
            glyph.is_emoji = true;
            if let Some(cluster) = text.get(glyph.index..end(glyph.index)) {
                glyph.id = cluster_id(cluster);
            }
        }
    }
}

/// How one glyph is rasterized.
#[derive(Debug, PartialEq)]
pub(crate) enum Raster<'a, Face> {
    /// Android draws `cluster` with `face`.
    Android { face: &'a Face, cluster: String },
    /// Upstream's swash path.
    Upstream,
}

/// `colour`: GPUI asks for a colour sprite (`RenderGlyphParams::is_emoji`); `face`: the glyph's font is a system emoji
/// face; `cluster`: the text a layout shaped the glyph from.
pub(crate) fn route<'a, Face>(
    colour: bool,
    face: Option<&'a Face>,
    cluster: Option<String>,
) -> Raster<'a, Face> {
    match (colour, face, cluster) {
        (true, Some(face), Some(cluster)) => Raster::Android { face, cluster },
        // A monochrome sprite (Android returns colour pixels only), another font, or a glyph no layout produced
        // (nothing to draw from): swash, which draws CBDT emoji but leaves COLRv1 blank.
        (false, _, _) | (true, None, _) | (true, Some(_), None) => Raster::Upstream,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ShapedGlyph, ShapedRun, point, px};

    const LATIN: FontId = FontId(0);
    const EMOJI: FontId = FontId(1);

    /// `(font, [(glyph id, text index)])` per run.
    type Runs<'a> = &'a [(FontId, &'a [(u32, usize)])];

    fn layout(runs: Runs) -> LineLayout {
        LineLayout {
            runs: runs
                .iter()
                .map(|(font_id, glyphs)| ShapedRun {
                    font_id: *font_id,
                    glyphs: glyphs
                        .iter()
                        .map(|(id, index)| ShapedGlyph {
                            id: GlyphId(*id),
                            position: point(px(0.), px(0.)),
                            index: *index,
                            is_emoji: false,
                        })
                        .collect(),
                })
                .collect(),
            ..LineLayout::default()
        }
    }

    #[test]
    fn mark_table() {
        // (name, text, runs in, runs out: per run its glyphs as cluster text, or `#id` for a font's own glyph)
        let rows: &[(&str, &str, Runs, &[&[&str]])] = &[
            (
                "no emoji: untouched",
                "ab",
                &[(LATIN, &[(1, 0), (2, 1)])],
                &[&["#1", "#2"]],
            ),
            (
                "one emoji after Latin",
                "a😀",
                &[(LATIN, &[(1, 0)]), (EMOJI, &[(5, 1)])],
                &[&["#1"], &["😀"]],
            ),
            (
                "ZWJ family: one glyph for the whole sequence, Latin after",
                "👩‍👩‍👧b",
                &[(EMOJI, &[(9, 0)]), (LATIN, &[(2, 18)])],
                &[&["👩‍👩‍👧"], &["#2"]],
            ),
            (
                "skin tone and flag: one glyph each",
                "👍🏽🇨🇦",
                &[(EMOJI, &[(7, 0), (8, 8)])],
                &[&["👍🏽", "🇨🇦"]],
            ),
            (
                "keycap: digit, VS16 and U+20E3 in one cluster",
                "1️⃣",
                &[(EMOJI, &[(4, 0)])],
                &[&["1️⃣"]],
            ),
            (
                "sequence the font has no glyph for (API 28): its glyphs share an index, the first draws it all",
                "🧑🏾‍🦱 🧑🏾",
                &[
                    (EMOJI, &[(20, 0), (3, 0), (21, 0)]),
                    (LATIN, &[(1, 15)]),
                    (EMOJI, &[(20, 16)]),
                ],
                &[&["🧑🏾‍🦱"], &["#1"], &["🧑🏾"]],
            ),
            (
                "right-to-left line: the next index, not the next glyph",
                "😀😃",
                &[(EMOJI, &[(6, 4), (5, 0)])],
                &[&["😃", "😀"]],
            ),
        ];
        for (name, text, runs, expected) in rows {
            let mut layout = layout(runs);
            let mut clusters = Clusters::default();
            mark(
                &mut layout,
                text,
                |font| font == EMOJI,
                |cluster| clusters.id(cluster),
            );
            let marked: Vec<Vec<String>> = layout
                .runs
                .iter()
                .map(|run| {
                    run.glyphs
                        .iter()
                        .map(|glyph| {
                            assert_eq!(glyph.is_emoji, run.font_id == EMOJI, "{name}: is_emoji");
                            clusters
                                .text(glyph.id)
                                .map_or_else(|| format!("#{}", glyph.id.0), str::to_string)
                        })
                        .collect()
                })
                .collect();
            assert_eq!(&marked, expected, "{name}");
        }
    }

    #[test]
    fn cluster_ids() {
        let mut clusters = Clusters::default();
        let smile = clusters.id("😀");
        assert_eq!(smile, GlyphId(FIRST_CLUSTER_ID), "first id");
        assert_eq!(clusters.id("🇨🇦"), GlyphId(FIRST_CLUSTER_ID + 1), "next id");
        assert_eq!(clusters.id("😀"), smile, "same text, same id");
        assert_eq!(clusters.text(smile), Some("😀"), "text of an id");
        assert_eq!(clusters.text(GlyphId(5)), None, "a font's own glyph id");
        assert_eq!(
            clusters.text(GlyphId(FIRST_CLUSTER_ID + 2)),
            None,
            "an id not given out"
        );
    }

    #[test]
    fn route_table() {
        /// `(name, colour, emoji face, cluster, expected)`.
        type Row<'a> = (
            &'a str,
            bool,
            Option<&'a &'a str>,
            Option<&'a str>,
            Raster<'a, &'a str>,
        );
        let face = "NotoColorEmoji.ttf";
        let rows: &[Row] = &[
            (
                "colour glyph of an emoji face from a layout: Android",
                true,
                Some(&face),
                Some("😀"),
                Raster::Android {
                    face: &face,
                    cluster: "😀".into(),
                },
            ),
            (
                "monochrome request: upstream",
                false,
                Some(&face),
                Some("😀"),
                Raster::Upstream,
            ),
            (
                "another font: upstream",
                true,
                None,
                Some("😀"),
                Raster::Upstream,
            ),
            (
                "no cluster known: upstream",
                true,
                Some(&face),
                None,
                Raster::Upstream,
            ),
            (
                "plain text glyph: upstream",
                false,
                None,
                None,
                Raster::Upstream,
            ),
        ];
        for (name, colour, face, cluster, expected) in rows {
            assert_eq!(
                &route(*colour, *face, cluster.map(str::to_string)),
                expected,
                "{name}"
            );
        }
    }
}
