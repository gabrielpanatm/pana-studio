use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::kernel::{
    file_buffer_store::hash_bytes,
    project_workspace::{
        PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES,
        PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES,
    },
};

use super::metadata::{parse_font_metadata, validate_font_signature};
use super::{
    font_format_label, font_weight_file_segment, normalize_font_family_name, render_font_face_css,
    slugify_family, FontCssRegistration, FontDeliveryKind, FontFaceFamily, FontLicenseMetadata,
    FontOrigin, FontOwnership, FontVariationAxis, FontWeightRange, LocalFontFile, ROMANIAN_GLYPHS,
};

pub const LOCAL_FONT_IMPORT_SCHEMA_VERSION: u32 = 2;
const LOCAL_FONT_IMPORT_MAX_FILES: usize = 24;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFontImportFilePlan {
    pub source_path: String,
    pub destination_path: String,
    pub family: String,
    pub subfamily: Option<String>,
    pub size_bytes: u64,
    pub extension: String,
    pub format: String,
    pub weight: Option<u16>,
    pub weight_range: Option<FontWeightRange>,
    pub style: String,
    pub axes: Vec<FontVariationAxis>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFontImportFamilyPlan {
    pub id: String,
    pub family: String,
    pub directory: String,
    pub file_count: usize,
    pub variable: bool,
    pub license: FontLicenseMetadata,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFontImportPlan {
    pub schema_version: u32,
    pub plan_token: String,
    pub stylesheet_path: String,
    pub families: Vec<LocalFontImportFamilyPlan>,
    pub files: Vec<LocalFontImportFilePlan>,
    pub warnings: Vec<String>,
    pub conflicts: Vec<String>,
    pub changed: bool,
}

#[derive(Clone, Debug)]
pub struct LocalFontImportPreparedFile {
    pub plan: LocalFontImportFilePlan,
    pub bytes: Vec<u8>,
    pub content_hash: String,
}

#[derive(Clone, Debug)]
pub struct LocalFontImportPreparedFamily {
    pub family: FontFaceFamily,
    pub font_face_css: String,
}

#[derive(Clone, Debug)]
pub struct LocalFontImportPrepared {
    pub files: Vec<LocalFontImportPreparedFile>,
    pub families: Vec<LocalFontImportPreparedFamily>,
    pub warnings: Vec<String>,
    pub conflicts: Vec<String>,
}

pub fn prepare_local_font_import(
    source_paths: Vec<String>,
) -> Result<LocalFontImportPrepared, String> {
    if source_paths.is_empty() {
        return Err("Alege cel puțin un fișier de font.".to_string());
    }
    if source_paths.len() > LOCAL_FONT_IMPORT_MAX_FILES {
        return Err(format!(
            "Importul local acceptă cel mult {LOCAL_FONT_IMPORT_MAX_FILES} fișiere într-o acțiune."
        ));
    }

    let mut source_paths = source_paths;
    source_paths.sort();
    source_paths.dedup();

    let mut total_bytes = 0u64;
    let mut warnings = Vec::new();
    let mut conflicts = Vec::new();
    let mut seen_hashes = HashMap::<String, String>::new();
    let mut seen_variants = HashMap::<String, (String, String)>::new();
    let mut prepared_files = Vec::new();

    for source_path in source_paths {
        let path = PathBuf::from(&source_path);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Nu am putut inspecta fontul local {source_path}: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Font Manager a refuzat {source_path}: sursa este symlink."
            ));
        }
        if !metadata.is_file() {
            return Err(format!(
                "Font Manager a refuzat {source_path}: sursa nu este un fișier obișnuit."
            ));
        }
        if metadata.len() == 0 {
            return Err(format!("Fontul local {source_path} este gol."));
        }
        if metadata.len() > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES {
            return Err(format!(
                "Fontul local {source_path} depășește limita de {} MB per fișier.",
                PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES / 1024 / 1024
            ));
        }
        total_bytes = total_bytes
            .checked_add(metadata.len())
            .ok_or_else(|| "Dimensiunea importului local a depășit limita numerică.".to_string())?;
        if total_bytes > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES {
            return Err(format!(
                "Importul local depășește limita de {} MB pentru resursele binare ale sesiunii.",
                PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_TOTAL_BYTES / 1024 / 1024
            ));
        }

        let extension = supported_extension(&path)?;
        let bytes = fs::read(&path)
            .map_err(|error| format!("Nu am putut citi fontul local {source_path}: {error}"))?;
        validate_font_signature(&bytes, &extension, &source_path)?;
        let font_metadata = parse_font_metadata(&bytes)
            .map_err(|error| format!("Fontul local {source_path} nu este valid: {error}"))?;
        let content_hash = hash_bytes(&bytes);

        if let Some(first_path) = seen_hashes.get(&content_hash) {
            warnings.push(format!(
                "{source_path} este identic cu {first_path} și nu va fi importat de două ori."
            ));
            continue;
        }
        seen_hashes.insert(content_hash.clone(), source_path.clone());

        let family_slug = slugify_family(&font_metadata.family);
        let directory = format!("static/fonturi/{family_slug}");
        let weight_segment =
            font_weight_file_segment(font_metadata.weight, font_metadata.weight_range);
        let hash_segment = &content_hash[..content_hash.len().min(8)];
        let file_name = format!(
            "{}-{}-{}-{}.{}",
            family_slug,
            slugify_family(&font_metadata.style),
            weight_segment,
            hash_segment,
            extension
        );
        let destination_path = format!("{directory}/{file_name}");
        let variant_key = format!(
            "{}|{}|{}",
            normalize_family_key(&font_metadata.family),
            font_metadata.style,
            weight_segment
        );
        if let Some((first_hash, first_path)) = seen_variants.get(&variant_key) {
            if first_hash != &content_hash {
                conflicts.push(format!(
                    "{} și {} declară aceeași variantă internă {} / {} / {}, dar au conținut diferit.",
                    first_path,
                    source_path,
                    font_metadata.family,
                    font_metadata.style,
                    weight_segment
                ));
            }
        } else {
            seen_variants.insert(variant_key, (content_hash.clone(), source_path.clone()));
        }

        prepared_files.push(LocalFontImportPreparedFile {
            plan: LocalFontImportFilePlan {
                source_path,
                destination_path,
                family: font_metadata.family,
                subfamily: font_metadata.subfamily,
                size_bytes: bytes.len() as u64,
                extension: extension.clone(),
                format: font_format_label(&extension).to_string(),
                weight: font_metadata.weight,
                weight_range: font_metadata.weight_range,
                style: font_metadata.style,
                axes: font_metadata.axes,
            },
            bytes,
            content_hash,
        });
    }

    if prepared_files.is_empty() {
        return Err(
            "Fișierele alese sunt duplicate exacte; nu există nimic de importat.".to_string(),
        );
    }

    let mut family_groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, file) in prepared_files.iter().enumerate() {
        family_groups
            .entry(normalize_family_key(&file.plan.family))
            .or_default()
            .push(index);
    }

    let mut families = Vec::new();
    for indexes in family_groups.values() {
        let first = &prepared_files[indexes[0]];
        let family_name = first.plan.family.clone();
        let directory = first
            .plan
            .destination_path
            .rsplit_once('/')
            .map(|(directory, _)| directory.to_string())
            .unwrap_or_else(|| format!("static/fonturi/{}", slugify_family(&family_name)));
        let mut files = Vec::new();
        let mut css_rules = Vec::new();
        let mut license = FontLicenseMetadata::default();

        for index in indexes {
            let prepared = &prepared_files[*index];
            let file_name = Path::new(&prepared.plan.destination_path)
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default();
            let metadata = parse_font_metadata(&prepared.bytes)?;
            if license.is_empty() && !metadata.license.is_empty() {
                license = metadata.license.clone();
            }
            files.push(LocalFontFile {
                file: prepared.plan.destination_path.clone(),
                file_name: file_name.clone(),
                size_bytes: prepared.plan.size_bytes,
                extension: prepared.plan.extension.clone(),
                format: prepared.plan.format.clone(),
                text_optimized: false,
                content_hash: prepared.content_hash.clone(),
                internal_family: Some(prepared.plan.family.clone()),
                subfamily: prepared.plan.subfamily.clone(),
                weight: prepared.plan.weight,
                weight_range: prepared.plan.weight_range,
                style: Some(prepared.plan.style.clone()),
                axes: prepared.plan.axes.clone(),
                license: metadata.license,
                unicode_range: None,
                romanian_glyphs: metadata.romanian_glyphs,
                declared_weight: prepared.plan.weight,
                declared_weight_range: prepared.plan.weight_range,
                declared_style: Some(prepared.plan.style.clone()),
                preload: super::FontPreloadRegistration::default(),
            });
            css_rules.push(local_font_face_css(&prepared.plan, &file_name));
        }

        files.sort_by(|left, right| {
            left.weight_range
                .map(|range| range.start)
                .or(left.weight)
                .unwrap_or(400)
                .cmp(
                    &right
                        .weight_range
                        .map(|range| range.start)
                        .or(right.weight)
                        .unwrap_or(400),
                )
                .then_with(|| left.style.cmp(&right.style))
        });
        families.push(LocalFontImportPreparedFamily {
            family: FontFaceFamily {
                id: format!("css:{}", normalize_family_key(&family_name)),
                family: family_name,
                directories: vec![directory],
                origin: FontOrigin::Local,
                theme_name: None,
                delivery: FontDeliveryKind::Local,
                ownership: FontOwnership::Managed,
                romanian_supported: Some(ROMANIAN_GLYPHS.iter().all(|required| {
                    files
                        .iter()
                        .any(|file| file.romanian_glyphs.contains(required))
                })),
                files,
                faces: Vec::new(),
                issues: Vec::new(),
                license,
                registration: FontCssRegistration::default(),
            },
            font_face_css: css_rules.join("\n\n"),
        });
    }

    Ok(LocalFontImportPrepared {
        files: prepared_files,
        families,
        warnings,
        conflicts,
    })
}

fn supported_extension(path: &Path) -> Result<String, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(extension.as_str(), "woff2" | "woff" | "ttf" | "otf") {
        Ok(extension)
    } else {
        Err(format!(
            "Font Manager acceptă doar WOFF2, WOFF, TTF și OTF: {}.",
            path.to_string_lossy()
        ))
    }
}

fn local_font_face_css(plan: &LocalFontImportFilePlan, file_name: &str) -> String {
    let family_slug = slugify_family(&plan.family);
    let public_url = format!("/fonturi/{family_slug}/{file_name}");
    render_font_face_css(
        &plan.family,
        &plan.style,
        plan.weight,
        plan.weight_range,
        &public_url,
        &plan.format,
        None,
    )
}

fn normalize_family_key(family: &str) -> String {
    normalize_font_family_name(family)
}
