use std::collections::BTreeMap;

use serde::Serialize;

use crate::source_graph::tera_cst::{TeraCstKind, TeraCstNode, TeraTagKind};

/// Editor-owned structural projection. It intentionally contains no Tera
/// parser/AST type: Zola/Tera validate the complete project authoritatively.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticDocument {
    pub nodes: Vec<TeraSemanticNode>,
    pub component_definitions: Vec<TeraComponentDefinition>,
    pub component_calls: Vec<TeraComponentCall>,
    pub legacy_syntax: Vec<TeraLegacySyntax>,
}

impl TeraSemanticDocument {
    pub(crate) fn from_cst(source: &str, nodes: &[TeraCstNode]) -> Self {
        let mut document = Self {
            nodes: project_semantic_nodes(source, nodes),
            ..Self::default()
        };
        project_components_and_legacy(source, nodes, &mut document);
        document
    }

    pub(crate) fn walk(&self) -> Vec<&TeraSemanticNode> {
        let mut result = Vec::new();
        for node in &self.nodes {
            node.walk_into(&mut result);
        }
        result
    }

    pub(crate) fn template_facts(&self) -> TeraTemplateFacts {
        let mut facts = TeraTemplateFacts::default();
        for node in self.walk() {
            match node {
                TeraSemanticNode::Extends { template } => {
                    if facts.extends.is_none() {
                        facts.extends = Some(template.clone());
                    }
                }
                TeraSemanticNode::Include {
                    templates,
                    ignore_missing,
                } => {
                    push_unique_all(&mut facts.includes, templates);
                    facts.include_groups.push(TeraIncludeFact {
                        targets: templates.clone(),
                        ignore_missing: *ignore_missing,
                    });
                }
                TeraSemanticNode::Block { name, .. } => push_unique(&mut facts.blocks, name),
                _ => {}
            }
        }
        facts.components = self
            .component_definitions
            .iter()
            .map(|definition| definition.name.clone())
            .collect();
        facts
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TeraTemplateFacts {
    pub(crate) extends: Option<String>,
    pub(crate) includes: Vec<String>,
    pub(crate) include_groups: Vec<TeraIncludeFact>,
    pub(crate) blocks: Vec<String>,
    pub(crate) components: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TeraIncludeFact {
    pub(crate) targets: Vec<String>,
    pub(crate) ignore_missing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSourceRange {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraComponentDefinition {
    pub name: String,
    pub namespace: Option<String>,
    pub arguments: Vec<TeraComponentParameter>,
    pub rest_argument: Option<String>,
    pub range: TeraSourceRange,
    pub body_range: Option<TeraSourceRange>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraComponentParameter {
    pub name: String,
    pub argument_type: Option<String>,
    pub required: bool,
    pub default_value: Option<TeraSemanticExpression>,
    pub range: TeraSourceRange,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraComponentCall {
    pub name: String,
    pub namespace: Option<String>,
    pub arguments: Vec<TeraComponentArgument>,
    pub range: TeraSourceRange,
    pub call_range: TeraSourceRange,
    pub body_range: Option<TeraSourceRange>,
    pub parent_call: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraComponentArgument {
    pub name: Option<String>,
    pub expression: TeraSemanticExpression,
    pub spread: bool,
    pub range: TeraSourceRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TeraLegacySyntaxKind {
    RemovedDefinition,
    RemovedImport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraLegacySyntax {
    pub kind: TeraLegacySyntaxKind,
    pub range: TeraSourceRange,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TeraSemanticNode {
    Super,
    Text {
        value: String,
    },
    Variable {
        expression: TeraSemanticExpression,
    },
    ComponentDefinition {
        name: String,
        body: Vec<TeraSemanticNode>,
    },
    ComponentCall {
        name: String,
        body: Vec<TeraSemanticNode>,
    },
    Extends {
        template: String,
    },
    Include {
        templates: Vec<String>,
        ignore_missing: bool,
    },
    Set {
        key: String,
        global: bool,
        value: TeraSemanticExpression,
    },
    SetBlock {
        key: String,
        body: Vec<TeraSemanticNode>,
    },
    Raw {
        value: String,
    },
    FilterSection {
        filter: TeraSemanticCall,
        body: Vec<TeraSemanticNode>,
    },
    Block {
        name: String,
        body: Vec<TeraSemanticNode>,
    },
    For {
        key: Option<String>,
        value: String,
        container: TeraSemanticExpression,
        body: Vec<TeraSemanticNode>,
        empty_body: Option<Vec<TeraSemanticNode>>,
    },
    If {
        branches: Vec<TeraSemanticBranch>,
        otherwise: Option<Vec<TeraSemanticNode>>,
    },
    Break,
    Continue,
    Comment {
        value: String,
    },
}

impl TeraSemanticNode {
    fn walk_into<'a>(&'a self, result: &mut Vec<&'a TeraSemanticNode>) {
        result.push(self);
        match self {
            Self::ComponentDefinition { body, .. }
            | Self::ComponentCall { body, .. }
            | Self::SetBlock { body, .. }
            | Self::FilterSection { body, .. }
            | Self::Block { body, .. } => walk_nodes(body, result),
            Self::For {
                body, empty_body, ..
            } => {
                walk_nodes(body, result);
                if let Some(empty_body) = empty_body {
                    walk_nodes(empty_body, result);
                }
            }
            Self::If {
                branches,
                otherwise,
            } => {
                for branch in branches {
                    walk_nodes(&branch.body, result);
                }
                if let Some(otherwise) = otherwise {
                    walk_nodes(otherwise, result);
                }
            }
            _ => {}
        }
    }
}

fn walk_nodes<'a>(nodes: &'a [TeraSemanticNode], result: &mut Vec<&'a TeraSemanticNode>) {
    for node in nodes {
        node.walk_into(result);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticBranch {
    pub condition: TeraSemanticExpression,
    pub body: Vec<TeraSemanticNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticExpression {
    pub value: TeraSemanticValue,
    pub negated: bool,
    pub filters: Vec<TeraSemanticCall>,
}

impl TeraSemanticExpression {
    pub(crate) fn parse(source: &str) -> Self {
        parse_expression(source.trim())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum TeraSemanticValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Identifier(String),
    Math {
        operator: String,
        left: Box<TeraSemanticExpression>,
        right: Box<TeraSemanticExpression>,
    },
    Logic {
        operator: String,
        left: Box<TeraSemanticExpression>,
        right: Box<TeraSemanticExpression>,
    },
    Test {
        identifier: String,
        name: String,
        negated: bool,
        arguments: Vec<TeraSemanticExpression>,
    },
    FunctionCall(TeraSemanticCall),
    Array(Vec<TeraSemanticExpression>),
    Map(BTreeMap<String, TeraSemanticExpression>),
    Spread(Box<TeraSemanticExpression>),
    Slice {
        value: Box<TeraSemanticExpression>,
        start: Option<Box<TeraSemanticExpression>>,
        end: Option<Box<TeraSemanticExpression>>,
    },
    OptionalChain {
        value: Box<TeraSemanticExpression>,
        member: String,
    },
    Ternary {
        condition: Box<TeraSemanticExpression>,
        truthy: Box<TeraSemanticExpression>,
        falsy: Box<TeraSemanticExpression>,
    },
    Comprehension {
        value: Box<TeraSemanticExpression>,
        binding: String,
        iterable: Box<TeraSemanticExpression>,
        condition: Option<Box<TeraSemanticExpression>>,
    },
    StringConcat(Vec<TeraSemanticValue>),
    In {
        negated: bool,
        needle: Box<TeraSemanticExpression>,
        haystack: Box<TeraSemanticExpression>,
    },
    Raw(String),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeraSemanticCall {
    pub namespace: Option<String>,
    pub name: String,
    pub arguments: BTreeMap<String, TeraSemanticExpression>,
}

fn project_semantic_nodes(source: &str, nodes: &[TeraCstNode]) -> Vec<TeraSemanticNode> {
    let mut result = Vec::new();
    let mut scopes = Vec::<SemanticScope>::new();
    for node in nodes {
        let content = node.content(source).trim();
        match &node.kind {
            TeraCstKind::Text if !node.full_text(source).is_empty() => {
                append_semantic_node(
                    &mut result,
                    &mut scopes,
                    TeraSemanticNode::Text {
                        value: node.full_text(source).to_string(),
                    },
                );
            }
            TeraCstKind::Variable if content.starts_with("</") => {
                close_semantic_scope(&mut result, &mut scopes);
            }
            TeraCstKind::Variable if is_component_call(content) => {
                let name = component_call_name(content).unwrap_or_default();
                if content.trim_end().ends_with("/>") {
                    append_semantic_node(
                        &mut result,
                        &mut scopes,
                        TeraSemanticNode::ComponentCall {
                            name,
                            body: Vec::new(),
                        },
                    );
                } else {
                    scopes.push(SemanticScope::ComponentCall {
                        name,
                        body: Vec::new(),
                    });
                }
            }
            TeraCstKind::Variable => append_semantic_node(
                &mut result,
                &mut scopes,
                if content == "super()" {
                    TeraSemanticNode::Super
                } else {
                    TeraSemanticNode::Variable {
                        expression: TeraSemanticExpression::parse(content),
                    }
                },
            ),
            TeraCstKind::Comment => append_semantic_node(
                &mut result,
                &mut scopes,
                TeraSemanticNode::Comment {
                    value: content.to_string(),
                },
            ),
            TeraCstKind::Raw => append_semantic_node(
                &mut result,
                &mut scopes,
                TeraSemanticNode::Raw {
                    value: node.content(source).to_string(),
                },
            ),
            TeraCstKind::Tag(tag) => project_semantic_tag(tag, content, &mut result, &mut scopes),
            _ => {}
        }
    }
    while !scopes.is_empty() {
        close_semantic_scope(&mut result, &mut scopes);
    }
    result
}

enum SemanticScope {
    Block {
        name: String,
        body: Vec<TeraSemanticNode>,
    },
    ComponentDefinition {
        name: String,
        body: Vec<TeraSemanticNode>,
    },
    ComponentCall {
        name: String,
        body: Vec<TeraSemanticNode>,
    },
    For {
        key: Option<String>,
        value: String,
        container: TeraSemanticExpression,
        body: Vec<TeraSemanticNode>,
        empty_body: Option<Vec<TeraSemanticNode>>,
    },
    If {
        branches: Vec<TeraSemanticBranch>,
        otherwise: Option<Vec<TeraSemanticNode>>,
    },
    SetBlock {
        key: String,
        body: Vec<TeraSemanticNode>,
    },
    Filter {
        filter: TeraSemanticCall,
        body: Vec<TeraSemanticNode>,
    },
}

impl SemanticScope {
    fn body_mut(&mut self) -> &mut Vec<TeraSemanticNode> {
        match self {
            Self::Block { body, .. }
            | Self::ComponentDefinition { body, .. }
            | Self::ComponentCall { body, .. }
            | Self::SetBlock { body, .. }
            | Self::Filter { body, .. } => body,
            Self::For {
                body, empty_body, ..
            } => empty_body.as_mut().unwrap_or(body),
            Self::If {
                branches,
                otherwise,
            } => otherwise
                .as_mut()
                .unwrap_or_else(|| &mut branches.last_mut().expect("if has a branch").body),
        }
    }

    fn finish(self) -> TeraSemanticNode {
        match self {
            Self::Block { name, body } => TeraSemanticNode::Block { name, body },
            Self::ComponentDefinition { name, body } => {
                TeraSemanticNode::ComponentDefinition { name, body }
            }
            Self::ComponentCall { name, body } => TeraSemanticNode::ComponentCall { name, body },
            Self::For {
                key,
                value,
                container,
                body,
                empty_body,
            } => TeraSemanticNode::For {
                key,
                value,
                container,
                body,
                empty_body,
            },
            Self::If {
                branches,
                otherwise,
            } => TeraSemanticNode::If {
                branches,
                otherwise,
            },
            Self::SetBlock { key, body } => TeraSemanticNode::SetBlock { key, body },
            Self::Filter { filter, body } => TeraSemanticNode::FilterSection { filter, body },
        }
    }
}

fn append_semantic_node(
    output: &mut Vec<TeraSemanticNode>,
    scopes: &mut [SemanticScope],
    node: TeraSemanticNode,
) {
    if let Some(scope) = scopes.last_mut() {
        scope.body_mut().push(node);
    } else {
        output.push(node);
    }
}

fn close_semantic_scope(output: &mut Vec<TeraSemanticNode>, scopes: &mut Vec<SemanticScope>) {
    let Some(scope) = scopes.pop() else {
        return;
    };
    append_semantic_node(output, scopes, scope.finish());
}

fn project_semantic_tag(
    tag: &TeraTagKind,
    content: &str,
    output: &mut Vec<TeraSemanticNode>,
    scopes: &mut Vec<SemanticScope>,
) {
    match tag {
        TeraTagKind::Extends => {
            if let Some(template) = extract_strings(content).into_iter().next() {
                append_semantic_node(output, scopes, TeraSemanticNode::Extends { template });
            }
        }
        TeraTagKind::Include => append_semantic_node(
            output,
            scopes,
            TeraSemanticNode::Include {
                templates: extract_strings(content),
                ignore_missing: content
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .contains("ignore missing"),
            },
        ),
        TeraTagKind::Block => scopes.push(SemanticScope::Block {
            name: word_after(content, "block").unwrap_or_default(),
            body: Vec::new(),
        }),
        TeraTagKind::ComponentDefinition => scopes.push(SemanticScope::ComponentDefinition {
            name: definition_name(content).unwrap_or_default(),
            body: Vec::new(),
        }),
        TeraTagKind::ComponentCall => scopes.push(SemanticScope::ComponentCall {
            name: component_call_name(content).unwrap_or_default(),
            body: Vec::new(),
        }),
        TeraTagKind::For => {
            if let Some((bindings, container)) = parse_for_header(content) {
                scopes.push(SemanticScope::For {
                    key: (bindings.len() > 1).then(|| bindings[0].clone()),
                    value: bindings.last().cloned().unwrap_or_default(),
                    container: TeraSemanticExpression::parse(container),
                    body: Vec::new(),
                    empty_body: None,
                });
            }
        }
        TeraTagKind::If => scopes.push(SemanticScope::If {
            branches: vec![TeraSemanticBranch {
                condition: TeraSemanticExpression::parse(
                    content.strip_prefix("if").unwrap_or(content),
                ),
                body: Vec::new(),
            }],
            otherwise: None,
        }),
        TeraTagKind::Elif => {
            if let Some(SemanticScope::If {
                branches,
                otherwise,
            }) = scopes.last_mut()
            {
                *otherwise = None;
                branches.push(TeraSemanticBranch {
                    condition: TeraSemanticExpression::parse(
                        content.strip_prefix("elif").unwrap_or(content),
                    ),
                    body: Vec::new(),
                });
            }
        }
        TeraTagKind::Else => match scopes.last_mut() {
            Some(SemanticScope::For { empty_body, .. }) => *empty_body = Some(Vec::new()),
            Some(SemanticScope::If { otherwise, .. }) => *otherwise = Some(Vec::new()),
            _ => {}
        },
        TeraTagKind::Set | TeraTagKind::SetGlobal => {
            if let Some((key, value)) = parse_assignment(content) {
                append_semantic_node(
                    output,
                    scopes,
                    TeraSemanticNode::Set {
                        key,
                        global: matches!(tag, TeraTagKind::SetGlobal),
                        value: TeraSemanticExpression::parse(value),
                    },
                );
            }
        }
        TeraTagKind::SetBlock => scopes.push(SemanticScope::SetBlock {
            key: word_after(content, "set").unwrap_or_default(),
            body: Vec::new(),
        }),
        TeraTagKind::Filter => scopes.push(SemanticScope::Filter {
            filter: parse_call(content.strip_prefix("filter").unwrap_or(content).trim()),
            body: Vec::new(),
        }),
        TeraTagKind::Break => append_semantic_node(output, scopes, TeraSemanticNode::Break),
        TeraTagKind::Continue => append_semantic_node(output, scopes, TeraSemanticNode::Continue),
        TeraTagKind::EndBlock
        | TeraTagKind::EndComponentDefinition
        | TeraTagKind::EndComponentCall
        | TeraTagKind::EndFor
        | TeraTagKind::EndIf
        | TeraTagKind::EndSetBlock
        | TeraTagKind::EndFilter => close_semantic_scope(output, scopes),
        _ => {}
    }
}

fn project_components_and_legacy(
    source: &str,
    nodes: &[TeraCstNode],
    document: &mut TeraSemanticDocument,
) {
    let mut open_definitions = Vec::<usize>::new();
    let mut open_calls = Vec::<usize>::new();
    for node in nodes {
        let content = node.content(source).trim();
        match &node.kind {
            TeraCstKind::Tag(TeraTagKind::ComponentDefinition) => {
                if let Some(mut definition) = parse_component_definition(source, node, content) {
                    definition.body_range = None;
                    let index = document.component_definitions.len();
                    document.component_definitions.push(definition);
                    open_definitions.push(index);
                }
            }
            TeraCstKind::Tag(TeraTagKind::EndComponentDefinition) => {
                if let Some(index) = open_definitions.pop() {
                    let definition = &mut document.component_definitions[index];
                    definition.range = source_range(source, definition.range.start, node.end);
                    definition.body_range = Some(source_range(
                        source,
                        definition
                            .range
                            .start
                            .min(node.start)
                            .max(definition.range.start),
                        node.start,
                    ));
                    if let Some(opening) = nodes
                        .iter()
                        .find(|candidate| candidate.start == definition.range.start)
                    {
                        definition.body_range = Some(source_range(source, opening.end, node.start));
                    }
                }
            }
            TeraCstKind::Variable if content.starts_with("</") => {
                if let Some(index) = open_calls.pop() {
                    let call = &mut document.component_calls[index];
                    call.range = source_range(source, call.range.start, node.end);
                    call.body_range = Some(source_range(source, call.call_range.end, node.start));
                }
            }
            TeraCstKind::Variable if is_component_call(content) => {
                if let Some(call) =
                    parse_component_call(source, node, content, open_calls.last().copied())
                {
                    let is_self_closing = content.trim_end().ends_with("/>");
                    let index = document.component_calls.len();
                    document.component_calls.push(call);
                    if !is_self_closing {
                        open_calls.push(index);
                    }
                }
            }
            TeraCstKind::Tag(TeraTagKind::ComponentCall) => {
                if let Some(call) =
                    parse_component_call(source, node, content, open_calls.last().copied())
                {
                    let index = document.component_calls.len();
                    document.component_calls.push(call);
                    open_calls.push(index);
                }
            }
            TeraCstKind::Tag(TeraTagKind::EndComponentCall) => {
                if let Some(index) = open_calls.pop() {
                    let call = &mut document.component_calls[index];
                    call.range = source_range(source, call.range.start, node.end);
                    call.body_range = Some(source_range(source, call.call_range.end, node.start));
                }
            }
            TeraCstKind::Tag(TeraTagKind::LegacyImport) => {
                document.legacy_syntax.push(TeraLegacySyntax {
                    kind: TeraLegacySyntaxKind::RemovedImport,
                    range: source_range(source, node.start, node.end),
                });
            }
            TeraCstKind::Tag(TeraTagKind::LegacyDefinition) => {
                document.legacy_syntax.push(TeraLegacySyntax {
                    kind: TeraLegacySyntaxKind::RemovedDefinition,
                    range: source_range(source, node.start, node.end),
                });
            }
            _ => {}
        }
    }
}

fn parse_component_definition(
    source: &str,
    node: &TeraCstNode,
    content: &str,
) -> Option<TeraComponentDefinition> {
    let header = content.strip_prefix("component")?.trim();
    let open = header.find('(')?;
    let close = matching_delimiter(header, open, b'(', b')')?;
    let full_name = header[..open].trim().to_string();
    if full_name.is_empty() {
        return None;
    }
    let content_in_source = node.content(source);
    let header_offset = content_in_source.find(header)? + node.content_start;
    let arguments_offset = header_offset + open + 1;
    let mut arguments = Vec::new();
    let mut rest_argument = None;
    for (start, end) in split_top_level_spans(&header[open + 1..close], b',') {
        let raw = &header[open + 1 + start..open + 1 + end];
        let leading = raw.len() - raw.trim_start().len();
        let trailing = raw.trim_end().len();
        let raw = &raw[leading..trailing];
        if raw.is_empty() {
            continue;
        }
        let absolute_start = arguments_offset + start + leading;
        let absolute_end = arguments_offset + start + trailing;
        if let Some(rest) = raw.strip_prefix("...") {
            let name = rest.trim();
            if !name.is_empty() {
                rest_argument = Some(name.to_string());
            }
            continue;
        }
        let (declaration, default) = split_once_top_level(raw, '=')
            .map(|(left, right)| (left.trim(), Some(right.trim())))
            .unwrap_or((raw, None));
        let (name, argument_type) = declaration
            .split_once(':')
            .map(|(name, kind)| (name.trim(), Some(kind.trim().to_ascii_lowercase())))
            .unwrap_or((declaration.trim(), None));
        if name.is_empty() {
            continue;
        }
        arguments.push(TeraComponentParameter {
            name: name.to_string(),
            argument_type,
            required: default.is_none(),
            default_value: default.map(TeraSemanticExpression::parse),
            range: source_range(source, absolute_start, absolute_end),
        });
    }
    Some(TeraComponentDefinition {
        namespace: component_namespace(&full_name),
        name: full_name,
        arguments,
        rest_argument,
        range: source_range(source, node.start, node.end),
        body_range: None,
    })
}

fn parse_component_call(
    source: &str,
    node: &TeraCstNode,
    content: &str,
    parent_call: Option<usize>,
) -> Option<TeraComponentCall> {
    let trimmed = content.trim();
    let inner = trimmed
        .strip_prefix('<')?
        .trim_end_matches('>')
        .trim_end_matches('/')
        .trim();
    if inner.starts_with('/') {
        return None;
    }
    let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
    let name = inner[..name_end].trim().to_string();
    if name.is_empty() {
        return None;
    }
    let content_in_source = node.content(source);
    let inner_offset = node.content_start + content_in_source.find(inner)?;
    let mut arguments = Vec::new();
    let arguments_source = &inner[name_end..];
    for (start, end) in split_top_level_whitespace_spans(arguments_source) {
        let raw = arguments_source[start..end].trim();
        if raw.is_empty() {
            continue;
        }
        let absolute_start = inner_offset + name_end + start;
        let absolute_end = inner_offset + name_end + end;
        if let Some(spread) = raw
            .strip_prefix("{...")
            .and_then(|value| value.strip_suffix('}'))
        {
            arguments.push(TeraComponentArgument {
                name: None,
                expression: TeraSemanticExpression::parse(spread),
                spread: true,
                range: source_range(source, absolute_start, absolute_end),
            });
            continue;
        }
        let (argument_name, expression) = if let Some((key, value)) = split_once_top_level(raw, '=')
        {
            let expression = value
                .trim()
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
                .unwrap_or(value.trim());
            (
                key.trim().to_string(),
                TeraSemanticExpression::parse(expression),
            )
        } else {
            (raw.to_string(), TeraSemanticExpression::parse(raw))
        };
        arguments.push(TeraComponentArgument {
            name: Some(argument_name),
            expression,
            spread: false,
            range: source_range(source, absolute_start, absolute_end),
        });
    }
    let range = source_range(source, node.start, node.end);
    Some(TeraComponentCall {
        namespace: component_namespace(&name),
        name,
        arguments,
        range: range.clone(),
        call_range: range,
        body_range: None,
        parent_call,
    })
}

fn parse_expression(source: &str) -> TeraSemanticExpression {
    let source = source.trim();
    let mut pipe_spans = split_top_level_spans(source, b'|');
    let base_span = pipe_spans.first().copied().unwrap_or((0, source.len()));
    let mut filters = Vec::new();
    for (start, end) in pipe_spans.drain(1..) {
        let filter = source[start..end].trim();
        if !filter.is_empty() {
            filters.push(parse_call(filter));
        }
    }
    let base = source[base_span.0..base_span.1].trim();
    let (negated, base) = base
        .strip_prefix("not ")
        .map(|value| (true, value.trim()))
        .unwrap_or((false, base));
    TeraSemanticExpression {
        value: parse_value(base),
        negated,
        filters,
    }
}

fn parse_value(source: &str) -> TeraSemanticValue {
    let source = source.trim();
    if let Some(value) = quoted_value(source) {
        return TeraSemanticValue::String(value);
    }
    if source == "true" || source == "false" {
        return TeraSemanticValue::Boolean(source == "true");
    }
    if let Ok(value) = source.parse::<i64>() {
        return TeraSemanticValue::Integer(value);
    }
    if let Ok(value) = source.parse::<f64>() {
        return TeraSemanticValue::Float(value);
    }
    if let Some(rest) = source.strip_prefix("...") {
        return TeraSemanticValue::Spread(Box::new(TeraSemanticExpression::parse(rest)));
    }
    if let Some((truthy, rest)) = split_keyword_top_level(source, " if ") {
        if let Some((condition, falsy)) = split_keyword_top_level(rest, " else ") {
            return TeraSemanticValue::Ternary {
                condition: Box::new(TeraSemanticExpression::parse(condition)),
                truthy: Box::new(TeraSemanticExpression::parse(truthy)),
                falsy: Box::new(TeraSemanticExpression::parse(falsy)),
            };
        }
    }
    if source.starts_with('[') && source.ends_with(']') {
        let inner = &source[1..source.len() - 1];
        if let Some((value, rest)) = split_keyword_top_level(inner, " for ") {
            if let Some((binding, iterable_and_condition)) = split_keyword_top_level(rest, " in ") {
                let (iterable, condition) = split_keyword_top_level(iterable_and_condition, " if ")
                    .map(|(iterable, condition)| (iterable, Some(condition)))
                    .unwrap_or((iterable_and_condition, None));
                return TeraSemanticValue::Comprehension {
                    value: Box::new(TeraSemanticExpression::parse(value)),
                    binding: binding.trim().to_string(),
                    iterable: Box::new(TeraSemanticExpression::parse(iterable)),
                    condition: condition
                        .map(|value| Box::new(TeraSemanticExpression::parse(value))),
                };
            }
        }
        return TeraSemanticValue::Array(
            split_top_level_spans(inner, b',')
                .into_iter()
                .filter_map(|(start, end)| {
                    let value = inner[start..end].trim();
                    (!value.is_empty()).then(|| TeraSemanticExpression::parse(value))
                })
                .collect(),
        );
    }
    if source.starts_with('{') && source.ends_with('}') {
        let inner = &source[1..source.len() - 1];
        let mut values = BTreeMap::new();
        for (start, end) in split_top_level_spans(inner, b',') {
            let entry = inner[start..end].trim();
            if let Some((key, value)) = split_once_top_level(entry, ':') {
                values.insert(
                    quoted_value(key.trim()).unwrap_or_else(|| key.trim().to_string()),
                    TeraSemanticExpression::parse(value),
                );
            }
        }
        return TeraSemanticValue::Map(values);
    }
    if let Some((base, member)) = split_keyword_top_level(source, "?.") {
        return TeraSemanticValue::OptionalChain {
            value: Box::new(TeraSemanticExpression::parse(base)),
            member: member.trim().to_string(),
        };
    }
    if source.ends_with(']') {
        if let Some(open) = find_top_level_suffix_open(source, b'[', b']') {
            let slice = &source[open + 1..source.len() - 1];
            if let Some((start, end)) = split_once_top_level(slice, ':') {
                return TeraSemanticValue::Slice {
                    value: Box::new(TeraSemanticExpression::parse(&source[..open])),
                    start: (!start.trim().is_empty())
                        .then(|| Box::new(TeraSemanticExpression::parse(start))),
                    end: (!end.trim().is_empty())
                        .then(|| Box::new(TeraSemanticExpression::parse(end))),
                };
            }
        }
    }
    for operator in [" or ", " and "] {
        if let Some((left, right)) = split_keyword_top_level(source, operator) {
            return TeraSemanticValue::Logic {
                operator: operator.trim().to_string(),
                left: Box::new(TeraSemanticExpression::parse(left)),
                right: Box::new(TeraSemanticExpression::parse(right)),
            };
        }
    }
    if let Some((left, right)) = split_keyword_top_level(source, " not in ") {
        return TeraSemanticValue::In {
            negated: true,
            needle: Box::new(TeraSemanticExpression::parse(left)),
            haystack: Box::new(TeraSemanticExpression::parse(right)),
        };
    }
    if let Some((left, right)) = split_keyword_top_level(source, " in ") {
        return TeraSemanticValue::In {
            negated: false,
            needle: Box::new(TeraSemanticExpression::parse(left)),
            haystack: Box::new(TeraSemanticExpression::parse(right)),
        };
    }
    if source.ends_with(')') && source.contains('(') {
        return TeraSemanticValue::FunctionCall(parse_call(source));
    }
    if is_identifier_path(source) {
        TeraSemanticValue::Identifier(source.to_string())
    } else {
        TeraSemanticValue::Raw(source.to_string())
    }
}

fn parse_call(source: &str) -> TeraSemanticCall {
    let source = source.trim();
    let (qualified_name, arguments_source) = source
        .find('(')
        .and_then(|open| {
            source
                .strip_suffix(')')
                .map(|_| (&source[..open], &source[open + 1..source.len() - 1]))
        })
        .unwrap_or((source, ""));
    let (namespace, name) = qualified_name
        .rsplit_once('.')
        .map(|(namespace, name)| (Some(namespace.to_string()), name.to_string()))
        .unwrap_or((None, qualified_name.to_string()));
    let mut arguments = BTreeMap::new();
    for (index, (start, end)) in split_top_level_spans(arguments_source, b',')
        .into_iter()
        .enumerate()
    {
        let argument = arguments_source[start..end].trim();
        if argument.is_empty() {
            continue;
        }
        let (key, value) = split_once_top_level(argument, '=')
            .map(|(key, value)| (key.trim().to_string(), value.trim()))
            .unwrap_or_else(|| (format!("__positional_{index}"), argument));
        arguments.insert(key, TeraSemanticExpression::parse(value));
    }
    TeraSemanticCall {
        namespace,
        name,
        arguments,
    }
}

fn parse_for_header(content: &str) -> Option<(Vec<String>, &str)> {
    let header = content.strip_prefix("for")?.trim();
    let (bindings, container) = split_keyword_top_level(header, " in ")?;
    let bindings = bindings
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!bindings.is_empty()).then_some((bindings, container.trim()))
}

fn parse_assignment(content: &str) -> Option<(String, &str)> {
    let header = content
        .strip_prefix("set_global")
        .or_else(|| content.strip_prefix("set"))?
        .trim();
    let (key, value) = split_once_top_level(header, '=')?;
    Some((key.trim().to_string(), value.trim()))
}

fn source_range(source: &str, start: usize, end: usize) -> TeraSourceRange {
    let start = start.min(source.len());
    let end = end.max(start).min(source.len());
    let (line, column) = line_column(source, start);
    let (end_line, end_column) = line_column(source, end);
    TeraSourceRange {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map(|(_, suffix)| suffix.chars().count() + 1)
        .unwrap_or_else(|| prefix.chars().count() + 1);
    (line, column)
}

fn split_top_level_spans(source: &str, separator: u8) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut stack = Vec::<u8>::new();
    let mut quote = None;
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if byte == active && bytes.get(index.wrapping_sub(1)) != Some(&b'\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
        } else if matches!(byte, b'(' | b'[' | b'{') {
            stack.push(byte);
        } else if matches!(byte, b')' | b']' | b'}') {
            stack.pop();
        } else if byte == separator && stack.is_empty() {
            result.push((start, index));
            start = index + 1;
        }
        index += 1;
    }
    result.push((start, source.len()));
    result
}

fn split_top_level_whitespace_spans(source: &str) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start = None;
    let mut stack = Vec::<u8>::new();
    let mut quote = None;
    let bytes = source.as_bytes();
    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(active) = quote {
            if byte == active && bytes.get(index.wrapping_sub(1)) != Some(&b'\\') {
                quote = None;
            }
        } else if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
        } else if matches!(byte, b'(' | b'[' | b'{') {
            stack.push(byte);
        } else if matches!(byte, b')' | b']' | b'}') {
            stack.pop();
        }
        if byte.is_ascii_whitespace() && stack.is_empty() && quote.is_none() {
            if let Some(item_start) = start.take() {
                result.push((item_start, index));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(start) = start {
        result.push((start, source.len()));
    }
    result
}

fn split_once_top_level(source: &str, separator: char) -> Option<(&str, &str)> {
    let separator = separator as u8;
    let spans = split_top_level_spans(source, separator);
    (spans.len() > 1).then(|| {
        let first = spans[0];
        (&source[first.0..first.1], &source[first.1 + 1..])
    })
}

fn split_keyword_top_level<'a>(source: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let mut stack = Vec::<u8>::new();
    let mut quote = None;
    let bytes = source.as_bytes();
    let keyword_bytes = keyword.as_bytes();
    let mut index = 0usize;
    while index + keyword_bytes.len() <= bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if byte == active && bytes.get(index.wrapping_sub(1)) != Some(&b'\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
        } else if matches!(byte, b'(' | b'[' | b'{') {
            stack.push(byte);
        } else if matches!(byte, b')' | b']' | b'}') {
            stack.pop();
        } else if stack.is_empty() && &bytes[index..index + keyword_bytes.len()] == keyword_bytes {
            return Some((&source[..index], &source[index + keyword.len()..]));
        }
        index += 1;
    }
    None
}

fn matching_delimiter(source: &str, open: usize, opening: u8, closing: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut quote = None;
    for index in open..bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if byte == active && bytes.get(index.wrapping_sub(1)) != Some(&b'\\') {
                quote = None;
            }
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
        } else if byte == opening {
            depth += 1;
        } else if byte == closing {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn find_top_level_suffix_open(source: &str, opening: u8, closing: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    for index in (0..bytes.len()).rev() {
        if bytes[index] == closing {
            depth += 1;
        } else if bytes[index] == opening {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn quoted_value(source: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let quote = *bytes.first()?;
    (matches!(quote, b'\'' | b'"' | b'`') && bytes.last() == Some(&quote) && bytes.len() >= 2)
        .then(|| source[1..source.len() - 1].to_string())
}

fn extract_strings(source: &str) -> Vec<String> {
    let mut result = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let quote = bytes[index];
        if !matches!(quote, b'\'' | b'"' | b'`') {
            index += 1;
            continue;
        }
        let start = index + 1;
        index = start;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index < bytes.len() {
            result.push(source[start..index].to_string());
        }
        index += 1;
    }
    result
}

fn word_after(source: &str, keyword: &str) -> Option<String> {
    source
        .strip_prefix(keyword)?
        .split_whitespace()
        .next()
        .map(|value| {
            value
                .trim_matches(|character: char| {
                    !character.is_ascii_alphanumeric()
                        && character != '_'
                        && character != '.'
                        && character != '-'
                })
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn definition_name(source: &str) -> Option<String> {
    let header = source.strip_prefix("component")?.trim();
    Some(header.split('(').next()?.trim().to_string()).filter(|value| !value.is_empty())
}

fn component_call_name(source: &str) -> Option<String> {
    let inner = source
        .trim()
        .strip_prefix('<')?
        .trim_start_matches('/')
        .trim();
    Some(
        inner
            .split(|character: char| {
                character.is_whitespace() || character == '/' || character == '>'
            })
            .next()?
            .to_string(),
    )
    .filter(|value| !value.is_empty())
}

fn component_namespace(name: &str) -> Option<String> {
    name.rsplit_once('.')
        .map(|(namespace, _)| namespace.to_string())
}

fn is_component_call(content: &str) -> bool {
    let content = content.trim();
    content.starts_with('<') && !content.starts_with("</")
}

fn is_identifier_path(source: &str) -> bool {
    !source.is_empty()
        && source.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '_' | '.' | '-' | '?' | '[' | ']' | '\'' | '"')
        })
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|candidate| candidate == value) {
        values.push(value.to_string());
    }
}

fn push_unique_all(values: &mut Vec<String>, candidates: &[String]) {
    for candidate in candidates {
        push_unique(values, candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_graph::tera_cst::parse_tera_cst;

    #[test]
    fn indexes_typed_namespaced_components_and_exact_call_ranges() {
        let source = r#"{% component ui.card(title: string, count: integer = 2, ...attrs) %}
<article>{{ title }}</article>
{% endcomponent ui.card %}
{{<ui.card title="Exemplu" count={items[1:3] | length} {...extra} />}}
{% <ui.card title> %}<p>Body</p>{% </ui.card> %}"#;
        let cst = parse_tera_cst(source, "components.html");
        let semantic = cst.semantics().expect("semantic projection");

        assert_eq!(semantic.component_definitions.len(), 1);
        let definition = &semantic.component_definitions[0];
        assert_eq!(definition.name, "ui.card");
        assert_eq!(definition.namespace.as_deref(), Some("ui"));
        assert_eq!(definition.arguments.len(), 2);
        assert_eq!(
            definition.arguments[0].argument_type.as_deref(),
            Some("string")
        );
        assert!(definition.arguments[0].required);
        assert_eq!(definition.rest_argument.as_deref(), Some("attrs"));
        assert_eq!(semantic.component_calls.len(), 2);
        assert_eq!(
            &source[semantic.component_calls[0].range.start..semantic.component_calls[0].range.end],
            "{{<ui.card title=\"Exemplu\" count={items[1:3] | length} {...extra} />}}"
        );
        assert_eq!(
            semantic.component_calls[1]
                .body_range
                .as_ref()
                .map(|range| &source[range.start..range.end]),
            Some("<p>Body</p>")
        );
    }

    #[test]
    fn represents_new_expression_shapes_without_upstream_ast_access() {
        for expression in [
            "{\"name\": page.title}",
            "...attributes",
            "items[1:3]",
            "page.extra?.title",
            "page.title if page else \"fallback\"",
            "[item.title for item in pages if item.visible]",
        ] {
            assert!(!matches!(
                TeraSemanticExpression::parse(expression).value,
                TeraSemanticValue::Raw(_)
            ));
        }
    }
}
