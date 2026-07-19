use std::collections::HashMap;

use super::rules::{
    get_class_rules as parse_class_rules, get_class_rules_in_media, has_media_block,
    upsert_css_rule_desktop, upsert_css_rule_in_media_ordered, CssProperty,
};

#[derive(Clone, Debug, Default)]
pub struct CssBreakpointValues {
    pub tablet: Option<String>,
    pub mobile: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CssRuleContext {
    pub file: String,
    pub selector: String,
    pub viewport: String,
    pub resolved_breakpoint: Option<String>,
    pub base_rules: Vec<CssProperty>,
    pub viewport_rules: Vec<CssProperty>,
    pub has_base_rule: bool,
    pub has_viewport_rule: bool,
}

pub fn get_rules_at_viewport(
    breakpoints: &CssBreakpointValues,
    source: &str,
    viewport: &str,
    selector: &str,
) -> Vec<CssProperty> {
    match viewport {
        "tablet" => get_viewport_rules(breakpoints, source, "bp-tableta", "1024px", selector),
        "mobile" => get_viewport_rules(breakpoints, source, "bp-mobil", "768px", selector),
        _ => parse_class_rules(source, selector),
    }
}

pub fn get_rule_context(
    breakpoints: &CssBreakpointValues,
    relative_path: String,
    source: &str,
    selector: String,
    viewport: String,
) -> CssRuleContext {
    let base_rules = parse_class_rules(source, &selector);
    let has_base_rule = !base_rules.is_empty();

    let (resolved_breakpoint, viewport_rules) = match viewport.as_str() {
        "tablet" => {
            get_viewport_rule_context(breakpoints, source, "bp-tableta", "1024px", &selector)
        }
        "mobile" => get_viewport_rule_context(breakpoints, source, "bp-mobil", "768px", &selector),
        _ => (None, base_rules.clone()),
    };
    let has_viewport_rule = !viewport_rules.is_empty();

    CssRuleContext {
        file: relative_path,
        selector,
        viewport,
        resolved_breakpoint,
        base_rules,
        viewport_rules,
        has_base_rule,
        has_viewport_rule,
    }
}

pub fn write_rule_at_viewport(
    breakpoints: &CssBreakpointValues,
    source: &str,
    selector: &str,
    properties: &HashMap<String, String>,
    viewport: &str,
) -> String {
    match viewport {
        "tablet" => write_viewport_rule(
            breakpoints,
            source,
            "bp-tableta",
            "1024px",
            selector,
            properties,
        ),
        "mobile" => write_viewport_rule(
            breakpoints,
            source,
            "bp-mobil",
            "768px",
            selector,
            properties,
        ),
        _ => upsert_css_rule_desktop(source, selector, properties),
    }
}

fn resolve_breakpoint(breakpoints: &CssBreakpointValues, var_name: &str) -> Option<String> {
    match var_name {
        "bp-tableta" => breakpoints.tablet.clone(),
        "bp-mobil" => breakpoints.mobile.clone(),
        _ => None,
    }
}

fn viewport_media_token(var_name: &str) -> String {
    format!("${}", var_name)
}

fn get_viewport_rules(
    breakpoints: &CssBreakpointValues,
    source: &str,
    var_name: &str,
    fallback: &str,
    selector: &str,
) -> Vec<CssProperty> {
    let token = viewport_media_token(var_name);
    let token_rules = get_class_rules_in_media(source, &token, selector);
    if !token_rules.is_empty() {
        return token_rules;
    }

    let resolved =
        resolve_breakpoint(breakpoints, var_name).unwrap_or_else(|| fallback.to_string());
    get_class_rules_in_media(source, &resolved, selector)
}

fn get_viewport_rule_context(
    breakpoints: &CssBreakpointValues,
    source: &str,
    var_name: &str,
    fallback: &str,
    selector: &str,
) -> (Option<String>, Vec<CssProperty>) {
    let token = viewport_media_token(var_name);
    let token_rules = get_class_rules_in_media(source, &token, selector);
    if !token_rules.is_empty() {
        return (Some(token), token_rules);
    }

    let resolved =
        resolve_breakpoint(breakpoints, var_name).unwrap_or_else(|| fallback.to_string());
    let resolved_rules = get_class_rules_in_media(source, &resolved, selector);
    (Some(resolved), resolved_rules)
}

fn write_viewport_rule(
    breakpoints: &CssBreakpointValues,
    source: &str,
    var_name: &str,
    fallback: &str,
    selector: &str,
    properties: &HashMap<String, String>,
) -> String {
    let token = viewport_media_token(var_name);
    let resolved =
        resolve_breakpoint(breakpoints, var_name).unwrap_or_else(|| fallback.to_string());
    let order_px = parse_breakpoint_px(&resolved)
        .unwrap_or_else(|| parse_breakpoint_px(fallback).unwrap_or(0));
    let media_query = if has_media_block(source, &token) || !has_media_block(source, &resolved) {
        token
    } else {
        resolved
    };

    upsert_css_rule_in_media_ordered(source, &media_query, order_px, selector, properties)
}

fn parse_breakpoint_px(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    let num_end = trimmed.find(|c: char| !c.is_ascii_digit())?;
    trimmed[..num_end].parse().ok()
}
