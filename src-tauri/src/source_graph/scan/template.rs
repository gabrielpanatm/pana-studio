use std::{collections::HashMap, path::Path};

use crate::{
    localization::LocalizedDiagnostic,
    source_graph::{
        asset_references::scan_html_asset_references,
        html::{html_label, should_project_html_tag},
        markdown::MarkdownSourceNode,
        mixed_cst::{parse_mixed_cst, MixedCstDocument, MixedCstKind},
        model::{
            SourceCapabilities, SourceCapabilityReason, SourceDiagnosticSeverity,
            SourceGraphInclude, SourceNodeKind, SourceOrigin, SourceRelationKind,
        },
        scan::{
            builder::SourceGraphBuilder,
            files::{read_source, relative_project_path, template_name},
            ranges::source_range,
            summary::{TemplateSummary, TeraScopeSummary},
        },
        tera::{tera_items_from_document, TeraItemKind},
        zola::extract_zola_template_references,
    },
};

#[derive(Clone)]
struct SetPrelude {
    variable: String,
    start: usize,
    end: usize,
    parent: Option<String>,
}

struct TeraProjectionNode {
    node_id: String,
    kind: SourceNodeKind,
    parent_id: Option<String>,
    start: usize,
    end: usize,
}

struct ProjectedHtmlBody {
    node_id: String,
    start: usize,
    end: usize,
}

// Root identity, origin and workspace source projection remain explicit scanner inputs.
#[allow(clippy::too_many_arguments)]
pub(super) fn scan_template(
    project_root: &Path,
    zola_root: &Path,
    path: &Path,
    origin: SourceOrigin,
    theme_name: Option<String>,
    draft_sources: &HashMap<String, String>,
    builder: &mut SourceGraphBuilder,
) -> TemplateSummary {
    let file = relative_project_path(project_root, path);
    let name = template_name(zola_root, path, theme_name.as_deref());
    let is_partial = name.starts_with("partials/")
        || name.starts_with("listing-items/")
        || name.starts_with("macros/")
        || name.starts_with("shortcodes/");
    let file_node_kind = if is_partial {
        SourceNodeKind::Partial
    } else {
        SourceNodeKind::Template
    };
    let node_id = builder.add_node(
        file_node_kind,
        file.clone(),
        origin.clone(),
        theme_name.clone(),
        name.clone(),
        None,
        None,
        SourceCapabilities::code_only(SourceCapabilityReason::TeraTemplateFile),
    );

    let source = read_source(&file, draft_sources, builder);
    // The template/partial node is also the addressable root of a fragment
    // opened directly in Template Workbench. Keeping its full-file range in
    // SourceGraph gives HTML and Tera insertions one Rust-owned anchor even
    // when the file is completely empty and has no child nodes yet.
    builder.update_node_range(&node_id, source_range(&source, 0, source.len()));
    let mixed_document = parse_mixed_cst(&source, &name);
    debug_assert!(mixed_document.is_lossless());
    let tera_document = &mixed_document.tera;
    if !tera_document.is_valid_tera() {
        builder.add_diagnostic(
            SourceDiagnosticSeverity::Error,
            LocalizedDiagnostic::new("source-graph-tera-syntax-invalid").with_argument(
                "details",
                tera_document
                    .validation_error()
                    .unwrap_or("unknown Tera error"),
            ),
            Some(file.clone()),
            None,
        );
    }
    let mut scope_stack = vec![node_id.clone()];
    let mut extends = None;
    let mut includes = Vec::new();
    let mut include_groups = Vec::new();
    let mut imports = Vec::new();
    let mut blocks = Vec::new();
    let mut macros = Vec::new();
    let mut open_scopes: Vec<TeraScopeSummary> = Vec::new();
    let mut completed_scopes: Vec<TeraScopeSummary> = Vec::new();
    let mut set_preludes: Vec<SetPrelude> = Vec::new();
    let mut tera_projection_nodes = Vec::<TeraProjectionNode>::new();

    for item in tera_items_from_document(tera_document) {
        match item.kind {
            TeraItemKind::EndScope => {
                if scope_stack.len() > 1 {
                    scope_stack.pop();
                }
                if let Some(mut scope) = open_scopes.pop() {
                    scope.end = item.end;
                    builder.update_node_range(
                        &scope.node_id,
                        source_range(&source, scope.start, item.end),
                    );
                    completed_scopes.push(scope);
                }
            }
            TeraItemKind::Node => {
                let Some(kind) = item.node_kind.clone() else {
                    continue;
                };
                let range = source_range(&source, item.start, item.end);
                if is_partial {
                    match kind {
                        SourceNodeKind::Extends => {
                            builder.add_diagnostic(
                                SourceDiagnosticSeverity::Warning,
                                LocalizedDiagnostic::new("source-graph-partial-extends-invalid"),
                                Some(file.clone()),
                                Some(range),
                            );
                            continue;
                        }
                        SourceNodeKind::Block => {
                            builder.add_diagnostic(
                                SourceDiagnosticSeverity::Warning,
                                LocalizedDiagnostic::new("source-graph-partial-block-invalid")
                                    .with_argument("name", name.clone())
                                    .with_argument("block", item.label.clone()),
                                Some(file.clone()),
                                Some(range),
                            );
                            continue;
                        }
                        _ => {}
                    }
                }
                let parent = scope_stack.last().cloned();
                let item_node_id = builder.add_node(
                    kind.clone(),
                    file.clone(),
                    origin.clone(),
                    theme_name.clone(),
                    item.label.clone(),
                    Some(range),
                    parent.clone(),
                    SourceCapabilities::code_only(tera_reason(&kind)),
                );
                tera_projection_nodes.push(TeraProjectionNode {
                    node_id: item_node_id.clone(),
                    kind: kind.clone(),
                    parent_id: parent.clone(),
                    start: item.start,
                    end: item.end,
                });

                match kind {
                    SourceNodeKind::Extends => {
                        if extends.is_some() {
                            builder.add_diagnostic(
                                SourceDiagnosticSeverity::Warning,
                                LocalizedDiagnostic::new("source-graph-multiple-extends"),
                                Some(file.clone()),
                                Some(source_range(&source, item.start, item.end)),
                            );
                        }
                        extends = item.target.clone();
                    }
                    SourceNodeKind::Include => {
                        for target in &item.targets {
                            push_unique(&mut includes, Some(target.clone()));
                        }
                        include_groups.push(SourceGraphInclude {
                            targets: item.targets.clone(),
                            ignore_missing: item.ignore_missing,
                        });
                    }
                    SourceNodeKind::Import => push_unique(&mut imports, item.target.clone()),
                    SourceNodeKind::Block => {
                        if blocks.iter().any(|(block, _)| block == &item.label) {
                            builder.add_diagnostic(
                                SourceDiagnosticSeverity::Warning,
                                LocalizedDiagnostic::new("source-graph-duplicate-tera-block")
                                    .with_argument("block", item.label.clone()),
                                Some(file.clone()),
                                Some(source_range(&source, item.start, item.end)),
                            );
                        }
                        blocks.push((item.label.clone(), item_node_id.clone()));
                        builder.add_relation(
                            node_id.clone(),
                            item_node_id.clone(),
                            SourceRelationKind::DefinesBlock,
                            item.label.clone(),
                        );
                    }
                    SourceNodeKind::Macro => push_unique(&mut macros, Some(item.label.clone())),
                    _ => {}
                }

                if matches!(kind, SourceNodeKind::Set | SourceNodeKind::SetGlobal) {
                    if let Some(variable) =
                        crate::source_graph::tera::set_assignment_name(&item.label)
                    {
                        set_preludes.push(SetPrelude {
                            variable,
                            start: item.start,
                            end: item.end,
                            parent: parent.clone(),
                        });
                    }
                }

                if item.opens_scope() {
                    let scope_start = if kind == SourceNodeKind::For {
                        take_loop_prelude_start(
                            &source,
                            &item.label,
                            item.start,
                            parent.as_ref(),
                            &mut set_preludes,
                        )
                        .unwrap_or(item.start)
                    } else {
                        item.start
                    };
                    scope_stack.push(item_node_id.clone());
                    open_scopes.push(TeraScopeSummary {
                        node_id: item_node_id,
                        kind,
                        start: scope_start,
                        end: source.len(),
                    });
                }
            }
        }
    }

    for scope in open_scopes.drain(..) {
        builder.update_node_range(
            &scope.node_id,
            source_range(&source, scope.start, source.len()),
        );
        completed_scopes.push(scope);
    }

    if let Some(semantics) = tera_document.semantics() {
        let facts = semantics.template_facts();
        let cst_block_names = blocks
            .iter()
            .map(|(block, _)| block.clone())
            .collect::<Vec<_>>();
        if !is_partial && cst_block_names != facts.blocks {
            builder.add_diagnostic(
                SourceDiagnosticSeverity::Error,
                LocalizedDiagnostic::new("source-graph-tera-cst-ast-mismatch"),
                Some(file.clone()),
                None,
            );
        }
        extends = facts.extends;
        includes = facts.includes;
        include_groups = facts
            .include_groups
            .into_iter()
            .map(|include| SourceGraphInclude {
                targets: include.targets,
                ignore_missing: include.ignore_missing,
            })
            .collect();
        imports = facts.imports;
        macros = facts.macros;
    }

    let projected_html_bodies = add_mixed_html_nodes(
        &file,
        &source,
        &node_id,
        origin.clone(),
        theme_name.clone(),
        &completed_scopes,
        &mixed_document,
        builder,
    );
    reparent_tera_nodes_inside_html(&tera_projection_nodes, &projected_html_bodies, builder);

    let zola_references = extract_zola_template_references(&source);
    let literal_asset_references = scan_html_asset_references(&source, &mixed_document);
    let markdown_source_nodes = tera_projection_nodes
        .iter()
        .map(|node| MarkdownSourceNode {
            id: node.node_id.clone(),
            kind: node.kind.clone(),
            start: node.start,
            end: node.end,
        })
        .collect::<Vec<_>>();
    let markdown_projections =
        crate::source_graph::markdown::analyze_template_markdown_with_source_nodes(
            &file,
            &source,
            &node_id,
            &markdown_source_nodes,
        )
        .projections;
    if zola_references.dynamic_data_loads > 0 {
        builder.add_diagnostic(
            crate::source_graph::model::SourceDiagnosticSeverity::Warning,
            LocalizedDiagnostic::new("source-graph-dynamic-load-data")
                .with_argument("count", zola_references.dynamic_data_loads as u64)
                .with_argument("file", file.clone()),
            Some(file.clone()),
            None,
        );
    }

    TemplateSummary {
        id: node_id.clone(),
        file,
        name,
        node_id,
        origin,
        theme_name,
        is_partial,
        extends,
        includes,
        include_groups,
        imports,
        get_pages: zola_references.get_pages,
        get_sections: zola_references.get_sections,
        internal_links: zola_references.internal_links,
        asset_urls: zola_references.asset_urls,
        asset_hashes: zola_references.asset_hashes,
        asset_reference_eligible: literal_asset_references.eligible(),
        asset_reference_unanalysable: literal_asset_references.unanalysable,
        literal_asset_references: literal_asset_references.references,
        data_loads: zola_references.data_loads,
        image_metadata: zola_references.image_metadata,
        image_resizes: zola_references.image_resizes,
        blocks,
        macros,
        semantics: tera_document.semantics().cloned(),
        markdown_projections,
    }
}

fn take_loop_prelude_start(
    source: &str,
    for_label: &str,
    for_start: usize,
    parent: Option<&String>,
    set_preludes: &mut Vec<SetPrelude>,
) -> Option<usize> {
    let collection_root = crate::source_graph::tera::for_collection_root(for_label)?;
    let index = set_preludes.iter().rev().position(|candidate| {
        candidate.variable == collection_root
            && candidate.parent.as_ref() == parent
            && source
                .get(candidate.end..for_start)
                .map(|gap| gap.trim().is_empty())
                .unwrap_or(false)
    })?;
    Some(set_preludes.remove(set_preludes.len() - 1 - index).start)
}

// Mixed-CST projection keeps its source, scope and builder evidence independently borrowed.
#[allow(clippy::too_many_arguments)]
fn add_mixed_html_nodes(
    file: &str,
    source: &str,
    template_node_id: &str,
    origin: SourceOrigin,
    theme_name: Option<String>,
    tera_scopes: &[TeraScopeSummary],
    document: &MixedCstDocument,
    builder: &mut SourceGraphBuilder,
) -> Vec<ProjectedHtmlBody> {
    let mut projected_elements = HashMap::<usize, (String, usize)>::new();
    let mut projected_bodies = Vec::new();

    for (element_index, element) in document.elements.iter().enumerate() {
        if is_managed_icon_descendant(element, document, source) {
            continue;
        }
        let Some(opening_node) = document.nodes.get(element.opening_node) else {
            continue;
        };
        let MixedCstKind::StartTag(tag) = &opening_node.kind else {
            continue;
        };
        if !should_project_html_tag(&tag.name) {
            continue;
        }

        let parent_scope = innermost_tera_scope(tera_scopes, opening_node.start, opening_node.end);
        let html_parent =
            projected_html_parent(element.parent, &document.elements, &projected_elements);
        let parent_node_id = match (parent_scope, html_parent.as_ref()) {
            (Some(scope), Some((html_node_id, html_start))) if *html_start > scope.start => {
                html_node_id.as_str()
            }
            (Some(scope), _) => scope.node_id.as_str(),
            (None, Some((html_node_id, _))) => html_node_id.as_str(),
            (None, None) => template_node_id,
        };
        let raw = opening_node.full_text(source);
        let element_end = element
            .closing_node
            .and_then(|closing_node| document.nodes.get(closing_node))
            .map(|closing_node| closing_node.end)
            .unwrap_or(opening_node.end);
        let node_id = builder.add_node(
            SourceNodeKind::Html,
            file.to_string(),
            origin.clone(),
            theme_name.clone(),
            html_label(&tag.name, raw),
            Some(source_range(source, opening_node.start, element_end)),
            Some(parent_node_id.to_string()),
            html_capabilities(parent_scope),
        );
        if let Some(closing_start) = element
            .closing_node
            .and_then(|closing_node| document.nodes.get(closing_node))
            .map(|closing_node| closing_node.start)
        {
            if opening_node.end <= closing_start {
                projected_bodies.push(ProjectedHtmlBody {
                    node_id: node_id.clone(),
                    start: opening_node.end,
                    end: closing_start,
                });
            }
        }
        let block_marker_attribute = ["data-pana-block", "data-pana-component"]
            .into_iter()
            .find(|attribute| html_attribute_value(source, tag, attribute).is_some());
        if let Some((marker_attribute, block_id)) = block_marker_attribute
            .and_then(|attribute| {
                html_attribute_value(source, tag, attribute).map(|value| (attribute, value))
            })
            .map(|(attribute, value)| (attribute, value.trim()))
            .filter(|(_, value)| !value.is_empty())
        {
            let marker_range = tag
                .attributes
                .iter()
                .find(|attribute| attribute.name.eq_ignore_ascii_case(marker_attribute))
                .map(|attribute| {
                    source_range(
                        source,
                        attribute.name_start,
                        attribute.value_end.unwrap_or(attribute.name_end),
                    )
                });
            builder.add_node(
                SourceNodeKind::BlockMarker,
                file.to_string(),
                origin.clone(),
                theme_name.clone(),
                block_id.to_string(),
                marker_range,
                Some(node_id.clone()),
                SourceCapabilities::code_only(SourceCapabilityReason::NativeBlockMarker),
            );
        }
        projected_elements.insert(element_index, (node_id, opening_node.start));
    }
    projected_bodies
}

fn reparent_tera_nodes_inside_html(
    tera_nodes: &[TeraProjectionNode],
    html_bodies: &[ProjectedHtmlBody],
    builder: &mut SourceGraphBuilder,
) {
    let containers = tera_nodes
        .iter()
        .filter_map(|node| {
            innermost_html_body(html_bodies, node.start, node.end)
                .map(|body| (node.node_id.as_str(), body.node_id.as_str()))
        })
        .collect::<HashMap<_, _>>();

    for node in tera_nodes {
        let Some(container_id) = containers.get(node.node_id.as_str()).copied() else {
            continue;
        };
        let parent_container = node
            .parent_id
            .as_deref()
            .and_then(|parent_id| containers.get(parent_id).copied());
        if parent_container != Some(container_id) {
            builder.reparent_node(&node.node_id, container_id);
        }
    }
}

fn innermost_html_body(
    bodies: &[ProjectedHtmlBody],
    start: usize,
    end: usize,
) -> Option<&ProjectedHtmlBody> {
    bodies
        .iter()
        .filter(|body| body.start <= start && end <= body.end)
        .max_by_key(|body| (body.start, usize::MAX - body.end))
}

fn is_managed_icon_descendant(
    element: &crate::source_graph::mixed_cst::HtmlElementCst,
    document: &MixedCstDocument,
    source: &str,
) -> bool {
    let mut parent = element.parent;
    while let Some(parent_index) = parent {
        let Some(parent_element) = document.elements.get(parent_index) else {
            return false;
        };
        let Some(opening_node) = document.nodes.get(parent_element.opening_node) else {
            return false;
        };
        if let MixedCstKind::StartTag(tag) = &opening_node.kind {
            let is_icon = tag.name.eq_ignore_ascii_case("svg")
                && html_attribute_value(source, tag, "data-pana-block")
                    .is_some_and(|value| value.trim() == "icon");
            if is_icon {
                return true;
            }
        }
        parent = parent_element.parent;
    }
    false
}

fn html_attribute_value<'a>(
    source: &'a str,
    tag: &crate::source_graph::mixed_cst::HtmlStartTagCst,
    name: &str,
) -> Option<&'a str> {
    let attribute = tag
        .attributes
        .iter()
        .find(|attribute| attribute.name.eq_ignore_ascii_case(name))?;
    source.get(attribute.value_start?..attribute.value_end?)
}

fn projected_html_parent(
    mut parent: Option<usize>,
    elements: &[crate::source_graph::mixed_cst::HtmlElementCst],
    projected: &HashMap<usize, (String, usize)>,
) -> Option<(String, usize)> {
    while let Some(parent_index) = parent {
        if let Some(parent_node) = projected.get(&parent_index) {
            return Some(parent_node.clone());
        }
        parent = elements
            .get(parent_index)
            .and_then(|element| element.parent);
    }
    None
}

fn innermost_tera_scope(
    scopes: &[TeraScopeSummary],
    start: usize,
    end: usize,
) -> Option<&TeraScopeSummary> {
    scopes
        .iter()
        .filter(|scope| scope.start <= start && end <= scope.end)
        .max_by_key(|scope| (scope.start, usize::MAX - scope.end))
}

fn html_capabilities(parent_scope: Option<&TeraScopeSummary>) -> SourceCapabilities {
    let Some(scope) = parent_scope else {
        return SourceCapabilities::visual_html();
    };

    match scope.kind {
        SourceNodeKind::For => {
            SourceCapabilities::code_only(SourceCapabilityReason::HtmlInTeraLoop)
        }
        SourceNodeKind::If => {
            SourceCapabilities::code_only(SourceCapabilityReason::HtmlInTeraCondition)
        }
        SourceNodeKind::Macro => {
            SourceCapabilities::code_only(SourceCapabilityReason::HtmlInTeraMacro)
        }
        SourceNodeKind::Set | SourceNodeKind::SetGlobal | SourceNodeKind::Filter => {
            SourceCapabilities::code_only(SourceCapabilityReason::HtmlInTeraLocalScope)
        }
        SourceNodeKind::Raw => SourceCapabilities::code_only(SourceCapabilityReason::HtmlInTeraRaw),
        SourceNodeKind::Block => SourceCapabilities::visual_html(),
        _ => SourceCapabilities::visual_html(),
    }
}

fn push_unique(values: &mut Vec<String>, value: Option<String>) {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    if !values.contains(&value) {
        values.push(value);
    }
}

fn tera_reason(kind: &SourceNodeKind) -> SourceCapabilityReason {
    match kind {
        SourceNodeKind::Extends => SourceCapabilityReason::TeraExtends,
        SourceNodeKind::Block => SourceCapabilityReason::TeraBlock,
        SourceNodeKind::Include => SourceCapabilityReason::TeraInclude,
        SourceNodeKind::Import => SourceCapabilityReason::TeraImport,
        SourceNodeKind::Macro => SourceCapabilityReason::TeraMacro,
        SourceNodeKind::For => SourceCapabilityReason::TeraFor,
        SourceNodeKind::If => SourceCapabilityReason::TeraIf,
        SourceNodeKind::Elif => SourceCapabilityReason::TeraElif,
        SourceNodeKind::Else => SourceCapabilityReason::TeraElse,
        SourceNodeKind::Set => SourceCapabilityReason::TeraSet,
        SourceNodeKind::SetGlobal => SourceCapabilityReason::TeraSetGlobal,
        SourceNodeKind::Filter => SourceCapabilityReason::TeraFilter,
        SourceNodeKind::Break => SourceCapabilityReason::TeraBreak,
        SourceNodeKind::Continue => SourceCapabilityReason::TeraContinue,
        SourceNodeKind::Super => SourceCapabilityReason::TeraSuper,
        SourceNodeKind::TeraVariable => SourceCapabilityReason::TeraVariable,
        SourceNodeKind::MacroCall => SourceCapabilityReason::TeraMacroCall,
        SourceNodeKind::FunctionCall => SourceCapabilityReason::TeraFunctionCall,
        SourceNodeKind::Shortcode => SourceCapabilityReason::ZolaShortcode,
        SourceNodeKind::BlockMarker => SourceCapabilityReason::NativeBlockMarker,
        SourceNodeKind::TeraComment => SourceCapabilityReason::TeraComment,
        SourceNodeKind::Raw => SourceCapabilityReason::TeraRaw,
        _ => SourceCapabilityReason::TeraSyntax,
    }
}
