use std::collections::BTreeMap;

use crate::source_graph::tera_semantics::{
    TeraSemanticCall, TeraSemanticDocument, TeraSemanticExpression, TeraSemanticNode,
    TeraSemanticValue,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ZolaTeraRuntimeKind {
    Function,
    Filter,
    Test,
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
    pub(crate) required_arguments: &'static [&'static str],
    pub(crate) optional_arguments: &'static [&'static str],
    pub(crate) deprecated_arguments: &'static [(&'static str, &'static str)],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ZolaTeraRuntimeDeprecation {
    pub(crate) function: &'static str,
    pub(crate) argument: &'static str,
    pub(crate) replacement: &'static str,
}

pub(crate) const ZOLA_TERA_RUNTIME: &[ZolaTeraRuntimeDescriptor] = &[
    runtime_filter("base64_decode", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("base64_encode", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("date", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("filesizeformat", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("format", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("json_encode", ZolaTeraRuntimeAvailability::Builtin),
    runtime_function_with_arguments(
        "get_env",
        ZolaTeraRuntimeAvailability::Early,
        &["name"],
        &[],
        &[],
    ),
    runtime_function("get_hash", ZolaTeraRuntimeAvailability::Early),
    runtime_function("get_image_metadata", ZolaTeraRuntimeAvailability::Early),
    runtime_function("get_random", ZolaTeraRuntimeAvailability::Builtin),
    runtime_function_with_arguments(
        "get_page",
        ZolaTeraRuntimeAvailability::Late,
        &["path"],
        &["lang", "allow_missing"],
        &[],
    ),
    runtime_function_with_arguments(
        "get_section",
        ZolaTeraRuntimeAvailability::Late,
        &["path"],
        &["lang", "allow_missing"],
        &[],
    ),
    runtime_function_with_arguments(
        "get_taxonomy",
        ZolaTeraRuntimeAvailability::Late,
        &["kind"],
        &["lang", "required"],
        &[],
    ),
    runtime_function_with_arguments(
        "get_taxonomy_term",
        ZolaTeraRuntimeAvailability::Late,
        &["kind", "term"],
        &["lang", "required"],
        &[],
    ),
    runtime_function_with_arguments(
        "get_taxonomy_url",
        ZolaTeraRuntimeAvailability::Late,
        &["kind"],
        &["term", "name", "lang", "required"],
        &[("name", "term")],
    ),
    runtime_function("get_url", ZolaTeraRuntimeAvailability::Early),
    runtime_function("load_data", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("markdown", ZolaTeraRuntimeAvailability::Early),
    runtime_function("now", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("num_format", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("regex_replace", ZolaTeraRuntimeAvailability::Builtin),
    runtime_function("resize_image", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("shuffle", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("slugify", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("spaceless", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("striptags", ZolaTeraRuntimeAvailability::Builtin),
    runtime_function_with_arguments(
        "text_direction",
        ZolaTeraRuntimeAvailability::Early,
        &[],
        &["lang"],
        &[],
    ),
    runtime_function("trans", ZolaTeraRuntimeAvailability::Early),
    runtime_filter("urlencode", ZolaTeraRuntimeAvailability::Builtin),
    runtime_filter("urlencode_strict", ZolaTeraRuntimeAvailability::Builtin),
    runtime_test("after", ZolaTeraRuntimeAvailability::Builtin),
    runtime_test("before", ZolaTeraRuntimeAvailability::Builtin),
    runtime_test("matching", ZolaTeraRuntimeAvailability::Builtin),
];

const fn runtime_function(
    name: &'static str,
    availability: ZolaTeraRuntimeAvailability,
) -> ZolaTeraRuntimeDescriptor {
    ZolaTeraRuntimeDescriptor {
        name,
        kind: ZolaTeraRuntimeKind::Function,
        availability,
        required_arguments: &[],
        optional_arguments: &[],
        deprecated_arguments: &[],
    }
}

const fn runtime_function_with_arguments(
    name: &'static str,
    availability: ZolaTeraRuntimeAvailability,
    required_arguments: &'static [&'static str],
    optional_arguments: &'static [&'static str],
    deprecated_arguments: &'static [(&'static str, &'static str)],
) -> ZolaTeraRuntimeDescriptor {
    ZolaTeraRuntimeDescriptor {
        name,
        kind: ZolaTeraRuntimeKind::Function,
        availability,
        required_arguments,
        optional_arguments,
        deprecated_arguments,
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
        required_arguments: &[],
        optional_arguments: &[],
        deprecated_arguments: &[],
    }
}

const fn runtime_test(
    name: &'static str,
    availability: ZolaTeraRuntimeAvailability,
) -> ZolaTeraRuntimeDescriptor {
    ZolaTeraRuntimeDescriptor {
        name,
        kind: ZolaTeraRuntimeKind::Test,
        availability,
        required_arguments: &[],
        optional_arguments: &[],
        deprecated_arguments: &[],
    }
}

pub(crate) fn collect_zola_runtime_uses(
    document: &TeraSemanticDocument,
) -> Vec<ZolaTeraRuntimeDescriptor> {
    let mut uses = BTreeMap::new();
    collect_nodes(&document.nodes, &mut uses);
    for definition in &document.component_definitions {
        for argument in &definition.arguments {
            if let Some(default) = argument.default_value.as_ref() {
                collect_expression(default, &mut uses);
            }
        }
    }
    for call in &document.component_calls {
        for argument in &call.arguments {
            collect_expression(&argument.expression, &mut uses);
        }
    }
    uses.into_values().collect()
}

pub(crate) fn collect_zola_runtime_deprecations(
    document: &TeraSemanticDocument,
) -> Vec<ZolaTeraRuntimeDeprecation> {
    let mut issues = Vec::new();
    collect_deprecations_from_nodes(&document.nodes, &mut issues);
    for definition in &document.component_definitions {
        for argument in &definition.arguments {
            if let Some(default) = argument.default_value.as_ref() {
                collect_deprecations_from_expression(default, &mut issues);
            }
        }
    }
    for call in &document.component_calls {
        for argument in &call.arguments {
            collect_deprecations_from_expression(&argument.expression, &mut issues);
        }
    }
    issues.sort_by_key(|issue| (issue.function, issue.argument, issue.replacement));
    issues.dedup();
    issues
}

fn collect_deprecations_from_nodes(
    nodes: &[TeraSemanticNode],
    issues: &mut Vec<ZolaTeraRuntimeDeprecation>,
) {
    for node in nodes {
        match node {
            TeraSemanticNode::Variable { expression } => {
                collect_deprecations_from_expression(expression, issues)
            }
            TeraSemanticNode::ComponentDefinition { body, .. }
            | TeraSemanticNode::ComponentCall { body, .. }
            | TeraSemanticNode::SetBlock { body, .. }
            | TeraSemanticNode::Block { body, .. } => collect_deprecations_from_nodes(body, issues),
            TeraSemanticNode::Set { value, .. } => {
                collect_deprecations_from_expression(value, issues)
            }
            TeraSemanticNode::FilterSection { filter, body } => {
                collect_deprecations_from_call(filter, issues);
                collect_deprecations_from_nodes(body, issues);
            }
            TeraSemanticNode::For {
                container,
                body,
                empty_body,
                ..
            } => {
                collect_deprecations_from_expression(container, issues);
                collect_deprecations_from_nodes(body, issues);
                if let Some(empty_body) = empty_body {
                    collect_deprecations_from_nodes(empty_body, issues);
                }
            }
            TeraSemanticNode::If {
                branches,
                otherwise,
            } => {
                for branch in branches {
                    collect_deprecations_from_expression(&branch.condition, issues);
                    collect_deprecations_from_nodes(&branch.body, issues);
                }
                if let Some(otherwise) = otherwise {
                    collect_deprecations_from_nodes(otherwise, issues);
                }
            }
            _ => {}
        }
    }
}

fn collect_deprecations_from_expression(
    expression: &TeraSemanticExpression,
    issues: &mut Vec<ZolaTeraRuntimeDeprecation>,
) {
    for filter in &expression.filters {
        collect_deprecations_from_call(filter, issues);
    }
    collect_deprecations_from_value(&expression.value, issues);
}

fn collect_deprecations_from_value(
    value: &TeraSemanticValue,
    issues: &mut Vec<ZolaTeraRuntimeDeprecation>,
) {
    match value {
        TeraSemanticValue::Math { left, right, .. }
        | TeraSemanticValue::Logic { left, right, .. }
        | TeraSemanticValue::In {
            needle: left,
            haystack: right,
            ..
        } => {
            collect_deprecations_from_expression(left, issues);
            collect_deprecations_from_expression(right, issues);
        }
        TeraSemanticValue::Test { arguments, .. } | TeraSemanticValue::Array(arguments) => {
            for argument in arguments {
                collect_deprecations_from_expression(argument, issues);
            }
        }
        TeraSemanticValue::FunctionCall(call) => collect_deprecations_from_call(call, issues),
        TeraSemanticValue::Map(values) => {
            for value in values.values() {
                collect_deprecations_from_expression(value, issues);
            }
        }
        TeraSemanticValue::Spread(value) | TeraSemanticValue::OptionalChain { value, .. } => {
            collect_deprecations_from_expression(value, issues)
        }
        TeraSemanticValue::Slice { value, start, end } => {
            collect_deprecations_from_expression(value, issues);
            if let Some(start) = start {
                collect_deprecations_from_expression(start, issues);
            }
            if let Some(end) = end {
                collect_deprecations_from_expression(end, issues);
            }
        }
        TeraSemanticValue::Ternary {
            condition,
            truthy,
            falsy,
        } => {
            collect_deprecations_from_expression(condition, issues);
            collect_deprecations_from_expression(truthy, issues);
            collect_deprecations_from_expression(falsy, issues);
        }
        TeraSemanticValue::Comprehension {
            value,
            iterable,
            condition,
            ..
        } => {
            collect_deprecations_from_expression(value, issues);
            collect_deprecations_from_expression(iterable, issues);
            if let Some(condition) = condition {
                collect_deprecations_from_expression(condition, issues);
            }
        }
        TeraSemanticValue::StringConcat(values) => {
            for value in values {
                collect_deprecations_from_value(value, issues);
            }
        }
        _ => {}
    }
}

fn collect_deprecations_from_call(
    call: &TeraSemanticCall,
    issues: &mut Vec<ZolaTeraRuntimeDeprecation>,
) {
    if call.namespace.is_none() {
        if let Some(descriptor) = ZOLA_TERA_RUNTIME.iter().find(|descriptor| {
            descriptor.kind == ZolaTeraRuntimeKind::Function && descriptor.name == call.name
        }) {
            for &(argument, replacement) in descriptor.deprecated_arguments {
                if call.arguments.contains_key(argument)
                    && !call.arguments.contains_key(replacement)
                {
                    issues.push(ZolaTeraRuntimeDeprecation {
                        function: descriptor.name,
                        argument,
                        replacement,
                    });
                }
            }
        }
    }
    for argument in call.arguments.values() {
        collect_deprecations_from_expression(argument, issues);
    }
}

fn collect_nodes(
    nodes: &[TeraSemanticNode],
    uses: &mut BTreeMap<(ZolaTeraRuntimeKind, &'static str), ZolaTeraRuntimeDescriptor>,
) {
    for node in nodes {
        match node {
            TeraSemanticNode::Variable { expression } => collect_expression(expression, uses),
            TeraSemanticNode::ComponentDefinition { body, .. }
            | TeraSemanticNode::ComponentCall { body, .. }
            | TeraSemanticNode::SetBlock { body, .. } => {
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
        TeraSemanticValue::Test {
            name, arguments, ..
        } => {
            if let Some(descriptor) = ZOLA_TERA_RUNTIME.iter().copied().find(|descriptor| {
                descriptor.kind == ZolaTeraRuntimeKind::Test && descriptor.name == name
            }) {
                uses.insert((descriptor.kind, descriptor.name), descriptor);
            }
            for argument in arguments {
                collect_expression(argument, uses);
            }
        }
        TeraSemanticValue::Array(arguments) => {
            for argument in arguments {
                collect_expression(argument, uses);
            }
        }
        TeraSemanticValue::FunctionCall(call) => {
            collect_call(call, ZolaTeraRuntimeKind::Function, uses);
        }
        TeraSemanticValue::Map(values) => {
            for value in values.values() {
                collect_expression(value, uses);
            }
        }
        TeraSemanticValue::Spread(value) | TeraSemanticValue::OptionalChain { value, .. } => {
            collect_expression(value, uses);
        }
        TeraSemanticValue::Slice { value, start, end } => {
            collect_expression(value, uses);
            if let Some(start) = start {
                collect_expression(start, uses);
            }
            if let Some(end) = end {
                collect_expression(end, uses);
            }
        }
        TeraSemanticValue::Ternary {
            condition,
            truthy,
            falsy,
        } => {
            collect_expression(condition, uses);
            collect_expression(truthy, uses);
            collect_expression(falsy, uses);
        }
        TeraSemanticValue::Comprehension {
            value,
            iterable,
            condition,
            ..
        } => {
            collect_expression(value, uses);
            collect_expression(iterable, uses);
            if let Some(condition) = condition {
                collect_expression(condition, uses);
            }
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
    use super::*;
    use crate::source_graph::tera_cst::parse_tera_cst;

    #[test]
    fn runtime_catalog_matches_the_functions_and_filters_registered_by_pinned_zola() {
        assert_eq!(crate::zola_engine::EMBEDDED_ZOLA_REVISION.len(), 40);
        assert_eq!(
            ZOLA_TERA_RUNTIME
                .iter()
                .filter(|descriptor| descriptor.kind == ZolaTeraRuntimeKind::Function)
                .count(),
            15
        );
        let page = ZOLA_TERA_RUNTIME
            .iter()
            .find(|descriptor| descriptor.name == "get_page")
            .unwrap();
        assert_eq!(page.required_arguments, &["path"]);
        assert_eq!(page.optional_arguments, &["lang", "allow_missing"]);
        let section = ZOLA_TERA_RUNTIME
            .iter()
            .find(|descriptor| descriptor.name == "get_section")
            .unwrap();
        assert_eq!(section.optional_arguments, &["lang", "allow_missing"]);
        let environment = ZOLA_TERA_RUNTIME
            .iter()
            .find(|descriptor| descriptor.name == "get_env")
            .unwrap();
        assert_eq!(environment.required_arguments, &["name"]);
        let direction = ZOLA_TERA_RUNTIME
            .iter()
            .find(|descriptor| descriptor.name == "text_direction")
            .unwrap();
        assert_eq!(direction.optional_arguments, &["lang"]);
        let taxonomy_url = ZOLA_TERA_RUNTIME
            .iter()
            .find(|descriptor| descriptor.name == "get_taxonomy_url")
            .unwrap();
        assert!(taxonomy_url.optional_arguments.contains(&"term"));
        assert!(taxonomy_url.optional_arguments.contains(&"lang"));
        assert_eq!(taxonomy_url.deprecated_arguments, &[("name", "term")]);
        assert_eq!(
            ZOLA_TERA_RUNTIME
                .iter()
                .filter(|descriptor| descriptor.kind == ZolaTeraRuntimeKind::Filter)
                .count(),
            15
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
                "get_taxonomy_term",
                "get_taxonomy_url"
            ]
        );
    }

    #[test]
    fn semantic_runtime_uses_include_nested_functions_and_zola_filters() {
        let source = r#"{% set data = load_data(path="date/catalog.toml") %}
{% set article = get_page(path="blog/post.md") %}
{{ article.title | markdown }}
{{ trans(key="welcome") | num_format }}
{{ "abc" | regex_replace(pattern="a", rep="b") }}
{{<cards.render value={get_taxonomy(kind="tags")} />}}
"#;
        let cst = parse_tera_cst(source, "runtime.html");
        let document = cst.semantics().expect("semantic projection");
        let uses = collect_zola_runtime_uses(document);

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

    #[test]
    fn taxonomy_name_argument_reports_the_023_deprecation() {
        let cst = parse_tera_cst(
            "{{ get_taxonomy_url(kind=\"tags\", name=\"rust\") }}",
            "taxonomy.html",
        );
        let issues = collect_zola_runtime_deprecations(cst.semantics().unwrap());
        assert_eq!(
            issues,
            vec![ZolaTeraRuntimeDeprecation {
                function: "get_taxonomy_url",
                argument: "name",
                replacement: "term",
            }]
        );
    }
}
