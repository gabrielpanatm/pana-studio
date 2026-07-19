use std::collections::HashMap;

use crate::source_graph::{
    model::{SourceDiagnosticSeverity, SourceGraphPage, SourceRelationKind},
    scan::{
        builder::SourceGraphBuilder,
        style::conventional_style_files_for_template,
        summary::{AssetSummary, DataFileSummary, TemplateSummary},
    },
    zola::{
        data_file_reference_keys, normalize_static_asset_reference,
        normalize_zola_content_reference, normalize_zola_data_file_reference,
        normalize_zola_template_reference, static_asset_reference_keys,
        zola_content_load_reference, zola_content_project_file_reference,
        zola_template_reference_keys,
    },
};
use crate::zola_theme::ZolaThemeResolver;

pub(super) fn add_template_relations(
    templates: &[TemplateSummary],
    template_node_by_name: &HashMap<String, String>,
    block_node_by_template_and_name: &HashMap<(String, String), String>,
    builder: &mut SourceGraphBuilder,
) {
    for template in templates {
        if let Some(parent) = template.extends.as_ref() {
            add_template_target_relation(
                template,
                parent,
                SourceRelationKind::Extends,
                template_node_by_name,
                builder,
            );
            add_block_override_relations(
                template,
                parent,
                block_node_by_template_and_name,
                builder,
            );
        }
        for include in &template.includes {
            add_template_target_relation(
                template,
                include,
                SourceRelationKind::Includes,
                template_node_by_name,
                builder,
            );
        }
        for import in &template.imports {
            add_template_target_relation(
                template,
                import,
                SourceRelationKind::Imports,
                template_node_by_name,
                builder,
            );
        }
    }
}

pub(super) fn add_template_content_relations(
    templates: &[TemplateSummary],
    pages: &[SourceGraphPage],
    builder: &mut SourceGraphBuilder,
) {
    let content_node_by_path = content_node_map(pages);
    for template in templates {
        for path in &template.get_pages {
            add_content_target_relation(
                template,
                path,
                SourceRelationKind::GetsPage,
                &content_node_by_path,
                builder,
            );
        }
        for path in &template.get_sections {
            add_content_target_relation(
                template,
                path,
                SourceRelationKind::GetsSection,
                &content_node_by_path,
                builder,
            );
        }
        for path in &template.internal_links {
            add_content_target_relation(
                template,
                path,
                SourceRelationKind::InternalContentLink,
                &content_node_by_path,
                builder,
            );
        }
    }
}

pub(super) fn add_template_style_relations(
    templates: &[TemplateSummary],
    style_by_file: &HashMap<String, String>,
    resolver: &ZolaThemeResolver,
    builder: &mut SourceGraphBuilder,
) {
    for template in templates {
        for style_file in conventional_style_files_for_template(resolver, template) {
            if let Some(style_node_id) = style_by_file.get(&style_file) {
                builder.add_relation(
                    template.node_id.clone(),
                    style_node_id.clone(),
                    SourceRelationKind::UsesStyle,
                    style_file,
                );
            }
        }
    }
}

pub(super) fn add_template_asset_relations(
    templates: &[TemplateSummary],
    asset_node_by_reference: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) {
    for template in templates {
        for path in &template.asset_urls {
            add_asset_target_relation(
                template,
                path,
                SourceRelationKind::AssetUrl,
                asset_node_by_reference,
                builder,
            );
        }
        for path in &template.asset_hashes {
            add_asset_target_relation(
                template,
                path,
                SourceRelationKind::AssetHash,
                asset_node_by_reference,
                builder,
            );
        }
        for path in &template.image_metadata {
            add_asset_target_relation(
                template,
                path,
                SourceRelationKind::ImageMetadata,
                asset_node_by_reference,
                builder,
            );
        }
        for path in &template.image_resizes {
            add_asset_target_relation(
                template,
                path,
                SourceRelationKind::ImageResize,
                asset_node_by_reference,
                builder,
            );
        }
    }
}

pub(super) fn add_template_load_data_relations(
    templates: &[TemplateSummary],
    asset_node_by_reference: &HashMap<String, String>,
    data_file_node_by_reference: &HashMap<String, String>,
    content_node_by_path: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) {
    for template in templates {
        for path in &template.data_loads {
            add_load_data_target_relation(
                template,
                path,
                asset_node_by_reference,
                data_file_node_by_reference,
                content_node_by_path,
                builder,
            );
        }
    }
}

pub(super) fn content_node_map(pages: &[SourceGraphPage]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for page in pages {
        if let Some(content_path) = zola_content_project_file_reference(&page.file) {
            map.entry(content_path)
                .or_insert_with(|| page.content_node_id.clone());
        }
    }
    map
}

pub(super) fn asset_reference_map(assets: &[AssetSummary]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for asset in assets {
        for key in static_asset_reference_keys(&asset.logical_path) {
            map.entry(key).or_insert_with(|| asset.node_id.clone());
        }
    }
    map
}

pub(super) fn data_file_reference_map(data_files: &[DataFileSummary]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for data_file in data_files {
        for key in data_file_reference_keys(&data_file.logical_path) {
            map.entry(key).or_insert_with(|| data_file.node_id.clone());
        }
    }
    map
}

pub(super) fn template_node_map(templates: &[TemplateSummary]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for template in templates {
        for key in zola_template_reference_keys(&template.name) {
            map.entry(key).or_insert_with(|| template.node_id.clone());
        }
    }
    map
}

pub(super) fn template_summary_map(
    templates: &[TemplateSummary],
) -> HashMap<String, TemplateSummary> {
    let mut map = HashMap::new();
    for template in templates {
        for key in zola_template_reference_keys(&template.name) {
            map.entry(key).or_insert_with(|| template.clone());
        }
    }
    map
}

pub(super) fn block_node_map(templates: &[TemplateSummary]) -> HashMap<(String, String), String> {
    let mut map = HashMap::new();
    for template in templates {
        for (block_name, node_id) in &template.blocks {
            map.entry((template.name.clone(), block_name.clone()))
                .or_insert_with(|| node_id.clone());
        }
    }
    map
}

fn add_content_target_relation(
    template: &TemplateSummary,
    target: &str,
    kind: SourceRelationKind,
    content_node_by_path: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) {
    let normalized = normalize_zola_content_reference(target);
    if let Some(target_node_id) = content_node_by_path.get(&normalized) {
        builder.add_relation(
            template.node_id.clone(),
            target_node_id.clone(),
            kind,
            normalized,
        );
    } else {
        builder.add_diagnostic(
            SourceDiagnosticSeverity::Warning,
            format!("Conținut Zola referențiat dar negăsit: {}", target),
            Some(template.file.clone()),
            None,
        );
    }
}

fn add_template_target_relation(
    template: &TemplateSummary,
    target: &str,
    kind: SourceRelationKind,
    template_node_by_name: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) {
    let normalized = normalize_zola_template_reference(target);
    if let Some(target_node_id) = template_node_by_name.get(&normalized) {
        builder.add_relation(
            template.node_id.clone(),
            target_node_id.clone(),
            kind,
            normalized,
        );
    } else {
        builder.add_diagnostic(
            SourceDiagnosticSeverity::Warning,
            format!("Template referențiat dar negăsit: {}", target),
            Some(template.file.clone()),
            None,
        );
    }
}

fn add_asset_target_relation(
    template: &TemplateSummary,
    target: &str,
    kind: SourceRelationKind,
    asset_node_by_reference: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) {
    let normalized = normalize_static_asset_reference(target);
    if let Some(target_node_id) = asset_node_by_reference.get(&normalized) {
        if normalized
            .split(['?', '#'])
            .next()
            .is_some_and(|path| path.to_ascii_lowercase().ends_with(".js"))
        {
            builder.add_relation(
                template.node_id.clone(),
                target_node_id.clone(),
                SourceRelationKind::UsesScript,
                normalized.clone(),
            );
        }
        builder.add_relation(
            template.node_id.clone(),
            target_node_id.clone(),
            kind,
            normalized,
        );
    }
}

fn add_load_data_target_relation(
    template: &TemplateSummary,
    target: &str,
    asset_node_by_reference: &HashMap<String, String>,
    data_file_node_by_reference: &HashMap<String, String>,
    content_node_by_path: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) {
    let normalized_data = normalize_zola_data_file_reference(target);
    if let Some(target_node_id) = data_file_node_by_reference.get(&normalized_data) {
        builder.add_relation(
            template.node_id.clone(),
            target_node_id.clone(),
            SourceRelationKind::DataFileLoad,
            normalized_data,
        );
        return;
    }

    if let Some(normalized_content) = zola_content_load_reference(target) {
        if let Some(target_node_id) = content_node_by_path.get(&normalized_content) {
            builder.add_relation(
                template.node_id.clone(),
                target_node_id.clone(),
                SourceRelationKind::ContentDataLoad,
                normalized_content,
            );
            return;
        }
    }

    let normalized_asset = normalize_static_asset_reference(target);
    if let Some(target_node_id) = asset_node_by_reference.get(&normalized_asset) {
        builder.add_relation(
            template.node_id.clone(),
            target_node_id.clone(),
            SourceRelationKind::DataLoad,
            normalized_asset,
        );
    } else {
        builder.add_diagnostic(
            SourceDiagnosticSeverity::Warning,
            format!(
                "Fișier local Zola referențiat de load_data dar negăsit: {}",
                target
            ),
            Some(template.file.clone()),
            None,
        );
    }
}

fn add_block_override_relations(
    template: &TemplateSummary,
    parent: &str,
    block_node_by_template_and_name: &HashMap<(String, String), String>,
    builder: &mut SourceGraphBuilder,
) {
    let parent_name = normalize_zola_template_reference(parent);
    for (block_name, child_node_id) in &template.blocks {
        let key = (parent_name.clone(), block_name.clone());
        if let Some(parent_node_id) = block_node_by_template_and_name.get(&key) {
            builder.add_relation(
                child_node_id.clone(),
                parent_node_id.clone(),
                SourceRelationKind::OverridesBlock,
                block_name.clone(),
            );
        }
    }
}
