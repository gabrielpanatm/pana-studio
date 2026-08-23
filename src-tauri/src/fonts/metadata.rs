use allsorts::{
    binary::read::ReadScope,
    font::read_cmap_subtable,
    font_data::FontData,
    tables::{
        cmap::Cmap,
        os2::{FsSelectionFlag, Os2},
        variable_fonts::fvar::FvarTable,
        FontTableProvider, NameTable,
    },
    tag,
};
use serde::Serialize;

pub const ROMANIAN_GLYPHS: [char; 10] = ['ă', 'â', 'î', 'ș', 'ț', 'Ă', 'Â', 'Î', 'Ș', 'Ț'];

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontLicenseMetadata {
    pub description: Option<String>,
    pub url: Option<String>,
}

impl FontLicenseMetadata {
    pub fn is_empty(&self) -> bool {
        self.description.is_none() && self.url.is_none()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontVariationAxis {
    pub tag: String,
    pub min: f64,
    pub default: f64,
    pub max: f64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontWeightRange {
    pub start: u16,
    pub end: u16,
}

#[derive(Clone, Debug)]
pub struct ParsedFontMetadata {
    pub family: String,
    pub subfamily: Option<String>,
    pub weight: Option<u16>,
    pub weight_range: Option<FontWeightRange>,
    pub style: String,
    pub axes: Vec<FontVariationAxis>,
    pub license: FontLicenseMetadata,
    pub romanian_glyphs: Vec<char>,
}

pub fn parse_font_metadata(bytes: &[u8]) -> Result<ParsedFontMetadata, String> {
    let scope = ReadScope::new(bytes);
    let font_data = scope
        .read::<FontData<'_>>()
        .map_err(|error| format!("container OpenType invalid ({error})"))?;
    let provider = font_data
        .table_provider(0)
        .map_err(|error| format!("fontul nu expune primul face ({error})"))?;
    let name_data = provider
        .read_table_data(tag::NAME)
        .map_err(|error| format!("tabela name lipsește sau este invalidă ({error})"))?;
    let name_table = ReadScope::new(&name_data)
        .read::<NameTable<'_>>()
        .map_err(|error| format!("tabela name este invalidă ({error})"))?;
    let family = clean_name(
        name_table
            .string_for_id(NameTable::TYPOGRAPHIC_FAMILY_NAME)
            .or_else(|| name_table.string_for_id(NameTable::WWS_FAMILY_NAME))
            .or_else(|| name_table.string_for_id(NameTable::FONT_FAMILY_NAME)),
    )
    .ok_or_else(|| "fontul nu declară o familie internă utilizabilă".to_string())?;
    let subfamily = clean_name(
        name_table
            .string_for_id(NameTable::TYPOGRAPHIC_SUBFAMILY_NAME)
            .or_else(|| name_table.string_for_id(NameTable::WWS_SUBFAMILY_NAME))
            .or_else(|| name_table.string_for_id(NameTable::FONT_SUBFAMILY_NAME)),
    );

    let os2 = provider
        .table_data(tag::OS_2)
        .map_err(|error| format!("tabela OS/2 nu poate fi citită ({error})"))?
        .map(|data| {
            ReadScope::new(&data)
                .read_dep::<Os2>(data.len())
                .map_err(|error| format!("tabela OS/2 este invalidă ({error})"))
        })
        .transpose()?;
    let axes = provider
        .table_data(tag::FVAR)
        .map_err(|error| format!("tabela fvar nu poate fi citită ({error})"))?
        .map(|data| parse_variation_axes(&data))
        .transpose()?
        .unwrap_or_default();
    let weight_axis = axes
        .iter()
        .find(|axis| axis.tag.eq_ignore_ascii_case("wght"));
    let weight_range = weight_axis.map(|axis| FontWeightRange {
        start: font_weight_from_axis(axis.min),
        end: font_weight_from_axis(axis.max),
    });
    let weight = if weight_range.is_some() {
        None
    } else {
        Some(
            os2.as_ref()
                .map(|table| table.us_weight_class.clamp(1, 1000))
                .unwrap_or(400),
        )
    };
    let lower_subfamily = subfamily
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let style = if os2
        .as_ref()
        .is_some_and(|table| table.fs_selection.contains(FsSelectionFlag::OBLIQUE))
        || lower_subfamily.contains("oblique")
    {
        "oblique"
    } else if os2
        .as_ref()
        .is_some_and(|table| table.fs_selection.contains(FsSelectionFlag::ITALIC))
        || lower_subfamily.contains("italic")
    {
        "italic"
    } else {
        "normal"
    }
    .to_string();
    let romanian_glyphs = provider
        .table_data(tag::CMAP)
        .ok()
        .flatten()
        .map(|data| romanian_glyph_coverage(&data))
        .unwrap_or_default();

    Ok(ParsedFontMetadata {
        family,
        subfamily,
        weight,
        weight_range,
        style,
        axes,
        license: FontLicenseMetadata {
            description: clean_name(name_table.string_for_id(NameTable::LICENSE_DESCRIPTION)),
            url: clean_name(name_table.string_for_id(NameTable::LICENSE_INFO_URL)),
        },
        romanian_glyphs,
    })
}

pub fn validate_font_signature(
    bytes: &[u8],
    extension: &str,
    source_path: &str,
) -> Result<(), String> {
    let magic = bytes
        .get(..4)
        .ok_or_else(|| format!("Fontul local {source_path} este prea scurt."))?;
    let matches_extension = match extension {
        "woff2" => magic == b"wOF2",
        "woff" => magic == b"wOFF",
        "ttf" => magic == [0, 1, 0, 0] || magic == b"true",
        "otf" => magic == b"OTTO",
        _ => false,
    };
    if matches_extension {
        Ok(())
    } else {
        Err(format!(
            "Extensia .{extension} nu corespunde semnăturii binare a fontului {source_path}."
        ))
    }
}

fn romanian_glyph_coverage(bytes: &[u8]) -> Vec<char> {
    let Some(subtable) = ReadScope::new(bytes)
        .read::<Cmap<'_>>()
        .ok()
        .and_then(|cmap| read_cmap_subtable(&cmap).ok().flatten())
        .map(|(_, subtable)| subtable)
    else {
        return Vec::new();
    };
    ROMANIAN_GLYPHS
        .into_iter()
        .filter(|character| {
            subtable
                .map_glyph(*character as u32)
                .ok()
                .flatten()
                .is_some_and(|glyph| glyph != 0)
        })
        .collect()
}

fn parse_variation_axes(bytes: &[u8]) -> Result<Vec<FontVariationAxis>, String> {
    let table = ReadScope::new(bytes)
        .read::<FvarTable<'_>>()
        .map_err(|error| format!("tabela fvar este invalidă ({error})"))?;
    Ok(table
        .axes()
        .map(|axis| FontVariationAxis {
            tag: String::from_utf8_lossy(&axis.axis_tag.to_be_bytes()).into_owned(),
            min: fixed_to_f64(axis.min_value.raw_value()),
            default: fixed_to_f64(axis.default_value.raw_value()),
            max: fixed_to_f64(axis.max_value.raw_value()),
        })
        .collect())
}

fn fixed_to_f64(raw: i32) -> f64 {
    f64::from(raw) / 65_536.0
}

fn font_weight_from_axis(value: f64) -> u16 {
    value.round().clamp(1.0, 1000.0) as u16
}

fn clean_name(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().replace('\0', ""))
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTER_VARIABLE: &[u8] = include_bytes!(
        "../../resources/project-starters/cadru/project/static/fonturi/inter-400-700-latin-ext.woff2"
    );

    #[test]
    fn reads_internal_woff2_family_weight_range_and_axes() {
        validate_font_signature(INTER_VARIABLE, "woff2", "inter.woff2").unwrap();
        let metadata = parse_font_metadata(INTER_VARIABLE).unwrap();

        assert_eq!(metadata.family, "Inter");
        assert_eq!(
            metadata.weight_range,
            Some(FontWeightRange {
                start: 100,
                end: 900
            })
        );
        assert!(metadata
            .axes
            .iter()
            .any(|axis| axis.tag == "wght" && axis.min == 100.0 && axis.max == 900.0));
        assert!(metadata.romanian_glyphs.contains(&'ș'));
        assert!(!metadata.romanian_glyphs.contains(&'â'));
    }

    #[test]
    fn rejects_extension_signature_mismatch_before_parser() {
        let error = validate_font_signature(INTER_VARIABLE, "ttf", "inter.ttf").unwrap_err();
        assert!(error.contains("nu corespunde semnăturii binare"));
    }
}
