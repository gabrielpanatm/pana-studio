#[cfg(test)]
use std::fs;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toml_edit::{DocumentMut, Item, Table};

use crate::{
    kernel::project_workspace::{
        ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
        WorkspaceMutationMetadata, WorkspaceResourceDelete, WorkspaceResourceMutation,
    },
    source_graph::{zola::zola_frontmatter_range, SourceGraph},
};

pub const CONTENT_MODEL_SCHEMA_VERSION: u32 = 1;
pub const CONTENT_MODEL_PROJECT_PATH: &str = ".panastudio/project.toml";
pub const CONTENT_MODEL_ASSIGNMENTS_PATH: &str = ".panastudio/assignments.toml";
const CONTENT_MODEL_DIRECTORY: &str = ".panastudio/content-models";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFieldKind {
    Text,
    Textarea,
    Markdown,
    Number,
    Boolean,
    Date,
    Select,
    Url,
    Color,
    Image,
    Group,
    Repeater,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentFieldChoice {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentFieldDefinition {
    #[serde(default)]
    pub id: String,
    pub key: String,
    pub label: String,
    pub kind: ContentFieldKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub help: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
    #[serde(default)]
    pub choices: Vec<ContentFieldChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default)]
    pub fields: Vec<ContentFieldDefinition>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelDefinition {
    pub schema_version: u32,
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub fields: Vec<ContentFieldDefinition>,
    #[serde(skip, default)]
    pub file: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelAssignment {
    pub section_path: String,
    pub model_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomFieldTemplateUsage {
    pub model_id: String,
    pub field_id: String,
    pub field_key: String,
    pub template_file: String,
    pub expression: String,
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelPageBinding {
    pub page_file: String,
    pub section_path: String,
    pub model_id: String,
    pub values: BTreeMap<String, serde_json::Value>,
    pub missing_required_fields: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub file: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelCatalog {
    pub schema_version: u32,
    pub metadata_present: bool,
    pub models: Vec<ContentModelDefinition>,
    pub assignments: Vec<ContentModelAssignment>,
    pub page_bindings: Vec<ContentModelPageBinding>,
    pub template_usages: Vec<CustomFieldTemplateUsage>,
    pub diagnostics: Vec<ContentModelDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProjectContract {
    schema_version: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct AssignmentContract {
    schema_version: u32,
    #[serde(default)]
    assignments: Vec<ContentModelAssignment>,
}

impl Default for ProjectContract {
    fn default() -> Self {
        Self {
            schema_version: CONTENT_MODEL_SCHEMA_VERSION,
        }
    }
}

pub(crate) fn build_content_model_catalog_from_workspace_projection(
    _project_root: &Path,
    source_texts: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    graph: &SourceGraph,
) -> ContentModelCatalog {
    let mut catalog = ContentModelCatalog {
        schema_version: CONTENT_MODEL_SCHEMA_VERSION,
        ..Default::default()
    };
    let project_source =
        read_project_source(source_texts, deleted_sources, CONTENT_MODEL_PROJECT_PATH);
    catalog.metadata_present = project_source.is_some();
    if let Some(source) = project_source {
        match toml_edit::de::from_str::<ProjectContract>(&source) {
            Ok(contract) if contract.schema_version == CONTENT_MODEL_SCHEMA_VERSION => {}
            Ok(contract) => catalog.diagnostics.push(diagnostic(
                "error",
                "content_model_schema_mismatch",
                format!(
                    "Metadatele Pană Studio folosesc schema {}, dar aplicația cere schema {}.",
                    contract.schema_version, CONTENT_MODEL_SCHEMA_VERSION
                ),
                Some(CONTENT_MODEL_PROJECT_PATH),
            )),
            Err(error) => catalog.diagnostics.push(diagnostic(
                "error",
                "content_model_project_invalid",
                format!("Metadatele proiectului nu sunt TOML valid: {error}"),
                Some(CONTENT_MODEL_PROJECT_PATH),
            )),
        }
    }

    let model_sources = source_texts
        .iter()
        .filter(|(path, _)| is_model_path(path))
        .map(|(path, source)| (path.clone(), source.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut seen_ids = BTreeSet::new();
    for (file, source) in model_sources {
        match toml_edit::de::from_str::<ContentModelDefinition>(&source) {
            Ok(mut model) => {
                model.file = file.clone();
                if let Err(error) = validate_model(&mut model) {
                    catalog.diagnostics.push(diagnostic(
                        "error",
                        "content_model_invalid",
                        error,
                        Some(&file),
                    ));
                    continue;
                }
                let canonical_file = model_path(&model.id);
                if file != canonical_file {
                    catalog.diagnostics.push(diagnostic(
                        "error",
                        "content_model_file_identity_mismatch",
                        format!(
                            "Modelul „{}” trebuie păstrat în {}, nu în {}.",
                            model.id, canonical_file, file
                        ),
                        Some(&file),
                    ));
                    continue;
                }
                if !seen_ids.insert(model.id.clone()) {
                    catalog.diagnostics.push(diagnostic(
                        "error",
                        "content_model_duplicate_id",
                        format!("ID-ul de model „{}” este duplicat.", model.id),
                        Some(&file),
                    ));
                    continue;
                }
                catalog.models.push(model);
            }
            Err(error) => catalog.diagnostics.push(diagnostic(
                "error",
                "content_model_file_invalid",
                format!("Schema de conținut nu este TOML valid: {error}"),
                Some(&file),
            )),
        }
    }
    catalog
        .models
        .sort_by(|left, right| left.label.cmp(&right.label));

    let assignments_source = read_project_source(
        source_texts,
        deleted_sources,
        CONTENT_MODEL_ASSIGNMENTS_PATH,
    );
    if let Some(source) = assignments_source {
        match toml_edit::de::from_str::<AssignmentContract>(&source) {
            Ok(contract) if contract.schema_version == CONTENT_MODEL_SCHEMA_VERSION => {
                catalog.assignments = normalize_assignments(
                    contract.assignments,
                    &catalog.models,
                    &mut catalog.diagnostics,
                );
            }
            Ok(contract) => catalog.diagnostics.push(diagnostic(
                "error",
                "content_model_assignments_schema_mismatch",
                format!(
                    "Atribuirile folosesc schema {} în loc de {}.",
                    contract.schema_version, CONTENT_MODEL_SCHEMA_VERSION
                ),
                Some(CONTENT_MODEL_ASSIGNMENTS_PATH),
            )),
            Err(error) => catalog.diagnostics.push(diagnostic(
                "error",
                "content_model_assignments_invalid",
                format!("Atribuirile modelelor nu sunt TOML valid: {error}"),
                Some(CONTENT_MODEL_ASSIGNMENTS_PATH),
            )),
        }
    }

    catalog.page_bindings = build_page_bindings(
        source_texts,
        graph,
        &catalog.models,
        &catalog.assignments,
        &mut catalog.diagnostics,
    );
    catalog.template_usages = build_template_usages(
        source_texts,
        graph,
        &catalog.models,
        &catalog.assignments,
        &catalog.page_bindings,
    );
    catalog
}

fn read_project_source(
    source_texts: &HashMap<String, String>,
    deleted_sources: &HashSet<String>,
    relative_path: &str,
) -> Option<String> {
    if deleted_sources.contains(relative_path) {
        return None;
    }
    source_texts.get(relative_path).cloned()
}

fn is_model_path(path: &str) -> bool {
    path.starts_with(&format!("{CONTENT_MODEL_DIRECTORY}/")) && path.ends_with(".toml")
}

fn diagnostic(
    severity: &str,
    code: &str,
    message: String,
    file: Option<&str>,
) -> ContentModelDiagnostic {
    ContentModelDiagnostic {
        severity: severity.to_string(),
        code: code.to_string(),
        message,
        file: file.map(str::to_string),
    }
}

fn normalize_assignments(
    assignments: Vec<ContentModelAssignment>,
    models: &[ContentModelDefinition],
    diagnostics: &mut Vec<ContentModelDiagnostic>,
) -> Vec<ContentModelAssignment> {
    let model_ids = models
        .iter()
        .map(|model| model.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for mut assignment in assignments {
        assignment.section_path = normalize_section_path(&assignment.section_path);
        if !model_ids.contains(assignment.model_id.as_str()) {
            diagnostics.push(diagnostic(
                "error",
                "content_model_assignment_unknown_model",
                format!(
                    "Secțiunea {} referă modelul inexistent „{}”.",
                    assignment.section_path, assignment.model_id
                ),
                Some(CONTENT_MODEL_ASSIGNMENTS_PATH),
            ));
            continue;
        }
        if !seen.insert(assignment.section_path.clone()) {
            diagnostics.push(diagnostic(
                "error",
                "content_model_assignment_duplicate_section",
                format!(
                    "Secțiunea {} are mai multe modele atașate.",
                    assignment.section_path
                ),
                Some(CONTENT_MODEL_ASSIGNMENTS_PATH),
            ));
            continue;
        }
        normalized.push(assignment);
    }
    normalized.sort_by(|left, right| left.section_path.cmp(&right.section_path));
    normalized
}

fn normalize_section_path(path: &str) -> String {
    let mut normalized = path
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();
    if !normalized.starts_with("content/") {
        normalized = format!("content/{normalized}");
    }
    if !normalized.ends_with(".md") {
        normalized = normalized.trim_end_matches('/').to_string();
        normalized.push_str("/_index.md");
    }
    normalized
}

fn validate_identifier(value: &str, label: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        });
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{label} „{value}” acceptă doar litere ASCII, cifre, _ și -."
        ))
    }
}

pub fn validate_model(model: &mut ContentModelDefinition) -> Result<(), String> {
    if model.schema_version != CONTENT_MODEL_SCHEMA_VERSION {
        return Err(format!(
            "Modelul {} folosește schema incompatibilă {}.",
            model.id, model.schema_version
        ));
    }
    validate_identifier(model.id.trim(), "ID-ul modelului")?;
    if model.label.trim().is_empty() {
        return Err("Eticheta modelului este obligatorie.".to_string());
    }
    let mut ids = BTreeSet::new();
    validate_fields(&model.id, &mut model.fields, &mut ids, "")
}

fn validate_fields(
    model_id: &str,
    fields: &mut [ContentFieldDefinition],
    ids: &mut BTreeSet<String>,
    parent_path: &str,
) -> Result<(), String> {
    let mut keys = BTreeSet::new();
    for field in fields {
        validate_identifier(field.key.trim(), "Cheia câmpului")?;
        let path = if parent_path.is_empty() {
            field.key.clone()
        } else {
            format!("{parent_path}.{}", field.key)
        };
        if field.id.trim().is_empty() {
            field.id = stable_field_id(model_id, &path);
        }
        validate_identifier(field.id.trim(), "ID-ul câmpului")?;
        if field.label.trim().is_empty() {
            return Err(format!("Câmpul {} nu are etichetă.", field.key));
        }
        if !ids.insert(field.id.clone()) {
            return Err(format!("ID-ul de câmp „{}” este duplicat.", field.id));
        }
        if !keys.insert(field.key.clone()) {
            return Err(format!("Cheia de câmp „{}” este duplicată.", field.key));
        }
        if field.kind == ContentFieldKind::Select && field.choices.is_empty() {
            return Err(format!(
                "Câmpul select „{}” cere cel puțin o opțiune.",
                field.key
            ));
        }
        if field
            .minimum
            .is_some_and(|minimum| field.maximum.is_some_and(|maximum| minimum > maximum))
        {
            return Err(format!(
                "Limita minimă depășește limita maximă pentru {}.",
                field.key
            ));
        }
        let mut choices = BTreeSet::new();
        if field.kind == ContentFieldKind::Select
            && field.choices.iter().any(|choice| {
                choice.value.trim().is_empty() || !choices.insert(choice.value.clone())
            })
        {
            return Err(format!(
                "Opțiunile câmpului {} trebuie să aibă valori unice și nenule.",
                field.key
            ));
        }
        if !matches!(
            field.kind,
            ContentFieldKind::Group | ContentFieldKind::Repeater
        ) && !field.fields.is_empty()
        {
            return Err(format!(
                "Doar câmpurile group/repeater pot conține subcâmpuri: {}.",
                field.key
            ));
        }
        validate_fields(model_id, &mut field.fields, ids, &path)?;
        if let Some(default_value) = field.default_value.as_ref() {
            validate_value_at_path(field, default_value, &path).map_err(|error| {
                format!(
                    "Valoarea implicită a câmpului {} este invalidă: {error}",
                    field.key
                )
            })?;
        }
    }
    Ok(())
}

fn stable_field_id(model_id: &str, key: &str) -> String {
    let digest = Sha256::digest(format!("{model_id}\0{key}").as_bytes());
    let suffix = digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("field_{suffix}")
}

fn build_page_bindings(
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    models: &[ContentModelDefinition],
    assignments: &[ContentModelAssignment],
    diagnostics: &mut Vec<ContentModelDiagnostic>,
) -> Vec<ContentModelPageBinding> {
    let models = models
        .iter()
        .map(|model| (model.id.as_str(), model))
        .collect::<HashMap<_, _>>();
    let mut bindings = Vec::new();
    for page in &graph.pages {
        let Some(assignment) = matching_assignment(&page.file, assignments) else {
            continue;
        };
        let Some(model) = models.get(assignment.model_id.as_str()) else {
            continue;
        };
        let source = source_texts.get(&page.file).cloned();
        let values = source
            .as_deref()
            .and_then(|source| {
                read_extra_values(source)
                    .map_err(|error| {
                        diagnostics.push(diagnostic(
                            "error",
                            "content_model_page_values_invalid",
                            error,
                            Some(&page.file),
                        ));
                    })
                    .ok()
            })
            .unwrap_or_default();
        let mut missing_required_fields = Vec::new();
        collect_missing_required_fields(&model.fields, &values, "", &mut missing_required_fields);
        let managed_keys = field_keys(&model.fields);
        for key in values.keys().filter(|key| !managed_keys.contains(*key)) {
            diagnostics.push(diagnostic(
                "warning",
                "content_model_unmanaged_extra",
                format!(
                    "Pagina {} păstrează extra.{key} în afara contractului {}.",
                    page.file, model.id
                ),
                Some(&page.file),
            ));
        }
        for field in &model.fields {
            if let Some(value) = values.get(&field.key) {
                if let Err(error) = validate_value_at_path(field, value, &field.key) {
                    diagnostics.push(diagnostic(
                        "warning",
                        "content_model_value_invalid",
                        error,
                        Some(&page.file),
                    ));
                }
            }
        }
        if !missing_required_fields.is_empty() {
            diagnostics.push(diagnostic(
                "warning",
                "content_model_required_values_missing",
                format!(
                    "Pagina {} nu completează {} câmpuri obligatorii din modelul {}.",
                    page.file,
                    missing_required_fields.len(),
                    model.id
                ),
                Some(&page.file),
            ));
        }
        bindings.push(ContentModelPageBinding {
            page_file: page.file.clone(),
            section_path: assignment.section_path.clone(),
            model_id: assignment.model_id.clone(),
            values,
            missing_required_fields,
        });
    }
    bindings.sort_by(|left, right| left.page_file.cmp(&right.page_file));
    bindings
}

fn matching_assignment<'a>(
    page_file: &str,
    assignments: &'a [ContentModelAssignment],
) -> Option<&'a ContentModelAssignment> {
    assignments
        .iter()
        .filter(|assignment| page_belongs_to_section(page_file, &assignment.section_path))
        .max_by_key(|assignment| assignment.section_path.len())
}

fn page_belongs_to_section(page_file: &str, section_path: &str) -> bool {
    let directory = section_path.trim_end_matches("_index.md");
    page_file != section_path && page_file.starts_with(directory) && page_file.ends_with(".md")
}

fn read_extra_values(source: &str) -> Result<BTreeMap<String, serde_json::Value>, String> {
    let (start, end) = zola_frontmatter_range(source)
        .ok_or_else(|| "Pagina nu are frontmatter Zola valid.".to_string())?;
    let frontmatter = &source[start..end];
    let root = if source.trim_start_matches('\u{feff}').starts_with("+++") {
        toml_edit::de::from_str::<serde_json::Value>(frontmatter)
            .map_err(|error| format!("Frontmatter TOML invalid: {error}"))?
    } else {
        serde_yaml::from_str::<serde_json::Value>(frontmatter)
            .map_err(|error| format!("Frontmatter YAML invalid: {error}"))?
    };
    Ok(root
        .get("extra")
        .and_then(serde_json::Value::as_object)
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default())
}

fn build_template_usages(
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    models: &[ContentModelDefinition],
    assignments: &[ContentModelAssignment],
    page_bindings: &[ContentModelPageBinding],
) -> Vec<CustomFieldTemplateUsage> {
    let mut usages = Vec::new();
    let dynamic_widget_catalog = ContentModelCatalog {
        models: models.to_vec(),
        ..Default::default()
    };
    let relevant_templates = models
        .iter()
        .map(|model| {
            (
                model.id.as_str(),
                template_files_for_model(graph, page_bindings, assignments, &model.id),
            )
        })
        .collect::<HashMap<_, _>>();
    for template in &graph.templates {
        let Some(source) = source_texts.get(&template.file).cloned() else {
            continue;
        };
        for model in models {
            if relevant_templates
                .get(model.id.as_str())
                .and_then(Option::as_ref)
                .is_some_and(|files| !files.contains(&template.file))
            {
                continue;
            }
            for (field, path) in flatten_field_paths(&model.fields) {
                for scope in ["page.extra", "section.extra"] {
                    for expression in tera_path_expressions(scope, &path) {
                        for absolute in expression_offsets(&source, &expression) {
                            usages.push(CustomFieldTemplateUsage {
                                model_id: model.id.clone(),
                                field_id: field.id.clone(),
                                field_key: path.clone(),
                                template_file: template.file.clone(),
                                expression: expression.clone(),
                                offset: absolute,
                            });
                        }
                    }
                }
            }
        }
        usages.extend(dynamic_marker_usages(&source, &template.file, models));
        usages.extend(
            crate::kernel::dynamic_widgets::project_dynamic_field_usages(
                &source,
                &template.file,
                &dynamic_widget_catalog,
            ),
        );
    }
    usages.sort_by(|left, right| {
        (left.template_file.as_str(), left.offset)
            .cmp(&(right.template_file.as_str(), right.offset))
    });
    usages
}

pub fn refresh_content_model_template_usages(
    _project_root: &Path,
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    catalog: &mut ContentModelCatalog,
) {
    catalog.template_usages = build_template_usages(
        source_texts,
        graph,
        &catalog.models,
        &catalog.assignments,
        &catalog.page_bindings,
    );
}

fn template_files_for_model(
    graph: &SourceGraph,
    page_bindings: &[ContentModelPageBinding],
    assignments: &[ContentModelAssignment],
    model_id: &str,
) -> Option<BTreeSet<String>> {
    let sections = assignments
        .iter()
        .filter(|assignment| assignment.model_id == model_id)
        .map(|assignment| assignment.section_path.as_str())
        .collect::<BTreeSet<_>>();
    if sections.is_empty() {
        return None;
    }

    let mut files = BTreeSet::new();
    for section in sections {
        files.extend(template_files_for_section(
            graph,
            page_bindings,
            model_id,
            section,
        )?);
    }
    Some(files)
}

fn template_files_for_section(
    graph: &SourceGraph,
    page_bindings: &[ContentModelPageBinding],
    model_id: &str,
    section_path: &str,
) -> Option<BTreeSet<String>> {
    template_files_for_binding_scope(graph, page_bindings, model_id, Some(section_path))
}

fn template_files_for_other_sections(
    graph: &SourceGraph,
    page_bindings: &[ContentModelPageBinding],
    assignments: &[ContentModelAssignment],
    model_id: &str,
    excluded_section_path: &str,
) -> Option<BTreeSet<String>> {
    let sections = assignments
        .iter()
        .filter(|assignment| {
            assignment.model_id == model_id && assignment.section_path != excluded_section_path
        })
        .map(|assignment| assignment.section_path.clone())
        .collect::<BTreeSet<_>>();
    let mut files = BTreeSet::new();
    for section in sections {
        files.extend(template_files_for_section(
            graph,
            page_bindings,
            model_id,
            &section,
        )?);
    }
    Some(files)
}

fn template_files_for_binding_scope(
    graph: &SourceGraph,
    page_bindings: &[ContentModelPageBinding],
    model_id: &str,
    section_path: Option<&str>,
) -> Option<BTreeSet<String>> {
    let bound_pages = page_bindings
        .iter()
        .filter(|binding| {
            binding.model_id == model_id
                && section_path.is_none_or(|section| binding.section_path == section)
        })
        .map(|binding| binding.page_file.as_str())
        .collect::<BTreeSet<_>>();
    let mut pending = graph
        .pages
        .iter()
        .filter(|page| bound_pages.contains(page.file.as_str()))
        .filter_map(|page| page.resolved_template.clone())
        .collect::<Vec<_>>();
    if let Some(section_path) = section_path {
        for section in graph.pages.iter().filter(|page| page.file == section_path) {
            pending.extend(section.resolved_template.iter().cloned());
            pending.extend(section.frontmatter_template.iter().cloned());
            pending.extend(section.frontmatter_page_template.iter().cloned());
        }
    }
    if pending.is_empty() {
        return None;
    }
    let mut names = BTreeSet::new();
    let mut files = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !names.insert(name.clone()) {
            continue;
        }
        for template in graph.templates.iter().filter(|template| {
            template.name == name
                || template.file == name
                || template.file.strip_prefix("templates/") == Some(name.as_str())
        }) {
            files.insert(template.file.clone());
            pending.extend(template.extends.iter().cloned());
            pending.extend(template.includes.iter().cloned());
            pending.extend(template.imports.iter().cloned());
        }
    }
    Some(files)
}

fn dynamic_marker_usages(
    source: &str,
    template_file: &str,
    models: &[ContentModelDefinition],
) -> Vec<CustomFieldTemplateUsage> {
    let mut usages = Vec::new();
    let mut cursor = 0;
    const START: &str = "{# pana:dynamic ";
    while let Some(found) = source[cursor..].find(START) {
        let offset = cursor + found;
        let body_start = offset + START.len();
        let Some(end) = source[body_start..].find("#}") else {
            break;
        };
        let expression = &source[offset..body_start + end + 2];
        let attributes = source[body_start..body_start + end]
            .split_ascii_whitespace()
            .filter_map(|part| part.split_once('='))
            .collect::<HashMap<_, _>>();
        let (Some(model_id), Some(field_id), Some(path)) = (
            attributes.get("model"),
            attributes.get("field"),
            attributes.get("path"),
        ) else {
            cursor = body_start + end + 2;
            continue;
        };
        if attributes.get("scope").copied() == Some("item") {
            if let Some(model) = models.iter().find(|model| model.id == *model_id) {
                if field_path_by_id(&model.fields, field_id)
                    .is_some_and(|canonical| canonical.join(".") == *path)
                {
                    usages.push(CustomFieldTemplateUsage {
                        model_id: (*model_id).to_string(),
                        field_id: (*field_id).to_string(),
                        field_key: (*path).to_string(),
                        template_file: template_file.to_string(),
                        expression: expression.to_string(),
                        offset,
                    });
                }
            }
        }
        cursor = body_start + end + 2;
    }
    usages
}

fn flatten_field_paths(
    fields: &[ContentFieldDefinition],
) -> Vec<(&ContentFieldDefinition, String)> {
    fn visit<'a>(
        fields: &'a [ContentFieldDefinition],
        parent: &str,
        flattened: &mut Vec<(&'a ContentFieldDefinition, String)>,
    ) {
        for field in fields {
            let path = if parent.is_empty() {
                field.key.clone()
            } else {
                format!("{parent}.{}", field.key)
            };
            flattened.push((field, path.clone()));
            visit(&field.fields, &path, flattened);
        }
    }

    let mut flattened = Vec::new();
    visit(fields, "", &mut flattened);
    flattened
}

fn model_field_paths(fields: &[ContentFieldDefinition]) -> BTreeSet<String> {
    flatten_field_paths(fields)
        .into_iter()
        .map(|(_, path)| path)
        .collect()
}

fn tera_path_expressions(scope: &str, path: &str) -> Vec<String> {
    let segments = path.split('.').collect::<Vec<_>>();
    let double_quoted = segments
        .iter()
        .map(|segment| format!("[\"{segment}\"]"))
        .collect::<String>();
    let single_quoted = segments
        .iter()
        .map(|segment| format!("['{segment}']"))
        .collect::<String>();
    vec![
        format!("{scope}.{path}"),
        format!("{scope}{double_quoted}"),
        format!("{scope}{single_quoted}"),
    ]
}

fn expression_offsets(source: &str, expression: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut cursor = 0;
    while let Some(found) = source[cursor..].find(expression) {
        let absolute = cursor + found;
        let preceding = source[..absolute].chars().next_back();
        let following = source[absolute + expression.len()..].chars().next();
        let clean_start = preceding
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
        let clean_end = following.is_none_or(|character| {
            !character.is_ascii_alphanumeric() && character != '_' && character != '.'
        });
        if clean_start && clean_end {
            offsets.push(absolute);
        }
        cursor = absolute + expression.len();
    }
    offsets
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
#[allow(clippy::large_enum_variant)]
pub enum ContentModelMutationOperation {
    CreateModel {
        id: String,
        label: String,
        description: String,
    },
    UpdateModel {
        model_id: String,
        label: String,
        description: String,
    },
    RenameModel {
        model_id: String,
        new_id: String,
        label: String,
        description: String,
    },
    DeleteModel {
        model_id: String,
    },
    UpsertField {
        model_id: String,
        parent_field_id: Option<String>,
        original_field_id: Option<String>,
        field: ContentFieldDefinition,
    },
    RemoveField {
        model_id: String,
        parent_field_id: Option<String>,
        field_id: String,
    },
    ReorderField {
        model_id: String,
        parent_field_id: Option<String>,
        field_id: String,
        target_index: usize,
    },
    AttachModel {
        model_id: String,
        section_path: String,
    },
    DetachModel {
        model_id: String,
        section_path: String,
    },
    ReplaceModel {
        section_path: String,
        from_model_id: String,
        to_model_id: String,
        #[serde(default)]
        field_migrations: BTreeMap<String, String>,
    },
    SetPageValues {
        page_file: String,
        values: BTreeMap<String, serde_json::Value>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelMutationInput {
    pub operation: ContentModelMutationOperation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelMutationPlan {
    pub schema_version: u32,
    pub plan_id: String,
    pub operation: String,
    pub label: String,
    pub touched_files: Vec<String>,
    pub affected_pages: Vec<String>,
    pub affected_keys: Vec<String>,
    pub destructive: bool,
    pub blocked: bool,
    pub blockers: Vec<String>,
    pub template_usages: Vec<CustomFieldTemplateUsage>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct PlannedContentModelMutation {
    pub plan: ContentModelMutationPlan,
    pub changes: Vec<WorkspaceResourceMutation>,
    pub deletes: Vec<WorkspaceResourceDelete>,
}

pub fn plan_content_model_mutation(
    project_root: &Path,
    graph: &SourceGraph,
    source_texts: &HashMap<String, String>,
    input: &ContentModelMutationInput,
) -> Result<PlannedContentModelMutation, String> {
    let catalog = graph.content_models.clone();
    if let Some(error) = catalog
        .diagnostics
        .iter()
        .find(|entry| entry.severity == "error")
    {
        return Err(format!(
            "Catalogul modelelor este invalid: {}",
            error.message
        ));
    }
    let mut changes = BTreeMap::<String, String>::new();
    let mut deletes = Vec::new();
    let mut assignments = catalog.assignments.clone();
    let mut affected_pages = BTreeSet::new();
    let mut affected_keys = BTreeSet::new();
    let mut blockers = Vec::new();
    let mut relevant_usages = Vec::new();
    let mut warnings = Vec::new();
    let (operation, label, destructive) = match &input.operation {
        ContentModelMutationOperation::CreateModel {
            id,
            label,
            description,
        } => {
            let id = id.trim();
            validate_identifier(id, "ID-ul modelului")?;
            if catalog.models.iter().any(|model| model.id == id) {
                return Err(format!("Modelul „{id}” există deja."));
            }
            let mut model = ContentModelDefinition {
                schema_version: CONTENT_MODEL_SCHEMA_VERSION,
                id: id.to_string(),
                label: label.trim().to_string(),
                description: description.trim().to_string(),
                fields: Vec::new(),
                file: model_path(id),
            };
            validate_model(&mut model)?;
            changes.insert(model.file.clone(), serialize_model(&model)?);
            (
                "create_model",
                "Creare model de conținut".to_string(),
                false,
            )
        }
        ContentModelMutationOperation::UpdateModel {
            model_id,
            label,
            description,
        } => {
            let mut model = require_model(&catalog, model_id)?.clone();
            model.label = label.trim().to_string();
            model.description = description.trim().to_string();
            validate_model(&mut model)?;
            changes.insert(model.file.clone(), serialize_model(&model)?);
            (
                "update_model",
                "Actualizare model de conținut".to_string(),
                false,
            )
        }
        ContentModelMutationOperation::RenameModel {
            model_id,
            new_id,
            label,
            description,
        } => {
            let new_id = new_id.trim();
            validate_identifier(new_id, "ID-ul nou al modelului")?;
            if new_id == model_id {
                return Err("ID-ul nou este identic cu ID-ul curent.".to_string());
            }
            if catalog.models.iter().any(|model| model.id == new_id) {
                return Err(format!("Modelul „{new_id}” există deja."));
            }
            let original = require_model(&catalog, model_id)?;
            let mut renamed = original.clone();
            renamed.id = new_id.to_string();
            renamed.label = label.trim().to_string();
            renamed.description = description.trim().to_string();
            renamed.file = model_path(new_id);
            validate_model(&mut renamed)?;

            relevant_usages = catalog
                .template_usages
                .iter()
                .filter(|usage| usage.model_id == *model_id)
                .cloned()
                .collect();
            for assignment in &mut assignments {
                if assignment.model_id == *model_id {
                    assignment.model_id = new_id.to_string();
                }
            }
            stage_rename_dynamic_marker_model(
                project_root,
                source_texts,
                graph,
                model_id,
                new_id,
                &mut changes,
            )?;
            changes.insert(renamed.file.clone(), serialize_model(&renamed)?);
            changes.insert(
                CONTENT_MODEL_ASSIGNMENTS_PATH.to_string(),
                serialize_assignments(&assignments)?,
            );
            deletes.push(WorkspaceResourceDelete {
                relative_path: original.file.clone(),
            });
            warnings.push(format!(
                "Assignments și marker-ele dinamice au fost migrate de la {model_id} la {new_id}."
            ));
            (
                "rename_model",
                "Redenumire model de conținut".to_string(),
                false,
            )
        }
        ContentModelMutationOperation::DeleteModel { model_id } => {
            let model = require_model(&catalog, model_id)?;
            affected_keys.extend(model_field_paths(&model.fields));
            relevant_usages = catalog
                .template_usages
                .iter()
                .filter(|usage| usage.model_id == *model_id)
                .cloned()
                .collect();
            if !relevant_usages.is_empty() {
                blockers.push(format!(
                    "{} legături Tera folosesc câmpurile modelului.",
                    relevant_usages.len()
                ));
            }
            for assignment in assignments
                .iter()
                .filter(|entry| entry.model_id == *model_id)
                .cloned()
                .collect::<Vec<_>>()
            {
                stage_remove_model_values(
                    project_root,
                    source_texts,
                    &catalog,
                    model,
                    &assignment,
                    &mut changes,
                    &mut affected_pages,
                )?;
            }
            assignments.retain(|entry| entry.model_id != *model_id);
            changes.insert(
                CONTENT_MODEL_ASSIGNMENTS_PATH.to_string(),
                serialize_assignments(&assignments)?,
            );
            deletes.push(WorkspaceResourceDelete {
                relative_path: model.file.clone(),
            });
            (
                "delete_model",
                "Ștergere model de conținut".to_string(),
                true,
            )
        }
        ContentModelMutationOperation::UpsertField {
            model_id,
            parent_field_id,
            original_field_id,
            field,
        } => {
            let mut model = require_model(&catalog, model_id)?.clone();
            let field = field.clone();
            let original_path = original_field_id
                .as_deref()
                .and_then(|field_id| field_path_by_id(&model.fields, field_id));
            let original_parent = original_field_id
                .as_deref()
                .and_then(|field_id| field_parent_id_by_id(&model.fields, field_id));
            if original_field_id.is_some() && original_path.is_none() {
                return Err(format!(
                    "Câmpul „{}” nu mai există.",
                    original_field_id.as_deref().unwrap_or_default()
                ));
            }
            if original_field_id.is_some()
                && original_parent.as_deref() != parent_field_id.as_deref()
            {
                return Err(
                    "Mutarea unui câmp între containere cere o operație explicită de migrare."
                        .to_string(),
                );
            }
            let next_path = original_path.as_ref().map(|path| {
                let mut path = path.clone();
                if let Some(last) = path.last_mut() {
                    *last = field.key.clone();
                }
                path
            });
            if let (Some(original), Some(old_path), Some(new_path)) = (
                original_field_id,
                original_path.as_ref(),
                next_path.as_ref(),
            ) {
                if old_path != new_path {
                    affected_keys.insert(old_path.join("."));
                    affected_keys.insert(new_path.join("."));
                    let ids = field_subtree_ids(
                        find_field(&model.fields, original)
                            .ok_or_else(|| format!("Câmpul „{original}” nu mai există."))?,
                    );
                    relevant_usages = catalog
                        .template_usages
                        .iter()
                        .filter(|usage| ids.contains(&usage.field_id))
                        .cloned()
                        .collect();
                    stage_rename_field_values(
                        project_root,
                        source_texts,
                        &catalog,
                        model_id,
                        old_path,
                        new_path,
                        &mut changes,
                        &mut affected_pages,
                    )?;
                    stage_rename_template_references(
                        project_root,
                        source_texts,
                        graph,
                        old_path,
                        new_path,
                        None,
                        &mut changes,
                    )?;
                    stage_rename_dynamic_marker_paths(
                        project_root,
                        source_texts,
                        graph,
                        &model,
                        model_id,
                        old_path,
                        new_path,
                        &mut changes,
                    )?;
                    warnings.push(format!(
                        "Valorile și expresiile Tera au fost migrate de la {} la {}.",
                        old_path.join("."),
                        new_path.join(".")
                    ));
                }
            }
            let fields = field_container_mut(&mut model.fields, parent_field_id.as_deref())?;
            match original_field_id {
                Some(original) => {
                    let slot = fields
                        .iter_mut()
                        .find(|existing| existing.id == *original)
                        .ok_or_else(|| format!("Câmpul „{original}” nu mai există."))?;
                    *slot = field;
                }
                None => fields.push(field),
            }
            validate_model(&mut model)?;
            changes.insert(model.file.clone(), serialize_model(&model)?);
            (
                "upsert_field",
                "Actualizare câmp personalizat".to_string(),
                false,
            )
        }
        ContentModelMutationOperation::RemoveField {
            model_id,
            parent_field_id,
            field_id,
        } => {
            let mut model = require_model(&catalog, model_id)?.clone();
            let field_path = field_path_by_id(&model.fields, field_id)
                .ok_or_else(|| format!("Câmpul „{field_id}” nu mai există."))?;
            affected_keys.insert(field_path.join("."));
            let fields = field_container_mut(&mut model.fields, parent_field_id.as_deref())?;
            let index = fields
                .iter()
                .position(|field| field.id == *field_id)
                .ok_or_else(|| format!("Câmpul „{field_id}” nu mai există."))?;
            let field = fields[index].clone();
            let subtree_ids = field_subtree_ids(&field);
            relevant_usages = catalog
                .template_usages
                .iter()
                .filter(|usage| subtree_ids.contains(&usage.field_id))
                .cloned()
                .collect();
            if !relevant_usages.is_empty() {
                blockers.push(format!(
                    "Câmpul este folosit în {} expresii Tera.",
                    relevant_usages.len()
                ));
            }
            stage_remove_field_values(
                project_root,
                source_texts,
                &catalog,
                model_id,
                &field_path,
                &mut changes,
                &mut affected_pages,
            )?;
            fields.remove(index);
            changes.insert(model.file.clone(), serialize_model(&model)?);
            (
                "remove_field",
                "Ștergere câmp personalizat".to_string(),
                true,
            )
        }
        ContentModelMutationOperation::ReorderField {
            model_id,
            parent_field_id,
            field_id,
            target_index,
        } => {
            let mut model = require_model(&catalog, model_id)?.clone();
            let fields = field_container_mut(&mut model.fields, parent_field_id.as_deref())?;
            let index = fields
                .iter()
                .position(|field| field.id == *field_id)
                .ok_or_else(|| format!("Câmpul „{field_id}” nu mai există."))?;
            let field = fields.remove(index);
            let target = (*target_index).min(fields.len());
            fields.insert(target, field);
            changes.insert(model.file.clone(), serialize_model(&model)?);
            (
                "reorder_field",
                "Reordonare câmp personalizat".to_string(),
                false,
            )
        }
        ContentModelMutationOperation::AttachModel {
            model_id,
            section_path,
        } => {
            let model = require_model(&catalog, model_id)?;
            affected_keys.extend(model_field_paths(&model.fields));
            let section_path = normalize_section_path(section_path);
            require_section(graph, &section_path)?;
            if let Some(existing) = assignments
                .iter()
                .find(|entry| entry.section_path == section_path)
            {
                return Err(format!(
                    "Secțiunea are deja atașat modelul „{}”.",
                    existing.model_id
                ));
            }
            let mut adopted_values = 0usize;
            let mut invalid_values = 0usize;
            for page in graph
                .pages
                .iter()
                .filter(|page| page_belongs_to_section(&page.file, &section_path))
            {
                let Some(source) = source_texts.get(&page.file).cloned() else {
                    continue;
                };
                let values = read_extra_values(&source)?;
                let mut page_has_managed_values = false;
                for field in &model.fields {
                    let Some(value) = values.get(&field.key) else {
                        continue;
                    };
                    adopted_values += 1;
                    page_has_managed_values = true;
                    if validate_value_at_path(field, value, &field.key).is_err() {
                        invalid_values += 1;
                    }
                }
                if page_has_managed_values {
                    affected_pages.insert(page.file.clone());
                }
            }
            if adopted_values > 0 {
                warnings.push(format!(
                    "Modelul va adopta {adopted_values} valori `extra` existente din {} pagini; valorile nu sunt suprascrise.",
                    affected_pages.len()
                ));
            }
            if invalid_values > 0 {
                warnings.push(format!(
                    "{invalid_values} valori existente nu respectă încă validarea modelului și vor fi raportate ca neconforme."
                ));
            }
            assignments.push(ContentModelAssignment {
                section_path,
                model_id: model_id.clone(),
            });
            assignments.sort_by(|left, right| left.section_path.cmp(&right.section_path));
            changes.insert(
                CONTENT_MODEL_ASSIGNMENTS_PATH.to_string(),
                serialize_assignments(&assignments)?,
            );
            (
                "attach_model",
                "Atașare model la secțiune".to_string(),
                false,
            )
        }
        ContentModelMutationOperation::DetachModel {
            model_id,
            section_path,
        } => {
            let model = require_model(&catalog, model_id)?;
            affected_keys.extend(model_field_paths(&model.fields));
            let section_path = normalize_section_path(section_path);
            if !assignments
                .iter()
                .any(|entry| entry.section_path == section_path && entry.model_id == *model_id)
            {
                return Err("Atașarea cerută nu mai există.".to_string());
            }
            let section_templates =
                template_files_for_section(graph, &catalog.page_bindings, model_id, &section_path);
            relevant_usages = catalog
                .template_usages
                .iter()
                .filter(|usage| {
                    usage.model_id == *model_id
                        && section_templates
                            .as_ref()
                            .is_none_or(|files| files.contains(&usage.template_file))
                })
                .cloned()
                .collect();
            if !relevant_usages.is_empty() {
                blockers.push(format!(
                    "{} legături Tera folosesc câmpurile modelului.",
                    relevant_usages.len()
                ));
            }
            let assignment = ContentModelAssignment {
                section_path: section_path.clone(),
                model_id: model_id.clone(),
            };
            stage_remove_model_values(
                project_root,
                source_texts,
                &catalog,
                model,
                &assignment,
                &mut changes,
                &mut affected_pages,
            )?;
            assignments.retain(|entry| {
                !(entry.section_path == section_path && entry.model_id == *model_id)
            });
            changes.insert(
                CONTENT_MODEL_ASSIGNMENTS_PATH.to_string(),
                serialize_assignments(&assignments)?,
            );
            (
                "detach_model",
                "Detașează modelul și șterge datele".to_string(),
                true,
            )
        }
        ContentModelMutationOperation::ReplaceModel {
            section_path,
            from_model_id,
            to_model_id,
            field_migrations,
        } => {
            let from_model = require_model(&catalog, from_model_id)?;
            let to_model = require_model(&catalog, to_model_id)?;
            affected_keys.extend(model_field_paths(&from_model.fields));
            affected_keys.extend(model_field_paths(&to_model.fields));
            if from_model_id == to_model_id {
                return Err("Modelul înlocuitor trebuie să fie diferit.".to_string());
            }
            let section_path = normalize_section_path(section_path);
            require_section(graph, &section_path)?;
            let section_templates = template_files_for_section(
                graph,
                &catalog.page_bindings,
                from_model_id,
                &section_path,
            );
            let has_other_sections = catalog.assignments.iter().any(|assignment| {
                assignment.model_id == *from_model_id && assignment.section_path != section_path
            });
            let other_section_templates = template_files_for_other_sections(
                graph,
                &catalog.page_bindings,
                &catalog.assignments,
                from_model_id,
                &section_path,
            );
            let assignment = assignments
                .iter_mut()
                .find(|entry| {
                    entry.section_path == section_path && entry.model_id == *from_model_id
                })
                .ok_or_else(|| "Atașarea care trebuie înlocuită nu mai există.".to_string())?;

            let mut migrations = Vec::new();
            let mut marker_migrations = Vec::new();
            let mut migrated_ids = BTreeSet::new();
            for (from_field_id, to_field_id) in field_migrations {
                let from_field = find_field(&from_model.fields, from_field_id)
                    .ok_or_else(|| format!("Câmpul sursă „{from_field_id}” nu există."))?;
                let to_field = find_field(&to_model.fields, to_field_id)
                    .ok_or_else(|| format!("Câmpul destinație „{to_field_id}” nu există."))?;
                if from_field.kind != to_field.kind {
                    return Err(format!(
                        "Migrarea {} → {} cere tipuri identice.",
                        from_field.label, to_field.label
                    ));
                }
                let from_path = field_path_by_id(&from_model.fields, from_field_id)
                    .expect("field existence checked");
                let to_path = field_path_by_id(&to_model.fields, to_field_id)
                    .expect("field existence checked");
                if from_path.len() != 1 || to_path.len() != 1 {
                    return Err(
                        "Migrarea între modele acceptă momentan câmpuri-rădăcină; grupurile și repeaterele identice sunt păstrate prin aceeași cheie."
                            .to_string(),
                    );
                }
                marker_migrations.push((
                    from_field_id.clone(),
                    to_field_id.clone(),
                    from_path.clone(),
                    to_path.clone(),
                ));
                migrations.push((from_path, to_path));
                migrated_ids.insert(from_field_id.clone());
            }
            relevant_usages = catalog
                .template_usages
                .iter()
                .filter(|usage| {
                    usage.model_id == *from_model_id
                        && section_templates
                            .as_ref()
                            .is_none_or(|files| files.contains(&usage.template_file))
                })
                .cloned()
                .collect();
            let unmigrated_usages = relevant_usages
                .iter()
                .filter(|usage| !migrated_ids.contains(&usage.field_id))
                .count();
            if unmigrated_usages > 0 {
                blockers.push(format!(
                    "{unmigrated_usages} legături Tera ale modelului vechi nu au o migrare."
                ));
            }
            let shared_usages = if has_other_sections {
                relevant_usages
                    .iter()
                    .filter(|usage| {
                        other_section_templates
                            .as_ref()
                            .is_none_or(|files| files.contains(&usage.template_file))
                    })
                    .count()
            } else {
                0
            };
            if shared_usages > 0 {
                blockers.push(format!(
                    "{shared_usages} legături Tera se află în șabloane comune cu alte secțiuni care păstrează modelul vechi; separă șablonul sau migrează toate secțiunile."
                ));
            }
            stage_replace_model_values(
                project_root,
                source_texts,
                &catalog,
                from_model,
                to_model,
                &section_path,
                &migrations,
                &mut changes,
                &mut affected_pages,
            )?;
            for (from_path, to_path) in &migrations {
                stage_rename_template_references(
                    project_root,
                    source_texts,
                    graph,
                    from_path,
                    to_path,
                    section_templates.as_ref(),
                    &mut changes,
                )?;
            }
            for (from_field_id, to_field_id, from_path, to_path) in &marker_migrations {
                stage_replace_dynamic_marker_binding(
                    project_root,
                    source_texts,
                    graph,
                    from_model_id,
                    from_field_id,
                    from_path,
                    to_model_id,
                    to_field_id,
                    to_path,
                    section_templates.as_ref(),
                    &mut changes,
                )?;
            }
            assignment.model_id = to_model_id.clone();
            changes.insert(
                CONTENT_MODEL_ASSIGNMENTS_PATH.to_string(),
                serialize_assignments(&assignments)?,
            );
            warnings.push(format!(
                "Modelul {} este înlocuit cu {}; valorile fără corespondent vor fi eliminate.",
                from_model.label, to_model.label
            ));
            (
                "replace_model",
                "Înlocuire și migrare model de conținut".to_string(),
                true,
            )
        }
        ContentModelMutationOperation::SetPageValues { page_file, values } => {
            let binding = catalog
                .page_bindings
                .iter()
                .find(|binding| binding.page_file == *page_file)
                .ok_or_else(|| format!("Pagina {page_file} nu are un model de conținut atașat."))?;
            let model = require_model(&catalog, &binding.model_id)?;
            affected_keys.extend(values.keys().cloned());
            validate_page_values(model, values)?;
            let source = required_source(project_root, source_texts, page_file)?;
            let next = rewrite_extra_values(&source, &field_keys(&model.fields), values)?;
            changes.insert(page_file.clone(), next);
            affected_pages.insert(page_file.clone());
            (
                "set_page_values",
                "Actualizare câmpuri personalizate".to_string(),
                false,
            )
        }
    };
    ensure_metadata_contracts(&catalog, &mut changes)?;
    let changes = changes
        .into_iter()
        .map(|(relative_path, contents)| WorkspaceResourceMutation {
            relative_path,
            contents,
            create_only: false,
        })
        .collect::<Vec<_>>();
    let touched_files = changes
        .iter()
        .map(|change| change.relative_path.clone())
        .chain(deletes.iter().map(|delete| delete.relative_path.clone()))
        .collect::<Vec<_>>();
    let plan_id = content_plan_id(input, &changes, &deletes);
    Ok(PlannedContentModelMutation {
        plan: ContentModelMutationPlan {
            schema_version: CONTENT_MODEL_SCHEMA_VERSION,
            plan_id,
            operation: operation.to_string(),
            label,
            touched_files,
            affected_pages: affected_pages.into_iter().collect(),
            affected_keys: affected_keys.into_iter().collect(),
            destructive,
            blocked: !blockers.is_empty(),
            blockers,
            template_usages: relevant_usages,
            warnings,
        },
        changes,
        deletes,
    })
}

pub fn stage_content_model_mutation(
    workspace: &mut ProjectWorkspace,
    planned: PlannedContentModelMutation,
    now_ms: u128,
) -> Result<(ContentModelMutationPlan, ProjectWorkspaceMutationReceipt), String> {
    if planned.plan.blocked {
        return Err(format!(
            "Planul este blocat: {}",
            planned.plan.blockers.join(" ")
        ));
    }
    let identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    let plan = planned.plan;
    let mutation = workspace.stage_composite_changes(
        &identity,
        WorkspaceMutationMetadata {
            label: plan.label.clone(),
            source: "content_models.semantic".to_string(),
            coalesce_key: None,
            transaction_id: Some(format!("content-model-{}", plan.plan_id)),
        },
        planned.changes,
        planned.deletes,
        None,
        now_ms,
    )?;
    Ok((plan, mutation))
}

fn model_path(id: &str) -> String {
    format!("{CONTENT_MODEL_DIRECTORY}/{id}.toml")
}

fn require_model<'a>(
    catalog: &'a ContentModelCatalog,
    id: &str,
) -> Result<&'a ContentModelDefinition, String> {
    catalog
        .models
        .iter()
        .find(|model| model.id == id)
        .ok_or_else(|| format!("Modelul „{id}” nu există."))
}

fn find_field<'a>(
    fields: &'a [ContentFieldDefinition],
    field_id: &str,
) -> Option<&'a ContentFieldDefinition> {
    for field in fields {
        if field.id == field_id {
            return Some(field);
        }
        if let Some(found) = find_field(&field.fields, field_id) {
            return Some(found);
        }
    }
    None
}

fn find_field_mut<'a>(
    fields: &'a mut [ContentFieldDefinition],
    field_id: &str,
) -> Option<&'a mut ContentFieldDefinition> {
    for field in fields {
        if field.id == field_id {
            return Some(field);
        }
        if let Some(found) = find_field_mut(&mut field.fields, field_id) {
            return Some(found);
        }
    }
    None
}

fn field_container_mut<'a>(
    fields: &'a mut Vec<ContentFieldDefinition>,
    parent_field_id: Option<&str>,
) -> Result<&'a mut Vec<ContentFieldDefinition>, String> {
    let Some(parent_field_id) = parent_field_id else {
        return Ok(fields);
    };
    let parent = find_field_mut(fields, parent_field_id)
        .ok_or_else(|| format!("Containerul „{parent_field_id}” nu mai există."))?;
    if !matches!(
        parent.kind,
        ContentFieldKind::Group | ContentFieldKind::Repeater
    ) {
        return Err(format!(
            "Câmpul „{}” nu poate conține subcâmpuri.",
            parent.label
        ));
    }
    Ok(&mut parent.fields)
}

fn field_path_by_id(fields: &[ContentFieldDefinition], field_id: &str) -> Option<Vec<String>> {
    fn visit(fields: &[ContentFieldDefinition], field_id: &str, path: &mut Vec<String>) -> bool {
        for field in fields {
            path.push(field.key.clone());
            if field.id == field_id || visit(&field.fields, field_id, path) {
                return true;
            }
            path.pop();
        }
        false
    }

    let mut path = Vec::new();
    visit(fields, field_id, &mut path).then_some(path)
}

fn field_item_path_by_id(fields: &[ContentFieldDefinition], field_id: &str) -> Option<Vec<String>> {
    fn visit(
        fields: &[ContentFieldDefinition],
        field_id: &str,
        item_parent: Option<&[String]>,
    ) -> Option<Vec<String>> {
        for field in fields {
            let item_path = item_parent.map(|parent| {
                let mut path = parent.to_vec();
                path.push(field.key.clone());
                path
            });
            if field.id == field_id {
                return item_path;
            }
            let empty = Vec::new();
            let next_item_parent = if field.kind == ContentFieldKind::Repeater {
                Some(empty.as_slice())
            } else {
                item_path.as_deref()
            };
            if let Some(found) = visit(&field.fields, field_id, next_item_parent) {
                return Some(found);
            }
        }
        None
    }

    visit(fields, field_id, None)
}

fn field_parent_id_by_id(fields: &[ContentFieldDefinition], field_id: &str) -> Option<String> {
    for field in fields {
        if field.fields.iter().any(|child| child.id == field_id) {
            return Some(field.id.clone());
        }
        if let Some(parent) = field_parent_id_by_id(&field.fields, field_id) {
            return Some(parent);
        }
    }
    None
}

fn field_subtree_ids(field: &ContentFieldDefinition) -> BTreeSet<String> {
    let mut ids = BTreeSet::from([field.id.clone()]);
    for child in &field.fields {
        ids.extend(field_subtree_ids(child));
    }
    ids
}

fn require_section(graph: &SourceGraph, section_path: &str) -> Result<(), String> {
    if graph.pages.iter().any(|page| {
        page.file == section_path
            && matches!(
                page.page_kind,
                crate::source_graph::model::SourcePageKind::Section
                    | crate::source_graph::model::SourcePageKind::Home
            )
    }) {
        Ok(())
    } else {
        Err(format!("SourceGraph nu conține secțiunea {section_path}."))
    }
}

fn serialize_model(model: &ContentModelDefinition) -> Result<String, String> {
    toml_edit::ser::to_string_pretty(model)
        .map_err(|error| format!("Modelul nu poate fi serializat: {error}"))
}

fn serialize_assignments(assignments: &[ContentModelAssignment]) -> Result<String, String> {
    toml_edit::ser::to_string_pretty(&AssignmentContract {
        schema_version: CONTENT_MODEL_SCHEMA_VERSION,
        assignments: assignments.to_vec(),
    })
    .map_err(|error| format!("Atribuirile nu pot fi serializate: {error}"))
}

fn ensure_metadata_contracts(
    catalog: &ContentModelCatalog,
    changes: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    if !catalog.metadata_present && !changes.contains_key(CONTENT_MODEL_PROJECT_PATH) {
        changes.insert(
            CONTENT_MODEL_PROJECT_PATH.to_string(),
            toml_edit::ser::to_string_pretty(&ProjectContract::default())
                .map_err(|error| error.to_string())?,
        );
    }
    if !changes.contains_key(CONTENT_MODEL_ASSIGNMENTS_PATH) && catalog.assignments.is_empty() {
        changes.insert(
            CONTENT_MODEL_ASSIGNMENTS_PATH.to_string(),
            serialize_assignments(&[])?,
        );
    }
    Ok(())
}

fn stage_remove_model_values(
    project_root: &Path,
    source_texts: &HashMap<String, String>,
    catalog: &ContentModelCatalog,
    model: &ContentModelDefinition,
    assignment: &ContentModelAssignment,
    changes: &mut BTreeMap<String, String>,
    affected_pages: &mut BTreeSet<String>,
) -> Result<(), String> {
    let keys = field_keys(&model.fields);
    for binding in catalog.page_bindings.iter().filter(|binding| {
        binding.model_id == model.id && binding.section_path == assignment.section_path
    }) {
        let source = changes
            .get(&binding.page_file)
            .cloned()
            .unwrap_or(required_source(
                project_root,
                source_texts,
                &binding.page_file,
            )?);
        let next = rewrite_extra_values(&source, &keys, &BTreeMap::new())?;
        if next != source {
            changes.insert(binding.page_file.clone(), next);
            affected_pages.insert(binding.page_file.clone());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stage_replace_model_values(
    project_root: &Path,
    source_texts: &HashMap<String, String>,
    catalog: &ContentModelCatalog,
    from_model: &ContentModelDefinition,
    to_model: &ContentModelDefinition,
    section_path: &str,
    migrations: &[(Vec<String>, Vec<String>)],
    changes: &mut BTreeMap<String, String>,
    affected_pages: &mut BTreeSet<String>,
) -> Result<(), String> {
    let from_keys = field_keys(&from_model.fields);
    let to_keys = field_keys(&to_model.fields);
    let managed_keys = from_keys.union(&to_keys).cloned().collect::<BTreeSet<_>>();
    for binding in catalog
        .page_bindings
        .iter()
        .filter(|binding| binding.model_id == from_model.id && binding.section_path == section_path)
    {
        let source = changes
            .get(&binding.page_file)
            .cloned()
            .unwrap_or(required_source(
                project_root,
                source_texts,
                &binding.page_file,
            )?);
        let values = read_extra_values(&source)?;
        let mut replacement = to_keys
            .iter()
            .filter_map(|key| values.get(key).cloned().map(|value| (key.clone(), value)))
            .collect::<BTreeMap<_, _>>();
        for (from_path, to_path) in migrations {
            let (Some(from_key), Some(to_key)) = (from_path.first(), to_path.first()) else {
                continue;
            };
            let Some(value) = values.get(from_key).cloned() else {
                continue;
            };
            if from_key != to_key && replacement.contains_key(to_key) {
                return Err(format!(
                    "Pagina {} conține deja extra.{to_key}; migrarea nu suprascrie valoarea existentă.",
                    binding.page_file
                ));
            }
            replacement.insert(to_key.clone(), value);
        }
        let next = rewrite_extra_values(&source, &managed_keys, &replacement)?;
        if next != source {
            changes.insert(binding.page_file.clone(), next);
            affected_pages.insert(binding.page_file.clone());
        }
    }
    Ok(())
}

fn stage_remove_field_values(
    project_root: &Path,
    source_texts: &HashMap<String, String>,
    catalog: &ContentModelCatalog,
    model_id: &str,
    field_path: &[String],
    changes: &mut BTreeMap<String, String>,
    affected_pages: &mut BTreeSet<String>,
) -> Result<(), String> {
    let Some(root_key) = field_path.first() else {
        return Err("Calea câmpului care trebuie eliminat este goală.".to_string());
    };
    for binding in catalog
        .page_bindings
        .iter()
        .filter(|binding| binding.model_id == model_id)
    {
        let source = changes
            .get(&binding.page_file)
            .cloned()
            .unwrap_or(required_source(
                project_root,
                source_texts,
                &binding.page_file,
            )?);
        let next = if field_path.len() == 1 {
            rewrite_extra_values(
                &source,
                &BTreeSet::from([root_key.clone()]),
                &BTreeMap::new(),
            )?
        } else {
            let mut values = read_extra_values(&source)?;
            let Some(root_value) = values.get_mut(root_key) else {
                continue;
            };
            if !remove_nested_value(root_value, &field_path[1..]) {
                continue;
            }
            let replacement = if structurally_empty(root_value) {
                BTreeMap::new()
            } else {
                BTreeMap::from([(root_key.clone(), root_value.clone())])
            };
            rewrite_extra_values(&source, &BTreeSet::from([root_key.clone()]), &replacement)?
        };
        if next != source {
            changes.insert(binding.page_file.clone(), next);
            affected_pages.insert(binding.page_file.clone());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stage_rename_field_values(
    project_root: &Path,
    source_texts: &HashMap<String, String>,
    catalog: &ContentModelCatalog,
    model_id: &str,
    old_path: &[String],
    new_path: &[String],
    changes: &mut BTreeMap<String, String>,
    affected_pages: &mut BTreeSet<String>,
) -> Result<(), String> {
    let (Some(old_root), Some(new_root)) = (old_path.first(), new_path.first()) else {
        return Err("Migrarea cere căi de câmp complete.".to_string());
    };
    for binding in catalog
        .page_bindings
        .iter()
        .filter(|binding| binding.model_id == model_id)
    {
        let source = changes
            .get(&binding.page_file)
            .cloned()
            .unwrap_or(required_source(
                project_root,
                source_texts,
                &binding.page_file,
            )?);
        let mut values = read_extra_values(&source)?;
        let changed = if old_path.len() == 1 && new_path.len() == 1 {
            if old_root == new_root || !values.contains_key(old_root) {
                false
            } else {
                if values.contains_key(new_root) {
                    return Err(format!(
                        "Pagina {} conține deja cheia extra.{new_root}; migrarea nu poate suprascrie date independente.",
                        binding.page_file
                    ));
                }
                let value = values
                    .remove(old_root)
                    .expect("key presence checked before removal");
                values.insert(new_root.clone(), value);
                true
            }
        } else {
            if old_root != new_root {
                return Err(
                    "Migrarea simultană a rădăcinii și a unui subcâmp nu este permisă.".to_string(),
                );
            }
            let Some(root_value) = values.get_mut(old_root) else {
                continue;
            };
            rename_nested_value(root_value, &old_path[1..], &new_path[1..]).map_err(|reason| {
                format!(
                    "Pagina {} nu poate migra {} la {}: {reason}",
                    binding.page_file,
                    old_path.join("."),
                    new_path.join(".")
                )
            })?
        };
        if !changed {
            continue;
        }
        let next = rewrite_extra_values(
            &source,
            &BTreeSet::from([old_root.clone(), new_root.clone()]),
            &values
                .into_iter()
                .filter(|(key, _)| key == old_root || key == new_root)
                .collect(),
        )?;
        if next != source {
            changes.insert(binding.page_file.clone(), next);
            affected_pages.insert(binding.page_file.clone());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stage_rename_template_references(
    _project_root: &Path,
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    old_path: &[String],
    new_path: &[String],
    allowed_files: Option<&BTreeSet<String>>,
    changes: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let old_path = old_path.join(".");
    let new_path = new_path.join(".");
    for template in &graph.templates {
        if allowed_files.is_some_and(|files| !files.contains(&template.file)) {
            continue;
        }
        let source = changes
            .get(&template.file)
            .cloned()
            .or_else(|| source_texts.get(&template.file).cloned())
            .ok_or_else(|| format!("ProjectWorkspace nu urmărește sursa {}.", template.file))?;
        let mut next = source.clone();
        for scope in ["page.extra", "section.extra"] {
            for (old_expression, new_expression) in tera_path_expressions(scope, &old_path)
                .into_iter()
                .zip(tera_path_expressions(scope, &new_path))
            {
                next = replace_expression_prefix(&next, &old_expression, &new_expression);
            }
        }
        if next != source {
            changes.insert(template.file.clone(), next);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stage_rename_dynamic_marker_paths(
    project_root: &Path,
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    model: &ContentModelDefinition,
    model_id: &str,
    old_path: &[String],
    new_path: &[String],
    changes: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let old_path = old_path.join(".");
    let new_path = new_path.join(".");
    for template in &graph.templates {
        let source = tracked_template_source(project_root, source_texts, changes, &template.file)?;
        let next = rewrite_dynamic_item_binding_expressions(
            &source, model, model_id, &old_path, &new_path,
        );
        let next = rewrite_dynamic_marker_path_prefix(&next, model_id, &old_path, &new_path);
        if next != source {
            changes.insert(template.file.clone(), next);
        }
    }
    Ok(())
}

fn rewrite_dynamic_item_binding_expressions(
    source: &str,
    model: &ContentModelDefinition,
    model_id: &str,
    old_path: &str,
    new_path: &str,
) -> String {
    const START: &str = "{# pana:dynamic ";
    let mut rendered = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(found) = source[cursor..].find(START) {
        let absolute = cursor + found;
        let body_start = absolute + START.len();
        let Some(end) = source[body_start..].find("#}") else {
            break;
        };
        let marker_end = body_start + end + 2;
        let next_marker = source[marker_end..]
            .find(START)
            .map(|offset| marker_end + offset)
            .unwrap_or(source.len());
        let attributes = source[body_start..body_start + end]
            .split_ascii_whitespace()
            .filter_map(|part| part.split_once('='))
            .collect::<HashMap<_, _>>();
        let mut owned_body = source[marker_end..next_marker].to_string();
        if attributes.get("model").copied() == Some(model_id)
            && attributes.get("scope").copied() == Some("item")
        {
            let marker_path = attributes.get("path").copied().unwrap_or_default();
            let path_matches =
                marker_path == old_path || marker_path.starts_with(&format!("{old_path}."));
            if path_matches {
                if let Some(old_item_path) = attributes
                    .get("field")
                    .and_then(|field_id| field_item_path_by_id(&model.fields, field_id))
                {
                    let old_item_path = old_item_path.join(".");
                    let renamed_full_path = if marker_path == old_path {
                        new_path.to_string()
                    } else {
                        format!(
                            "{new_path}.{}",
                            marker_path
                                .strip_prefix(&format!("{old_path}."))
                                .unwrap_or_default()
                        )
                    };
                    let segment_count = old_item_path.split('.').count();
                    let new_item_path = renamed_full_path
                        .split('.')
                        .rev()
                        .take(segment_count)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join(".");
                    owned_body = replace_expression_prefix_once(
                        &owned_body,
                        &format!("item.{old_item_path}"),
                        &format!("item.{new_item_path}"),
                    );
                }
            }
        }
        rendered.push_str(&source[cursor..marker_end]);
        rendered.push_str(&owned_body);
        cursor = next_marker;
    }
    rendered.push_str(&source[cursor..]);
    rendered
}

fn stage_rename_dynamic_marker_model(
    project_root: &Path,
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    old_model_id: &str,
    new_model_id: &str,
    changes: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for template in &graph.templates {
        let source = tracked_template_source(project_root, source_texts, changes, &template.file)?;
        let next = rewrite_dynamic_marker_comments(&source, |attributes, marker| {
            if attributes.get("model").copied() != Some(old_model_id) {
                return marker.to_string();
            }
            marker.replacen(
                &format!("model={old_model_id}"),
                &format!("model={new_model_id}"),
                1,
            )
        });
        if next != source {
            changes.insert(template.file.clone(), next);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn stage_replace_dynamic_marker_binding(
    project_root: &Path,
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    from_model_id: &str,
    from_field_id: &str,
    from_path: &[String],
    to_model_id: &str,
    to_field_id: &str,
    to_path: &[String],
    allowed_files: Option<&BTreeSet<String>>,
    changes: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let from_path = from_path.join(".");
    let to_path = to_path.join(".");
    for template in &graph.templates {
        if allowed_files.is_some_and(|files| !files.contains(&template.file)) {
            continue;
        }
        let source = tracked_template_source(project_root, source_texts, changes, &template.file)?;
        let next = rewrite_dynamic_marker_binding(
            &source,
            from_model_id,
            from_field_id,
            &from_path,
            to_model_id,
            to_field_id,
            &to_path,
        );
        if next != source {
            changes.insert(template.file.clone(), next);
        }
    }
    Ok(())
}

fn tracked_template_source(
    _project_root: &Path,
    source_texts: &HashMap<String, String>,
    changes: &BTreeMap<String, String>,
    template_file: &str,
) -> Result<String, String> {
    changes
        .get(template_file)
        .cloned()
        .or_else(|| source_texts.get(template_file).cloned())
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește sursa {template_file}."))
}

fn rewrite_dynamic_marker_path_prefix(
    source: &str,
    model_id: &str,
    old_path: &str,
    new_path: &str,
) -> String {
    rewrite_dynamic_marker_comments(source, |attributes, marker| {
        if attributes.get("model").copied() != Some(model_id) {
            return marker.to_string();
        }
        let Some(path) = attributes.get("path").copied() else {
            return marker.to_string();
        };
        let next_path = if path == old_path {
            new_path.to_string()
        } else if let Some(suffix) = path.strip_prefix(&format!("{old_path}.")) {
            format!("{new_path}.{suffix}")
        } else {
            return marker.to_string();
        };
        marker.replacen(&format!("path={path}"), &format!("path={next_path}"), 1)
    })
}

#[allow(clippy::too_many_arguments)]
fn rewrite_dynamic_marker_binding(
    source: &str,
    from_model_id: &str,
    from_field_id: &str,
    from_path: &str,
    to_model_id: &str,
    to_field_id: &str,
    to_path: &str,
) -> String {
    rewrite_dynamic_marker_comments(source, |attributes, marker| {
        if attributes.get("model").copied() != Some(from_model_id)
            || attributes.get("field").copied() != Some(from_field_id)
            || attributes.get("path").copied() != Some(from_path)
        {
            return marker.to_string();
        }
        marker
            .replacen(
                &format!("model={from_model_id}"),
                &format!("model={to_model_id}"),
                1,
            )
            .replacen(
                &format!("field={from_field_id}"),
                &format!("field={to_field_id}"),
                1,
            )
            .replacen(&format!("path={from_path}"), &format!("path={to_path}"), 1)
    })
}

fn rewrite_dynamic_marker_comments(
    source: &str,
    mut rewrite: impl FnMut(&HashMap<&str, &str>, &str) -> String,
) -> String {
    const START: &str = "{# pana:dynamic ";
    let mut rendered = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(found) = source[cursor..].find(START) {
        let absolute = cursor + found;
        let body_start = absolute + START.len();
        let Some(end) = source[body_start..].find("#}") else {
            break;
        };
        let marker_end = body_start + end + 2;
        let marker = &source[absolute..marker_end];
        let attributes = source[body_start..body_start + end]
            .split_ascii_whitespace()
            .filter_map(|part| part.split_once('='))
            .collect::<HashMap<_, _>>();
        rendered.push_str(&source[cursor..absolute]);
        rendered.push_str(&rewrite(&attributes, marker));
        cursor = marker_end;
    }
    rendered.push_str(&source[cursor..]);
    rendered
}

fn replace_expression_prefix(source: &str, old: &str, new: &str) -> String {
    let mut rendered = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(found) = source[cursor..].find(old) {
        let absolute = cursor + found;
        let preceding = source[..absolute].chars().next_back();
        let following = source[absolute + old.len()..].chars().next();
        let clean_start = preceding
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
        let clean_end = following
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
        rendered.push_str(&source[cursor..absolute]);
        if clean_start && clean_end {
            rendered.push_str(new);
        } else {
            rendered.push_str(old);
        }
        cursor = absolute + old.len();
    }
    rendered.push_str(&source[cursor..]);
    rendered
}

fn replace_expression_prefix_once(source: &str, old: &str, new: &str) -> String {
    let mut cursor = 0;
    while let Some(found) = source[cursor..].find(old) {
        let absolute = cursor + found;
        let preceding = source[..absolute].chars().next_back();
        let following = source[absolute + old.len()..].chars().next();
        let clean_start = preceding
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
        let clean_end = following
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
        if clean_start && clean_end {
            return format!(
                "{}{}{}",
                &source[..absolute],
                new,
                &source[absolute + old.len()..]
            );
        }
        cursor = absolute + old.len();
    }
    source.to_string()
}

fn remove_nested_value(value: &mut serde_json::Value, path: &[String]) -> bool {
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

fn rename_nested_value(
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

fn structurally_empty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::Object(object) => object.is_empty(),
        serde_json::Value::Array(items) => items.is_empty() || items.iter().all(structurally_empty),
        _ => false,
    }
}

fn required_source(
    _project_root: &Path,
    source_texts: &HashMap<String, String>,
    path: &str,
) -> Result<String, String> {
    source_texts
        .get(path)
        .cloned()
        .ok_or_else(|| format!("ProjectWorkspace nu urmărește sursa {path}."))
}

fn field_keys(fields: &[ContentFieldDefinition]) -> BTreeSet<String> {
    fields.iter().map(|field| field.key.clone()).collect()
}

fn validate_page_values(
    model: &ContentModelDefinition,
    values: &BTreeMap<String, serde_json::Value>,
) -> Result<(), String> {
    let fields = model
        .fields
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect::<HashMap<_, _>>();
    for key in values.keys() {
        if !fields.contains_key(key.as_str()) {
            return Err(format!(
                "Valoarea „{key}” nu aparține modelului {}.",
                model.id
            ));
        }
    }
    for field in fields.values() {
        let value = values.get(&field.key).or(field.default_value.as_ref());
        if field.required && value.is_none_or(serde_json::Value::is_null) {
            return Err(format!(
                "Câmpul obligatoriu „{}” nu are valoare.",
                field.label
            ));
        }
        if let Some(value) = value.filter(|value| !value.is_null()) {
            validate_value_at_path(field, value, &field.key)?;
        }
    }
    Ok(())
}

fn validate_value_at_path(
    field: &ContentFieldDefinition,
    value: &serde_json::Value,
    path: &str,
) -> Result<(), String> {
    let valid = match field.kind {
        ContentFieldKind::Text
        | ContentFieldKind::Textarea
        | ContentFieldKind::Markdown
        | ContentFieldKind::Date
        | ContentFieldKind::Url
        | ContentFieldKind::Color
        | ContentFieldKind::Image
        | ContentFieldKind::Select => value.is_string(),
        ContentFieldKind::Number => value.is_number(),
        ContentFieldKind::Boolean => value.is_boolean(),
        ContentFieldKind::Group => value.is_object(),
        ContentFieldKind::Repeater => value.is_array(),
    };
    if !valid {
        return Err(format!(
            "Valoarea pentru „{}” nu corespunde tipului {:?}.",
            path, field.kind
        ));
    }
    if field.kind == ContentFieldKind::Select {
        let selected = value.as_str().unwrap_or_default();
        if !field.choices.iter().any(|choice| choice.value == selected) {
            return Err(format!(
                "Valoarea „{selected}” nu este o opțiune permisă pentru {}.",
                field.label
            ));
        }
    }
    if let Some(text) = value.as_str() {
        if let Some(pattern) = field
            .pattern
            .as_deref()
            .filter(|pattern| !pattern.is_empty())
        {
            let expression = regex::Regex::new(pattern).map_err(|error| {
                format!(
                    "Pattern-ul câmpului „{}” este invalid: {error}",
                    field.label
                )
            })?;
            if !expression.is_match(text) {
                return Err(format!(
                    "Valoarea pentru „{path}” nu respectă pattern-ul câmpului."
                ));
            }
        }
        match field.kind {
            ContentFieldKind::Date
                if !regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$")
                    .expect("static date regex")
                    .is_match(text) =>
            {
                return Err(format!("Valoarea pentru „{path}” nu este o dată ISO."));
            }
            ContentFieldKind::Url
                if !text.starts_with('/')
                    && !text.starts_with('#')
                    && url::Url::parse(text).is_err() =>
            {
                return Err(format!(
                    "Valoarea pentru „{path}” nu este un URL absolut sau o cale de site."
                ));
            }
            ContentFieldKind::Color
                if !regex::Regex::new(
                    r"^(#[0-9A-Fa-f]{3,4}|#[0-9A-Fa-f]{6}|#[0-9A-Fa-f]{8}|[A-Za-z][A-Za-z0-9-]*)$",
                )
                .expect("static color regex")
                .is_match(text) =>
            {
                return Err(format!(
                    "Valoarea pentru „{path}” nu este o culoare CSS simplă."
                ));
            }
            _ => {}
        }
    }
    if let Some(number) = value.as_f64() {
        if field.minimum.is_some_and(|minimum| number < minimum)
            || field.maximum.is_some_and(|maximum| number > maximum)
        {
            return Err(format!(
                "Valoarea numerică pentru „{}” este în afara limitelor.",
                field.label
            ));
        }
    }
    match field.kind {
        ContentFieldKind::Group => {
            validate_nested_object(
                &field.fields,
                value.as_object().expect("type checked"),
                path,
            )?;
        }
        ContentFieldKind::Repeater => {
            for (index, item) in value.as_array().expect("type checked").iter().enumerate() {
                let object = item.as_object().ok_or_else(|| {
                    format!("Elementul {index} din „{path}” trebuie să fie un obiect.")
                })?;
                validate_nested_object(&field.fields, object, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_nested_object(
    fields: &[ContentFieldDefinition],
    object: &serde_json::Map<String, serde_json::Value>,
    parent_path: &str,
) -> Result<(), String> {
    let definitions = fields
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect::<HashMap<_, _>>();
    for key in object.keys() {
        if !definitions.contains_key(key.as_str()) {
            return Err(format!(
                "Valoarea „{parent_path}.{key}” nu aparține contractului."
            ));
        }
    }
    for field in fields {
        let path = format!("{parent_path}.{}", field.key);
        let value = object.get(&field.key).or(field.default_value.as_ref());
        if field.required && value.is_none_or(serde_json::Value::is_null) {
            return Err(format!("Câmpul obligatoriu „{path}” nu are valoare."));
        }
        if let Some(value) = value.filter(|value| !value.is_null()) {
            validate_value_at_path(field, value, &path)?;
        }
    }
    Ok(())
}

fn collect_missing_required_fields(
    fields: &[ContentFieldDefinition],
    values: &BTreeMap<String, serde_json::Value>,
    parent_path: &str,
    missing: &mut Vec<String>,
) {
    for field in fields {
        let path = if parent_path.is_empty() {
            field.key.clone()
        } else {
            format!("{parent_path}.{}", field.key)
        };
        let value = values.get(&field.key).or(field.default_value.as_ref());
        if field.required && value.is_none_or(serde_json::Value::is_null) {
            missing.push(field.id.clone());
        }
        let Some(value) = value else {
            continue;
        };
        match field.kind {
            ContentFieldKind::Group => {
                if let Some(object) = value.as_object() {
                    let nested = object
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<BTreeMap<_, _>>();
                    collect_missing_required_fields(&field.fields, &nested, &path, missing);
                }
            }
            ContentFieldKind::Repeater => {
                for item in value.as_array().into_iter().flatten() {
                    if let Some(object) = item.as_object() {
                        let nested = object
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect::<BTreeMap<_, _>>();
                        collect_missing_required_fields(&field.fields, &nested, &path, missing);
                    }
                }
            }
            _ => {}
        }
    }
}

fn rewrite_extra_values(
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

fn content_plan_id(
    input: &ContentModelMutationInput,
    changes: &[WorkspaceResourceMutation],
    deletes: &[WorkspaceResourceDelete],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(input).expect("ContentModelMutationInput serialization cannot fail"),
    );
    for change in changes {
        hasher.update(change.relative_path.as_bytes());
        hasher.update([0]);
        hasher.update(change.contents.as_bytes());
        hasher.update([0xff]);
    }
    for delete in deletes {
        hasher.update(delete.relative_path.as_bytes());
        hasher.update([0xfe]);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{
                ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
            },
            project_workspace::ProjectWorkspace,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    };

    fn fixture_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-content-model-{name}-{nonce}"))
    }

    fn field(
        id: &str,
        key: &str,
        kind: ContentFieldKind,
        fields: Vec<ContentFieldDefinition>,
    ) -> ContentFieldDefinition {
        ContentFieldDefinition {
            id: id.to_string(),
            key: key.to_string(),
            label: key.to_string(),
            kind,
            required: false,
            help: String::new(),
            default_value: None,
            choices: Vec::new(),
            minimum: None,
            maximum: None,
            pattern: None,
            fields,
        }
    }

    fn test_workspace(root: &Path, sources: HashMap<String, String>) -> ProjectWorkspace {
        for (relative_path, source) in &sources {
            let absolute = root.join(relative_path);
            if let Some(parent) = absolute.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(absolute, source).unwrap();
        }
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = ProjectSessionSnapshot {
            schema_version: 1,
            id: "content-model-mutation-test".to_string(),
            project_root: canonical.clone(),
            zola_root: canonical.clone(),
            session_dir: root.join("session").to_string_lossy().to_string(),
            manifest_path: root.join("session.json").to_string_lossy().to_string(),
            opened_at_ms: 1,
            last_seen_at_ms: 1,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: canonical,
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: sources.len(),
                directory_count: 2,
            },
        };
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 64,
                max_file_bytes: 1024 * 1024,
                max_total_bytes: 4 * 1024 * 1024,
            },
        );
        for (relative_path, source) in sources {
            let (language, role) = if relative_path.ends_with(".html") {
                (TextBufferLanguage::Html, TextBufferRole::Template)
            } else if relative_path.ends_with(".md") {
                (TextBufferLanguage::Markdown, TextBufferRole::Page)
            } else {
                (TextBufferLanguage::Toml, TextBufferRole::Config)
            };
            documents.insert_loaded_file(FileBufferEntry {
                relative_path: relative_path.clone(),
                absolute_path: root.join(&relative_path).to_string_lossy().to_string(),
                language,
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
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(root).unwrap(),
        )
        .unwrap();
        ProjectWorkspace::new(
            session.clone(),
            accepted,
            documents,
            PageJsDraftStore::new(&session),
        )
        .unwrap()
    }

    #[test]
    fn toml_extra_rewrite_preserves_unknown_frontmatter_and_unmanaged_extra() {
        let source = "+++\ntitle = \"Serviciu\"\n[extra]\nkeep = \"da\"\nprice = 20\n+++\nCorp\n";
        let next = rewrite_extra_values(
            source,
            &BTreeSet::from(["price".to_string()]),
            &BTreeMap::from([("price".to_string(), serde_json::json!(35))]),
        )
        .unwrap();
        assert!(next.contains("title = \"Serviciu\""));
        assert!(next.contains("keep = \"da\""));
        assert!(next.contains("price = 35"));
        assert!(next.ends_with("Corp\n"));
    }

    #[test]
    fn yaml_extra_rewrite_preserves_comments_formatting_and_unmanaged_values() {
        let source = concat!(
            "---\n",
            "# comentariu document\n",
            "title: Serviciu # titlu păstrat\n",
            "extra:\n",
            "  # comentariu independent\n",
            "  keep: da\n",
            "  price: 20\n",
            "  nested:\n",
            "    flag: true\n",
            "---\n",
            "Corp\n"
        );
        let next = rewrite_extra_values(
            source,
            &BTreeSet::from(["price".to_string()]),
            &BTreeMap::from([("price".to_string(), serde_json::json!(35))]),
        )
        .unwrap();
        assert!(next.contains("# comentariu document"));
        assert!(next.contains("title: Serviciu # titlu păstrat"));
        assert!(next.contains("  # comentariu independent"));
        assert!(next.contains("  keep: da"));
        assert!(next.contains("  nested:\n    flag: true"));
        assert!(next.contains("  price: 35"));
        assert!(!next.contains("price: 20"));
        assert!(next.ends_with("Corp\n"));
    }

    #[test]
    fn yaml_inline_extra_is_rejected_instead_of_losing_comments() {
        let source = "---\ntitle: Serviciu\nextra: {keep: da, price: 20} # păstrează\n---\nCorp\n";
        let error = rewrite_extra_values(
            source,
            &BTreeSet::from(["price".to_string()]),
            &BTreeMap::from([("price".to_string(), serde_json::json!(35))]),
        )
        .unwrap_err();
        assert!(error.contains("forma inline"));
        assert!(error.contains("nu pierde"));
    }

    #[test]
    fn yaml_detach_removes_empty_extra_but_preserves_document_and_body() {
        let source = concat!(
            "---\n",
            "title: Serviciu\n",
            "extra:\n",
            "  # explicație păstrată\n",
            "  price: 20\n",
            "---\n",
            "Corp\n"
        );
        let next = rewrite_extra_values(
            source,
            &BTreeSet::from(["price".to_string()]),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(next.contains("title: Serviciu"));
        assert!(next.contains("# explicație păstrată"));
        assert!(!next.contains("extra:"));
        assert!(!next.contains("price:"));
        assert!(next.ends_with("Corp\n"));
    }

    #[test]
    fn detach_cleanup_removes_only_managed_keys() {
        let source = "+++\ntitle = \"Serviciu\"\n[extra]\nkeep = \"da\"\nprice = 20\n+++\n";
        let next = rewrite_extra_values(
            source,
            &BTreeSet::from(["price".to_string()]),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(next.contains("keep = \"da\""));
        assert!(!next.contains("price"));
    }

    #[test]
    fn generated_field_identity_is_stable() {
        assert_eq!(
            stable_field_id("service", "price"),
            stable_field_id("service", "price")
        );
        assert_ne!(
            stable_field_id("service", "price"),
            stable_field_id("service", "color")
        );
    }

    #[cfg(unix)]
    #[test]
    fn workspace_projection_excludes_external_metadata_symlinks() {
        use std::os::unix::fs::symlink;

        let root = fixture_root("path-safety");
        let outside = fixture_root("path-safety-outside");
        fs::create_dir_all(root.join(".panastudio")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("project.toml"), "schema_version = 1\n").unwrap();
        symlink(
            outside.join("project.toml"),
            root.join(CONTENT_MODEL_PROJECT_PATH),
        )
        .unwrap();
        symlink(&outside, root.join(CONTENT_MODEL_DIRECTORY)).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"https://example.com\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(root.join("templates/index.html"), "<main>Acasă</main>").unwrap();

        let fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
        let projection = fixture.projection();
        assert!(!projection
            .source_texts
            .contains_key(CONTENT_MODEL_PROJECT_PATH));
        assert!(!projection
            .source_texts
            .keys()
            .any(|path| path.starts_with(CONTENT_MODEL_DIRECTORY)));
        let graph = fixture.build_source_graph().unwrap();
        assert!(!graph.content_models.metadata_present);
        assert!(graph.content_models.models.is_empty());

        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn typed_values_validate_constraints_groups_repeaters_and_unknown_keys() {
        let mut amount = field("field_amount", "amount", ContentFieldKind::Number, vec![]);
        amount.required = true;
        amount.minimum = Some(1.0);
        amount.maximum = Some(100.0);
        let mut status = field("field_status", "status", ContentFieldKind::Select, vec![]);
        status.choices = vec![ContentFieldChoice {
            value: "active".to_string(),
            label: "Activ".to_string(),
        }];
        let mut caption = field("field_caption", "caption", ContentFieldKind::Text, vec![]);
        caption.required = true;
        caption.pattern = Some(r"^[A-Z]".to_string());
        let gallery = field(
            "field_gallery",
            "gallery",
            ContentFieldKind::Repeater,
            vec![caption],
        );
        let mut model = ContentModelDefinition {
            schema_version: CONTENT_MODEL_SCHEMA_VERSION,
            id: "service".to_string(),
            label: "Serviciu".to_string(),
            description: String::new(),
            fields: vec![
                amount,
                status,
                field("field_url", "url", ContentFieldKind::Url, vec![]),
                field("field_date", "date", ContentFieldKind::Date, vec![]),
                field("field_color", "color", ContentFieldKind::Color, vec![]),
                gallery,
            ],
            file: model_path("service"),
        };
        validate_model(&mut model).unwrap();
        let valid = BTreeMap::from([
            ("amount".to_string(), serde_json::json!(20)),
            ("status".to_string(), serde_json::json!("active")),
            ("url".to_string(), serde_json::json!("/servicii/")),
            ("date".to_string(), serde_json::json!("2026-08-02")),
            ("color".to_string(), serde_json::json!("#18a36f")),
            (
                "gallery".to_string(),
                serde_json::json!([{"caption": "Imagine"}]),
            ),
        ]);
        validate_page_values(&model, &valid).unwrap();

        let mut invalid_number = valid.clone();
        invalid_number.insert("amount".to_string(), serde_json::json!(101));
        assert!(validate_page_values(&model, &invalid_number)
            .unwrap_err()
            .contains("în afara limitelor"));
        let mut invalid_repeater = valid.clone();
        invalid_repeater.insert("gallery".to_string(), serde_json::json!([{}]));
        assert!(validate_page_values(&model, &invalid_repeater)
            .unwrap_err()
            .contains("obligatoriu"));
        let mut unknown = valid;
        unknown.insert("legacy".to_string(), serde_json::json!(true));
        assert!(validate_page_values(&model, &unknown)
            .unwrap_err()
            .contains("nu aparține modelului"));
    }

    #[test]
    fn nested_cleanup_preserves_siblings_across_groups_and_repeaters() {
        let mut group = serde_json::json!({
            "heading": "Titlu",
            "items": [
                {"label": "Unu", "url": "/unu"},
                {"label": "Doi", "url": "/doi"}
            ]
        });
        assert!(remove_nested_value(
            &mut group,
            &["items".to_string(), "url".to_string()]
        ));
        assert_eq!(
            group,
            serde_json::json!({
                "heading": "Titlu",
                "items": [{"label": "Unu"}, {"label": "Doi"}]
            })
        );
    }

    #[test]
    fn nested_rename_migrates_each_repeater_item_without_overwrite() {
        let mut value = serde_json::json!([
            {"label": "Unu"},
            {"label": "Doi"}
        ]);
        assert!(
            rename_nested_value(&mut value, &["label".to_string()], &["title".to_string()])
                .unwrap()
        );
        assert_eq!(
            value,
            serde_json::json!([{"title": "Unu"}, {"title": "Doi"}])
        );

        let mut collision = serde_json::json!({"label": "Unu", "title": "Existent"});
        assert!(rename_nested_value(
            &mut collision,
            &["label".to_string()],
            &["title".to_string()]
        )
        .is_err());
    }

    #[test]
    fn nested_schema_allows_repeated_keys_in_different_containers() {
        let mut model = ContentModelDefinition {
            schema_version: CONTENT_MODEL_SCHEMA_VERSION,
            id: "service".to_string(),
            label: "Serviciu".to_string(),
            description: String::new(),
            fields: vec![
                field(
                    "",
                    "hero",
                    ContentFieldKind::Group,
                    vec![field("", "title", ContentFieldKind::Text, vec![])],
                ),
                field(
                    "",
                    "card",
                    ContentFieldKind::Group,
                    vec![field("", "title", ContentFieldKind::Text, vec![])],
                ),
            ],
            file: model_path("service"),
        };
        validate_model(&mut model).unwrap();
        assert_ne!(model.fields[0].fields[0].id, model.fields[1].fields[0].id);
    }

    #[test]
    fn tera_usage_scanner_tracks_dotted_and_bracket_paths_with_boundaries() {
        assert_eq!(
            expression_offsets(
                "{{ page.extra.hero.title }} {{ page.extra.hero.titleSuffix }}",
                "page.extra.hero.title"
            ),
            vec![3]
        );
        assert_eq!(
            expression_offsets(
                "{{ page.extra[\"hero\"][\"title\"] }}",
                "page.extra[\"hero\"][\"title\"]"
            ),
            vec![3]
        );
        assert_eq!(
            replace_expression_prefix(
                "{{ page.extra.hero.title }} {{ page.extra.hero.titleSuffix }}",
                "page.extra.hero.title",
                "page.extra.hero.heading"
            ),
            "{{ page.extra.hero.heading }} {{ page.extra.hero.titleSuffix }}"
        );
    }

    #[test]
    fn dynamic_markers_follow_field_renames_and_model_replacements() {
        let source = concat!(
            "{# pana:dynamic model=service field=field_group path=hero scope=page presentation=text #}",
            "{# pana:dynamic model=service field=field_title path=hero.title scope=item presentation=text #}",
            "{# pana:dynamic model=other field=field_title path=hero.title scope=page presentation=text #}"
        );
        let renamed = rewrite_dynamic_marker_path_prefix(source, "service", "hero", "intro");
        assert!(renamed.contains("model=service field=field_group path=intro scope=page"));
        assert!(renamed.contains("model=service field=field_title path=intro.title scope=item"));
        assert!(renamed.contains("model=other field=field_title path=hero.title scope=page"));

        let replaced = rewrite_dynamic_marker_binding(
            &renamed,
            "service",
            "field_group",
            "intro",
            "premium",
            "field_content",
            "content",
        );
        assert!(replaced.contains(
            "model=premium field=field_content path=content scope=page presentation=text"
        ));
        assert!(replaced.contains("model=service field=field_title path=intro.title scope=item"));
    }

    #[test]
    fn repeater_field_rename_updates_only_its_owned_item_expression() {
        let model = ContentModelDefinition {
            schema_version: CONTENT_MODEL_SCHEMA_VERSION,
            id: "service".to_string(),
            label: "Serviciu".to_string(),
            description: String::new(),
            fields: vec![field(
                "field_gallery",
                "gallery",
                ContentFieldKind::Repeater,
                vec![field(
                    "field_caption",
                    "caption",
                    ContentFieldKind::Text,
                    vec![],
                )],
            )],
            file: model_path("service"),
        };
        let source = concat!(
            "{# pana:dynamic model=service field=field_caption path=gallery.caption scope=item presentation=text #}\n",
            "{{ item.caption }} {{ item.caption }}\n",
            "{# pana:dynamic model=other field=field_caption path=gallery.caption scope=item presentation=text #}\n",
            "{{ item.caption }}"
        );
        let renamed = rewrite_dynamic_item_binding_expressions(
            source,
            &model,
            "service",
            "gallery.caption",
            "gallery.heading",
        );
        assert!(renamed.contains("{{ item.heading }} {{ item.caption }}"));
        assert!(renamed.ends_with("{{ item.caption }}"));
    }

    #[test]
    fn catalog_tracks_list_and_single_templates_for_an_empty_assigned_section() {
        let root = fixture_root("empty-section-templates");
        fs::create_dir_all(root.join("content/services")).unwrap();
        fs::create_dir_all(root.join("templates/services")).unwrap();
        fs::create_dir_all(root.join(CONTENT_MODEL_DIRECTORY)).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"https://example.com\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<!doctype html><html><body>Acasă</body></html>",
        )
        .unwrap();
        fs::write(
            root.join("content/services/_index.md"),
            "+++\ntitle = \"Servicii\"\ntemplate = \"services/list.html\"\npage_template = \"services/single.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/services/list.html"),
            "{% for page in section.pages %}{{ page.extra.price }}{% endfor %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/services/single.html"),
            "{{ page.extra.price }}",
        )
        .unwrap();
        fs::write(
            root.join(CONTENT_MODEL_PROJECT_PATH),
            "schema_version = 1\n",
        )
        .unwrap();
        fs::write(
            root.join(CONTENT_MODEL_ASSIGNMENTS_PATH),
            "schema_version = 1\n\n[[assignments]]\nsectionPath = \"content/services/_index.md\"\nmodelId = \"service\"\n",
        )
        .unwrap();
        fs::write(
            root.join(model_path("service")),
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Serviciu\"\n\n[[fields]]\nid = \"field_price\"\nkey = \"price\"\nlabel = \"Preț\"\nkind = \"number\"\n",
        )
        .unwrap();

        let fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
        let graph = fixture.build_source_graph().unwrap();
        assert!(graph.content_models.page_bindings.is_empty());
        assert_eq!(
            graph
                .content_models
                .template_usages
                .iter()
                .map(|usage| usage.template_file.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "templates/services/list.html",
                "templates/services/single.html"
            ])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn catalog_projects_assignments_values_and_template_usages() {
        let root = fixture_root("catalog");
        fs::create_dir_all(root.join("content/services")).unwrap();
        fs::create_dir_all(root.join("content/articles")).unwrap();
        fs::create_dir_all(root.join("content/portfolio")).unwrap();
        fs::create_dir_all(root.join("templates/services")).unwrap();
        fs::create_dir_all(root.join("templates/articles")).unwrap();
        fs::create_dir_all(root.join(CONTENT_MODEL_DIRECTORY)).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"https://example.com\"\ndefault_language = \"en\"\n\n[languages.ro]\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<!doctype html><html><body>Acasă</body></html>",
        )
        .unwrap();
        fs::write(
            root.join("content/services/_index.md"),
            "+++\ntitle = \"Servicii\"\ntemplate = \"services/list.html\"\npage_template = \"services/single.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/services/consultanta.md"),
            "+++\ntitle = \"Consultanță\"\n[extra]\nprice = 120\n+++\nCorp\n",
        )
        .unwrap();
        fs::write(
            root.join("content/services/audit.md"),
            "+++\ntitle = \"Audit\"\n[extra]\nprice = 80\n+++\nCorp\n",
        )
        .unwrap();
        fs::write(
            root.join("content/services/consultanta.ro.md"),
            "+++\ntitle = \"Consultanță RO\"\n[extra]\nprice = 130\n+++\nCorp\n",
        )
        .unwrap();
        fs::write(
            root.join("content/articles/_index.md"),
            "+++\ntitle = \"Articole\"\ntemplate = \"articles/list.html\"\npage_template = \"articles/single.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/articles/anunt.md"),
            "+++\ntitle = \"Anunț\"\n[extra]\nprice = 40\n+++\nCorp\n",
        )
        .unwrap();
        fs::write(
            root.join("content/portfolio/_index.md"),
            "+++\ntitle = \"Portofoliu\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/portfolio/studiu.md"),
            "+++\ntitle = \"Studiu\"\n[extra]\nprice = \"necunoscut\"\n+++\nCorp\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/services/single.html"),
            "{# pana:dynamic model=service field=field_price path=price scope=page presentation=text #}<strong>{{ page.extra.price }}</strong>",
        )
        .unwrap();
        fs::write(
            root.join("templates/services/list.html"),
            "{% for page in section.pages %}<a href=\"{{ page.permalink }}\">{{ page.extra.price }}</a>{% endfor %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/articles/single.html"),
            "{# pana:dynamic model=service field=field_price path=price scope=page presentation=text #}<em>{{ page.extra.price }}</em>",
        )
        .unwrap();
        fs::write(
            root.join("templates/articles/list.html"),
            "{% for page in section.pages %}<a href=\"{{ page.permalink }}\">{{ page.extra.price }}</a>{% endfor %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/unrelated.html"),
            "<span>{{ page.extra.price }}</span>",
        )
        .unwrap();
        fs::write(
            root.join(CONTENT_MODEL_PROJECT_PATH),
            "schema_version = 1\n",
        )
        .unwrap();
        fs::write(
            root.join(CONTENT_MODEL_ASSIGNMENTS_PATH),
            "schema_version = 1\n\n[[assignments]]\nsectionPath = \"content/services/_index.md\"\nmodelId = \"service\"\n\n[[assignments]]\nsectionPath = \"content/articles/_index.md\"\nmodelId = \"service\"\n",
        )
        .unwrap();
        fs::write(
            root.join(model_path("service")),
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Serviciu\"\n\n[[fields]]\nid = \"field_price\"\nkey = \"price\"\nlabel = \"Preț\"\nkind = \"number\"\n",
        )
        .unwrap();
        fs::write(
            root.join(model_path("premium")),
            "schemaVersion = 1\nid = \"premium\"\nlabel = \"Serviciu premium\"\n\n[[fields]]\nid = \"field_cost\"\nkey = \"cost\"\nlabel = \"Cost\"\nkind = \"number\"\n",
        )
        .unwrap();

        let fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
        let projection = fixture.projection();
        let graph = fixture.build_source_graph().unwrap();
        assert_eq!(graph.content_models.models.len(), 2);
        assert_eq!(graph.content_models.assignments.len(), 2);
        assert_eq!(graph.content_models.page_bindings.len(), 4);
        assert_eq!(
            graph
                .content_models
                .page_bindings
                .iter()
                .find(|binding| binding.page_file == "content/services/consultanta.md")
                .unwrap()
                .values["price"],
            serde_json::json!(120)
        );
        assert!(graph
            .content_models
            .page_bindings
            .iter()
            .any(|binding| binding.page_file == "content/services/consultanta.ro.md"));
        assert_eq!(graph.content_models.template_usages.len(), 4);
        assert!(graph
            .content_models
            .template_usages
            .iter()
            .all(|usage| usage.field_id == "field_price"));

        let build_output = root.join("public");
        crate::zola_engine::with_zola_engine("test build modele de conținut", || {
            let mut site = zola_site::Site::new(&root, Path::new("zola.toml"))
                .map_err(|error| error.to_string())?;
            site.set_output_path(&build_output);
            site.load().map_err(|error| error.to_string())?;
            site.build().map_err(|error| error.to_string())
        })
        .unwrap();
        let rendered_service =
            fs::read_to_string(build_output.join("services/consultanta/index.html")).unwrap();
        assert!(rendered_service.contains("120"));
        let rendered_service_archive =
            fs::read_to_string(build_output.join("services/index.html")).unwrap();
        assert!(rendered_service_archive.contains("120"));
        assert!(rendered_service_archive.contains("80"));
        fs::remove_dir_all(&build_output).unwrap();

        let attach_existing = plan_content_model_mutation(
            &root,
            &graph,
            &projection.source_texts,
            &ContentModelMutationInput {
                operation: ContentModelMutationOperation::AttachModel {
                    model_id: "service".to_string(),
                    section_path: "content/portfolio/_index.md".to_string(),
                },
            },
        )
        .unwrap();
        assert_eq!(attach_existing.plan.affected_keys, ["price"]);
        assert_eq!(
            attach_existing.plan.affected_pages,
            ["content/portfolio/studiu.md"]
        );
        assert_eq!(attach_existing.plan.warnings.len(), 2);
        assert!(!attach_existing
            .changes
            .iter()
            .any(|change| change.relative_path == "content/portfolio/studiu.md"));

        let model_rename = plan_content_model_mutation(
            &root,
            &graph,
            &projection.source_texts,
            &ContentModelMutationInput {
                operation: ContentModelMutationOperation::RenameModel {
                    model_id: "service".to_string(),
                    new_id: "service_entry".to_string(),
                    label: "Serviciu actualizat".to_string(),
                    description: "Contract redenumit".to_string(),
                },
            },
        )
        .unwrap();
        assert!(!model_rename.plan.destructive);
        assert!(model_rename
            .deletes
            .iter()
            .any(|delete| delete.relative_path == model_path("service")));
        assert!(model_rename
            .changes
            .iter()
            .any(|change| change.relative_path == model_path("service_entry")));
        assert!(model_rename
            .changes
            .iter()
            .find(|change| change.relative_path == CONTENT_MODEL_ASSIGNMENTS_PATH)
            .unwrap()
            .contents
            .contains("modelId = \"service_entry\""));
        assert!(model_rename
            .changes
            .iter()
            .find(|change| change.relative_path == "templates/services/single.html")
            .unwrap()
            .contents
            .contains("pana:dynamic model=service_entry"));

        let rename = plan_content_model_mutation(
            &root,
            &graph,
            &projection.source_texts,
            &ContentModelMutationInput {
                operation: ContentModelMutationOperation::UpsertField {
                    model_id: "service".to_string(),
                    parent_field_id: None,
                    original_field_id: Some("field_price".to_string()),
                    field: field("field_price", "cost", ContentFieldKind::Number, vec![]),
                },
            },
        )
        .unwrap();
        assert!(!rename.plan.blocked);
        assert_eq!(
            rename.plan.affected_pages,
            [
                "content/articles/anunt.md",
                "content/services/audit.md",
                "content/services/consultanta.md",
                "content/services/consultanta.ro.md"
            ]
        );
        assert_eq!(rename.plan.affected_keys, ["cost", "price"]);
        let renamed_page = rename
            .changes
            .iter()
            .find(|change| change.relative_path == "content/services/consultanta.md")
            .unwrap();
        assert!(renamed_page.contents.contains("cost = 120"));
        assert!(!renamed_page.contents.contains("price = 120"));
        let renamed_localized_page = rename
            .changes
            .iter()
            .find(|change| change.relative_path == "content/services/consultanta.ro.md")
            .unwrap();
        assert!(renamed_localized_page.contents.contains("cost = 130"));
        let renamed_template = rename
            .changes
            .iter()
            .find(|change| change.relative_path == "templates/services/single.html")
            .unwrap();
        assert!(renamed_template.contents.contains("page.extra.cost"));
        let renamed_archive_template = rename
            .changes
            .iter()
            .find(|change| change.relative_path == "templates/services/list.html")
            .unwrap();
        assert!(renamed_archive_template
            .contents
            .contains("page.extra.cost"));

        let replacement = plan_content_model_mutation(
            &root,
            &graph,
            &projection.source_texts,
            &ContentModelMutationInput {
                operation: ContentModelMutationOperation::ReplaceModel {
                    section_path: "content/services/_index.md".to_string(),
                    from_model_id: "service".to_string(),
                    to_model_id: "premium".to_string(),
                    field_migrations: BTreeMap::from([(
                        "field_price".to_string(),
                        "field_cost".to_string(),
                    )]),
                },
            },
        )
        .unwrap();
        assert!(replacement.plan.destructive);
        assert!(!replacement.plan.blocked);
        let replacement_page = replacement
            .changes
            .iter()
            .find(|change| change.relative_path == "content/services/consultanta.md")
            .unwrap();
        assert!(replacement_page.contents.contains("cost = 120"));
        assert!(!replacement_page.contents.contains("price = 120"));
        assert_eq!(replacement.plan.affected_pages.len(), 3);
        assert!(!replacement.changes.iter().any(|change| {
            change.relative_path == "content/articles/anunt.md"
                || change.relative_path == "templates/articles/single.html"
                || change.relative_path == "templates/articles/list.html"
        }));
        let replacement_archive = replacement
            .changes
            .iter()
            .find(|change| change.relative_path == "templates/services/list.html")
            .unwrap();
        assert!(replacement_archive.contents.contains("page.extra.cost"));
        assert!(!replacement_archive.contents.contains("page.extra.price"));
        let replacement_assignments = replacement
            .changes
            .iter()
            .find(|change| change.relative_path == CONTENT_MODEL_ASSIGNMENTS_PATH)
            .unwrap();
        assert!(replacement_assignments
            .contents
            .contains("modelId = \"premium\""));

        let detach = plan_content_model_mutation(
            &root,
            &graph,
            &projection.source_texts,
            &ContentModelMutationInput {
                operation: ContentModelMutationOperation::DetachModel {
                    model_id: "service".to_string(),
                    section_path: "content/services/_index.md".to_string(),
                },
            },
        )
        .unwrap();
        assert!(detach.plan.destructive);
        assert!(detach.plan.blocked);
        assert_eq!(detach.plan.affected_keys, ["price"]);
        assert_eq!(detach.plan.template_usages.len(), 2);

        fs::create_dir_all(root.join("templates/shared")).unwrap();
        fs::write(
            root.join("templates/services/single.html"),
            "{% include \"shared/value.html\" %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/articles/single.html"),
            "{% include \"shared/value.html\" %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/shared/value.html"),
            "{# pana:dynamic model=service field=field_price path=price scope=page presentation=text #}{{ page.extra.price }}",
        )
        .unwrap();
        let shared_fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
        let shared_projection = shared_fixture.projection();
        let shared_graph = shared_fixture.build_source_graph().unwrap();
        let shared_replacement = plan_content_model_mutation(
            &root,
            &shared_graph,
            &shared_projection.source_texts,
            &ContentModelMutationInput {
                operation: ContentModelMutationOperation::ReplaceModel {
                    section_path: "content/services/_index.md".to_string(),
                    from_model_id: "service".to_string(),
                    to_model_id: "premium".to_string(),
                    field_migrations: BTreeMap::from([(
                        "field_price".to_string(),
                        "field_cost".to_string(),
                    )]),
                },
            },
        )
        .unwrap();
        assert!(shared_replacement.plan.blocked);
        assert!(shared_replacement
            .plan
            .blockers
            .iter()
            .any(|blocker| blocker.contains("șabloane comune")));

        let mut deleted_fixture =
            crate::project_model::test_support::ProjectModelTestFixture::from_integration_disk_boundary(
                &root,
            )
            .unwrap();
        deleted_fixture.delete(model_path("service"));
        let projected_after_delete = deleted_fixture.build_source_graph().unwrap();
        assert!(!projected_after_delete
            .content_models
            .models
            .iter()
            .any(|model| model.id == "service"));
        fs::write(
            root.join(model_path("alias")),
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Alias invalid\"\n",
        )
        .unwrap();
        let mismatched =
            crate::source_graph::build_source_graph_from_integration_disk_boundary(&root).unwrap();
        assert!(mismatched
            .content_models
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "content_model_file_identity_mismatch" }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mutation_stages_metadata_as_one_undoable_project_workspace_transaction() {
        let root = fixture_root("workspace");
        fs::create_dir_all(&root).unwrap();
        let sources = HashMap::from([
            (
                "zola.toml".to_string(),
                "base_url = \"https://example.com\"\n".to_string(),
            ),
            (
                "content/_index.md".to_string(),
                "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n".to_string(),
            ),
            (
                "templates/index.html".to_string(),
                "<main>Acasă</main>\n".to_string(),
            ),
        ]);
        let mut workspace = test_workspace(&root, sources);
        let projection = workspace.capture_projection_snapshot().unwrap();
        let graph =
            crate::source_graph::build_source_graph_from_workspace_projection(&root, &projection)
                .unwrap();
        let planned = plan_content_model_mutation(
            &root,
            &graph,
            &projection.source_texts,
            &ContentModelMutationInput {
                operation: ContentModelMutationOperation::CreateModel {
                    id: "service".to_string(),
                    label: "Serviciu".to_string(),
                    description: "Contract test".to_string(),
                },
            },
        )
        .unwrap();
        let (plan, receipt) = stage_content_model_mutation(&mut workspace, planned, 2).unwrap();
        assert!(receipt.changed);
        assert_eq!(receipt.history.undo_count, 1);
        assert!(plan
            .touched_files
            .contains(&CONTENT_MODEL_PROJECT_PATH.to_string()));
        assert!(workspace
            .documents
            .text_for(&model_path("service"))
            .is_some());

        let undo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace
            .undo(&undo_identity, 3)
            .expect("content model undo");
        assert!(workspace
            .documents
            .text_for(&model_path("service"))
            .is_none());
        let redo_identity = ProjectWorkspaceIdentity {
            expected_project_root: workspace.session.project_root.clone(),
            expected_session_id: workspace.runtime_session_id(),
            expected_revision: workspace.revision,
        };
        workspace
            .redo(&redo_identity, 4)
            .expect("content model redo");
        assert!(workspace
            .documents
            .text_for(&model_path("service"))
            .is_some());
        let _ = fs::remove_dir_all(root);
    }
}
