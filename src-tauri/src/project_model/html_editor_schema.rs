use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;

const HTML_EDITOR_SCHEMA_JSON: &str = include_str!("../../../src/lib/html/editor-schema.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HtmlEditorSchema {
    design_safe: DesignSafePolicy,
    tags: HashMap<String, HtmlTagCapability>,
    attributes: HashMap<String, HtmlAttributeCapability>,
    dynamic_attributes: HashMap<String, HtmlAttributeCapability>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesignSafePolicy {
    forbidden_elements: Vec<String>,
    forbidden_attributes: Vec<String>,
    forbidden_attribute_prefixes: Vec<String>,
    active_schemes: Vec<String>,
    forbidden_meta_http_equiv: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HtmlTagCapability {
    family: String,
    source_editable: bool,
    live_projectable: bool,
    preview_mode: String,
    accepts_children: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HtmlAttributeCapability {
    semantic: String,
    empty_policy: String,
    #[serde(default)]
    elements: Vec<String>,
    #[serde(default)]
    values: Vec<String>,
    source_editable: bool,
    live_projectable: bool,
    #[serde(default)]
    preview_mode: Option<String>,
}

fn schema() -> &'static HtmlEditorSchema {
    static SCHEMA: OnceLock<HtmlEditorSchema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(HTML_EDITOR_SCHEMA_JSON)
            .expect("editor-schema.json trebuie validat la testare și build")
    })
}

pub(crate) fn tag_transition_diagnostic(current_tag: &str, new_tag: &str) -> Option<String> {
    let schema = schema();
    let Some(current) = schema.tags.get(&current_tag.to_ascii_lowercase()) else {
        return Some(format!(
            "Tag-ul curent <{current_tag}> nu este declarat în schema editorului HTML."
        ));
    };
    let Some(destination) = schema.tags.get(&new_tag.to_ascii_lowercase()) else {
        return Some(format!(
            "Tag-ul nou <{new_tag}> nu este declarat în schema editorului HTML."
        ));
    };
    if !destination.source_editable || destination.preview_mode == "blocked" {
        return Some(format!(
            "Tag-ul nou <{new_tag}> este blocat de schema editorului HTML."
        ));
    }
    if !destination.live_projectable || destination.preview_mode != "live" {
        return Some(format!(
            "Tag-ul nou <{new_tag}> nu poate avea o țintă stabilă în Design Safe Preview."
        ));
    }
    if !current.accepts_children || !destination.accepts_children {
        return Some(format!(
            "Conversia structurală între <{current_tag}> și <{new_tag}> este blocată pentru elemente void."
        ));
    }
    if current.family != destination.family {
        return Some(format!(
            "Conversia structurală din <{current_tag}> în <{new_tag}> nu păstrează categoria de conținut."
        ));
    }
    None
}

pub(crate) fn is_forbidden_element(name: &str) -> bool {
    contains_ascii_case_insensitive(&schema().design_safe.forbidden_elements, name)
}

pub(crate) fn is_forbidden_attribute_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    contains_ascii_case_insensitive(&schema().design_safe.forbidden_attributes, &normalized)
        || schema()
            .design_safe
            .forbidden_attribute_prefixes
            .iter()
            .any(|prefix| normalized.starts_with(prefix))
}

pub(crate) fn is_forbidden_meta_http_equiv(value: &str) -> bool {
    contains_ascii_case_insensitive(
        &schema().design_safe.forbidden_meta_http_equiv,
        value.trim(),
    )
}

pub(crate) fn has_active_script_scheme(value: &str) -> bool {
    let mut scheme = String::new();
    for character in value.chars() {
        if character == ':' {
            return contains_ascii_case_insensitive(&schema().design_safe.active_schemes, &scheme);
        }
        if character.is_ascii_whitespace() || character.is_ascii_control() {
            continue;
        }
        if character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.') {
            scheme.push(character.to_ascii_lowercase());
            continue;
        }
        return false;
    }
    false
}

pub(crate) fn validate_visual_attribute_mutation(
    tag: &str,
    name: &str,
    value: Option<&str>,
) -> Result<(), String> {
    // Removing an authored unsafe/unknown attribute is always a safe source
    // operation. Setting it must pass the shared editor schema below.
    let Some(value) = value else {
        return Ok(());
    };
    let normalized_name = name.trim().to_ascii_lowercase();
    let normalized_tag = tag.trim().to_ascii_lowercase();
    let capability = attribute_capability(&normalized_name).ok_or_else(|| {
        format!("Atributul {normalized_name} nu este declarat în schema editorului HTML.")
    })?;
    if !capability.source_editable {
        return Err(format!(
            "Atributul {normalized_name} este blocat de schema editorului HTML."
        ));
    }
    if !capability.elements.is_empty()
        && !capability
            .elements
            .iter()
            .any(|element| element.eq_ignore_ascii_case(&normalized_tag))
    {
        return Err(format!(
            "Atributul {normalized_name} nu se aplică elementului <{normalized_tag}>."
        ));
    }
    if value.is_empty() && capability.empty_policy == "remove" {
        return Err(format!(
            "Atributul {normalized_name} cere RemoveAttribute pentru valoarea goală."
        ));
    }
    if has_active_script_scheme(value) {
        return Err(format!(
            "Atributul {normalized_name} a refuzat o schemă URL activă."
        ));
    }

    let trimmed = value.trim();
    let normalized_value = trimmed.to_ascii_lowercase();
    match capability.semantic.as_str() {
        "booleanPresence" => {
            if !value.is_empty() && !normalized_value.eq_ignore_ascii_case(&normalized_name) {
                return Err(format!(
                    "Atributul boolean {normalized_name} acceptă forma minimizată sau numele propriu."
                ));
            }
        }
        "ariaBoolean" => {
            if !matches!(normalized_value.as_str(), "true" | "false") {
                return Err(format!(
                    "Atributul {normalized_name} acceptă explicit doar true sau false."
                ));
            }
        }
        "enumerated" => {
            if !capability
                .values
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(&normalized_value))
            {
                return Err(format!(
                    "Atributul {normalized_name} are valoare enumerată invalidă: {value}."
                ));
            }
        }
        "integer" => validate_integer(&normalized_name, trimmed, false, false)?,
        "nonNegativeInteger" => validate_integer(&normalized_name, trimmed, true, false)?,
        "positiveInteger" => validate_integer(&normalized_name, trimmed, true, true)?,
        "number" => validate_number(&normalized_name, trimmed)?,
        "numberOrAny" if normalized_value != "any" => validate_number(&normalized_name, trimmed)?,
        "numberList" => {
            if trimmed
                .split(',')
                .map(str::trim)
                .any(|part| part.is_empty() || part.parse::<f64>().is_err())
            {
                return Err(format!(
                    "Atributul {normalized_name} cere o listă de numere separată prin virgulă."
                ));
            }
        }
        "script" => {
            return Err(format!(
                "Atributul activ {normalized_name} este blocat de schema editorului HTML."
            ));
        }
        _ => {}
    }
    Ok(())
}

/// Reports whether an authored attribute mutation can be accelerated directly
/// in the Design Safe DOM. Source-only attributes remain valid canonical
/// ProjectWorkspace edits, but must wait for the sanitized Zola projection.
pub(crate) fn is_live_projectable_attribute(name: &str) -> bool {
    let normalized_name = name.trim().to_ascii_lowercase();
    let Some(capability) = attribute_capability(&normalized_name) else {
        return false;
    };
    capability.live_projectable && capability.preview_mode.as_deref().unwrap_or("live") == "live"
}

fn attribute_capability(name: &str) -> Option<&'static HtmlAttributeCapability> {
    let schema = schema();
    if let Some(capability) = schema.attributes.get(name) {
        return Some(capability);
    }
    if name.starts_with("data-") && name.len() > 5 {
        return schema.dynamic_attributes.get("data-*");
    }
    if name.starts_with("aria-") && name.len() > 5 {
        return schema.dynamic_attributes.get("aria-*");
    }
    if name.starts_with("on") && name.len() > 2 {
        return schema.dynamic_attributes.get("on*");
    }
    None
}

fn validate_integer(
    name: &str,
    value: &str,
    non_negative: bool,
    positive: bool,
) -> Result<(), String> {
    let parsed = value
        .parse::<i64>()
        .map_err(|_| format!("Atributul {name} cere un număr întreg."))?;
    if (non_negative && parsed < 0) || (positive && parsed == 0) {
        return Err(format!(
            "Atributul {name} are valoare numerică în afara intervalului."
        ));
    }
    Ok(())
}

fn validate_number(name: &str, value: &str) -> Result<(), String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("Atributul {name} cere o valoare numerică."))?;
    if !parsed.is_finite() {
        return Err(format!("Atributul {name} cere o valoare numerică finită."));
    }
    Ok(())
}

fn contains_ascii_case_insensitive(values: &[String], candidate: &str) -> bool {
    values
        .iter()
        .any(|value| value.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_schema_is_valid_and_covers_design_safe_contract() {
        let schema = schema();
        assert!(schema.tags.len() > 50);
        assert!(is_forbidden_element("IFRAME"));
        assert!(is_forbidden_attribute_name("onclick"));
        assert!(is_forbidden_attribute_name("target"));
        assert!(has_active_script_scheme(" java\nscript:alert(1)"));
        assert!(is_forbidden_meta_http_equiv("Refresh"));
        assert!(is_live_projectable_attribute("href"));
        assert!(is_live_projectable_attribute("aria-label"));
        assert!(!is_live_projectable_attribute("download"));
        assert!(!is_live_projectable_attribute("target"));
        assert!(!is_live_projectable_attribute("onclick"));
        assert!(!is_live_projectable_attribute("atribut-necunoscut"));
    }

    #[test]
    fn shared_schema_allows_only_same_family_live_tag_transitions() {
        assert!(tag_transition_diagnostic("section", "article").is_none());
        assert!(tag_transition_diagnostic("ul", "section").is_some());
        assert!(tag_transition_diagnostic("div", "iframe").is_some());
        assert!(tag_transition_diagnostic("div", "img").is_some());
        assert!(tag_transition_diagnostic("a", "button").is_some());
    }
}
