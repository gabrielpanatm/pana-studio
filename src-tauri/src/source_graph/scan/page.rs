use std::{collections::HashMap, path::Path};

use crate::source_graph::{
    model::{
        SourceCapabilities, SourceGraphPage, SourceNodeKind, SourceOrigin, SourceRelationKind,
    },
    scan::{
        builder::SourceGraphBuilder,
        files::{normalize_template_name, read_source, relative_project_path},
        style::conventional_style_files_for_template,
        summary::TemplateSummary,
    },
    zola::{
        parse_zola_content_frontmatter, resolve_zola_page_template,
        resolve_zola_section_page_template, zola_content_page_kind, zola_content_url,
    },
};
use crate::zola_theme::ZolaThemeResolver;

pub(super) fn scan_content_page(
    project_root: &Path,
    zola_root: &Path,
    path: &Path,
    template_node_by_name: &HashMap<String, String>,
    template_by_name: &HashMap<String, TemplateSummary>,
    style_by_file: &HashMap<String, String>,
    resolver: &ZolaThemeResolver,
    draft_sources: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) -> SourceGraphPage {
    let file = relative_project_path(project_root, path);
    let source = read_source(path, &file, draft_sources, builder);
    let frontmatter = parse_zola_content_frontmatter(&source);
    let page_kind = zola_content_page_kind(zola_root, path);
    let title = frontmatter
        .title
        .clone()
        .unwrap_or_else(|| fallback_page_title(path));
    let resolved_template = resolve_zola_page_template(&frontmatter.template, &page_kind);
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
        SourceCapabilities::code_only("Pagină Markdown Zola."),
    );
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
            format!("Template-ul paginii nu a fost găsit: {}", template),
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
            format!(
                "Template-ul page_template al secțiunii nu a fost găsit: {}",
                template
            ),
            Some(file.clone()),
            None,
        );
    }

    SourceGraphPage {
        id: node_id.clone(),
        file,
        title,
        url: zola_content_url(zola_root, path),
        page_kind,
        frontmatter_template: frontmatter.template,
        frontmatter_page_template: frontmatter.page_template,
        resolved_template,
        content_node_id: node_id,
        template_node_id,
        page_template_node_id,
    }
}

fn fallback_page_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Pagină")
        .replace(['_', '-'], " ")
}
