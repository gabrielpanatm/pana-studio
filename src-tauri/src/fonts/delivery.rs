use serde::Serialize;

use crate::localization::LocalizedDiagnostic;

use super::{
    managed_font_end_marker, managed_font_start_marker, normalize_font_family_name, FontFaceFamily,
    FontFaceIssueSeverity, FontRoleAssignment, LocalFontFile,
};

const PRELOAD_START: &str = "<!-- pana-studio-font-preload:start -->";
const PRELOAD_END: &str = "<!-- pana-studio-font-preload:end -->";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FontDisplayMode {
    Auto,
    Block,
    Swap,
    Fallback,
    Optional,
}

impl FontDisplayMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "block" => Ok(Self::Block),
            "swap" => Ok(Self::Swap),
            "fallback" => Ok(Self::Fallback),
            "optional" => Ok(Self::Optional),
            _ => Err(format!(
                "Valoare font-display necunoscută: {value}. Folosește auto, block, swap, fallback sau optional."
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Block => "block",
            Self::Swap => "swap",
            Self::Fallback => "fallback",
            Self::Optional => "optional",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontPreloadRegistration {
    pub preloaded: bool,
    pub managed: bool,
    pub templates: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontDeliveryDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontDeliveryDiagnostic {
    pub severity: FontDeliveryDiagnosticSeverity,
    pub code: String,
    pub message_diagnostic: LocalizedDiagnostic,
    pub family: Option<String>,
    pub file: Option<String>,
}

pub fn annotate_font_preloads<'a>(
    mut families: Vec<FontFaceFamily>,
    sources: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<FontFaceFamily> {
    let templates = sources
        .filter(|(path, _)| is_template_path(path))
        .collect::<Vec<_>>();

    for family in &mut families {
        for file in &mut family.files {
            let public_url = public_font_url(&file.file);
            let mut paths = Vec::new();
            let mut managed = false;
            for (path, source) in &templates {
                let links = font_preload_hrefs(source);
                if links
                    .iter()
                    .any(|href| normalize_public_href(href) == public_url)
                {
                    paths.push((*path).to_string());
                    managed |= managed_preload_source(source).ok().flatten().is_some_and(
                        |managed_source| {
                            font_preload_hrefs(managed_source)
                                .iter()
                                .any(|href| normalize_public_href(href) == public_url)
                        },
                    );
                }
            }
            paths.sort();
            paths.dedup();
            file.preload = FontPreloadRegistration {
                preloaded: !paths.is_empty(),
                managed,
                templates: paths,
            };
        }
    }

    families
}

pub fn prepare_font_display_update<'a>(
    sources: impl Iterator<Item = (&'a str, &'a str)>,
    family: &str,
    display: FontDisplayMode,
) -> Result<(String, String), String> {
    let marker = managed_font_start_marker(family);
    let matches = sources
        .filter(|(_, source)| source.contains(&marker))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(format!(
            "Familia {family} nu are un bloc @font-face gestionat de Pană Studio."
        ));
    }
    if matches.len() > 1 {
        return Err(format!(
            "Familia {family} are blocuri gestionate în {} stylesheet-uri; modificarea este blocată pentru a evita surse divergente.",
            matches.len()
        ));
    }
    let (path, source) = matches[0];
    Ok((
        path.to_string(),
        update_managed_font_display(source, family, display)?,
    ))
}

pub fn select_font_preload_template<'a>(
    sources: impl Iterator<Item = (&'a str, &'a str)>,
) -> Option<String> {
    sources
        .filter(|(path, source)| is_template_path(path) && contains_ascii_case(source, "</head>"))
        .map(|(path, source)| {
            let normalized = path.replace('\\', "/");
            let local = !normalized.starts_with("themes/") && !normalized.contains("/themes/");
            let score = if source.contains(PRELOAD_START) {
                0
            } else if local && normalized == "templates/base.html" {
                10
            } else if local && normalized.ends_with("/templates/base.html") {
                20
            } else if local
                && (normalized.ends_with("/layout.html") || normalized.ends_with("/_layout.html"))
            {
                30
            } else if local {
                40
            } else if normalized.ends_with("/templates/base.html") {
                60
            } else {
                80
            };
            (score, normalized.len(), normalized)
        })
        .min()
        .map(|(_, _, path)| path)
}

pub fn prepare_font_preload_update(
    source: &str,
    graph_families: &[FontFaceFamily],
    target_file: &str,
    enabled: bool,
) -> Result<String, String> {
    let target = graph_families
        .iter()
        .flat_map(|family| family.files.iter())
        .find(|file| file.file == target_file)
        .ok_or_else(|| format!("Fișierul {target_file} nu mai există în FontFaceGraph."))?;
    if target.preload.preloaded && !target.preload.managed {
        return Err(format!(
            "{} este preîncărcat prin cod extern blocului gestionat. Pană Studio nu îl poate modifica fără să preia autoritatea asupra acelui cod.",
            target.file_name
        ));
    }

    let managed_source = managed_preload_source(source)?;
    let managed_hrefs = managed_source.map(font_preload_hrefs).unwrap_or_default();
    let mut selected = graph_families
        .iter()
        .flat_map(|family| family.files.iter())
        .filter(|file| {
            let url = public_font_url(&file.file);
            managed_hrefs
                .iter()
                .any(|href| normalize_public_href(href) == url)
        })
        .cloned()
        .collect::<Vec<_>>();

    selected.retain(|file| file.file != target_file);
    if enabled {
        selected.push(target.clone());
    }
    selected.sort_by(|left, right| left.file.cmp(&right.file));
    selected.dedup_by(|left, right| left.file == right.file);
    upsert_font_preload_block(source, &selected)
}

pub fn font_delivery_diagnostics(
    families: &[FontFaceFamily],
    roles: &[FontRoleAssignment],
) -> Vec<FontDeliveryDiagnostic> {
    let active_families = roles
        .iter()
        .filter_map(|role| role.family.as_deref())
        .map(normalize_font_family_name)
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut preload_count = 0usize;

    for family in families {
        for issue in &family.issues {
            let severity = match issue.severity {
                FontFaceIssueSeverity::Info => FontDeliveryDiagnosticSeverity::Info,
                FontFaceIssueSeverity::Warning => FontDeliveryDiagnosticSeverity::Warning,
                FontFaceIssueSeverity::Error => FontDeliveryDiagnosticSeverity::Error,
            };
            diagnostics.push(FontDeliveryDiagnostic {
                severity,
                code: issue.code.clone(),
                message_diagnostic: LocalizedDiagnostic::new("font-face-graph-issue")
                    .with_argument("message", issue.message.clone()),
                family: Some(family.family.clone()),
                file: issue.file.clone(),
            });
        }
        if !family.registration.registered {
            if family.issues.is_empty() {
                diagnostics.push(FontDeliveryDiagnostic {
                    severity: FontDeliveryDiagnosticSeverity::Error,
                    code: "font_face_missing".to_string(),
                    message_diagnostic: LocalizedDiagnostic::new("font-delivery-face-missing")
                        .with_argument("family", family.family.clone()),
                    family: Some(family.family.clone()),
                    file: None,
                });
            }
        } else if family.registration.display_modes.is_empty() {
            diagnostics.push(FontDeliveryDiagnostic {
                severity: FontDeliveryDiagnosticSeverity::Warning,
                code: "font_display_missing".to_string(),
                message_diagnostic: LocalizedDiagnostic::new("font-delivery-display-missing")
                    .with_argument("family", family.family.clone()),
                family: Some(family.family.clone()),
                file: None,
            });
        } else if family.registration.display_modes.len() > 1 {
            diagnostics.push(FontDeliveryDiagnostic {
                severity: FontDeliveryDiagnosticSeverity::Warning,
                code: "font_display_mixed".to_string(),
                message_diagnostic: LocalizedDiagnostic::new("font-delivery-display-mixed")
                    .with_argument("family", family.family.clone())
                    .with_argument("modes", family.registration.display_modes.join(", ")),
                family: Some(family.family.clone()),
                file: None,
            });
        }

        let active = active_families.contains(&normalize_font_family_name(&family.family));
        for file in &family.files {
            if !file.preload.preloaded {
                continue;
            }
            preload_count += 1;
            if !active {
                diagnostics.push(FontDeliveryDiagnostic {
                    severity: FontDeliveryDiagnosticSeverity::Warning,
                    code: "preload_unused_family".to_string(),
                    message_diagnostic: LocalizedDiagnostic::new(
                        "font-delivery-preload-unused-family",
                    )
                    .with_argument("file", file.file_name.clone())
                    .with_argument("family", family.family.clone()),
                    family: Some(family.family.clone()),
                    file: Some(file.file.clone()),
                });
            }
            if file.extension != "woff2" {
                diagnostics.push(FontDeliveryDiagnostic {
                    severity: FontDeliveryDiagnosticSeverity::Info,
                    code: "preload_non_woff2".to_string(),
                    message_diagnostic: LocalizedDiagnostic::new("font-delivery-preload-non-woff2")
                        .with_argument("file", file.file_name.clone())
                        .with_argument("format", file.extension.to_ascii_uppercase()),
                    family: Some(family.family.clone()),
                    file: Some(file.file.clone()),
                });
            }
        }
    }

    if preload_count > 3 {
        diagnostics.push(FontDeliveryDiagnostic {
            severity: FontDeliveryDiagnosticSeverity::Warning,
            code: "preload_budget_exceeded".to_string(),
            message_diagnostic: LocalizedDiagnostic::new("font-delivery-preload-budget-exceeded")
                .with_argument("count", preload_count as u64),
            family: None,
            file: None,
        });
    }
    diagnostics
}

fn update_managed_font_display(
    source: &str,
    family: &str,
    display: FontDisplayMode,
) -> Result<String, String> {
    let start = managed_font_start_marker(family);
    let end = managed_font_end_marker(family);
    let start_index = source
        .find(&start)
        .ok_or_else(|| format!("Markerul de început pentru blocul gestionat {family} lipsește."))?;
    let body_start = start_index + start.len();
    let relative_end = source[body_start..]
        .find(&end)
        .ok_or_else(|| format!("Markerul de sfârșit pentru blocul gestionat {family} lipsește."))?;
    let body_end = body_start + relative_end;
    let body = &source[body_start..body_end];
    let mut changed = 0usize;
    let mut updated = String::with_capacity(body.len());
    for line in body.split_inclusive('\n') {
        let without_newline = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = without_newline.trim_start();
        if trimmed.to_ascii_lowercase().starts_with("font-display:") {
            let indent = &without_newline[..without_newline.len() - trimmed.len()];
            updated.push_str(indent);
            updated.push_str("font-display: ");
            updated.push_str(display.as_str());
            updated.push(';');
            changed += 1;
        } else {
            updated.push_str(without_newline);
        }
        if line.ends_with('\n') {
            updated.push('\n');
        }
    }
    if changed == 0 {
        return Err(format!(
            "Blocul gestionat pentru {family} nu conține nicio declarație font-display."
        ));
    }
    Ok(format!(
        "{}{}{}",
        &source[..body_start],
        updated,
        &source[body_end..]
    ))
}

fn upsert_font_preload_block(source: &str, files: &[LocalFontFile]) -> Result<String, String> {
    let existing_start = source.find(PRELOAD_START);
    let existing_range = if let Some(start) = existing_start {
        let search_start = start + PRELOAD_START.len();
        let relative_end = source[search_start..].find(PRELOAD_END).ok_or_else(|| {
            "Blocul preload are marker de început fără marker de sfârșit.".to_string()
        })?;
        Some((start, search_start + relative_end + PRELOAD_END.len()))
    } else {
        if source.contains(PRELOAD_END) {
            return Err("Blocul preload are marker de sfârșit fără marker de început.".to_string());
        }
        None
    };

    if files.is_empty() {
        let Some((start, end)) = existing_range else {
            return Ok(source.to_string());
        };
        let mut removal_end = end;
        if source[removal_end..].starts_with('\n') {
            removal_end += 1;
        }
        return Ok(format!("{}{}", &source[..start], &source[removal_end..]));
    }

    let indent = existing_range
        .map(|(start, _)| line_indent_at(source, start))
        .unwrap_or_else(|| {
            find_ascii_case(source, "</head>")
                .map(|index| line_indent_at(source, index))
                .unwrap_or_default()
        });
    let block = build_font_preload_block(files, &indent);
    if let Some((start, end)) = existing_range {
        return Ok(format!("{}{}{}", &source[..start], block, &source[end..]));
    }

    let head_end = find_ascii_case(source, "</head>").ok_or_else(|| {
        "Template-ul ales pentru preload nu conține un element </head>.".to_string()
    })?;
    let prefix = &source[..head_end];
    let separator = if prefix.ends_with('\n') { "" } else { "\n" };
    Ok(format!(
        "{prefix}{separator}{block}\n{}",
        &source[head_end..]
    ))
}

fn build_font_preload_block(files: &[LocalFontFile], indent: &str) -> String {
    let mut lines = vec![format!("{indent}{PRELOAD_START}")];
    for file in files {
        lines.push(format!(
            "{indent}<link rel=\"preload\" href=\"{}\" as=\"font\" type=\"{}\" crossorigin>",
            public_font_url(&file.file),
            font_mime_type(file)
        ));
    }
    lines.push(format!("{indent}{PRELOAD_END}"));
    lines.join("\n")
}

fn managed_preload_source(source: &str) -> Result<Option<&str>, String> {
    let Some(start) = source.find(PRELOAD_START) else {
        if source.contains(PRELOAD_END) {
            return Err("Blocul preload are marker de sfârșit fără marker de început.".to_string());
        }
        return Ok(None);
    };
    let body_start = start + PRELOAD_START.len();
    let relative_end = source[body_start..].find(PRELOAD_END).ok_or_else(|| {
        "Blocul preload are marker de început fără marker de sfârșit.".to_string()
    })?;
    Ok(Some(&source[body_start..body_start + relative_end]))
}

fn font_preload_hrefs(source: &str) -> Vec<String> {
    html_link_tags(source)
        .into_iter()
        .filter(|tag| {
            html_attribute(tag, "rel").is_some_and(|value| {
                value
                    .split_ascii_whitespace()
                    .any(|token| token.eq_ignore_ascii_case("preload"))
            }) && html_attribute(tag, "as").is_some_and(|value| value.eq_ignore_ascii_case("font"))
        })
        .filter_map(|tag| html_attribute(tag, "href"))
        .collect()
}

fn html_link_tags(source: &str) -> Vec<&str> {
    let lower = source.to_ascii_lowercase();
    let mut cursor = 0usize;
    let mut tags = Vec::new();
    while let Some(relative_start) = lower[cursor..].find("<link") {
        let start = cursor + relative_start;
        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let end = start + relative_end + 1;
        let comment_start = lower[..start].rfind("<!--");
        let comment_end = lower[..start].rfind("-->");
        if comment_start.is_some() && comment_start > comment_end {
            cursor = end;
            continue;
        }
        tags.push(&source[start..end]);
        cursor = end;
    }
    tags
}

fn html_attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let needle = name.as_bytes();
    let mut cursor = 0usize;
    while cursor + needle.len() <= bytes.len() {
        let relative = lower[cursor..].find(name)?;
        let start = cursor + relative;
        let before_ok = start == 0
            || bytes[start - 1].is_ascii_whitespace()
            || matches!(bytes[start - 1], b'<' | b'/');
        let after = start + needle.len();
        let after_ok =
            after == bytes.len() || bytes[after].is_ascii_whitespace() || bytes[after] == b'=';
        if !before_ok || !after_ok {
            cursor = after;
            continue;
        }
        let mut value_start = after;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        if value_start >= bytes.len() || bytes[value_start] != b'=' {
            return Some(String::new());
        }
        value_start += 1;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        if value_start >= bytes.len() {
            return None;
        }
        let quote = bytes[value_start];
        if matches!(quote, b'\'' | b'"') {
            value_start += 1;
            let relative_end = tag[value_start..].find(quote as char)?;
            return Some(tag[value_start..value_start + relative_end].to_string());
        }
        let value_end = bytes[value_start..]
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || *byte == b'>')
            .map(|offset| value_start + offset)
            .unwrap_or(bytes.len());
        return Some(tag[value_start..value_end].to_string());
    }
    None
}

fn public_font_url(project_relative_file: &str) -> String {
    let normalized = project_relative_file.replace('\\', "/");
    let static_relative = normalized
        .strip_prefix("static/")
        .or_else(|| normalized.split_once("/static/").map(|(_, path)| path))
        .unwrap_or(normalized.as_str());
    format!("/{}", static_relative.trim_start_matches('/'))
}

fn normalize_public_href(href: &str) -> String {
    let href = href.split(['?', '#']).next().unwrap_or(href).trim();
    if href.starts_with('/') {
        href.to_string()
    } else {
        format!("/{}", href.trim_start_matches("./"))
    }
}

fn font_mime_type(file: &LocalFontFile) -> &'static str {
    match file.extension.as_str() {
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    }
}

fn is_template_path(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".html")
}

fn contains_ascii_case(source: &str, needle: &str) -> bool {
    find_ascii_case(source, needle).is_some()
}

fn find_ascii_case(source: &str, needle: &str) -> Option<usize> {
    source
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn line_indent_at(source: &str, index: usize) -> String {
    let line_start = source[..index].rfind('\n').map_or(0, |offset| offset + 1);
    source[line_start..index]
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{
        FontCssRegistration, FontLicenseMetadata, FontOrigin, FontVariationAxis, FontWeightRange,
    };

    fn file(path: &str) -> LocalFontFile {
        LocalFontFile {
            file: path.to_string(),
            file_name: path.split('/').next_back().unwrap_or(path).to_string(),
            size_bytes: 100,
            extension: "woff2".to_string(),
            format: "woff2".to_string(),
            text_optimized: false,
            content_hash: "hash".to_string(),
            internal_family: Some("Geist".to_string()),
            subfamily: Some("Regular".to_string()),
            weight: Some(400),
            weight_range: None::<FontWeightRange>,
            style: Some("normal".to_string()),
            axes: Vec::<FontVariationAxis>::new(),
            license: FontLicenseMetadata::default(),
            unicode_range: None,
            romanian_glyphs: crate::fonts::ROMANIAN_GLYPHS.to_vec(),
            declared_weight: Some(400),
            declared_weight_range: None,
            declared_style: Some("normal".to_string()),
            preload: FontPreloadRegistration::default(),
        }
    }

    fn family(font_file: LocalFontFile) -> FontFaceFamily {
        FontFaceFamily {
            id: "css:geist".to_string(),
            family: "Geist".to_string(),
            directories: vec!["static/fonturi/geist".to_string()],
            origin: FontOrigin::Local,
            theme_name: None,
            delivery: crate::fonts::FontDeliveryKind::Local,
            ownership: crate::fonts::FontOwnership::Managed,
            romanian_supported: Some(true),
            files: vec![font_file],
            faces: Vec::new(),
            issues: Vec::new(),
            license: FontLicenseMetadata::default(),
            registration: FontCssRegistration::default(),
        }
    }

    #[test]
    fn managed_preload_is_inserted_before_head_end_and_removed_cleanly() {
        let font_file = file("static/fonturi/geist/geist-400.woff2");
        let source = "<html>\n  <head>\n  </head>\n</html>\n";
        let inserted = prepare_font_preload_update(
            source,
            &[family(font_file.clone())],
            &font_file.file,
            true,
        )
        .expect("insert preload");
        assert!(inserted.contains("href=\"/fonturi/geist/geist-400.woff2\""));
        assert!(inserted.find(PRELOAD_START) < inserted.find("</head>"));

        let graph_families = annotate_font_preloads(
            vec![family(font_file.clone())],
            [("templates/base.html", inserted.as_str())].into_iter(),
        );
        let removed =
            prepare_font_preload_update(&inserted, &graph_families, &font_file.file, false)
                .expect("remove preload");
        assert!(!removed.contains(PRELOAD_START));
        assert!(!removed.contains("geist-400.woff2"));
    }

    #[test]
    fn font_display_update_only_changes_managed_family_block() {
        let source = "/* pana-studio-font:geist:start */\n@font-face {\n  font-family: 'Geist';\n  font-display: swap;\n}\n/* pana-studio-font:geist:end */\n";
        let updated = update_managed_font_display(source, "Geist", FontDisplayMode::Optional)
            .expect("update display");
        assert!(updated.contains("font-display: optional;"));
        assert!(!updated.contains("font-display: swap;"));
    }

    #[test]
    fn template_selection_prefers_local_base_template() {
        let sources = [
            (
                "themes/demo/templates/base.html",
                "<html><head></head></html>",
            ),
            ("templates/index.html", "<html><head></head></html>"),
            ("templates/base.html", "<html><head></head></html>"),
        ];
        assert_eq!(
            select_font_preload_template(sources.into_iter()).as_deref(),
            Some("templates/base.html")
        );
    }
}
