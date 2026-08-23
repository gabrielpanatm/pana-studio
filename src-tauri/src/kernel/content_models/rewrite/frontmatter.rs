use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

use toml_edit::{DocumentMut, Item, Table};

use crate::source_graph::zola::zola_frontmatter_range;

pub(in crate::kernel::content_models) fn remove_nested_value(
    value: &mut serde_json::Value,
    path: &[String],
) -> bool {
    let Some(key) = path.first() else {
        return false;
    };
    match value {
        serde_json::Value::Object(object) => {
            if path.len() == 1 {
                object.remove(key).is_some()
            } else {
                object
                    .get_mut(key)
                    .is_some_and(|child| remove_nested_value(child, &path[1..]))
            }
        }
        serde_json::Value::Array(items) => {
            let mut changed = false;
            for item in items {
                changed = remove_nested_value(item, path) || changed;
            }
            changed
        }
        _ => false,
    }
}

pub(in crate::kernel::content_models) fn rename_nested_value(
    value: &mut serde_json::Value,
    old_path: &[String],
    new_path: &[String],
) -> Result<bool, String> {
    if old_path.len() != new_path.len() || old_path.is_empty() {
        return Err("căile imbricate nu au aceeași structură".to_string());
    }
    match value {
        serde_json::Value::Object(object) => {
            if old_path.len() == 1 {
                let old_key = &old_path[0];
                let new_key = &new_path[0];
                if old_key == new_key || !object.contains_key(old_key) {
                    return Ok(false);
                }
                if object.contains_key(new_key) {
                    return Err(format!("cheia {new_key} există deja"));
                }
                let value = object
                    .remove(old_key)
                    .expect("key presence checked before removal");
                object.insert(new_key.clone(), value);
                Ok(true)
            } else {
                if old_path[0] != new_path[0] {
                    return Err("părintele câmpului s-a schimbat".to_string());
                }
                match object.get_mut(&old_path[0]) {
                    Some(child) => rename_nested_value(child, &old_path[1..], &new_path[1..]),
                    None => Ok(false),
                }
            }
        }
        serde_json::Value::Array(items) => {
            let mut changed = false;
            for item in items {
                changed = rename_nested_value(item, old_path, new_path)? || changed;
            }
            Ok(changed)
        }
        _ => Ok(false),
    }
}

pub(in crate::kernel::content_models) fn structurally_empty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::Object(object) => object.is_empty(),
        serde_json::Value::Array(items) => items.is_empty() || items.iter().all(structurally_empty),
        _ => false,
    }
}

pub(in crate::kernel::content_models) fn required_source(
    _project_root: &Path,
    source_texts: &HashMap<String, String>,
    path: &str,
) -> Result<String, String> {
    source_texts
        .get(path)
        .cloned()
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește sursa {path}."))
}

pub(in crate::kernel::content_models) fn rewrite_extra_values(
    source: &str,
    managed_keys: &BTreeSet<String>,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<String, String> {
    let (start, end) = zola_frontmatter_range(source)
        .ok_or_else(|| "Pagina nu are frontmatter Zola delimitat valid.".to_string())?;
    let frontmatter = &source[start..end];
    let rendered = if source.trim_start_matches('\u{feff}').starts_with("+++") {
        rewrite_toml_extra(frontmatter, managed_keys, values)?
    } else {
        rewrite_yaml_extra(frontmatter, managed_keys, values)?
    };
    let mut next = source.to_string();
    next.replace_range(start..end, &with_leading_newline(rendered));
    Ok(next)
}

fn rewrite_toml_extra(
    frontmatter: &str,
    managed_keys: &BTreeSet<String>,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<String, String> {
    let mut document = frontmatter
        .parse::<DocumentMut>()
        .map_err(|error| format!("Frontmatter TOML invalid: {error}"))?;
    if document.get("extra").is_none() {
        document
            .as_table_mut()
            .insert("extra", Item::Table(Table::new()));
    }
    let extra = document
        .get_mut("extra")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| "Câmpul extra trebuie să fie un tabel TOML.".to_string())?;
    for key in managed_keys {
        extra.remove(key);
    }
    let serializable = values
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut values_document = toml_edit::ser::to_document(&serializable)
        .map_err(|error| format!("Valorile nu pot fi serializate TOML: {error}"))?;
    for key in serializable.keys() {
        if let Some(item) = values_document.as_table_mut().remove(key) {
            extra.insert(key, item);
        }
    }
    if extra.is_empty() {
        document.as_table_mut().remove("extra");
    }
    Ok(document.to_string())
}

fn rewrite_yaml_extra(
    frontmatter: &str,
    managed_keys: &BTreeSet<String>,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<String, String> {
    let mut root = serde_yaml::from_str::<serde_yaml::Value>(frontmatter)
        .map_err(|error| format!("Frontmatter YAML invalid: {error}"))?;
    let mapping = root
        .as_mapping_mut()
        .ok_or_else(|| "Frontmatter YAML trebuie să fie un obiect.".to_string())?;
    let extra_key = serde_yaml::Value::String("extra".to_string());
    if !mapping.contains_key(&extra_key) {
        mapping.insert(
            extra_key.clone(),
            serde_yaml::Value::Mapping(Default::default()),
        );
    }
    let extra = mapping
        .get_mut(&extra_key)
        .and_then(serde_yaml::Value::as_mapping_mut)
        .ok_or_else(|| "Câmpul extra trebuie să fie un obiect YAML.".to_string())?;
    for key in managed_keys {
        extra.remove(serde_yaml::Value::String(key.clone()));
    }
    for (key, value) in values.iter().filter(|(_, value)| !value.is_null()) {
        let yaml = serde_yaml::to_value(value)
            .map_err(|error| format!("Valoarea {key} nu poate fi serializată YAML: {error}"))?;
        extra.insert(serde_yaml::Value::String(key.clone()), yaml);
    }
    if extra.is_empty() {
        mapping.remove(&extra_key);
    }
    let serializable = values
        .iter()
        .filter(|(_, value)| !value.is_null())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let rendered = rewrite_yaml_extra_losslessly(frontmatter, managed_keys, &serializable, &root)?;
    let reparsed = serde_yaml::from_str::<serde_yaml::Value>(&rendered)
        .map_err(|error| format!("Frontmatter YAML rescris invalid: {error}"))?;
    if reparsed != root {
        return Err(
            "Rescrierea YAML lossless nu reproduce valorile planificate; operația a fost anulată."
                .to_string(),
        );
    }
    Ok(rendered)
}

fn rewrite_yaml_extra_losslessly(
    frontmatter: &str,
    managed_keys: &BTreeSet<String>,
    values: &BTreeMap<String, serde_json::Value>,
    desired_root: &serde_yaml::Value,
) -> Result<String, String> {
    let lines = frontmatter.split_inclusive('\n').collect::<Vec<_>>();
    let extra_line = lines
        .iter()
        .position(|line| yaml_mapping_key(line, 0).is_some_and(|(key, _)| key == "extra"));
    let desired_extra_empty = desired_root
        .as_mapping()
        .and_then(|mapping| mapping.get(serde_yaml::Value::String("extra".to_string())))
        .is_none();

    let Some(extra_line) = extra_line else {
        if values.is_empty() {
            return Ok(frontmatter.to_string());
        }
        let mut rendered = frontmatter.to_string();
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str("extra:\n");
        rendered.push_str(&render_yaml_entries(values, 2)?);
        return Ok(rendered);
    };

    let (_, colon) =
        yaml_mapping_key(lines[extra_line], 0).expect("the extra mapping line was located above");
    let inline = lines[extra_line][colon + 1..]
        .trim_end_matches(['\r', '\n'])
        .trim();
    if !inline.is_empty() && !inline.starts_with('#') {
        return Err(
            "Câmpul YAML `extra` folosește forma inline; conversia vizuală este oprită pentru a nu pierde formatarea sau comentariile. Transformă-l în `extra:` cu chei indentate."
                .to_string(),
        );
    }

    let extra_end = (extra_line + 1..lines.len())
        .find(|index| {
            let line = lines[*index];
            !yaml_blank_or_comment(line) && yaml_indentation(line) == 0
        })
        .unwrap_or(lines.len());
    let child_indent = lines[extra_line + 1..extra_end]
        .iter()
        .filter(|line| !yaml_blank_or_comment(line))
        .map(|line| yaml_indentation(line))
        .filter(|indent| *indent > 0)
        .min()
        .unwrap_or(2);

    let direct_keys = (extra_line + 1..extra_end)
        .filter_map(|index| {
            yaml_mapping_key(lines[index], child_indent).map(|(key, _)| (index, key))
        })
        .collect::<Vec<_>>();
    let mut removed = vec![false; lines.len()];
    for (position, (start, key)) in direct_keys.iter().enumerate() {
        if !managed_keys.contains(key) {
            continue;
        }
        let end = direct_keys
            .get(position + 1)
            .map(|(index, _)| *index)
            .unwrap_or(extra_end);
        for index in *start..end {
            if !yaml_blank_or_comment(lines[index]) {
                removed[index] = true;
            }
        }
    }

    if desired_extra_empty {
        removed[extra_line] = true;
        for index in extra_line + 1..extra_end {
            if !yaml_blank_or_comment(lines[index]) {
                removed[index] = true;
            }
        }
    }

    let inserted = if desired_extra_empty || values.is_empty() {
        String::new()
    } else {
        render_yaml_entries(values, child_indent)?
    };
    let mut rendered = String::with_capacity(frontmatter.len() + inserted.len());
    for (index, line) in lines.iter().enumerate() {
        if index == extra_end {
            push_yaml_insertion(&mut rendered, &inserted);
        }
        if !removed[index] {
            rendered.push_str(line);
        }
    }
    if extra_end == lines.len() {
        push_yaml_insertion(&mut rendered, &inserted);
    }
    Ok(rendered)
}

fn push_yaml_insertion(rendered: &mut String, inserted: &str) {
    if inserted.is_empty() {
        return;
    }
    if !rendered.is_empty() && !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered.push_str(inserted);
}

fn render_yaml_entries(
    values: &BTreeMap<String, serde_json::Value>,
    indentation: usize,
) -> Result<String, String> {
    let serialized = serde_yaml::to_string(values)
        .map_err(|error| format!("Valorile nu pot fi serializate YAML: {error}"))?;
    let serialized = serialized.trim_start_matches("---\n");
    let prefix = " ".repeat(indentation);
    Ok(serialized
        .split_inclusive('\n')
        .map(|line| format!("{prefix}{line}"))
        .collect())
}

fn yaml_mapping_key(line: &str, expected_indent: usize) -> Option<(String, usize)> {
    if yaml_indentation(line) != expected_indent {
        return None;
    }
    let line = line.trim_end_matches(['\r', '\n']);
    let text = &line[expected_indent..];
    if text.is_empty() || text.starts_with(['#', '-']) {
        return None;
    }
    let mut single_quote = false;
    let mut double_quote = false;
    let mut escaped = false;
    for (offset, character) in text.char_indices() {
        if double_quote && escaped {
            escaped = false;
            continue;
        }
        if double_quote && character == '\\' {
            escaped = true;
            continue;
        }
        match character {
            '\'' if !double_quote => single_quote = !single_quote,
            '"' if !single_quote => double_quote = !double_quote,
            ':' if !single_quote && !double_quote => {
                let raw_key = text[..offset].trim();
                let value = serde_yaml::from_str::<serde_yaml::Value>(raw_key).ok()?;
                let key = value.as_str()?.to_string();
                return Some((key, expected_indent + offset));
            }
            _ => {}
        }
    }
    None
}

fn yaml_indentation(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ')
        .count()
}

fn yaml_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

fn with_leading_newline(rendered: String) -> String {
    if rendered.starts_with('\n') {
        rendered
    } else {
        format!("\n{rendered}")
    }
}
