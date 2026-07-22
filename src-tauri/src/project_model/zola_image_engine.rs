use std::{fs, path::Path};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::project_model::move_engine::{
    parse_html_tag_at, source_location_at_offset, ProjectSourceEditLocation,
};

use super::{
    attribute_engine::{
        insert_raw_tag_attribute, raw_tag_attributes, remove_tag_attribute, set_tag_attribute_value,
    },
    model::ProjectModel,
};

const MARKER_PREFIX: &str = "{# pana-studio:zola-image:v1:";
const MARKER_SUFFIX: &str = " #}";
const CONTRACT_VERSION: u8 = 1;
const MAX_IMAGE_DIMENSION: u32 = 16_384;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectZolaImageIntent {
    pub enabled: bool,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub operation: Option<ZolaImageOperation>,
    #[serde(default)]
    pub format: Option<ZolaImageFormat>,
    #[serde(default)]
    pub quality: Option<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZolaImageOperation {
    FitWidth,
    Fit,
    Fill,
}

impl ZolaImageOperation {
    fn as_zola_str(self) -> &'static str {
        match self {
            Self::FitWidth => "fit_width",
            Self::Fit => "fit",
            Self::Fill => "fill",
        }
    }

    fn requires_height(self) -> bool {
        matches!(self, Self::Fit | Self::Fill)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZolaImageFormat {
    Auto,
    Webp,
    Avif,
    Jpg,
    Png,
}

impl ZolaImageFormat {
    fn as_zola_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Jpg => "jpg",
            Self::Png => "png",
        }
    }

    fn uses_quality(self) -> bool {
        !matches!(self, Self::Png)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ZolaImagePresentation {
    pub source_url: String,
    pub source_path: String,
    pub width: u32,
    pub height: Option<u32>,
    pub operation: ZolaImageOperation,
    pub format: ZolaImageFormat,
    pub quality: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct OriginalAttribute {
    name: String,
    raw: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ZolaImageContractMetadata {
    version: u8,
    variable: String,
    presentation: ZolaImagePresentation,
    original_attributes: Vec<OriginalAttribute>,
}

#[derive(Clone, Debug)]
pub struct ZolaImageApplication {
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub source_start_line: usize,
    pub presentation: Option<ZolaImagePresentation>,
}

#[derive(Clone, Debug)]
struct ExistingContract {
    prelude_start: usize,
    metadata: ZolaImageContractMetadata,
}

pub fn apply_zola_image_contract(
    model: &ProjectModel,
    file: &str,
    source: &str,
    opening_start: usize,
    intent: &ProjectZolaImageIntent,
) -> Result<ZolaImageApplication, String> {
    let opening = parse_html_tag_at(source, opening_start)
        .ok_or_else(|| "Locația nu mai indică un tag HTML stabil.".to_string())?;
    if opening.is_closing || !opening.tag.eq_ignore_ascii_case("img") {
        return Err("Procesarea Zola poate fi aplicată numai unui element <img>.".to_string());
    }
    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Nu am putut citi tag-ul <img>.".to_string())?;
    let existing = inspect_contract(source, opening.start, opening_source)?;

    if !intent.enabled {
        let existing = existing.ok_or_else(|| {
            "Elementul <img> nu are un contract de procesare Zola administrat de Pană Studio."
                .to_string()
        })?;
        let restored = restore_original_attributes(opening_source, &existing.metadata)?;
        let contents = replace_range(source, existing.prelude_start, opening.end, &restored);
        let target_location = source_location_at_offset(&contents, file, existing.prelude_start);
        return Ok(ZolaImageApplication {
            source_start_line: target_location.line,
            target_location,
            contents,
            presentation: None,
        });
    }

    let presentation = normalized_presentation(intent)?;
    validate_local_image_source(model, &presentation)?;

    let (replace_start, mut metadata) = if let Some(existing) = existing {
        (existing.prelude_start, existing.metadata)
    } else {
        (
            opening.start,
            ZolaImageContractMetadata {
                version: CONTRACT_VERSION,
                variable: stable_variable(file, opening.start),
                presentation: presentation.clone(),
                original_attributes: capture_original_attributes(opening_source),
            },
        )
    };

    if metadata.presentation.source_url != presentation.source_url {
        replace_original_source(&mut metadata, &presentation.source_url);
    }
    metadata.presentation = presentation.clone();
    let prelude = render_prelude(&metadata)?;
    let managed_opening = render_managed_opening(opening_source, &metadata.variable);
    let replacement = format!("{prelude}{managed_opening}");
    let contents = replace_range(source, replace_start, opening.end, &replacement);
    let next_opening_start = replace_start + prelude.len();
    let target_location = source_location_at_offset(&contents, file, next_opening_start);

    Ok(ZolaImageApplication {
        source_start_line: target_location.line,
        target_location,
        contents,
        presentation: Some(presentation),
    })
}

pub fn inspect_zola_image_at(
    source: &str,
    opening_start: usize,
) -> Result<Option<ZolaImagePresentation>, String> {
    let Some(opening) = parse_html_tag_at(source, opening_start) else {
        return Ok(None);
    };
    if opening.is_closing || !opening.tag.eq_ignore_ascii_case("img") {
        return Ok(None);
    }
    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Nu am putut citi tag-ul <img>.".to_string())?;
    Ok(inspect_contract(source, opening.start, opening_source)?
        .map(|contract| contract.metadata.presentation))
}

pub fn zola_image_contract_start(
    source: &str,
    opening_start: usize,
) -> Result<Option<usize>, String> {
    let Some(opening) = parse_html_tag_at(source, opening_start) else {
        return Ok(None);
    };
    if opening.is_closing || !opening.tag.eq_ignore_ascii_case("img") {
        return Ok(None);
    }
    let opening_source = source
        .get(opening.start..opening.end)
        .ok_or_else(|| "Nu am putut citi tag-ul <img>.".to_string())?;
    Ok(inspect_contract(source, opening.start, opening_source)?
        .map(|contract| contract.prelude_start))
}

pub fn contains_zola_image_contract(source: &str) -> bool {
    source.contains(MARKER_PREFIX)
}

pub fn encode_preview_presentation(presentation: &ZolaImagePresentation) -> Result<String, String> {
    let json = serde_json::to_vec(presentation)
        .map_err(|error| format!("Nu am putut serializa starea imaginii Zola: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

fn inspect_contract(
    source: &str,
    opening_start: usize,
    opening_source: &str,
) -> Result<Option<ExistingContract>, String> {
    let prefix = source.get(..opening_start).unwrap_or("");
    let Some(marker_start) = prefix.rfind(MARKER_PREFIX) else {
        return Ok(None);
    };
    let segment = &prefix[marker_start..];
    if segment.contains('<') {
        return Ok(None);
    }
    let payload_start = MARKER_PREFIX.len();
    let Some(payload_end) = segment.find(MARKER_SUFFIX) else {
        return Err("Contractul imaginii Zola are markerul incomplet.".to_string());
    };
    let payload = &segment[payload_start..payload_end];
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| "Contractul imaginii Zola are metadata invalidă.".to_string())?;
    let metadata: ZolaImageContractMetadata = serde_json::from_slice(&decoded)
        .map_err(|_| "Contractul imaginii Zola are metadata JSON invalidă.".to_string())?;
    validate_metadata(&metadata)?;
    let expected_prelude = render_prelude(&metadata)?;
    if segment != expected_prelude {
        return Err(
            "Contractul imaginii Zola a fost modificat manual și nu poate fi actualizat în siguranță."
                .to_string(),
        );
    }
    validate_managed_opening(opening_source, &metadata.variable)?;
    Ok(Some(ExistingContract {
        prelude_start: marker_start,
        metadata,
    }))
}

fn normalized_presentation(
    intent: &ProjectZolaImageIntent,
) -> Result<ZolaImagePresentation, String> {
    let source_url = intent
        .source_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Procesarea Zola necesită URL-ul sursei locale.".to_string())?;
    validate_source_url(source_url)?;
    let source_path = normalize_source_path(
        intent
            .source_path
            .as_deref()
            .ok_or_else(|| "Procesarea Zola necesită calea locală a imaginii.".to_string())?,
    )?;
    let operation = intent
        .operation
        .ok_or_else(|| "Operația resize_image lipsește.".to_string())?;
    let format = intent
        .format
        .ok_or_else(|| "Formatul resize_image lipsește.".to_string())?;
    let width = intent
        .width
        .ok_or_else(|| "Lățimea resize_image lipsește.".to_string())?;
    validate_dimension(width, "Lățimea")?;
    let height = intent.height;
    if operation.requires_height() {
        validate_dimension(
            height.ok_or_else(|| "Operația selectată necesită și înălțime.".to_string())?,
            "Înălțimea",
        )?;
    } else if let Some(height) = height {
        validate_dimension(height, "Înălțimea")?;
    }
    let quality = intent
        .quality
        .ok_or_else(|| "Calitatea resize_image lipsește.".to_string())?;
    if !(1..=100).contains(&quality) {
        return Err("Calitatea imaginii trebuie să fie între 1 și 100.".to_string());
    }
    Ok(ZolaImagePresentation {
        source_url: source_url.to_string(),
        source_path,
        width,
        height: operation.requires_height().then_some(height).flatten(),
        operation,
        format,
        quality,
    })
}

fn validate_metadata(metadata: &ZolaImageContractMetadata) -> Result<(), String> {
    if metadata.version != CONTRACT_VERSION {
        return Err("Versiunea contractului imaginii Zola nu este suportată.".to_string());
    }
    if !metadata.variable.starts_with("pana_image_")
        || !metadata
            .variable
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err("Variabila contractului imaginii Zola este invalidă.".to_string());
    }
    validate_source_url(&metadata.presentation.source_url)?;
    normalize_source_path(&metadata.presentation.source_path)?;
    validate_dimension(metadata.presentation.width, "Lățimea")?;
    if metadata.presentation.operation.requires_height() {
        validate_dimension(
            metadata
                .presentation
                .height
                .ok_or_else(|| "Contractul imaginii Zola nu are înălțime.".to_string())?,
            "Înălțimea",
        )?;
    }
    if !(1..=100).contains(&metadata.presentation.quality) {
        return Err("Calitatea din contractul imaginii Zola este invalidă.".to_string());
    }
    for original in &metadata.original_attributes {
        if !matches!(original.name.as_str(), "src" | "width" | "height")
            || original.raw.contains(['\n', '\r', '\0'])
        {
            return Err(
                "Atributele originale din contractul imaginii Zola sunt invalide.".to_string(),
            );
        }
    }
    Ok(())
}

fn validate_dimension(value: u32, label: &str) -> Result<(), String> {
    if value == 0 || value > MAX_IMAGE_DIMENSION {
        return Err(format!(
            "{label} trebuie să fie între 1 și {MAX_IMAGE_DIMENSION}px."
        ));
    }
    Ok(())
}

fn validate_source_url(value: &str) -> Result<(), String> {
    if !value.starts_with('/')
        || value.starts_with("//")
        || value.contains(['?', '#', '{', '}', '\\', '\n', '\r', '\0'])
        || value.split('/').any(|segment| segment == "..")
    {
        return Err(
            "Procesarea Zola acceptă numai un URL local, static și fără query sau expresii Tera."
                .to_string(),
        );
    }
    Ok(())
}

fn normalize_source_path(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || value.contains(['\n', '\r', '\0', '{', '}'])
    {
        return Err("Calea imaginii Zola trebuie să fie relativă și statică.".to_string());
    }
    let segments: Vec<&str> = value.split('/').collect();
    if segments
        .iter()
        .any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
    {
        return Err("Calea imaginii Zola conține segmente nesigure.".to_string());
    }
    Ok(segments.join("/"))
}

fn validate_local_image_source(
    model: &ProjectModel,
    presentation: &ZolaImagePresentation,
) -> Result<(), String> {
    let path = &presentation.source_path;
    let allowed = path.starts_with("static/")
        || path.starts_with("content/")
        || model
            .source_graph
            .active_theme
            .as_deref()
            .is_some_and(|theme| path.starts_with(&format!("themes/{theme}/static/")));
    if !allowed {
        return Err(
            "Imaginea trebuie să aparțină folderului static, content sau temei Zola active."
                .to_string(),
        );
    }
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp" | "avif") {
        return Err("Formatul sursei nu este suportat de acest contract resize_image.".to_string());
    }

    let full = model.zola_root.join(path);
    reject_symlink_components(&model.zola_root, path)?;
    let metadata =
        fs::metadata(&full).map_err(|_| format!("Imaginea locală {path} nu mai există."))?;
    if !metadata.is_file() {
        return Err(format!("Sursa Zola {path} nu este un fișier obișnuit."));
    }
    let canonical_root = model
        .zola_root
        .canonicalize()
        .map_err(|error| format!("Nu am putut valida root-ul Zola: {error}"))?;
    let canonical_file = full
        .canonicalize()
        .map_err(|error| format!("Nu am putut valida imaginea locală: {error}"))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err("Imaginea locală iese din root-ul Zola.".to_string());
    }

    validate_public_url_mapping(model, presentation)?;
    Ok(())
}

fn reject_symlink_components(zola_root: &Path, relative: &str) -> Result<(), String> {
    let mut current = zola_root.to_path_buf();
    for segment in relative.split('/') {
        current.push(segment);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| format!("Nu am putut valida componenta {segment} a imaginii."))?;
        if metadata.file_type().is_symlink() {
            return Err("Procesarea Zola refuză imaginile accesate prin symlink.".to_string());
        }
    }
    Ok(())
}

fn validate_public_url_mapping(
    model: &ProjectModel,
    presentation: &ZolaImagePresentation,
) -> Result<(), String> {
    let expected = public_url_for_source_path(&presentation.source_path)
        .ok_or_else(|| "Nu am putut deriva URL-ul public al imaginii.".to_string())?;
    if expected != presentation.source_url {
        return Err(format!(
            "URL-ul {} nu corespunde sursei locale {} (așteptat {}).",
            presentation.source_url, presentation.source_path, expected
        ));
    }

    if let Some(theme) = model.source_graph.active_theme.as_deref() {
        let logical = expected.trim_start_matches('/');
        let local = model.zola_root.join("static").join(logical);
        let themed = model
            .zola_root
            .join("themes")
            .join(theme)
            .join("static")
            .join(logical);
        if local.is_file() && themed.is_file() {
            return Err(format!(
                "URL-ul {expected} este ambiguu între static local și tema {theme}."
            ));
        }
    }
    Ok(())
}

fn public_url_for_source_path(path: &str) -> Option<String> {
    if let Some(logical) = path.strip_prefix("static/") {
        return Some(format!("/{}", logical.trim_start_matches('/')));
    }
    if let Some(after_themes) = path.strip_prefix("themes/") {
        let (_theme, logical) = after_themes.split_once("/static/")?;
        return Some(format!("/{}", logical.trim_start_matches('/')));
    }
    path.strip_prefix("content/")
        .map(|logical| format!("/content/{}", logical.trim_start_matches('/')))
}

fn render_prelude(metadata: &ZolaImageContractMetadata) -> Result<String, String> {
    let json = serde_json::to_vec(metadata)
        .map_err(|error| format!("Nu am putut serializa contractul imaginii Zola: {error}"))?;
    let payload = URL_SAFE_NO_PAD.encode(json);
    let p = &metadata.presentation;
    let path = serde_json::to_string(&p.source_path)
        .map_err(|error| format!("Nu am putut serializa calea imaginii: {error}"))?;
    let mut arguments = vec![format!("path={path}"), format!("width={}", p.width)];
    if p.operation.requires_height() {
        arguments.push(format!(
            "height={}",
            p.height.expect("height validated for operation")
        ));
    }
    arguments.push(format!("op=\"{}\"", p.operation.as_zola_str()));
    arguments.push(format!("format=\"{}\"", p.format.as_zola_str()));
    if p.format.uses_quality() {
        arguments.push(format!("quality={}", p.quality));
    }
    Ok(format!(
        "{MARKER_PREFIX}{payload}{MARKER_SUFFIX}{{% set {} = resize_image({}) %}}",
        metadata.variable,
        arguments.join(", ")
    ))
}

fn render_managed_opening(opening: &str, variable: &str) -> String {
    let mut next = opening.to_string();
    next = set_tag_attribute_value(&next, "src", &format!("{{{{ {variable}.url | safe }}}}"));
    next = set_tag_attribute_value(&next, "width", &format!("{{{{ {variable}.width }}}}"));
    set_tag_attribute_value(&next, "height", &format!("{{{{ {variable}.height }}}}"))
}

fn validate_managed_opening(opening: &str, variable: &str) -> Result<(), String> {
    let expected = [
        ("src", format!("{{{{ {variable}.url | safe }}}}")),
        ("width", format!("{{{{ {variable}.width }}}}")),
        ("height", format!("{{{{ {variable}.height }}}}")),
    ];
    let attributes = raw_tag_attributes(opening);
    for (name, expected_value) in expected {
        let matching: Vec<_> = attributes
            .iter()
            .filter(|attribute| attribute.name == name)
            .collect();
        if matching.len() != 1 || matching[0].value.as_deref() != Some(expected_value.as_str()) {
            return Err(format!(
                "Atributul {name} administrat de contractul imaginii Zola a fost modificat manual."
            ));
        }
    }
    Ok(())
}

fn capture_original_attributes(opening: &str) -> Vec<OriginalAttribute> {
    raw_tag_attributes(opening)
        .into_iter()
        .filter(|attribute| matches!(attribute.name.as_str(), "src" | "width" | "height"))
        .map(|attribute| OriginalAttribute {
            name: attribute.name,
            raw: attribute.raw,
        })
        .collect()
}

fn replace_original_source(metadata: &mut ZolaImageContractMetadata, source_url: &str) {
    metadata
        .original_attributes
        .retain(|attribute| attribute.name != "src");
    metadata.original_attributes.insert(
        0,
        OriginalAttribute {
            name: "src".to_string(),
            raw: format!("src=\"{}\"", escape_attr_value(source_url)),
        },
    );
}

fn restore_original_attributes(
    opening: &str,
    metadata: &ZolaImageContractMetadata,
) -> Result<String, String> {
    let mut restored = opening.to_string();
    for name in ["src", "width", "height"] {
        restored = remove_tag_attribute(&restored, name);
    }
    for attribute in &metadata.original_attributes {
        restored = insert_raw_tag_attribute(&restored, &attribute.name, &attribute.raw)?;
    }
    Ok(restored)
}

fn stable_variable(file: &str, opening_start: usize) -> String {
    let digest = Sha256::digest(format!("{file}:{opening_start}").as_bytes());
    let hex = format!("{digest:x}");
    format!("pana_image_{}", &hex[..12])
}

fn escape_attr_value(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn replace_range(source: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut next = String::with_capacity(source.len() - (end - start) + replacement.len());
    next.push_str(&source[..start]);
    next.push_str(replacement);
    next.push_str(&source[end..]);
    next
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::{
        build_project_model,
        delete_engine::{plan_html_delete, ProjectHtmlDeleteIntent},
        duplicate_engine::{plan_html_duplicate, ProjectHtmlDuplicateIntent},
        move_engine::{plan_html_move, ProjectHtmlMoveIntent, ProjectMovePosition},
    };

    use super::*;

    #[test]
    fn enables_updates_and_disables_reversible_contract() {
        let root = test_project("reversible");
        let source = "<img class=\"hero\" src='/images/hero.jpg' width=\"900\" alt=\"Hero\">\n";
        fs::write(root.join("templates/index.html"), source).unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let intent = enabled_intent(1200, ZolaImageOperation::FitWidth, None);
        let enabled =
            apply_zola_image_contract(&model, "templates/index.html", source, 0, &intent).unwrap();
        assert!(enabled.contents.contains("resize_image(path=\"static/images/hero.jpg\", width=1200, op=\"fit_width\", format=\"webp\", quality=82)"));
        assert!(enabled.contents.contains("{{ pana_image_"));
        let opening = enabled.contents.find("<img").unwrap();
        assert_eq!(
            inspect_zola_image_at(&enabled.contents, opening).unwrap(),
            enabled.presentation
        );
        let index = crate::preview::preprocess::SourceIdIndex::for_template_source(
            "templates/index.html",
            &enabled.contents,
        );
        let annotated = crate::preview::preprocess::preprocess_template(
            &enabled.contents,
            "templates/index.html",
            Some(&index),
        );
        assert!(annotated.contains("data-pana-zola-image=\""));

        let updated_intent = enabled_intent(640, ZolaImageOperation::Fill, Some(360));
        let updated = apply_zola_image_contract(
            &model,
            "templates/index.html",
            &enabled.contents,
            opening,
            &updated_intent,
        )
        .unwrap();
        assert!(updated
            .contents
            .contains("width=640, height=360, op=\"fill\""));

        let updated_opening = updated.contents.find("<img").unwrap();
        let disabled = apply_zola_image_contract(
            &model,
            "templates/index.html",
            &updated.contents,
            updated_opening,
            &ProjectZolaImageIntent {
                enabled: false,
                source_url: None,
                source_path: None,
                width: None,
                height: None,
                operation: None,
                format: None,
                quality: None,
            },
        )
        .unwrap();
        assert!(!disabled.contents.contains("pana-studio:zola-image"));
        assert!(disabled.contents.contains("src='/images/hero.jpg'"));
        assert!(disabled.contents.contains("width=\"900\""));
        assert!(disabled.contents.contains("class=\"hero\""));
        assert!(disabled.contents.contains("alt=\"Hero\""));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_external_dynamic_missing_and_ambiguous_sources() {
        let root = test_project("refusals");
        fs::write(root.join("zola.toml"), "base_url = '/'\ntheme = \"demo\"\n").unwrap();
        fs::create_dir_all(root.join("themes/demo/static/images")).unwrap();
        fs::write(root.join("themes/demo/static/images/hero.jpg"), b"theme").unwrap();
        let source = "<img src=\"/images/hero.jpg\">";
        fs::write(root.join("templates/index.html"), source).unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();

        let ambiguous = apply_zola_image_contract(
            &model,
            "templates/index.html",
            source,
            0,
            &enabled_intent(800, ZolaImageOperation::FitWidth, None),
        )
        .unwrap_err();
        assert!(ambiguous.contains("ambiguu"));

        let mut external = enabled_intent(800, ZolaImageOperation::FitWidth, None);
        external.source_url = Some("https://example.test/hero.jpg".to_string());
        assert!(
            apply_zola_image_contract(&model, "templates/index.html", source, 0, &external,)
                .unwrap_err()
                .contains("URL local")
        );

        let mut dynamic = enabled_intent(800, ZolaImageOperation::FitWidth, None);
        dynamic.source_url = Some("/{{ hero }}".to_string());
        assert!(
            apply_zola_image_contract(&model, "templates/index.html", source, 0, &dynamic,)
                .unwrap_err()
                .contains("URL local")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_manually_modified_managed_contract() {
        let root = test_project("tamper");
        let source = "<img src=\"/images/hero.jpg\">";
        fs::write(root.join("templates/index.html"), source).unwrap();
        let model = build_project_model(&root, &HashMap::new()).unwrap();
        let enabled = apply_zola_image_contract(
            &model,
            "templates/index.html",
            source,
            0,
            &enabled_intent(800, ZolaImageOperation::FitWidth, None),
        )
        .unwrap();
        let tampered = enabled.contents.replace(".url | safe", ".url");
        let opening = tampered.find("<img").unwrap();
        assert!(inspect_zola_image_at(&tampered, opening)
            .unwrap_err()
            .contains("modificat manual"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn structural_delete_move_and_duplicate_keep_contract_attached() {
        let root = test_project("structural");
        let source = concat!(
            "<main>\n",
            "  <img src=\"/images/hero.jpg\" alt=\"Hero\">\n",
            "  <p class=\"target\">Țintă</p>\n",
            "</main>\n",
        );
        fs::write(root.join("templates/index.html"), source).unwrap();
        let disk_model = build_project_model(&root, &HashMap::new()).unwrap();
        let opening = source.find("<img").unwrap();
        let enabled = apply_zola_image_contract(
            &disk_model,
            "templates/index.html",
            source,
            opening,
            &enabled_intent(800, ZolaImageOperation::FitWidth, None),
        )
        .unwrap();
        let projected = build_project_model(
            &root,
            &HashMap::from([("templates/index.html".to_string(), enabled.contents.clone())]),
        )
        .unwrap();
        let image_id = node_id(&projected, "<img>");
        let target_id = node_id(&projected, "<p .target>");

        let deleted = plan_html_delete(
            &projected,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some(image_id.clone()),
                target_location: None,
                target_tag: Some("img".to_string()),
                target_selector: None,
            },
            &HashMap::new(),
        );
        assert!(deleted.allowed, "{:?}", deleted.diagnostic);
        assert!(!deleted.patch.unwrap().contents.contains(MARKER_PREFIX));

        let moved_image = plan_html_move(
            &projected,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(image_id.clone()),
                target_source_id: Some(target_id.clone()),
                source_location: None,
                target_location: None,
                source_tag: Some("img".to_string()),
                target_tag: Some("p".to_string()),
                source_selector: None,
                target_selector: None,
                position: ProjectMovePosition::After,
            },
            &HashMap::new(),
        );
        assert!(moved_image.allowed, "{:?}", moved_image.diagnostic);
        let moved_contents = moved_image.patch.unwrap().contents;
        let moved_opening = moved_contents.find("<img").unwrap();
        assert!(inspect_zola_image_at(&moved_contents, moved_opening)
            .unwrap()
            .is_some());

        let moved_before_image = plan_html_move(
            &projected,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(target_id),
                target_source_id: Some(image_id.clone()),
                source_location: None,
                target_location: None,
                source_tag: Some("p".to_string()),
                target_tag: Some("img".to_string()),
                source_selector: None,
                target_selector: None,
                position: ProjectMovePosition::Before,
            },
            &HashMap::new(),
        );
        assert!(
            moved_before_image.allowed,
            "{:?}",
            moved_before_image.diagnostic
        );
        let before_contents = moved_before_image.patch.unwrap().contents;
        assert!(before_contents.find("<p").unwrap() < before_contents.find(MARKER_PREFIX).unwrap());
        let before_opening = before_contents.find("<img").unwrap();
        assert!(inspect_zola_image_at(&before_contents, before_opening)
            .unwrap()
            .is_some());

        let duplicated = plan_html_duplicate(
            &projected,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(image_id),
                source_location: None,
                source_tag: Some("img".to_string()),
                source_selector: None,
            },
            &HashMap::new(),
        );
        assert!(duplicated.allowed, "{:?}", duplicated.diagnostic);
        let duplicate_patch = duplicated.patch.unwrap();
        assert!(duplicate_patch.zola_image_contract);
        assert_eq!(duplicate_patch.contents.matches(MARKER_PREFIX).count(), 2);
        for image in crate::source_graph::html::parse_html_opening_tags(&duplicate_patch.contents)
            .into_iter()
            .filter(|item| item.tag == "img")
        {
            assert!(
                inspect_zola_image_at(&duplicate_patch.contents, image.start)
                    .unwrap()
                    .is_some()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    fn enabled_intent(
        width: u32,
        operation: ZolaImageOperation,
        height: Option<u32>,
    ) -> ProjectZolaImageIntent {
        ProjectZolaImageIntent {
            enabled: true,
            source_url: Some("/images/hero.jpg".to_string()),
            source_path: Some("static/images/hero.jpg".to_string()),
            width: Some(width),
            height,
            operation: Some(operation),
            format: Some(ZolaImageFormat::Webp),
            quality: Some(82),
        }
    }

    fn test_project(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-zola-image-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("static/images")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = 'Test'\ntemplate = 'index.html'\n+++\n",
        )
        .unwrap();
        fs::write(root.join("static/images/hero.jpg"), b"image").unwrap();
        root
    }

    fn node_id(model: &ProjectModel, label: &str) -> String {
        model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == label)
            .unwrap_or_else(|| {
                panic!(
                    "missing {label}; labels: {:?}",
                    model
                        .source_graph
                        .nodes
                        .iter()
                        .map(|node| node.label.as_str())
                        .collect::<Vec<_>>()
                )
            })
            .id
            .clone()
    }
}
