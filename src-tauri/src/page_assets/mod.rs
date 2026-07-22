use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    css::page::{page_css_href, page_scss_relative_path, remove_page_stylesheet_link},
    js::{PageJsConfig, PanaComponent},
    zola_links::template_contains_asset_path,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageAssetContractRequest {
    pub template_path: String,
    pub template_source: String,
    pub stylesheet_source: Option<String>,
    pub stylesheet_known: Option<bool>,
    pub page_js_config: Option<PageJsConfig>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageAssetContractTextPlan {
    pub changed: bool,
    pub contents: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageAssetContractPlan {
    pub template_path: String,
    pub stylesheet_path: String,
    pub stylesheet_href: String,
    pub active_data_anim_ids: Vec<String>,
    pub active_generated_classes: Vec<String>,
    pub template: PageAssetContractTextPlan,
    pub stylesheet: PageAssetContractTextPlan,
    pub page_js_config: PageJsConfig,
    pub page_js_changed: bool,
    pub diagnostics: Vec<String>,
}

pub fn plan_page_asset_contract(request: PageAssetContractRequest) -> PageAssetContractPlan {
    let template_path = normalize_template_path(&request.template_path);
    let stylesheet_rel = page_scss_relative_path(&template_path);
    let stylesheet_path = to_project_relative_path(&stylesheet_rel);
    let stylesheet_href = page_css_href(&template_path);
    let mut diagnostics = Vec::new();

    let active_data_anim_set = data_anim_ids_in_source(&request.template_source, &mut diagnostics);
    let active_classes = class_tokens_in_source(&request.template_source, &mut diagnostics);
    let active_generated_classes = active_classes
        .iter()
        .filter(|class_name| is_generated_pana_class(class_name))
        .cloned()
        .collect::<Vec<_>>();

    let stylesheet_source = request.stylesheet_source.unwrap_or_default();
    let stylesheet_known = request.stylesheet_known.unwrap_or_else(|| {
        !stylesheet_source.trim().is_empty()
            || template_contains_asset_path(&request.template_source, &stylesheet_href)
    });
    let next_stylesheet = remove_stale_generated_css_rules(&stylesheet_source, &active_classes);
    let has_effective_stylesheet_rules = has_effective_css_rules(&next_stylesheet);
    let next_template = if !has_effective_stylesheet_rules
        && (stylesheet_known
            || template_contains_asset_path(&request.template_source, &stylesheet_href))
    {
        remove_page_stylesheet_link(&request.template_source, &stylesheet_href)
    } else {
        request.template_source.clone()
    };

    let current_page_js = normalize_page_js_config(request.page_js_config.unwrap_or_default());
    let next_page_js = reconcile_page_js_motion(&current_page_js, &active_data_anim_set);

    PageAssetContractPlan {
        template_path,
        stylesheet_path,
        stylesheet_href,
        active_data_anim_ids: active_data_anim_set.into_iter().collect(),
        active_generated_classes,
        template: PageAssetContractTextPlan {
            changed: next_template != request.template_source,
            contents: next_template,
        },
        stylesheet: PageAssetContractTextPlan {
            changed: next_stylesheet != stylesheet_source,
            contents: next_stylesheet,
        },
        page_js_changed: next_page_js != current_page_js,
        page_js_config: next_page_js,
        diagnostics,
    }
}

fn normalize_template_path(path: &str) -> String {
    path.trim().trim_start_matches('/').to_string()
}

fn to_project_relative_path(path: &str) -> String {
    path.to_string()
}

fn attribute_values(
    source: &str,
    attribute_name: &str,
    diagnostics: &mut Vec<String>,
) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0;

    while let Some(relative_attr) = source[cursor..].find(attribute_name) {
        let attr_pos = cursor + relative_attr;
        cursor = attr_pos + attribute_name.len();
        if !is_attribute_name_at(source, attr_pos, attribute_name) {
            continue;
        }
        let Some((value, end)) = read_quoted_attribute_value(source, cursor) else {
            diagnostics.push(format!("Atribut {attribute_name} fără valoare citibilă."));
            continue;
        };
        cursor = end;
        values.push(value);
    }

    values
}

fn is_attribute_name_at(source: &str, attr_pos: usize, attr: &str) -> bool {
    let before_ok = source[..attr_pos]
        .chars()
        .last()
        .map(|character| !is_attribute_name_character(character))
        .unwrap_or(true);
    let after_pos = attr_pos + attr.len();
    let after_ok = source[after_pos..]
        .chars()
        .next()
        .map(|character| character.is_ascii_whitespace() || character == '=')
        .unwrap_or(false);
    before_ok && after_ok
}

fn is_attribute_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':')
}

fn read_quoted_attribute_value(source: &str, after_name: usize) -> Option<(String, usize)> {
    let mut cursor = skip_ascii_whitespace(source, after_name);
    if source[cursor..].chars().next()? != '=' {
        return None;
    }
    cursor += 1;
    cursor = skip_ascii_whitespace(source, cursor);
    let quote = source[cursor..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = cursor + quote.len_utf8();
    let value_end = source[value_start..].find(quote)? + value_start;
    Some((
        source[value_start..value_end].to_string(),
        value_end + quote.len_utf8(),
    ))
}

fn skip_ascii_whitespace(source: &str, mut cursor: usize) -> usize {
    while let Some(character) = source[cursor..].chars().next() {
        if !character.is_ascii_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

fn data_anim_ids_in_source(source: &str, diagnostics: &mut Vec<String>) -> BTreeSet<String> {
    attribute_values(source, "data-anim", diagnostics)
        .into_iter()
        .filter_map(|value| non_empty_trimmed(value.as_str()))
        .collect()
}

fn class_tokens_in_source(source: &str, diagnostics: &mut Vec<String>) -> BTreeSet<String> {
    let mut classes = BTreeSet::new();
    for value in attribute_values(source, "class", diagnostics) {
        for token in value.split_whitespace() {
            if let Some(class_name) = non_empty_trimmed(token) {
                classes.insert(class_name);
            }
        }
    }
    classes
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn is_generated_pana_class(class_name: &str) -> bool {
    let class_name = class_name.trim().to_ascii_lowercase();
    if !class_name.starts_with("ps-") {
        return false;
    }
    if !class_name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return false;
    }
    let Some(last_dash) = class_name.rfind('-') else {
        return false;
    };
    if last_dash <= "ps-".len() {
        return false;
    }
    let suffix = &class_name[last_dash + 1..];
    suffix.len() >= 6
        && suffix
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

pub fn remove_stale_generated_css_rules(source: &str, active_classes: &BTreeSet<String>) -> String {
    normalize_cleaned_css(
        &remove_stale_generated_css_rules_inner(source, active_classes),
        source,
    )
}

fn remove_stale_generated_css_rules_inner(
    source: &str,
    active_classes: &BTreeSet<String>,
) -> String {
    let mut result = String::new();
    let mut cursor = 0;

    while cursor < source.len() {
        let Some(relative_open) = source[cursor..].find('{') else {
            break;
        };
        let open_brace = cursor + relative_open;
        let selector_start = selector_start_before(source, cursor, open_brace);
        let selector = source[selector_start..open_brace].trim();
        let Some(close_brace) = find_matching_brace(source, open_brace) else {
            break;
        };

        if selector.starts_with('@') {
            let inner = &source[open_brace + 1..close_brace];
            let cleaned_inner = remove_stale_generated_css_rules(inner, active_classes);
            if has_effective_css_rules(&cleaned_inner) {
                result.push_str(&source[cursor..open_brace + 1]);
                result.push_str(&cleaned_inner);
                result.push_str(&source[close_brace..close_brace + 1]);
            } else {
                result.push_str(&source[cursor..selector_start]);
            }
            cursor = close_brace + 1;
            continue;
        }

        if should_remove_rule(selector, active_classes) {
            result.push_str(&source[cursor..selector_start]);
            cursor = close_brace + 1;
            continue;
        }

        result.push_str(&source[cursor..close_brace + 1]);
        cursor = close_brace + 1;
    }

    result.push_str(&source[cursor..]);
    result
}

fn normalize_cleaned_css(cleaned: &str, original: &str) -> String {
    let mut next = cleaned.replace("\r\n", "\n");
    while next.contains("\n\n\n\n") {
        next = next.replace("\n\n\n\n", "\n\n\n");
    }
    let trimmed = next.trim_end();
    if original.trim().is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

fn find_matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 1usize;
    for (relative_index, character) in source[open_brace + 1..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_brace + 1 + relative_index);
                }
            }
            _ => {}
        }
    }
    None
}

fn selector_start_before(source: &str, search_start: usize, open_brace: usize) -> usize {
    let mut start = open_brace;
    while start > search_start {
        let Some((previous_index, previous)) = source[..start].char_indices().next_back() else {
            break;
        };
        if previous == '}' || previous == ';' {
            break;
        }
        start = previous_index;
    }
    start
}

fn class_names_in_selector(selector: &str) -> Vec<String> {
    let mut classes = Vec::new();
    let mut cursor = 0;
    while let Some(relative_dot) = selector[cursor..].find('.') {
        let start = cursor + relative_dot + 1;
        let mut end = start;
        for (relative_index, character) in selector[start..].char_indices() {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                end = start + relative_index + character.len_utf8();
            } else {
                break;
            }
        }
        if end > start {
            classes.push(selector[start..end].to_string());
        }
        cursor = end.max(start);
    }
    classes
}

fn selector_part_targets_only_stale_generated_class(
    selector: &str,
    active_classes: &BTreeSet<String>,
) -> bool {
    let classes = class_names_in_selector(selector);
    let generated = classes
        .iter()
        .filter(|class_name| is_generated_pana_class(class_name))
        .collect::<Vec<_>>();
    if generated.is_empty() {
        return false;
    }
    let has_active_generated = generated
        .iter()
        .any(|class_name| active_classes.contains(class_name.as_str()));
    let has_stale_generated = generated
        .iter()
        .any(|class_name| !active_classes.contains(class_name.as_str()));
    has_stale_generated && !has_active_generated
}

fn should_remove_rule(selector: &str, active_classes: &BTreeSet<String>) -> bool {
    let parts = selector
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    !parts.is_empty()
        && parts
            .iter()
            .all(|part| selector_part_targets_only_stale_generated_class(part, active_classes))
}

fn has_effective_css_rules(source: &str) -> bool {
    let without_comments = remove_block_comments(source);
    for line in without_comments.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("@import")
            || trimmed.starts_with('@')
        {
            continue;
        }
        if trimmed.contains('{') {
            return true;
        }
    }
    false
}

fn remove_block_comments(source: &str) -> String {
    let mut result = String::new();
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find("/*") {
        let start = cursor + relative_start;
        result.push_str(&source[cursor..start]);
        let Some(relative_end) = source[start + 2..].find("*/") else {
            cursor = source.len();
            break;
        };
        cursor = start + 2 + relative_end + 2;
    }
    result.push_str(&source[cursor..]);
    result
}

fn normalize_page_js_config(config: PageJsConfig) -> PageJsConfig {
    let mut seen = HashSet::new();
    let components = config
        .components
        .into_iter()
        .filter_map(|component| {
            let id = component.id.trim().to_string();
            if id.is_empty() || !seen.insert(id.clone()) {
                None
            } else {
                Some(PanaComponent { id })
            }
        })
        .collect();

    PageJsConfig {
        version: Some(config.version.unwrap_or(1)),
        components,
        motion: Some(normalize_motion_config(config.motion)),
    }
}

fn normalize_motion_config(motion: Option<Value>) -> Value {
    let anime_version = motion
        .as_ref()
        .and_then(|value| value.get("animeVersion"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("4.4.1")
        .to_string();
    let active_item_id = motion
        .as_ref()
        .and_then(|value| value.get("activeItemId"))
        .and_then(Value::as_str)
        .map(|value| json!(value))
        .unwrap_or(Value::Null);
    let items = motion
        .as_ref()
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    json!({
        "schemaVersion": 1,
        "animeVersion": anime_version,
        "activeItemId": active_item_id,
        "items": items,
    })
}

fn reconcile_page_js_motion(
    config: &PageJsConfig,
    active_data_anim_ids: &BTreeSet<String>,
) -> PageJsConfig {
    let motion = normalize_motion_config(config.motion.clone());
    let anime_version = motion
        .get("animeVersion")
        .and_then(Value::as_str)
        .unwrap_or("4.4.1")
        .to_string();
    let active_item_id = motion.get("activeItemId").cloned().unwrap_or(Value::Null);
    let items = motion
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| motion_item_still_targets_active_data_anim(item, active_data_anim_ids))
        .collect::<Vec<_>>();

    PageJsConfig {
        version: Some(config.version.unwrap_or(1)),
        components: config.components.clone(),
        motion: Some(json!({
            "schemaVersion": 1,
            "animeVersion": anime_version,
            "activeItemId": active_item_id,
            "items": items,
        })),
    }
}

fn motion_item_still_targets_active_data_anim(
    item: &Value,
    active_data_anim_ids: &BTreeSet<String>,
) -> bool {
    let ids = data_anim_ids_referenced_by_motion_item(item);
    ids.is_empty() || ids.iter().all(|id| active_data_anim_ids.contains(id))
}

fn data_anim_ids_referenced_by_motion_item(item: &Value) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let target = item.get("target");
    if let Some(data_anim) = target
        .and_then(|target| target.get("dataAnim"))
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed)
    {
        ids.insert(data_anim);
    }
    if let Some(selector) = target
        .and_then(|target| target.get("selector"))
        .and_then(Value::as_str)
    {
        ids.extend(data_anim_ids_in_selector(selector));
    }
    if let Some(selector) = item.get("targetSelector").and_then(Value::as_str) {
        ids.extend(data_anim_ids_in_selector(selector));
    }
    ids
}

fn data_anim_ids_in_selector(selector: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let mut cursor = 0;
    while let Some(relative_attr) = selector[cursor..].find("[data-anim") {
        let attr_pos = cursor + relative_attr + 1;
        let mut after_name = attr_pos + "data-anim".len();
        after_name = skip_ascii_whitespace(selector, after_name);
        if selector[after_name..].chars().next() != Some('=') {
            cursor = after_name;
            continue;
        }
        after_name += 1;
        after_name = skip_ascii_whitespace(selector, after_name);
        let Some(quote) = selector[after_name..].chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' {
            cursor = after_name + quote.len_utf8();
            continue;
        }
        let value_start = after_name + quote.len_utf8();
        let Some(relative_end) = selector[value_start..].find(quote) else {
            break;
        };
        let value_end = value_start + relative_end;
        if let Some(id) = non_empty_trimmed(&selector[value_start..value_end]) {
            ids.insert(id);
        }
        cursor = value_end + quote.len_utf8();
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        template_source: &str,
        stylesheet_source: &str,
        page_js_config: PageJsConfig,
    ) -> PageAssetContractRequest {
        PageAssetContractRequest {
            template_path: "templates/index.html".to_string(),
            template_source: template_source.to_string(),
            stylesheet_source: Some(stylesheet_source.to_string()),
            stylesheet_known: Some(!stylesheet_source.trim().is_empty()),
            page_js_config: Some(page_js_config),
        }
    }

    #[test]
    fn removes_stale_generated_css_rules_and_keeps_active_or_manual_rules() {
        let plan = plan_page_asset_contract(request(
            r#"<main class="ps-hero-abcdef manual"></main>"#,
            ".ps-hero-abcdef { color: red; }\n.ps-old-123456 { color: blue; }\n.manual { color: green; }\n",
            PageJsConfig::default(),
        ));

        assert!(plan.stylesheet.changed);
        assert!(plan.stylesheet.contents.contains(".ps-hero-abcdef"));
        assert!(!plan.stylesheet.contents.contains(".ps-old-123456"));
        assert!(plan.stylesheet.contents.contains(".manual"));
        assert_eq!(plan.active_generated_classes, vec!["ps-hero-abcdef"]);
    }

    #[test]
    fn removes_empty_at_rules_after_generated_css_cleanup() {
        let active_classes = BTreeSet::new();
        let cleaned = remove_stale_generated_css_rules(
            "@media (max-width: 700px) {\n.ps-old-123456 { display: block; }\n}\n",
            &active_classes,
        );

        assert_eq!(cleaned, "\n");
        assert!(!has_effective_css_rules(&cleaned));
    }

    #[test]
    fn filters_motion_items_targeting_removed_data_anim_ids() {
        let page_js_config = PageJsConfig {
            version: Some(1),
            components: vec![PanaComponent {
                id: "accordion".to_string(),
            }],
            motion: Some(json!({
                "schemaVersion": 1,
                "animeVersion": "4.4.1",
                "activeItemId": "old",
                "items": [
                    {
                        "id": "active",
                        "type": "animation",
                        "target": { "dataAnim": "hero", "selector": "[data-anim=\"hero\"]" }
                    },
                    {
                        "id": "old",
                        "type": "animation",
                        "target": { "selector": "[data-anim='removed']" }
                    },
                    {
                        "id": "manual",
                        "type": "custom",
                        "target": { "selector": ".manual" }
                    }
                ]
            })),
        };

        let plan = plan_page_asset_contract(request(
            r#"<section data-anim="hero"></section>"#,
            "",
            page_js_config,
        ));

        let items = plan
            .page_js_config
            .motion
            .as_ref()
            .and_then(|motion| motion.get("items"))
            .and_then(Value::as_array)
            .unwrap();
        let ids = items
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["active", "manual"]);
        assert!(plan.page_js_changed);
        assert_eq!(plan.page_js_config.components[0].id, "accordion");
    }

    #[test]
    fn removes_page_css_link_when_no_effective_rules_remain() {
        let plan = plan_page_asset_contract(request(
            r#"{% extends "base.html" %}
{% block css_pagina %}<link rel="stylesheet" href="/pagini/index.css">{% endblock %}
{% block content %}<p>Text</p>{% endblock %}
"#,
            ".ps-old-123456 { color: blue; }\n",
            PageJsConfig::default(),
        ));

        assert!(plan.template.changed);
        assert!(!plan.template.contents.contains("css_pagina"));
        assert!(!plan.template.contents.contains("/pagini/index.css"));
        assert_eq!(plan.stylesheet.contents, "\n");
    }
}
