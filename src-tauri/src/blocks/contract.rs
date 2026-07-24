use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    css::page::{
        page_css_href, page_scss_relative_path, plan_page_stylesheet_link_source,
        remove_page_stylesheet_link,
    },
    js::{NativeBlockRuntimeEntry, PageJsConfig},
};

use super::native::{
    known_native_block_ids, native_block_by_id, native_block_instance_id, native_block_preview_css,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockContractRequest {
    pub template_path: String,
    pub template_source: String,
    pub stylesheet_source: Option<String>,
    pub page_js_config: Option<PageJsConfig>,
    pub ensure_block_id: Option<String>,
    pub cachebust_assets: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockContractTextPlan {
    pub changed: bool,
    pub contents: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockContractPlan {
    pub template_path: String,
    pub stylesheet_path: String,
    pub stylesheet_href: String,
    pub active_block_ids: Vec<String>,
    pub template: NativeBlockContractTextPlan,
    pub stylesheet: NativeBlockContractTextPlan,
    pub page_js_config: PageJsConfig,
    pub page_js_changed: bool,
    pub preview_css: String,
    pub diagnostics: Vec<String>,
}

pub fn plan_native_block_contract(request: NativeBlockContractRequest) -> NativeBlockContractPlan {
    let template_path = normalize_template_path(&request.template_path);
    let stylesheet_rel = page_scss_relative_path(&template_path);
    let stylesheet_path = to_project_relative_path(&stylesheet_rel);
    let stylesheet_href = page_css_href(&template_path);
    let mut diagnostics = Vec::new();

    let active_set = block_ids_in_template_source(&request.template_source, &mut diagnostics);
    let active_set = ensure_requested_block(
        active_set,
        request.ensure_block_id.as_deref(),
        &mut diagnostics,
    );
    let active_block_ids = active_ids_in_registry_order(&active_set);

    let stylesheet_source = request.stylesheet_source.unwrap_or_default();
    let had_managed_block_styles = has_any_block_style_contract(&stylesheet_source);
    let next_stylesheet = reconcile_block_style_source(&stylesheet_source, &active_set);
    let next_template_source =
        reconcile_block_instance_source(&request.template_source, &mut diagnostics);
    let next_template = if active_set.is_empty() {
        if had_managed_block_styles && next_stylesheet.trim().is_empty() {
            remove_page_stylesheet_link(&next_template_source, &stylesheet_href)
        } else {
            next_template_source
        }
    } else {
        plan_page_stylesheet_link_source(
            &next_template_source,
            &stylesheet_href,
            request.cachebust_assets.unwrap_or(false),
        )
    };

    let current_page_js = normalize_page_js_config(request.page_js_config.unwrap_or_default());
    let next_page_js = reconcile_page_js_blocks(&current_page_js, &active_set);
    let preview_css = native_block_preview_css(active_block_ids.iter().map(String::as_str));

    NativeBlockContractPlan {
        template_path,
        stylesheet_path,
        stylesheet_href,
        active_block_ids,
        template: NativeBlockContractTextPlan {
            changed: next_template != request.template_source,
            contents: next_template,
        },
        stylesheet: NativeBlockContractTextPlan {
            changed: next_stylesheet != stylesheet_source,
            contents: next_stylesheet,
        },
        page_js_changed: next_page_js != current_page_js,
        page_js_config: next_page_js,
        preview_css,
        diagnostics,
    }
}

fn normalize_template_path(path: &str) -> String {
    path.trim().trim_start_matches('/').to_string()
}

fn to_project_relative_path(path: &str) -> String {
    path.to_string()
}

fn known_block_id_set() -> HashSet<&'static str> {
    known_native_block_ids().collect()
}

fn active_ids_in_registry_order(active: &BTreeSet<String>) -> Vec<String> {
    known_native_block_ids()
        .filter(|id| active.contains(*id))
        .map(str::to_string)
        .collect()
}

fn block_ids_in_template_source(source: &str, diagnostics: &mut Vec<String>) -> BTreeSet<String> {
    let mut active = BTreeSet::new();
    let known = known_block_id_set();
    for attribute_name in ["data-pana-block", "data-pana-component"] {
        let mut cursor = 0;
        while let Some(relative_attr) = source[cursor..].find(attribute_name) {
            let attr_pos = cursor + relative_attr;
            cursor = attr_pos + attribute_name.len();
            let Some((value, end)) = read_quoted_attribute_value(source, cursor) else {
                diagnostics.push(format!("Atribut {attribute_name} fără valoare citibilă."));
                continue;
            };
            cursor = end;
            let id = value.trim();
            if id.is_empty() {
                diagnostics.push(format!("Atribut {attribute_name} gol ignorat."));
                continue;
            }
            if known.contains(id) {
                active.insert(id.to_string());
            } else {
                diagnostics.push(format!(
                    "Blocul {id} există în template, dar nu este cunoscut de NativeBlockRegistry Rust."
                ));
            }
        }
    }

    active
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

fn reconcile_block_instance_source(source: &str, diagnostics: &mut Vec<String>) -> String {
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find('<') {
        let tag_start = cursor + relative_start;
        let Some(relative_end) = source[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_end + 1;
        output.push_str(&source[cursor..tag_start]);
        let tag = &source[tag_start..tag_end];
        output.push_str(&reconcile_block_instance_tag(tag, diagnostics));
        cursor = tag_end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn reconcile_block_instance_tag(tag: &str, diagnostics: &mut Vec<String>) -> String {
    if !tag.starts_with('<')
        || tag.starts_with("</")
        || tag.starts_with("<!")
        || tag.starts_with("<?")
        || (!tag.contains("data-pana-block") && !tag.contains("data-pana-component"))
    {
        return tag.to_string();
    }

    let block_marker = find_tag_attribute_value(tag, "data-pana-block")
        .or_else(|| find_tag_attribute_value(tag, "data-pana-component"));
    let Some((block_id, _, _)) = block_marker else {
        return tag.to_string();
    };
    let block_id = block_id.trim();
    if block_id.is_empty() || native_block_by_id(block_id).is_none() {
        return tag.to_string();
    }

    let Some((data_anim, _, _)) = find_tag_attribute_value(tag, "data-anim") else {
        diagnostics.push(format!(
            "Blocul {block_id} nu are data-anim; data-pana-instance nu poate fi normalizat."
        ));
        return tag.to_string();
    };
    let data_anim = data_anim.trim();
    if data_anim.is_empty() {
        diagnostics.push(format!(
            "Blocul {block_id} are data-anim gol; data-pana-instance nu poate fi normalizat."
        ));
        return tag.to_string();
    }

    let expected = native_block_instance_id(block_id, data_anim);
    let Some((current, value_start, value_end)) =
        find_tag_attribute_value(tag, "data-pana-instance")
    else {
        return insert_tag_attribute(tag, "data-pana-instance", &expected);
    };
    if current == expected {
        return tag.to_string();
    }
    replace_range(tag, value_start, value_end, &escape_attr_value(&expected))
}

fn find_tag_attribute_value(tag: &str, attr: &str) -> Option<(String, usize, usize)> {
    let mut cursor = 0;
    while let Some(relative_attr) = tag[cursor..].find(attr) {
        let attr_start = cursor + relative_attr;
        let attr_end = attr_start + attr.len();
        if !is_attr_boundary_before(tag, attr_start) || !is_attr_boundary_after(tag, attr_end) {
            cursor = attr_end;
            continue;
        }
        let mut value_cursor = skip_ascii_whitespace(tag, attr_end);
        if tag[value_cursor..].chars().next()? != '=' {
            cursor = attr_end;
            continue;
        }
        value_cursor += 1;
        value_cursor = skip_ascii_whitespace(tag, value_cursor);
        let quote = tag[value_cursor..].chars().next()?;
        if quote != '"' && quote != '\'' {
            cursor = attr_end;
            continue;
        }
        let value_start = value_cursor + quote.len_utf8();
        let value_end = tag[value_start..].find(quote)? + value_start;
        return Some((
            tag[value_start..value_end].to_string(),
            value_start,
            value_end,
        ));
    }
    None
}

fn is_attr_boundary_before(source: &str, index: usize) -> bool {
    source[..index]
        .chars()
        .next_back()
        .map(|character| {
            character.is_ascii_whitespace()
                || character == '<'
                || character == '/'
                || character == '%'
        })
        .unwrap_or(true)
}

fn is_attr_boundary_after(source: &str, index: usize) -> bool {
    source[index..]
        .chars()
        .next()
        .map(|character| {
            character.is_ascii_whitespace()
                || character == '='
                || character == '/'
                || character == '>'
        })
        .unwrap_or(true)
}

fn insert_tag_attribute(tag: &str, attr: &str, value: &str) -> String {
    let insert_at = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    format!(
        "{} {}=\"{}\"{}",
        &tag[..insert_at],
        attr,
        escape_attr_value(value),
        &tag[insert_at..]
    )
}

fn replace_range(source: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut next = String::with_capacity(source.len() - (end - start) + replacement.len());
    next.push_str(&source[..start]);
    next.push_str(replacement);
    next.push_str(&source[end..]);
    next
}

fn escape_attr_value(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
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

fn ensure_requested_block(
    mut active: BTreeSet<String>,
    ensure_block_id: Option<&str>,
    diagnostics: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(id) = ensure_block_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return active;
    };
    if native_block_by_id(id).is_some() {
        active.insert(id.to_string());
    } else {
        diagnostics.push(format!(
            "Blocul cerut pentru contract ({id}) nu există în NativeBlockRegistry Rust."
        ));
    }
    active
}

fn block_style_marker(id: &str, edge: &str) -> String {
    format!("/* pana:block {id}:{edge} */")
}

fn legacy_block_style_marker(id: &str, edge: &str) -> String {
    format!("/* pana:component {id}:{edge} */")
}

fn block_style_block(id: &str) -> Option<String> {
    let block = native_block_by_id(id)?;
    Some(format!(
        "{}\n{}\n{}",
        block_style_marker(id, "start"),
        block.scss.trim(),
        block_style_marker(id, "end")
    ))
}

fn has_any_block_style_contract(source: &str) -> bool {
    known_native_block_ids().any(|id| {
        source.contains(&block_style_marker(id, "start"))
            || source.contains(&legacy_block_style_marker(id, "start"))
    })
}

fn reconcile_block_style_source(source: &str, active: &BTreeSet<String>) -> String {
    if active.is_empty() && !has_any_block_style_contract(source) {
        return source.to_string();
    }

    let mut next = source.to_string();
    for id in known_native_block_ids() {
        if active.contains(id) {
            if let Some(block) = block_style_block(id) {
                next = upsert_block_style_block(&next, id, &block);
            }
        } else {
            next = remove_block_style_block(&next, id);
        }
    }

    normalize_block_stylesheet(&next)
}

fn locate_block_style_block(source: &str, id: &str) -> Option<(usize, usize)> {
    for (start_marker, end_marker) in [
        (
            block_style_marker(id, "start"),
            block_style_marker(id, "end"),
        ),
        (
            legacy_block_style_marker(id, "start"),
            legacy_block_style_marker(id, "end"),
        ),
    ] {
        let Some(start) = source.find(&start_marker) else {
            continue;
        };
        let Some(relative_end) = source[start..].find(&end_marker) else {
            continue;
        };
        let end_marker_start = relative_end + start;
        return Some((start, end_marker_start + end_marker.len()));
    }
    None
}

fn locate_block_style_block_with_padding(source: &str, id: &str) -> Option<(usize, usize)> {
    let (mut start, mut end) = locate_block_style_block(source, id)?;
    while start > 0 && source.get(start - 1..start) == Some("\n") {
        start -= 1;
        if start == 0 || source.get(start - 1..start) == Some("\n") {
            break;
        }
    }
    while source.get(end..end + 1) == Some("\n") {
        end += 1;
        if source.get(end..end + 1) == Some("\n") {
            break;
        }
    }
    Some((start, end))
}

fn upsert_block_style_block(source: &str, id: &str, block: &str) -> String {
    if let Some((start, end)) = locate_block_style_block(source, id) {
        return format!("{}{}\n{}", &source[..start], block, &source[end..]);
    }
    if source.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{}\n\n{block}\n", source.trim_end())
    }
}

fn remove_block_style_block(source: &str, id: &str) -> String {
    let Some((start, end)) = locate_block_style_block_with_padding(source, id) else {
        return source.to_string();
    };
    format!("{}{}", &source[..start], &source[end..])
}

fn normalize_block_stylesheet(source: &str) -> String {
    let mut next = source.replace("\r\n", "\n");
    while next.contains("\n\n\n\n") {
        next = next.replace("\n\n\n\n", "\n\n\n");
    }
    let trimmed = next.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

fn normalize_page_js_config(config: PageJsConfig) -> PageJsConfig {
    let mut seen = HashSet::new();
    let blocks: Vec<NativeBlockRuntimeEntry> = config
        .blocks
        .into_iter()
        .filter_map(|block| {
            let id = block.id.trim().to_string();
            if id.is_empty() || !seen.insert(id.clone()) {
                None
            } else {
                Some(NativeBlockRuntimeEntry { id })
            }
        })
        .collect();

    if blocks.is_empty() && config.motion.is_none() {
        return PageJsConfig::default();
    }

    PageJsConfig {
        version: Some(config.version.unwrap_or(1)),
        blocks,
        motion: config.motion,
    }
}

fn reconcile_page_js_blocks(config: &PageJsConfig, active: &BTreeSet<String>) -> PageJsConfig {
    let known = known_block_id_set();
    let mut blocks: Vec<NativeBlockRuntimeEntry> = config
        .blocks
        .iter()
        .filter(|block| !known.contains(block.id.as_str()))
        .cloned()
        .collect();
    blocks.extend(
        active
            .iter()
            .map(|id| NativeBlockRuntimeEntry { id: id.clone() }),
    );

    if blocks.is_empty() && config.motion.is_none() {
        return PageJsConfig::default();
    }

    PageJsConfig {
        version: Some(config.version.unwrap_or(1)),
        blocks,
        motion: config.motion.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        template_source: &str,
        stylesheet_source: &str,
        page_js_config: PageJsConfig,
    ) -> NativeBlockContractRequest {
        NativeBlockContractRequest {
            template_path: "templates/index.html".to_string(),
            template_source: template_source.to_string(),
            stylesheet_source: Some(stylesheet_source.to_string()),
            page_js_config: Some(page_js_config),
            ensure_block_id: None,
            cachebust_assets: Some(false),
        }
    }

    #[test]
    fn plans_block_styles_js_and_template_link_from_template_source() {
        let plan = plan_native_block_contract(request(
            r#"{% extends "base.html" %}
{% block content %}
<span data-pana-component="counter">0</span>
{% endblock content %}
"#,
            "",
            PageJsConfig::default(),
        ));

        assert_eq!(plan.stylesheet_path, "sass/pagini/index.scss");
        assert_eq!(plan.stylesheet_href, "/pagini/index.css");
        assert_eq!(plan.active_block_ids, vec!["counter".to_string()]);
        assert!(plan.template.changed);
        assert!(plan.template.contents.contains("{% block css_pagina %}"));
        assert!(plan.stylesheet.changed);
        assert!(plan
            .stylesheet
            .contents
            .contains("pana:block counter:start"));
        assert_eq!(plan.page_js_config.blocks[0].id, "counter");
        assert!(plan.page_js_changed);
        assert!(plan.preview_css.contains(".counter"));
    }

    #[test]
    fn removes_stale_block_styles_and_preserves_unknown_js_entries() {
        let stale_counter = block_style_block("counter").unwrap();
        let page_js_config = PageJsConfig {
            version: Some(1),
            blocks: vec![
                NativeBlockRuntimeEntry {
                    id: "counter".to_string(),
                },
                NativeBlockRuntimeEntry {
                    id: "custom-widget".to_string(),
                },
            ],
            motion: None,
        };
        let plan = plan_native_block_contract(request(
            r#"{% block content %}<p>Fără bloc.</p>{% endblock content %}"#,
            &format!("{stale_counter}\n\n.manual {{ color: red; }}\n"),
            page_js_config,
        ));

        assert!(plan.stylesheet.changed);
        assert!(!plan
            .stylesheet
            .contents
            .contains("pana:component counter:start"));
        assert!(plan.stylesheet.contents.contains(".manual"));
        assert_eq!(plan.page_js_config.blocks.len(), 1);
        assert_eq!(plan.page_js_config.blocks[0].id, "custom-widget");
        assert!(plan.page_js_changed);
        assert!(!plan.template.changed);
    }

    #[test]
    fn ensure_block_id_adds_registry_block_even_before_rescan() {
        let mut req = request(
            r#"{% block content %}<main></main>{% endblock content %}"#,
            "",
            PageJsConfig::default(),
        );
        req.ensure_block_id = Some("accordion".to_string());

        let plan = plan_native_block_contract(req);

        assert_eq!(plan.active_block_ids, vec!["accordion".to_string()]);
        assert!(plan
            .stylesheet
            .contents
            .contains("pana:block accordion:start"));
        assert_eq!(plan.page_js_config.blocks[0].id, "accordion");
    }

    #[test]
    fn normalizes_legacy_block_instance_ids_from_rust_contract() {
        let plan = plan_native_block_contract(request(
            r#"{% block content %}
<div data-pana-component="tabs" data-anim="ps-tabs-fresh" data-pana-instance="tabs-old">
  <button data-pana-tabs-tab>Tab</button>
</div>
<span data-pana-component="counter" data-anim="ps-counter-missing">0</span>
{% endblock content %}"#,
            "",
            PageJsConfig::default(),
        ));

        assert!(plan.template.changed);
        assert!(plan
            .template
            .contents
            .contains(r#"data-pana-instance="tabs-tabs-fresh""#));
        assert!(plan
            .template
            .contents
            .contains(r#"data-pana-instance="counter-counter-missing""#));
        assert!(!plan.template.contents.contains("tabs-old"));
        assert_eq!(
            plan.active_block_ids,
            vec!["counter".to_string(), "tabs".to_string()]
        );
    }
}
