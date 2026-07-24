use std::{
    hash::{Hash, Hasher},
    ops::Range,
};

use toml_edit::{Document, DocumentMut, Item, Table, Value};

use crate::source_graph::{
    model::{
        SourceDataFormat, SourceDataNode, SourceDataNodeKind, SourceDataPathSegment,
        SourceDataValueKind,
    },
    scan::ranges::source_range,
};

pub(crate) struct LosslessTomlDocument {
    source: String,
    _document: DocumentMut,
    pub(crate) nodes: Vec<SourceDataNode>,
}

impl LosslessTomlDocument {
    pub(crate) fn reconstruct(&self) -> String {
        self.source.clone()
    }

    pub(crate) fn is_lossless(&self) -> bool {
        self.reconstruct() == self.source
    }
}

pub(crate) fn data_format_for_file(file: &str) -> SourceDataFormat {
    match file
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("toml") => SourceDataFormat::Toml,
        Some("json") => SourceDataFormat::Json,
        Some("yaml" | "yml") => SourceDataFormat::Yaml,
        Some("csv") => SourceDataFormat::Csv,
        Some("bib" | "bibtex") => SourceDataFormat::Bibtex,
        Some("xml") => SourceDataFormat::Xml,
        _ => SourceDataFormat::Unknown,
    }
}

pub(crate) fn parse_lossless_toml(
    source: &str,
    file: &str,
) -> Result<LosslessTomlDocument, String> {
    let parsed = Document::parse(source.to_string()).map_err(|error| error.to_string())?;
    let mut nodes = vec![SourceDataNode {
        id: data_node_id(file, &[], &SourceDataNodeKind::Document, 0),
        kind: SourceDataNodeKind::Document,
        path: Vec::new(),
        key: None,
        value_kind: None,
        value_preview: None,
        range: Some(source_range(source, 0, source.len())),
        key_range: None,
        parent_id: None,
        children: Vec::new(),
    }];
    let root_id = nodes[0].id.clone();
    project_table(parsed.as_table(), source, file, &[], &root_id, &mut nodes);
    project_comments(source, file, &root_id, &mut nodes);
    rebuild_children(&mut nodes);

    let projection = LosslessTomlDocument {
        source: source.to_string(),
        _document: parsed.into_mut(),
        nodes,
    };
    debug_assert!(projection.is_lossless());
    Ok(projection)
}

pub(crate) fn parse_zola_data_adapter(
    source: &str,
    file: &str,
    format: &SourceDataFormat,
) -> Result<Vec<SourceDataNode>, String> {
    let value = match format {
        SourceDataFormat::Json => {
            serde_json::from_str(source).map_err(|error| error.to_string())?
        }
        SourceDataFormat::Yaml => {
            serde_yaml::from_str(source).map_err(|error| error.to_string())?
        }
        SourceDataFormat::Csv => csv_value(source)?,
        SourceDataFormat::Bibtex => bibtex_value(source)?,
        SourceDataFormat::Xml => roxmltree_to_serde::xml_str_to_json(source, &Default::default())
            .map_err(|error| error.to_string())?,
        SourceDataFormat::Toml | SourceDataFormat::Unknown => {
            return Err("Formatul nu folosește un adaptor Zola extern.".to_string())
        }
    };

    let root_id = data_node_id(file, &[], &SourceDataNodeKind::Document, 0);
    let mut nodes = vec![SourceDataNode {
        id: root_id.clone(),
        kind: SourceDataNodeKind::Document,
        path: Vec::new(),
        key: None,
        value_kind: None,
        value_preview: None,
        range: Some(source_range(source, 0, source.len())),
        key_range: None,
        parent_id: None,
        children: Vec::new(),
    }];
    project_json_children(&value, file, &[], &root_id, &mut nodes);
    rebuild_children(&mut nodes);
    Ok(nodes)
}

pub(crate) fn rebase_data_node_ranges(
    nodes: &mut [SourceDataNode],
    full_source: &str,
    offset: usize,
) {
    for node in nodes {
        if let Some(range) = node.range.as_mut() {
            *range = source_range(
                full_source,
                offset.saturating_add(range.start),
                offset.saturating_add(range.end),
            );
        }
        if let Some(range) = node.key_range.as_mut() {
            *range = source_range(
                full_source,
                offset.saturating_add(range.start),
                offset.saturating_add(range.end),
            );
        }
    }
}

fn csv_value(source: &str) -> Result<serde_json::Value, String> {
    let mut reader = csv::Reader::from_reader(source.as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| format!("Antet CSV invalid: {error}"))?
        .iter()
        .map(|value| serde_json::Value::String(value.to_string()))
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    for result in reader.records() {
        let record = result
            .map_err(|error| format!("Înregistrare CSV invalidă: {error}"))?
            .iter()
            .map(|value| serde_json::Value::String(value.to_string()))
            .collect::<Vec<_>>();
        records.push(serde_json::Value::Array(record));
    }
    Ok(serde_json::json!({
        "headers": headers,
        "records": records,
    }))
}

fn bibtex_value(source: &str) -> Result<serde_json::Value, String> {
    let model = nom_bibtex::Bibtex::parse(source).map_err(|error| error.to_string())?;
    let preambles = model
        .preambles()
        .iter()
        .map(|value| serde_json::Value::String(value.to_string()))
        .collect::<Vec<_>>();
    let comments = model
        .comments()
        .iter()
        .map(|value| serde_json::Value::String(value.to_string()))
        .collect::<Vec<_>>();
    let variables = model
        .variables()
        .iter()
        .map(|(key, value)| {
            (
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let bibliographies = model
        .bibliographies()
        .iter()
        .map(|bibliography| {
            let tags = bibliography
                .tags()
                .iter()
                .map(|(key, value)| {
                    (
                        key.to_lowercase(),
                        serde_json::Value::String(value.to_string()),
                    )
                })
                .collect::<serde_json::Map<_, _>>();
            serde_json::json!({
                "entry_type": bibliography.entry_type(),
                "citation_key": bibliography.citation_key(),
                "tags": tags,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "preambles": preambles,
        "comments": comments,
        "variables": variables,
        "bibliographies": bibliographies,
    }))
}

fn project_json_children(
    value: &serde_json::Value,
    file: &str,
    parent_path: &[SourceDataPathSegment],
    parent_id: &str,
    nodes: &mut Vec<SourceDataNode>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let mut path = parent_path.to_vec();
                path.push(SourceDataPathSegment::Key(key.clone()));
                project_json_value(value, file, path, Some(key.clone()), parent_id, nodes);
            }
        }
        serde_json::Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                let mut path = parent_path.to_vec();
                path.push(SourceDataPathSegment::Index(index));
                project_json_value(value, file, path, Some(index.to_string()), parent_id, nodes);
            }
        }
        _ => {}
    }
}

fn project_json_value(
    value: &serde_json::Value,
    file: &str,
    path: Vec<SourceDataPathSegment>,
    key: Option<String>,
    parent_id: &str,
    nodes: &mut Vec<SourceDataNode>,
) {
    let (kind, value_kind, preview) = match value {
        serde_json::Value::Object(object) => (
            SourceDataNodeKind::Table,
            Some(SourceDataValueKind::Table),
            Some(format!("{} câmpuri", object.len())),
        ),
        serde_json::Value::Array(array) => (
            SourceDataNodeKind::Array,
            Some(SourceDataValueKind::Array),
            Some(format!("{} elemente", array.len())),
        ),
        serde_json::Value::String(value) => (
            SourceDataNodeKind::Value,
            Some(SourceDataValueKind::String),
            Some(bounded_preview(value)),
        ),
        serde_json::Value::Number(value) => (
            SourceDataNodeKind::Value,
            Some(if value.is_i64() || value.is_u64() {
                SourceDataValueKind::Integer
            } else {
                SourceDataValueKind::Float
            }),
            Some(value.to_string()),
        ),
        serde_json::Value::Bool(value) => (
            SourceDataNodeKind::Value,
            Some(SourceDataValueKind::Boolean),
            Some(value.to_string()),
        ),
        serde_json::Value::Null => (
            SourceDataNodeKind::Value,
            Some(SourceDataValueKind::Null),
            Some("null".to_string()),
        ),
    };
    let occurrence = nodes
        .iter()
        .filter(|node| node.kind == kind && node.path == path)
        .count();
    let id = data_node_id(file, &path, &kind, occurrence);
    nodes.push(SourceDataNode {
        id: id.clone(),
        kind,
        path: path.clone(),
        key,
        value_kind,
        value_preview: preview,
        // Adaptoarele Zola oferă valoarea semantică, nu token spans. Fără
        // range exact, plannerul trebuie să refuze rescrierea aproximativă.
        range: None,
        key_range: None,
        parent_id: Some(parent_id.to_string()),
        children: Vec::new(),
    });
    project_json_children(value, file, &path, &id, nodes);
}

fn bounded_preview(value: &str) -> String {
    let mut chars = value.chars();
    let bounded = chars.by_ref().take(160).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

fn project_table(
    table: &Table,
    source: &str,
    file: &str,
    parent_path: &[SourceDataPathSegment],
    parent_id: &str,
    nodes: &mut Vec<SourceDataNode>,
) {
    for (key, item) in table.iter() {
        let mut path = parent_path.to_vec();
        path.push(SourceDataPathSegment::Key(key.to_string()));
        let key_range = table
            .key(key)
            .and_then(|key| key.span())
            .map(|range| source_range(source, range.start, range.end));
        project_item(
            item,
            source,
            file,
            path,
            Some(key.to_string()),
            key_range,
            parent_id,
            nodes,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn project_item(
    item: &Item,
    source: &str,
    file: &str,
    path: Vec<SourceDataPathSegment>,
    key: Option<String>,
    key_range: Option<crate::source_graph::model::SourceRange>,
    parent_id: &str,
    nodes: &mut Vec<SourceDataNode>,
) {
    match item {
        Item::None => {}
        Item::Table(table) => {
            let id = push_node(
                source,
                file,
                SourceDataNodeKind::Table,
                path.clone(),
                key,
                None,
                None,
                item.span(),
                key_range,
                parent_id,
                nodes,
            );
            project_table(table, source, file, &path, &id, nodes);
        }
        Item::ArrayOfTables(array) => {
            let id = push_node(
                source,
                file,
                SourceDataNodeKind::ArrayOfTables,
                path.clone(),
                key,
                Some(SourceDataValueKind::ArrayOfTables),
                Some(format!("{} elemente", array.len())),
                item.span(),
                key_range,
                parent_id,
                nodes,
            );
            for (index, table) in array.iter().enumerate() {
                let mut element_path = path.clone();
                element_path.push(SourceDataPathSegment::Index(index));
                let element_id = push_node(
                    source,
                    file,
                    SourceDataNodeKind::TableElement,
                    element_path.clone(),
                    Some(index.to_string()),
                    Some(SourceDataValueKind::Table),
                    None,
                    table.span(),
                    None,
                    &id,
                    nodes,
                );
                project_table(table, source, file, &element_path, &element_id, nodes);
            }
        }
        Item::Value(value) => project_value(
            value,
            source,
            file,
            path,
            key,
            key_range,
            parent_id,
            nodes,
            SourceDataNodeKind::Value,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_value(
    value: &Value,
    source: &str,
    file: &str,
    path: Vec<SourceDataPathSegment>,
    key: Option<String>,
    key_range: Option<crate::source_graph::model::SourceRange>,
    parent_id: &str,
    nodes: &mut Vec<SourceDataNode>,
    scalar_kind: SourceDataNodeKind,
) {
    match value {
        Value::Array(array) => {
            let id = push_node(
                source,
                file,
                SourceDataNodeKind::Array,
                path.clone(),
                key,
                Some(SourceDataValueKind::Array),
                Some(format!("{} elemente", array.len())),
                value.span(),
                key_range,
                parent_id,
                nodes,
            );
            for (index, element) in array.iter().enumerate() {
                let mut element_path = path.clone();
                element_path.push(SourceDataPathSegment::Index(index));
                project_value(
                    element,
                    source,
                    file,
                    element_path,
                    Some(index.to_string()),
                    None,
                    &id,
                    nodes,
                    SourceDataNodeKind::ArrayElement,
                );
            }
        }
        Value::InlineTable(table) => {
            let id = push_node(
                source,
                file,
                SourceDataNodeKind::InlineTable,
                path.clone(),
                key,
                Some(SourceDataValueKind::InlineTable),
                Some(format!("{} câmpuri", table.len())),
                value.span(),
                key_range,
                parent_id,
                nodes,
            );
            for (field, field_value) in table.iter() {
                let mut field_path = path.clone();
                field_path.push(SourceDataPathSegment::Key(field.to_string()));
                let field_key_range = table
                    .key(field)
                    .and_then(|key| key.span())
                    .map(|range| source_range(source, range.start, range.end));
                project_value(
                    field_value,
                    source,
                    file,
                    field_path,
                    Some(field.to_string()),
                    field_key_range,
                    &id,
                    nodes,
                    SourceDataNodeKind::Value,
                );
            }
        }
        _ => {
            push_node(
                source,
                file,
                scalar_kind,
                path,
                key,
                Some(value_kind(value)),
                Some(value_preview(value)),
                value.span(),
                key_range,
                parent_id,
                nodes,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_node(
    source: &str,
    file: &str,
    kind: SourceDataNodeKind,
    path: Vec<SourceDataPathSegment>,
    key: Option<String>,
    value_kind: Option<SourceDataValueKind>,
    value_preview: Option<String>,
    span: Option<Range<usize>>,
    key_range: Option<crate::source_graph::model::SourceRange>,
    parent_id: &str,
    nodes: &mut Vec<SourceDataNode>,
) -> String {
    let occurrence = nodes
        .iter()
        .filter(|node| node.kind == kind && node.path == path)
        .count();
    let id = data_node_id(file, &path, &kind, occurrence);
    nodes.push(SourceDataNode {
        id: id.clone(),
        kind,
        path,
        key,
        value_kind,
        value_preview,
        range: span.map(|range| source_range(source, range.start, range.end)),
        key_range,
        parent_id: Some(parent_id.to_string()),
        children: Vec::new(),
    });
    id
}

fn value_kind(value: &Value) -> SourceDataValueKind {
    match value {
        Value::String(_) => SourceDataValueKind::String,
        Value::Integer(_) => SourceDataValueKind::Integer,
        Value::Float(_) => SourceDataValueKind::Float,
        Value::Boolean(_) => SourceDataValueKind::Boolean,
        Value::Datetime(_) => SourceDataValueKind::Datetime,
        Value::Array(_) => SourceDataValueKind::Array,
        Value::InlineTable(_) => SourceDataValueKind::InlineTable,
    }
}

fn value_preview(value: &Value) -> String {
    let preview = match value {
        Value::String(value) => value.value().clone(),
        Value::Integer(value) => value.value().to_string(),
        Value::Float(value) => value.value().to_string(),
        Value::Boolean(value) => value.value().to_string(),
        Value::Datetime(value) => value.value().to_string(),
        Value::Array(value) => format!("{} elemente", value.len()),
        Value::InlineTable(value) => format!("{} câmpuri", value.len()),
    };
    let mut chars = preview.chars();
    let bounded = chars.by_ref().take(160).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

fn project_comments(source: &str, file: &str, root_id: &str, nodes: &mut Vec<SourceDataNode>) {
    for (index, range) in toml_comment_ranges(source).into_iter().enumerate() {
        nodes.push(SourceDataNode {
            id: data_node_id(
                file,
                &[SourceDataPathSegment::Index(index)],
                &SourceDataNodeKind::Comment,
                0,
            ),
            kind: SourceDataNodeKind::Comment,
            path: Vec::new(),
            key: None,
            value_kind: None,
            value_preview: source
                .get(range.clone())
                .map(|comment| comment.trim_start_matches('#').trim().to_string()),
            range: Some(source_range(source, range.start, range.end)),
            key_range: None,
            parent_id: Some(root_id.to_string()),
            children: Vec::new(),
        });
    }
}

fn toml_comment_ranges(source: &str) -> Vec<Range<usize>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum StringMode {
        Basic,
        Literal,
        MultilineBasic,
        MultilineLiteral,
    }

    let bytes = source.as_bytes();
    let mut ranges = Vec::new();
    let mut cursor = 0usize;
    let mut mode = None;
    let mut escaped = false;
    while cursor < bytes.len() {
        match mode {
            Some(StringMode::Basic) => {
                if escaped {
                    escaped = false;
                } else if bytes[cursor] == b'\\' {
                    escaped = true;
                } else if bytes[cursor] == b'"' {
                    mode = None;
                }
                cursor += 1;
            }
            Some(StringMode::Literal) => {
                if bytes[cursor] == b'\'' {
                    mode = None;
                }
                cursor += 1;
            }
            Some(StringMode::MultilineBasic) => {
                if escaped {
                    escaped = false;
                    cursor += 1;
                } else if bytes[cursor] == b'\\' {
                    escaped = true;
                    cursor += 1;
                } else if source[cursor..].starts_with("\"\"\"") {
                    mode = None;
                    cursor += 3;
                } else {
                    cursor += 1;
                }
            }
            Some(StringMode::MultilineLiteral) => {
                if source[cursor..].starts_with("'''") {
                    mode = None;
                    cursor += 3;
                } else {
                    cursor += 1;
                }
            }
            None => {
                if source[cursor..].starts_with("\"\"\"") {
                    mode = Some(StringMode::MultilineBasic);
                    cursor += 3;
                } else if source[cursor..].starts_with("'''") {
                    mode = Some(StringMode::MultilineLiteral);
                    cursor += 3;
                } else if bytes[cursor] == b'"' {
                    mode = Some(StringMode::Basic);
                    cursor += 1;
                } else if bytes[cursor] == b'\'' {
                    mode = Some(StringMode::Literal);
                    cursor += 1;
                } else if bytes[cursor] == b'#' {
                    let start = cursor;
                    while cursor < bytes.len() && bytes[cursor] != b'\n' {
                        cursor += 1;
                    }
                    ranges.push(start..cursor);
                } else {
                    cursor += 1;
                }
            }
        }
    }
    ranges
}

fn rebuild_children(nodes: &mut [SourceDataNode]) {
    let links = nodes
        .iter()
        .filter_map(|node| {
            node.parent_id
                .as_ref()
                .map(|parent| (parent.clone(), node.id.clone()))
        })
        .collect::<Vec<_>>();
    for (parent_id, child_id) in links {
        if let Some(parent) = nodes.iter_mut().find(|node| node.id == parent_id) {
            parent.children.push(child_id);
        }
    }
}

fn data_node_id(
    file: &str,
    path: &[SourceDataPathSegment],
    kind: &SourceDataNodeKind,
    occurrence: usize,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "pana-data-node-v1".hash(&mut hasher);
    file.hash(&mut hasher);
    kind.hash(&mut hasher);
    path.hash(&mut hasher);
    occurrence.hash(&mut hasher);
    format!("data_{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_projection_is_lossless_and_indexes_all_semantic_shapes_and_comments() {
        let source = r#"# Meniu principal
titlu = "Pană # Studio"
activ = true
culori = ["verde", "alb"] # paletă
meta = { limba = "ro", versiune = 2 }

[[elemente]]
eticheta = "Acasă"
url = "/"

[[elemente]]
eticheta = "Contact"
url = "/contact/"
"#;
        let document = parse_lossless_toml(source, "date/meniu.toml").unwrap();

        assert!(document.is_lossless());
        assert_eq!(document.reconstruct(), source);
        assert!(document
            .nodes
            .iter()
            .any(|node| node.kind == SourceDataNodeKind::ArrayOfTables));
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| node.kind == SourceDataNodeKind::TableElement)
                .count(),
            2
        );
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| node.kind == SourceDataNodeKind::Comment)
                .count(),
            2
        );
        assert!(!document
            .nodes
            .iter()
            .any(|node| node.value_preview.as_deref() == Some("Studio")));
    }

    #[test]
    fn hash_characters_inside_every_toml_string_form_are_not_comments() {
        let source = "a = \"#\"\nb = '#'\nc = \"\"\"#\"\"\"\nd = '''#'''\n# real\n";
        let document = parse_lossless_toml(source, "date/test.toml").unwrap();
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| node.kind == SourceDataNodeKind::Comment)
                .count(),
            1
        );
    }

    #[test]
    fn zola_adapters_project_json_yaml_csv_bibtex_and_xml_semantics() {
        let fixtures = [
            (
                SourceDataFormat::Json,
                "date/catalog.json",
                r#"{"items":[{"title":"Unu"}]}"#,
                "title",
            ),
            (
                SourceDataFormat::Yaml,
                "date/catalog.yaml",
                "items:\n  - title: Unu\n",
                "title",
            ),
            (
                SourceDataFormat::Csv,
                "date/catalog.csv",
                "title,url\nUnu,/unu/\n",
                "records",
            ),
            (
                SourceDataFormat::Bibtex,
                "date/catalog.bib",
                "@article{unu, title={Titlu}}\n",
                "bibliographies",
            ),
            (
                SourceDataFormat::Xml,
                "date/catalog.xml",
                "<items><title>Unu</title></items>",
                "title",
            ),
        ];

        for (format, file, source, expected_key) in fixtures {
            let nodes = parse_zola_data_adapter(source, file, &format).unwrap();
            assert_eq!(nodes[0].kind, SourceDataNodeKind::Document);
            assert!(
                nodes
                    .iter()
                    .any(|node| node.key.as_deref() == Some(expected_key)),
                "{format:?} nu proiectează cheia {expected_key}"
            );
        }
    }
}
