mod delivery;
mod graph;
mod local_import;
mod roles;

pub use delivery::{
    annotate_font_preloads, font_delivery_diagnostics, prepare_font_display_update,
    prepare_font_preload_update, select_font_preload_template, FontDeliveryDiagnostic,
    FontDisplayMode, FontPreloadRegistration,
};
pub use graph::build_font_face_graph;
pub use local_import::{
    prepare_local_font_import, FontLicenseMetadata, FontVariationAxis, LocalFontImportFamilyPlan,
    LocalFontImportPlan, LocalFontImportPrepared, LOCAL_FONT_IMPORT_SCHEMA_VERSION,
};
pub use roles::{prepare_font_role_assignment, read_font_roles, FontRoleAssignment, FontRoleId};

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    env,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use crate::{kernel::file_buffer_store::hash_bytes, zola_theme::ZolaThemeResolver};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE};

const GOOGLE_FONTS_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
pub(super) const ROMANIAN_GLYPHS: [char; 10] = ['ă', 'â', 'î', 'ș', 'ț', 'Ă', 'Â', 'Î', 'Ș', 'Ț'];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFaceGraph {
    pub schema_version: u32,
    pub roots: Vec<FontRoot>,
    pub families: Vec<FontFaceFamily>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontCssRegistration {
    pub registered: bool,
    pub managed: bool,
    pub stylesheets: Vec<String>,
    pub display_modes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FontDeliveryKind {
    Local,
    System,
    External,
    Missing,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FontOwnership {
    Managed,
    Detected,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FontFaceIssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFaceIssue {
    pub code: String,
    pub severity: FontFaceIssueSeverity,
    pub message: String,
    pub file: Option<String>,
    pub stylesheet: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFaceSource {
    pub stylesheet: String,
    pub url: String,
    pub resolved_file: Option<String>,
    pub delivery: FontDeliveryKind,
    pub ownership: FontOwnership,
    pub external: bool,
    pub dynamic: bool,
    pub weight: Option<u16>,
    pub weight_range: Option<FontWeightRange>,
    pub style: String,
    pub display: Option<String>,
    pub unicode_range: Option<String>,
    pub managed: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFontDownloadResult {
    pub family: FontFaceFamily,
    pub font_face_css: String,
    pub css_url: String,
    pub license_file: String,
    pub license_source_url: String,
    pub variable: bool,
    pub text_optimized: bool,
    pub optimized_character_count: usize,
}

#[derive(Clone, Debug)]
pub struct GoogleFontDownloadPlan {
    pub result: GoogleFontDownloadResult,
    pub writes: Vec<GoogleFontDownloadFileWrite>,
    pub license_text: String,
}

#[derive(Clone, Debug)]
pub struct GoogleFontDownloadFileWrite {
    pub project_relative_path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFontCatalogFamily {
    pub family: String,
    pub category: Option<String>,
    pub variants: Vec<String>,
    pub weights: Vec<u16>,
    pub subsets: Vec<String>,
    pub axes: Vec<GoogleFontAxis>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFontAxis {
    pub tag: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontRoot {
    pub relative_path: String,
    pub origin: FontOrigin,
    pub theme_name: Option<String>,
    pub exists: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFaceFamily {
    pub id: String,
    pub family: String,
    pub directories: Vec<String>,
    pub origin: FontOrigin,
    pub theme_name: Option<String>,
    pub delivery: FontDeliveryKind,
    pub ownership: FontOwnership,
    pub romanian_supported: Option<bool>,
    pub files: Vec<LocalFontFile>,
    pub faces: Vec<FontFaceSource>,
    pub issues: Vec<FontFaceIssue>,
    pub license: FontLicenseMetadata,
    pub registration: FontCssRegistration,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFontFile {
    pub file: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub extension: String,
    pub format: String,
    pub text_optimized: bool,
    pub content_hash: String,
    pub internal_family: Option<String>,
    pub subfamily: Option<String>,
    pub weight: Option<u16>,
    pub weight_range: Option<FontWeightRange>,
    pub style: Option<String>,
    pub axes: Vec<FontVariationAxis>,
    pub license: FontLicenseMetadata,
    pub unicode_range: Option<String>,
    pub romanian_glyphs: Vec<char>,
    pub declared_weight: Option<u16>,
    pub declared_weight_range: Option<FontWeightRange>,
    pub declared_style: Option<String>,
    pub preload: FontPreloadRegistration,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontWeightRange {
    pub start: u16,
    pub end: u16,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum FontOrigin {
    Local,
    Theme,
    External,
}

#[derive(Clone, Debug)]
pub(super) struct FontRootCandidate {
    pub(super) absolute_path: PathBuf,
    pub(super) relative_path: String,
    pub(super) origin: FontOrigin,
    pub(super) theme_name: Option<String>,
}

static GOOGLE_FONT_CATALOG: OnceLock<Mutex<Option<Vec<GoogleFontCatalogFamily>>>> = OnceLock::new();

#[cfg(test)]
fn font_family_registration_count(source: &str, family: &str) -> usize {
    let normalized_family = normalize_font_family_name(family);
    css_font_face_blocks(source)
        .into_iter()
        .filter(|block| {
            parse_css_declarations(block)
                .get("font-family")
                .is_some_and(|value| normalize_font_family_name(value) == normalized_family)
        })
        .count()
}

pub fn select_font_face_stylesheet<'a>(
    sources: impl Iterator<Item = (&'a str, &'a str)>,
) -> Option<String> {
    sources
        .filter(|(path, _)| {
            is_stylesheet_path(path) && !path.starts_with("public/") && !path.contains("/public/")
        })
        .filter_map(|(path, source)| {
            let normalized = path.replace('\\', "/");
            let score = if source.contains("/* pana-studio-font:") {
                0
            } else if normalized.ends_with("sass/css-framework/_baza.scss") {
                10
            } else if normalized.ends_with("/_baza.scss") || normalized == "_baza.scss" {
                20
            } else if source.contains("@font-face") {
                30
            } else {
                return None;
            };
            Some((score, normalized.len(), normalized))
        })
        .min()
        .map(|(_, _, path)| path)
}

pub fn upsert_managed_font_face_block(
    source: &str,
    family: &str,
    font_face_css: &str,
) -> Result<String, String> {
    let start = managed_font_start_marker(family);
    let end = managed_font_end_marker(family);
    let block = format!("{start}\n{}\n{end}", font_face_css.trim());

    if let Some(start_index) = source.find(&start) {
        let end_search_start = start_index + start.len();
        let Some(relative_end) = source[end_search_start..].find(&end) else {
            return Err(format!(
                "Blocul @font-face administrat pentru {family} are marker de început fără marker de sfârșit."
            ));
        };
        let end_index = end_search_start + relative_end + end.len();
        return Ok(format!(
            "{}{}{}",
            &source[..start_index],
            block,
            &source[end_index..]
        ));
    }
    if source.contains(&end) {
        return Err(format!(
            "Blocul @font-face administrat pentru {family} are marker de sfârșit fără marker de început."
        ));
    }

    let trimmed = source.trim_end();
    if trimmed.is_empty() {
        Ok(format!("{block}\n"))
    } else {
        Ok(format!("{trimmed}\n\n{block}\n"))
    }
}

pub fn remove_managed_font_face_block(source: &str, family: &str) -> Result<String, String> {
    let start = managed_font_start_marker(family);
    let end = managed_font_end_marker(family);
    let Some(start_index) = source.find(&start) else {
        if source.contains(&end) {
            return Err(format!(
                "Blocul @font-face administrat pentru {family} are marker de sfârșit fără marker de început."
            ));
        }
        return Err(format!(
            "Blocul @font-face administrat pentru {family} nu mai există."
        ));
    };
    let end_search_start = start_index + start.len();
    let Some(relative_end) = source[end_search_start..].find(&end) else {
        return Err(format!(
            "Blocul @font-face administrat pentru {family} are marker de început fără marker de sfârșit."
        ));
    };
    let end_index = end_search_start + relative_end + end.len();
    let before = source[..start_index].trim_end();
    let after = source[end_index..].trim_start_matches(['\r', '\n']);

    match (before.is_empty(), after.is_empty()) {
        (true, true) => Ok(String::new()),
        (true, false) => Ok(after.to_string()),
        (false, true) => Ok(format!("{before}\n")),
        (false, false) => Ok(format!("{before}\n\n{after}")),
    }
}

pub fn search_google_fonts(
    query: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<GoogleFontCatalogFamily>, String> {
    let catalog = google_font_catalog()?;
    Ok(filter_google_font_catalog(&catalog, query, limit, offset))
}

fn filter_google_font_catalog(
    catalog: &[GoogleFontCatalogFamily],
    query: &str,
    limit: usize,
    offset: usize,
) -> Vec<GoogleFontCatalogFamily> {
    let query = query.trim().to_ascii_lowercase();
    let limit = limit.clamp(1, 120);
    let mut matches = Vec::new();
    let mut skipped = 0usize;

    for family in catalog {
        if !query.is_empty() && !family.family.to_ascii_lowercase().contains(&query) {
            continue;
        }

        if skipped < offset {
            skipped += 1;
            continue;
        }

        matches.push(family.clone());
        if matches.len() >= limit {
            break;
        }
    }

    matches
}

pub fn plan_google_font_family_download(
    family: &str,
    weights: &[u16],
    styles: &[String],
    variable: bool,
    axes: &[GoogleFontAxis],
    character_set: Option<&str>,
) -> Result<GoogleFontDownloadPlan, String> {
    let family = family.trim();
    if family.is_empty() {
        return Err("Numele familiei Google Fonts este gol.".to_string());
    }

    let variable_range = if variable {
        Some(normalized_variable_weight_range(weights))
    } else {
        None
    };
    let weights = if let Some(range) = variable_range {
        vec![range.start, range.end]
    } else {
        normalized_weights(weights)
    };
    let styles = normalized_google_font_styles(styles);
    let advanced_axes = normalized_google_font_axes(family, axes)?;
    let character_set = normalize_google_character_set(character_set)?;
    let css_url = google_fonts_css_url(
        family,
        &weights,
        &styles,
        variable,
        &advanced_axes,
        character_set.as_deref(),
    );
    let client = google_fonts_client()?;
    let license = fetch_google_font_license(&client, family)?;
    let css = client
        .get(&css_url)
        .header(ACCEPT, "text/css,*/*;q=0.1")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .send()
        .map_err(|error| format!("Nu am putut citi CSS-ul Google Fonts: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Google Fonts a răspuns cu eroare: {error}"))?
        .text()
        .map_err(|error| format!("Nu am putut citi răspunsul Google Fonts: {error}"))?;

    let parsed_faces = parse_google_font_faces(&css);
    let faces = parsed_faces
        .iter()
        .filter(|face| is_woff2_google_font_face(face))
        .cloned()
        .collect::<Vec<_>>();
    if faces.is_empty() && !parsed_faces.is_empty() {
        let formats = google_font_face_formats(&parsed_faces);
        return Err(format!(
            "Google Fonts a returnat CSS, dar nu WOFF2 pentru această familie. Formate primite: {formats}."
        ));
    }
    if faces.is_empty() {
        return Err("Google Fonts nu a returnat fișiere WOFF2 pentru această familie.".to_string());
    }

    let family_slug = slugify_family(family);
    let license_file = format!("static/fonturi/{family_slug}/LICENTA.txt");
    let mut files = Vec::new();
    let mut writes = Vec::new();
    let mut css_blocks = Vec::new();

    for (index, face) in faces.iter().enumerate() {
        let extension = "woff2".to_string();
        let weight_range = face.weight_range.or(variable_range);
        let weight_segment = font_weight_file_segment(face.weight, weight_range);
        let optimization_segment = character_set
            .as_ref()
            .map(|characters| {
                let hash = hash_bytes(characters.as_bytes());
                format!("-text-{}", &hash[..hash.len().min(8)])
            })
            .unwrap_or_default();
        let file_name = format!(
            "{}-{}-{}{}-{}.{}",
            family_slug,
            face.style.as_deref().unwrap_or("normal"),
            weight_segment,
            optimization_segment,
            index + 1,
            extension
        );
        let bytes = client
            .get(&face.url)
            .send()
            .map_err(|error| format!("Nu am putut descărca fontul {}: {error}", face.url))?
            .error_for_status()
            .map_err(|error| {
                format!(
                    "Google Fonts a răspuns cu eroare pentru {}: {error}",
                    face.url
                )
            })?
            .bytes()
            .map_err(|error| format!("Nu am putut citi fontul {}: {error}", face.url))?;
        let parsed_metadata = local_import::parse_font_metadata(&bytes).ok();
        let project_relative = format!("static/fonturi/{family_slug}/{file_name}");
        let public_url = format!("/fonturi/{family_slug}/{file_name}");
        files.push(LocalFontFile {
            file: project_relative.clone(),
            file_name: file_name.clone(),
            size_bytes: bytes.len() as u64,
            extension: extension.clone(),
            format: font_format_label(&extension).to_string(),
            text_optimized: character_set.is_some(),
            content_hash: hash_bytes(&bytes),
            internal_family: Some(family.to_string()),
            subfamily: None,
            weight: face.weight,
            weight_range,
            style: face.style.clone(),
            axes: parsed_metadata
                .as_ref()
                .map(|metadata| metadata.axes.clone())
                .unwrap_or_default(),
            license: FontLicenseMetadata::default(),
            unicode_range: face.unicode_range.clone(),
            romanian_glyphs: Vec::new(),
            declared_weight: face.weight,
            declared_weight_range: weight_range,
            declared_style: face.style.clone(),
            preload: FontPreloadRegistration::default(),
        });
        css_blocks.push(google_font_face_css(
            family,
            face,
            &public_url,
            &extension,
            weight_range,
        ));
        writes.push(GoogleFontDownloadFileWrite {
            project_relative_path: project_relative,
            bytes: bytes.to_vec(),
        });
    }

    Ok(GoogleFontDownloadPlan {
        result: GoogleFontDownloadResult {
            family: FontFaceFamily {
                id: format!("css:{}", normalize_font_family_name(family)),
                family: family.to_string(),
                directories: vec![format!("static/fonturi/{family_slug}")],
                origin: FontOrigin::Local,
                theme_name: None,
                delivery: FontDeliveryKind::Local,
                ownership: FontOwnership::Managed,
                romanian_supported: None,
                files,
                faces: Vec::new(),
                issues: Vec::new(),
                license: FontLicenseMetadata {
                    description: Some(license.identifier.clone()),
                    url: Some(license.source_url.clone()),
                },
                registration: FontCssRegistration::default(),
            },
            font_face_css: css_blocks.join("\n\n"),
            css_url,
            license_file,
            license_source_url: license.source_url,
            variable: variable || !advanced_axes.is_empty(),
            text_optimized: character_set.is_some(),
            optimized_character_count: character_set
                .as_ref()
                .map(|characters| characters.chars().count())
                .unwrap_or_default(),
        },
        writes,
        license_text: license.text,
    })
}

pub(super) fn is_stylesheet_path(path: &str) -> bool {
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("css" | "scss")
    )
}

#[cfg(test)]
fn css_font_face_blocks(source: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative_start) = source[cursor..].find("@font-face") {
        let start = cursor + relative_start;
        let Some(relative_open) = source[start..].find('{') else {
            break;
        };
        let open = start + relative_open;
        let mut depth = 0usize;
        let mut close = None;
        for (offset, character) in source[open..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        close = Some(open + offset + character.len_utf8());
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            break;
        };
        blocks.push(&source[open + 1..close - 1]);
        cursor = close;
    }
    blocks
}

pub(super) fn normalize_font_family_name(value: &str) -> String {
    value
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"'))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(crate) fn managed_font_start_marker(family: &str) -> String {
    format!("/* pana-studio-font:{}:start */", slugify_family(family))
}

pub(crate) fn managed_font_end_marker(family: &str) -> String {
    format!("/* pana-studio-font:{}:end */", slugify_family(family))
}

fn google_font_catalog() -> Result<Vec<GoogleFontCatalogFamily>, String> {
    let cache = GOOGLE_FONT_CATALOG.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(catalog) = guard.as_ref() {
            return Ok(catalog.clone());
        }
    }

    let catalog = fetch_google_font_catalog()?;
    let mut guard = cache
        .lock()
        .map_err(|_| "Nu am putut bloca cache-ul Google Fonts.".to_string())?;
    *guard = Some(catalog.clone());
    Ok(catalog)
}

fn fetch_google_font_catalog() -> Result<Vec<GoogleFontCatalogFamily>, String> {
    let client = google_fonts_client()?;
    let url = google_font_catalog_url();
    let text = client
        .get(&url)
        .header(ACCEPT, "application/json,text/plain,*/*;q=0.1")
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .send()
        .map_err(|error| format!("Nu am putut citi catalogul Google Fonts: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Google Fonts a răspuns cu eroare pentru catalog: {error}"))?
        .text()
        .map_err(|error| format!("Nu am putut citi catalogul Google Fonts: {error}"))?;

    parse_google_font_catalog(&text)
}

fn google_fonts_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(GOOGLE_FONTS_USER_AGENT)
        .build()
        .map_err(|error| format!("Nu am putut pregăti clientul HTTP: {error}"))
}

struct GoogleFontLicense {
    identifier: String,
    text: String,
    source_url: String,
}

fn fetch_google_font_license(
    client: &reqwest::blocking::Client,
    family: &str,
) -> Result<GoogleFontLicense, String> {
    let repository_slug = family
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if repository_slug.is_empty() {
        return Err("Familia nu poate fi mapată la catalogul oficial de licențe.".to_string());
    }

    for root in ["ofl", "apache", "ufl"] {
        let base =
            format!("https://raw.githubusercontent.com/google/fonts/main/{root}/{repository_slug}");
        let metadata_url = format!("{base}/METADATA.pb");
        let response = client
            .get(&metadata_url)
            .send()
            .map_err(|error| format!("Nu am putut verifica licența Google Fonts: {error}"))?;
        if !response.status().is_success() {
            continue;
        }
        let metadata = response
            .text()
            .map_err(|error| format!("Nu am putut citi METADATA.pb pentru {family}: {error}"))?;
        let identifier = metadata
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("license:")
                    .map(|value| value.trim().trim_matches('"').to_string())
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| root.to_ascii_uppercase());
        let mut candidates = metadata
            .lines()
            .filter_map(|line| {
                let value = line.trim().strip_prefix("source_file:")?;
                let value = value.trim().trim_matches('"');
                let file_name = value.rsplit('/').next().unwrap_or(value);
                let lower = file_name.to_ascii_lowercase();
                (lower.ends_with(".txt")
                    && (lower.contains("license")
                        || lower.contains("ofl")
                        || lower.contains("ufl")))
                .then(|| file_name.to_string())
            })
            .collect::<Vec<_>>();
        candidates.extend(
            ["OFL.txt", "LICENSE.txt", "UFL.txt"]
                .into_iter()
                .map(str::to_string),
        );
        candidates.sort();
        candidates.dedup();

        for candidate in candidates {
            let source_url = format!("{base}/{candidate}");
            let response = client.get(&source_url).send().map_err(|error| {
                format!("Nu am putut descărca licența pentru {family}: {error}")
            })?;
            if !response.status().is_success() {
                continue;
            }
            let text = response.text().map_err(|error| {
                format!("Nu am putut citi licența oficială pentru {family}: {error}")
            })?;
            if text.trim().is_empty() {
                continue;
            }
            return Ok(GoogleFontLicense {
                identifier,
                text,
                source_url,
            });
        }
    }

    Err(format!(
        "Font Manager nu a găsit licența oficială pentru {family} în repository-ul google/fonts și a oprit instalarea."
    ))
}

fn google_font_catalog_url() -> String {
    if let Ok(key) = env::var("GOOGLE_FONTS_API_KEY") {
        let key = key.trim();
        if !key.is_empty() {
            return format!(
                "https://www.googleapis.com/webfonts/v1/webfonts?sort=popularity&capability=WOFF2&capability=VF&key={key}"
            );
        }
    }
    "https://fonts.google.com/metadata/fonts".to_string()
}

fn parse_google_font_catalog(text: &str) -> Result<Vec<GoogleFontCatalogFamily>, String> {
    let json_start = text
        .find('{')
        .ok_or_else(|| "Catalogul Google Fonts nu conține JSON valid.".to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text[json_start..])
        .map_err(|error| format!("Nu am putut parsa catalogul Google Fonts: {error}"))?;
    let families = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .or_else(|| {
            value
                .get("familyMetadataList")
                .and_then(serde_json::Value::as_array)
        })
        .ok_or_else(|| "Catalogul Google Fonts nu are lista de familii așteptată.".to_string())?;

    let mut catalog = families
        .iter()
        .filter_map(parse_google_catalog_family)
        .collect::<Vec<_>>();
    catalog.sort_by(|left, right| {
        left.family
            .to_ascii_lowercase()
            .cmp(&right.family.to_ascii_lowercase())
    });
    Ok(catalog)
}

fn parse_google_catalog_family(value: &serde_json::Value) -> Option<GoogleFontCatalogFamily> {
    let family = value
        .get("family")
        .or_else(|| value.get("name"))
        .and_then(serde_json::Value::as_str)?
        .to_string();
    let variants = parse_google_catalog_variants(value);
    let mut weights = variants
        .iter()
        .filter_map(|variant| weight_from_variant(variant))
        .collect::<Vec<_>>();
    weights.extend(weights_from_fonts_array(value));
    weights.extend(weights_from_axes(value));
    weights.sort_unstable();
    weights.dedup();
    if weights.is_empty() {
        weights.push(400);
    }

    Some(GoogleFontCatalogFamily {
        family,
        category: value
            .get("category")
            .and_then(serde_json::Value::as_str)
            .map(normalize_google_category),
        variants,
        weights,
        subsets: parse_string_array(value.get("subsets")),
        axes: parse_google_catalog_axes(value),
    })
}

fn parse_google_catalog_variants(value: &serde_json::Value) -> Vec<String> {
    let mut variants = parse_string_array(value.get("variants"));
    if variants.is_empty() {
        if let Some(fonts) = value.get("fonts").and_then(serde_json::Value::as_array) {
            for font in fonts {
                let weight = font
                    .get("weight")
                    .and_then(|value| {
                        value
                            .as_u64()
                            .or_else(|| value.as_str().and_then(|item| item.parse::<u64>().ok()))
                    })
                    .unwrap_or(400);
                let style = font
                    .get("style")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("normal");
                variants.push(if style == "italic" {
                    format!("{weight}italic")
                } else if weight == 400 {
                    "regular".to_string()
                } else {
                    weight.to_string()
                });
            }
        }
    }
    variants.sort();
    variants.dedup();
    variants
}

fn parse_google_catalog_axes(value: &serde_json::Value) -> Vec<GoogleFontAxis> {
    value
        .get("axes")
        .and_then(serde_json::Value::as_array)
        .map(|axes| {
            axes.iter()
                .filter_map(|axis| {
                    let tag = axis
                        .get("tag")
                        .and_then(serde_json::Value::as_str)?
                        .to_string();
                    let start = axis
                        .get("start")
                        .or_else(|| axis.get("min"))
                        .and_then(serde_json::Value::as_f64)?;
                    let end = axis
                        .get("end")
                        .or_else(|| axis.get("max"))
                        .and_then(serde_json::Value::as_f64)?;
                    Some(GoogleFontAxis { tag, start, end })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn weights_from_fonts_array(value: &serde_json::Value) -> Vec<u16> {
    value
        .get("fonts")
        .and_then(serde_json::Value::as_array)
        .map(|fonts| {
            fonts
                .iter()
                .filter_map(|font| {
                    font.get("weight")
                        .and_then(|value| {
                            value.as_u64().or_else(|| {
                                value.as_str().and_then(|item| item.parse::<u64>().ok())
                            })
                        })
                        .and_then(|weight| u16::try_from(weight).ok())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn weights_from_axes(value: &serde_json::Value) -> Vec<u16> {
    parse_google_catalog_axes(value)
        .into_iter()
        .find(|axis| axis.tag.eq_ignore_ascii_case("wght"))
        .map(|axis| {
            let start = ((axis.start / 100.0).ceil() as u16 * 100).clamp(100, 900);
            let end = ((axis.end / 100.0).floor() as u16 * 100).clamp(100, 900);
            (start..=end).step_by(100).collect()
        })
        .unwrap_or_default()
}

fn weight_from_variant(variant: &str) -> Option<u16> {
    if variant == "regular" || variant == "italic" {
        return Some(400);
    }
    let numeric = variant.trim_end_matches("italic");
    numeric.parse::<u16>().ok()
}

fn normalize_google_category(category: &str) -> String {
    category.to_ascii_lowercase().replace('_', "-")
}

pub(super) fn font_roots(zola_root: &Path, resolver: &ZolaThemeResolver) -> Vec<FontRootCandidate> {
    let mut roots = vec![FontRootCandidate {
        absolute_path: zola_root.join("static").join("fonturi"),
        relative_path: "static/fonturi".to_string(),
        origin: FontOrigin::Local,
        theme_name: None,
    }];

    if let Some(theme) = resolver.active_theme() {
        roots.push(FontRootCandidate {
            absolute_path: zola_root
                .join("themes")
                .join(theme)
                .join("static")
                .join("fonturi"),
            relative_path: format!("themes/{theme}/static/fonturi"),
            origin: FontOrigin::Theme,
            theme_name: Some(theme.to_string()),
        });
    }

    roots
}

#[derive(Clone, Debug)]
struct GoogleFontFace {
    url: String,
    format: Option<String>,
    weight: Option<u16>,
    weight_range: Option<FontWeightRange>,
    style: Option<String>,
    unicode_range: Option<String>,
}

fn normalized_weights(weights: &[u16]) -> Vec<u16> {
    let mut normalized: Vec<u16> = weights
        .iter()
        .copied()
        .filter(|weight| (100..=900).contains(weight) && weight % 100 == 0)
        .collect();
    normalized.sort_unstable();
    normalized.dedup();
    if normalized.is_empty() {
        vec![400, 700]
    } else {
        normalized
    }
}

fn normalized_variable_weight_range(weights: &[u16]) -> FontWeightRange {
    let mut valid = weights
        .iter()
        .copied()
        .filter(|weight| (1..=1000).contains(weight))
        .collect::<Vec<_>>();
    valid.sort_unstable();
    valid.dedup();
    let start = valid.first().copied().unwrap_or(100).clamp(1, 1000);
    let end = valid.last().copied().unwrap_or(900).clamp(1, 1000);
    if start <= end {
        FontWeightRange { start, end }
    } else {
        FontWeightRange {
            start: end,
            end: start,
        }
    }
}

fn normalized_google_font_styles(styles: &[String]) -> Vec<String> {
    let mut normalized = styles
        .iter()
        .map(|style| style.trim().to_ascii_lowercase())
        .filter(|style| matches!(style.as_str(), "normal" | "italic"))
        .collect::<Vec<_>>();
    normalized.sort_by_key(|style| u8::from(style == "italic"));
    normalized.dedup();
    if normalized.is_empty() {
        vec!["normal".to_string()]
    } else {
        normalized
    }
}

fn normalized_google_font_axes(
    family: &str,
    requested: &[GoogleFontAxis],
) -> Result<Vec<GoogleFontAxis>, String> {
    if requested.is_empty() {
        return Ok(Vec::new());
    }
    if requested.len() > 8 {
        return Err("Google Fonts a refuzat mai mult de 8 axe variabile.".to_string());
    }
    let catalog = google_font_catalog()?;
    let catalog_family = catalog
        .iter()
        .find(|candidate| candidate.family.eq_ignore_ascii_case(family))
        .ok_or_else(|| format!("Familia {family} nu mai există în catalogul Google Fonts."))?;
    let mut normalized = Vec::new();
    for axis in requested {
        let requested_tag = axis.tag.trim();
        if matches!(requested_tag.to_ascii_lowercase().as_str(), "ital" | "wght") {
            return Err(format!(
                "Axa {requested_tag} este controlată separat de stil și greutate."
            ));
        }
        if requested_tag.len() != 4
            || !requested_tag
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(format!(
                "Tag-ul axei {requested_tag} nu este OpenType valid."
            ));
        }
        if !axis.start.is_finite() || !axis.end.is_finite() || axis.start > axis.end {
            return Err(format!("Intervalul axei {requested_tag} este invalid."));
        }
        let available = catalog_family
            .axes
            .iter()
            .find(|candidate| candidate.tag.eq_ignore_ascii_case(requested_tag))
            .ok_or_else(|| {
                format!(
                    "Familia {family} nu declară axa {requested_tag} în catalogul Google Fonts."
                )
            })?;
        let tag = available.tag.clone();
        if axis.start < available.start || axis.end > available.end {
            return Err(format!(
                "Axa {tag} trebuie să rămână în intervalul {}–{} declarat de Google Fonts.",
                format_axis_number(available.start),
                format_axis_number(available.end)
            ));
        }
        if normalized
            .iter()
            .any(|candidate: &GoogleFontAxis| candidate.tag.eq_ignore_ascii_case(&tag))
        {
            return Err(format!("Axa {tag} a fost solicitată de două ori."));
        }
        normalized.push(GoogleFontAxis {
            tag,
            start: axis.start,
            end: axis.end,
        });
    }
    normalized.sort_by(|left, right| left.tag.cmp(&right.tag));
    Ok(normalized)
}

fn normalize_google_character_set(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let mut normalized = String::new();
    let mut seen = HashSet::new();
    for character in value.chars() {
        let character = if character.is_whitespace() {
            ' '
        } else {
            character
        };
        if character.is_control() {
            return Err(
                "Optimizarea caracterelor nu acceptă caractere de control invizibile.".to_string(),
            );
        }
        if seen.insert(character) {
            normalized.push(character);
        }
    }
    if normalized.trim().is_empty() {
        return Ok(None);
    }
    if normalized.chars().count() > 320 {
        return Err(
            "Optimizarea Google Fonts acceptă cel mult 320 de caractere unice per instalare."
                .to_string(),
        );
    }
    Ok(Some(normalized))
}

fn google_fonts_css_url(
    family: &str,
    weights: &[u16],
    styles: &[String],
    variable: bool,
    advanced_axes: &[GoogleFontAxis],
    character_set: Option<&str>,
) -> String {
    let family_query = percent_encode_family(family);
    let weight_values = if variable {
        let range = normalized_variable_weight_range(weights);
        if range.start == range.end {
            range.start.to_string()
        } else {
            format!("{}..{}", range.start, range.end)
        }
    } else {
        String::new()
    };
    let styles = normalized_google_font_styles(styles);
    let has_ital_axis = styles != ["normal"];
    let mut axis_tags = advanced_axes
        .iter()
        .map(|axis| axis.tag.as_str())
        .chain(std::iter::once("wght"))
        .chain(has_ital_axis.then_some("ital"))
        .collect::<Vec<_>>();
    axis_tags.sort();
    let rows = if variable {
        styles
            .iter()
            .map(|style| {
                axis_tags
                    .iter()
                    .map(|tag| match *tag {
                        "ital" => u8::from(style == "italic").to_string(),
                        "wght" => weight_values.clone(),
                        tag => {
                            let axis = advanced_axes
                                .iter()
                                .find(|axis| axis.tag == tag)
                                .expect("normalized Google axis must exist");
                            format_axis_range(axis.start, axis.end)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
    } else {
        styles
            .iter()
            .flat_map(|style| {
                weights
                    .iter()
                    .map(|weight| {
                        axis_tags
                            .iter()
                            .map(|tag| match *tag {
                                "ital" => u8::from(style == "italic").to_string(),
                                "wght" => weight.to_string(),
                                tag => {
                                    let axis = advanced_axes
                                        .iter()
                                        .find(|axis| axis.tag == tag)
                                        .expect("normalized Google axis must exist");
                                    format_axis_range(axis.start, axis.end)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let axis_spec = format!("{}@{}", axis_tags.join(","), rows.join(";"));
    let mut url =
        format!("https://fonts.googleapis.com/css2?family={family_query}:{axis_spec}&display=swap");
    if let Some(character_set) = character_set {
        url.push_str("&text=");
        url.push_str(&percent_encode_query_value(character_set));
    }
    url
}

fn format_axis_range(start: f64, end: f64) -> String {
    if (start - end).abs() < f64::EPSILON {
        format_axis_number(start)
    } else {
        format!("{}..{}", format_axis_number(start), format_axis_number(end))
    }
}

fn format_axis_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        let formatted = format!("{value:.4}");
        formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn percent_encode_family(family: &str) -> String {
    family
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => (byte as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn percent_encode_query_value(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (*byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn parse_google_font_faces(css: &str) -> Vec<GoogleFontFace> {
    let mut faces = Vec::new();
    let mut rest = css;

    while let Some(start) = rest.find("@font-face") {
        rest = &rest[start + "@font-face".len()..];
        let Some(open) = rest.find('{') else {
            break;
        };
        rest = &rest[open + 1..];
        let Some(close) = rest.find('}') else {
            break;
        };
        let block = &rest[..close];
        rest = &rest[close + 1..];

        let declarations = parse_css_declarations(block);
        let Some(src) = declarations.get("src") else {
            continue;
        };
        let Some(url) = extract_css_url(src) else {
            continue;
        };

        let (weight, weight_range) = declarations
            .get("font-weight")
            .map(|weight| parse_font_weight(weight))
            .unwrap_or((None, None));

        faces.push(GoogleFontFace {
            url,
            format: extract_css_format(src),
            weight,
            weight_range,
            style: declarations
                .get("font-style")
                .map(|style| style.trim().to_string()),
            unicode_range: declarations
                .get("unicode-range")
                .map(|range| range.trim().to_string()),
        });
    }

    faces
}

pub(super) fn parse_font_weight(value: &str) -> (Option<u16>, Option<FontWeightRange>) {
    let weights = value
        .split_whitespace()
        .filter_map(|item| item.trim().parse::<u16>().ok())
        .filter(|weight| (100..=900).contains(weight))
        .collect::<Vec<_>>();

    if weights.len() >= 2 {
        let start = weights[0].min(weights[1]);
        let end = weights[0].max(weights[1]);
        return (None, Some(FontWeightRange { start, end }));
    }

    (weights.first().copied(), None)
}

fn parse_css_declarations(block: &str) -> BTreeMap<String, String> {
    let mut declarations = BTreeMap::new();
    for declaration in block.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim();
        if property.is_empty() {
            continue;
        }
        declarations.insert(property.to_string(), value.trim().to_string());
    }
    declarations
}

fn extract_css_url(src: &str) -> Option<String> {
    let start = src.find("url(")? + 4;
    let end = src[start..].find(')')? + start;
    Some(src[start..end].trim().trim_matches(['\'', '"']).to_string())
}

fn extract_css_format(src: &str) -> Option<String> {
    let start = src.find("format(")? + 7;
    let end = src[start..].find(')')? + start;
    Some(src[start..end].trim().trim_matches(['\'', '"']).to_string())
}

fn font_extension_from_url_exact(url: &str) -> Option<String> {
    let without_query = url.split('?').next().unwrap_or(url);
    without_query
        .rsplit('.')
        .next()
        .filter(|extension| matches!(*extension, "woff2" | "woff" | "ttf" | "otf"))
        .map(ToString::to_string)
}

fn is_woff2_google_font_face(face: &GoogleFontFace) -> bool {
    face.format
        .as_deref()
        .map(|format| format.eq_ignore_ascii_case("woff2"))
        .unwrap_or(false)
        || font_extension_from_url_exact(&face.url)
            .map(|extension| extension.eq_ignore_ascii_case("woff2"))
            .unwrap_or(false)
}

fn google_font_face_formats(faces: &[GoogleFontFace]) -> String {
    let mut formats = faces
        .iter()
        .map(|face| {
            face.format
                .clone()
                .or_else(|| font_extension_from_url_exact(&face.url))
                .unwrap_or_else(|| "necunoscut".to_string())
        })
        .collect::<Vec<_>>();
    formats.sort();
    formats.dedup();
    formats.join(", ")
}

fn font_weight_file_segment(weight: Option<u16>, weight_range: Option<FontWeightRange>) -> String {
    if let Some(range) = weight_range {
        if range.start == range.end {
            range.start.to_string()
        } else {
            format!("{}-{}", range.start, range.end)
        }
    } else {
        weight.unwrap_or(400).to_string()
    }
}

fn font_weight_css_value(weight: Option<u16>, weight_range: Option<FontWeightRange>) -> String {
    if let Some(range) = weight_range {
        if range.start == range.end {
            range.start.to_string()
        } else {
            format!("{} {}", range.start, range.end)
        }
    } else {
        weight.unwrap_or(400).to_string()
    }
}

fn google_font_face_css(
    family: &str,
    face: &GoogleFontFace,
    public_url: &str,
    extension: &str,
    weight_range: Option<FontWeightRange>,
) -> String {
    let mut lines = vec![
        "@font-face {".to_string(),
        format!("  font-family: '{}';", css_string_escape(family)),
        format!(
            "  font-style: {};",
            face.style.as_deref().unwrap_or("normal")
        ),
        format!(
            "  font-weight: {};",
            font_weight_css_value(face.weight, weight_range)
        ),
        "  font-display: swap;".to_string(),
        format!(
            "  src: url('{}') format('{}');",
            public_url,
            face.format
                .as_deref()
                .unwrap_or_else(|| font_format_label(extension))
        ),
    ];
    if let Some(unicode_range) = face.unicode_range.as_deref() {
        lines.push(format!("  unicode-range: {};", unicode_range));
    }
    lines.push("}".to_string());
    lines.join("\n")
}

fn slugify_family(family: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for character in family.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "font".to_string()
    } else {
        slug
    }
}

fn css_string_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

pub(super) fn is_supported_font_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase()),
        Some(extension) if matches!(extension.as_str(), "woff2" | "woff" | "ttf" | "otf")
    )
}

pub(super) fn is_text_optimized_font_file_name(file_name: &str) -> bool {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name);
    let segments = stem.split('-').collect::<Vec<_>>();
    segments.windows(2).any(|pair| {
        pair[0] == "text"
            && pair[1].len() == 8
            && pair[1].bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

pub(super) fn local_font_file_from_bytes(
    relative_zola_path: &str,
    file_name: &str,
    extension: &str,
    bytes: &[u8],
) -> LocalFontFile {
    match local_import::parse_font_metadata(bytes) {
        Ok(metadata) => LocalFontFile {
            file: relative_zola_path.to_string(),
            file_name: file_name.to_string(),
            size_bytes: bytes.len() as u64,
            extension: extension.to_string(),
            format: font_format_label(extension).to_string(),
            text_optimized: is_text_optimized_font_file_name(file_name),
            content_hash: hash_bytes(bytes),
            internal_family: Some(metadata.family),
            subfamily: metadata.subfamily,
            weight: metadata.weight,
            weight_range: metadata.weight_range,
            style: Some(metadata.style),
            axes: metadata.axes,
            license: metadata.license,
            unicode_range: None,
            romanian_glyphs: metadata.romanian_glyphs,
            declared_weight: None,
            declared_weight_range: None,
            declared_style: None,
            preload: FontPreloadRegistration::default(),
        },
        Err(_) => LocalFontFile {
            file: relative_zola_path.to_string(),
            file_name: file_name.to_string(),
            size_bytes: bytes.len() as u64,
            extension: extension.to_string(),
            format: font_format_label(extension).to_string(),
            text_optimized: is_text_optimized_font_file_name(file_name),
            content_hash: hash_bytes(bytes),
            internal_family: None,
            subfamily: None,
            weight: detect_font_weight(file_name),
            weight_range: detect_font_weight_range(file_name),
            style: detect_font_style(file_name),
            axes: Vec::new(),
            license: FontLicenseMetadata::default(),
            unicode_range: None,
            romanian_glyphs: Vec::new(),
            declared_weight: None,
            declared_weight_range: None,
            declared_style: None,
            preload: FontPreloadRegistration::default(),
        },
    }
}

pub(super) fn font_file_sort_weight(file: &LocalFontFile) -> u16 {
    file.weight_range
        .map(|range| range.start)
        .or(file.weight)
        .unwrap_or(400)
}

pub(super) fn font_format_label(extension: &str) -> &'static str {
    match extension {
        "woff2" => "woff2",
        "woff" => "woff",
        "ttf" => "truetype",
        "otf" => "opentype",
        _ => "font",
    }
}

fn detect_font_weight(file_name: &str) -> Option<u16> {
    if detect_font_weight_range(file_name).is_some() {
        return None;
    }

    let lower = file_name.to_ascii_lowercase();

    for weight in (100..=900).step_by(100) {
        if lower.contains(&weight.to_string()) {
            return Some(weight);
        }
    }

    let named_weights = [
        ("thin", 100),
        ("extralight", 200),
        ("extra-light", 200),
        ("light", 300),
        ("regular", 400),
        ("normal", 400),
        ("book", 400),
        ("medium", 500),
        ("semibold", 600),
        ("semi-bold", 600),
        ("demibold", 600),
        ("bold", 700),
        ("extrabold", 800),
        ("extra-bold", 800),
        ("black", 900),
        ("heavy", 900),
    ];

    named_weights
        .iter()
        .find(|(name, _)| lower.contains(name))
        .map(|(_, weight)| *weight)
}

fn detect_font_weight_range(file_name: &str) -> Option<FontWeightRange> {
    let lower = file_name.to_ascii_lowercase();
    let mut weights = Vec::new();
    let mut current = String::new();

    for character in lower.chars() {
        if character.is_ascii_digit() {
            current.push(character);
        } else if !current.is_empty() {
            if let Ok(weight) = current.parse::<u16>() {
                if (100..=900).contains(&weight) && weight % 100 == 0 {
                    weights.push(weight);
                }
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(weight) = current.parse::<u16>() {
            if (100..=900).contains(&weight) && weight % 100 == 0 {
                weights.push(weight);
            }
        }
    }

    weights.sort_unstable();
    weights.dedup();
    if weights.len() >= 2 && (lower.contains("variable") || lower.contains("var")) {
        let start = weights.first().copied().unwrap_or(100);
        let end = weights.last().copied().unwrap_or(900);
        return Some(FontWeightRange { start, end });
    }

    if weights.len() >= 2
        && lower.contains(&format!("{}-{}", weights[0], weights[weights.len() - 1]))
    {
        return Some(FontWeightRange {
            start: weights[0],
            end: weights[weights.len() - 1],
        });
    }

    None
}

fn detect_font_style(file_name: &str) -> Option<String> {
    let lower = file_name.to_ascii_lowercase();
    if lower.contains("italic") {
        Some("italic".to_string())
    } else if lower.contains("oblique") {
        Some("oblique".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css2_url_uses_variable_weight_range() {
        let url = google_fonts_css_url(
            "Inter Tight",
            &[100, 900],
            &["normal".to_string()],
            true,
            &[],
            None,
        );
        assert!(url.contains("family=Inter+Tight:wght@100..900"));
    }

    #[test]
    fn css2_url_requests_normal_and_italic_variants_explicitly() {
        let url = google_fonts_css_url(
            "Inter Tight",
            &[400, 700],
            &["normal".to_string(), "italic".to_string()],
            false,
            &[],
            None,
        );
        assert!(url.contains("family=Inter+Tight:ital,wght@0,400;0,700;1,400;1,700"));
    }

    #[test]
    fn css2_url_orders_advanced_variable_axes_alphabetically() {
        let url = google_fonts_css_url(
            "Roboto Flex",
            &[100, 1000],
            &["normal".to_string()],
            true,
            &[
                GoogleFontAxis {
                    tag: "wdth".to_string(),
                    start: 25.0,
                    end: 151.0,
                },
                GoogleFontAxis {
                    tag: "opsz".to_string(),
                    start: 8.0,
                    end: 144.0,
                },
            ],
            None,
        );
        assert!(url.contains("family=Roboto+Flex:opsz,wdth,wght@8..144,25..151,100..1000"));
    }

    #[test]
    fn css2_url_keeps_weight_static_when_only_optical_size_is_variable() {
        let url = google_fonts_css_url(
            "Newsreader",
            &[400, 700],
            &["normal".to_string()],
            false,
            &[GoogleFontAxis {
                tag: "opsz".to_string(),
                start: 6.0,
                end: 72.0,
            }],
            None,
        );

        assert!(url.contains("family=Newsreader:opsz,wght@6..72,400;6..72,700"));
    }

    #[test]
    fn css2_url_preserves_case_for_custom_opentype_axes() {
        let url = google_fonts_css_url(
            "Roboto Flex",
            &[400],
            &["normal".to_string()],
            false,
            &[
                GoogleFontAxis {
                    tag: "opsz".to_string(),
                    start: 8.0,
                    end: 144.0,
                },
                GoogleFontAxis {
                    tag: "GRAD".to_string(),
                    start: -200.0,
                    end: 150.0,
                },
            ],
            None,
        );

        assert!(url.contains("family=Roboto+Flex:GRAD,opsz,wght@-200..150,8..144,400"));
    }

    #[test]
    fn css2_url_encodes_an_explicit_character_set() {
        let characters = normalize_google_character_set(Some("Pană Pană!"))
            .unwrap()
            .unwrap();
        let url = google_fonts_css_url(
            "Inter",
            &[400],
            &["normal".to_string()],
            false,
            &[],
            Some(&characters),
        );

        assert!(url.ends_with("&text=Pan%C4%83%20%21"));
    }

    #[test]
    fn recognizes_only_managed_text_optimization_file_names() {
        assert!(is_text_optimized_font_file_name(
            "inter-normal-400-text-a1b2c3d4-1.woff2"
        ));
        assert!(!is_text_optimized_font_file_name(
            "inter-normal-400-text-preview-1.woff2"
        ));
        assert!(!is_text_optimized_font_file_name("context-a1b2c3d4.woff2"));
    }

    #[test]
    fn parses_variable_font_weight_range() {
        let (weight, range) = parse_font_weight("100 900");
        assert_eq!(weight, None);
        assert_eq!(
            range,
            Some(FontWeightRange {
                start: 100,
                end: 900
            })
        );
    }

    #[test]
    fn detects_variable_range_from_downloaded_file_name() {
        assert_eq!(
            detect_font_weight_range("inter-normal-100-900-1.woff2"),
            Some(FontWeightRange {
                start: 100,
                end: 900
            })
        );
    }

    #[test]
    fn filters_google_catalog_with_offset() {
        let catalog = ["Afacad", "Agu Display", "Akshar", "Inter"]
            .into_iter()
            .map(|family| GoogleFontCatalogFamily {
                family: family.to_string(),
                category: None,
                variants: vec!["regular".to_string()],
                weights: vec![400],
                subsets: vec!["latin".to_string()],
                axes: Vec::new(),
            })
            .collect::<Vec<_>>();

        let results = filter_google_font_catalog(&catalog, "A", 2, 1);
        assert_eq!(
            results
                .iter()
                .map(|font| font.family.as_str())
                .collect::<Vec<_>>(),
            vec!["Agu Display", "Akshar"]
        );
    }

    #[test]
    fn parses_google_css2_woff2_face() {
        let css = r#"
        @font-face {
          font-family: 'League Gothic';
          font-style: normal;
          font-weight: 400;
          font-stretch: 100%;
          font-display: swap;
          src: url(https://fonts.gstatic.com/s/leaguegothic/v13/example.woff2) format('woff2');
          unicode-range: U+0000-00FF;
        }
        "#;

        let faces = parse_google_font_faces(css);
        assert_eq!(faces.len(), 1);
        assert!(is_woff2_google_font_face(&faces[0]));
        assert_eq!(faces[0].weight, Some(400));
    }

    #[test]
    fn managed_font_face_block_is_idempotent_and_replaceable() {
        let initial = "$color: red;\n";
        let first = upsert_managed_font_face_block(
            initial,
            "Inter",
            "@font-face { font-family: 'Inter'; font-display: swap; }",
        )
        .unwrap();
        let second = upsert_managed_font_face_block(
            &first,
            "Inter",
            "@font-face { font-family: 'Inter'; font-display: optional; }",
        )
        .unwrap();

        assert_eq!(second.matches("pana-studio-font:inter:start").count(), 1);
        assert!(!second.contains("font-display: swap"));
        assert!(second.contains("font-display: optional"));
    }

    #[test]
    fn managed_font_face_block_can_be_removed_without_damaging_neighbors() {
        let source = "body { color: black; }\n\n/* pana-studio-font:inter:start */\n@font-face { font-family: 'Inter'; }\n/* pana-studio-font:inter:end */\n\nmain { display: block; }\n";
        let removed = remove_managed_font_face_block(source, "Inter")
            .expect("managed font block should be removable");

        assert_eq!(
            removed,
            "body { color: black; }\n\nmain { display: block; }\n"
        );
        assert!(!removed.contains("@font-face"));
    }

    #[test]
    fn registration_count_detects_an_external_duplicate_after_managed_removal() {
        let source = "/* pana-studio-font:inter:start */\n@font-face { font-family: 'Inter'; }\n/* pana-studio-font:inter:end */\n@font-face { font-family: \"Inter\"; src: url('/external.woff2'); }\n";
        let removed = remove_managed_font_face_block(source, "Inter").unwrap();

        assert_eq!(font_family_registration_count(&removed, "Inter"), 1);
    }

    #[test]
    fn stylesheet_selection_prefers_the_framework_base_and_ignores_public_output() {
        let selected = select_font_face_stylesheet(
            [
                (
                    "public/css/framework.css",
                    "@font-face { font-family: 'Built'; }",
                ),
                ("sass/pagini/index.scss", ".page { display: block; }"),
                (
                    "themes/demo/sass/css-framework/_baza.scss",
                    "body { margin: 0; }",
                ),
            ]
            .into_iter(),
        );

        assert_eq!(
            selected.as_deref(),
            Some("themes/demo/sass/css-framework/_baza.scss")
        );
    }
}
