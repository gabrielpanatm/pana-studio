use std::{collections::HashMap, path::Path};

use crate::zola_theme::ZolaThemeResolver;
use crate::{
    localization::LocalizedDiagnostic,
    source_graph::{
        model::{
            SourceCapabilities, SourceCapabilityReason, SourceDataFormat, SourceGraphPage,
            SourceNodeKind, SourceOrigin, SourceRelationKind,
        },
        scan::{
            builder::SourceGraphBuilder,
            data_file::project_data_nodes_into_source_graph,
            files::{normalize_template_name, read_source, relative_project_path},
            style::conventional_style_files_for_template,
            summary::TemplateSummary,
        },
        structured_data::{parse_lossless_toml, parse_zola_data_adapter, rebase_data_node_ranges},
        tera_cst::{parse_tera_cst, TeraCstKind},
        zola::{
            collect_zola_runtime_deprecations, parse_zola_content_frontmatter,
            resolve_zola_page_template, resolve_zola_section_page_template, zola_content_page_kind,
            zola_content_url, zola_frontmatter_range,
        },
    },
};
use zola_config::Config;

// Root identity, workspace projection and resolver inputs are separate trust boundaries.
#[allow(clippy::too_many_arguments)]
pub(super) fn scan_content_page(
    project_root: &Path,
    zola_root: &Path,
    path: &Path,
    inherited_page_template: Option<&str>,
    template_node_by_name: &HashMap<String, String>,
    template_by_name: &HashMap<String, TemplateSummary>,
    style_by_file: &HashMap<String, String>,
    resolver: &ZolaThemeResolver,
    route_config: Option<&Config>,
    draft_sources: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) -> SourceGraphPage {
    let file = relative_project_path(project_root, path);
    let source = read_source(&file, draft_sources, builder);
    let frontmatter = parse_zola_content_frontmatter(&source);
    let page_kind = zola_content_page_kind(zola_root, path);
    let title = frontmatter
        .title
        .clone()
        .unwrap_or_else(|| fallback_page_title(path));
    let effective_template = frontmatter
        .template
        .clone()
        .or_else(|| inherited_page_template.map(str::to_string));
    let resolved_template = resolve_zola_page_template(&effective_template, &page_kind);
    let resolved_page_template =
        resolve_zola_section_page_template(&frontmatter.page_template, &page_kind);
    let node_id = builder.add_node(
        SourceNodeKind::Page,
        file.clone(),
        SourceOrigin::Local,
        None,
        title.clone(),
        None,
        None,
        SourceCapabilities::code_only(SourceCapabilityReason::MarkdownPage),
    );
    let (frontmatter_format, frontmatter_parse_error, mut frontmatter_nodes) =
        project_frontmatter(&source, &file, builder);
    project_data_nodes_into_source_graph(
        &file,
        &node_id,
        &SourceOrigin::Local,
        None,
        &mut frontmatter_nodes,
        builder,
    );
    let tera_document = parse_tera_cst(&source, &file);
    debug_assert!(tera_document.is_lossless());
    if !tera_document.is_valid_tera() {
        builder.add_diagnostic(
            crate::source_graph::model::SourceDiagnosticSeverity::Error,
            LocalizedDiagnostic::new("source-graph-content-tera-syntax-invalid").with_argument(
                "details",
                tera_document
                    .validation_error()
                    .unwrap_or("unknown Tera error"),
            ),
            Some(file.clone()),
            None,
        );
    }
    let mut component_calls = tera_document
        .semantics()
        .map(|semantics| semantics.component_calls.clone())
        .unwrap_or_default();
    if let Some(semantics) = tera_document.semantics() {
        for deprecation in collect_zola_runtime_deprecations(semantics) {
            builder.add_diagnostic(
                crate::source_graph::model::SourceDiagnosticSeverity::Warning,
                LocalizedDiagnostic::new("source-graph-zola-runtime-argument-deprecated")
                    .with_argument("function", deprecation.function)
                    .with_argument("argument", deprecation.argument)
                    .with_argument("replacement", deprecation.replacement),
                Some(file.clone()),
                None,
            );
        }
    }
    project_component_call_nodes(&file, &node_id, &mut component_calls, builder);
    diagnose_removed_content_calls(&source, &file, &tera_document, template_by_name, builder);
    let template_node_id = resolved_template.as_ref().and_then(|template| {
        template_node_by_name
            .get(&normalize_template_name(template))
            .cloned()
    });

    if let (Some(template), Some(template_node_id)) =
        (resolved_template.as_ref(), template_node_id.as_ref())
    {
        builder.add_relation(
            node_id.clone(),
            template_node_id.clone(),
            SourceRelationKind::PageTemplate,
            template.clone(),
        );

        let normalized_template = normalize_template_name(template);
        if let Some(template_summary) = template_by_name.get(&normalized_template) {
            for style_project_path in
                conventional_style_files_for_template(resolver, template_summary)
            {
                if let Some(style_node_id) = style_by_file.get(&style_project_path) {
                    builder.add_relation(
                        node_id.clone(),
                        style_node_id.clone(),
                        SourceRelationKind::UsesStyle,
                        style_project_path,
                    );
                    break;
                }
            }
        }
    } else if let Some(template) = resolved_template.as_ref() {
        builder.add_diagnostic(
            crate::source_graph::model::SourceDiagnosticSeverity::Warning,
            LocalizedDiagnostic::new("source-graph-page-template-missing")
                .with_argument("template", template.clone()),
            Some(file.clone()),
            None,
        );
    }

    let page_template_node_id = resolved_page_template.as_ref().and_then(|template| {
        template_node_by_name
            .get(&normalize_template_name(template))
            .cloned()
    });
    if let (Some(template), Some(template_node_id)) = (
        resolved_page_template.as_ref(),
        page_template_node_id.as_ref(),
    ) {
        builder.add_relation(
            node_id.clone(),
            template_node_id.clone(),
            SourceRelationKind::SectionPageTemplate,
            template.clone(),
        );
    } else if let Some(template) = resolved_page_template.as_ref() {
        builder.add_diagnostic(
            crate::source_graph::model::SourceDiagnosticSeverity::Warning,
            LocalizedDiagnostic::new("source-graph-section-page-template-missing")
                .with_argument("template", template.clone()),
            Some(file.clone()),
            None,
        );
    }

    SourceGraphPage {
        id: node_id.clone(),
        file,
        title,
        url: zola_content_url(zola_root, path, &frontmatter, route_config),
        page_kind,
        frontmatter_template: frontmatter.template,
        frontmatter_page_template: frontmatter.page_template,
        resolved_template,
        content_node_id: node_id,
        template_node_id,
        page_template_node_id,
        frontmatter_format,
        frontmatter_parse_error,
        frontmatter_nodes,
        taxonomies: frontmatter.taxonomies,
        component_calls,
    }
}

fn project_component_call_nodes(
    file: &str,
    parent_node_id: &str,
    calls: &mut [crate::source_graph::tera_semantics::TeraComponentCall],
    builder: &mut SourceGraphBuilder,
) {
    let mut node_ids = Vec::with_capacity(calls.len());
    for call in calls.iter() {
        let parent = call
            .parent_call
            .and_then(|index| node_ids.get(index))
            .cloned()
            .unwrap_or_else(|| parent_node_id.to_string());
        let node_id = builder.add_node(
            SourceNodeKind::ComponentCall,
            file.to_string(),
            SourceOrigin::Local,
            None,
            call.name.clone(),
            Some(tera_range_to_source_range(&call.range)),
            Some(parent),
            SourceCapabilities::code_only(SourceCapabilityReason::TeraComponentCall),
        );
        node_ids.push(node_id);
    }
}

fn diagnose_removed_content_calls(
    source: &str,
    file: &str,
    document: &crate::source_graph::tera_cst::TeraCstDocument,
    template_by_name: &HashMap<String, TemplateSummary>,
    builder: &mut SourceGraphBuilder,
) {
    let removed_names = template_by_name
        .keys()
        .filter_map(|name| {
            name.strip_prefix("shortcodes/").and_then(|name| {
                name.strip_suffix(".html")
                    .or_else(|| name.strip_suffix(".md"))
            })
        })
        .collect::<std::collections::HashSet<_>>();
    if removed_names.is_empty() {
        return;
    }
    for node in &document.nodes {
        if !matches!(node.kind, TeraCstKind::Variable) {
            continue;
        }
        let content = node.content(source).trim();
        let Some(name) = content.split('(').next().map(str::trim) else {
            continue;
        };
        if !content.contains('(') || !removed_names.contains(name) {
            continue;
        }
        builder.add_diagnostic(
            crate::source_graph::model::SourceDiagnosticSeverity::Error,
            LocalizedDiagnostic::new("source-graph-legacy-shortcode-incompatible")
                .with_argument("name", name.to_string()),
            Some(file.to_string()),
            Some(crate::source_graph::scan::ranges::source_range(
                source, node.start, node.end,
            )),
        );
    }
}

fn tera_range_to_source_range(
    range: &crate::source_graph::tera_semantics::TeraSourceRange,
) -> crate::source_graph::model::SourceRange {
    crate::source_graph::model::SourceRange {
        start: range.start,
        end: range.end,
        line: range.line,
        column: range.column,
        end_line: range.end_line,
        end_column: range.end_column,
    }
}

fn project_frontmatter(
    source: &str,
    file: &str,
    builder: &mut SourceGraphBuilder,
) -> (
    Option<SourceDataFormat>,
    Option<String>,
    Vec<crate::source_graph::model::SourceDataNode>,
) {
    let Some((start, end)) = zola_frontmatter_range(source) else {
        return (None, None, Vec::new());
    };
    let body = &source[start..end];
    let without_bom = source.trim_start_matches('\u{feff}');
    let format = if without_bom.starts_with("+++") {
        SourceDataFormat::Toml
    } else {
        SourceDataFormat::Yaml
    };
    let parsed = match format {
        SourceDataFormat::Toml => parse_lossless_toml(body, file).map(|document| document.nodes),
        SourceDataFormat::Yaml => parse_zola_data_adapter(body, file, &format),
        _ => unreachable!(),
    };
    match parsed {
        Ok(mut nodes) => {
            rebase_data_node_ranges(&mut nodes, source, start);
            (Some(format), None, nodes)
        }
        Err(error) => {
            builder.add_diagnostic(
                crate::source_graph::model::SourceDiagnosticSeverity::Error,
                LocalizedDiagnostic::new("source-graph-frontmatter-invalid")
                    .with_argument("format", format!("{format:?}"))
                    .with_argument("details", error.clone()),
                Some(file.to_string()),
                Some(crate::source_graph::scan::ranges::source_range(
                    source, start, end,
                )),
            );
            (Some(format), Some(error), Vec::new())
        }
    }
}

fn fallback_page_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Pagină")
        .replace(['_', '-'], " ")
}
