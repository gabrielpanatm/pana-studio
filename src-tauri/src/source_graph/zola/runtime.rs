use std::collections::BTreeMap;

use crate::source_graph::tera_semantics::{
    TeraSemanticCall, TeraSemanticDocument, TeraSemanticExpression, TeraSemanticNode,
    TeraSemanticValue,
};

pub(crate) const PINNED_ZOLA_REVISION: &str = "29540e9897dbe8aca388b13f7bdf615985f6ca2c";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ZolaTeraRuntimeKind {
    Function,
    Filter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ZolaTeraRuntimeAvailability {
    Builtin,
    Early,
    Late,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ZolaTeraRuntimeDescriptor {
    pub(crate) name: &'static str,
    pub(crate) kind: ZolaTeraRuntimeKind,
    pub(crate) availability: ZolaTeraRuntimeAvailability,
}

pub(crate) const ZOLA_TERA_RUNTIME: &[ZolaTeraRuntimeDescriptor] = &[
    runtime_filter("base64_decode", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("base64_encode", ZolaTeraRuntimeAvailability::Builtin),
    runtime_function("get_hash", ZolaTeraRuntimeAvailability::Early),
    runtime_function("get_image_metadata", ZolaTeraRuntimeAvailability::Early),
    runtime_function("get_page", ZolaTeraRuntimeAvailability::Late),
    runtime_function("get_section", ZolaTeraRuntimeAvailability::Late),
    runtime_function("get_taxonomy", ZolaTeraRuntimeAvailability::Late),
    runtime_function("get_taxonomy_term", ZolaTeraRuntimeAvailability::Late),
    runtime_function("get_taxonomy_url", ZolaTeraRuntimeAvailability::Early),
    runtime_function("get_url", ZolaTeraRuntimeAvailability::Early),
    runtime_function("load_data", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("markdown", ZolaTeraRuntimeAvailability::Early),
    runtime_function("now", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("num_format", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("regex_replace", ZolaTeraRuntimeAvailability::Builtin),
    runtime_function("resize_image", ZolaTeraRuntimeAvailability::Early),
    runtime_function("trans", ZolaTeraRuntimeAvailability::Early),
];

const fn runtime_function(
    name: &'static str,
    availability: ZolaTeraRuntimeAvailability,
) -> ZolaTeraRuntimeDescriptor {
    ZolaTeraRuntimeDescriptor {
        name,
        kind: ZolaTeraRuntimeKind::Function,
        availability,
    }
}

const fn runtime_filter(
    name: &'static str,
    availability: ZolaTeraRuntimeAvailability,
) -> ZolaTeraRuntimeDescriptor {
    ZolaTeraRuntimeDescriptor {
        name,
        kind: ZolaTeraRuntimeKind::Filter,
        availability,
    }
}

pub(crate) fn collect_zola_runtime_uses(
    document: &TeraSemanticDocument,
) -> Vec<ZolaTeraRuntimeDescriptor> {
    let mut uses = BTreeMap::new();
    collect_nodes(&document.nodes, &mut uses);
    uses.into_values().collect()
}

fn collect_nodes(
    nodes: &[TeraSemanticNode],
    uses: &mut BTreeMap<(ZolaTeraRuntimeKind, &'static str), ZolaTeraRuntimeDescriptor>,
) {
    for node in nodes {
        match node {
            TeraSemanticNode::Variable { expression } => collect_expression(expression, uses),
            TeraSemanticNode::MacroDefinition {
                arguments, body, ..
            } => {
                for expression in arguments.values().flatten() {
                    collect_expression(expression, uses);
                }
                collect_nodes(body, uses);
            }
            TeraSemanticNode::Set { value, .. } => collect_expression(value, uses),
            TeraSemanticNode::FilterSection { filter, body } => {
                collect_call(filter, ZolaTeraRuntimeKind::Filter, uses);
                collect_nodes(body, uses);
            }
            TeraSemanticNode::Block { body, .. } => collect_nodes(body, uses),
            TeraSemanticNode::For {
                container,
                body,
                empty_body,
                ..
            } => {
                collect_expression(container, uses);
                collect_nodes(body, uses);
                if let Some(empty_body) = empty_body {
                    collect_nodes(empty_body, uses);
                }
            }
            TeraSemanticNode::If {
                branches,
                otherwise,
            } => {
                for branch in branches {
                    collect_expression(&branch.condition, uses);
                    collect_nodes(&branch.body, uses);
                }
                if let Some(otherwise) = otherwise {
                    collect_nodes(otherwise, uses);
                }
            }
            _ => {}
        }
    }
}

fn collect_expression(
    expression: &TeraSemanticExpression,
    uses: &mut BTreeMap<(ZolaTeraRuntimeKind, &'static str), ZolaTeraRuntimeDescriptor>,
) {
    for filter in &expression.filters {
        collect_call(filter, ZolaTeraRuntimeKind::Filter, uses);
    }
    collect_value(&expression.value, uses);
}

fn collect_value(
    value: &TeraSemanticValue,
    uses: &mut BTreeMap<(ZolaTeraRuntimeKind, &'static str), ZolaTeraRuntimeDescriptor>,
) {
    match value {
        TeraSemanticValue::Math { left, right, .. }
        | TeraSemanticValue::Logic { left, right, .. } => {
            collect_expression(left, uses);
            collect_expression(right, uses);
        }
        TeraSemanticValue::Test { arguments, .. } | TeraSemanticValue::Array(arguments) => {
            for argument in arguments {
                collect_expression(argument, uses);
            }
        }
        TeraSemanticValue::MacroCall(call) => {
            collect_call_arguments(call, uses);
        }
        TeraSemanticValue::FunctionCall(call) => {
            collect_call(call, ZolaTeraRuntimeKind::Function, uses);
        }
        TeraSemanticValue::StringConcat(values) => {
            for value in values {
                collect_value(value, uses);
            }
        }
        TeraSemanticValue::In {
            needle, haystack, ..
        } => {
            collect_expression(needle, uses);
            collect_expression(haystack, uses);
        }
        _ => {}
    }
}

fn collect_call(
    call: &TeraSemanticCall,
    kind: ZolaTeraRuntimeKind,
    uses: &mut BTreeMap<(ZolaTeraRuntimeKind, &'static str), ZolaTeraRuntimeDescriptor>,
) {
    if call.namespace.is_none() {
        if let Some(descriptor) = ZOLA_TERA_RUNTIME
            .iter()
            .copied()
            .find(|descriptor| descriptor.kind == kind && descriptor.name == call.name)
        {
            uses.insert((descriptor.kind, descriptor.name), descriptor);
        }
    }
    collect_call_arguments(call, uses);
}

fn collect_call_arguments(
    call: &TeraSemanticCall,
    uses: &mut BTreeMap<(ZolaTeraRuntimeKind, &'static str), ZolaTeraRuntimeDescriptor>,
) {
    for argument in call.arguments.values() {
        collect_expression(argument, uses);
    }
}

#[cfg(test)]
mod tests {
    use tera::Template;

    use super::*;

    #[test]
    fn runtime_catalog_matches_the_functions_and_filters_registered_by_pinned_zola() {
        assert_eq!(PINNED_ZOLA_REVISION.len(), 40);
        assert_eq!(
            ZOLA_TERA_RUNTIME
                .iter()
                .filter(|descriptor| descriptor.kind == ZolaTeraRuntimeKind::Function)
                .count(),
            12
        );
        assert_eq!(
            ZOLA_TERA_RUNTIME
                .iter()
                .filter(|descriptor| descriptor.kind == ZolaTeraRuntimeKind::Filter)
                .count(),
            5
        );
        assert_eq!(
            ZOLA_TERA_RUNTIME
                .iter()
                .filter(|descriptor| {
                    descriptor.availability == ZolaTeraRuntimeAvailability::Late
                })
                .map(|descriptor| descriptor.name)
                .collect::<Vec<_>>(),
            vec![
                "get_page",
                "get_section",
                "get_taxonomy",
                "get_taxonomy_term"
            ]
        );
    }

    #[test]
    fn semantic_runtime_uses_include_nested_functions_and_zola_filters() {
        let template = Template::new(
            "runtime.html",
            None,
            r#"{% set data = load_data(path="date/catalog.toml") %}
{% set article = get_page(path="blog/post.md") %}
{{ article.title | markdown }}
{{ trans(key="welcome") | num_format }}
{{ "abc" | regex_replace(pattern="a", rep="b") }}
{{ cards::render(value=get_taxonomy(kind="tags")) }}
"#,
        )
        .unwrap();
        let document = TeraSemanticDocument::from_template(&template);
        let uses = collect_zola_runtime_uses(&document);

        assert_eq!(
            uses.iter()
                .map(|descriptor| (descriptor.kind, descriptor.name))
                .collect::<Vec<_>>(),
            vec![
                (ZolaTeraRuntimeKind::Function, "get_page"),
                (ZolaTeraRuntimeKind::Function, "get_taxonomy"),
                (ZolaTeraRuntimeKind::Function, "load_data"),
                (ZolaTeraRuntimeKind::Function, "trans"),
                (ZolaTeraRuntimeKind::Filter, "markdown"),
                (ZolaTeraRuntimeKind::Filter, "num_format"),
                (ZolaTeraRuntimeKind::Filter, "regex_replace"),
            ]
        );
    }
}
