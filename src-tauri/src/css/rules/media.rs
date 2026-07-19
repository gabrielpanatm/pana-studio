use std::collections::HashMap;

use crate::css::rules::{
    selector::{base_selector_of, locate_rule_block, locate_top_level_block},
    write::{has_non_empty_properties, insert_after_base_rule, upsert_css_rule},
};

pub fn has_media_block(source: &str, media_query: &str) -> bool {
    locate_media_block(source, media_query).is_some()
}

pub fn upsert_css_rule_in_media_ordered(
    css: &str,
    media_query: &str,
    order_px: u32,
    selector: &str,
    properties: &HashMap<String, String>,
) -> String {
    if let Some((block_start, block_end)) = locate_media_block(css, media_query) {
        let inner = &css[block_start..block_end];
        let new_inner = if locate_rule_block(inner, selector).is_some() {
            upsert_css_rule(inner, selector, properties)
        } else if let Some(base) = base_selector_of(selector) {
            if locate_rule_block(inner, base).is_some() {
                insert_after_base_rule(inner, selector, base, properties)
            } else {
                upsert_css_rule(inner, selector, properties)
            }
        } else {
            upsert_css_rule(inner, selector, properties)
        };
        format!("{}{}{}", &css[..block_start], new_inner, &css[block_end..])
    } else {
        if !has_non_empty_properties(properties) {
            return css.to_string();
        }

        let insert_pos = media_block_insert_position(css, order_px);

        let mut rule_block = String::new();
        rule_block.push_str(&format!("@media (max-width: {}) {{\n", media_query));
        rule_block.push_str(&format!("  {} {{\n", selector));
        for (prop, value) in properties {
            if value.trim().is_empty() {
                continue;
            }
            rule_block.push_str(&format!("    {}: {};\n", prop, value));
        }
        rule_block.push_str("  }\n");
        rule_block.push_str("}\n");

        let before = css[..insert_pos].trim_end_matches('\n');
        let after = &css[insert_pos..];
        format!("{}\n\n{}\n{}", before, rule_block, after)
    }
}

pub(super) fn locate_media_block(css: &str, media_query: &str) -> Option<(usize, usize)> {
    let expected = format!("@media(max-width:{})", compact_css(media_query));
    locate_top_level_block(css, |header| compact_css(header) == expected)
        .map(|(_, _, content_start, content_end, _)| (content_start, content_end))
}

fn media_block_insert_position(css: &str, new_bp_px: u32) -> usize {
    let mut cursor = 0usize;
    let mut last_end = 0usize;
    while cursor < css.len() {
        let Some((header_start, open, _, _, block_end)) =
            locate_top_level_block(&css[cursor..], |header| {
                compact_css(header).starts_with("@media")
            })
        else {
            break;
        };
        let absolute_header_start = cursor + header_start;
        let absolute_open = cursor + open;
        let absolute_block_end = cursor + block_end;
        let header = &css[absolute_header_start..absolute_open];
        if parse_max_width_px(header).is_some_and(|bp| bp < new_bp_px) {
            return css[..absolute_header_start]
                .rfind('\n')
                .map(|position| position + 1)
                .unwrap_or(0);
        }
        last_end = absolute_block_end;
        cursor = absolute_block_end;
    }

    if last_end > 0 {
        last_end
    } else {
        css.len()
    }
}

fn compact_css(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn parse_px_value(s: &str) -> Option<u32> {
    let trimmed = s.trim();
    let num_end = trimmed.find(|c: char| !c.is_ascii_digit())?;
    trimmed[..num_end].parse().ok()
}

fn parse_max_width_px(header: &str) -> Option<u32> {
    let pos = header.find("max-width")?;
    let after = &header[pos..];
    let colon = after.find(':')?;
    let value_part = after[colon + 1..].trim();
    parse_px_value(value_part)
}
