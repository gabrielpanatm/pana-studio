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
    validation::{field_item_path_by_id, field_keys, serialize_assignments},
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
pub(in crate::kernel::content_models) fn stage_rename_dynamic_marker_paths(
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

pub(in crate::kernel::content_models) fn rewrite_dynamic_item_binding_expressions(
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

pub(in crate::kernel::content_models) fn stage_rename_dynamic_marker_model(
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
pub(in crate::kernel::content_models) fn stage_replace_dynamic_marker_binding(
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

pub(in crate::kernel::content_models) fn rewrite_dynamic_marker_path_prefix(
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
pub(in crate::kernel::content_models) fn rewrite_dynamic_marker_binding(
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
