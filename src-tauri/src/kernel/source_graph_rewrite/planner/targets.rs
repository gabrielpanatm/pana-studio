use std::collections::HashMap;

use crate::source_graph::{
    model::{SourceGraph, SourceOrigin},
    zola::{
        local_static_asset_project_file_reference, local_zola_data_project_file_reference,
        local_zola_template_project_file_reference, normalize_zola_template_reference,
        zola_content_project_file_reference,
    },
};

#[derive(Clone)]
pub(super) struct TemplateRewriteTarget {
    pub relative_path: String,
    pub old_name: String,
    pub new_name: String,
}

pub(super) fn rewrite_targets_for_entry(
    graph: &SourceGraph,
    source_relative_path: &str,
    destination_relative_path: &str,
) -> Result<HashMap<String, TemplateRewriteTarget>, String> {
    let mut targets = HashMap::new();
    for template in &graph.templates {
        if !path_is_inside_entry(&template.file, source_relative_path) {
            continue;
        }
        if template.origin != SourceOrigin::Local {
            return Err(format!(
                "SourceGraphRewrite blocat pentru {}: prima felie rescrie doar template-uri locale.",
                template.file
            ));
        }
        let destination_file = replace_entry_prefix(
            &template.file,
            source_relative_path,
            destination_relative_path,
        )?;
        let new_name =
            local_zola_template_project_file_reference(&destination_file).ok_or_else(|| {
                format!(
                    "SourceGraphRewrite blocat pentru {}: destinația {} nu rămâne sub templates/.",
                    template.file, destination_file
                )
            })?;
        let old_name = normalize_zola_template_reference(&template.name);
        if old_name == new_name {
            continue;
        }
        targets.insert(
            template.node_id.clone(),
            TemplateRewriteTarget {
                relative_path: template.file.clone(),
                old_name,
                new_name,
            },
        );
    }
    for page in &graph.pages {
        if !path_is_inside_entry(&page.file, source_relative_path) {
            continue;
        }
        let destination_file =
            replace_entry_prefix(&page.file, source_relative_path, destination_relative_path)?;
        let new_name = zola_content_project_file_reference(&destination_file).ok_or_else(|| {
            format!(
                "SourceGraphRewrite blocat pentru {}: destinația {} nu rămâne sub content/.",
                page.file, destination_file
            )
        })?;
        let old_name = zola_content_project_file_reference(&page.file).ok_or_else(|| {
            format!(
                "SourceGraphRewrite blocat pentru {}: pagina Source Graph nu este sub content/.",
                page.file
            )
        })?;
        if old_name == new_name {
            continue;
        }
        targets.insert(
            page.content_node_id.clone(),
            TemplateRewriteTarget {
                relative_path: page.file.clone(),
                old_name,
                new_name,
            },
        );
    }
    for asset in &graph.assets {
        if !path_is_inside_entry(&asset.file, source_relative_path) {
            continue;
        }
        if asset.origin != SourceOrigin::Local {
            return Err(format!(
                "SourceGraphRewrite blocat pentru {}: prima felie rescrie doar asset-uri locale.",
                asset.file
            ));
        }
        let destination_file =
            replace_entry_prefix(&asset.file, source_relative_path, destination_relative_path)?;
        let new_name =
            local_static_asset_project_file_reference(&destination_file).ok_or_else(|| {
                format!(
                    "SourceGraphRewrite blocat pentru {}: destinația {} nu rămâne sub static/.",
                    asset.file, destination_file
                )
            })?;
        let old_name = local_static_asset_project_file_reference(&asset.file).ok_or_else(|| {
            format!(
                "SourceGraphRewrite blocat pentru {}: asset-ul Source Graph nu este sub static/.",
                asset.file
            )
        })?;
        if old_name == new_name {
            continue;
        }
        targets.insert(
            asset.node_id.clone(),
            TemplateRewriteTarget {
                relative_path: asset.file.clone(),
                old_name,
                new_name,
            },
        );
    }
    for data_file in &graph.data_files {
        if !path_is_inside_entry(&data_file.file, source_relative_path) {
            continue;
        }
        if data_file.origin != SourceOrigin::Local {
            return Err(format!(
                "SourceGraphRewrite blocat pentru {}: prima felie rescrie doar fișiere data locale.",
                data_file.file
            ));
        }
        let destination_file = replace_entry_prefix(
            &data_file.file,
            source_relative_path,
            destination_relative_path,
        )?;
        let new_name =
            local_zola_data_project_file_reference(&destination_file).ok_or_else(|| {
                format!(
                    "SourceGraphRewrite blocat pentru {}: destinația {} nu rămâne sub date/.",
                    data_file.file, destination_file
                )
            })?;
        let old_name = local_zola_data_project_file_reference(&data_file.file).ok_or_else(|| {
            format!(
                "SourceGraphRewrite blocat pentru {}: fișierul data Source Graph nu este sub date/.",
                data_file.file
            )
        })?;
        if old_name == new_name {
            continue;
        }
        targets.insert(
            data_file.node_id.clone(),
            TemplateRewriteTarget {
                relative_path: data_file.file.clone(),
                old_name,
                new_name,
            },
        );
    }
    Ok(targets)
}

fn path_is_inside_entry(file: &str, source_relative_path: &str) -> bool {
    file == source_relative_path
        || file
            .strip_prefix(source_relative_path)
            .map(|rest| rest.starts_with('/'))
            .unwrap_or(false)
}

fn replace_entry_prefix(
    file: &str,
    source_relative_path: &str,
    destination_relative_path: &str,
) -> Result<String, String> {
    if file == source_relative_path {
        return Ok(destination_relative_path.to_string());
    }
    let rest = file
        .strip_prefix(source_relative_path)
        .filter(|rest| rest.starts_with('/'))
        .ok_or_else(|| {
            format!(
                "SourceGraphRewrite blocat: {} nu este în interiorul {}.",
                file, source_relative_path
            )
        })?;
    Ok(format!("{destination_relative_path}{rest}"))
}
