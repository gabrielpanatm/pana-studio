use crate::{
    kernel::dynamic_widgets::DynamicWidgetSourceInstance, project_model::model::ProjectModel,
    source_graph::model::SourceNode,
};

use super::move_engine::{resolve_html_element_span, same_model_path, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StructuralEnvelopeKind {
    HtmlElement,
    DynamicWidget,
}

#[derive(Clone, Copy)]
pub(super) struct StructuralEnvelope<'a> {
    pub span: Span,
    pub opening_start: usize,
    pub kind: StructuralEnvelopeKind,
    pub dynamic_widget: Option<&'a DynamicWidgetSourceInstance>,
}

impl StructuralEnvelope<'_> {
    pub fn preserves_internal_indentation(self) -> bool {
        self.kind == StructuralEnvelopeKind::DynamicWidget
    }
}

/// Resolves the smallest source unit which may be moved, deleted or duplicated
/// without separating generated HTML from the Rust-owned contract that created
/// it. A dynamic widget is one atomic envelope:
/// start marker + generated body + end marker.
pub(super) fn structural_envelope_for_html_node<'a>(
    model: &'a ProjectModel,
    source: &str,
    node: &SourceNode,
) -> Result<StructuralEnvelope<'a>, String> {
    let range = node
        .range
        .as_ref()
        .ok_or_else(|| "Elementul HTML nu are range stabil în Source Graph.".to_string())?;
    let html_span = resolve_html_element_span(source, range.start)?;

    if let Some(instance) = dynamic_widget_for_node(model, node) {
        if instance.range.start > instance.range.end || instance.range.end > source.len() {
            return Err(format!(
                "Contractul widgetului dinamic {} are un range invalid; reconstruiește proiecția înaintea mutației.",
                instance.instance_id
            ));
        }
        return Ok(StructuralEnvelope {
            span: Span {
                start: instance.range.start,
                end: instance.range.end,
            },
            opening_start: range.start,
            kind: StructuralEnvelopeKind::DynamicWidget,
            dynamic_widget: Some(instance),
        });
    }

    if let Some(instance_id) = dynamic_widget_instance_attribute(source, html_span) {
        return Err(format!(
            "Elementul declară instanța dinamică {instance_id}, dar corpul său nu mai este cuprins între markerii contractului. Deschide sursa în Cod și repară markerii înaintea unei mutații structurale."
        ));
    }

    Ok(StructuralEnvelope {
        span: html_span,
        opening_start: range.start,
        kind: StructuralEnvelopeKind::HtmlElement,
        dynamic_widget: None,
    })
}

pub(super) fn dynamic_widget_for_node<'a>(
    model: &'a ProjectModel,
    node: &SourceNode,
) -> Option<&'a DynamicWidgetSourceInstance> {
    model
        .source_graph
        .dynamic_widget_graph
        .source_instances
        .iter()
        .find(|instance| {
            same_model_path(&instance.file, &node.file)
                && (instance.source_node_ids.iter().any(|id| id == &node.id)
                    || instance
                        .root_source_node_ids
                        .iter()
                        .any(|id| id == &node.id))
        })
}

fn dynamic_widget_instance_attribute(source: &str, span: Span) -> Option<String> {
    let opening = source.get(span.start..span.end)?;
    let opening_end = opening.find('>')?;
    attribute_value(&opening[..=opening_end], "data-pana-widget-instance")
}

fn attribute_value(opening: &str, name: &str) -> Option<String> {
    let mut cursor = 0usize;
    while let Some(relative) = opening.get(cursor..)?.find(name) {
        let start = cursor + relative;
        let before_is_boundary = start == 0
            || opening
                .as_bytes()
                .get(start.wrapping_sub(1))
                .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'<');
        let after = start + name.len();
        let after_is_boundary = opening
            .as_bytes()
            .get(after)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b'=');
        if !before_is_boundary || !after_is_boundary {
            cursor = after;
            continue;
        }
        let rest = opening.get(after..)?.trim_start();
        let value = rest.strip_prefix('=')?.trim_start();
        let quote = value.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let quoted = value.get(quote.len_utf8()..)?;
        let end = quoted.find(quote)?;
        return Some(quoted[..end].to_string());
    }
    None
}
