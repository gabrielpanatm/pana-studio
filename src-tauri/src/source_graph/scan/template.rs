use std::{collections::HashMap, path::Path};

use crate::source_graph::{
    html::{parse_html_opening_tags, HtmlItem},
    model::{
        SourceCapabilities, SourceDiagnosticSeverity, SourceNodeKind, SourceOrigin,
        SourceRelationKind,
    },
    scan::{
        builder::SourceGraphBuilder,
        files::{read_source, relative_project_path, template_name},
        ranges::source_range,
        summary::{TemplateSummary, TeraScopeSummary},
    },
    tera::{parse_tera_items, TeraItemKind},
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
    let is_partial = name.starts_with("partials/") || name.starts_with("macros/");
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
    let mut scope_stack = vec![node_id.clone()];
    let mut extends = None;
    let mut includes = Vec::new();
    let mut imports = Vec::new();
    let mut blocks = Vec::new();
    let mut macros = Vec::new();
    let mut open_scopes: Vec<TeraScopeSummary> = Vec::new();
    let mut completed_scopes: Vec<TeraScopeSummary> = Vec::new();
    let mut set_preludes: Vec<SetPrelude> = Vec::new();

    for item in parse_tera_items(&source) {
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
                    SourceNodeKind::Include => push_unique(&mut includes, item.target.clone()),
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

                if kind == SourceNodeKind::Set {
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

                if tera_item_opens_scope(&kind) {
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

    for item in parse_html_opening_tags(&source) {
        add_html_node(
            &file,
            &source,
            &node_id,
            origin.clone(),
            theme_name.clone(),
            &completed_scopes,
            item,
            builder,
        );
    }

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

fn add_html_node(
    file: &str,
    source: &str,
    template_node_id: &str,
    origin: SourceOrigin,
    theme_name: Option<String>,
    tera_scopes: &[TeraScopeSummary],
    item: HtmlItem,
    builder: &mut SourceGraphBuilder,
) {
    let range = source_range(source, item.start, item.end);
    let label = if item.label.is_empty() {
        format!("<{}>", item.tag)
    } else {
        item.label
    };
    let parent_scope = innermost_tera_scope(tera_scopes, item.start, item.end);
    let parent_node_id = parent_scope
        .map(|scope| scope.node_id.as_str())
        .unwrap_or(template_node_id);
    builder.add_node(
        SourceNodeKind::Html,
        file.to_string(),
        origin,
        theme_name,
        label,
        Some(range),
        Some(parent_node_id.to_string()),
        html_capabilities(parent_scope),
    );
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
        SourceNodeKind::Set | SourceNodeKind::With => SourceCapabilities::code_only(
            "Element aflat într-un scope Tera local; editarea se face în cod.",
        ),
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

fn tera_item_opens_scope(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::With
    )
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
        SourceNodeKind::Set => "Setare Tera.",
        SourceNodeKind::With => "Context Tera.",
        SourceNodeKind::TeraVariable => "Variabilă Tera.",
        SourceNodeKind::TeraComment => "Comentariu Tera.",
        SourceNodeKind::Raw => "Bloc raw Tera.",
        _ => "Sintaxă Tera.",
    }
}
