use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{
    kernel::content_schema::{ContentModelDefinition, CustomFieldTemplateUsage},
    source_graph::{zola::zola_frontmatter_range, SourceGraph},
};

use super::{
    usage_index::build_template_usages,
    validation::{
        collect_missing_required_fields, field_keys, model_path, validate_model,
        validate_value_at_path,
    },
    CONTENT_MODEL_ASSIGNMENTS_PATH, CONTENT_MODEL_DIRECTORY, CONTENT_MODEL_PROJECT_PATH,
    CONTENT_MODEL_SCHEMA_VERSION,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentModelAssignment {
    pub section_path: String,
    pub model_id: String,
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
pub(super) struct ProjectContract {
    pub(super) schema_version: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct AssignmentContract {
    pub(super) schema_version: u32,
    #[serde(default)]
    pub(super) assignments: Vec<ContentModelAssignment>,
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

pub(super) fn normalize_section_path(path: &str) -> String {
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

pub(super) fn page_belongs_to_section(page_file: &str, section_path: &str) -> bool {
    let directory = section_path.trim_end_matches("_index.md");
    page_file != section_path && page_file.starts_with(directory) && page_file.ends_with(".md")
}

pub(super) fn read_extra_values(
    source: &str,
) -> Result<BTreeMap<String, serde_json::Value>, String> {
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
