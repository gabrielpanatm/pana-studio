pub(super) fn toml_string(source: &str, section: Option<&str>, key: &str) -> Option<String> {
    let raw = toml_raw_value(source, section, key)?;
    Some(raw.trim().trim_matches('"').trim_matches('\'').to_string())
}

pub(super) fn toml_bool(source: &str, section: Option<&str>, key: &str) -> Option<bool> {
    match toml_raw_value(source, section, key)?.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub(super) fn toml_u32(source: &str, section: Option<&str>, key: &str) -> Option<u32> {
    toml_raw_value(source, section, key)?.trim().parse().ok()
}

pub(super) fn toml_string_array(source: &str, section: Option<&str>, key: &str) -> Vec<String> {
    let Some(raw) = toml_raw_value(source, section, key) else {
        return Vec::new();
    };
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Vec::new();
    }
    trimmed
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter_map(|entry| {
            let value = entry.trim().trim_matches('"').trim_matches('\'');
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

pub(super) fn toml_paginated_sitemap(source: &str) -> bool {
    if let Some(value) = toml_raw_value(source, None, "exclude_paginated_pages_in_sitemap") {
        return match value.trim().trim_matches('"').trim_matches('\'') {
            "all" | "true" => true,
            "none" | "false" => false,
            _ => false,
        };
    }
    false
}

pub(super) fn toml_raw_value(source: &str, section: Option<&str>, key: &str) -> Option<String> {
    let (start, end) = section_bounds(source, section);
    source
        .lines()
        .skip(start)
        .take(end.saturating_sub(start))
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || !toml_key_matches(trimmed, key) {
                return None;
            }
            trimmed
                .find('=')
                .map(|eq| strip_inline_comment(&trimmed[eq + 1..]).trim().to_string())
        })
}

pub(super) fn upsert_toml_value(
    source: &str,
    section: Option<&str>,
    key: &str,
    value: String,
) -> String {
    let mut lines: Vec<String> = source.lines().map(ToString::to_string).collect();
    let (mut start, mut end) = section_bounds_from_lines(&lines, section);
    if let Some(section) = section {
        if start == end && !section_exists(&lines, section) {
            if !lines.is_empty() && !lines.last().is_some_and(|line| line.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("[{}]", section));
            start = lines.len();
            end = start;
        }
    }

    for index in start..end {
        if toml_key_matches(lines[index].trim(), key) {
            let indent: String = lines[index]
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            lines[index] = format!("{}{} = {}", indent, key, value);
            return finish_toml_lines(lines);
        }
    }

    lines.insert(end, format!("{} = {}", key, value));
    finish_toml_lines(lines)
}

pub(super) fn remove_toml_key(source: &str, section: Option<&str>, key: &str) -> String {
    let mut lines: Vec<String> = source.lines().map(ToString::to_string).collect();
    let (start, end) = section_bounds_from_lines(&lines, section);
    lines = lines
        .into_iter()
        .enumerate()
        .filter_map(|(index, line)| {
            if index >= start && index < end && toml_key_matches(line.trim(), key) {
                None
            } else {
                Some(line)
            }
        })
        .collect();
    finish_toml_lines(lines)
}

pub(super) fn upsert_or_remove_u32(
    source: &str,
    section: Option<&str>,
    key: &str,
    value: Option<u32>,
) -> String {
    value
        .map(|value| upsert_toml_value(source, section, key, value.to_string()))
        .unwrap_or_else(|| remove_toml_key(source, section, key))
}

fn section_bounds(source: &str, section: Option<&str>) -> (usize, usize) {
    let lines: Vec<String> = source.lines().map(ToString::to_string).collect();
    section_bounds_from_lines(&lines, section)
}

fn section_bounds_from_lines(lines: &[String], section: Option<&str>) -> (usize, usize) {
    let Some(section) = section else {
        let end = lines
            .iter()
            .position(|line| is_section_header(line.trim()))
            .unwrap_or(lines.len());
        return (0, end);
    };

    let header = format!("[{}]", section);
    let Some(header_index) = lines.iter().position(|line| line.trim() == header) else {
        return (lines.len(), lines.len());
    };
    let start = header_index + 1;
    let end = lines[start..]
        .iter()
        .position(|line| is_section_header(line.trim()))
        .map(|relative| start + relative)
        .unwrap_or(lines.len());
    (start, end)
}

fn section_exists(lines: &[String], section: &str) -> bool {
    let header = format!("[{}]", section);
    lines.iter().any(|line| line.trim() == header)
}

fn is_section_header(trimmed: &str) -> bool {
    trimmed.starts_with('[') && trimmed.ends_with(']')
}

fn toml_key_matches(trimmed: &str, key: &str) -> bool {
    trimmed
        .strip_prefix(key)
        .and_then(|rest| rest.trim_start().strip_prefix('='))
        .is_some()
}

fn strip_inline_comment(value: &str) -> &str {
    value.split('#').next().unwrap_or(value)
}

pub(super) fn toml_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn toml_array(values: &[String]) -> String {
    let values = values
        .iter()
        .filter(|value| !value.trim().is_empty())
        .map(|value| toml_quote(value.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", values)
}

fn finish_toml_lines(lines: Vec<String>) -> String {
    let mut result = lines.join("\n");
    result.push('\n');
    result
}

pub(super) fn extract_toml_string(source: &str, key: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) {
            if let Some(eq) = trimmed.find('=') {
                let val = trimmed[eq + 1..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                return Some(val.to_string());
            }
        }
    }
    None
}

pub(super) fn upsert_toml_string(source: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
    let mut found = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with(key) && trimmed.contains('=') {
            *line = format!("{} = \"{}\"", key, value);
            found = true;
            break;
        }
    }
    if !found {
        lines.insert(0, format!("{} = \"{}\"", key, value));
    }
    lines.join("\n") + "\n"
}
