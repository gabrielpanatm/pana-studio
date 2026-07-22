use std::collections::{BTreeMap, BTreeSet};

use crate::{
    project_model::model::{ProjectModel, ProjectModelFile, ProjectModelFileKind},
    source_graph::model::SourceRange,
};

use super::model::{
    DesignClassEntry, DesignClassInventorySnapshot, DesignClassOccurrence,
    DesignClassOccurrenceKind, DesignClassRenameChange, DesignClassRenamePlan,
    DESIGN_CLASS_INVENTORY_SCHEMA_VERSION,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawOccurrence {
    start: usize,
    end: usize,
    kind: DesignClassOccurrenceKind,
}

pub fn build_design_class_inventory(
    model: &ProjectModel,
    runtime_session_id: String,
    workspace_revision: u64,
) -> DesignClassInventorySnapshot {
    let mut grouped: BTreeMap<String, Vec<DesignClassOccurrence>> = BTreeMap::new();
    for file in &model.files {
        for (name, occurrence) in file_class_occurrences(file) {
            grouped
                .entry(name)
                .or_default()
                .push(DesignClassOccurrence {
                    file: file.relative_path.clone(),
                    kind: occurrence.kind,
                    range: range_at(&file.contents, occurrence.start, occurrence.end),
                });
        }
    }
    let classes = grouped
        .into_iter()
        .map(|(name, mut occurrences)| {
            occurrences.sort_by(|left, right| {
                left.file
                    .cmp(&right.file)
                    .then_with(|| left.range.start.cmp(&right.range.start))
            });
            let files = occurrences
                .iter()
                .map(|occurrence| occurrence.file.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let markup_occurrences = occurrences
                .iter()
                .filter(|occurrence| occurrence.kind == DesignClassOccurrenceKind::Markup)
                .count();
            let selector_occurrences = occurrences.len() - markup_occurrences;
            DesignClassEntry {
                name,
                markup_occurrences,
                selector_occurrences,
                files,
                occurrences,
            }
        })
        .collect();

    DesignClassInventorySnapshot {
        schema_version: DESIGN_CLASS_INVENTORY_SCHEMA_VERSION,
        project_root: model.project_root.to_string_lossy().to_string(),
        runtime_session_id,
        workspace_revision,
        project_model_revision: model.revision.clone(),
        classes,
    }
}

pub fn plan_design_class_rename(
    model: &ProjectModel,
    old_name: &str,
    new_name: &str,
) -> Result<DesignClassRenamePlan, String> {
    let old_name = validate_class_name(old_name, "Clasa sursă")?;
    let new_name = validate_class_name(new_name, "Clasa destinație")?;
    if old_name == new_name {
        return Err("Redenumirea clasei nu schimbă numele.".to_string());
    }

    let inventory = build_design_class_inventory(model, String::new(), 0);
    if !inventory.classes.iter().any(|entry| entry.name == old_name) {
        return Err(format!("Clasa .{old_name} nu există în sursele indexate."));
    }
    if inventory.classes.iter().any(|entry| entry.name == new_name) {
        return Err(format!(
            "Clasa .{new_name} există deja. Unificarea a două clase cere o operație explicită, nu rename."
        ));
    }

    let mut changes = Vec::new();
    let mut total = 0;
    for file in &model.files {
        let mut ranges = file_class_occurrences(file)
            .into_iter()
            .filter_map(|(name, occurrence)| (name == old_name).then_some(occurrence))
            .collect::<Vec<_>>();
        if ranges.is_empty() {
            continue;
        }
        ranges.sort_by(|left, right| right.start.cmp(&left.start));
        let mut contents = file.contents.clone();
        for occurrence in &ranges {
            contents.replace_range(occurrence.start..occurrence.end, &new_name);
        }
        total += ranges.len();
        changes.push(DesignClassRenameChange {
            relative_path: file.relative_path.clone(),
            contents,
            replacement_count: ranges.len(),
        });
    }

    Ok(DesignClassRenamePlan {
        old_name,
        new_name,
        changes,
        replacement_count: total,
    })
}

fn validate_class_name(value: &str, label: &str) -> Result<String, String> {
    let value = value.trim().trim_start_matches('.');
    if value.is_empty() {
        return Err(format!("{label} nu poate fi goală."));
    }
    if value.len() > 128 {
        return Err(format!("{label} depășește limita de 128 de caractere."));
    }
    let mut characters = value.chars();
    let first = characters.next().unwrap_or_default();
    if !(first.is_ascii_alphabetic() || first == '_' || first == '-')
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Err(format!(
            "{label} trebuie să fie un identificator CSS simplu (litere, cifre, _ și -)."
        ));
    }
    Ok(value.to_string())
}

fn file_class_occurrences(file: &ProjectModelFile) -> Vec<(String, RawOccurrence)> {
    match file.kind {
        ProjectModelFileKind::Template | ProjectModelFileKind::Content => {
            markup_class_occurrences(&file.contents)
        }
        ProjectModelFileKind::Style => style_class_occurrences(&file.contents),
        _ => Vec::new(),
    }
}

fn markup_class_occurrences(source: &str) -> Vec<(String, RawOccurrence)> {
    let bytes = source.as_bytes();
    let lower = source.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let mut output = Vec::new();
    let mut cursor = 0;

    while cursor + 5 <= bytes.len() {
        let Some(relative) = lower[cursor..].find("class") else {
            break;
        };
        let start = cursor + relative;
        let end = start + 5;
        let before_ok = start == 0 || !is_attribute_name_byte(lower_bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_attribute_name_byte(lower_bytes[end]);
        if !before_ok || !after_ok {
            cursor = end;
            continue;
        }
        let mut index = end;
        skip_ascii_whitespace(bytes, &mut index);
        if bytes.get(index) != Some(&b'=') {
            cursor = end;
            continue;
        }
        index += 1;
        skip_ascii_whitespace(bytes, &mut index);
        let Some(&quote) = bytes.get(index) else {
            break;
        };
        if quote != b'\'' && quote != b'"' {
            cursor = end;
            continue;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        for (token_start, token_end) in ascii_tokens(bytes, value_start, index) {
            let token = &source[token_start..token_end];
            if is_simple_class_name(token) {
                output.push((
                    token.to_string(),
                    RawOccurrence {
                        start: token_start,
                        end: token_end,
                        kind: DesignClassOccurrenceKind::Markup,
                    },
                ));
            }
        }
        cursor = index + 1;
    }
    output
}

fn style_class_occurrences(source: &str) -> Vec<(String, RawOccurrence)> {
    let bytes = source.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    let mut quote: Option<u8> = None;
    let mut block_comment = false;
    let mut line_comment = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                index = (index + 2).min(bytes.len());
            } else {
                if byte == active_quote {
                    quote = None;
                }
                index += 1;
            }
            continue;
        }
        if (byte == b'\'' || byte == b'"') && index < bytes.len() {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            block_comment = true;
            index += 2;
            continue;
        }
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            line_comment = true;
            index += 2;
            continue;
        }
        if byte != b'.' {
            index += 1;
            continue;
        }
        let start = index + 1;
        if start >= bytes.len() || !is_class_start(bytes[start]) {
            index += 1;
            continue;
        }
        let mut end = start + 1;
        while end < bytes.len() && is_class_continue(bytes[end]) {
            end += 1;
        }
        let token = &source[start..end];
        if is_simple_class_name(token) {
            output.push((
                token.to_string(),
                RawOccurrence {
                    start,
                    end,
                    kind: DesignClassOccurrenceKind::Style,
                },
            ));
        }
        index = end;
    }
    output
}

fn skip_ascii_whitespace(bytes: &[u8], index: &mut usize) {
    while bytes.get(*index).is_some_and(u8::is_ascii_whitespace) {
        *index += 1;
    }
}

fn ascii_tokens(bytes: &[u8], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut tokens = Vec::new();
    let mut index = start;
    while index < end {
        while index < end && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let token_start = index;
        while index < end && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if token_start < index {
            tokens.push((token_start, index));
        }
    }
    tokens
}

fn is_attribute_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' || byte == b':'
}

fn is_class_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'-'
}

fn is_class_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn is_simple_class_name(value: &str) -> bool {
    validate_class_name(value, "Clasa").is_ok()
}

fn range_at(source: &str, start: usize, end: usize) -> SourceRange {
    let safe_start = start.min(source.len());
    let safe_end = end.max(safe_start).min(source.len());
    let prefix = &source[..safe_start];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map(|(_, tail)| tail.chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);
    SourceRange {
        start: safe_start,
        end: safe_end,
        line,
        column,
        end_line: line,
        end_column: column + source[safe_start..safe_end].chars().count(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        project_model::model::{ProjectModel, ProjectModelFile},
        source_graph::model::SourceGraph,
    };

    use super::*;

    fn model(files: Vec<ProjectModelFile>) -> ProjectModel {
        ProjectModel {
            project_root: PathBuf::from("/tmp/design-system-test"),
            zola_root: PathBuf::from("/tmp/design-system-test/sursa"),
            revision: "revision".to_string(),
            files,
            source_graph: SourceGraph {
                project_root: String::new(),
                zola_root: String::new(),
                active_theme: None,
                pages: Vec::new(),
                templates: Vec::new(),
                styles: Vec::new(),
                scripts: Vec::new(),
                assets: Vec::new(),
                data_files: Vec::new(),
                nodes: Vec::new(),
                relations: Vec::new(),
                diagnostics: Vec::new(),
            },
            tera_graph: crate::project_model::model::TeraGraph {
                templates: Vec::new(),
                nodes: Vec::new(),
                relations: Vec::new(),
            },
            diagnostics: Vec::new(),
        }
    }

    fn file(path: &str, kind: ProjectModelFileKind, contents: &str) -> ProjectModelFile {
        ProjectModelFile {
            relative_path: path.to_string(),
            kind,
            contents: contents.to_string(),
            size_bytes: contents.len(),
            revision: "file-revision".to_string(),
            from_draft: false,
        }
    }

    #[test]
    fn inventory_combines_markup_and_style_occurrences() {
        let model = model(vec![
            file(
                "sursa/templates/index.html",
                ProjectModelFileKind::Template,
                r#"<main class="hero hero-wide {{ dynamic }}"></main>"#,
            ),
            file(
                "sursa/sass/main.scss",
                ProjectModelFileKind::Style,
                ".hero, .hero-wide:hover { color: red; } // .ignored\n/* .also-ignored */",
            ),
        ]);
        let snapshot = build_design_class_inventory(&model, "runtime".to_string(), 3);
        let hero = snapshot
            .classes
            .iter()
            .find(|entry| entry.name == "hero")
            .unwrap();
        assert_eq!(hero.markup_occurrences, 1);
        assert_eq!(hero.selector_occurrences, 1);
        assert_eq!(hero.files.len(), 2);
        assert!(!snapshot.classes.iter().any(|entry| entry.name == "ignored"));
    }

    #[test]
    fn rename_changes_only_exact_class_tokens() {
        let model = model(vec![
            file(
                "sursa/templates/index.html",
                ProjectModelFileKind::Template,
                r#"<div class="card card-large"></div><div data-class="card"></div>"#,
            ),
            file(
                "sursa/sass/main.scss",
                ProjectModelFileKind::Style,
                ".card { &.active {} } .card-large {} content: \".card\";",
            ),
        ]);
        let plan = plan_design_class_rename(&model, ".card", "panel").unwrap();
        assert_eq!(plan.replacement_count, 2);
        assert!(plan.changes[0]
            .contents
            .contains(r#"class="panel card-large""#));
        assert!(plan.changes[0].contents.contains(r#"data-class="card""#));
        assert!(plan.changes[1].contents.contains(".panel {"));
        assert!(plan.changes[1].contents.contains(".card-large"));
        assert!(plan.changes[1].contents.contains(r#"content: ".card""#));
    }

    #[test]
    fn rename_refuses_existing_destination_class() {
        let model = model(vec![file(
            "sursa/sass/main.scss",
            ProjectModelFileKind::Style,
            ".old {} .existing {}",
        )]);
        let error = plan_design_class_rename(&model, "old", "existing").unwrap_err();
        assert!(error.contains("există deja"));
    }
}
