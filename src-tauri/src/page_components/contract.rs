use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    css::page::{page_css_href, page_scss_relative_path, plan_page_stylesheet_link_source},
    js::{PageJsConfig, PanaComponent},
};

use super::{
    known_page_component_ids, page_component_by_id, page_component_instance_id,
    page_component_preview_css,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentContractRequest {
    pub template_path: String,
    pub template_source: String,
    pub stylesheet_source: Option<String>,
    pub page_js_config: Option<PageJsConfig>,
    pub ensure_component_id: Option<String>,
    pub cachebust_assets: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentContractTextPlan {
    pub changed: bool,
    pub contents: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageComponentContractPlan {
    pub template_path: String,
    pub stylesheet_path: String,
    pub stylesheet_href: String,
    pub active_component_ids: Vec<String>,
    pub template: PageComponentContractTextPlan,
    pub stylesheet: PageComponentContractTextPlan,
    pub page_js_config: PageJsConfig,
    pub page_js_changed: bool,
    pub preview_css: String,
    pub diagnostics: Vec<String>,
}

pub fn plan_page_component_contract(
    request: PageComponentContractRequest,
) -> PageComponentContractPlan {
    let template_path = normalize_template_path(&request.template_path);
    let stylesheet_rel = page_scss_relative_path(&template_path);
    let stylesheet_path = to_project_relative_path(&stylesheet_rel);
    let stylesheet_href = page_css_href(&template_path);
    let mut diagnostics = Vec::new();

    let active_set = component_ids_in_template_source(&request.template_source, &mut diagnostics);
    let active_set = ensure_requested_component(
        active_set,
        request.ensure_component_id.as_deref(),
        &mut diagnostics,
    );
    let active_component_ids = active_ids_in_registry_order(&active_set);

    let stylesheet_source = request.stylesheet_source.unwrap_or_default();
    let next_stylesheet = reconcile_component_style_source(&stylesheet_source, &active_set);
    let next_template_source =
        reconcile_component_instance_source(&request.template_source, &mut diagnostics);
    let next_template = if active_set.is_empty() {
        next_template_source
    } else {
        plan_page_stylesheet_link_source(
            &next_template_source,
            &stylesheet_href,
            request.cachebust_assets.unwrap_or(false),
        )
    };

    let current_page_js = normalize_page_js_config(request.page_js_config.unwrap_or_default());
    let next_page_js = reconcile_page_js_components(&current_page_js, &active_set);
    let preview_css = page_component_preview_css(active_component_ids.iter().map(String::as_str));

    PageComponentContractPlan {
        template_path,
        stylesheet_path,
        stylesheet_href,
        active_component_ids,
        template: PageComponentContractTextPlan {
            changed: next_template != request.template_source,
            contents: next_template,
        },
        stylesheet: PageComponentContractTextPlan {
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
    path.trim()
        .trim_start_matches('/')
        .strip_prefix("sursa/")
        .unwrap_or_else(|| path.trim().trim_start_matches('/'))
        .to_string()
}

fn to_project_relative_path(path: &str) -> String {
    if path.starts_with("sursa/") {
        path.to_string()
    } else {
        format!("sursa/{path}")
    }
}

fn known_component_id_set() -> HashSet<&'static str> {
    known_page_component_ids().collect()
}

fn active_ids_in_registry_order(active: &BTreeSet<String>) -> Vec<String> {
    known_page_component_ids()
        .filter(|id| active.contains(*id))
        .map(str::to_string)
        .collect()
}

fn component_ids_in_template_source(
    source: &str,
    diagnostics: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut active = BTreeSet::new();
    let known = known_component_id_set();
    let mut cursor = 0;

    while let Some(relative_attr) = source[cursor..].find("data-pana-component") {
        let attr_pos = cursor + relative_attr;
        cursor = attr_pos + "data-pana-component".len();
        let Some((value, end)) = read_quoted_attribute_value(source, cursor) else {
            diagnostics.push("Atribut data-pana-component fără valoare citibilă.".to_string());
            continue;
        };
        cursor = end;
        let id = value.trim();
        if id.is_empty() {
            diagnostics.push("Atribut data-pana-component gol ignorat.".to_string());
            continue;
        }
        if known.contains(id) {
            active.insert(id.to_string());
        } else {
            diagnostics.push(format!(
                "Componenta {id} există în template, dar nu este cunoscută de Page Component Registry Rust."
            ));
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

fn reconcile_component_instance_source(source: &str, diagnostics: &mut Vec<String>) -> String {
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
        output.push_str(&reconcile_component_instance_tag(tag, diagnostics));
        cursor = tag_end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn reconcile_component_instance_tag(tag: &str, diagnostics: &mut Vec<String>) -> String {
    if !tag.starts_with('<')
        || tag.starts_with("</")
        || tag.starts_with("<!")
        || tag.starts_with("<?")
        || !tag.contains("data-pana-component")
    {
        return tag.to_string();
    }

    let Some((component_id, _, _)) = find_tag_attribute_value(tag, "data-pana-component") else {
        return tag.to_string();
    };
    let component_id = component_id.trim();
    if component_id.is_empty() || page_component_by_id(component_id).is_none() {
        return tag.to_string();
    }

    let Some((data_anim, _, _)) = find_tag_attribute_value(tag, "data-anim") else {
        diagnostics.push(format!(
            "Componenta {component_id} nu are data-anim; data-pana-instance nu poate fi normalizat."
        ));
        return tag.to_string();
    };
    let data_anim = data_anim.trim();
    if data_anim.is_empty() {
        diagnostics.push(format!(
            "Componenta {component_id} are data-anim gol; data-pana-instance nu poate fi normalizat."
        ));
        return tag.to_string();
    }

    let expected = page_component_instance_id(component_id, data_anim);
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

fn ensure_requested_component(
    mut active: BTreeSet<String>,
    ensure_component_id: Option<&str>,
    diagnostics: &mut Vec<String>,
) -> BTreeSet<String> {
    let Some(id) = ensure_component_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return active;
    };
    if page_component_by_id(id).is_some() {
        active.insert(id.to_string());
    } else {
        diagnostics.push(format!(
            "Componenta cerută pentru contract ({id}) nu există în Page Component Registry Rust."
        ));
    }
    active
}

fn component_style_marker(id: &str, edge: &str) -> String {
    format!("/* pana:component {id}:{edge} */")
}

fn component_style_block(id: &str) -> Option<String> {
    let component = page_component_by_id(id)?;
    Some(format!(
        "{}\n{}\n{}",
        component_style_marker(id, "start"),
        component.scss.trim(),
        component_style_marker(id, "end")
    ))
}

fn has_any_component_style_contract(source: &str) -> bool {
    known_page_component_ids().any(|id| source.contains(&component_style_marker(id, "start")))
}

fn reconcile_component_style_source(source: &str, active: &BTreeSet<String>) -> String {
    if active.is_empty() && !has_any_component_style_contract(source) {
        return source.to_string();
    }

    let mut next = source.to_string();
    for id in known_page_component_ids() {
        if active.contains(id) {
            if let Some(block) = component_style_block(id) {
                next = upsert_component_style_block(&next, id, &block);
            }
        } else {
            next = remove_component_style_block(&next, id);
        }
    }

    normalize_component_stylesheet(&next)
}

fn locate_component_style_block(source: &str, id: &str) -> Option<(usize, usize)> {
    let start_marker = component_style_marker(id, "start");
    let end_marker = component_style_marker(id, "end");
    let start = source.find(&start_marker)?;
    let end_marker_start = source[start..].find(&end_marker)? + start;
    let end = end_marker_start + end_marker.len();
    Some((start, end))
}

fn locate_component_style_block_with_padding(source: &str, id: &str) -> Option<(usize, usize)> {
    let (mut start, mut end) = locate_component_style_block(source, id)?;
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

fn upsert_component_style_block(source: &str, id: &str, block: &str) -> String {
    if let Some((start, end)) = locate_component_style_block(source, id) {
        return format!("{}{}\n{}", &source[..start], block, &source[end..]);
    }
    if source.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{}\n\n{block}\n", source.trim_end())
    }
}

fn remove_component_style_block(source: &str, id: &str) -> String {
    let Some((start, end)) = locate_component_style_block_with_padding(source, id) else {
        return source.to_string();
    };
    format!("{}{}", &source[..start], &source[end..])
}

fn normalize_component_stylesheet(source: &str) -> String {
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
        motion: config.motion,
    }
}

fn reconcile_page_js_components(config: &PageJsConfig, active: &BTreeSet<String>) -> PageJsConfig {
    let known = known_component_id_set();
    let mut components: Vec<PanaComponent> = config
        .components
        .iter()
        .filter(|component| !known.contains(component.id.as_str()))
        .cloned()
        .collect();
    components.extend(active.iter().map(|id| PanaComponent { id: id.clone() }));

    PageJsConfig {
        version: Some(config.version.unwrap_or(1)),
        components,
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
    ) -> PageComponentContractRequest {
        PageComponentContractRequest {
            template_path: "templates/index.html".to_string(),
            template_source: template_source.to_string(),
            stylesheet_source: Some(stylesheet_source.to_string()),
            page_js_config: Some(page_js_config),
            ensure_component_id: None,
            cachebust_assets: Some(false),
        }
    }

    #[test]
    fn plans_component_styles_js_and_template_link_from_template_source() {
        let plan = plan_page_component_contract(request(
            r#"{% extends "base.html" %}
{% block content %}
<span data-pana-component="counter">0</span>
{% endblock content %}
"#,
            "",
            PageJsConfig::default(),
        ));

        assert_eq!(plan.stylesheet_path, "sursa/sass/pagini/index.scss");
        assert_eq!(plan.stylesheet_href, "/pagini/index.css");
        assert_eq!(plan.active_component_ids, vec!["counter".to_string()]);
        assert!(plan.template.changed);
        assert!(plan.template.contents.contains("{% block css_pagina %}"));
        assert!(plan.stylesheet.changed);
        assert!(plan
            .stylesheet
            .contents
            .contains("pana:component counter:start"));
        assert_eq!(plan.page_js_config.components[0].id, "counter");
        assert!(plan.page_js_changed);
        assert!(plan.preview_css.contains(".counter"));
    }

    #[test]
    fn removes_stale_component_styles_and_preserves_unknown_js_components() {
        let stale_counter = component_style_block("counter").unwrap();
        let page_js_config = PageJsConfig {
            version: Some(1),
            components: vec![
                PanaComponent {
                    id: "counter".to_string(),
                },
                PanaComponent {
                    id: "custom-widget".to_string(),
                },
            ],
            motion: None,
        };
        let plan = plan_page_component_contract(request(
            r#"{% block content %}<p>Fara componenta.</p>{% endblock content %}"#,
            &format!("{stale_counter}\n\n.manual {{ color: red; }}\n"),
            page_js_config,
        ));

        assert!(plan.stylesheet.changed);
        assert!(!plan
            .stylesheet
            .contents
            .contains("pana:component counter:start"));
        assert!(plan.stylesheet.contents.contains(".manual"));
        assert_eq!(plan.page_js_config.components.len(), 1);
        assert_eq!(plan.page_js_config.components[0].id, "custom-widget");
        assert!(plan.page_js_changed);
        assert!(!plan.template.changed);
    }

    #[test]
    fn ensure_component_id_adds_registry_component_even_before_rescan() {
        let mut req = request(
            r#"{% block content %}<main></main>{% endblock content %}"#,
            "",
            PageJsConfig::default(),
        );
        req.ensure_component_id = Some("accordion".to_string());

        let plan = plan_page_component_contract(req);

        assert_eq!(plan.active_component_ids, vec!["accordion".to_string()]);
        assert!(plan
            .stylesheet
            .contents
            .contains("pana:component accordion:start"));
        assert_eq!(plan.page_js_config.components[0].id, "accordion");
    }

    #[test]
    fn normalizes_component_instance_ids_from_rust_contract() {
        let plan = plan_page_component_contract(request(
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
            plan.active_component_ids,
            vec!["counter".to_string(), "tabs".to_string()]
        );
    }
}
