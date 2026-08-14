use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use tauri_utils::html::{parse, NodeRef};

use crate::css::page::{
    page_css_href, page_scss_relative_path, plan_page_stylesheet_link_source,
    remove_page_stylesheet_link,
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
    pub ensure_block_id: Option<String>,
    #[serde(skip)]
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
    let styled_set = active_set
        .iter()
        .filter(|id| {
            native_block_by_id(id).is_some_and(|block| !block.functional_scss.trim().is_empty())
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let stylesheet_source = request.stylesheet_source.unwrap_or_default();
    let had_managed_block_styles = has_any_block_style_contract(&stylesheet_source);
    let next_stylesheet = reconcile_block_style_source(&stylesheet_source, &styled_set);
    let next_template_source =
        reconcile_block_instance_source(&request.template_source, &mut diagnostics);
    let next_template = if styled_set.is_empty() {
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

pub(crate) fn block_ids_in_template_source(
    source: &str,
    diagnostics: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut active = BTreeSet::new();
    let known = known_block_id_set();
    collect_block_ids(&parse(source.to_string()), &known, &mut active, diagnostics);
    active
}

fn collect_block_ids(
    node: &NodeRef,
    known: &HashSet<&'static str>,
    active: &mut BTreeSet<String>,
    diagnostics: &mut Vec<String>,
) {
    if let Some(element) = node.as_element() {
        if let Some(value) = element.attributes.borrow().get("data-pana-block") {
            let id = value.trim();
            if id.is_empty() {
                diagnostics.push("Atribut data-pana-block gol ignorat.".to_string());
            } else if known.contains(id) {
                active.insert(id.to_string());
            } else {
                diagnostics.push(format!(
                    "Blocul {id} există în template, dar nu este cunoscut de NativeBlockRegistry Rust."
                ));
            }
        }
    }
    for child in node.children() {
        collect_block_ids(&child, known, active, diagnostics);
    }
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
        || !tag.contains("data-pana-block")
    {
        return tag.to_string();
    }

    let block_marker = find_tag_attribute_value(tag, "data-pana-block");
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

fn block_style_block(id: &str) -> Option<String> {
    let block = native_block_by_id(id)?;
    let functional_scss = block.functional_scss.trim();
    if functional_scss.is_empty() {
        return None;
    }
    Some(format!(
        "{}\n{}\n{}",
        block_style_marker(id, "start"),
        functional_scss,
        block_style_marker(id, "end")
    ))
}

fn has_any_block_style_contract(source: &str) -> bool {
    known_native_block_ids().any(|id| source.contains(&block_style_marker(id, "start")))
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
            } else {
                next = remove_block_style_block(&next, id);
            }
        } else {
            next = remove_block_style_block(&next, id);
        }
    }

    normalize_block_stylesheet(&next)
}

fn locate_block_style_block(source: &str, id: &str) -> Option<(usize, usize)> {
    let start_marker = block_style_marker(id, "start");
    let end_marker = block_style_marker(id, "end");
    let start = source.find(&start_marker)?;
    let relative_end = source[start..].find(&end_marker)?;
    let end_marker_start = relative_end + start;
    Some((start, end_marker_start + end_marker.len()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::NativeBlockRuntimePlan;

    fn request(template_source: &str, stylesheet_source: &str) -> NativeBlockContractRequest {
        NativeBlockContractRequest {
            template_path: "templates/index.html".to_string(),
            template_source: template_source.to_string(),
            stylesheet_source: Some(stylesheet_source.to_string()),
            ensure_block_id: None,
            cachebust_assets: Some(false),
        }
    }

    #[test]
    fn plans_only_functional_block_styles_js_and_template_link() {
        let plan = plan_native_block_contract(request(
            r#"{% extends "base.html" %}
{% block content %}
<div data-pana-block="dialog" data-anim="ps-dialog-test"></div>
{% endblock content %}
"#,
            "",
        ));

        assert_eq!(plan.stylesheet_path, "sass/pagini/index.scss");
        assert_eq!(plan.stylesheet_href, "/pagini/index.css");
        assert_eq!(plan.active_block_ids, vec!["dialog".to_string()]);
        assert!(plan.template.changed);
        assert!(plan.template.contents.contains("{% block css_pagina %}"));
        assert!(plan.stylesheet.changed);
        assert!(plan.stylesheet.contents.contains("pana:block dialog:start"));
        let runtime = NativeBlockRuntimePlan::from_template_source(&plan.template.contents);
        assert_eq!(runtime.provider_ids(), &["dialog"]);
        assert!(plan.preview_css.contains(".dialog__overlay"));
        assert!(!plan.preview_css.contains("--pana-block-"));
    }

    #[test]
    fn removes_stale_block_styles_without_persisting_runtime_entries() {
        let stale_dialog = block_style_block("dialog").unwrap();
        let plan = plan_native_block_contract(request(
            r#"{% block content %}<p>Fără bloc.</p>{% endblock content %}"#,
            &format!("{stale_dialog}\n\n.manual {{ color: red; }}\n"),
        ));

        assert!(plan.stylesheet.changed);
        assert!(!plan.stylesheet.contents.contains("pana:block dialog:start"));
        assert!(plan.stylesheet.contents.contains(".manual"));
        assert!(NativeBlockRuntimePlan::from_template_source(&plan.template.contents).is_empty());
        assert!(!plan.template.changed);
    }

    #[test]
    fn ensure_block_id_adds_registry_block_even_before_rescan() {
        let mut req = request(
            r#"{% block content %}<main></main>{% endblock content %}"#,
            "",
        );
        req.ensure_block_id = Some("accordion".to_string());

        let plan = plan_native_block_contract(req);

        assert_eq!(plan.active_block_ids, vec!["accordion".to_string()]);
        assert!(!plan.stylesheet.changed);
        assert!(plan.stylesheet.contents.is_empty());
        assert!(!plan.template.changed);
    }

    #[test]
    fn static_icon_never_generates_page_css_or_javascript_runtime() {
        let plan = plan_native_block_contract(request(
            concat!(
                "{% block content %}",
                "<svg data-pana-block=\"icon\" data-pana-icon=\"tabler-outline:home\" ",
                "data-pana-instance=\"icon-test\"><path d=\"M 1 1\"></path></svg>",
                "{% endblock content %}",
            ),
            "",
        ));

        assert_eq!(plan.active_block_ids, vec!["icon".to_string()]);
        assert!(!plan.template.changed);
        assert!(!plan.stylesheet.changed);
        assert!(plan.stylesheet.contents.is_empty());
        assert!(NativeBlockRuntimePlan::from_template_source(&plan.template.contents).is_empty());
        assert!(plan.preview_css.is_empty());
        assert!(!plan.template.contents.contains("/pagini/index.css"));
    }

    #[test]
    fn runtime_reconciliation_deduplicates_instances_and_removes_the_last_provider() {
        let inserted = plan_native_block_contract(request(
            r#"{% block content %}
<section data-pana-block="accordion"></section>
<section data-pana-block="accordion"></section>
<div data-pana-block="slider"></div>
{% endblock content %}"#,
            "",
        ));

        assert_eq!(
            NativeBlockRuntimePlan::from_template_source(&inserted.template.contents)
                .provider_ids(),
            &["accordion", "slider"]
        );

        let accordion_deleted = plan_native_block_contract(request(
            r#"{% block content %}<div data-pana-block="slider"></div>{% endblock content %}"#,
            "",
        ));
        assert_eq!(
            NativeBlockRuntimePlan::from_template_source(&accordion_deleted.template.contents)
                .provider_ids(),
            &["slider"]
        );

        let last_provider_deleted = plan_native_block_contract(request(
            r#"{% block content %}<p>Fără bloc funcțional.</p>{% endblock content %}"#,
            "",
        ));
        assert!(NativeBlockRuntimePlan::from_template_source(
            &last_provider_deleted.template.contents
        )
        .is_empty());
    }

    #[test]
    fn deleting_the_last_provider_keeps_motion_without_any_block_runtime() {
        let motion = crate::js::MotionDocument::from_value(serde_json::json!({
            "schemaVersion": 2,
            "animeVersion": crate::js::MotionRuntimeContract::current().anime_version,
            "interactions": [{
                "id": "fade",
                "name": "Fade",
                "trigger": { "type": "load" },
                "triggerTarget": { "kind": "element", "dataAnim": "hero" },
                "actions": [{
                    "type": "animate",
                    "id": "fade-action",
                    "name": "Fade",
                    "target": { "kind": "element", "dataAnim": "hero" },
                    "properties": [{
                        "id": "opacity",
                        "name": "opacity",
                        "category": "style",
                        "from": { "kind": "number", "value": "0" },
                        "to": { "kind": "number", "value": "1" }
                    }]
                }]
            }]
        }))
        .expect("valid Motion fixture");
        let plan = plan_native_block_contract(request(
            r#"{% block content %}<h1 data-anim="hero">Titlu</h1>{% endblock content %}"#,
            "",
        ));

        let config = crate::js::PageJsConfig {
            motion: Some(motion),
        };
        let runtime = crate::js::PageRuntimePlan::from_sources(&plan.template.contents, &config);
        let generated = crate::js::generate_page_js(&runtime);
        assert!(generated.contains("/js/vendor/animejs-4.4.1/timeline/index.js"));
        assert!(!generated.contains("PanaMotionRuntime"));
        assert!(!generated.contains("PanaBlockRuntime"));
        assert!(!generated.contains("PANA BLOCK PROVIDER:"));
    }

    #[test]
    fn normalizes_canonical_block_instance_ids_from_rust_contract() {
        let plan = plan_native_block_contract(request(
            r#"{% block content %}
<div data-pana-block="tabs" data-anim="ps-tabs-fresh" data-pana-instance="tabs-old">
  <button data-pana-tabs-tab>Tab</button>
</div>
<span data-pana-block="counter" data-anim="ps-counter-missing">0</span>
{% endblock content %}"#,
            "",
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
