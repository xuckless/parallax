//! Which glyphs Android draws: those of the system colour emoji faces (`fonts::plan`'s `emoji`), each drawn from the
//! cluster of text it was shaped from. Pure: the JNI shell is [`crate::text_system`]. Why: `docs/android-emoji.md`.

use std::ops::Range;

use gpui::{FontId, GlyphId, LineLayout};

/// Marks every glyph of an emoji face's run as emoji (GPUI then paints it as a colour sprite) and returns each one's
/// cluster: the text from its index up to the next glyph's index in the line, or to `text_len`.
pub(crate) fn clusters(
    layout: &mut LineLayout,
    text_len: usize,
    is_emoji_face: impl Fn(FontId) -> bool,
) -> Vec<((FontId, GlyphId), Range<usize>)> {
    // Most lines have no emoji: an empty Vec does not allocate, so they cost one lookup per run.
    let emoji_runs: Vec<usize> = (0..layout.runs.len())
        .filter(|run| is_emoji_face(layout.runs[*run].font_id))
        .collect();
    if emoji_runs.is_empty() {
        return Vec::new();
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
            .unwrap_or(text_len)
    };
    let mut clusters = Vec::new();
    for run in emoji_runs {
        let run = &mut layout.runs[run];
        for glyph in &mut run.glyphs {
            glyph.is_emoji = true;
            clusters.push(((run.font_id, glyph.id), glyph.index..end(glyph.index)));
        }
    }
    clusters
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
    fn clusters_table() {
        // (name, text, runs, expected clusters as text)
        let rows: &[(&str, &str, Runs, &[&str])] = &[
            ("no emoji", "ab", &[(LATIN, &[(1, 0), (2, 1)])], &[]),
            (
                "one emoji at the end",
                "a😀",
                &[(LATIN, &[(1, 0)]), (EMOJI, &[(5, 1)])],
                &["😀"],
            ),
            (
                "ZWJ family: one glyph for the whole sequence, Latin after",
                "👩‍👩‍👧b",
                &[(EMOJI, &[(9, 0)]), (LATIN, &[(2, 18)])],
                &["👩‍👩‍👧"],
            ),
            (
                "skin tone and flag: one glyph each",
                "👍🏽🇨🇦",
                &[(EMOJI, &[(7, 0), (8, 8)])],
                &["👍🏽", "🇨🇦"],
            ),
            (
                "keycap: digit, VS16 and U+20E3 in one cluster",
                "1️⃣",
                &[(EMOJI, &[(4, 0)])],
                &["1️⃣"],
            ),
            (
                "right-to-left line: the next index, not the next glyph",
                "😀😃",
                &[(EMOJI, &[(6, 4), (5, 0)])],
                &["😃", "😀"],
            ),
        ];
        for (name, text, runs, expected) in rows {
            let mut layout = layout(runs);
            let clusters = clusters(&mut layout, text.len(), |font| font == EMOJI);
            let texts: Vec<&str> = clusters
                .iter()
                .map(|(_, range)| &text[range.clone()])
                .collect();
            assert_eq!(&texts, expected, "{name}");
            for run in &layout.runs {
                for glyph in &run.glyphs {
                    assert_eq!(glyph.is_emoji, run.font_id == EMOJI, "{name}: is_emoji");
                }
            }
        }
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
