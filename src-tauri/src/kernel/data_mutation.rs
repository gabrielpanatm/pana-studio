use std::path::Path;

use serde::{Deserialize, Serialize};
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Key, Table, Value};

use crate::{
    kernel::{
        component_mutation::validate_semantic_workspace_candidate,
        project_path::normalize_project_relative_path,
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
            WorkspaceMutationMetadata, WorkspaceResourceMutation,
        },
    },
    source_graph::{
        build_source_graph_from_workspace_projection,
        model::{
            SourceDataFormat, SourceDataNode, SourceDataNodeKind, SourceDataPathSegment,
            SourceDataValueKind,
        },
    },
};

pub const DATA_MUTATION_SCHEMA_VERSION: u32 = 1;
pub const DATA_NODE_EDITOR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DataMutationOperation {
    CreateFile,
    UpdateNode,
    InsertChild,
    DeleteNode,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DataDraftKind {
    String,
    Integer,
    Float,
    Boolean,
    Datetime,
    Array,
    InlineTable,
    Table,
    ArrayOfTables,
}

impl DataDraftKind {
    fn is_scalar(self) -> bool {
        matches!(
            self,
            Self::String | Self::Integer | Self::Float | Self::Boolean | Self::Datetime
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataMutationInput {
    pub operation: DataMutationOperation,
    pub file: String,
    pub node_id: Option<String>,
    pub key: Option<String>,
    pub draft_kind: Option<DataDraftKind>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataMutationPlan {
    pub schema_version: u32,
    pub operation: DataMutationOperation,
    pub file: String,
    pub node_id: Option<String>,
    pub touched_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DataNodeEditorSnapshot {
    pub schema_version: u32,
    pub file: String,
    pub node_id: String,
    pub key: Option<String>,
    pub draft_kind: Option<DataDraftKind>,
    pub value: Option<String>,
    pub editable_key: bool,
    pub editable_value: bool,
}

pub fn read_data_node_editor_snapshot(
    project_root: &Path,
    workspace: &ProjectWorkspace,
    file: &str,
    node_id: &str,
) -> Result<DataNodeEditorSnapshot, String> {
    let file = normalize_data_file_path(file)?;
    let projection = workspace.capture_projection_lease()?;
    let graph = build_source_graph_from_workspace_projection(project_root, &projection)?;
    let data_file = graph
        .data_files
        .iter()
        .find(|candidate| candidate.file == file)
        .ok_or_else(|| format!("SourceGraph nu mai conține fișierul de date {file}."))?;
    if data_file.format != SourceDataFormat::Toml {
        return Err("Editorul tipizat este disponibil numai pentru TOML.".to_string());
    }
    let node = data_file
        .nodes
        .iter()
        .find(|candidate| candidate.id == node_id)
        .ok_or_else(|| format!("Nodul TOML {node_id} nu mai există în revizia curentă."))?;
    let source = workspace
        .documents
        .text_for(&file)
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește sursa text pentru {file}."))?;
    let mut document = source
        .parse::<DocumentMut>()
        .map_err(|error| format!("Document TOML invalid: {error}"))?;
    let value = if is_scalar_node(node) {
        let item = item_at_path_mut(document.as_table_mut(), &node.path)?;
        item.as_value().map(exact_scalar_value).transpose()?
    } else {
        None
    };
    Ok(DataNodeEditorSnapshot {
        schema_version: DATA_NODE_EDITOR_SCHEMA_VERSION,
        file,
        node_id: node.id.clone(),
        key: node.key.clone(),
        draft_kind: node.value_kind.as_ref().and_then(draft_kind_for_value_kind),
        value,
        editable_key: node.key_range.is_some()
            && matches!(node.path.last(), Some(SourceDataPathSegment::Key(_))),
        editable_value: is_scalar_node(node),
    })
}

pub fn stage_validated_data_mutation(
    project_root: &Path,
    workspace: &mut ProjectWorkspace,
    input: DataMutationInput,
    now_ms: u128,
) -> Result<(DataMutationPlan, ProjectWorkspaceMutationReceipt), String> {
    let file = normalize_data_file_path(&input.file)?;
    let (contents, create_only) = match input.operation {
        DataMutationOperation::CreateFile => {
            if workspace.documents.files.contains_key(&file) {
                return Err(format!(
                    "Fișierul de date {file} există deja în sesiunea proiectului."
                ));
            }
            let source = input.value.as_deref().unwrap_or_default();
            source
                .parse::<DocumentMut>()
                .map_err(|error| format!("Conținutul inițial TOML este invalid: {error}"))?;
            (source.to_string(), true)
        }
        _ => {
            let projection = workspace.capture_projection_lease()?;
            let graph = build_source_graph_from_workspace_projection(project_root, &projection)?;
            let data_file = graph
                .data_files
                .iter()
                .find(|candidate| candidate.file == file)
                .ok_or_else(|| {
                    format!(
                        "SourceGraph nu mai conține fișierul de date {file}. Reîncarcă proiectul."
                    )
                })?;
            if data_file.format != SourceDataFormat::Toml {
                return Err(format!(
                    "Editarea vizuală sigură este disponibilă numai pentru TOML; {file} este {:?}.",
                    data_file.format
                ));
            }
            if let Some(error) = data_file.parse_error.as_deref() {
                return Err(format!(
                    "Fișierul {file} trebuie corectat în editorul de cod înaintea editării vizuale: {error}"
                ));
            }
            let node_id = input
                .node_id
                .as_deref()
                .ok_or_else(|| "Mutația vizuală cere un nod semantic TOML.".to_string())?;
            let node = data_file
                .nodes
                .iter()
                .find(|candidate| candidate.id == node_id)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "Nodul TOML {node_id} nu mai există în revizia curentă. Selectează-l din nou."
                    )
                })?;
            let source = workspace.documents.text_for(&file).ok_or_else(|| {
                format!("ProjectWorkspace nu urmărește sursa text pentru {file}.")
            })?;
            (mutate_toml_source(&source, &node, &input)?, false)
        }
    };

    let plan = DataMutationPlan {
        schema_version: DATA_MUTATION_SCHEMA_VERSION,
        operation: input.operation,
        file: file.clone(),
        node_id: input.node_id.clone(),
        touched_files: vec![file.clone()],
    };
    let mutation = WorkspaceResourceMutation {
        relative_path: file,
        contents,
        create_only,
    };
    let metadata = WorkspaceMutationMetadata {
        label: mutation_label(input.operation).to_string(),
        source: "data.visual_mutation".to_string(),
        coalesce_key: None,
        transaction_id: None,
    };

    let mut candidate = workspace.clone();
    let candidate_identity = current_identity(&candidate);
    candidate.stage_resource_texts(
        &candidate_identity,
        metadata.clone(),
        vec![mutation.clone()],
        now_ms,
    )?;
    validate_semantic_workspace_candidate(project_root, &candidate, "Mutația datelor TOML")?;

    let identity = current_identity(workspace);
    let receipt = workspace.stage_resource_texts(&identity, metadata, vec![mutation], now_ms)?;
    Ok((plan, receipt))
}

fn mutation_label(operation: DataMutationOperation) -> &'static str {
    match operation {
        DataMutationOperation::CreateFile => "Creare fișier de date",
        DataMutationOperation::UpdateNode => "Editare date TOML",
        DataMutationOperation::InsertChild => "Adăugare date TOML",
        DataMutationOperation::DeleteNode => "Ștergere date TOML",
    }
}

fn current_identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

fn normalize_data_file_path(path: &str) -> Result<String, String> {
    let normalized = normalize_project_relative_path(path.trim())?;
    if normalized.starts_with("date/")
        && normalized.ends_with(".toml")
        && normalized.len() > "date/.toml".len()
    {
        Ok(normalized)
    } else {
        Err("Fișierele vizuale TOML trebuie să fie în date/ și să aibă extensia .toml.".to_string())
    }
}

fn mutate_toml_source(
    source: &str,
    node: &SourceDataNode,
    input: &DataMutationInput,
) -> Result<String, String> {
    let rewritten = match input.operation {
        DataMutationOperation::UpdateNode => update_node_source(source, node, input)?,
        DataMutationOperation::InsertChild => insert_child_source(source, node, input)?,
        DataMutationOperation::DeleteNode => delete_node_source(source, node)?,
        DataMutationOperation::CreateFile => {
            return Err("Crearea fișierului nu mută un nod existent.".to_string())
        }
    };
    rewritten
        .parse::<DocumentMut>()
        .map_err(|error| format!("Mutația vizuală ar produce TOML invalid: {error}"))?;
    Ok(rewritten)
}

fn update_node_source(
    source: &str,
    node: &SourceDataNode,
    input: &DataMutationInput,
) -> Result<String, String> {
    if matches!(
        node.kind,
        SourceDataNodeKind::Document | SourceDataNodeKind::Comment | SourceDataNodeKind::Opaque
    ) {
        return Err("Acest tip de nod TOML nu poate fi actualizat vizual.".to_string());
    }
    let mut replacements = Vec::<(usize, usize, String)>::new();
    if let Some(next_key) = input.key.as_deref() {
        let current_key = node
            .key
            .as_deref()
            .ok_or_else(|| "Nodul selectat nu are o cheie TOML editabilă.".to_string())?;
        if next_key != current_key {
            require_valid_key(next_key)?;
            let range = node.key_range.as_ref().ok_or_else(|| {
                "Cheia nu are un interval sursă exact; editarea aproximativă este refuzată."
                    .to_string()
            })?;
            replacements.push((range.start, range.end, Key::new(next_key).to_string()));
        }
    }
    if let Some(kind) = input.draft_kind {
        if !kind.is_scalar() || !is_scalar_node(node) {
            return Err("Numai valorile TOML scalare pot fi înlocuite direct.".to_string());
        }
        let next_source_value = input.value.as_deref().unwrap_or_default();
        let mut document = source
            .parse::<DocumentMut>()
            .map_err(|error| format!("Document TOML invalid: {error}"))?;
        let current = item_at_path_mut(document.as_table_mut(), &node.path)?
            .as_value()
            .ok_or_else(|| "Nodul selectat nu mai este o valoare TOML.".to_string())?;
        let current_kind = node.value_kind.as_ref().and_then(draft_kind_for_value_kind);
        let current_value = exact_scalar_value(current)?;
        if current_kind != Some(kind) || current_value != next_source_value {
            let range = node.range.as_ref().ok_or_else(|| {
                "Valoarea nu are un interval sursă exact; editarea aproximativă este refuzată."
                    .to_string()
            })?;
            let value = scalar_value(kind, next_source_value)?;
            replacements.push((range.start, range.end, value.to_string()));
        }
    }
    if replacements.is_empty() {
        return Ok(source.to_string());
    }
    apply_exact_replacements(source, replacements)
}

fn is_scalar_node(node: &SourceDataNode) -> bool {
    matches!(
        node.value_kind,
        Some(
            SourceDataValueKind::String
                | SourceDataValueKind::Integer
                | SourceDataValueKind::Float
                | SourceDataValueKind::Boolean
                | SourceDataValueKind::Datetime
        )
    )
}

fn draft_kind_for_value_kind(kind: &SourceDataValueKind) -> Option<DataDraftKind> {
    match kind {
        SourceDataValueKind::String => Some(DataDraftKind::String),
        SourceDataValueKind::Integer => Some(DataDraftKind::Integer),
        SourceDataValueKind::Float => Some(DataDraftKind::Float),
        SourceDataValueKind::Boolean => Some(DataDraftKind::Boolean),
        SourceDataValueKind::Datetime => Some(DataDraftKind::Datetime),
        SourceDataValueKind::Array => Some(DataDraftKind::Array),
        SourceDataValueKind::InlineTable => Some(DataDraftKind::InlineTable),
        SourceDataValueKind::Table => Some(DataDraftKind::Table),
        SourceDataValueKind::ArrayOfTables => Some(DataDraftKind::ArrayOfTables),
        SourceDataValueKind::Null | SourceDataValueKind::Unknown => None,
    }
}

fn exact_scalar_value(value: &Value) -> Result<String, String> {
    match value {
        Value::String(value) => Ok(value.value().clone()),
        Value::Integer(value) => Ok(value.value().to_string()),
        Value::Float(value) => Ok(value.value().to_string()),
        Value::Boolean(value) => Ok(value.value().to_string()),
        Value::Datetime(value) => Ok(value.value().to_string()),
        Value::Array(_) | Value::InlineTable(_) => {
            Err("Nodul selectat nu este o valoare scalară.".to_string())
        }
    }
}

fn insert_child_source(
    source: &str,
    node: &SourceDataNode,
    input: &DataMutationInput,
) -> Result<String, String> {
    if matches!(
        node.kind,
        SourceDataNodeKind::Comment
            | SourceDataNodeKind::Value
            | SourceDataNodeKind::ArrayElement
            | SourceDataNodeKind::Opaque
    ) {
        return Err("Selectează documentul, un tabel sau o listă pentru adăugare.".to_string());
    }
    let kind = input
        .draft_kind
        .ok_or_else(|| "Alege tipul noii valori TOML.".to_string())?;
    let mut document = source
        .parse::<DocumentMut>()
        .map_err(|error| format!("Document TOML invalid: {error}"))?;
    if node.kind == SourceDataNodeKind::Document {
        insert_into_table(
            document.as_table_mut(),
            required_key(input.key.as_deref())?,
            kind,
            input.value.as_deref().unwrap_or_default(),
        )?;
    } else {
        let target = item_at_path_mut(document.as_table_mut(), &node.path)?;
        insert_into_item(
            target,
            input.key.as_deref(),
            kind,
            input.value.as_deref().unwrap_or_default(),
        )?;
    }
    Ok(document.to_string())
}

fn delete_node_source(source: &str, node: &SourceDataNode) -> Result<String, String> {
    if matches!(
        node.kind,
        SourceDataNodeKind::Document | SourceDataNodeKind::Comment | SourceDataNodeKind::Opaque
    ) {
        return Err("Documentul și comentariile nu se șterg ca noduri vizuale.".to_string());
    }
    let mut document = source
        .parse::<DocumentMut>()
        .map_err(|error| format!("Document TOML invalid: {error}"))?;
    remove_path(document.as_table_mut(), &node.path)?;
    Ok(document.to_string())
}

fn item_at_path_mut<'a>(
    root: &'a mut Table,
    path: &[SourceDataPathSegment],
) -> Result<&'a mut Item, String> {
    let (first, rest) = path
        .split_first()
        .ok_or_else(|| "Path-ul documentului nu desemnează un nod copil.".to_string())?;
    let SourceDataPathSegment::Key(first_key) = first else {
        return Err("Un path TOML de la rădăcină trebuie să înceapă cu o cheie.".to_string());
    };
    let mut current = root
        .get_mut(first_key)
        .ok_or_else(|| format!("Cheia TOML {first_key} nu mai există."))?;
    for segment in rest {
        current = match segment {
            SourceDataPathSegment::Key(key) => current
                .get_mut(key.as_str())
                .ok_or_else(|| format!("Cheia TOML {key} nu mai există."))?,
            SourceDataPathSegment::Index(index) => current
                .get_mut(*index)
                .ok_or_else(|| format!("Indexul TOML {index} nu mai există."))?,
        };
    }
    Ok(current)
}

fn remove_path(root: &mut Table, path: &[SourceDataPathSegment]) -> Result<(), String> {
    let (last, parent_path) = path
        .split_last()
        .ok_or_else(|| "Rădăcina documentului nu poate fi ștearsă.".to_string())?;
    if parent_path.is_empty() {
        let SourceDataPathSegment::Key(key) = last else {
            return Err("Un copil al documentului trebuie să aibă o cheie.".to_string());
        };
        return root
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| format!("Cheia TOML {key} nu mai există."));
    }
    let parent = item_at_path_mut(root, parent_path)?;
    match (parent, last) {
        (Item::Table(table), SourceDataPathSegment::Key(key)) => table
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| format!("Cheia TOML {key} nu mai există.")),
        (Item::Value(Value::InlineTable(table)), SourceDataPathSegment::Key(key)) => table
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| format!("Cheia TOML {key} nu mai există.")),
        (Item::ArrayOfTables(array), SourceDataPathSegment::Index(index)) => {
            if *index >= array.len() {
                Err(format!("Indexul TOML {index} nu mai există."))
            } else {
                array.remove(*index);
                Ok(())
            }
        }
        (Item::Value(Value::Array(array)), SourceDataPathSegment::Index(index)) => {
            if *index >= array.len() {
                Err(format!("Indexul TOML {index} nu mai există."))
            } else {
                array.remove(*index);
                Ok(())
            }
        }
        _ => Err("Părintele nodului nu acceptă ștergerea semantică cerută.".to_string()),
    }
}

fn insert_into_item(
    target: &mut Item,
    key: Option<&str>,
    kind: DataDraftKind,
    raw_value: &str,
) -> Result<(), String> {
    match target {
        Item::Table(table) => insert_into_table(table, required_key(key)?, kind, raw_value),
        Item::ArrayOfTables(array) => {
            if kind != DataDraftKind::Table {
                return Err("O listă de tabele acceptă numai un rând nou de tip tabel.".to_string());
            }
            array.push(Table::new());
            Ok(())
        }
        Item::Value(Value::InlineTable(table)) => {
            let key = required_key(key)?;
            if table.contains_key(key) {
                return Err(format!("Cheia TOML {key} există deja în tabel."));
            }
            table.insert(key, value_for_container(kind, raw_value)?);
            Ok(())
        }
        Item::Value(Value::Array(array)) => {
            if key.is_some_and(|value| !value.trim().is_empty()) {
                return Err("Elementele unei liste TOML nu au cheie.".to_string());
            }
            array.push_formatted(value_for_container(kind, raw_value)?);
            Ok(())
        }
        _ => Err("Nodul selectat nu este un container TOML editabil.".to_string()),
    }
}

fn insert_into_table(
    table: &mut Table,
    key: &str,
    kind: DataDraftKind,
    raw_value: &str,
) -> Result<(), String> {
    require_valid_key(key)?;
    if table.contains_key(key) {
        return Err(format!("Cheia TOML {key} există deja în tabel."));
    }
    let item = match kind {
        DataDraftKind::Table => Item::Table(Table::new()),
        DataDraftKind::ArrayOfTables => {
            let mut array = ArrayOfTables::new();
            array.push(Table::new());
            Item::ArrayOfTables(array)
        }
        _ => Item::Value(value_for_container(kind, raw_value)?),
    };
    table.insert(key, item);
    Ok(())
}

fn value_for_container(kind: DataDraftKind, raw_value: &str) -> Result<Value, String> {
    match kind {
        DataDraftKind::String
        | DataDraftKind::Integer
        | DataDraftKind::Float
        | DataDraftKind::Boolean
        | DataDraftKind::Datetime => scalar_value(kind, raw_value),
        DataDraftKind::Array => Ok(Value::Array(Array::new())),
        DataDraftKind::InlineTable | DataDraftKind::Table => {
            Ok(Value::InlineTable(InlineTable::new()))
        }
        DataDraftKind::ArrayOfTables => {
            Err("O listă de tabele nu poate fi inclusă într-o valoare inline.".to_string())
        }
    }
}

fn scalar_value(kind: DataDraftKind, raw_value: &str) -> Result<Value, String> {
    match kind {
        DataDraftKind::String => Ok(Value::from(raw_value)),
        DataDraftKind::Integer => raw_value
            .trim()
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| "Valoarea nu este un număr întreg TOML valid.".to_string()),
        DataDraftKind::Float => raw_value
            .trim()
            .parse::<f64>()
            .map(Value::from)
            .map_err(|_| "Valoarea nu este un număr zecimal TOML valid.".to_string()),
        DataDraftKind::Boolean => raw_value
            .trim()
            .parse::<bool>()
            .map(Value::from)
            .map_err(|_| "Valoarea booleană trebuie să fie true sau false.".to_string()),
        DataDraftKind::Datetime => {
            let value = raw_value
                .trim()
                .parse::<Value>()
                .map_err(|error| format!("Data sau ora TOML este invalidă: {error}"))?;
            if matches!(value, Value::Datetime(_)) {
                Ok(value)
            } else {
                Err("Valoarea introdusă nu este o dată sau oră TOML.".to_string())
            }
        }
        _ => Err("Tipul selectat nu este o valoare scalară TOML.".to_string()),
    }
}

fn required_key(key: Option<&str>) -> Result<&str, String> {
    let key = key.unwrap_or_default();
    require_valid_key(key)?;
    Ok(key)
}

fn require_valid_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Cheia TOML nu poate fi goală.".to_string());
    }
    if key.chars().any(char::is_control) {
        return Err("Cheia TOML nu poate conține caractere de control.".to_string());
    }
    Ok(())
}

fn apply_exact_replacements(
    source: &str,
    mut replacements: Vec<(usize, usize, String)>,
) -> Result<String, String> {
    replacements.sort_by(|left, right| right.0.cmp(&left.0));
    let mut previous_start = source.len();
    let mut rewritten = source.to_string();
    for (start, end, replacement) in replacements {
        if start > end || end > source.len() || end > previous_start {
            return Err(
                "Intervalele sursă ale mutației TOML sunt invalide sau se suprapun.".to_string(),
            );
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err("Intervalul sursă TOML nu respectă limitele UTF-8.".to_string());
        }
        rewritten.replace_range(start..end, &replacement);
        previous_start = start;
    }
    Ok(rewritten)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs, path::PathBuf};

    use super::*;
    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            observability::now_ms,
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::WorkspaceHistoryDirection,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
        source_graph::structured_data::parse_lossless_toml,
    };

    fn node(source: &str, predicate: impl Fn(&SourceDataNode) -> bool) -> SourceDataNode {
        parse_lossless_toml(source, "date/test.toml")
            .unwrap()
            .nodes
            .into_iter()
            .find(predicate)
            .unwrap()
    }

    fn input(
        operation: DataMutationOperation,
        node_id: &str,
        key: Option<&str>,
        draft_kind: Option<DataDraftKind>,
        value: Option<&str>,
    ) -> DataMutationInput {
        DataMutationInput {
            operation,
            file: "date/test.toml".to_string(),
            node_id: Some(node_id.to_string()),
            key: key.map(str::to_string),
            draft_kind,
            value: value.map(str::to_string),
        }
    }

    #[test]
    fn updates_key_and_scalar_without_touching_comments_or_neighbors() {
        let source = "# antet\ntitlu = \"Vechi\" # păstrat\nactiv = true\n";
        let selected = node(source, |node| node.key.as_deref() == Some("titlu"));
        let rewritten = mutate_toml_source(
            source,
            &selected,
            &input(
                DataMutationOperation::UpdateNode,
                &selected.id,
                Some("nume site"),
                Some(DataDraftKind::String),
                Some("Pană Studio"),
            ),
        )
        .unwrap();

        assert_eq!(
            rewritten,
            "# antet\n\"nume site\" = \"Pană Studio\" # păstrat\nactiv = true\n"
        );
    }

    #[test]
    fn inserts_and_removes_a_table_row_losslessly() {
        let source = "# catalog\n[[servicii]]\ntitlu = \"Audit\"\n";
        let collection = node(source, |node| {
            node.kind == SourceDataNodeKind::ArrayOfTables
                && node.key.as_deref() == Some("servicii")
        });
        let inserted = mutate_toml_source(
            source,
            &collection,
            &input(
                DataMutationOperation::InsertChild,
                &collection.id,
                None,
                Some(DataDraftKind::Table),
                None,
            ),
        )
        .unwrap();
        assert!(inserted.starts_with("# catalog\n[[servicii]]\ntitlu = \"Audit\"\n"));
        assert_eq!(inserted.matches("[[servicii]]").count(), 2);

        let second = node(&inserted, |node| {
            node.kind == SourceDataNodeKind::TableElement
                && node.path.last() == Some(&SourceDataPathSegment::Index(1))
        });
        let removed = mutate_toml_source(
            &inserted,
            &second,
            &input(
                DataMutationOperation::DeleteNode,
                &second.id,
                None,
                None,
                None,
            ),
        )
        .unwrap();
        assert_eq!(removed, source);
    }

    #[test]
    fn inserts_nested_visual_shapes_with_valid_toml() {
        let source = "";
        let root = node(source, |node| node.kind == SourceDataNodeKind::Document);
        let with_table = mutate_toml_source(
            source,
            &root,
            &input(
                DataMutationOperation::InsertChild,
                &root.id,
                Some("contact"),
                Some(DataDraftKind::Table),
                None,
            ),
        )
        .unwrap();
        let contact = node(&with_table, |node| node.key.as_deref() == Some("contact"));
        let completed = mutate_toml_source(
            &with_table,
            &contact,
            &input(
                DataMutationOperation::InsertChild,
                &contact.id,
                Some("email"),
                Some(DataDraftKind::String),
                Some("salut@example.test"),
            ),
        )
        .unwrap();

        assert_eq!(completed, "[contact]\nemail = \"salut@example.test\"\n");
        completed.parse::<DocumentMut>().unwrap();
    }

    #[test]
    fn renames_a_table_header_through_its_exact_key_span() {
        let source = "[contact]\nemail = \"salut@example.test\"\n";
        let table = node(source, |node| {
            node.kind == SourceDataNodeKind::Table && node.key.as_deref() == Some("contact")
        });
        let rewritten = mutate_toml_source(
            source,
            &table,
            &input(
                DataMutationOperation::UpdateNode,
                &table.id,
                Some("date contact"),
                None,
                None,
            ),
        )
        .unwrap();

        assert_eq!(
            rewritten,
            "[\"date contact\"]\nemail = \"salut@example.test\"\n"
        );
    }

    #[test]
    fn unchanged_semantic_value_preserves_its_original_toml_representation() {
        let source = "titlu = 'Vechi'\n";
        let selected = node(source, |node| node.key.as_deref() == Some("titlu"));
        let rewritten = mutate_toml_source(
            source,
            &selected,
            &input(
                DataMutationOperation::UpdateNode,
                &selected.id,
                Some("titlu"),
                Some(DataDraftKind::String),
                Some("Vechi"),
            ),
        )
        .unwrap();

        assert_eq!(rewritten, source);
    }

    #[test]
    fn validated_visual_edit_is_one_workspace_history_entry() {
        let root = test_root("workspace-history");
        let original = "titlu = \"Vechi\"\nactiv = true\n";
        let mut workspace = test_workspace(
            &root,
            HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = \"https://example.test\"\n".to_string(),
                ),
                ("date/site.toml".to_string(), original.to_string()),
            ]),
        );
        let graph = build_source_graph_from_workspace_projection(
            &root,
            &workspace.capture_projection_lease().unwrap(),
        )
        .unwrap();
        let selected = graph
            .data_files
            .iter()
            .find(|file| file.file == "date/site.toml")
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.key.as_deref() == Some("titlu"))
            .unwrap();

        let (_plan, receipt) = stage_validated_data_mutation(
            &root,
            &mut workspace,
            DataMutationInput {
                operation: DataMutationOperation::UpdateNode,
                file: "date/site.toml".to_string(),
                node_id: Some(selected.id.clone()),
                key: Some("titlu".to_string()),
                draft_kind: Some(DataDraftKind::String),
                value: Some("Pană Studio".to_string()),
            },
            2,
        )
        .unwrap();

        assert_eq!(receipt.history.undo_count, 1);
        assert_eq!(
            workspace.documents.text_for("date/site.toml").as_deref(),
            Some("titlu = \"Pană Studio\"\nactiv = true\n")
        );
        let undo = workspace.undo(&current_identity(&workspace), 3).unwrap();
        assert!(matches!(undo.direction, WorkspaceHistoryDirection::Undo));
        assert_eq!(
            workspace.documents.text_for("date/site.toml").as_deref(),
            Some(original)
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pana-data-mutation-{label}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("date")).unwrap();
        root
    }

    fn test_workspace(root: &Path, sources: HashMap<String, String>) -> ProjectWorkspace {
        for (path, source) in &sources {
            let absolute = root.join(path);
            if let Some(parent) = absolute.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(absolute, source).unwrap();
        }
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "data-mutation-test".to_string(),
            project_root: canonical.clone(),
            zola_root: canonical.clone(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: canonical.clone(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: sources.len(),
                directory_count: 3,
            },
        };
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 128,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 8 * 1024 * 1024,
            },
        );
        let mut sorted_sources = sources.into_iter().collect::<Vec<_>>();
        sorted_sources.sort_by(|left, right| left.0.cmp(&right.0));
        for (relative_path, source) in sorted_sources {
            let role = if relative_path == "zola.toml" {
                TextBufferRole::Config
            } else {
                TextBufferRole::Data
            };
            documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.clone(),
                absolute_path: root.join(&relative_path).to_string_lossy().to_string(),
                language: TextBufferLanguage::Toml,
                role,
                baseline: FileBufferBaseline {
                    hash: hash_text(&source),
                    modified_ms: 1,
                    size: source.len() as u64,
                    readonly: false,
                },
                baseline_text: source,
                draft: None,
                revision: 1,
            });
        }
        let manifest = read_project_disk_manifest(root).unwrap();
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            manifest,
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        ProjectWorkspace::new(session, accepted, documents, page_js).unwrap()
    }
}
