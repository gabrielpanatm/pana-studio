use serde::{Deserialize, Serialize};

use crate::{
    blocks::{
        native_block_by_id, native_block_root_class_name, node_has_native_block_ancestor,
        node_is_native_block, node_is_slider_slot_container, render_native_block_html,
        render_native_block_slot_item_html, unique_native_block_identity,
        validate_native_block_slot_insert, NativeBlockSlotMutationContext,
    },
    project_model::model::{ProjectModel, ProjectModelFileKind},
    source_graph::{
        identity::SourceTextEdit,
        model::{SourceNode, SourceNodeKind, SourceOrigin},
        tera::{parse_tera_items, TeraItemKind},
    },
};

use super::move_engine::{
    append_document_fragment, can_receive_children, content_revision, html_tag_at,
    insert_line_block, inserted_block_start_line, inside_prefix_for_insert, line_block_after_index,
    line_block_before_index, line_number_at_offset, parse_html_tag_at, resolve_html_element_span,
    resolve_html_node_for_anchor, same_model_path, source_location_at_offset,
    source_missing_message, ProjectMovePosition, ProjectSourceEditLocation, Span,
};
use super::structural_edit::{format_html_fragment, normalize_html_subtree, StructuralPlacement};
use super::structural_envelope::{structural_envelope_for_html_node, StructuralEnvelopeKind};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertIntent {
    pub target_source_id: Option<String>,
    pub target_tag: Option<String>,
    pub target_kind: Option<String>,
    pub position: ProjectMovePosition,
    pub element: ProjectHtmlInsertElement,
    #[serde(default)]
    pub native_block_slot: Option<NativeBlockSlotMutationContext>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertElement {
    pub kind: Option<String>,
    pub block_id: Option<String>,
    pub tag: String,
    pub class_name: Option<String>,
    pub text: Option<String>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertPlan {
    pub allowed: bool,
    pub diagnostic: Option<String>,
    pub model_revision: String,
    pub patch: Option<ProjectHtmlInsertPatch>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHtmlInsertPatch {
    pub file: String,
    pub resolved_target_id: String,
    pub target_label: Option<String>,
    pub target_tag: Option<String>,
    pub inserted_label: String,
    pub before_revision: String,
    pub after_revision: String,
    pub contents: String,
    pub target_location: ProjectSourceEditLocation,
    pub inserted_location: ProjectSourceEditLocation,
    pub inserted_start_line: usize,
    pub position: ProjectMovePosition,
    pub target_start_line: usize,
    pub line_shift_start: usize,
    pub line_shift: isize,
    pub tag: String,
    pub class_name: String,
    pub text: String,
    pub html: String,
    pub block_id: Option<String>,
    pub data_anim: Option<String>,
    pub block_instance_id: Option<String>,
    #[serde(skip)]
    exact_edit: Option<SourceTextEdit>,
    #[serde(skip)]
    inserted_offset: Option<usize>,
    #[serde(skip)]
    inside_child_index: Option<usize>,
}

impl ProjectHtmlInsertPatch {
    pub(crate) fn exact_source_edit(&self) -> Option<SourceTextEdit> {
        self.exact_edit.clone()
    }

    pub(crate) fn inserted_offset(&self) -> Option<usize> {
        self.inserted_offset
    }

    pub(crate) fn inside_child_index(&self) -> Option<usize> {
        self.inside_child_index
    }
}

struct InsertApplication {
    contents: String,
    inserted_location: ProjectSourceEditLocation,
    inserted_start_line: usize,
    target_start_line: usize,
    line_shift_start: usize,
    line_shift: isize,
    exact_edit: Option<SourceTextEdit>,
    inserted_offset: Option<usize>,
}

pub fn plan_html_insert(
    model: &ProjectModel,
    intent: &ProjectHtmlInsertIntent,
    active_document_path: Option<&str>,
) -> ProjectHtmlInsertPlan {
    match plan_html_insert_inner(model, intent, active_document_path) {
        Ok(patch) => ProjectHtmlInsertPlan {
            allowed: true,
            diagnostic: None,
            model_revision: model.revision.clone(),
            patch: Some(patch),
        },
        Err(message) => ProjectHtmlInsertPlan {
            allowed: false,
            diagnostic: Some(message),
            model_revision: model.revision.clone(),
            patch: None,
        },
    }
}

fn plan_html_insert_inner(
    model: &ProjectModel,
    intent: &ProjectHtmlInsertIntent,
    active_document_path: Option<&str>,
) -> Result<ProjectHtmlInsertPatch, String> {
    let snippet = build_insert_snippet(model, intent)?;
    if let Some(context) = intent.native_block_slot.as_ref() {
        if intent.position != ProjectMovePosition::Inside {
            return Err(
                "Un item de slot nativ poate fi inserat doar în containerul slotului.".to_string(),
            );
        }
        validate_native_block_slot_insert(model, context, intent.target_source_id.as_deref())?;
    }
    let document_root_kind = intent.target_kind.as_deref().map(str::trim);
    if document_root_kind
        .is_some_and(|kind| matches!(kind, "empty-tera-slot" | "active-document-root"))
    {
        return plan_html_insert_into_active_document_root(
            model,
            intent,
            &snippet,
            active_document_path,
            document_root_kind == Some("active-document-root"),
        );
    }

    if let Some(target_node) = resolve_html_node_for_anchor(
        model,
        intent.target_source_id.as_deref(),
        intent.target_tag.as_deref(),
    ) {
        if node_is_slider_slot_container(model, target_node) && intent.native_block_slot.is_none() {
            return Err(
                "Slotul Slider este administrat exclusiv prin BlockPropertiesPane și intenții Rust tipizate."
                    .to_string(),
            );
        }
        if snippet.block_id.as_deref() == Some("slider")
            && (node_has_native_block_ancestor(model, target_node, "slider")
                || (intent.position == ProjectMovePosition::Inside
                    && node_is_native_block(model, target_node, "slider")))
        {
            return Err("Slider în slider este blocat de contractul Rust v1.".to_string());
        }
        return plan_html_insert_from_source_node(intent, &snippet, target_node, model);
    }

    Err(source_missing_message(
        "destinație",
        intent.target_source_id.as_deref(),
    ))
}

fn plan_html_insert_into_active_document_root(
    model: &ProjectModel,
    intent: &ProjectHtmlInsertIntent,
    snippet: &InsertSnippet,
    active_document_path: Option<&str>,
    accepts_existing_content: bool,
) -> Result<ProjectHtmlInsertPatch, String> {
    if intent.position != ProjectMovePosition::Inside {
        return Err("Rădăcina documentului activ acceptă inserări numai în interior.".to_string());
    }
    let active_document_path = active_document_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            "HTML Insert Engine nu poate confirma documentul activ pentru rădăcina de autor."
                .to_string()
        })?;
    let target_node = resolve_active_document_root_anchor(model, intent).ok_or_else(|| {
        source_missing_message(
            "rădăcina documentului activ",
            intent.target_source_id.as_deref(),
        )
    })?;
    if target_node.origin != SourceOrigin::Local
        || !same_model_path(&target_node.file, active_document_path)
    {
        return Err(format!(
            "Blocul Tera aparține sursei externe {}, nu documentului activ {}. Deschide sursa externă înainte de editare.",
            target_node.file, active_document_path
        ));
    }

    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &target_node.file))
        .ok_or_else(|| {
            format!(
                "Nu am găsit documentul activ {} în Project Model.",
                target_node.file
            )
        })?;
    if file.kind != ProjectModelFileKind::Template {
        return Err(
            "Rădăcina Tera editabilă este disponibilă numai în template-uri Zola.".to_string(),
        );
    }

    if matches!(
        target_node.kind,
        SourceNodeKind::Template | SourceNodeKind::Partial
    ) {
        let owner = model
            .source_graph
            .templates
            .iter()
            .find(|template| template.node_id == target_node.id)
            .ok_or_else(|| {
                "Rădăcina fragmentului activ nu mai aparține unui template din Source Graph."
                    .to_string()
            })?;
        if target_node.parent.is_some()
            || !same_model_path(&owner.file, active_document_path)
            || owner.origin != SourceOrigin::Local
        {
            return Err(
                "Rădăcina fragmentului nu este documentul local deschis în Workbench.".to_string(),
            );
        }
        if !accepts_existing_content && !file.contents.trim().is_empty() {
            return Err(
                "Rădăcina documentului nu mai este goală; reconstruiește Canvas-ul înaintea inserării."
                    .to_string(),
            );
        }
        if is_complete_html_document_source(&file.contents) {
            return Err(
                "Un document HTML complet se editează prin elementele sale reale, nu prin rădăcina de fragment."
                    .to_string(),
            );
        }
        let placement = StructuralPlacement::for_direct_target(&file.contents, 0);
        let inserted = format_html_fragment(&snippet.html, "", &placement.style)?;
        let applied = append_document_fragment(&file.contents, &inserted);
        let exact_edit = SourceTextEdit {
            old_start: applied.insertion_offset,
            old_end: applied.insertion_offset,
            new_start: applied.insertion_offset,
            new_end: applied.insertion_offset + applied.inserted_length,
        };
        return Ok(ProjectHtmlInsertPatch {
            file: target_node.file.clone(),
            resolved_target_id: target_node.id.clone(),
            target_label: None,
            target_tag: None,
            inserted_label: format!("<{}>", snippet.tag),
            before_revision: file.revision.clone(),
            after_revision: content_revision(&applied.contents),
            contents: applied.contents,
            target_location: source_location_at_offset(
                &file.contents,
                &target_node.file,
                target_node
                    .range
                    .as_ref()
                    .map(|range| range.start)
                    .unwrap_or(0),
            ),
            inserted_location: ProjectSourceEditLocation {
                file: target_node.file.clone(),
                line: applied.inserted_start_line,
                column: 1,
            },
            inserted_start_line: applied.inserted_start_line,
            position: intent.position,
            target_start_line: target_node
                .range
                .as_ref()
                .map(|range| range.line)
                .unwrap_or(1),
            line_shift_start: applied.inserted_start_line,
            line_shift: applied.line_shift,
            tag: snippet.tag.clone(),
            class_name: snippet.class_name.clone(),
            text: snippet.text.clone(),
            html: snippet.html.clone(),
            block_id: snippet.block_id.clone(),
            data_anim: snippet.data_anim.clone(),
            block_instance_id: snippet.block_instance_id.clone(),
            exact_edit: Some(exact_edit),
            inserted_offset: Some(applied.inserted_offset),
            inside_child_index: Some(target_node.children.len()),
        });
    }
    if target_node.kind != SourceNodeKind::Block {
        return Err(
            "Rădăcina documentului activ nu este un block sau un fragment editabil.".to_string(),
        );
    }
    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Blocul Tera activ nu are range stabil în Source Graph.".to_string())?;
    let target_source = file
        .contents
        .get(target_range.start..target_range.end)
        .ok_or_else(|| "Range-ul blocului Tera activ este invalid.".to_string())?;
    let items = parse_tera_items(target_source);
    let opening = items
        .iter()
        .find(|item| {
            item.kind == TeraItemKind::Node
                && item.node_kind == Some(SourceNodeKind::Block)
                && item.start == 0
        })
        .ok_or_else(|| "Range-ul activ nu mai începe cu un bloc Tera.".to_string())?;
    let closing = items
        .iter()
        .rev()
        .find(|item| item.kind == TeraItemKind::EndScope)
        .ok_or_else(|| "Blocul Tera activ nu mai are închidere stabilă.".to_string())?;
    let body = target_source
        .get(opening.end..closing.start)
        .ok_or_else(|| "Interiorul blocului Tera activ are un range invalid.".to_string())?;
    if !accepts_existing_content && !body.trim().is_empty() {
        return Err(
            "Slotul Tera nu mai este gol; reconstruiește Canvas-ul înaintea inserării.".to_string(),
        );
    }

    let placement = StructuralPlacement::for_direct_target(&file.contents, target_range.start);
    let block_indent = placement.indent.as_str();
    let child_indent = placement.child_indent();
    let inserted = format_html_fragment(&snippet.html, &child_indent, &placement.style)?;
    let opening_end = target_range.start + opening.end;
    let insert_at = target_range.start + closing.start;
    let before_insert = inside_prefix_for_insert(&file.contents, opening_end, insert_at);
    let inserted_start_line = line_number_at_offset(&before_insert, before_insert.len()) + 1;
    let contents = format!(
        "{}{}{}{}{}{}",
        before_insert,
        placement.style.line_ending(),
        inserted,
        placement.style.line_ending(),
        block_indent,
        &file.contents[insert_at..]
    );
    let replacement_length = placement.style.line_ending().len()
        + inserted.len()
        + placement.style.line_ending().len()
        + block_indent.len();

    Ok(ProjectHtmlInsertPatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        target_label: None,
        target_tag: None,
        inserted_label: format!("<{}>", snippet.tag),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&contents),
        contents,
        target_location: source_location_at_offset(
            &file.contents,
            &target_node.file,
            target_range.start,
        ),
        inserted_location: ProjectSourceEditLocation {
            file: target_node.file.clone(),
            line: inserted_start_line,
            column: child_indent.chars().count() + 1,
        },
        inserted_start_line,
        position: intent.position,
        target_start_line: target_range.line,
        line_shift_start: inserted_start_line,
        line_shift: snippet_line_count(&inserted) as isize + 1,
        tag: snippet.tag.clone(),
        class_name: snippet.class_name.clone(),
        text: snippet.text.clone(),
        html: snippet.html.clone(),
        block_id: snippet.block_id.clone(),
        data_anim: snippet.data_anim.clone(),
        block_instance_id: snippet.block_instance_id.clone(),
        exact_edit: Some(SourceTextEdit {
            old_start: before_insert.len(),
            old_end: insert_at,
            new_start: before_insert.len(),
            new_end: before_insert.len() + replacement_length,
        }),
        inserted_offset: Some(
            before_insert.len() + placement.style.line_ending().len() + child_indent.len(),
        ),
        inside_child_index: Some(target_node.children.len()),
    })
}

fn resolve_active_document_root_anchor<'a>(
    model: &'a ProjectModel,
    intent: &ProjectHtmlInsertIntent,
) -> Option<&'a SourceNode> {
    intent
        .target_source_id
        .as_deref()
        .and_then(|source_id| model.source_graph.node_by_id(source_id))
        .filter(|node| {
            matches!(
                node.kind,
                SourceNodeKind::Block | SourceNodeKind::Template | SourceNodeKind::Partial
            )
        })
}

fn is_complete_html_document_source(source: &str) -> bool {
    let normalized = source.trim_start().to_ascii_lowercase();
    normalized.starts_with("<!doctype html") || normalized.starts_with("<html")
}

fn plan_html_insert_from_source_node(
    intent: &ProjectHtmlInsertIntent,
    snippet: &InsertSnippet,
    target_node: &SourceNode,
    model: &ProjectModel,
) -> Result<ProjectHtmlInsertPatch, String> {
    if !target_node.capabilities.can_edit_visual {
        return Err(target_node
            .capabilities
            .technical_reason()
            .map(str::to_string)
            .unwrap_or_else(|| "Destinația nu este editabilă vizual.".to_string()));
    }

    let file = model
        .files
        .iter()
        .find(|file| same_model_path(&file.relative_path, &target_node.file))
        .ok_or_else(|| {
            format!(
                "Nu am găsit fișierul {} în Project Model.",
                target_node.file
            )
        })?;
    if file.kind != ProjectModelFileKind::Template {
        return Err(
            "HTML Insert Engine este activ doar pentru template-uri Zola/Tera.".to_string(),
        );
    }

    let target_range = target_node
        .range
        .as_ref()
        .ok_or_else(|| "Destinația nu are range stabil în Source Graph.".to_string())?;
    let envelope = structural_envelope_for_html_node(model, &file.contents, target_node)?;
    if intent.position == ProjectMovePosition::Inside
        && envelope.kind == StructuralEnvelopeKind::DynamicWidget
    {
        return Err(
            "Corpul unui widget dinamic este generat de contractul său. Adaugă elementul înainte sau după widget ori editează proprietățile widgetului."
                .to_string(),
        );
    }
    let target_span = if intent.position == ProjectMovePosition::Inside {
        resolve_html_element_span(&file.contents, target_range.start)?
    } else {
        envelope.span
    };
    let target_tag = html_tag_at(&file.contents, target_range.start)?;
    validate_insert_target(&target_tag, intent.position)?;

    let target_location =
        source_location_at_offset(&file.contents, &target_node.file, target_span.start);
    let placement = StructuralPlacement::for_html_target(model, &file.contents, target_node);
    let applied = apply_html_insert(
        &file.contents,
        &target_node.file,
        target_span,
        &target_tag,
        intent.position,
        &snippet.html,
        &placement,
    )?;

    Ok(ProjectHtmlInsertPatch {
        file: target_node.file.clone(),
        resolved_target_id: target_node.id.clone(),
        target_label: Some(target_node.label.clone()),
        target_tag: Some(target_tag.clone()),
        inserted_label: format!("<{}>", snippet.tag),
        before_revision: file.revision.clone(),
        after_revision: content_revision(&applied.contents),
        contents: applied.contents,
        target_location,
        inserted_location: applied.inserted_location,
        inserted_start_line: applied.inserted_start_line,
        position: intent.position,
        target_start_line: applied.target_start_line,
        line_shift_start: applied.line_shift_start,
        line_shift: applied.line_shift,
        tag: snippet.tag.clone(),
        class_name: snippet.class_name.clone(),
        text: snippet.text.clone(),
        html: snippet.html.clone(),
        block_id: snippet.block_id.clone(),
        data_anim: snippet.data_anim.clone(),
        block_instance_id: snippet.block_instance_id.clone(),
        exact_edit: applied.exact_edit,
        inserted_offset: applied.inserted_offset,
        inside_child_index: (intent.position == ProjectMovePosition::Inside)
            .then_some(target_node.children.len()),
    })
}

fn validate_insert_target(tag: &str, position: ProjectMovePosition) -> Result<(), String> {
    if tag.eq_ignore_ascii_case("html") {
        return Err("Elementul <html> nu este o destinație de inserare vizuală.".to_string());
    }
    if tag.eq_ignore_ascii_case("body") && position != ProjectMovePosition::Inside {
        return Err("Elementul <body> poate primi inserări doar în interior.".to_string());
    }
    if position == ProjectMovePosition::Inside
        && !tag.eq_ignore_ascii_case("body")
        && !can_receive_children(tag)
    {
        return Err(format!("<{tag}> nu este container pentru copii."));
    }
    Ok(())
}

struct InsertSnippet {
    tag: String,
    class_name: String,
    text: String,
    html: String,
    block_id: Option<String>,
    data_anim: Option<String>,
    block_instance_id: Option<String>,
}

fn build_insert_snippet(
    model: &ProjectModel,
    intent: &ProjectHtmlInsertIntent,
) -> Result<InsertSnippet, String> {
    let element = &intent.element;
    let kind = element
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .ok_or_else(|| "Inserarea HTML cere kind explicit html sau block.".to_string())?;
    if kind == "nativeBlockSlotItem" {
        let context = intent.native_block_slot.as_ref().ok_or_else(|| {
            "Inserarea unui item de slot nativ cere contextul Rust al slotului.".to_string()
        })?;
        if element.block_id.as_deref().map(str::trim) != Some(context.provider_id.trim()) {
            return Err("Providerul itemului nu corespunde contractului slotului.".to_string());
        }
        if !element.tag.trim().is_empty() && !element.tag.eq_ignore_ascii_case("div") {
            return Err("Rendererul Rust al slide-ului cere tag <div>.".to_string());
        }
        let html = render_native_block_slot_item_html(&context.provider_id, &context.slot_id)?;
        return Ok(InsertSnippet {
            tag: "div".to_string(),
            class_name: "slider__slide".to_string(),
            text: String::new(),
            html,
            block_id: None,
            data_anim: None,
            block_instance_id: None,
        });
    }
    if kind == "block" {
        return build_native_block_insert_snippet(model, element);
    }
    if kind != "html" {
        return Err(format!(
            "Inserarea HTML nu acceptă element kind {kind}; sunt permise html și block."
        ));
    }
    if element
        .block_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err("Elementul html nu poate declara blockId.".to_string());
    }

    let tag = normalize_tag(&element.tag)?;
    let class_name = normalize_class_name(element.class_name.as_deref().unwrap_or(""));
    let text = normalize_text(element.text.as_deref().unwrap_or(""));
    Ok(InsertSnippet {
        html: build_html_snippet(&tag, &class_name, &text),
        tag,
        class_name,
        text,
        block_id: None,
        data_anim: None,
        block_instance_id: None,
    })
}

fn build_native_block_insert_snippet(
    model: &ProjectModel,
    element: &ProjectHtmlInsertElement,
) -> Result<InsertSnippet, String> {
    let block_id = element
        .block_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Inserarea blocului nu a primit blockId.".to_string())?;
    let block = native_block_by_id(block_id)
        .ok_or_else(|| format!("Blocul {block_id} nu există în NativeBlockRegistry Rust."))?;
    let provided_tag = element.tag.trim().to_ascii_lowercase();
    if !provided_tag.is_empty() && provided_tag != block.tag {
        return Err(format!(
            "Blocul {} cere tag <{}>, dar UI-ul a cerut <{}>.",
            block.id, block.tag, provided_tag
        ));
    }

    let seed = format!(
        "{}:{}:{}:{}",
        model.revision,
        block.id,
        element.label.as_deref().unwrap_or(block.label),
        block.tag
    );
    let identity = unique_native_block_identity(block.id, &seed, |candidate| {
        model
            .files
            .iter()
            .any(|file| file.contents.contains(candidate))
    });
    let html = render_native_block_html(block, &identity);

    Ok(InsertSnippet {
        tag: block.tag.to_string(),
        class_name: native_block_root_class_name(block, &identity),
        text: block.text.to_string(),
        html,
        block_id: Some(block.id.to_string()),
        data_anim: Some(identity.data_anim),
        block_instance_id: Some(identity.instance_id),
    })
}

fn apply_html_insert(
    source: &str,
    file: &str,
    target_span: Span,
    target_tag: &str,
    position: ProjectMovePosition,
    snippet: &str,
    placement: &StructuralPlacement,
) -> Result<InsertApplication, String> {
    let target_indent = placement.indent.as_str();
    match position {
        ProjectMovePosition::Before => {
            let inserted = format_html_fragment(snippet, target_indent, &placement.style)?;
            let insert_at = line_block_before_index(source, target_span.start);
            let inserted_start_line = inserted_block_start_line(source, insert_at);
            let contents = insert_line_block(source, insert_at, &inserted);
            let inserted_length = contents.len().saturating_sub(source.len());
            let leading_break_length =
                usize::from(insert_at > 0 && source.as_bytes().get(insert_at - 1) != Some(&b'\n'))
                    * placement.style.line_ending().len();
            let target_offset = target_span.start.saturating_add(inserted_length);
            Ok(InsertApplication {
                target_start_line: line_number_at_offset(&contents, target_offset),
                contents,
                inserted_location: ProjectSourceEditLocation {
                    file: file.to_string(),
                    line: inserted_start_line,
                    column: target_indent.chars().count() + 1,
                },
                inserted_start_line,
                line_shift_start: inserted_start_line,
                line_shift: snippet_line_count(&inserted) as isize,
                exact_edit: Some(SourceTextEdit {
                    old_start: insert_at,
                    old_end: insert_at,
                    new_start: insert_at,
                    new_end: insert_at + inserted_length,
                }),
                inserted_offset: Some(insert_at + leading_break_length + target_indent.len()),
            })
        }
        ProjectMovePosition::After => {
            let inserted = format_html_fragment(snippet, target_indent, &placement.style)?;
            let insert_at = line_block_after_index(source, target_span.end);
            let inserted_start_line = inserted_block_start_line(source, insert_at);
            let contents = insert_line_block(source, insert_at, &inserted);
            let inserted_length = contents.len().saturating_sub(source.len());
            let leading_break_length =
                usize::from(insert_at > 0 && source.as_bytes().get(insert_at - 1) != Some(&b'\n'))
                    * placement.style.line_ending().len();
            Ok(InsertApplication {
                target_start_line: line_number_at_offset(&contents, target_span.start),
                contents,
                inserted_location: ProjectSourceEditLocation {
                    file: file.to_string(),
                    line: inserted_start_line,
                    column: target_indent.chars().count() + 1,
                },
                inserted_start_line,
                line_shift_start: inserted_start_line,
                line_shift: snippet_line_count(&inserted) as isize,
                exact_edit: Some(SourceTextEdit {
                    old_start: insert_at,
                    old_end: insert_at,
                    new_start: insert_at,
                    new_end: insert_at + inserted_length,
                }),
                inserted_offset: Some(insert_at + leading_break_length + target_indent.len()),
            })
        }
        ProjectMovePosition::Inside => {
            let target_source = source
                .get(target_span.start..target_span.end)
                .ok_or_else(|| "Range destinație invalid pentru inserare.".to_string())?;
            let close_tag = format!("</{target_tag}>");
            let close_offset = target_source
                .to_ascii_lowercase()
                .rfind(&close_tag.to_ascii_lowercase())
                .ok_or_else(|| format!("Nu am găsit {close_tag} pentru inserare."))?;
            let opening = parse_html_tag_at(source, target_span.start).ok_or_else(|| {
                "Nu am putut reciti tag-ul destinație pentru inserare.".to_string()
            })?;
            let child_indent = placement.child_indent();
            let inserted = format_html_fragment(snippet, &child_indent, &placement.style)?;
            let insert_at = target_span.start + close_offset;
            let before_insert = inside_prefix_for_insert(source, opening.end, insert_at);
            let inserted_start_line =
                line_number_at_offset(&before_insert, before_insert.len()) + 1;
            let next_contents = format!(
                "{}{}{}{}{}{}",
                before_insert,
                placement.style.line_ending(),
                inserted,
                placement.style.line_ending(),
                target_indent,
                &source[insert_at..]
            );
            let contents = normalize_html_subtree(
                &next_contents,
                target_span.start,
                target_indent,
                &placement.style,
            )?;
            let exact_edit = (contents == next_contents).then_some(SourceTextEdit {
                old_start: before_insert.len(),
                old_end: insert_at,
                new_start: before_insert.len(),
                new_end: before_insert.len()
                    + placement.style.line_ending().len()
                    + inserted.len()
                    + placement.style.line_ending().len()
                    + target_indent.len(),
            });
            let inserted_offset = exact_edit.as_ref().map(|_| {
                before_insert.len() + placement.style.line_ending().len() + child_indent.len()
            });
            Ok(InsertApplication {
                target_start_line: line_number_at_offset(&contents, target_span.start),
                contents,
                inserted_location: ProjectSourceEditLocation {
                    file: file.to_string(),
                    line: inserted_start_line,
                    column: child_indent.chars().count() + 1,
                },
                inserted_start_line,
                line_shift_start: inserted_start_line,
                line_shift: snippet_line_count(&inserted) as isize + 1,
                exact_edit,
                inserted_offset,
            })
        }
    }
}

fn normalize_tag(value: &str) -> Result<String, String> {
    let tag = value.trim().to_ascii_lowercase();
    let mut chars = tag.chars();
    let Some(first) = chars.next() else {
        return Err("HTML Insert Engine a primit tag gol.".to_string());
    };
    if !first.is_ascii_lowercase() {
        return Err(format!("HTML Insert Engine a primit tag invalid: {value}."));
    }
    if !chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    }) {
        return Err(format!("HTML Insert Engine a primit tag invalid: {value}."));
    }
    Ok(tag)
}

fn normalize_class_name(value: &str) -> String {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty() && !token.contains('\0'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_text(value: &str) -> String {
    value.trim().chars().take(4000).collect()
}

fn snippet_line_count(value: &str) -> usize {
    value.split('\n').count()
}

fn build_html_snippet(tag: &str, class_name: &str, text: &str) -> String {
    let attrs = root_attrs(class_name);
    if tag == "a" {
        return format!("<a{attrs} href=\"#\">{}</a>", text_or(text, "Link nou"));
    }
    if tag == "button" {
        return format!(
            "<button{attrs} type=\"button\">{}</button>",
            text_or(text, "Buton nou")
        );
    }
    if tag == "hgroup" {
        return format!(
            "<hgroup{attrs}><h2>{}</h2><p>Subtitlu</p></hgroup>",
            text_or(text, "Titlu grup")
        );
    }
    if tag == "details" {
        return format!(
            "<details{attrs}><summary>{}</summary><p>Conținut detalii</p></details>",
            text_or(text, "Detalii")
        );
    }
    if tag == "dialog" {
        return format!(
            "<dialog{attrs} open>{}</dialog>",
            text_or(text, "Dialog nou")
        );
    }
    if tag == "bdo" {
        return format!("<bdo{attrs} dir=\"ltr\">{}</bdo>", text_or(text, "Text"));
    }
    if tag == "data" {
        return format!(
            "<data{attrs} value=\"\">{}</data>",
            text_or(text, "Valoare")
        );
    }
    if tag == "ruby" {
        return format!(
            "<ruby{attrs}>{}<rp> (</rp><rt>Pronunție</rt><rp>)</rp></ruby>",
            text_or(text, "Text")
        );
    }
    if tag == "img" {
        return format!(
            "<img{attrs} src=\"\" alt=\"{}\">",
            escape_attr(text_or_raw(text, "Imagine"))
        );
    }
    if tag == "input" {
        return format!(
            "<input{attrs} type=\"text\" placeholder=\"{}\">",
            escape_attr(text_or_raw(text, "Text"))
        );
    }
    if tag == "source" {
        return format!("<source{attrs} src=\"\" type=\"\">");
    }
    if tag == "track" {
        return format!(
            "<track{attrs} src=\"\" kind=\"captions\" srclang=\"ro\" label=\"Română\">"
        );
    }
    if tag == "video" {
        return format!("<video{attrs} controls></video>");
    }
    if tag == "audio" {
        return format!("<audio{attrs} controls></audio>");
    }
    if tag == "iframe" {
        return format!(
            "<iframe{attrs} src=\"\" title=\"{}\"></iframe>",
            escape_attr(text_or_raw(text, "Iframe"))
        );
    }
    if tag == "canvas" {
        return format!(
            "<canvas{attrs} width=\"300\" height=\"150\">{}</canvas>",
            text_or(text, "Canvas indisponibil")
        );
    }
    if tag == "object" {
        return format!(
            "<object{attrs} data=\"\" type=\"\"><p>{}</p></object>",
            text_or(text, "Conținut indisponibil")
        );
    }
    if tag == "embed" {
        return format!("<embed{attrs} src=\"\" type=\"\">");
    }
    if tag == "map" {
        return format!(
            "<map{attrs} name=\"harta\"><area shape=\"rect\" coords=\"\" href=\"#\" alt=\"Zonă\"></map>"
        );
    }
    if tag == "area" {
        return format!(
            "<area{attrs} shape=\"rect\" coords=\"\" href=\"#\" alt=\"{}\">",
            escape_attr(text_or_raw(text, "Zonă"))
        );
    }
    if tag == "picture" {
        return format!(
            "<picture{attrs}><img src=\"\" alt=\"{}\"></picture>",
            escape_attr(text_or_raw(text, "Imagine"))
        );
    }
    if tag == "template" {
        return format!(
            "<template{attrs}><div>{}</div></template>",
            text_or(text, "Conținut șablon")
        );
    }
    if tag == "slot" {
        return format!("<slot{attrs}>{}</slot>", text_or(text, "Conținut implicit"));
    }
    if tag == "ul" {
        return format!(
            "<ul{attrs}><li>{}</li></ul>",
            text_or(text, "Element listă")
        );
    }
    if tag == "ol" {
        return format!(
            "<ol{attrs}><li>{}</li></ol>",
            text_or(text, "Element listă")
        );
    }
    if tag == "menu" {
        return format!(
            "<menu{attrs}><li>{}</li></menu>",
            text_or(text, "Element meniu")
        );
    }
    if tag == "dl" {
        return format!(
            "<dl{attrs}><dt>{}</dt><dd>Descriere</dd></dl>",
            text_or(text, "Termen")
        );
    }
    if tag == "form" {
        return format!(
            "<form{attrs}><button type=\"submit\">{}</button></form>",
            text_or(text, "Trimite")
        );
    }
    if tag == "textarea" {
        return format!(
            "<textarea{attrs} placeholder=\"{}\"></textarea>",
            escape_attr(text_or_raw(text, "Text"))
        );
    }
    if tag == "select" {
        return format!(
            "<select{attrs}><option>{}</option></select>",
            text_or(text, "Opțiune")
        );
    }
    if tag == "optgroup" {
        return format!(
            "<optgroup{attrs} label=\"{}\"><option>Opțiune</option></optgroup>",
            escape_attr(text_or_raw(text, "Grup"))
        );
    }
    if tag == "datalist" {
        return format!(
            "<datalist{attrs}><option value=\"{}\"></option></datalist>",
            escape_attr(text_or_raw(text, "Opțiune"))
        );
    }
    if tag == "fieldset" {
        return format!(
            "<fieldset{attrs}><legend>{}</legend></fieldset>",
            text_or(text, "Legendă")
        );
    }
    if tag == "progress" {
        return format!("<progress{attrs} value=\"0\" max=\"100\">0%</progress>");
    }
    if tag == "meter" {
        return format!("<meter{attrs} min=\"0\" max=\"100\" value=\"0\">0</meter>");
    }
    if tag == "table" {
        return format!(
            "<table{attrs}><tbody><tr><td>{}</td></tr></tbody></table>",
            text_or(text, "Celulă")
        );
    }
    if tag == "colgroup" {
        return format!("<colgroup{attrs}><col></colgroup>");
    }
    if tag == "thead" {
        return format!(
            "<thead{attrs}><tr><th>{}</th></tr></thead>",
            text_or(text, "Titlu")
        );
    }
    if tag == "tbody" {
        return format!(
            "<tbody{attrs}><tr><td>{}</td></tr></tbody>",
            text_or(text, "Celulă")
        );
    }
    if tag == "tfoot" {
        return format!(
            "<tfoot{attrs}><tr><td>{}</td></tr></tfoot>",
            text_or(text, "Total")
        );
    }
    if tag == "tr" {
        return format!("<tr{attrs}><td>{}</td></tr>", text_or(text, "Celulă"));
    }
    if tag == "th" {
        return format!("<th{attrs}>{}</th>", text_or(text, "Titlu"));
    }
    if tag == "td" {
        return format!("<td{attrs}>{}</td>", text_or(text, "Celulă"));
    }
    if tag == "caption" {
        return format!(
            "<caption{attrs}>{}</caption>",
            text_or(text, "Descriere tabel")
        );
    }
    if is_void_snippet_tag(tag) {
        return format!("<{tag}{attrs}>");
    }
    format!(
        "<{tag}{attrs}>{}</{tag}>",
        if text.trim().is_empty() {
            String::new()
        } else {
            escape_text(text)
        }
    )
}

fn root_attrs(class_name: &str) -> String {
    if class_name.trim().is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", escape_attr(class_name))
    }
}

fn text_or(value: &str, fallback: &str) -> String {
    escape_text(text_or_raw(value, fallback))
}

fn text_or_raw<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    }
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(value: &str) -> String {
    escape_text(value).replace('"', "&quot;")
}

fn is_void_snippet_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::project_model::{
        delete_engine::plan_html_delete, duplicate_engine::plan_html_duplicate,
        move_engine::plan_html_move, test_support::ProjectModelTestFixture,
    };

    use super::*;

    #[test]
    fn plan_html_insert_inserts_child_with_project_model_anchor() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% block content %}\n",
                "<section class=\"hero\">\n",
                "  <h1>Titlu</h1>\n",
                "</section>\n",
                "{% endblock %}\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section .hero>")
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_tag: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "p".to_string(),
                    class_name: Some("lede".to_string()),
                    text: Some("Salut".to_string()),
                    label: Some("Paragraph".to_string()),
                },
                native_block_slot: None,
            },
            None,
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert!(patch.contents.contains("  <p class=\"lede\">Salut</p>"));
        assert_eq!(patch.inserted_location.line, 4);
        assert_eq!(patch.tag, "p");
    }

    #[test]
    fn plan_html_insert_populates_empty_block_owned_by_active_document() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% extends \"layout.html\" %}\n\n",
                "{% block content %}\n\n",
                "{% endblock content %}\n",
            ),
        )
        .unwrap();
        fixture.source(
            "templates/layout.html",
            "<body>{% block content %}{% endblock content %}</body>\n",
        );
        let model = fixture.build_model().unwrap();
        let content = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Block
                    && node.file == "templates/index.html"
                    && node.label == "content"
            })
            .unwrap();
        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(content.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("empty-tera-slot".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "section".to_string(),
                    class_name: Some("servicii".to_string()),
                    text: None,
                    label: Some("Secțiune".to_string()),
                },
                native_block_slot: None,
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.file, "templates/index.html");
        assert!(patch.contents.contains(concat!(
            "{% block content %}\n",
            "  <section class=\"servicii\"></section>\n",
            "{% endblock content %}",
        )));
        assert_eq!(patch.inserted_location.line, 4);
        assert_eq!(patch.inserted_location.column, 3);
    }

    #[test]
    fn plan_html_insert_appends_repeatedly_to_active_document_root() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "{% extends \"layout.html\" %}\n\n",
                "{% block content %}\n",
                "  <div class=\"primul\"></div>\n",
                "{% endblock content %}\n",
            ),
        )
        .unwrap();
        fixture.source(
            "templates/layout.html",
            "<body>{% block content %}{% endblock content %}</body>\n",
        );
        let model = fixture.build_model().unwrap();
        let content = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Block
                    && node.file == "templates/index.html"
                    && node.label == "content"
            })
            .unwrap();
        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(content.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("active-document-root".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "section".to_string(),
                    class_name: Some("al-doilea".to_string()),
                    text: None,
                    label: Some("Secțiune".to_string()),
                },
                native_block_slot: None,
            },
            Some("templates/index.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let contents = plan.patch.unwrap().contents;
        assert!(contents.contains(concat!(
            "{% block content %}\n",
            "  <div class=\"primul\"></div>\n",
            "  <section class=\"al-doilea\"></section>\n",
            "{% endblock content %}",
        )));
    }

    #[test]
    fn plan_html_insert_appends_repeatedly_to_direct_fragment_root() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        fixture.source("templates/listing-items/card.html", "\n");
        let model = fixture.build_model().unwrap();
        let fragment = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Partial
                    && node.file == "templates/listing-items/card.html"
                    && node.parent.is_none()
            })
            .expect("listing item root");
        let first = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(fragment.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("active-document-root".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "article".to_string(),
                    class_name: Some("card".to_string()),
                    text: None,
                    label: Some("Article".to_string()),
                },
                native_block_slot: None,
            },
            Some("templates/listing-items/card.html"),
        );
        assert!(first.allowed, "{:?}", first.diagnostic);
        let first_contents = first.patch.expect("first patch").contents;
        assert_eq!(first_contents, "<article class=\"card\"></article>\n");
        fixture.draft("templates/listing-items/card.html", &first_contents);
        let model = fixture.build_model().unwrap();
        let fragment = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Partial
                    && node.file == "templates/listing-items/card.html"
                    && node.parent.is_none()
            })
            .expect("rebuilt listing item root");
        let second = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(fragment.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("active-document-root".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "p".to_string(),
                    class_name: None,
                    text: Some("Descriere".to_string()),
                    label: Some("Paragraph".to_string()),
                },
                native_block_slot: None,
            },
            Some("templates/listing-items/card.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(second.allowed, "{:?}", second.diagnostic);
        let contents = second.patch.expect("second patch").contents;
        assert_eq!(
            contents,
            "<article class=\"card\"></article>\n<p>Descriere</p>\n"
        );
        assert!(!contents.contains("data-pana-active-document-root"));
    }

    #[test]
    fn plan_html_insert_refuses_empty_block_from_external_document() {
        let root = unique_test_dir();
        let mut fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            "{% block content %}\n\n{% endblock content %}\n",
        )
        .unwrap();
        fixture.source(
            "templates/arhiva.html",
            "{% block content %}\n\n{% endblock content %}\n",
        );
        let model = fixture.build_model().unwrap();
        let external_content = model
            .source_graph
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Block
                    && node.file == "templates/index.html"
                    && node.label == "content"
            })
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(external_content.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("empty-tera-slot".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "section".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Secțiune".to_string()),
                },
                native_block_slot: None,
            },
            Some("templates/arhiva.html"),
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.diagnostic.unwrap().contains("sursei externe"));
    }

    #[test]
    fn plan_html_insert_renders_registered_block_from_rust_registry() {
        let root = unique_test_dir();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<section></section>\n").unwrap();
        let model = fixture.build_model().unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section>")
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_tag: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("block".to_string()),
                    block_id: Some("counter".to_string()),
                    tag: "span".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Counter".to_string()),
                },
                native_block_slot: None,
            },
            None,
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.block_id.as_deref(), Some("counter"));
        assert!(patch.html.contains(r#"data-pana-block="counter""#));
        assert!(patch.html.contains("ps-counter-"));
        assert!(!patch.html.contains("__PANA_"));
        assert!(patch
            .contents
            .contains(r#"data-pana-instance="counter-counter-"#));
    }

    #[test]
    fn plan_html_insert_renders_atomic_icon_from_rust_registry() {
        let root = unique_test_dir();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<section></section>\n").unwrap();
        let model = fixture.build_model().unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section>")
            .unwrap();
        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_tag: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("block".to_string()),
                    block_id: Some("icon".to_string()),
                    tag: "svg".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Icon".to_string()),
                },
                native_block_slot: None,
            },
            None,
        );
        fs::remove_dir_all(&root).unwrap();

        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        assert_eq!(patch.block_id.as_deref(), Some("icon"));
        assert_eq!(patch.tag, "svg");
        assert!(patch
            .html
            .contains("data-pana-icon=\"tabler-outline:home\""));
        assert!(patch.html.contains("stroke=\"currentColor\""));
        assert!(patch.html.contains("<path d=\""));
        assert!(!patch.html.contains("<script"));
        assert!(patch.contents.contains("data-pana-block=\"icon\""));
    }

    #[test]
    fn plan_html_insert_renders_every_complete_native_block_from_registry() {
        for (block_id, tag, expected_structure) in [
            ("accordion", "div", "accordion__trigger"),
            ("tabs", "div", "tabs__tab"),
            ("dialog", "div", "dialog__panel"),
            ("offcanvas", "div", "offcanvas__panel"),
            ("nav-menu", "nav", "nav-menu__toggle"),
        ] {
            let root = unique_test_dir();
            let fixture =
                ProjectModelTestFixture::standard_zola(root.clone(), "<section></section>\n")
                    .unwrap();
            let model = fixture.build_model().unwrap();
            let section = model
                .source_graph
                .nodes
                .iter()
                .find(|node| node.label == "<section>")
                .unwrap();

            let plan = plan_html_insert(
                &model,
                &ProjectHtmlInsertIntent {
                    target_source_id: Some(section.id.clone()),
                    target_tag: Some("section".to_string()),
                    target_kind: Some("html".to_string()),
                    position: ProjectMovePosition::Inside,
                    element: ProjectHtmlInsertElement {
                        kind: Some("block".to_string()),
                        block_id: Some(block_id.to_string()),
                        tag: tag.to_string(),
                        class_name: None,
                        text: None,
                        label: Some(block_id.to_string()),
                    },
                    native_block_slot: None,
                },
                None,
            );

            fs::remove_dir_all(&root).unwrap();
            assert!(plan.allowed, "{block_id}: {:?}", plan.diagnostic);
            let patch = plan.patch.unwrap();
            assert_eq!(patch.block_id.as_deref(), Some(block_id));
            assert!(
                patch
                    .html
                    .contains(&format!(r#"data-pana-block="{block_id}""#)),
                "{block_id} nu are markerul canonic"
            );
            assert!(
                patch.html.contains(expected_structure),
                "{block_id} a pierdut structura {expected_structure}"
            );
            assert!(patch.contents.contains(expected_structure));
        }
    }

    #[test]
    fn plan_html_insert_rejects_location_without_source_id() {
        let root = unique_test_dir();
        let mut fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<main></main>\n").unwrap();
        fixture.source(
            "static/plain.html",
            concat!(
                "<!DOCTYPE html>\n",
                "<html>\n",
                "<body>\n",
                "  <section id=\"hero\">\n",
                "  </section>\n",
                "</body>\n",
                "</html>\n",
            ),
        );
        let model = fixture.build_model().unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: None,
                target_tag: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "p".to_string(),
                    class_name: Some("lede".to_string()),
                    text: Some("Salut".to_string()),
                    label: Some("Paragraph".to_string()),
                },
                native_block_slot: None,
            },
            None,
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan.patch.is_none());
    }

    #[test]
    fn plan_html_insert_blocks_unknown_block_id() {
        let root = unique_test_dir();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<section></section>\n").unwrap();
        let model = fixture.build_model().unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section>")
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_tag: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("block".to_string()),
                    block_id: Some("hero-card".to_string()),
                    tag: "section".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Hero Card".to_string()),
                },
                native_block_slot: None,
            },
            None,
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("NativeBlockRegistry Rust"));
    }

    #[test]
    fn plan_html_insert_rejects_legacy_component_kind() {
        let root = unique_test_dir();
        let fixture =
            ProjectModelTestFixture::standard_zola(root.clone(), "<section></section>\n").unwrap();
        let model = fixture.build_model().unwrap();
        let section = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<section>")
            .unwrap();

        let plan = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(section.id.clone()),
                target_tag: Some("section".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("component".to_string()),
                    block_id: Some("counter".to_string()),
                    tag: "span".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Contor legacy".to_string()),
                },
                native_block_slot: None,
            },
            None,
        );

        fs::remove_dir_all(&root).unwrap();
        assert!(!plan.allowed);
        assert!(plan
            .diagnostic
            .unwrap()
            .contains("sunt permise html și block"));
    }

    #[test]
    fn plan_html_insert_respects_dynamic_widget_boundaries() {
        let root = unique_test_dir();
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            concat!(
                "<main>\n",
                "  {# pana:widget schema=2 provider=dynamic-field instance=dynamic-field-insert01 props=00 #}\n",
                "  <div data-pana-widget-instance=\"dynamic-field-insert01\"><span>Valoare</span></div>\n",
                "  {# /pana:widget instance=dynamic-field-insert01 #}\n",
                "</main>\n",
            ),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let widget_root = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label.starts_with("<div"))
            .unwrap();
        let intent = |position| ProjectHtmlInsertIntent {
            target_source_id: Some(widget_root.id.clone()),
            target_tag: Some("div".to_string()),
            target_kind: Some("html".to_string()),
            position,
            element: ProjectHtmlInsertElement {
                kind: Some("html".to_string()),
                block_id: None,
                tag: "p".to_string(),
                class_name: None,
                text: Some("Nou".to_string()),
                label: Some("Paragraf".to_string()),
            },
            native_block_slot: None,
        };

        let before = plan_html_insert(&model, &intent(ProjectMovePosition::Before), None);
        let inside = plan_html_insert(&model, &intent(ProjectMovePosition::Inside), None);

        fs::remove_dir_all(&root).unwrap();
        assert!(before.allowed, "{:?}", before.diagnostic);
        let contents = before.patch.unwrap().contents;
        assert!(contents.find("<p>Nou</p>").unwrap() < contents.find("{# pana:widget").unwrap());
        assert_eq!(contents.matches("dynamic-field-insert01").count(), 3);
        assert!(!inside.allowed);
        assert!(inside
            .diagnostic
            .unwrap()
            .contains("Corpul unui widget dinamic"));
    }

    #[test]
    fn slider_slot_insert_is_rust_rendered_and_nested_slider_is_blocked() {
        let root = unique_test_dir();
        let slider = native_block_by_id("slider").unwrap();
        let identity = crate::blocks::native::NativeBlockIdentity {
            class_name: "ps-slider-test0001".to_string(),
            data_anim: "ps-slider-test0001".to_string(),
            instance_id: "slider-test0001".to_string(),
        };
        let markup = render_native_block_html(slider, &identity);
        let fixture = ProjectModelTestFixture::standard_zola(
            root.clone(),
            format!("<main>\n{markup}\n</main>\n"),
        )
        .unwrap();
        let model = fixture.build_model().unwrap();
        let slider_marker = model
            .source_graph
            .block_graph
            .source_instances
            .iter()
            .find(|instance| instance.provider_id == "slider")
            .and_then(|instance| {
                model
                    .source_graph
                    .nodes
                    .iter()
                    .find(|node| node.id == instance.source_node_id)
            })
            .unwrap();
        let slider_root = model
            .source_graph
            .nodes
            .iter()
            .find(|node| Some(node.id.as_str()) == slider_marker.parent.as_deref())
            .unwrap();
        let track = model
            .source_graph
            .nodes
            .iter()
            .find(|node| node.label == "<div .slider__track>")
            .unwrap();
        let slides = model
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.label == "<div .slider__slide>")
            .collect::<Vec<_>>();
        assert_eq!(slides.len(), 2);
        let slide = slides[0];
        let context = NativeBlockSlotMutationContext {
            provider_id: "slider".to_string(),
            slot_id: "slides".to_string(),
            root_source_id: slider_root.id.clone(),
            expected_model_revision: model.revision.clone(),
        };
        let slot_item = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(track.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("nativeBlockSlotItem".to_string()),
                    block_id: Some("slider".to_string()),
                    tag: "div".to_string(),
                    class_name: Some("evil-class".to_string()),
                    text: Some("<script>evil()</script>".to_string()),
                    label: None,
                },
                native_block_slot: Some(context.clone()),
            },
            None,
        );
        assert!(slot_item.allowed, "{:?}", slot_item.diagnostic);
        let patch = slot_item.patch.unwrap();
        assert!(patch.html.contains("data-pana-slider-slide"));
        assert!(!patch.html.contains("evil"));

        let generic_insert = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(track.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("html".to_string()),
                    block_id: None,
                    tag: "div".to_string(),
                    class_name: Some("fake-slide".to_string()),
                    text: None,
                    label: None,
                },
                native_block_slot: None,
            },
            None,
        );
        assert!(!generic_insert.allowed);

        let duplicate = plan_html_duplicate(
            &model,
            &crate::project_model::duplicate_engine::ProjectHtmlDuplicateIntent {
                source_source_id: Some(slides[0].id.clone()),
                source_tag: Some("div".to_string()),
                native_block_slot: Some(context.clone()),
            },
        );
        assert!(duplicate.allowed, "{:?}", duplicate.diagnostic);
        let generic_delete = plan_html_delete(
            &model,
            &crate::project_model::delete_engine::ProjectHtmlDeleteIntent {
                target_source_id: Some(slides[0].id.clone()),
                target_render_instance_id: None,
                target_tag: Some("div".to_string()),
                native_block_slot: None,
            },
        );
        assert!(!generic_delete.allowed);
        let delete = plan_html_delete(
            &model,
            &crate::project_model::delete_engine::ProjectHtmlDeleteIntent {
                target_source_id: Some(slides[0].id.clone()),
                target_render_instance_id: None,
                target_tag: Some("div".to_string()),
                native_block_slot: Some(context.clone()),
            },
        );
        assert!(delete.allowed, "{:?}", delete.diagnostic);
        let moved = plan_html_move(
            &model,
            &crate::project_model::move_engine::ProjectHtmlMoveIntent {
                source_source_id: Some(slides[0].id.clone()),
                target_source_id: Some(slides[1].id.clone()),
                source_tag: Some("div".to_string()),
                target_tag: Some("div".to_string()),
                position: ProjectMovePosition::After,
                native_block_slot: Some(context),
            },
        );
        assert!(moved.allowed, "{:?}", moved.diagnostic);

        let nested = plan_html_insert(
            &model,
            &ProjectHtmlInsertIntent {
                target_source_id: Some(slide.id.clone()),
                target_tag: Some("div".to_string()),
                target_kind: Some("html".to_string()),
                position: ProjectMovePosition::Inside,
                element: ProjectHtmlInsertElement {
                    kind: Some("block".to_string()),
                    block_id: Some("slider".to_string()),
                    tag: "div".to_string(),
                    class_name: None,
                    text: None,
                    label: Some("Slider".to_string()),
                },
                native_block_slot: None,
            },
            None,
        );
        fs::remove_dir_all(&root).unwrap();
        assert!(!nested.allowed);
        assert!(nested.diagnostic.unwrap().contains("Slider în slider"));
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-studio-insert-engine-{}-{stamp}",
            std::process::id()
        ))
    }
}
