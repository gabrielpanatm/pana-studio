use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    kernel::{
        content_schema::{
            ContentFieldDefinition, ContentModelDefinition, CustomFieldTemplateUsage,
        },
        project_workspace::{WorkspaceResourceDelete, WorkspaceResourceMutation},
    },
    source_graph::SourceGraph,
};

use super::{
    catalog::{
        normalize_section_path, page_belongs_to_section, read_extra_values, ContentModelAssignment,
    },
    rewrite::{
        ensure_metadata_contracts, stage_remove_field_values, stage_remove_model_values,
        stage_rename_dynamic_marker_model, stage_rename_dynamic_marker_paths,
        stage_rename_field_values, stage_rename_template_references,
        stage_replace_dynamic_marker_binding, stage_replace_model_values,
    },
    usage_index::{
        model_field_paths, template_files_for_other_sections, template_files_for_section,
    },
    validation::{
        field_container_mut, field_keys, field_parent_id_by_id, field_path_by_id,
        field_subtree_ids, find_field, model_path, require_model, require_section,
        serialize_assignments, serialize_model, validate_identifier, validate_model,
        validate_page_values, validate_value_at_path,
    },
    CONTENT_MODEL_ASSIGNMENTS_PATH, CONTENT_MODEL_SCHEMA_VERSION,
};

use super::rewrite::frontmatter::{required_source, rewrite_extra_values};

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
    let catalog = &graph.content_models;
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
            let mut model = require_model(catalog, model_id)?.clone();
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
            let original = require_model(catalog, model_id)?;
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
            let model = require_model(catalog, model_id)?;
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
                    catalog,
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
            let mut model = require_model(catalog, model_id)?.clone();
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
                        catalog,
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
            let mut model = require_model(catalog, model_id)?.clone();
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
                catalog,
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
            let mut model = require_model(catalog, model_id)?.clone();
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
            let model = require_model(catalog, model_id)?;
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
            let model = require_model(catalog, model_id)?;
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
                catalog,
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
            let from_model = require_model(catalog, from_model_id)?;
            let to_model = require_model(catalog, to_model_id)?;
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
                catalog,
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
            let model = require_model(catalog, &binding.model_id)?;
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
    ensure_metadata_contracts(catalog, &mut changes)?;
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
