use crate::{
    kernel::file_buffer_store::FileBufferStore,
    source_graph::{
        literals::find_first_string_literal,
        model::{SourceGraph, SourceNode, SourceNodeKind, SourceRelationKind},
        tera::{parse_tera_items, TeraItem, TeraItemKind},
        zola::{
            find_zola_frontmatter_template_literal, normalize_zola_template_reference,
            parse_zola_path_calls, rewrite_zola_content_load_reference,
            rewrite_zola_content_reference, rewrite_zola_data_file_reference,
            rewrite_zola_static_asset_reference, rewrite_zola_template_reference,
            zola_content_load_reference, zola_content_reference_for_relation,
            zola_data_file_reference_for_rewrite, zola_frontmatter_template_for_key,
            zola_path_function_for_relation, zola_static_asset_reference_for_rewrite,
        },
    },
};

use crate::kernel::source_graph_rewrite::model::{
    SourceGraphReferenceRewrite, SourceGraphRewriteDiagnostic,
};

use super::targets::TemplateRewriteTarget;

#[derive(Clone)]
pub(super) struct TextReplacement {
    pub range_start: usize,
    pub range_end: usize,
    pub new_text: String,
    pub rewrite: SourceGraphReferenceRewrite,
}

pub(super) fn plan_frontmatter_template_replacements(
    graph: &SourceGraph,
    store: &FileBufferStore,
    from_node: &SourceNode,
    target: &TemplateRewriteTarget,
    relation_kind: &str,
    frontmatter_key: &str,
    missing_explicit_code: &str,
    diagnostics: &mut Vec<SourceGraphRewriteDiagnostic>,
) -> Result<Vec<TextReplacement>, String> {
    let page = graph
        .pages
        .iter()
        .find(|page| page.content_node_id == from_node.id)
        .ok_or_else(|| {
            format!(
                "SourceGraphRewrite blocat pentru {}: relația {} nu are pagină Source Graph.",
                from_node.file, relation_kind
            )
        })?;

    if zola_frontmatter_template_for_key(page, frontmatter_key).is_none() {
        let message = if frontmatter_key == "template" {
            format!(
                "SourceGraphRewrite a blocat {}: template-ul implicit {} nu are referință explicită în frontmatter.",
                page.file, target.old_name
            )
        } else {
            format!(
                "SourceGraphRewrite a blocat {}: {} {} nu are referință explicită în frontmatter.",
                page.file, frontmatter_key, target.old_name
            )
        };
        diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
            missing_explicit_code,
            Some(page.file.clone()),
            message,
        ));
        return Ok(Vec::new());
    }

    let source = clean_baseline_text(store, &page.file)?;
    let Some((range_start, range_end, old_reference)) =
        find_zola_frontmatter_template_literal(&source, frontmatter_key, &target.old_name)
    else {
        diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
            "frontmatter_template_literal_not_found",
            Some(page.file.clone()),
            format!(
                "SourceGraphRewrite a blocat {}: nu poate localiza literalul frontmatter {} pentru template {}.",
                page.file, frontmatter_key, target.old_name
            ),
        ));
        return Ok(Vec::new());
    };
    let new_reference = rewrite_zola_template_reference(&old_reference, &target.new_name)?;

    Ok(vec![TextReplacement {
        range_start,
        range_end,
        new_text: new_reference.clone(),
        rewrite: SourceGraphReferenceRewrite {
            relative_path: page.file.clone(),
            target_relative_path: target.relative_path.clone(),
            relation_kind: relation_kind.to_string(),
            old_reference,
            new_reference,
            range_start,
            range_end,
        },
    }])
}

pub(super) fn plan_tera_template_replacements(
    store: &FileBufferStore,
    from_node: &SourceNode,
    relation_kind: &SourceRelationKind,
    target: &TemplateRewriteTarget,
    diagnostics: &mut Vec<SourceGraphRewriteDiagnostic>,
) -> Result<Vec<TextReplacement>, String> {
    let source = clean_baseline_text(store, &from_node.file)?;
    let node_kind = tera_node_kind_for_relation(relation_kind);
    let mut replacements = Vec::new();
    for item in parse_tera_items(&source) {
        if item.kind != TeraItemKind::Node || item.node_kind.as_ref() != Some(&node_kind) {
            continue;
        }
        let Some(item_target) = item.target.as_ref() else {
            continue;
        };
        if normalize_zola_template_reference(item_target) != target.old_name {
            continue;
        }
        let Some((range_start, range_end, old_reference)) =
            find_tera_item_literal_range(&source, &item)
        else {
            diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
                "tera_template_literal_not_found",
                Some(from_node.file.clone()),
                format!(
                    "SourceGraphRewrite a blocat {}: nu poate localiza literalul Tera pentru {}.",
                    from_node.file, target.old_name
                ),
            ));
            continue;
        };
        let new_reference = rewrite_zola_template_reference(&old_reference, &target.new_name)?;
        replacements.push(TextReplacement {
            range_start,
            range_end,
            new_text: new_reference.clone(),
            rewrite: SourceGraphReferenceRewrite {
                relative_path: from_node.file.clone(),
                target_relative_path: target.relative_path.clone(),
                relation_kind: relation_kind_label(relation_kind).to_string(),
                old_reference,
                new_reference,
                range_start,
                range_end,
            },
        });
    }
    Ok(replacements)
}

pub(super) fn plan_zola_content_function_replacements(
    store: &FileBufferStore,
    from_node: &SourceNode,
    relation_kind: &SourceRelationKind,
    target: &TemplateRewriteTarget,
    diagnostics: &mut Vec<SourceGraphRewriteDiagnostic>,
) -> Result<Vec<TextReplacement>, String> {
    let source = clean_baseline_text(store, &from_node.file)?;
    let expected_function = zola_path_function_for_relation(relation_kind);
    let mut replacements = Vec::new();
    for call in parse_zola_path_calls(&source) {
        if call.function != expected_function {
            continue;
        }
        if zola_content_reference_for_relation(&call.path, relation_kind).as_deref()
            != Some(target.old_name.as_str())
        {
            continue;
        }
        let new_reference =
            rewrite_zola_content_reference(&call.path, &target.new_name, relation_kind)?;
        replacements.push(TextReplacement {
            range_start: call.path_start,
            range_end: call.path_end,
            new_text: new_reference.clone(),
            rewrite: SourceGraphReferenceRewrite {
                relative_path: from_node.file.clone(),
                target_relative_path: target.relative_path.clone(),
                relation_kind: relation_kind_label(relation_kind).to_string(),
                old_reference: call.path,
                new_reference,
                range_start: call.path_start,
                range_end: call.path_end,
            },
        });
    }

    if replacements.is_empty() {
        diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
            "zola_content_path_literal_not_found",
            Some(from_node.file.clone()),
            format!(
                "SourceGraphRewrite a blocat {}: nu poate localiza literalul path pentru {}.",
                from_node.file, target.old_name
            ),
        ));
    }
    Ok(replacements)
}

pub(super) fn plan_zola_content_load_function_replacements(
    store: &FileBufferStore,
    from_node: &SourceNode,
    relation_kind: &SourceRelationKind,
    target: &TemplateRewriteTarget,
    diagnostics: &mut Vec<SourceGraphRewriteDiagnostic>,
) -> Result<Vec<TextReplacement>, String> {
    let source = clean_baseline_text(store, &from_node.file)?;
    let expected_function = zola_path_function_for_relation(relation_kind);
    let mut replacements = Vec::new();
    for call in parse_zola_path_calls(&source) {
        if call.function != expected_function {
            continue;
        }
        if zola_content_load_reference(&call.path).as_deref() != Some(target.old_name.as_str()) {
            continue;
        }
        let new_reference = rewrite_zola_content_load_reference(&call.path, &target.new_name)?;
        replacements.push(TextReplacement {
            range_start: call.path_start,
            range_end: call.path_end,
            new_text: new_reference.clone(),
            rewrite: SourceGraphReferenceRewrite {
                relative_path: from_node.file.clone(),
                target_relative_path: target.relative_path.clone(),
                relation_kind: relation_kind_label(relation_kind).to_string(),
                old_reference: call.path,
                new_reference,
                range_start: call.path_start,
                range_end: call.path_end,
            },
        });
    }

    if replacements.is_empty() {
        diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
            "zola_content_load_path_literal_not_found",
            Some(from_node.file.clone()),
            format!(
                "SourceGraphRewrite a blocat {}: nu poate localiza literalul path pentru content load {}.",
                from_node.file, target.old_name
            ),
        ));
    }
    Ok(replacements)
}

pub(super) fn plan_zola_asset_function_replacements(
    store: &FileBufferStore,
    from_node: &SourceNode,
    relation_kind: &SourceRelationKind,
    target: &TemplateRewriteTarget,
    diagnostics: &mut Vec<SourceGraphRewriteDiagnostic>,
) -> Result<Vec<TextReplacement>, String> {
    let source = clean_baseline_text(store, &from_node.file)?;
    let expected_function = zola_path_function_for_relation(relation_kind);
    let mut replacements = Vec::new();
    for call in parse_zola_path_calls(&source) {
        if call.function != expected_function {
            continue;
        }
        if zola_static_asset_reference_for_rewrite(&call.path).as_deref()
            != Some(target.old_name.as_str())
        {
            continue;
        }
        let new_reference = rewrite_zola_static_asset_reference(&call.path, &target.new_name)?;
        replacements.push(TextReplacement {
            range_start: call.path_start,
            range_end: call.path_end,
            new_text: new_reference.clone(),
            rewrite: SourceGraphReferenceRewrite {
                relative_path: from_node.file.clone(),
                target_relative_path: target.relative_path.clone(),
                relation_kind: relation_kind_label(relation_kind).to_string(),
                old_reference: call.path,
                new_reference,
                range_start: call.path_start,
                range_end: call.path_end,
            },
        });
    }

    if replacements.is_empty() {
        diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
            "zola_asset_path_literal_not_found",
            Some(from_node.file.clone()),
            format!(
                "SourceGraphRewrite a blocat {}: nu poate localiza literalul path pentru asset {}.",
                from_node.file, target.old_name
            ),
        ));
    }
    Ok(replacements)
}

pub(super) fn plan_zola_data_file_function_replacements(
    store: &FileBufferStore,
    from_node: &SourceNode,
    relation_kind: &SourceRelationKind,
    target: &TemplateRewriteTarget,
    diagnostics: &mut Vec<SourceGraphRewriteDiagnostic>,
) -> Result<Vec<TextReplacement>, String> {
    let source = clean_baseline_text(store, &from_node.file)?;
    let expected_function = zola_path_function_for_relation(relation_kind);
    let mut replacements = Vec::new();
    for call in parse_zola_path_calls(&source) {
        if call.function != expected_function {
            continue;
        }
        if zola_data_file_reference_for_rewrite(&call.path).as_deref()
            != Some(target.old_name.as_str())
        {
            continue;
        }
        let new_reference = rewrite_zola_data_file_reference(&call.path, &target.new_name)?;
        replacements.push(TextReplacement {
            range_start: call.path_start,
            range_end: call.path_end,
            new_text: new_reference.clone(),
            rewrite: SourceGraphReferenceRewrite {
                relative_path: from_node.file.clone(),
                target_relative_path: target.relative_path.clone(),
                relation_kind: relation_kind_label(relation_kind).to_string(),
                old_reference: call.path,
                new_reference,
                range_start: call.path_start,
                range_end: call.path_end,
            },
        });
    }

    if replacements.is_empty() {
        diagnostics.push(SourceGraphRewriteDiagnostic::blocked(
            "zola_data_file_path_literal_not_found",
            Some(from_node.file.clone()),
            format!(
                "SourceGraphRewrite a blocat {}: nu poate localiza literalul path pentru data file {}.",
                from_node.file, target.old_name
            ),
        ));
    }
    Ok(replacements)
}

pub(super) fn relation_kind_label(kind: &SourceRelationKind) -> &'static str {
    match kind {
        SourceRelationKind::PageTemplate => "page_template",
        SourceRelationKind::SectionPageTemplate => "section_page_template",
        SourceRelationKind::GetsPage => "gets_page",
        SourceRelationKind::GetsSection => "gets_section",
        SourceRelationKind::InternalContentLink => "internal_content_link",
        SourceRelationKind::AssetUrl => "asset_url",
        SourceRelationKind::AssetHash => "asset_hash",
        SourceRelationKind::DataLoad => "data_load",
        SourceRelationKind::DataFileLoad => "data_file_load",
        SourceRelationKind::ContentDataLoad => "content_data_load",
        SourceRelationKind::ImageMetadata => "image_metadata",
        SourceRelationKind::ImageResize => "image_resize",
        SourceRelationKind::Extends => "extends",
        SourceRelationKind::Includes => "includes",
        SourceRelationKind::Imports => "imports",
        SourceRelationKind::DefinesBlock => "defines_block",
        SourceRelationKind::OverridesBlock => "overrides_block",
        SourceRelationKind::UsesStyle => "uses_style",
        SourceRelationKind::UsesScript => "uses_script",
    }
}

fn clean_baseline_text(store: &FileBufferStore, relative_path: &str) -> Result<String, String> {
    let entry = store.files.get(relative_path).ok_or_else(|| {
        format!(
            "SourceGraphRewrite blocat pentru {relative_path}: FileBufferStore nu are baseline urmărit."
        )
    })?;
    if entry.draft.is_some() {
        return Err(format!(
            "SourceGraphRewrite blocat pentru {relative_path}: fișierul are draft nesalvat în FileBufferStore."
        ));
    }
    Ok(entry.baseline_text.clone())
}

fn tera_node_kind_for_relation(kind: &SourceRelationKind) -> SourceNodeKind {
    match kind {
        SourceRelationKind::Extends => SourceNodeKind::Extends,
        SourceRelationKind::Includes => SourceNodeKind::Include,
        SourceRelationKind::Imports => SourceNodeKind::Import,
        _ => SourceNodeKind::Tera,
    }
}

fn find_tera_item_literal_range(source: &str, item: &TeraItem) -> Option<(usize, usize, String)> {
    find_first_string_literal(source, item.start, item.end)
}
