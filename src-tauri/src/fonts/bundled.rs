use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::Serialize;

use super::{
    render_font_face_css, FontCssRegistration, FontDeliveryKind, FontFaceFamily,
    FontLicenseMetadata, FontOrigin, FontOwnership, FontPreloadRegistration, FontVariationAxis,
    FontWeightRange, LocalFontFile, ROMANIAN_GLYPHS,
};

pub struct EmbeddedFontLicense {
    pub identifier: &'static str,
    pub relative_path: &'static str,
    pub source_url: &'static str,
    pub text: &'static str,
}

pub struct EmbeddedFontFile {
    pub relative_path: &'static str,
    pub file_name: &'static str,
    pub subset: &'static str,
    pub unicode_range: &'static str,
    pub source_url: &'static str,
    pub sha256: &'static str,
    pub size_bytes: u64,
    pub style: &'static str,
    pub weight_start: u16,
    pub weight_end: u16,
    pub romanian_glyphs: &'static [char],
    pub bytes: &'static [u8],
}

pub struct EmbeddedFontFamily {
    pub id: &'static str,
    pub family: &'static str,
    pub category: &'static str,
    pub last_modified: &'static str,
    pub specimen_url: &'static str,
    pub css_url: &'static str,
    pub license: EmbeddedFontLicense,
    pub files: &'static [EmbeddedFontFile],
}

include!(concat!(env!("OUT_DIR"), "/pana-studio-font-library.rs"));

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledFontCatalogFamily {
    pub id: String,
    pub family: String,
    pub category: String,
    pub last_modified: String,
    pub specimen_url: String,
    pub css_url: String,
    pub styles: Vec<String>,
    pub weight_range: FontWeightRange,
    pub file_count: usize,
    pub size_bytes: u64,
    pub variable: bool,
    pub romanian_supported: bool,
    pub license: FontLicenseMetadata,
    pub license_file: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledFontPreviewFace {
    pub library_path: String,
    pub subset: String,
    pub unicode_range: String,
    pub style: String,
    pub weight_range: FontWeightRange,
    pub format: String,
    pub data_url: String,
    pub content_hash: String,
    pub source_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundledFontPreview {
    pub family_id: String,
    pub family: String,
    pub faces: Vec<BundledFontPreviewFace>,
}

pub struct BundledFontInstallPlan {
    pub family: FontFaceFamily,
    pub font_face_css: String,
    pub license_file: String,
    pub license_source_url: String,
    pub license_text: String,
    pub writes: Vec<BundledFontInstallWrite>,
}

pub struct BundledFontInstallWrite {
    pub project_relative_path: String,
    pub bytes: Vec<u8>,
}

pub fn bundled_font_catalog() -> Vec<BundledFontCatalogFamily> {
    EMBEDDED_FONT_LIBRARY
        .iter()
        .map(bundled_catalog_family)
        .collect()
}

pub fn embedded_font_family(id: &str) -> Option<&'static EmbeddedFontFamily> {
    let id = id.trim();
    EMBEDDED_FONT_LIBRARY.iter().find(|family| family.id == id)
}

pub fn bundled_font_preview(
    id: &str,
    requested_style: Option<&str>,
) -> Result<BundledFontPreview, String> {
    let embedded = embedded_font_family(id)
        .ok_or_else(|| format!("Biblioteca inclusă nu conține familia cu ID-ul {id}."))?;
    let requested_style = requested_style
        .map(str::trim)
        .filter(|style| !style.is_empty())
        .unwrap_or("normal");
    if !matches!(requested_style, "normal" | "italic") {
        return Err(format!(
            "Stilul {requested_style} nu este acceptat pentru previzualizarea fonturilor incluse."
        ));
    }
    let faces = embedded
        .files
        .iter()
        .filter(|file| file.style == requested_style)
        .map(|file| BundledFontPreviewFace {
            library_path: file.relative_path.to_string(),
            subset: file.subset.to_string(),
            unicode_range: file.unicode_range.to_string(),
            style: file.style.to_string(),
            weight_range: FontWeightRange {
                start: file.weight_start,
                end: file.weight_end,
            },
            format: "woff2".to_string(),
            data_url: format!(
                "data:font/woff2;base64,{}",
                BASE64_STANDARD.encode(file.bytes)
            ),
            content_hash: file.sha256.to_string(),
            source_url: file.source_url.to_string(),
        })
        .collect::<Vec<_>>();
    if faces.is_empty() {
        return Err(format!(
            "Familia {} nu include stilul {requested_style}.",
            embedded.family
        ));
    }
    Ok(BundledFontPreview {
        family_id: embedded.id.to_string(),
        family: embedded.family.to_string(),
        faces,
    })
}

pub fn prepare_bundled_font_install(id: &str) -> Result<BundledFontInstallPlan, String> {
    let embedded = embedded_font_family(id)
        .ok_or_else(|| format!("Biblioteca inclusă nu conține familia cu ID-ul {id}."))?;
    let directory = format!("static/fonturi/{}", embedded.id);
    let license = FontLicenseMetadata {
        description: Some(embedded.license.identifier.to_string()),
        url: Some(embedded.license.source_url.to_string()),
    };
    let mut files = Vec::with_capacity(embedded.files.len());
    let mut css = Vec::with_capacity(embedded.files.len());
    let mut writes = Vec::with_capacity(embedded.files.len());

    for file in embedded.files {
        let destination = format!("{directory}/{}", file.file_name);
        let public_url = format!("/fonturi/{}/{}", embedded.id, file.file_name);
        let weight_range = FontWeightRange {
            start: file.weight_start,
            end: file.weight_end,
        };
        let axes = vec![FontVariationAxis {
            tag: "wght".to_string(),
            min: f64::from(file.weight_start),
            default: f64::from(400u16.clamp(file.weight_start, file.weight_end)),
            max: f64::from(file.weight_end),
        }];
        files.push(LocalFontFile {
            file: destination.clone(),
            file_name: file.file_name.to_string(),
            size_bytes: file.size_bytes,
            extension: "woff2".to_string(),
            format: "woff2".to_string(),
            text_optimized: false,
            content_hash: file.sha256.to_string(),
            internal_family: Some(embedded.family.to_string()),
            subfamily: Some(file.style.to_string()),
            weight: None,
            weight_range: Some(weight_range),
            style: Some(file.style.to_string()),
            axes,
            license: license.clone(),
            unicode_range: Some(file.unicode_range.to_string()),
            romanian_glyphs: file.romanian_glyphs.to_vec(),
            declared_weight: None,
            declared_weight_range: Some(weight_range),
            declared_style: Some(file.style.to_string()),
            preload: FontPreloadRegistration::default(),
        });
        css.push(render_font_face_css(
            embedded.family,
            file.style,
            None,
            Some(weight_range),
            &public_url,
            "woff2",
            Some(file.unicode_range),
        ));
        writes.push(BundledFontInstallWrite {
            project_relative_path: destination,
            bytes: file.bytes.to_vec(),
        });
    }

    Ok(BundledFontInstallPlan {
        family: FontFaceFamily {
            id: format!("css:{}", super::normalize_font_family_name(embedded.family)),
            family: embedded.family.to_string(),
            directories: vec![directory.clone()],
            origin: FontOrigin::Bundled,
            theme_name: None,
            delivery: FontDeliveryKind::Local,
            ownership: FontOwnership::Managed,
            romanian_supported: Some(ROMANIAN_GLYPHS.iter().all(|glyph| {
                embedded
                    .files
                    .iter()
                    .any(|file| file.romanian_glyphs.contains(glyph))
            })),
            files,
            faces: Vec::new(),
            issues: Vec::new(),
            license,
            registration: FontCssRegistration::default(),
        },
        font_face_css: css.join("\n\n"),
        license_file: format!("{directory}/LICENSE.txt"),
        license_source_url: embedded.license.source_url.to_string(),
        license_text: embedded.license.text.to_string(),
        writes,
    })
}

fn bundled_catalog_family(family: &EmbeddedFontFamily) -> BundledFontCatalogFamily {
    let mut styles = family
        .files
        .iter()
        .map(|file| file.style.to_string())
        .collect::<Vec<_>>();
    styles.sort();
    styles.dedup();
    let weight_start = family
        .files
        .iter()
        .map(|file| file.weight_start)
        .min()
        .unwrap_or(400);
    let weight_end = family
        .files
        .iter()
        .map(|file| file.weight_end)
        .max()
        .unwrap_or(400);
    BundledFontCatalogFamily {
        id: family.id.to_string(),
        family: family.family.to_string(),
        category: family.category.to_string(),
        last_modified: family.last_modified.to_string(),
        specimen_url: family.specimen_url.to_string(),
        css_url: family.css_url.to_string(),
        styles,
        weight_range: FontWeightRange {
            start: weight_start,
            end: weight_end,
        },
        file_count: family.files.len(),
        size_bytes: family.files.iter().map(|file| file.size_bytes).sum(),
        variable: family
            .files
            .iter()
            .all(|file| file.weight_start < file.weight_end),
        romanian_supported: ROMANIAN_GLYPHS.iter().all(|glyph| {
            family
                .files
                .iter()
                .any(|file| file.romanian_glyphs.contains(glyph))
        }),
        license: FontLicenseMetadata {
            description: Some(family.license.identifier.to_string()),
            url: Some(family.license.source_url.to_string()),
        },
        license_file: family.license.relative_path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_complete_and_within_budget() {
        let catalog = bundled_font_catalog();
        assert_eq!(catalog.len(), 36);
        assert!(catalog.iter().all(|family| family.variable));
        assert!(catalog.iter().all(|family| family.romanian_supported));
        assert!(catalog.iter().map(|family| family.size_bytes).sum::<u64>() <= 5_500 * 1_024);
    }

    #[test]
    fn bundled_install_is_project_portable() {
        let plan = prepare_bundled_font_install("inter").unwrap();
        assert_eq!(plan.family.origin, FontOrigin::Bundled);
        assert!(plan.writes.iter().all(|write| write
            .project_relative_path
            .starts_with("static/fonturi/inter/")));
        assert!(plan.font_face_css.contains("/fonturi/inter/"));
        assert!(!plan.font_face_css.contains("asset:"));
        assert!(!plan.font_face_css.contains("fonts.gstatic.com"));
        assert!(plan.font_face_css.contains("font-display: swap"));
        assert!(plan.font_face_css.contains("font-weight: 100 900"));
        assert!(plan.font_face_css.contains("unicode-range:"));
        assert_eq!(
            plan.font_face_css.matches("@font-face").count(),
            plan.writes.len()
        );
        assert!(!plan.license_text.trim().is_empty());
    }

    #[test]
    fn bundled_catalog_rejects_arbitrary_ids_and_every_write_is_scoped() {
        assert!(prepare_bundled_font_install("../inter").is_err());
        assert!(prepare_bundled_font_install("INTER").is_err());
        for family in EMBEDDED_FONT_LIBRARY {
            let plan = prepare_bundled_font_install(family.id).unwrap();
            let prefix = format!("static/fonturi/{}/", family.id);
            let mut paths = std::collections::BTreeSet::new();
            for write in &plan.writes {
                assert!(write.project_relative_path.starts_with(&prefix));
                assert!(!write.project_relative_path.contains(".."));
                assert!(paths.insert(&write.project_relative_path));
            }
            assert!(plan.license_file.starts_with(&prefix));
            assert!(!plan.font_face_css.contains("https://"));
            assert!(!plan.font_face_css.contains("file://"));
        }
    }

    #[test]
    fn preview_transfers_only_the_selected_family_faces() {
        let preview = bundled_font_preview("source-serif-4", Some("normal")).unwrap();
        assert_eq!(preview.family_id, "source-serif-4");
        assert_eq!(preview.faces.len(), 2);
        assert!(preview.faces.iter().all(|face| face.style == "normal"));
        assert!(preview
            .faces
            .iter()
            .all(|face| face.data_url.starts_with("data:font/woff2;base64,")));
        assert!(bundled_font_preview("source-serif-4", Some("oblique")).is_err());
    }
}
