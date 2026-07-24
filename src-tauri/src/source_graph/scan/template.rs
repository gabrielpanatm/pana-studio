use std::{collections::HashMap, path::Path};

use crate::source_graph::{
    html::{html_label, should_project_html_tag},
    mixed_cst::{parse_mixed_cst, MixedCstDocument, MixedCstKind},
    model::{
        SourceCapabilities, SourceDiagnosticSeverity, SourceGraphInclude, SourceNodeKind,
        SourceOrigin, SourceRelationKind,
    },
    scan::{
        builder::SourceGraphBuilder,
        files::{read_source, relative_project_path, template_name},
        ranges::source_range,
        summary::{TemplateSummary, TeraScopeSummary},
    },
    tera::{tera_items_from_document, TeraItemKind},
    zola::extract_zola_template_references,
};

#[derive(Clone)]
struct SetPrelude {
    variable: String,
    start: usize,
    end: usize,
    parent: Option<String>,
}

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
        SourceCapabilities::code_only("Fișier template Tera."),
    );

    let source = read_source(path, &file, draft_sources, builder);
    let mixed_document = parse_mixed_cst(&source, &name);
    debug_assert!(mixed_document.is_lossless());
    let tera_document = &mixed_document.tera;
    if !tera_document.is_valid_tera() {
        builder.add_diagnostic(
            SourceDiagnosticSeverity::Error,
            format!(
                "Template-ul nu respectă gramatica Tera folosită de Zola: {}",
                tera_document
                    .validation_error()
                    .unwrap_or("eroare Tera necunoscută")
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

    for item in tera_items_from_document(&tera_document) {
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
                                "Partialurile nu trebuie să folosească extends. Creează un template de pagină/layout pentru moștenire Tera.",
                                Some(file.clone()),
                                Some(range),
                            );
                            continue;
                        }
                        SourceNodeKind::Block => {
                            builder.add_diagnostic(
                                SourceDiagnosticSeverity::Warning,
                                format!(
                                    "Partialul {} conține block Tera '{}'. Partialurile trebuie să fie fragmente incluse, fără block/endblock.",
                                    name, item.label
                                ),
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

                match kind {
                    SourceNodeKind::Extends => {
                        if extends.is_some() {
                            builder.add_diagnostic(
                                SourceDiagnosticSeverity::Warning,
                                "Template-ul are mai multe directive extends; Zola/Tera așteaptă una singură.",
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
                                format!("Block Tera duplicat în același template: {}", item.label),
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
                "CST-ul lossless și AST-ul Tera nu au reconciliat aceleași block-uri.",
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

    add_mixed_html_nodes(
        &file,
        &source,
        &node_id,
        origin.clone(),
        theme_name.clone(),
        &completed_scopes,
        &mixed_document,
        builder,
    );

    let zola_references = extract_zola_template_references(&source);

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
        data_loads: zola_references.data_loads,
        image_metadata: zola_references.image_metadata,
        image_resizes: zola_references.image_resizes,
        blocks,
        macros,
        semantics: tera_document.semantics().cloned(),
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

fn add_mixed_html_nodes(
    file: &str,
    source: &str,
    template_node_id: &str,
    origin: SourceOrigin,
    theme_name: Option<String>,
    tera_scopes: &[TeraScopeSummary],
    document: &MixedCstDocument,
    builder: &mut SourceGraphBuilder,
) {
    let mut projected_elements = HashMap::<usize, (String, usize)>::new();

    for (element_index, element) in document.elements.iter().enumerate() {
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
                SourceCapabilities::code_only(
                    "Marcaj furnizat de registrul blocurilor native Rust.",
                ),
            );
        }
        projected_elements.insert(element_index, (node_id, opening_node.start));
    }
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

fn innermost_tera_scope<'a>(
    scopes: &'a [TeraScopeSummary],
    start: usize,
    end: usize,
) -> Option<&'a TeraScopeSummary> {
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
        SourceNodeKind::For => SourceCapabilities::code_only(
            "Element randat într-un loop Tera; editarea vizuală directă este nesigură.",
        ),
        SourceNodeKind::If => SourceCapabilities::code_only(
            "Element randat condițional prin Tera; editarea vizuală directă este nesigură.",
        ),
        SourceNodeKind::Macro => SourceCapabilities::code_only(
            "Element definit într-un macro Tera; modificarea poate afecta mai multe folosiri.",
        ),
        SourceNodeKind::Set | SourceNodeKind::SetGlobal | SourceNodeKind::Filter => {
            SourceCapabilities::code_only(
                "Element aflat într-un scope Tera local; editarea se face în cod.",
            )
        }
        SourceNodeKind::Raw => SourceCapabilities::code_only(
            "Element aflat într-un bloc raw Tera; editarea vizuală este dezactivată.",
        ),
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

fn tera_reason(kind: &SourceNodeKind) -> &'static str {
    match kind {
        SourceNodeKind::Extends => "Moștenire Tera.",
        SourceNodeKind::Block => "Block Tera.",
        SourceNodeKind::Include => "Include Tera.",
        SourceNodeKind::Import => "Import Tera.",
        SourceNodeKind::Macro => "Macro Tera.",
        SourceNodeKind::For => "Buclă Tera.",
        SourceNodeKind::If => "Condiție Tera.",
        SourceNodeKind::Elif => "Ramură elif Tera.",
        SourceNodeKind::Else => "Ramură else Tera.",
        SourceNodeKind::Set => "Setare Tera.",
        SourceNodeKind::SetGlobal => "Setare globală Tera.",
        SourceNodeKind::Filter => "Bloc filter Tera.",
        SourceNodeKind::Break => "Break Tera.",
        SourceNodeKind::Continue => "Continue Tera.",
        SourceNodeKind::Super => "Apel super() Tera.",
        SourceNodeKind::TeraVariable => "Variabilă Tera.",
        SourceNodeKind::MacroCall => "Apel de macro Tera.",
        SourceNodeKind::FunctionCall => "Apel de funcție Tera/Zola.",
        SourceNodeKind::Shortcode => "Invocare shortcode Zola.",
        SourceNodeKind::BlockMarker => "Marcaj al unui provider de bloc nativ.",
        SourceNodeKind::TeraComment => "Comentariu Tera.",
        SourceNodeKind::Raw => "Bloc raw Tera.",
        _ => "Sintaxă Tera.",
    }
}
