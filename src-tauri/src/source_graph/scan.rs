use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};
use zola_config::Config;

mod asset;
mod builder;
mod data_file;
mod files;
mod incremental;
mod page;
pub(crate) mod ranges;
mod relations;
mod structured_document;
mod style;
mod summary;
mod template;

use crate::{
    kernel::project_workspace::WorkspaceProjectionSnapshot,
    localization::LocalizedDiagnostic,
    project::zola_project_root,
    source_graph::{
        identity::initialize_runtime_source_node_ids,
        model::{
            SourceDiagnosticSeverity, SourceGraph, SourceGraphAsset, SourceGraphDataFile,
            SourceGraphScript, SourceGraphStyle, SourceGraphTemplate, SourcePageKind,
        },
        scan::{
            asset::scan_asset,
            builder::SourceGraphBuilder,
            data_file::{scan_data_file, ZOLA_DATA_FILE_EXTENSIONS},
            files::{
                apply_virtual_file_projection, relative_project_path,
                require_safe_deleted_source_paths, require_safe_draft_source_paths,
                require_safe_scan_root,
            },
            page::scan_content_page,
            relations::{
                add_style_asset_relations, add_template_asset_relations,
                add_template_content_relations, add_template_load_data_relations,
                add_template_relations, add_template_script_relations,
                add_template_style_relations, asset_reference_map, block_node_map,
                content_node_map, data_file_reference_map, template_node_map, template_summary_map,
            },
            structured_document::scan_structured_toml_document,
            style::{scan_style, style_scope_for_file},
            summary::TemplateSummary,
            template::scan_template,
        },
        zola::{parse_zola_content_frontmatter, zola_content_page_kind},
    },
    zola_theme::{active_theme_from_source, ZolaThemeResolver},
};

pub(crate) use incremental::{
    rebuild_local_template_graph, SourceGraphIncrementalFallback,
    SourceGraphIncrementalTemplateReport,
};

pub fn build_source_graph_from_workspace_projection(
    project_root: &Path,
    projection: &WorkspaceProjectionSnapshot,
) -> Result<SourceGraph, String> {
    build_source_graph_internal(project_root, projection, true)
}

/// Audit needs a best-effort graph: project defects remain graph diagnostics
/// and must not abort the complete provider run. Projection identity and path
/// safety errors still return `Err` before any receipt can be produced.
pub(crate) fn build_source_graph_for_audit_from_workspace_projection(
    project_root: &Path,
    projection: &WorkspaceProjectionSnapshot,
) -> Result<SourceGraph, String> {
    build_source_graph_internal(project_root, projection, false)
}

fn build_source_graph_internal(
    project_root: &Path,
    projection: &WorkspaceProjectionSnapshot,
    fail_on_source_error: bool,
) -> Result<SourceGraph, String> {
    let draft_sources = &projection.source_texts;
    let deleted_sources = &projection.deleted_sources;
    let root = project_root
        .canonicalize()
        .map_err(|error| format!("Nu am putut rezolva folderul proiectului: {}", error))?;
    if root != Path::new(&projection.project_root) {
        return Err(format!(
            "Source Graph a refuzat proiecția pentru alt root: {} != {}.",
            root.display(),
            projection.project_root
        ));
    }
    let zola_root = zola_project_root(&root);
    let _ = require_safe_scan_root(&zola_root)?;
    require_safe_draft_source_paths(draft_sources)?;
    require_safe_deleted_source_paths(deleted_sources)?;
    let projected_config = ["zola.toml", "config.toml"]
        .iter()
        .find_map(|path| draft_sources.get(*path));
    let route_config_source = projected_config.cloned();
    let route_config = route_config_source
        .as_deref()
        .and_then(|source| Config::parse(source).ok());
    let theme_resolver = ZolaThemeResolver::new(
        projected_config.and_then(|source| active_theme_from_source(source)),
    );
    let active_theme = theme_resolver.active_theme().map(str::to_string);
    let mut builder = SourceGraphBuilder::new(&root, &zola_root, active_theme.clone());
    // Generated output is outside the immutable editable workspace.
    let output_root: Option<PathBuf> = None;
    let is_zola = projected_config.is_some();
    if !is_zola {
        builder.add_diagnostic(
            SourceDiagnosticSeverity::Warning,
            LocalizedDiagnostic::new("source-graph-not-zola-project"),
            None,
            None,
        );
        let mut graph = builder.finish(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        initialize_runtime_source_node_ids(&mut graph, &projection.runtime_session_id)?;
        return Ok(graph);
    }

    let mut content_files = Vec::new();
    let mut template_files = Vec::new();
    let mut style_files = Vec::new();
    let mut asset_files = Vec::new();
    let mut data_file_paths = Vec::new();

    let mut theme_template_files = Vec::new();
    let mut theme_style_files = Vec::new();
    let mut theme_asset_files = Vec::new();
    apply_virtual_file_projection(
        &root,
        &zola_root.join("content"),
        Some(&["md"]),
        draft_sources,
        deleted_sources,
        &mut content_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("templates"),
        Some(&["html", "md"]),
        draft_sources,
        deleted_sources,
        &mut template_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("sass"),
        Some(&["scss"]),
        draft_sources,
        deleted_sources,
        &mut style_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("static"),
        Some(&["css", "scss"]),
        draft_sources,
        deleted_sources,
        &mut style_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("static"),
        None,
        draft_sources,
        deleted_sources,
        &mut asset_files,
    )?;
    apply_virtual_file_projection(
        &root,
        &zola_root.join("date"),
        Some(ZOLA_DATA_FILE_EXTENSIONS),
        draft_sources,
        deleted_sources,
        &mut data_file_paths,
    )?;
    if let Some(theme) = active_theme.as_ref() {
        let theme_root = zola_root.join("themes").join(theme);
        apply_virtual_file_projection(
            &root,
            &theme_root.join("templates"),
            Some(&["html", "md"]),
            draft_sources,
            deleted_sources,
            &mut theme_template_files,
        )?;
        apply_virtual_file_projection(
            &root,
            &theme_root.join("sass"),
            Some(&["scss"]),
            draft_sources,
            deleted_sources,
            &mut theme_style_files,
        )?;
        apply_virtual_file_projection(
            &root,
            &theme_root.join("static"),
            Some(&["css", "scss"]),
            draft_sources,
            deleted_sources,
            &mut theme_style_files,
        )?;
        apply_virtual_file_projection(
            &root,
            &theme_root.join("static"),
            None,
            draft_sources,
            deleted_sources,
            &mut theme_asset_files,
        )?;
    }
    add_workspace_manifest_only_paths(
        &root,
        active_theme.as_deref(),
        projection,
        &mut asset_files,
        &mut theme_asset_files,
        &mut data_file_paths,
    )?;

    let mut templates = Vec::new();
    for path in template_files {
        templates.push(scan_template(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Local,
            None,
            draft_sources,
            &mut builder,
        ));
    }
    for path in theme_template_files {
        templates.push(scan_template(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Theme,
            active_theme.clone(),
            draft_sources,
            &mut builder,
        ));
    }

    let data_context = crate::source_graph::zola::ZolaDataResolutionContext {
        project_root: &root,
        zola_root: &zola_root,
        active_theme: active_theme.as_deref(),
        output_root: output_root.as_deref(),
        projected_sources: draft_sources,
        deleted_sources,
        exact_workspace_projection: true,
    };
    let mut resolved_data_files = BTreeMap::new();
    for path in data_file_paths {
        match crate::source_graph::zola::conventional_zola_data_file(&data_context, path) {
            Ok(candidate) => {
                resolved_data_files.insert(candidate.file.clone(), candidate);
            }
            Err(error) => builder.add_diagnostic(
                SourceDiagnosticSeverity::Warning,
                LocalizedDiagnostic::new("source-graph-conventional-data-invalid")
                    .with_argument("details", error),
                None,
                None,
            ),
        }
    }
    for template in &templates {
        for load_path in &template.data_loads {
            match crate::source_graph::zola::resolve_zola_load_data_file(&data_context, load_path) {
                Ok(Some(candidate)) => {
                    if let Some(existing) = resolved_data_files.get_mut(&candidate.file) {
                        for reference in candidate.load_paths {
                            if !existing.load_paths.contains(&reference) {
                                existing.load_paths.push(reference);
                            }
                        }
                    } else {
                        resolved_data_files.insert(candidate.file.clone(), candidate);
                    }
                }
                Ok(None) => builder.add_diagnostic(
                    SourceDiagnosticSeverity::Warning,
                    LocalizedDiagnostic::new("source-graph-load-data-missing")
                        .with_argument("path", load_path.clone()),
                    Some(template.file.clone()),
                    None,
                ),
                Err(error) => builder.add_diagnostic(
                    SourceDiagnosticSeverity::Warning,
                    LocalizedDiagnostic::new("source-graph-load-data-unresolved")
                        .with_argument("path", load_path.clone())
                        .with_argument("details", error),
                    Some(template.file.clone()),
                    None,
                ),
            }
        }
    }
    for candidate in resolved_data_files.values_mut() {
        candidate.load_paths.sort();
        candidate.load_paths.dedup();
    }
    let promoted_data_paths = resolved_data_files
        .values()
        .map(|candidate| candidate.path.clone())
        .collect::<HashSet<_>>();
    asset_files.retain(|path| !promoted_data_paths.contains(path));
    theme_asset_files.retain(|path| !promoted_data_paths.contains(path));

    let mut styles = Vec::new();
    for path in style_files {
        styles.push(scan_style(
            &root,
            &path,
            crate::source_graph::model::SourceOrigin::Local,
            None,
            draft_sources,
            &mut builder,
        ));
    }
    for path in theme_style_files {
        styles.push(scan_style(
            &root,
            &path,
            crate::source_graph::model::SourceOrigin::Theme,
            active_theme.clone(),
            draft_sources,
            &mut builder,
        ));
    }
    let mut assets = Vec::new();
    for path in asset_files {
        assets.push(scan_asset(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Local,
            None,
            &mut builder,
        ));
    }
    for path in theme_asset_files {
        assets.push(scan_asset(
            &root,
            &zola_root,
            &path,
            crate::source_graph::model::SourceOrigin::Theme,
            active_theme.clone(),
            &mut builder,
        ));
    }
    let mut data_files = Vec::new();
    for candidate in resolved_data_files.into_values() {
        data_files.push(scan_data_file(candidate, draft_sources, &mut builder));
    }

    let template_node_by_name = template_node_map(&templates);
    let template_by_name = template_summary_map(&templates);
    let block_node_by_template_and_name = block_node_map(&templates);
    let style_by_file: HashMap<String, String> = styles
        .iter()
        .map(|style| (style.file.clone(), style.node_id.clone()))
        .collect();
    let asset_node_by_file: HashMap<String, String> = assets
        .iter()
        .map(|asset| (asset.file.clone(), asset.node_id.clone()))
        .collect();
    let mut asset_node_by_reference = asset_reference_map(&assets);
    for data_file in &data_files {
        if !matches!(
            data_file.location,
            crate::source_graph::model::SourceDataLocation::Static
                | crate::source_graph::model::SourceDataLocation::Theme
        ) {
            continue;
        }
        for reference in data_file
            .load_paths
            .iter()
            .chain(std::iter::once(&data_file.logical_path))
        {
            let normalized = crate::source_graph::zola::normalize_static_asset_reference(reference);
            asset_node_by_reference
                .entry(normalized)
                .or_insert_with(|| data_file.node_id.clone());
        }
    }
    let data_file_node_by_reference = data_file_reference_map(&data_files);
    add_template_relations(
        &templates,
        &template_node_by_name,
        &block_node_by_template_and_name,
        &mut builder,
    );
    add_template_style_relations(&templates, &style_by_file, &theme_resolver, &mut builder);
    add_template_script_relations(
        &templates,
        &asset_node_by_file,
        &theme_resolver,
        &mut builder,
    );
    add_template_asset_relations(&templates, &asset_node_by_reference, &mut builder);
    add_style_asset_relations(&styles, &asset_node_by_reference, &mut builder);

    let asset_reference_eligible = templates
        .iter()
        .map(|template| template.asset_reference_eligible)
        .chain(styles.iter().map(|style| style.asset_reference_eligible))
        .sum::<usize>();
    let asset_reference_unanalysable = templates
        .iter()
        .map(|template| template.asset_reference_unanalysable)
        .chain(
            styles
                .iter()
                .map(|style| style.asset_reference_unanalysable),
        )
        .sum::<usize>();

    let section_page_templates =
        collect_section_page_templates(&root, &zola_root, &content_files, draft_sources);
    let mut pages = Vec::new();
    for path in content_files {
        let inherited_page_template =
            inherited_page_template(&zola_root, &path, &section_page_templates);
        pages.push(scan_content_page(
            &root,
            &zola_root,
            &path,
            inherited_page_template,
            &template_node_by_name,
            &template_by_name,
            &style_by_file,
            &theme_resolver,
            route_config.as_ref(),
            draft_sources,
            &mut builder,
        ));
    }
    let content_node_by_path = content_node_map(&pages);
    add_template_load_data_relations(
        &templates,
        &asset_node_by_reference,
        &data_file_node_by_reference,
        &content_node_by_path,
        &mut builder,
    );
    add_template_content_relations(&templates, &pages, &mut builder);

    let graph_templates = templates
        .into_iter()
        .map(graph_template_from_summary)
        .collect();
    let graph_styles = styles
        .into_iter()
        .map(|style| SourceGraphStyle {
            id: style.node_id.clone(),
            file: style.file.clone(),
            origin: style.origin,
            theme_name: style.theme_name,
            scope: style_scope_for_file(&style.file),
            node_id: style.node_id,
        })
        .collect();
    let graph_assets = assets
        .iter()
        .map(|asset| SourceGraphAsset {
            id: asset.node_id.clone(),
            file: asset.file.clone(),
            origin: asset.origin.clone(),
            theme_name: asset.theme_name.clone(),
            logical_path: asset.logical_path.clone(),
            node_id: asset.node_id.clone(),
        })
        .collect::<Vec<_>>();
    let graph_scripts = assets
        .into_iter()
        .filter(|asset| asset.is_script)
        .map(|script| SourceGraphScript {
            id: script.node_id.clone(),
            file: script.file,
            origin: script.origin,
            theme_name: script.theme_name,
            logical_path: script.logical_path,
            node_id: script.node_id,
        })
        .collect();
    let graph_data_files = data_files
        .into_iter()
        .map(|data_file| SourceGraphDataFile {
            id: data_file.node_id.clone(),
            file: data_file.file.clone(),
            origin: data_file.origin,
            theme_name: data_file.theme_name,
            logical_path: data_file.logical_path,
            load_paths: data_file.load_paths,
            location: data_file.location,
            node_id: data_file.node_id,
            format: data_file.format,
            parse_error: data_file.parse_error,
            nodes: data_file.nodes,
            capabilities: data_file.capabilities,
        })
        .collect();
    let mut structured_documents = Vec::new();
    if let Some(config_path) = ["zola.toml", "config.toml"]
        .iter()
        .map(|name| root.join(name))
        .find(|path| draft_sources.contains_key(&relative_project_path(&root, path)))
    {
        structured_documents.push(scan_structured_toml_document(
            &root,
            &config_path,
            crate::source_graph::model::SourceStructuredDocumentKind::ZolaConfig,
            draft_sources,
            &mut builder,
        ));
    }
    if let Some(theme) = active_theme.as_ref() {
        let theme_config = zola_root.join("themes").join(theme).join("theme.toml");
        if draft_sources.contains_key(&relative_project_path(&root, &theme_config)) {
            structured_documents.push(scan_structured_toml_document(
                &root,
                &theme_config,
                crate::source_graph::model::SourceStructuredDocumentKind::ThemeConfig,
                draft_sources,
                &mut builder,
            ));
        }
    }

    let mut graph = builder.finish(
        pages,
        graph_templates,
        graph_styles,
        graph_scripts,
        graph_assets,
        graph_data_files,
        structured_documents,
    );
    graph.asset_reference_coverage = crate::source_graph::model::SourceAssetReferenceCoverage {
        eligible: asset_reference_eligible,
        analyzed: asset_reference_eligible.saturating_sub(asset_reference_unanalysable),
        unanalysable: asset_reference_unanalysable,
    };
    initialize_runtime_source_node_ids(&mut graph, &projection.runtime_session_id)?;
    graph.component_graph = crate::source_graph::component_graph::build_component_graph(&graph);
    graph.block_graph = crate::blocks::graph::build_block_graph(&graph);
    graph.content_models =
        crate::kernel::content_models::build_content_model_catalog_from_workspace_projection(
            &root,
            draft_sources,
            deleted_sources,
            &graph,
        );
    graph.listing_items =
        crate::kernel::listing_items::build_listing_item_catalog_from_workspace_projection(
            &root,
            draft_sources,
            deleted_sources,
            &graph,
        );
    graph.dynamic_widget_graph =
        crate::kernel::dynamic_widgets::build_dynamic_widget_graph_from_workspace_projection(
            &root,
            draft_sources,
            deleted_sources,
            &graph,
        );
    graph.markdown_projections = crate::source_graph::markdown::build_markdown_projections(&graph);
    if fail_on_source_error {
        let read_error = graph
            .diagnostics
            .iter()
            .find(|diagnostic| matches!(diagnostic.severity, SourceDiagnosticSeverity::Error));
        if let Some(read_error) = read_error {
            return Err(serde_json::to_string(&read_error.diagnostic)
                .unwrap_or_else(|_| read_error.diagnostic.code.clone()));
        }
    }
    Ok(graph)
}

fn graph_template_from_summary(template: TemplateSummary) -> SourceGraphTemplate {
    SourceGraphTemplate {
        id: template.id,
        file: template.file,
        name: template.name,
        origin: template.origin,
        theme_name: template.theme_name,
        is_partial: template.is_partial,
        extends: template.extends,
        includes: template.includes,
        include_groups: template.include_groups,
        get_pages: template.get_pages,
        get_sections: template.get_sections,
        internal_links: template.internal_links,
        asset_urls: template.asset_urls,
        asset_hashes: template.asset_hashes,
        literal_asset_references: template.literal_asset_references,
        asset_reference_eligible: template.asset_reference_eligible,
        asset_reference_unanalysable: template.asset_reference_unanalysable,
        data_loads: template.data_loads,
        image_metadata: template.image_metadata,
        image_resizes: template.image_resizes,
        blocks: template
            .blocks
            .into_iter()
            .map(|(block, _node_id)| block)
            .collect(),
        component_definitions: template.component_definitions,
        component_calls: template.component_calls,
        semantics: template.semantics,
        markdown_projections: template.markdown_projections,
        node_id: template.node_id,
    }
}

#[derive(Clone, Debug)]
struct SectionPageTemplateBinding {
    directory: PathBuf,
    template: String,
}

fn collect_section_page_templates(
    project_root: &Path,
    zola_root: &Path,
    content_files: &[PathBuf],
    draft_sources: &HashMap<String, String>,
) -> Vec<SectionPageTemplateBinding> {
    content_files
        .iter()
        .filter(|path| {
            matches!(
                zola_content_page_kind(zola_root, path),
                SourcePageKind::Home | SourcePageKind::Section
            )
        })
        .filter_map(|path| {
            let file = relative_project_path(project_root, path);
            let source = draft_sources.get(&file)?.clone();
            let template = parse_zola_content_frontmatter(&source).page_template?;
            Some(SectionPageTemplateBinding {
                directory: content_directory(zola_root, path)?,
                template,
            })
        })
        .collect()
}

fn inherited_page_template<'a>(
    zola_root: &Path,
    page_path: &Path,
    bindings: &'a [SectionPageTemplateBinding],
) -> Option<&'a str> {
    if !matches!(
        zola_content_page_kind(zola_root, page_path),
        SourcePageKind::Page
    ) {
        return None;
    }
    let page_directory = content_directory(zola_root, page_path)?;
    bindings
        .iter()
        .filter(|binding| {
            binding.directory.as_os_str().is_empty()
                || page_directory.starts_with(&binding.directory)
        })
        .max_by_key(|binding| binding.directory.components().count())
        .map(|binding| binding.template.as_str())
}

fn content_directory(zola_root: &Path, content_path: &Path) -> Option<PathBuf> {
    content_path
        .strip_prefix(zola_root.join("content"))
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

fn add_workspace_manifest_only_paths(
    project_root: &Path,
    active_theme: Option<&str>,
    projection: &WorkspaceProjectionSnapshot,
    local_assets: &mut Vec<std::path::PathBuf>,
    theme_assets: &mut Vec<std::path::PathBuf>,
    data_files: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    projection
        .accepted_disk
        .require_identity(&projection.runtime_session_id, &projection.project_root)?;
    let projected_paths = projection
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .chain(projection.resource_bytes.keys().map(String::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    for projected_path in projected_paths {
        if projection.deleted_sources.contains(projected_path) {
            continue;
        }
        let relative = projected_path.replace('\\', "/");
        if relative.starts_with('/')
            || relative
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(format!(
                "Source Graph a refuzat path-ul manifest nesigur {}.",
                projected_path
            ));
        }
        let path = project_root.join(&relative);
        if relative.starts_with("static/") {
            local_assets.push(path);
        } else if active_theme
            .is_some_and(|theme| relative.starts_with(&format!("themes/{theme}/static/")))
        {
            theme_assets.push(path);
        } else if relative.starts_with("date/")
            && Path::new(&relative)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    ZOLA_DATA_FILE_EXTENSIONS
                        .iter()
                        .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                })
        {
            data_files.push(path);
        }
    }
    local_assets.sort();
    local_assets.dedup();
    theme_assets.sort();
    theme_assets.dedup();
    data_files.sort();
    data_files.dedup();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::test_support::ProjectModelTestFixture;
    use crate::source_graph::model::{
        ComponentDefinitionKind, ComponentDependencyKind, ComponentInvocationKind,
        ComponentResolutionStatus, SourceNodeKind, SourceOrigin, SourceRelationKind,
        SourceStyleScope,
    };

    use super::*;

    #[test]
    fn source_graph_includes_draft_only_template_for_atomic_workspace_planning() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "{% block content %}<main></main>{% endblock %}\n",
        )
        .unwrap();
        let mut fixture = ProjectModelTestFixture::from_integration_disk_boundary(&root).unwrap();
        fixture.draft(
            "templates/partials/hero.html",
            "<section class=\"hero\"></section>\n",
        );
        let graph = fixture.build_source_graph().unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert!(graph
            .templates
            .iter()
            .any(|template| template.name == "partials/hero.html" && template.is_partial));
        assert!(graph.nodes.iter().any(|node| {
            node.file == "templates/partials/hero.html"
                && node.kind == SourceNodeKind::Html
                && node.label == "<section .hero>"
        }));
    }

    #[test]
    fn tera_components_are_indexed_with_exact_ranges_in_templates_and_markdown() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/components")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        let markdown = concat!(
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
            "{{<ui.card title=\"Din Markdown\" />}}\n",
        );
        let template = "<main>{{<ui.card title=\"Din template\" />}}</main>\n";
        let definition = concat!(
            "{% component ui.card(title: string, tone: string=\"normal\", ...attrs) %}",
            "<article>{{ title }}</article>",
            "{% endcomponent ui.card %}\n",
        );
        fs::write(root.join("content/_index.md"), markdown).unwrap();
        fs::write(root.join("templates/index.html"), template).unwrap();
        fs::write(root.join("templates/components/ui.html"), definition).unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let component = graph
            .component_graph
            .definitions
            .iter()
            .find(|definition| {
                definition.kind == ComponentDefinitionKind::TeraComponent
                    && definition.name == "ui.card"
            })
            .expect("component definition");
        let definition_range = component.range.as_ref().expect("definition range");
        assert_eq!(
            &definition[definition_range.start..definition_range.end],
            definition.trim_end()
        );
        assert_eq!(component.parameters.len(), 3);
        assert_eq!(component.rest_parameter.as_deref(), Some("attrs"));

        let mut calls = graph
            .component_graph
            .invocations
            .iter()
            .filter(|invocation| invocation.kind == ComponentInvocationKind::TeraComponent)
            .collect::<Vec<_>>();
        calls.sort_by(|left, right| left.file.cmp(&right.file));
        assert_eq!(calls.len(), 2);
        for call in calls {
            assert_eq!(call.status, ComponentResolutionStatus::Resolved);
            assert_eq!(call.resolved_definition_ids, vec![component.id.clone()]);
            let source = if call.file == "content/_index.md" {
                markdown
            } else {
                template
            };
            let range = call.call_range.as_ref().expect("call range");
            assert!(source[range.start..range.end].starts_with("{{<ui.card"));
            assert_eq!(call.arguments.len(), 1);
            assert!(call.arguments[0].range.is_some());
        }
    }

    #[test]
    fn template_and_partial_roots_keep_full_file_ranges() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/listing-items")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(root.join("content/_index.md"), "+++\n+++\n").unwrap();
        let page_source = "<main>Acasă</main>\n";
        let fragment_source = "\n";
        fs::write(root.join("templates/index.html"), page_source).unwrap();
        fs::write(
            root.join("templates/listing-items/card.html"),
            fragment_source,
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        let page = graph
            .templates
            .iter()
            .find(|template| template.file == "templates/index.html")
            .unwrap();
        let fragment = graph
            .templates
            .iter()
            .find(|template| template.file == "templates/listing-items/card.html")
            .unwrap();
        let page_root = graph
            .nodes
            .iter()
            .find(|node| node.id == page.node_id)
            .unwrap();
        let fragment_root = graph
            .nodes
            .iter()
            .find(|node| node.id == fragment.node_id)
            .unwrap();

        assert_eq!(page_root.range.as_ref().unwrap().start, 0);
        assert_eq!(page_root.range.as_ref().unwrap().end, page_source.len());
        assert_eq!(fragment_root.range.as_ref().unwrap().start, 0);
        assert_eq!(
            fragment_root.range.as_ref().unwrap().end,
            fragment_source.len()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_graph_resolves_a_draft_only_load_data_target_outside_date() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(root.join("content/_index.md"), "+++\n+++\n").unwrap();
        fs::write(root.join("templates/index.html"), "<main>Inițial</main>").unwrap();
        let mut fixture = ProjectModelTestFixture::from_integration_disk_boundary(&root).unwrap();
        fixture
            .draft(
                "templates/index.html",
                r#"{% set site = load_data(path="content/site.toml") %}"#,
            )
            .draft("content/site.toml", "titlu = \"Draft\"\n");
        let graph = fixture.build_source_graph().unwrap();
        let data_file = graph
            .data_files
            .iter()
            .find(|data_file| data_file.file == "content/site.toml")
            .unwrap();
        assert_eq!(
            data_file.location,
            crate::source_graph::model::SourceDataLocation::Content
        );
        assert!(data_file.capabilities.can_edit_visual);
        assert!(data_file
            .nodes
            .iter()
            .any(|node| node.value_preview.as_deref() == Some("Draft")));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_graph_rejects_unsafe_virtual_draft_path() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        let fixture = ProjectModelTestFixture::from_integration_disk_boundary(&root).unwrap();
        let mut projection = fixture.projection();
        projection
            .source_texts
            .insert("../outside.html".to_string(), "<main></main>\n".to_string());

        let error = match build_source_graph_from_workspace_projection(&root, &projection) {
            Ok(_) => panic!("unsafe draft path should be rejected"),
            Err(error) => error,
        };
        fs::remove_dir_all(&root).unwrap();

        assert!(error.contains("path-ul draft nesigur"));
    }

    #[test]
    fn invalid_toml_frontmatter_returns_a_controlled_diagnostic() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"șir neînchis\n+++\n\nConținut păstrat.\n",
        )
        .unwrap();

        let error = match build_graph_from_integration_disk(&root) {
            Ok(_) => panic!("invalid TOML frontmatter should return a diagnostic"),
            Err(error) => error,
        };
        fs::remove_dir_all(&root).unwrap();

        let diagnostic =
            serde_json::from_str::<crate::localization::LocalizedDiagnostic>(&error).unwrap();
        assert_eq!(diagnostic.code, "source-graph-frontmatter-invalid");
        assert_eq!(
            diagnostic.arguments.get("format"),
            Some(&serde_json::Value::String("Toml".to_string()))
        );
        assert!(diagnostic
            .arguments
            .get("details")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|details| details.contains("invalid basic string")));
    }

    #[test]
    fn builds_minimal_zola_source_graph() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/partials")).unwrap();
        fs::create_dir_all(root.join("sass/pagini")).unwrap();
        fs::create_dir_all(root.join("sass/partials")).unwrap();
        fs::create_dir_all(root.join("static/js")).unwrap();

        fs::write(
            root.join("config.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n\nSalut\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/base.html"),
            "<body>{% block content %}{% endblock %}</body>",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "{% extends \"base.html\" %}{% block content %}{% include \"partials/header.html\" %}{% set cards = section.extra.cards %}<main class=\"hero\"></main>{% for card in cards %}<article class=\"card\"></article>{% endfor %}{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/header.html"),
            "<header></header>",
        )
        .unwrap();
        fs::write(root.join("sass/pagini/index.scss"), ".hero {}\n").unwrap();
        fs::write(root.join("sass/partials/_header.scss"), "header {}\n").unwrap();
        fs::write(
            root.join("static/js/header.js"),
            "document.querySelector('header');\n",
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(graph.pages.len(), 1);
        let zola_config = graph
            .structured_documents
            .iter()
            .find(|document| {
                document.kind
                    == crate::source_graph::model::SourceStructuredDocumentKind::ZolaConfig
            })
            .expect("zola config projection");
        assert!(zola_config.nodes.iter().any(|node| {
            node.key.as_deref() == Some("base_url")
                && node.value_preview.as_deref() == Some("http://example.test")
        }));
        assert_eq!(
            graph.pages[0].frontmatter_format,
            Some(crate::source_graph::model::SourceDataFormat::Toml)
        );
        assert!(graph.pages[0].frontmatter_nodes.iter().any(|node| {
            node.kind == crate::source_graph::model::SourceDataNodeKind::Value
                && node.key.as_deref() == Some("title")
                && node.value_preview.as_deref() == Some("Acasă")
        }));
        assert!(graph.templates.iter().any(|template| {
            template.name == "index.html" && template.extends.as_deref() == Some("base.html")
        }));
        assert!(graph.templates.iter().any(|template| {
            template.name == "index.html"
                && template
                    .includes
                    .contains(&"partials/header.html".to_string())
        }));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::PageTemplate));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::Extends));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::Includes));
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.kind == SourceRelationKind::UsesStyle));
        let header_template = graph
            .templates
            .iter()
            .find(|template| template.name == "partials/header.html")
            .unwrap();
        let header_style = graph
            .styles
            .iter()
            .find(|style| style.file == "sass/partials/_header.scss")
            .unwrap();
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::UsesStyle
                && relation.from == header_template.node_id
                && relation.to == header_style.node_id
        }));
        let header_script = graph
            .scripts
            .iter()
            .find(|script| script.file == "static/js/header.js")
            .unwrap();
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::UsesScript
                && relation.from == header_template.node_id
                && relation.to == header_script.node_id
        }));
        let header_component = graph
            .component_graph
            .definitions
            .iter()
            .find(|definition| {
                definition.kind == ComponentDefinitionKind::Partial
                    && definition.template_name.as_deref() == Some("partials/header.html")
            })
            .unwrap();
        assert!(header_component.dependencies.iter().any(|dependency| {
            dependency.kind == ComponentDependencyKind::Style
                && dependency.reference == "sass/partials/_header.scss"
        }));
        assert!(header_component.dependencies.iter().any(|dependency| {
            dependency.kind == ComponentDependencyKind::Script
                && dependency.reference == "static/js/header.js"
        }));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == SourceNodeKind::Html && node.label == "<main .hero>"));
        let main_node = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<main .hero>")
            .unwrap();
        let main_parent = graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == main_node.parent.as_deref())
            .unwrap();
        assert!(main_parent.kind == SourceNodeKind::Block);

        let card_node = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<article .card>")
            .unwrap();
        let card_parent = graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == card_node.parent.as_deref())
            .unwrap();
        assert!(card_parent.kind == SourceNodeKind::For);
        assert!(!card_node.capabilities.can_edit_visual);
    }

    #[test]
    fn source_graph_publishes_exact_multilingual_slug_routes() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = 'https://example.test'\ndefault_language = 'ro'\n[languages.en]\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.en.md"),
            "+++\ntitle = 'Home'\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/comunitate.en.md"),
            "+++\ntitle = 'Community'\nslug = 'community'\n+++\n",
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let routes = graph
            .pages
            .iter()
            .map(|page| (page.file.as_str(), page.url.as_str()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(routes.get("content/_index.en.md"), Some(&"/en/"));
        assert_eq!(
            routes.get("content/comunitate.en.md"),
            Some(&"/en/community/"),
        );
    }

    #[test]
    fn section_page_template_creates_source_graph_relation() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content/blog")).unwrap();
        fs::create_dir_all(root.join("templates/blog")).unwrap();

        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/blog/_index.md"),
            "+++\ntitle = \"Blog\"\npage_template = \"blog/page.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/blog/post.md"),
            "+++\ntitle = \"Post\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/section.html"),
            "<h1>{{ section.title }}</h1>",
        )
        .unwrap();
        fs::write(
            root.join("templates/blog/page.html"),
            "<h1>{{ page.title }}</h1>",
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let section = graph
            .pages
            .iter()
            .find(|page| page.file == "content/blog/_index.md")
            .unwrap();
        assert_eq!(
            section.frontmatter_page_template.as_deref(),
            Some("blog/page.html")
        );
        assert!(section.page_template_node_id.is_some());
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::SectionPageTemplate
                && relation.from == section.content_node_id
                && Some(&relation.to) == section.page_template_node_id.as_ref()
        }));
        let post = graph
            .pages
            .iter()
            .find(|page| page.file == "content/blog/post.md")
            .unwrap();
        assert_eq!(post.frontmatter_template, None);
        assert_eq!(post.resolved_template.as_deref(), Some("blog/page.html"));
        assert!(post.template_node_id.is_some());
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::PageTemplate
                && relation.from == post.content_node_id
                && Some(&relation.to) == post.template_node_id.as_ref()
        }));
    }

    #[test]
    fn zola_content_functions_create_source_graph_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content/blog")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();

        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/blog/_index.md"),
            "+++\ntitle = \"Blog\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/blog/post.md"),
            "+++\ntitle = \"Post\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"{% set post = get_page(path="blog/post.md") %}
{% set blog = get_section(path="blog/_index.md", metadata_only=true) %}
<a href="{{ get_url(path="@/blog/post.md") }}">Post</a>
<link rel="stylesheet" href="{{ get_url(path="css/site.css") }}">
"#,
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let post = graph
            .pages
            .iter()
            .find(|page| page.file == "content/blog/post.md")
            .unwrap();
        let section = graph
            .pages
            .iter()
            .find(|page| page.file == "content/blog/_index.md")
            .unwrap();
        assert!(template.get_pages.contains(&"blog/post.md".to_string()));
        assert!(template
            .get_sections
            .contains(&"blog/_index.md".to_string()));
        assert!(template
            .internal_links
            .contains(&"blog/post.md".to_string()));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::GetsPage
                && relation.from == template.node_id
                && relation.to == post.content_node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::GetsSection
                && relation.from == template.node_id
                && relation.to == section.content_node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::InternalContentLink
                && relation.from == template.node_id
                && relation.to == post.content_node_id
        }));
        assert!(!graph
            .diagnostics
            .iter()
            .any(
                |diagnostic| serde_json::to_string(&diagnostic.diagnostic.arguments)
                    .is_ok_and(|arguments| arguments.contains("css/site.css"))
            ));
    }

    #[test]
    fn zola_static_asset_functions_create_source_graph_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("static/js")).unwrap();
        fs::create_dir_all(root.join("static/css")).unwrap();
        fs::create_dir_all(root.join("static/data")).unwrap();
        fs::create_dir_all(root.join("static/img")).unwrap();

        fs::write(
            root.join("config.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n",
        )
        .unwrap();
        fs::write(root.join("static/js/app.js"), "console.log('ok');").unwrap();
        fs::write(root.join("static/css/site.css"), "body{}").unwrap();
        fs::write(root.join("static/data/catalog.json"), "{}").unwrap();
        fs::write(root.join("static/img/hero.png"), b"png").unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"<script src="{{ get_url(path="js/app.js") }}" integrity="{{ get_hash(path="static/js/app.js") }}"></script>
<link rel="stylesheet" href="{{ get_url(path="css/site.css") }}">
{% set data = load_data(path="static/data/catalog.json") %}
{% set meta = get_image_metadata(path="static/img/hero.png") %}
{% set image = resize_image(path="static/img/hero.png", width=640, op="fit_width") %}
"#,
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let script = graph
            .assets
            .iter()
            .find(|asset| asset.logical_path == "js/app.js")
            .unwrap();
        let stylesheet = graph
            .assets
            .iter()
            .find(|asset| asset.logical_path == "css/site.css")
            .unwrap();
        let data = graph
            .data_files
            .iter()
            .find(|data_file| data_file.file == "static/data/catalog.json")
            .unwrap();
        let image = graph
            .assets
            .iter()
            .find(|asset| asset.logical_path == "img/hero.png")
            .unwrap();

        assert!(template.asset_urls.contains(&"js/app.js".to_string()));
        assert!(template
            .asset_hashes
            .contains(&"static/js/app.js".to_string()));
        assert!(template
            .data_loads
            .contains(&"static/data/catalog.json".to_string()));
        assert!(!graph
            .assets
            .iter()
            .any(|asset| asset.file == "static/data/catalog.json"));
        assert!(template
            .image_metadata
            .contains(&"static/img/hero.png".to_string()));
        assert!(template
            .image_resizes
            .contains(&"static/img/hero.png".to_string()));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::AssetUrl
                && relation.from == template.node_id
                && relation.to == script.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::AssetHash
                && relation.from == template.node_id
                && relation.to == script.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::AssetUrl
                && relation.from == template.node_id
                && relation.to == stylesheet.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::DataFileLoad
                && relation.from == template.node_id
                && relation.to == data.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::ImageMetadata
                && relation.from == template.node_id
                && relation.to == image.node_id
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::ImageResize
                && relation.from == template.node_id
                && relation.to == image.node_id
        }));
    }

    #[test]
    fn zola_data_files_create_load_data_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("date")).unwrap();

        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("date/meniu.toml"),
            "[[item]]\nlabel = \"Acasă\"\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"{% set meniu = load_data(path="date/meniu.toml") %}"#,
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let data_file = graph
            .data_files
            .iter()
            .find(|data_file| data_file.logical_path == "date/meniu.toml")
            .unwrap();

        assert!(template.data_loads.contains(&"date/meniu.toml".to_string()));
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == SourceNodeKind::DataFile && node.file == "date/meniu.toml"));
        assert_eq!(
            data_file.format,
            crate::source_graph::model::SourceDataFormat::Toml
        );
        assert!(data_file.parse_error.is_none());
        assert!(data_file.nodes.iter().any(|node| {
            node.kind == crate::source_graph::model::SourceDataNodeKind::ArrayOfTables
                && node.key.as_deref() == Some("item")
        }));
        assert!(data_file.nodes.iter().any(|node| {
            node.kind == crate::source_graph::model::SourceDataNodeKind::Value
                && node.key.as_deref() == Some("label")
                && node.value_preview.as_deref() == Some("Acasă")
        }));
        assert!(graph.nodes.iter().any(|node| {
            node.kind == SourceNodeKind::DataValue
                && node.file == "date/meniu.toml"
                && node.label == "label"
        }));
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::DataFileLoad
                && relation.from == template.node_id
                && relation.to == data_file.node_id
        }));
    }

    #[test]
    fn load_data_catalog_resolves_the_complete_zola_search_domain() {
        let root = unique_test_dir();
        for directory in [
            "content/blog",
            "templates",
            "date",
            "data",
            "static/data",
            "generated/site",
            "themes/demo/templates",
            "themes/demo/static/data",
        ] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"demo\"\noutput_dir = \"generated/site\"\n",
        )
        .unwrap();
        fs::write(root.join("content/_index.md"), "+++\n+++\n").unwrap();
        fs::write(
            root.join("date/nefolosit.toml"),
            "titlu = \"Convențional\"\n",
        )
        .unwrap();
        fs::write(root.join("catalog.toml"), "titlu = \"Rădăcină\"\n").unwrap();
        fs::write(root.join("arbitrar.json"), r#"{"unused":true}"#).unwrap();
        fs::write(root.join("static/data/catalog.json"), r#"{"static":true}"#).unwrap();
        fs::write(root.join("content/blog/tabel.csv"), "id,nume\n1,Test\n").unwrap();
        fs::write(root.join("generated/site/cache.yaml"), "cache: true\n").unwrap();
        fs::write(
            root.join("themes/demo/static/data/tema.xml"),
            "<date><titlu>Temă</titlu></date>",
        )
        .unwrap();
        fs::write(root.join("data/precedenta.json"), r#"{"root":true}"#).unwrap();
        fs::write(
            root.join("static/data/precedenta.json"),
            r#"{"static":true}"#,
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"
{% set root = load_data(path="catalog.toml") %}
{% set static = load_data(path="data/catalog.json") %}
{% set content = load_data(path="content/blog/tabel.csv") %}
{% set output = load_data(path="cache.yaml") %}
{% set precedence = load_data(path="data/precedenta.json") %}
{% set missing = load_data(path="missing.json") %}
{% set dynamic = load_data(path=data_path) %}
{% set remote = load_data(url="https://example.test/catalog.json") %}
<a href="{{ get_url(path="catalog.toml") }}">Nu este asset static</a>
"#,
        )
        .unwrap();
        fs::write(
            root.join("themes/demo/templates/partial.html"),
            r#"{% set theme_data = load_data(path="data/tema.xml") %}"#,
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();

        let by_file = graph
            .data_files
            .iter()
            .map(|data_file| (data_file.file.as_str(), data_file))
            .collect::<HashMap<_, _>>();
        assert_eq!(
            by_file["date/nefolosit.toml"].location,
            crate::source_graph::model::SourceDataLocation::Date
        );
        assert!(by_file["date/nefolosit.toml"]
            .load_paths
            .contains(&"date/nefolosit.toml".to_string()));
        assert_eq!(
            by_file["catalog.toml"].location,
            crate::source_graph::model::SourceDataLocation::Project
        );
        assert!(by_file["catalog.toml"].capabilities.can_edit_visual);
        assert_eq!(
            by_file["static/data/catalog.json"].location,
            crate::source_graph::model::SourceDataLocation::Static
        );
        assert!(
            !by_file["static/data/catalog.json"]
                .capabilities
                .can_edit_visual
        );
        assert_eq!(
            by_file["content/blog/tabel.csv"].location,
            crate::source_graph::model::SourceDataLocation::Content
        );
        assert!(!by_file.contains_key("generated/site/cache.yaml"));
        assert_eq!(
            by_file["themes/demo/static/data/tema.xml"].location,
            crate::source_graph::model::SourceDataLocation::Theme
        );
        assert_eq!(
            by_file["themes/demo/static/data/tema.xml"].origin,
            SourceOrigin::Theme
        );
        assert!(
            !by_file["themes/demo/static/data/tema.xml"]
                .capabilities
                .can_open_in_code
        );
        assert!(!by_file.contains_key("static/data/precedenta.json"));
        assert!(by_file.contains_key("data/precedenta.json"));
        assert!(!by_file.contains_key("zola.toml"));
        assert!(!by_file.contains_key("arbitrar.json"));
        assert!(!graph
            .assets
            .iter()
            .any(|asset| asset.file == "static/data/catalog.json"));
        assert!(!graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::AssetUrl
                && relation.to == by_file["catalog.toml"].node_id
        }));
        assert_eq!(
            graph
                .relations
                .iter()
                .filter(|relation| relation.kind == SourceRelationKind::DataFileLoad)
                .count(),
            5
        );
        assert!(graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.diagnostic.code == "source-graph-dynamic-load-data"));
        assert!(!graph.diagnostics.iter().any(|diagnostic| {
            serde_json::to_string(&diagnostic.diagnostic.arguments)
                .is_ok_and(|arguments| arguments.contains("https://example.test/catalog.json"))
        }));
        assert!(graph
            .diagnostics
            .iter()
            .any(
                |diagnostic| diagnostic.diagnostic.code == "source-graph-load-data-missing"
                    && diagnostic.diagnostic.arguments.get("path")
                        == Some(&serde_json::Value::String("missing.json".to_string()))
            ));
        assert!(graph.diagnostics.iter().any(|diagnostic| {
            diagnostic.diagnostic.code == "source-graph-load-data-missing"
                && diagnostic.diagnostic.arguments.get("path")
                    == Some(&serde_json::Value::String("cache.yaml".to_string()))
        }));
        let stable_graph = build_graph_from_integration_disk(&root).unwrap();
        assert_eq!(
            stable_graph
                .data_files
                .iter()
                .find(|data_file| data_file.file == "static/data/catalog.json")
                .map(|data_file| data_file.node_id.as_str()),
            Some(by_file["static/data/catalog.json"].node_id.as_str())
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_data_catalog_excludes_external_generated_output() {
        let root = unique_test_dir();
        let output_name = format!(
            "pana-source-graph-output-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let output = root.parent().unwrap().join(&output_name);
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(
            root.join("zola.toml"),
            format!("base_url = '/'\noutput_dir = '../{output_name}'\n"),
        )
        .unwrap();
        fs::write(root.join("content/_index.md"), "+++\n+++\n").unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"{% set cache = load_data(path="cache.json") %}"#,
        )
        .unwrap();
        fs::write(output.join("cache.json"), r#"{"generated":true}"#).unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        assert!(!graph
            .data_files
            .iter()
            .any(|data_file| data_file.file == "@output/cache.json"));
        assert!(graph.diagnostics.iter().any(|diagnostic| {
            diagnostic.diagnostic.code == "source-graph-load-data-missing"
                && diagnostic.diagnostic.arguments.get("path")
                    == Some(&serde_json::Value::String("cache.json".to_string()))
        }));

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn zola_content_files_create_load_data_relations() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content/blog")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();

        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/blog/post.md"),
            "+++\ntitle = \"Post\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"{% set post = load_data(path="@/blog/post.md") %}
{% set post_copy = load_data(path="content/blog/post.md") %}
"#,
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let template = graph
            .templates
            .iter()
            .find(|template| template.name == "index.html")
            .unwrap();
        let data_file = graph
            .data_files
            .iter()
            .find(|data_file| data_file.file == "content/blog/post.md")
            .unwrap();

        assert!(template.data_loads.contains(&"@/blog/post.md".to_string()));
        assert!(template
            .data_loads
            .contains(&"content/blog/post.md".to_string()));
        assert_eq!(
            data_file.location,
            crate::source_graph::model::SourceDataLocation::Content
        );
        assert!(graph.relations.iter().any(|relation| {
            relation.kind == SourceRelationKind::DataFileLoad
                && relation.from == template.node_id
                && relation.to == data_file.node_id
        }));
    }

    #[test]
    fn resolves_active_theme_templates_as_fallback() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("themes/test-theme/templates/partials")).unwrap();
        fs::create_dir_all(root.join("themes/test-theme/static/css")).unwrap();
        fs::create_dir_all(root.join("themes/test-theme/sass/pagini")).unwrap();

        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n\nSalut\n",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/templates/index.html"),
            "{% extends \"base.html\" %}{% block content %}<main></main>{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/templates/base.html"),
            "<body>{% include \"partials/footer.html\" %}{% block content %}{% endblock %}</body>",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/templates/partials/footer.html"),
            "<footer></footer>",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/static/css/style.css"),
            "body { color: black; }",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/sass/pagini/index.scss"),
            ".theme-main { color: red; }",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/theme.toml"),
            "name = \"Test Theme\"\n",
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(graph.active_theme.as_deref(), Some("test-theme"));
        assert!(graph.structured_documents.iter().any(|document| {
            document.kind == crate::source_graph::model::SourceStructuredDocumentKind::ThemeConfig
                && document.file == "themes/test-theme/theme.toml"
                && document
                    .nodes
                    .iter()
                    .any(|node| node.key.as_deref() == Some("name"))
        }));
        let page = graph.pages.iter().find(|page| page.url == "/").unwrap();
        let template = page
            .template_node_id
            .as_ref()
            .and_then(|node_id| {
                graph
                    .templates
                    .iter()
                    .find(|template| &template.node_id == node_id)
            })
            .unwrap();
        assert_eq!(template.name, "index.html");
        assert_eq!(template.origin, SourceOrigin::Theme);
        assert_eq!(template.theme_name.as_deref(), Some("test-theme"));
        assert!(graph.templates.iter().any(|template| {
            template.name == "partials/footer.html" && template.origin == SourceOrigin::Theme
        }));
        assert!(graph.styles.iter().any(|style| {
            style.file == "themes/test-theme/static/css/style.css"
                && style.origin == SourceOrigin::Theme
                && matches!(style.scope, SourceStyleScope::Global)
        }));
        let theme_page_style = graph
            .styles
            .iter()
            .find(|style| style.file == "themes/test-theme/sass/pagini/index.scss")
            .unwrap();
        assert!(graph.relations.iter().any(|relation| {
            relation.from == page.id
                && relation.to == theme_page_style.node_id
                && relation.kind == SourceRelationKind::UsesStyle
        }));
    }

    #[test]
    fn local_template_overrides_theme_template_for_page_style() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("sass/pagini")).unwrap();
        fs::create_dir_all(root.join("themes/test-theme/templates")).unwrap();
        fs::create_dir_all(root.join("themes/test-theme/sass/pagini")).unwrap();

        fs::write(
            root.join("zola.toml"),
            "base_url = \"http://example.test\"\ntheme = \"test-theme\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\n+++\n\nSalut\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "{% block content %}<main class=\"local\"></main>{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("sass/pagini/index.scss"),
            ".local { color: blue; }",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/templates/index.html"),
            "{% block content %}<main class=\"theme\"></main>{% endblock %}",
        )
        .unwrap();
        fs::write(
            root.join("themes/test-theme/sass/pagini/index.scss"),
            ".theme { color: red; }",
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let page = graph.pages.iter().find(|page| page.url == "/").unwrap();
        let template = page
            .template_node_id
            .as_ref()
            .and_then(|node_id| {
                graph
                    .templates
                    .iter()
                    .find(|template| &template.node_id == node_id)
            })
            .unwrap();
        assert_eq!(template.origin, SourceOrigin::Local);

        let local_page_style = graph
            .styles
            .iter()
            .find(|style| style.file == "sass/pagini/index.scss")
            .unwrap();
        assert!(graph.relations.iter().any(|relation| {
            relation.from == page.id
                && relation.to == local_page_style.node_id
                && relation.kind == SourceRelationKind::UsesStyle
        }));
    }

    #[test]
    fn partial_blocks_are_diagnostics_not_layout_blocks() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates/partials")).unwrap();

        fs::write(
            root.join("config.toml"),
            "base_url = \"http://example.test\"\n",
        )
        .unwrap();
        fs::write(
            root.join("content/_index.md"),
            "+++\ntitle = \"Acasă\"\ntemplate = \"index.html\"\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("templates/index.html"),
            "{% include \"partials/cta.html\" %}",
        )
        .unwrap();
        fs::write(
            root.join("templates/partials/cta.html"),
            "{% block content %}<section class=\"cta\"></section>{% endblock %}",
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let partial = graph
            .templates
            .iter()
            .find(|template| template.name == "partials/cta.html")
            .unwrap();
        assert!(partial.is_partial);
        assert!(partial.blocks.is_empty());
        assert!(graph.diagnostics.iter().any(|diagnostic| {
            diagnostic.diagnostic.code == "source-graph-partial-block-invalid"
                && diagnostic.diagnostic.arguments.get("name")
                    == Some(&serde_json::Value::String("partials/cta.html".to_string()))
        }));
        assert!(!graph.nodes.iter().any(|node| {
            node.file.ends_with("templates/partials/cta.html") && node.kind == SourceNodeKind::Block
        }));
        let section = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<section .cta>")
            .unwrap();
        let section_parent = graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == section.parent.as_deref())
            .unwrap();
        assert!(matches!(section_parent.kind, SourceNodeKind::Partial));
    }

    #[test]
    fn mixed_html_and_tera_nodes_follow_the_nearest_real_structural_parent() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(
            root.join("templates/index.html"),
            r#"{% block content %}
<section class="outer">
  {% if visible %}
    <article class="card">
      <h2>{{ title }}</h2>
    </article>
  {% endif %}
</section>
{% endblock %}
"#,
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let node = |label: &str| {
            graph
                .nodes
                .iter()
                .find(|node| node.kind == SourceNodeKind::Html && node.label == label)
                .unwrap_or_else(|| {
                    panic!(
                        "lipsește {label}; noduri HTML: {:?}",
                        graph
                            .nodes
                            .iter()
                            .filter(|node| node.kind == SourceNodeKind::Html)
                            .map(|node| node.label.as_str())
                            .collect::<Vec<_>>()
                    )
                })
        };
        let parent = |node: &crate::source_graph::model::SourceNode| {
            graph
                .nodes
                .iter()
                .find(|candidate| Some(candidate.id.as_str()) == node.parent.as_deref())
                .unwrap()
        };
        let section = node("<section .outer>");
        let article = node("<article .card>");
        let heading = node("<h2>");
        let conditional = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::If)
            .unwrap();
        let title = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::TeraVariable && node.label == "title")
            .unwrap();

        assert_eq!(parent(section).kind, SourceNodeKind::Block);
        assert_eq!(parent(conditional).id, section.id);
        assert_eq!(parent(article).kind, SourceNodeKind::If);
        assert_eq!(parent(heading).id, article.id);
        assert_eq!(parent(title).id, heading.id);
    }

    #[test]
    fn managed_icon_is_one_atomic_source_graph_block() {
        let root = unique_test_dir();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        fs::write(root.join("content/_index.md"), "+++\n+++\n").unwrap();
        fs::write(
            root.join("templates/index.html"),
            concat!(
                "<main><svg class=\"icon ps-icon-test\" data-pana-block=\"icon\" ",
                "data-pana-instance=\"icon-test\" data-pana-icon=\"tabler-outline:home\" ",
                "viewBox=\"0 0 24 24\"><path d=\"M3 12h18\"></path>",
                "<path d=\"M12 3v18\"></path></svg></main>\n",
            ),
        )
        .unwrap();

        let graph = build_graph_from_integration_disk(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        let icon_root = graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Html
                    && node.file == "templates/index.html"
                    && node.label.starts_with("<svg")
            })
            .expect("rădăcina iconului");
        assert!(!graph.nodes.iter().any(|node| {
            node.kind == SourceNodeKind::Html
                && node.file == "templates/index.html"
                && node.label.starts_with("<path")
        }));
        let marker = graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::BlockMarker && node.label == "icon")
            .expect("marker icon");
        assert_eq!(marker.parent.as_deref(), Some(icon_root.id.as_str()));
        let instance = graph
            .block_graph
            .source_instances
            .iter()
            .find(|instance| instance.provider_id == "icon")
            .expect("instanță icon");
        assert_eq!(instance.definition_id.as_deref(), Some("native/icon"));
    }

    fn build_graph_from_integration_disk(root: &Path) -> Result<SourceGraph, String> {
        ProjectModelTestFixture::from_integration_disk_boundary(root)?.build_source_graph()
    }

    fn unique_test_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("panastudio-source-graph-{nanos}"))
    }
}
