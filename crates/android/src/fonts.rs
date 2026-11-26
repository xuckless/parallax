//! Android's system font list as plain data: `fonts.xml` (API 28–34) or `font_fallback.xml` (API 35+) parsed into
//! families and aliases, then planned into what to load and in which order. Pure: the shell that reads the files
//! is [`crate::text_system`]. How it fits together: `docs/android-fonts.md`.

use std::path::{Path, PathBuf};

use crate::Error;

/// Both XML formats, as far as Parallax reads them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FontConfig {
    /// In file order: Android tries fallback families in this order.
    pub(crate) families: Vec<Family>,
    pub(crate) aliases: Vec<Alias>,
}

/// One `<family>`: named (`sans-serif`), or a fallback (unnamed, often `lang`-tagged).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Family {
    pub(crate) name: Option<String>,
    /// BCP-47 tags from `lang="zh-Hant,zh-Bopo"` (API 35) or `lang="zh-Hant zh-Bopo"` (API 28).
    pub(crate) langs: Vec<String>,
    /// `ignore="true"`: Android itself skips the family.
    pub(crate) ignore: bool,
    pub(crate) fonts: Vec<FontFile>,
}

/// One `<font>`: a file in the font directory and the face inside it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FontFile {
    pub(crate) file: String,
    /// The face in a collection (`.ttc`): `index="2"`.
    pub(crate) index: u32,
    /// `fallbackFor="serif"`: a fallback only for that named family.
    pub(crate) fallback_for: Option<String>,
}

/// `<alias name="arial" to="sans-serif" weight="..."/>`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Alias {
    pub(crate) name: String,
    pub(crate) to: String,
    pub(crate) weight: Option<u16>,
}

/// Parses either format: both are `<familyset>` of `<family>` (with `<font>` children whose text is the file name)
/// and `<alias>`. Attributes Parallax does not use (`weight`, `style`, `variant`, `supportedAxes`, `<axis>`) are
/// skipped.
pub(crate) fn parse(xml: &str) -> Result<FontConfig, Error> {
    let document = roxmltree::Document::parse(xml).map_err(Error::FontsXml)?;
    let root = document.root_element();
    if !root.has_tag_name("familyset") {
        return Err(Error::FontsXmlRoot(root.tag_name().name().to_string()));
    }
    let mut config = FontConfig::default();
    for node in root.children().filter(roxmltree::Node::is_element) {
        match node.tag_name().name() {
            "family" => config.families.push(Family {
                name: node.attribute("name").map(str::to_string),
                langs: node
                    .attribute("lang")
                    .map(|langs| {
                        langs
                            .split([',', ' '])
                            .filter(|lang| !lang.is_empty())
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                ignore: node.attribute("ignore") == Some("true"),
                fonts: node
                    .children()
                    .filter(|child| child.has_tag_name("font"))
                    .map(|font| {
                        Ok(FontFile {
                            // The file name is the font's own text; `<axis>` children carry none.
                            file: font
                                .children()
                                .filter_map(|child| child.text())
                                .collect::<String>()
                                .trim()
                                .to_string(),
                            index: number(font, "index")?.unwrap_or(0),
                            fallback_for: font.attribute("fallbackFor").map(str::to_string),
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            }),
            "alias" => {
                let required = |attribute| {
                    node.attribute(attribute)
                        .map(str::to_string)
                        .ok_or_else(|| bad_attribute(node, attribute, None))
                };
                config.aliases.push(Alias {
                    name: required("name")?,
                    to: required("to")?,
                    weight: number(node, "weight")?,
                });
            }
            // OEM additions Parallax does not read yet; skipping keeps their fonts.xml usable.
            _ => {}
        }
    }
    Ok(config)
}

fn number<T: std::str::FromStr>(
    node: roxmltree::Node,
    attribute: &'static str,
) -> Result<Option<T>, Error> {
    node.attribute(attribute)
        .map(|value| {
            value
                .parse()
                .map_err(|_| bad_attribute(node, attribute, Some(value)))
        })
        .transpose()
}

fn bad_attribute(node: roxmltree::Node, attribute: &'static str, value: Option<&str>) -> Error {
    Error::FontsXmlAttribute {
        element: node.tag_name().name().to_string(),
        attribute,
        value: value.map(str::to_string),
    }
}

/// The script subtag fonts.xml tags colour emoji families with (`lang="und-Zsye"`): the emoji font and, on newer
/// images, the separate flags font. Android draws these faces' glyphs itself (`docs/android-emoji.md`).
const EMOJI_SCRIPT: &str = "Zsye";

/// Locales that name no script → the Han script Android's families are tagged with. Rows are tried in order; an
/// empty region matches any. Add a row for another language whose fonts.xml families carry a script.
const LIKELY_SCRIPTS: &[(&str, &str, &str)] = &[
    ("zh", "TW", "Hant"),
    ("zh", "HK", "Hant"),
    ("zh", "MO", "Hant"),
    ("zh", "", "Hans"),
];

/// One face of one file.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Face {
    pub(crate) path: PathBuf,
    pub(crate) index: u32,
}

/// A name apps can ask for (`sans-serif`, `monospace`, `arial`) → its face, and the weight an alias implies.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Generic {
    pub(crate) name: String,
    pub(crate) face: Face,
    pub(crate) weight: Option<u16>,
}

/// What the shell loads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FontPlan {
    /// Every file to load, once each, in XML order: cosmic-text's last-resort fallback tries faces in load order.
    pub(crate) files: Vec<PathBuf>,
    /// The first family's face ("first font is default"): GPUI's `.SystemUIFont`.
    pub(crate) default: Face,
    pub(crate) generics: Vec<Generic>,
    /// Language-tagged fallbacks (CJK), those matching the app's locales first, then XML order: per-language glyphs
    /// for Han text, as Android picks them.
    pub(crate) by_locale: Vec<Face>,
    /// The colour emoji faces (`EMOJI_SCRIPT` families), in XML order.
    pub(crate) emoji: Vec<Face>,
    /// Listed files not on disk (OEM images drop some) or not readable.
    pub(crate) missing: Vec<PathBuf>,
}

/// Plans the load: files in `dir` that are `readable`, ignored families skipped. A file that
/// is on disk but cannot be read (permissions, a broken OEM image) is `missing` like an absent one, so it costs only
/// itself, never the other system fonts.
pub(crate) fn plan(
    config: &FontConfig,
    dir: &Path,
    locales: &[String],
    readable: impl Fn(&Path) -> bool,
) -> Result<FontPlan, Error> {
    let path = |file: &str| dir.join(file);
    // A family's face for primary or per-language use: its first font that is on disk and not serif-only.
    let face = |family: &Family| {
        family
            .fonts
            .iter()
            .filter(|font| font.fallback_for.is_none())
            .map(|font| Face {
                path: path(&font.file),
                index: font.index,
            })
            .find(|face| readable(&face.path))
    };

    let mut files = Vec::new();
    let mut missing = Vec::new();
    for font in config
        .families
        .iter()
        .filter(|family| !family.ignore)
        .flat_map(|family| &family.fonts)
    {
        let path = path(&font.file);
        let list = if readable(&path) {
            &mut files
        } else {
            &mut missing
        };
        if !list.contains(&path) {
            list.push(path);
        }
    }

    let default = config
        .families
        .iter()
        .find(|family| !family.ignore)
        .and_then(face)
        .ok_or(Error::NoSystemFont)?;

    let families = config.families.iter().filter_map(|family| {
        Some(Generic {
            name: family.name.clone()?,
            face: face(family)?,
            weight: None,
        })
    });
    // An alias names its family; one to a family that is not listed (OEM edits) is dropped, as Android does.
    let named = |name: &str| {
        config
            .families
            .iter()
            .find(|family| family.name.as_deref() == Some(name))
            .and_then(face)
    };
    let aliases = config.aliases.iter().filter_map(|alias| {
        Some(Generic {
            name: alias.name.clone(),
            face: named(&alias.to)?,
            weight: alias.weight,
        })
    });
    let generics = families.chain(aliases).collect();

    let mut tagged: Vec<(usize, Face)> = config
        .families
        .iter()
        .filter(|family| {
            !family.ignore
                && family.name.is_none()
                && family.langs.iter().any(|lang| language(lang) != "und")
        })
        .filter_map(|family| {
            let rank = locales
                .iter()
                .position(|locale| family.langs.iter().any(|lang| matches(lang, locale)))
                .unwrap_or(locales.len());
            Some((rank, face(family)?))
        })
        .collect();
    tagged.sort_by_key(|(rank, _)| *rank); // stable: ties keep XML order
    let by_locale = tagged.into_iter().map(|(_, face)| face).collect();

    let emoji = config
        .families
        .iter()
        .filter(|family| {
            !family.ignore
                && family
                    .langs
                    .iter()
                    .any(|lang| explicit_script(lang) == Some(EMOJI_SCRIPT))
        })
        .flat_map(|family| &family.fonts)
        .map(|font| Face {
            path: path(&font.file),
            index: font.index,
        })
        .filter(|face| readable(&face.path))
        .collect();

    Ok(FontPlan {
        files,
        default,
        generics,
        by_locale,
        emoji,
        missing,
    })
}

fn subtags(tag: &str) -> impl Iterator<Item = &str> {
    tag.split(['-', '_'])
}

fn language(tag: &str) -> &str {
    subtags(tag).next().unwrap_or("")
}

/// The script subtag a tag names itself (`zh-Hant-TW`): the one four letters long.
fn explicit_script(tag: &str) -> Option<&str> {
    subtags(tag).skip(1).find(|subtag| subtag.len() == 4)
}

/// The script subtag (`zh-Hant-TW`), else the likely one for the language and region.
fn script(tag: &str) -> Option<&str> {
    explicit_script(tag).or_else(|| {
        let region = subtags(tag)
            .skip(1)
            .find(|subtag| subtag.len() == 2 || subtag.len() == 3)
            .unwrap_or("");
        LIKELY_SCRIPTS
            .iter()
            .find(|(lang, row_region, _)| {
                lang.eq_ignore_ascii_case(language(tag))
                    && (row_region.is_empty() || row_region.eq_ignore_ascii_case(region))
            })
            .map(|(_, _, script)| *script)
    })
}

/// A family's `lang` serves a locale when the languages match and, if the family names a script, so does the locale.
fn matches(family_lang: &str, locale: &str) -> bool {
    language(family_lang).eq_ignore_ascii_case(language(locale))
        && explicit_script(family_lang).is_none_or(|wanted| {
            script(locale).is_some_and(|script| script.eq_ignore_ascii_case(wanted))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// API 28 `fonts.xml` shape: `version`, weight/style on every font, a TTC index, `ignore`, aliases.
    const FONTS_XML_28: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<familyset version="23">
    <!-- first font is default -->
    <family name="sans-serif">
        <font weight="400" style="normal">Roboto-Regular.ttf</font>
        <font weight="700" style="normal">Roboto-Bold.ttf</font>
    </family>
    <alias name="sans-serif-medium" to="sans-serif" weight="500" />
    <alias name="arial" to="sans-serif" />
    <family name="monospace">
        <font weight="400" style="normal">DroidSansMono.ttf</font>
    </family>
    <alias name="courier" to="serif-monospace" />
    <family lang="und-Arab" variant="elegant">
        <font weight="400" style="normal">NotoNaskhArabic-Regular.ttf</font>
    </family>
    <family lang="zh-Hans">
        <font weight="400" style="normal" index="2">NotoSansCJK-Regular.ttc</font>
        <font weight="400" style="normal" index="2" fallbackFor="serif">NotoSerifCJK-Regular.otf</font>
    </family>
    <family lang="zh-Hant zh-Bopo">
        <font weight="400" style="normal" index="3">NotoSansCJK-Regular.ttc</font>
    </family>
    <family lang="ja">
        <font weight="400" style="normal" index="0">NotoSansCJK-Regular.ttc</font>
    </family>
    <family lang="ko">
        <font weight="400" style="normal" index="1">NotoSansCJK-Regular.ttc</font>
    </family>
    <family lang="und-Zsye">
        <font weight="400" style="normal">NotoColorEmoji.ttf</font>
    </family>
</familyset>"#;

    /// API 35 `font_fallback.xml` shape: no `version`, `supportedAxes` instead of weight, `<axis>` children,
    /// `postScriptName`, the file name on its own line.
    const FONT_FALLBACK_XML_35: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<familyset>
    <family name="sans-serif">
        <font supportedAxes="wght,ital">Roboto-Regular.ttf
          <axis tag="wdth" stylevalue="100" />
        </font>
   </family>
    <alias name="sans-serif-thin" to="sans-serif" weight="100" />
    <family lang="ja">
        <font weight="400" style="normal" index="0" postScriptName="NotoSansCJKjp-Regular">
            NotoSansCJK-Regular.ttc
        </font>
    </family>
    <family lang="und-Zsye" ignore="true">
        <font weight="400" style="normal">NotoColorEmojiLegacy.ttf</font>
    </family>
    <family lang="und-Zsye">
        <font weight="400" style="normal">NotoColorEmoji.ttf</font>
    </family>
    <family lang="und-Zsye">
        <font weight="400" style="normal">NotoColorEmojiFlags.ttf</font>
    </family>
    <family lang="und-Zsym">
        <font weight="400" style="normal">NotoSansSymbols-Regular-Subsetted2.ttf</font>
    </family>
</familyset>"#;

    fn font(file: &str, index: u32, fallback_for: Option<&str>) -> FontFile {
        FontFile {
            file: file.into(),
            index,
            fallback_for: fallback_for.map(str::to_string),
        }
    }

    fn family(name: Option<&str>, langs: &[&str], ignore: bool, fonts: Vec<FontFile>) -> Family {
        Family {
            name: name.map(str::to_string),
            langs: langs.iter().map(|lang| lang.to_string()).collect(),
            ignore,
            fonts,
        }
    }

    fn alias(name: &str, to: &str, weight: Option<u16>) -> Alias {
        Alias {
            name: name.into(),
            to: to.into(),
            weight,
        }
    }

    #[test]
    fn parse_table() -> Result<(), Error> {
        let rows: &[(&str, &str, FontConfig)] = &[
            (
                "fonts.xml (API 28): families, TTC indices, fallbackFor, aliases in place",
                FONTS_XML_28,
                FontConfig {
                    families: vec![
                        family(
                            Some("sans-serif"),
                            &[],
                            false,
                            vec![
                                font("Roboto-Regular.ttf", 0, None),
                                font("Roboto-Bold.ttf", 0, None),
                            ],
                        ),
                        family(
                            Some("monospace"),
                            &[],
                            false,
                            vec![font("DroidSansMono.ttf", 0, None)],
                        ),
                        family(
                            None,
                            &["und-Arab"],
                            false,
                            vec![font("NotoNaskhArabic-Regular.ttf", 0, None)],
                        ),
                        family(
                            None,
                            &["zh-Hans"],
                            false,
                            vec![
                                font("NotoSansCJK-Regular.ttc", 2, None),
                                font("NotoSerifCJK-Regular.otf", 2, Some("serif")),
                            ],
                        ),
                        family(
                            None,
                            &["zh-Hant", "zh-Bopo"],
                            false,
                            vec![font("NotoSansCJK-Regular.ttc", 3, None)],
                        ),
                        family(
                            None,
                            &["ja"],
                            false,
                            vec![font("NotoSansCJK-Regular.ttc", 0, None)],
                        ),
                        family(
                            None,
                            &["ko"],
                            false,
                            vec![font("NotoSansCJK-Regular.ttc", 1, None)],
                        ),
                        family(
                            None,
                            &["und-Zsye"],
                            false,
                            vec![font("NotoColorEmoji.ttf", 0, None)],
                        ),
                    ],
                    aliases: vec![
                        alias("sans-serif-medium", "sans-serif", Some(500)),
                        alias("arial", "sans-serif", None),
                        alias("courier", "serif-monospace", None),
                    ],
                },
            ),
            (
                "font_fallback.xml (API 35): axes skipped, file name trimmed, ignore kept",
                FONT_FALLBACK_XML_35,
                FontConfig {
                    families: vec![
                        family(
                            Some("sans-serif"),
                            &[],
                            false,
                            vec![font("Roboto-Regular.ttf", 0, None)],
                        ),
                        family(
                            None,
                            &["ja"],
                            false,
                            vec![font("NotoSansCJK-Regular.ttc", 0, None)],
                        ),
                        family(
                            None,
                            &["und-Zsye"],
                            true,
                            vec![font("NotoColorEmojiLegacy.ttf", 0, None)],
                        ),
                        family(
                            None,
                            &["und-Zsye"],
                            false,
                            vec![font("NotoColorEmoji.ttf", 0, None)],
                        ),
                        family(
                            None,
                            &["und-Zsye"],
                            false,
                            vec![font("NotoColorEmojiFlags.ttf", 0, None)],
                        ),
                        family(
                            None,
                            &["und-Zsym"],
                            false,
                            vec![font("NotoSansSymbols-Regular-Subsetted2.ttf", 0, None)],
                        ),
                    ],
                    aliases: vec![alias("sans-serif-thin", "sans-serif", Some(100))],
                },
            ),
            (
                "lang lists: commas (API 35), spaces (API 28), both",
                r#"<familyset><family lang="zh-Hant,zh-Bopo"/><family lang="zh-Hant zh-Bopo"/>
                    <family lang="und-Geor, und-Geok"/></familyset>"#,
                FontConfig {
                    families: vec![
                        family(None, &["zh-Hant", "zh-Bopo"], false, vec![]),
                        family(None, &["zh-Hant", "zh-Bopo"], false, vec![]),
                        family(None, &["und-Geor", "und-Geok"], false, vec![]),
                    ],
                    aliases: vec![],
                },
            ),
            (
                "unknown elements (OEM additions) are skipped",
                r#"<familyset><vendor-thing/><family name="a"><font>A.ttf</font></family></familyset>"#,
                FontConfig {
                    families: vec![family(Some("a"), &[], false, vec![font("A.ttf", 0, None)])],
                    aliases: vec![],
                },
            ),
        ];
        for (name, xml, expected) in rows {
            assert_eq!(&parse(xml)?, expected, "{name}");
        }
        Ok(())
    }

    #[test]
    fn parse_errors_table() {
        let rows: &[(&str, &str, &str)] = &[
            (
                "malformed XML",
                "<familyset><family>",
                "fonts.xml: the root node was opened but never closed",
            ),
            (
                "wrong root element",
                "<fonts/>",
                "fonts.xml: root element is <fonts>, not <familyset>",
            ),
            (
                "index is not a number",
                r#"<familyset><family><font index="two">A.ttc</font></family></familyset>"#,
                "fonts.xml: <font index> is missing or invalid: Some(\"two\")",
            ),
            (
                "alias without a target",
                r#"<familyset><alias name="arial"/></familyset>"#,
                "fonts.xml: <alias to> is missing or invalid: None",
            ),
            (
                "alias weight is not a number",
                r#"<familyset><alias name="a" to="b" weight="bold"/></familyset>"#,
                "fonts.xml: <alias weight> is missing or invalid: Some(\"bold\")",
            ),
        ];
        for (name, xml, prefix) in rows {
            let error = parse(xml).err().map(|error| error.to_string());
            assert!(
                error
                    .as_ref()
                    .is_some_and(|error| error.starts_with(prefix)),
                "{name}: {error:?}"
            );
        }
    }

    fn face(file: &str, index: u32) -> Face {
        Face {
            path: Path::new("/f").join(file),
            index,
        }
    }

    fn paths(files: &[&str]) -> Vec<PathBuf> {
        files
            .iter()
            .map(|file| Path::new("/f").join(file))
            .collect()
    }

    /// `(name, xml, locales, files on disk, expected by_locale)`.
    /// `(name, xml, files on disk, expected plan or error message)`.
    type PlanRow<'a> = (&'a str, &'a str, Vec<&'a str>, Result<FontPlan, &'a str>);

    type LocaleRow<'a> = (&'a str, &'a str, &'a [&'a str], Vec<Face>);

    #[test]
    fn by_locale_table() -> Result<(), Error> {
        let cjk = |indices: [u32; 4]| {
            indices
                .map(|index| face("NotoSansCJK-Regular.ttc", index))
                .to_vec()
        };
        let rows: &[LocaleRow] = &[
            ("no locale: XML order", FONTS_XML_28, &[], cjk([2, 3, 0, 1])),
            (
                "en-US matches none: XML order",
                FONTS_XML_28,
                &["en-US"],
                cjk([2, 3, 0, 1]),
            ),
            (
                "ja-JP: Japanese first",
                FONTS_XML_28,
                &["ja-JP"],
                cjk([0, 2, 3, 1]),
            ),
            (
                "ko-KR: Korean first",
                FONTS_XML_28,
                &["ko-KR"],
                cjk([1, 2, 3, 0]),
            ),
            (
                "zh-CN: likely script Hans",
                FONTS_XML_28,
                &["zh-CN"],
                cjk([2, 3, 0, 1]),
            ),
            (
                "zh-TW: likely script Hant",
                FONTS_XML_28,
                &["zh-TW"],
                cjk([3, 2, 0, 1]),
            ),
            (
                "zh-HK: likely script Hant",
                FONTS_XML_28,
                &["zh-HK"],
                cjk([3, 2, 0, 1]),
            ),
            (
                "zh-Hant-CN: explicit script wins",
                FONTS_XML_28,
                &["zh-Hant-CN"],
                cjk([3, 2, 0, 1]),
            ),
            (
                "Java underscore form ja_JP",
                FONTS_XML_28,
                &["ja_JP"],
                cjk([0, 2, 3, 1]),
            ),
            (
                "locale list order: en, ko, ja",
                FONTS_XML_28,
                &["en-US", "ko-KR", "ja-JP"],
                cjk([1, 0, 2, 3]),
            ),
        ];
        for (name, xml, locales, expected) in rows {
            let locales: Vec<String> = locales.iter().map(|locale| locale.to_string()).collect();
            let plan = plan(&parse(xml)?, Path::new("/f"), &locales, |_| true)?;
            assert_eq!(&plan.by_locale, expected, "{name}");
        }
        Ok(())
    }

    const NO_SYSTEM_FONT: &str = "no system font: the font list names no font file that is on disk";

    #[test]
    fn plan_table() -> Result<(), Error> {
        let all: &[&str] = &[
            "Roboto-Regular.ttf",
            "Roboto-Bold.ttf",
            "DroidSansMono.ttf",
            "NotoNaskhArabic-Regular.ttf",
            "NotoSansCJK-Regular.ttc",
            "NotoSerifCJK-Regular.otf",
            "NotoColorEmoji.ttf",
        ];
        let generic = |name: &str, file: &str, weight| Generic {
            name: name.into(),
            face: face(file, 0),
            weight,
        };
        let generics_28 = vec![
            generic("sans-serif", "Roboto-Regular.ttf", None),
            generic("monospace", "DroidSansMono.ttf", None),
            generic("sans-serif-medium", "Roboto-Regular.ttf", Some(500)),
            generic("arial", "Roboto-Regular.ttf", None),
        ];
        let rows: Vec<PlanRow> = vec![
            (
                "API 28: every file once, alias to an unlisted family dropped",
                FONTS_XML_28,
                all.to_vec(),
                Ok(FontPlan {
                    files: paths(all),
                    default: face("Roboto-Regular.ttf", 0),
                    generics: generics_28.clone(),
                    by_locale: [2, 3, 0, 1]
                        .map(|index| face("NotoSansCJK-Regular.ttc", index))
                        .to_vec(),
                    emoji: vec![face("NotoColorEmoji.ttf", 0)],
                    missing: vec![],
                }),
            ),
            (
                "missing files are listed, and the next font of a family stands in",
                FONTS_XML_28,
                vec!["Roboto-Bold.ttf", "DroidSansMono.ttf", "NotoColorEmoji.ttf"],
                Ok(FontPlan {
                    files: paths(&["Roboto-Bold.ttf", "DroidSansMono.ttf", "NotoColorEmoji.ttf"]),
                    default: face("Roboto-Bold.ttf", 0),
                    generics: vec![
                        generic("sans-serif", "Roboto-Bold.ttf", None),
                        generic("monospace", "DroidSansMono.ttf", None),
                        generic("sans-serif-medium", "Roboto-Bold.ttf", Some(500)),
                        generic("arial", "Roboto-Bold.ttf", None),
                    ],
                    by_locale: vec![],
                    emoji: vec![face("NotoColorEmoji.ttf", 0)],
                    missing: paths(&[
                        "Roboto-Regular.ttf",
                        "NotoNaskhArabic-Regular.ttf",
                        "NotoSansCJK-Regular.ttc",
                        "NotoSerifCJK-Regular.otf",
                    ]),
                }),
            ),
            (
                "an unreadable file is missing, the rest load",
                FONTS_XML_28,
                all.iter()
                    .copied()
                    .filter(|file| *file != "DroidSansMono.ttf")
                    .collect(),
                Ok(FontPlan {
                    files: paths(&[
                        "Roboto-Regular.ttf",
                        "Roboto-Bold.ttf",
                        "NotoNaskhArabic-Regular.ttf",
                        "NotoSansCJK-Regular.ttc",
                        "NotoSerifCJK-Regular.otf",
                        "NotoColorEmoji.ttf",
                    ]),
                    default: face("Roboto-Regular.ttf", 0),
                    generics: vec![
                        generic("sans-serif", "Roboto-Regular.ttf", None),
                        generic("sans-serif-medium", "Roboto-Regular.ttf", Some(500)),
                        generic("arial", "Roboto-Regular.ttf", None),
                    ],
                    by_locale: [2, 3, 0, 1]
                        .map(|index| face("NotoSansCJK-Regular.ttc", index))
                        .to_vec(),
                    emoji: vec![face("NotoColorEmoji.ttf", 0)],
                    missing: paths(&["DroidSansMono.ttf"]),
                }),
            ),
            (
                "API 35: COLRv1 emoji and flags are the emoji faces, ignored Legacy and symbols are not",
                FONT_FALLBACK_XML_35,
                vec![
                    "Roboto-Regular.ttf",
                    "NotoSansCJK-Regular.ttc",
                    "NotoColorEmoji.ttf",
                    "NotoColorEmojiLegacy.ttf",
                    "NotoColorEmojiFlags.ttf",
                    "NotoSansSymbols-Regular-Subsetted2.ttf",
                ],
                Ok(FontPlan {
                    files: paths(&[
                        "Roboto-Regular.ttf",
                        "NotoSansCJK-Regular.ttc",
                        "NotoColorEmoji.ttf",
                        "NotoColorEmojiFlags.ttf",
                        "NotoSansSymbols-Regular-Subsetted2.ttf",
                    ]),
                    default: face("Roboto-Regular.ttf", 0),
                    generics: vec![
                        generic("sans-serif", "Roboto-Regular.ttf", None),
                        generic("sans-serif-thin", "Roboto-Regular.ttf", Some(100)),
                    ],
                    by_locale: vec![face("NotoSansCJK-Regular.ttc", 0)],
                    emoji: vec![
                        face("NotoColorEmoji.ttf", 0),
                        face("NotoColorEmojiFlags.ttf", 0),
                    ],
                    missing: vec![],
                }),
            ),
            (
                "API 35 without the flags file: one emoji face, the flags file missing",
                FONT_FALLBACK_XML_35,
                vec![
                    "Roboto-Regular.ttf",
                    "NotoSansCJK-Regular.ttc",
                    "NotoColorEmoji.ttf",
                ],
                Ok(FontPlan {
                    files: paths(&[
                        "Roboto-Regular.ttf",
                        "NotoSansCJK-Regular.ttc",
                        "NotoColorEmoji.ttf",
                    ]),
                    default: face("Roboto-Regular.ttf", 0),
                    generics: vec![
                        generic("sans-serif", "Roboto-Regular.ttf", None),
                        generic("sans-serif-thin", "Roboto-Regular.ttf", Some(100)),
                    ],
                    by_locale: vec![face("NotoSansCJK-Regular.ttc", 0)],
                    emoji: vec![face("NotoColorEmoji.ttf", 0)],
                    missing: paths(&[
                        "NotoColorEmojiFlags.ttf",
                        "NotoSansSymbols-Regular-Subsetted2.ttf",
                    ]),
                }),
            ),
            (
                "no file on disk: no default font",
                FONTS_XML_28,
                vec![],
                Err(NO_SYSTEM_FONT),
            ),
            (
                "no families: no default font",
                "<familyset/>",
                all.to_vec(),
                Err(NO_SYSTEM_FONT),
            ),
        ];
        for (name, xml, readable, expected) in rows {
            let readable = paths(&readable);
            let planned = plan(&parse(xml)?, Path::new("/f"), &[], |path| {
                readable.iter().any(|file| file == path)
            });
            let planned = planned.map_err(|error| error.to_string());
            assert_eq!(planned, expected.map_err(str::to_string), "{name}");
        }
        Ok(())
    }
}
