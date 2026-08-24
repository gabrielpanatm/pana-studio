use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

use crate::{
    kernel::content_schema::{
        ContentFieldDefinition, ContentModelDefinition, CustomFieldTemplateUsage,
    },
    source_graph::SourceGraph,
};

use super::catalog::{ContentModelAssignment, ContentModelCatalog, ContentModelPageBinding};

pub(super) fn build_template_usages(
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    models: &[ContentModelDefinition],
    assignments: &[ContentModelAssignment],
    page_bindings: &[ContentModelPageBinding],
) -> Vec<CustomFieldTemplateUsage> {
    let relevant_templates = models
        .iter()
        .map(|model| {
            (
                model.id.as_str(),
                template_files_for_model(graph, page_bindings, assignments, &model.id),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut usages = graph
        .templates
        .iter()
        .flat_map(|template| {
            project_template_usages(source_texts, template, models, &relevant_templates, models)
        })
        .collect::<Vec<_>>();
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

fn project_template_usages(
    source_texts: &HashMap<String, String>,
    template: &crate::source_graph::model::SourceGraphTemplate,
    models: &[ContentModelDefinition],
    relevant_templates: &HashMap<&str, Option<BTreeSet<String>>>,
    dynamic_widget_models: &[ContentModelDefinition],
) -> Vec<CustomFieldTemplateUsage> {
    let Some(source) = source_texts.get(&template.file) else {
        return Vec::new();
    };
    let mut usages = Vec::new();
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
                    for absolute in expression_offsets(source, &expression) {
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
    usages.extend(
        crate::kernel::dynamic_widgets::project_dynamic_field_usages(
            source,
            &template.file,
            dynamic_widget_models,
        ),
    );
    usages
}

pub(crate) fn upsert_content_model_template_usages(
    source_texts: &HashMap<String, String>,
    graph: &SourceGraph,
    catalog: &mut ContentModelCatalog,
    template_file: &str,
) {
    let relevant_templates = catalog
        .models
        .iter()
        .map(|model| {
            (
                model.id.as_str(),
                template_files_for_model(
                    graph,
                    &catalog.page_bindings,
                    &catalog.assignments,
                    &model.id,
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    catalog
        .template_usages
        .retain(|usage| usage.template_file != template_file);
    if let Some(template) = graph
        .templates
        .iter()
        .find(|template| template.file == template_file)
    {
        catalog.template_usages.extend(project_template_usages(
            source_texts,
            template,
            &catalog.models,
            &relevant_templates,
            &catalog.models,
        ));
    }
    catalog.template_usages.sort_by(|left, right| {
        (left.template_file.as_str(), left.offset)
            .cmp(&(right.template_file.as_str(), right.offset))
    });
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

pub(super) fn template_files_for_section(
    graph: &SourceGraph,
    page_bindings: &[ContentModelPageBinding],
    model_id: &str,
    section_path: &str,
) -> Option<BTreeSet<String>> {
    template_files_for_binding_scope(graph, page_bindings, model_id, Some(section_path))
}

pub(super) fn template_files_for_other_sections(
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

pub(super) fn model_field_paths(fields: &[ContentFieldDefinition]) -> BTreeSet<String> {
    flatten_field_paths(fields)
        .into_iter()
        .map(|(_, path)| path)
        .collect()
}

pub(in crate::kernel::content_models) fn tera_path_expressions(
    scope: &str,
    path: &str,
) -> Vec<String> {
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

pub(in crate::kernel::content_models) fn expression_offsets(
    source: &str,
    expression: &str,
) -> Vec<usize> {
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
