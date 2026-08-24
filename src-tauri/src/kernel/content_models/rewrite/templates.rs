use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

use crate::{kernel::content_schema::ContentModelDefinition, source_graph::SourceGraph};

use super::frontmatter::{
    remove_nested_value, rename_nested_value, required_source, rewrite_extra_values,
    structurally_empty,
};
use crate::kernel::content_models::{
    catalog::{read_extra_values, ContentModelAssignment, ContentModelCatalog, ProjectContract},
    usage_index::tera_path_expressions,
    validation::{field_keys, serialize_assignments},
    CONTENT_MODEL_ASSIGNMENTS_PATH, CONTENT_MODEL_PROJECT_PATH,
};

pub(in crate::kernel::content_models) fn ensure_metadata_contracts(
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

pub(in crate::kernel::content_models) fn stage_remove_model_values(
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
pub(in crate::kernel::content_models) fn stage_replace_model_values(
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

pub(in crate::kernel::content_models) fn stage_remove_field_values(
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
pub(in crate::kernel::content_models) fn stage_rename_field_values(
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
pub(in crate::kernel::content_models) fn stage_rename_template_references(
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
pub(in crate::kernel::content_models) fn replace_expression_prefix(
    source: &str,
    old: &str,
    new: &str,
) -> String {
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
