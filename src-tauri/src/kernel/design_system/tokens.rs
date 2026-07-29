use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

use serde::Serialize;

use crate::{
    css::variables::parse_variables_from_source, kernel::file_buffer_store::FileBufferStore,
};

pub const DESIGN_TOKEN_CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignTokenVisualKind {
    Color,
    FontFamily,
    FontSize,
    FontWeight,
    LineHeight,
    LetterSpacing,
    Spacing,
    Radius,
    Shadow,
    Transition,
    Breakpoint,
    Layout,
    Layer,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTokenSnapshot {
    pub id: String,
    pub name: String,
    pub category_id: String,
    pub group_label: String,
    pub visual_kind: DesignTokenVisualKind,
    pub raw_value: String,
    pub resolved_value: Option<String>,
    pub dependencies: Vec<String>,
    pub source_path: String,
    pub source_line: usize,
    pub editable: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTokenCategorySnapshot {
    pub id: String,
    pub label: String,
    pub token_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTokenCatalogSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub categories: Vec<DesignTokenCategorySnapshot>,
    pub tokens: Vec<DesignTokenSnapshot>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
struct ParsedToken {
    name: String,
    raw_value: String,
    source_path: String,
    source_line: usize,
    group_label: String,
    editable: bool,
}

#[derive(Clone, Debug)]
struct Resolution {
    value: Option<String>,
    dependencies: Vec<String>,
    diagnostic: Option<String>,
}

const CATEGORY_ORDER: &[(&str, &str)] = &[
    ("color", "Culori"),
    ("typography", "Tipografie"),
    ("spacing", "Spațiere"),
    ("radius", "Colțuri"),
    ("shadow", "Umbre"),
    ("transition", "Tranziții"),
    ("breakpoint", "Breakpoints"),
    ("layout", "Layout"),
    ("layer", "Straturi"),
    ("other", "Altele"),
];

pub fn build_design_token_catalog(
    project_root: &str,
    runtime_session_id: &str,
    workspace_revision: u64,
    store: &FileBufferStore,
) -> Result<DesignTokenCatalogSnapshot, String> {
    require_complete_scss_inventory(store)?;
    let sources = store
        .files
        .iter()
        .filter(|(path, _)| {
            Path::new(path)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("scss"))
        })
        .map(|(path, entry)| {
            (
                path.as_str(),
                entry.current_text(),
                !entry.baseline.readonly,
            )
        })
        .collect::<Vec<_>>();
    Ok(build_catalog_from_sources(
        project_root,
        runtime_session_id,
        workspace_revision,
        &sources,
    ))
}

fn require_complete_scss_inventory(store: &FileBufferStore) -> Result<(), String> {
    let blocking = store.diagnostics.iter().find(|diagnostic| {
        matches!(
            diagnostic.code.as_str(),
            "max_files_reached"
                | "max_total_bytes_reached"
                | "file_too_large"
                | "unsafe_project_path"
                | "unstable_during_read"
                | "read_text_failed"
        ) && diagnostic
            .relative_path
            .as_deref()
            .map(|path| {
                Path::new(path)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("scss"))
            })
            .unwrap_or(true)
    });
    if let Some(diagnostic) = blocking {
        return Err(format!(
            "[design_token_inventory_incomplete] Catalogul tokenilor SCSS este incomplet ({}): {}",
            diagnostic.code, diagnostic.message,
        ));
    }
    Ok(())
}

fn build_catalog_from_sources(
    project_root: &str,
    runtime_session_id: &str,
    workspace_revision: u64,
    sources: &[(&str, &str, bool)],
) -> DesignTokenCatalogSnapshot {
    let mut parsed = Vec::new();
    for (path, source, editable) in sources {
        parsed.extend(parse_source_tokens(path, source, *editable));
    }

    let mut indices_by_name = BTreeMap::<String, Vec<usize>>::new();
    for (index, token) in parsed.iter().enumerate() {
        indices_by_name
            .entry(token.name.clone())
            .or_default()
            .push(index);
    }

    let mut cache = vec![None; parsed.len()];
    let mut resolving = Vec::new();
    for index in 0..parsed.len() {
        resolve_token(index, &parsed, &indices_by_name, &mut cache, &mut resolving);
    }

    let mut warnings = BTreeSet::new();
    let mut tokens = Vec::with_capacity(parsed.len());
    for (index, token) in parsed.iter().enumerate() {
        let resolution = cache[index].clone().unwrap_or_else(|| Resolution {
            value: None,
            dependencies: Vec::new(),
            diagnostic: Some(format!("Tokenul ${} nu a putut fi rezolvat.", token.name)),
        });
        let duplicate_diagnostic = indices_by_name
            .get(&token.name)
            .filter(|indices| indices.len() > 1)
            .map(|indices| {
                let sources = indices
                    .iter()
                    .map(|candidate| {
                        let duplicate = &parsed[*candidate];
                        format!("{}:{}", duplicate.source_path, duplicate.source_line)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "Tokenul ${} are {} declarații în sesiune: {}.",
                    token.name,
                    indices.len(),
                    sources
                )
            });
        let diagnostic = merge_diagnostics(resolution.diagnostic, duplicate_diagnostic);
        if let Some(message) = diagnostic.as_ref() {
            warnings.insert(message.clone());
        }
        let (category_id, visual_kind) = classify_token(
            &token.group_label,
            &token.name,
            resolution.value.as_deref().unwrap_or(&token.raw_value),
        );
        tokens.push(DesignTokenSnapshot {
            id: format!(
                "{}::${}:{}",
                token.source_path, token.name, token.source_line
            ),
            name: token.name.clone(),
            category_id: category_id.to_string(),
            group_label: token.group_label.clone(),
            visual_kind,
            raw_value: token.raw_value.clone(),
            resolved_value: resolution.value,
            dependencies: resolution.dependencies,
            source_path: token.source_path.clone(),
            source_line: token.source_line,
            editable: token.editable,
            diagnostic,
        });
    }

    let categories = CATEGORY_ORDER
        .iter()
        .filter_map(|(id, label)| {
            let token_count = tokens
                .iter()
                .filter(|token| token.category_id == *id)
                .count();
            (token_count > 0).then(|| DesignTokenCategorySnapshot {
                id: (*id).to_string(),
                label: (*label).to_string(),
                token_count,
            })
        })
        .collect();

    DesignTokenCatalogSnapshot {
        schema_version: DESIGN_TOKEN_CATALOG_SCHEMA_VERSION,
        project_root: project_root.to_string(),
        runtime_session_id: runtime_session_id.to_string(),
        workspace_revision,
        categories,
        tokens,
        warnings: warnings.into_iter().collect(),
    }
}

fn parse_source_tokens(path: &str, source: &str, editable: bool) -> Vec<ParsedToken> {
    let mut tokens = Vec::new();
    let mut group_label = "Fără grup".to_string();
    let mut in_block_comment = false;

    for (line_index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }
        if let Some(section) = section_heading(trimmed) {
            group_label = section;
            continue;
        }

        let mut variables = Vec::new();
        parse_variables_from_source(line, path, &mut variables);
        let Some(variable) = variables.into_iter().next() else {
            continue;
        };
        tokens.push(ParsedToken {
            name: variable.name,
            raw_value: variable.value,
            source_path: path.to_string(),
            source_line: line_index + 1,
            group_label: group_label.clone(),
            editable,
        });
    }
    tokens
}

fn section_heading(line: &str) -> Option<String> {
    let comment = line.strip_prefix("//")?.trim();
    let explicit = comment
        .strip_prefix("Categorie:")
        .or_else(|| comment.strip_prefix("Category:"))
        .or_else(|| comment.strip_prefix("Secțiune:"))
        .or_else(|| comment.strip_prefix("Section:"));
    if let Some(label) = explicit {
        let label = label.trim();
        return (!label.is_empty()).then(|| label.to_string());
    }
    if !comment.contains('─') && !comment.starts_with("---") && !comment.starts_with("===") {
        return None;
    }
    let label = comment
        .trim_matches(|character: char| {
            character.is_whitespace() || matches!(character, '─' | '-' | '=')
        })
        .trim();
    (!label.is_empty() && label.len() <= 80).then(|| label.to_string())
}

fn resolve_token(
    index: usize,
    tokens: &[ParsedToken],
    indices_by_name: &BTreeMap<String, Vec<usize>>,
    cache: &mut [Option<Resolution>],
    resolving: &mut Vec<usize>,
) -> Resolution {
    if let Some(cached) = cache[index].clone() {
        return cached;
    }
    if let Some(cycle_start) = resolving.iter().position(|candidate| *candidate == index) {
        let mut cycle = resolving[cycle_start..]
            .iter()
            .map(|candidate| format!("${}", tokens[*candidate].name))
            .collect::<Vec<_>>();
        cycle.push(format!("${}", tokens[index].name));
        return Resolution {
            value: None,
            dependencies: Vec::new(),
            diagnostic: Some(format!(
                "Dependență circulară între tokeni: {}.",
                cycle.join(" → ")
            )),
        };
    }

    resolving.push(index);
    let token = &tokens[index];
    let references = token_references(&token.raw_value);
    let mut dependencies = BTreeSet::new();
    let mut diagnostic = None;
    let mut replacements = HashMap::<String, String>::new();

    for reference in references {
        dependencies.insert(reference.clone());
        let candidates = indices_by_name.get(&reference).cloned().unwrap_or_default();
        let same_source = candidates
            .iter()
            .copied()
            .filter(|candidate| tokens[*candidate].source_path == token.source_path)
            .collect::<Vec<_>>();
        let target = if same_source.len() == 1 {
            same_source.first().copied()
        } else if same_source.is_empty() && candidates.len() == 1 {
            candidates.first().copied()
        } else {
            None
        };
        let Some(target) = target else {
            diagnostic = Some(if candidates.is_empty() {
                format!(
                    "Tokenul ${} referă ${}, care nu există în ProjectWorkspace.",
                    token.name, reference
                )
            } else {
                format!(
                    "Tokenul ${} referă ${}, dar referința este ambiguă între {} declarații.",
                    token.name,
                    reference,
                    candidates.len()
                )
            });
            break;
        };
        let nested = resolve_token(target, tokens, indices_by_name, cache, resolving);
        if let Some(message) = nested.diagnostic {
            diagnostic = Some(format!(
                "Tokenul ${} nu poate rezolva ${}: {}",
                token.name, reference, message
            ));
            break;
        }
        for dependency in nested.dependencies {
            dependencies.insert(dependency);
        }
        let Some(value) = nested.value else {
            diagnostic = Some(format!(
                "Tokenul ${} nu poate rezolva valoarea lui ${}.",
                token.name, reference
            ));
            break;
        };
        replacements.insert(reference, value);
    }

    let value = if diagnostic.is_none() {
        Some(
            replace_token_references(&token.raw_value, &replacements)
                .trim()
                .trim_end_matches("!default")
                .trim_end_matches("!global")
                .trim()
                .to_string(),
        )
    } else {
        None
    };
    resolving.pop();

    let resolution = Resolution {
        value,
        dependencies: dependencies.into_iter().collect(),
        diagnostic,
    };
    cache[index] = Some(resolution.clone());
    resolution
}

fn token_references(value: &str) -> Vec<String> {
    let mut references = BTreeSet::new();
    scan_token_references(value, |name| {
        references.insert(name.to_string());
        None
    });
    references.into_iter().collect()
}

fn replace_token_references(value: &str, replacements: &HashMap<String, String>) -> String {
    scan_token_references(value, |name| replacements.get(name).cloned())
}

fn scan_token_references(
    value: &str,
    mut replacement: impl FnMut(&str) -> Option<String>,
) -> String {
    let bytes = value.as_bytes();
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    let mut chunk_start = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'$' {
            cursor += 1;
            continue;
        }
        let name_start = cursor + 1;
        let mut name_end = name_start;
        while name_end < bytes.len()
            && (bytes[name_end].is_ascii_alphanumeric() || matches!(bytes[name_end], b'-' | b'_'))
        {
            name_end += 1;
        }
        if name_end == name_start {
            cursor += 1;
            continue;
        }
        output.push_str(&value[chunk_start..cursor]);
        let name = &value[name_start..name_end];
        if let Some(next) = replacement(name) {
            output.push_str(&next);
        } else {
            output.push_str(&value[cursor..name_end]);
        }
        cursor = name_end;
        chunk_start = cursor;
    }
    output.push_str(&value[chunk_start..]);
    output
}

fn classify_token(
    group_label: &str,
    name: &str,
    resolved_value: &str,
) -> (&'static str, DesignTokenVisualKind) {
    let group = normalize(group_label);
    let name = normalize(name);

    if contains_any(&group, &["breakpoint"]) {
        return ("breakpoint", DesignTokenVisualKind::Breakpoint);
    }
    if contains_any(&group, &["fonturi", "fonts"]) {
        return ("typography", DesignTokenVisualKind::FontFamily);
    }
    if contains_any(
        &group,
        &["brand", "neutre", "neutral", "stari ui", "states"],
    ) {
        return ("color", DesignTokenVisualKind::Color);
    }
    if contains_any(&group, &["semantic token"]) && looks_like_color_name(&name) {
        return ("color", DesignTokenVisualKind::Color);
    }
    if contains_any(&group, &["tipografie", "typography"]) {
        return ("typography", typography_kind(&name, resolved_value));
    }
    if contains_any(&group, &["spatiere", "spacing"]) {
        return ("spacing", DesignTokenVisualKind::Spacing);
    }
    if contains_any(&group, &["border radius", "raza", "colturi", "radius"]) {
        return ("radius", DesignTokenVisualKind::Radius);
    }
    if contains_any(&group, &["umbre", "shadow"]) {
        return ("shadow", DesignTokenVisualKind::Shadow);
    }
    if contains_any(&group, &["tranzitii", "transition", "motion"]) {
        return ("transition", DesignTokenVisualKind::Transition);
    }
    if contains_any(&group, &["z-index", "straturi", "layers"]) {
        return ("layer", DesignTokenVisualKind::Layer);
    }
    if contains_any(&group, &["layout"]) {
        return ("layout", DesignTokenVisualKind::Layout);
    }

    if looks_like_color_name(&name) || is_color_value(resolved_value) {
        return ("color", DesignTokenVisualKind::Color);
    }
    if name.starts_with("radius-") || name.contains("border-radius") {
        return ("radius", DesignTokenVisualKind::Radius);
    }
    if name.starts_with("space-") || contains_any(&name, &["gap", "padding", "margin"]) {
        return ("spacing", DesignTokenVisualKind::Spacing);
    }
    if name.starts_with("shadow-") {
        return ("shadow", DesignTokenVisualKind::Shadow);
    }
    if name.starts_with("transition-") || name.starts_with("duration-") {
        return ("transition", DesignTokenVisualKind::Transition);
    }
    if name.starts_with("bp-") || name.contains("breakpoint") {
        return ("breakpoint", DesignTokenVisualKind::Breakpoint);
    }
    if name.starts_with("z-") || name.contains("z-index") {
        return ("layer", DesignTokenVisualKind::Layer);
    }
    if name.starts_with("font-")
        || name.starts_with("text-")
        || name.starts_with("leading-")
        || name.starts_with("tracking-")
    {
        return ("typography", typography_kind(&name, resolved_value));
    }
    if contains_any(&name, &["container", "grid", "column"]) {
        return ("layout", DesignTokenVisualKind::Layout);
    }
    ("other", DesignTokenVisualKind::Other)
}

fn typography_kind(name: &str, value: &str) -> DesignTokenVisualKind {
    if name.starts_with("leading-") || name.contains("line-height") {
        return DesignTokenVisualKind::LineHeight;
    }
    if name.starts_with("tracking-") || name.contains("letter-spacing") {
        return DesignTokenVisualKind::LetterSpacing;
    }
    if name.starts_with("text-") || name.contains("font-size") {
        return DesignTokenVisualKind::FontSize;
    }
    if name.contains("weight") || (name.starts_with("font-") && value.trim().parse::<u16>().is_ok())
    {
        return DesignTokenVisualKind::FontWeight;
    }
    DesignTokenVisualKind::FontFamily
}

fn looks_like_color_name(name: &str) -> bool {
    name.starts_with("color-")
        || name.starts_with("bg-")
        || name.starts_with("background-")
        || name.starts_with("text-color-")
        || name == "text-body"
        || name == "text-heading"
        || name == "text-light"
        || name == "text-inverse"
        || name.starts_with("border-color")
        || name == "border-strong"
}

fn is_color_value(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.starts_with('#')
        || value.starts_with("rgb(")
        || value.starts_with("rgba(")
        || value.starts_with("hsl(")
        || value.starts_with("hsla(")
        || value.starts_with("hwb(")
        || value.starts_with("lab(")
        || value.starts_with("lch(")
        || value.starts_with("oklab(")
        || value.starts_with("oklch(")
        || value.starts_with("color(")
        || value.starts_with("color-mix(")
        || matches!(
            value.as_str(),
            "transparent" | "currentcolor" | "black" | "white"
        )
}

fn contains_any(value: &str, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| value.contains(candidate))
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace(['ă', 'â'], "a")
        .replace('î', "i")
        .replace(['ș', 'ş'], "s")
        .replace(['ț', 'ţ'], "t")
}

fn merge_diagnostics(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first} {second}")),
        (Some(first), None) => Some(first),
        (None, Some(second)) => Some(second),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_catalog_from_sources, DesignTokenVisualKind, DESIGN_TOKEN_CATALOG_SCHEMA_VERSION,
    };

    #[test]
    fn catalog_resolves_aliases_and_uses_source_sections_as_semantics() {
        let source = r#"
// ─── Brand ───
$color-primary: #3b82f6;

// ─── Semantic tokens ───
$bg-body: $color-primary;

// ─── Tipografie fluid ───
$text-base: clamp(1rem, 2vw, 1.25rem);

// ─── Border radius ───
$radius-l: 0.75rem;
"#;
        let catalog = build_catalog_from_sources(
            "/tmp/site",
            "runtime-1",
            7,
            &[("sass/_tokens.scss", source, true)],
        );

        assert_eq!(catalog.schema_version, DESIGN_TOKEN_CATALOG_SCHEMA_VERSION);
        assert_eq!(catalog.workspace_revision, 7);
        let background = catalog
            .tokens
            .iter()
            .find(|token| token.name == "bg-body")
            .unwrap();
        assert_eq!(background.category_id, "color");
        assert_eq!(background.resolved_value.as_deref(), Some("#3b82f6"));
        assert_eq!(background.dependencies, vec!["color-primary"]);
        assert_eq!(background.source_line, 6);

        let typography = catalog
            .tokens
            .iter()
            .find(|token| token.name == "text-base")
            .unwrap();
        assert_eq!(typography.visual_kind, DesignTokenVisualKind::FontSize);

        let radius = catalog
            .tokens
            .iter()
            .find(|token| token.name == "radius-l")
            .unwrap();
        assert_eq!(radius.category_id, "radius");
        assert_eq!(radius.visual_kind, DesignTokenVisualKind::Radius);
    }

    #[test]
    fn catalog_reports_missing_and_circular_references_without_guessing() {
        let source = r#"
$missing-consumer: $does-not-exist;
$cycle-a: $cycle-b;
$cycle-b: $cycle-a;
"#;
        let catalog = build_catalog_from_sources(
            "/tmp/site",
            "runtime-1",
            1,
            &[("sass/_tokens.scss", source, true)],
        );

        let missing = catalog
            .tokens
            .iter()
            .find(|token| token.name == "missing-consumer")
            .unwrap();
        assert!(missing.resolved_value.is_none());
        assert!(missing.diagnostic.as_deref().unwrap().contains("nu există"));

        let cycle = catalog
            .tokens
            .iter()
            .find(|token| token.name == "cycle-a")
            .unwrap();
        assert!(cycle.resolved_value.is_none());
        assert!(cycle.diagnostic.as_deref().unwrap().contains("circulară"));
    }

    #[test]
    fn pana_studio_theme_tokens_map_to_the_visual_catalog_without_frontend_heuristics() {
        let source = include_str!(
            "../../../resources/theme-packs/pana-studio/theme/sass/css-framework/_variabile.scss"
        );
        let catalog = build_catalog_from_sources(
            "/tmp/site",
            "runtime-1",
            3,
            &[("sass/css-framework/_variabile.scss", source, true)],
        );

        let semantic_background = catalog
            .tokens
            .iter()
            .find(|token| token.name == "bg-body")
            .unwrap();
        assert_eq!(semantic_background.category_id, "color");
        assert_eq!(
            semantic_background.resolved_value.as_deref(),
            Some("#ffffff")
        );

        let semantic_text = catalog
            .tokens
            .iter()
            .find(|token| token.name == "text-body")
            .unwrap();
        assert_eq!(semantic_text.category_id, "color");
        assert_eq!(semantic_text.resolved_value.as_deref(), Some("#4b5563"));

        let spacing = catalog
            .tokens
            .iter()
            .find(|token| token.name == "space-m")
            .unwrap();
        assert_eq!(spacing.category_id, "spacing");

        let radius = catalog
            .tokens
            .iter()
            .find(|token| token.name == "radius-m")
            .unwrap();
        assert_eq!(radius.category_id, "radius");
        assert!(catalog.warnings.is_empty());
    }
}
