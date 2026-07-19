use std::collections::HashMap;

use crate::css::rules::{
    declarations::parse_declarations,
    selector::{base_selector_of, locate_rule_block},
};

pub fn upsert_css_rule_desktop(
    css: &str,
    selector: &str,
    properties: &HashMap<String, String>,
) -> String {
    if locate_rule_block(css, selector).is_some() {
        return upsert_css_rule(css, selector, properties);
    }

    if !has_non_empty_properties(properties) {
        return css.to_string();
    }

    if let Some(base) = base_selector_of(selector) {
        if locate_rule_block(css, base).is_some() {
            return insert_after_base_rule(css, selector, base, properties);
        }
    }

    let mut new_rule = String::new();
    new_rule.push_str(selector);
    new_rule.push_str(" {\n");
    for (prop, value) in properties {
        if value.trim().is_empty() {
            continue;
        }
        new_rule.push_str(&format!("  {}: {};\n", prop, value));
    }
    new_rule.push_str("}\n");

    match first_media_line_start(css) {
        Some(insert_pos) => {
            let before = css[..insert_pos].trim_end_matches('\n');
            let after = &css[insert_pos..];
            format!("{}\n\n{}\n{}", before, new_rule, after)
        }
        None => {
            let mut result = css.to_string();
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push('\n');
            result.push_str(&new_rule);
            result
        }
    }
}

pub(super) fn insert_after_base_rule(
    css: &str,
    selector: &str,
    base: &str,
    properties: &HashMap<String, String>,
) -> String {
    let Some((_, _, _, _, block_end)) = locate_rule_block(css, base) else {
        return upsert_css_rule(css, selector, properties);
    };

    if !has_non_empty_properties(properties) {
        return css.to_string();
    }

    let mut new_rule = String::new();
    new_rule.push_str(selector);
    new_rule.push_str(" {\n");
    for (prop, value) in properties {
        if value.trim().is_empty() {
            continue;
        }
        new_rule.push_str(&format!("  {}: {};\n", prop, value));
    }
    new_rule.push_str("}\n");

    let before = css[..block_end].trim_end_matches('\n');
    let after = css[block_end..].trim_start_matches('\n');
    format!("{}\n\n{}\n{}", before, new_rule, after)
}

fn first_media_line_start(css: &str) -> Option<usize> {
    let pos = css.find("@media")?;
    let line_start = css[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    Some(line_start)
}

pub fn upsert_css_rule(css: &str, selector: &str, properties: &HashMap<String, String>) -> String {
    if let Some((sel_start, selector_end, content_start, content_end, block_end)) =
        locate_rule_block(css, selector)
    {
        let before = &css[..sel_start];
        let selector_text = &css[sel_start..selector_end];
        let content = &css[content_start..content_end];
        let after = &css[block_end..];
        let Some(new_content) = update_declarations(content, properties) else {
            return remove_rule_block(css, sel_start, block_end);
        };
        let selector_prefix = if before.is_empty() || before.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        let after_suffix = if after.is_empty() || after.starts_with('\n') {
            ""
        } else {
            "\n"
        };
        format!(
            "{}{}{} {{{}\n}}{}{}",
            before,
            selector_prefix,
            selector_text.trim(),
            new_content,
            after_suffix,
            after,
        )
    } else {
        let mut result = css.to_string();
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        if !has_non_empty_properties(properties) {
            return result;
        }
        result.push('\n');
        result.push_str(selector);
        result.push_str(" {\n");
        for (prop, value) in properties {
            if value.trim().is_empty() {
                continue;
            }
            result.push_str(&format!("  {}: {};\n", prop, value));
        }
        result.push_str("}\n");
        result
    }
}

fn update_declarations(content: &str, properties: &HashMap<String, String>) -> Option<String> {
    if can_rewrite_as_flat_declarations(content) {
        return update_flat_declarations(content, properties);
    }

    let mut written: HashMap<String, bool> = HashMap::new();
    let mut lines: Vec<String> = Vec::new();
    let mut has_meaningful_content = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(colon_pos) = trimmed.find(':') {
            let prop = trimmed[..colon_pos].trim().to_lowercase();
            if let Some(new_value) = properties.get(&prop) {
                written.insert(prop.clone(), true);
                if !new_value.trim().is_empty() {
                    has_meaningful_content = true;
                    lines.push(format!("  {}: {};", prop, new_value));
                }
                continue;
            }
        }
        if is_meaningful_css_line(trimmed) {
            has_meaningful_content = true;
        }
        lines.push(line.to_string());
    }

    for (prop, value) in properties {
        if !written.contains_key(prop.as_str()) && !value.trim().is_empty() {
            has_meaningful_content = true;
            lines.push(format!("  {}: {};", prop, value));
        }
    }

    trim_outer_blank_lines(&mut lines);

    if !has_meaningful_content {
        return None;
    }

    Some(if lines.is_empty() {
        String::new()
    } else {
        format!("\n{}", lines.join("\n"))
    })
}

fn can_rewrite_as_flat_declarations(content: &str) -> bool {
    !content.contains('{')
        && !content.contains('}')
        && !content.contains("/*")
        && !content.contains("//")
}

fn update_flat_declarations(content: &str, properties: &HashMap<String, String>) -> Option<String> {
    let declarations = parse_declarations(content);
    let mut written: HashMap<String, bool> = HashMap::new();
    let mut lines: Vec<String> = Vec::new();

    for declaration in declarations {
        let property_key = declaration.property.trim().to_lowercase();
        if let Some(new_value) = properties.get(&property_key) {
            written.insert(property_key.clone(), true);
            if !new_value.trim().is_empty() {
                lines.push(format!("  {}: {};", property_key, new_value.trim()));
            }
            continue;
        }

        if !declaration.value.trim().is_empty() {
            lines.push(format!(
                "  {}: {};",
                declaration.property.trim(),
                declaration.value.trim()
            ));
        }
    }

    for (prop, value) in properties {
        if !written.contains_key(prop.as_str()) && !value.trim().is_empty() {
            lines.push(format!("  {}: {};", prop, value.trim()));
        }
    }

    if lines.is_empty() {
        return None;
    }

    Some(format!("\n{}", lines.join("\n")))
}

pub(super) fn has_non_empty_properties(properties: &HashMap<String, String>) -> bool {
    properties.values().any(|value| !value.trim().is_empty())
}

fn is_meaningful_css_line(trimmed: &str) -> bool {
    !trimmed.is_empty()
        && !trimmed.starts_with("//")
        && !trimmed.starts_with("/*")
        && !trimmed.starts_with('*')
}

fn trim_outer_blank_lines(lines: &mut Vec<String>) {
    while lines.first().is_some_and(|line| line.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
}

fn remove_rule_block(css: &str, sel_start: usize, block_end: usize) -> String {
    let before = css[..sel_start].trim_end_matches('\n');
    let after = css[block_end..].trim_start_matches('\n');

    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{}\n", after.trim_end_matches('\n')),
        (false, true) => format!("{}\n", before),
        (false, false) => format!("{}\n\n{}", before, after),
    }
}
