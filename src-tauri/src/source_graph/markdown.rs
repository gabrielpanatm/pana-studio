use std::{
    collections::{HashMap, VecDeque},
    hash::{Hash, Hasher},
};

use crate::source_graph::{
    mixed_cst::parse_mixed_cst,
    model::{
        MarkdownProjection, MarkdownProjectionKind, MarkdownSourceBindingKind, SourceGraph,
        SourceNodeKind, SourceRange,
    },
    scan::ranges::source_range,
    tera_semantics::{
        TeraSemanticCall, TeraSemanticDocument, TeraSemanticExpression, TeraSemanticNode,
        TeraSemanticValue,
    },
};

#[cfg(test)]
use crate::source_graph::{
    identity::ProvisionalSourceNodeIdAllocator,
    tera::{parse_tera_items, TeraItemKind},
};

#[derive(Clone, Debug)]
pub(crate) struct MarkdownTemplateAnalysis {
    pub(crate) projections: Vec<MarkdownProjection>,
    #[cfg(test)]
    pub(crate) projection_by_location: HashMap<String, MarkdownProjection>,
}

#[derive(Clone, Debug)]
struct MarkdownProjectionAnchor {
    id: String,
    range: SourceRange,
    location: String,
}

#[derive(Clone, Debug)]
pub(crate) struct MarkdownSourceNode {
    pub(crate) id: String,
    pub(crate) kind: SourceNodeKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntityKind {
    Page,
    Section,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ValueProducer {
    Entity {
        kind: EntityKind,
        binding: MarkdownSourceBindingKind,
        static_content_path: Option<String>,
        runtime_owner: String,
    },
    PageCollection,
    Unknown,
}

#[derive(Default)]
struct SemanticSourceCursor {
    anchors: HashMap<SourceNodeKind, VecDeque<MarkdownProjectionAnchor>>,
}

impl SemanticSourceCursor {
    fn next(&mut self, kind: SourceNodeKind) -> Option<MarkdownProjectionAnchor> {
        self.anchors.get_mut(&kind).and_then(VecDeque::pop_front)
    }
}

pub(crate) fn build_markdown_projections(graph: &SourceGraph) -> Vec<MarkdownProjection> {
    let mut projections = graph
        .templates
        .iter()
        .flat_map(|template| template.markdown_projections.iter().cloned())
        .collect::<Vec<_>>();
    normalize_markdown_projections(&mut projections);
    projections
}

pub(crate) fn upsert_markdown_template(
    projections: &mut Vec<MarkdownProjection>,
    template: &crate::source_graph::model::SourceGraphTemplate,
) {
    projections.retain(|projection| projection.template_file != template.file);
    projections.extend(template.markdown_projections.iter().cloned());
    normalize_markdown_projections(projections);
}

fn normalize_markdown_projections(projections: &mut Vec<MarkdownProjection>) {
    projections.sort_by(|left, right| {
        left.template_file
            .cmp(&right.template_file)
            .then_with(|| {
                left.template_range
                    .as_ref()
                    .map(|range| range.start)
                    .cmp(&right.template_range.as_ref().map(|range| range.start))
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    projections.dedup_by(|left, right| left.id == right.id);
}

pub(crate) fn analyze_template_markdown_with_source_nodes(
    relative_path: &str,
    source: &str,
    template_root_id: &str,
    source_nodes: &[MarkdownSourceNode],
) -> MarkdownTemplateAnalysis {
    let graph_file = relative_path.trim_start_matches('/').replace('\\', "/");
    let template_name = logical_template_name(&graph_file);
    let mixed = parse_mixed_cst(source, &template_name);
    analyze_template(
        &graph_file,
        &template_name,
        template_root_id,
        source,
        mixed.tera.semantics(),
        source_nodes,
    )
}

#[cfg(test)]
pub(crate) fn analyze_template_markdown(
    relative_path: &str,
    source: &str,
) -> MarkdownTemplateAnalysis {
    let mut ids = ProvisionalSourceNodeIdAllocator::default();
    let root_id = ids.next();
    let source_nodes = parse_tera_items(source)
        .into_iter()
        .filter(|item| item.kind == TeraItemKind::Node)
        .filter_map(|item| {
            Some(MarkdownSourceNode {
                id: ids.next(),
                kind: item.node_kind?,
                start: item.start,
                end: item.end,
            })
        })
        .collect::<Vec<_>>();
    analyze_template_markdown_with_source_nodes(relative_path, source, &root_id, &source_nodes)
}

fn analyze_template(
    template_file: &str,
    _template_name: &str,
    _template_root_id: &str,
    source: &str,
    semantics: Option<&TeraSemanticDocument>,
    source_nodes: &[MarkdownSourceNode],
) -> MarkdownTemplateAnalysis {
    let mut cursor = source_anchors(template_file, source, source_nodes);
    let mut projections = Vec::new();
    if let Some(semantics) = semantics {
        collect_markdown_projections(
            &semantics.nodes,
            &mut cursor,
            &mut HashMap::new(),
            template_file,
            &mut projections,
        );
    }

    #[cfg(test)]
    let projection_by_location = projections
        .iter()
        .filter_map(|projection| {
            projection.template_range.as_ref().map(|range| {
                (
                    format!(
                        "{}:{}:{}",
                        projection.template_file, range.line, range.column
                    ),
                    projection.clone(),
                )
            })
        })
        .collect();
    MarkdownTemplateAnalysis {
        projections,
        #[cfg(test)]
        projection_by_location,
    }
}

fn source_anchors(
    template_file: &str,
    source: &str,
    source_nodes: &[MarkdownSourceNode],
) -> SemanticSourceCursor {
    let mut anchors = HashMap::<SourceNodeKind, VecDeque<MarkdownProjectionAnchor>>::new();
    for node in source_nodes {
        let range = source_range(source, node.start, node.end);
        anchors
            .entry(node.kind.clone())
            .or_default()
            .push_back(MarkdownProjectionAnchor {
                id: node.id.clone(),
                location: format!("{template_file}:{}:{}", range.line, range.column),
                range,
            });
    }
    SemanticSourceCursor { anchors }
}

fn collect_markdown_projections(
    nodes: &[TeraSemanticNode],
    cursor: &mut SemanticSourceCursor,
    environment: &mut HashMap<String, ValueProducer>,
    template_file: &str,
    output: &mut Vec<MarkdownProjection>,
) {
    for node in nodes {
        match node {
            TeraSemanticNode::Variable { expression } => {
                let anchor = cursor.next(SourceNodeKind::TeraVariable);
                if let (Some(anchor), Some(classification)) =
                    (anchor, classify_expression(expression, environment))
                {
                    output.push(projection_from_classification(
                        template_file,
                        anchor,
                        classification,
                    ));
                }
            }
            TeraSemanticNode::Set {
                key, global, value, ..
            } => {
                cursor.next(if *global {
                    SourceNodeKind::SetGlobal
                } else {
                    SourceNodeKind::Set
                });
                let producer = producer_for_expression(value, environment, key);
                environment.insert(key.clone(), producer);
            }
            TeraSemanticNode::For {
                key,
                value,
                container,
                body,
                empty_body,
            } => {
                let anchor = cursor.next(SourceNodeKind::For);
                let is_toc_collection = matches!(
                    &container.value,
                    TeraSemanticValue::Identifier(identifier) if identifier.ends_with(".toc")
                );
                let container = producer_for_container_expression(container, environment, value);
                let mut loop_environment = environment.clone();
                let item = if container == ValueProducer::PageCollection {
                    ValueProducer::Entity {
                        kind: EntityKind::Page,
                        binding: MarkdownSourceBindingKind::RuntimePage,
                        static_content_path: None,
                        runtime_owner: value.clone(),
                    }
                } else {
                    ValueProducer::Unknown
                };
                loop_environment.insert(value.clone(), item);
                if let Some(key) = key {
                    loop_environment.insert(key.clone(), ValueProducer::Unknown);
                }
                let toc_projection = anchor.and_then(|anchor| {
                    (is_toc_collection
                        && matches!(
                            &container,
                            ValueProducer::Entity {
                                kind: EntityKind::Page | EntityKind::Section,
                                ..
                            }
                        ))
                    .then(|| {
                        projection_from_classification(
                            template_file,
                            anchor,
                            classification_from_entity(
                                MarkdownProjectionKind::Toc,
                                container.clone(),
                            ),
                        )
                    })
                });
                if let Some(projection) = toc_projection {
                    output.push(projection);
                    let mut ignored = Vec::new();
                    collect_markdown_projections(
                        body,
                        cursor,
                        &mut loop_environment,
                        template_file,
                        &mut ignored,
                    );
                    if let Some(empty_body) = empty_body {
                        collect_markdown_projections(
                            empty_body,
                            cursor,
                            &mut environment.clone(),
                            template_file,
                            &mut ignored,
                        );
                    }
                    continue;
                }
                collect_markdown_projections(
                    body,
                    cursor,
                    &mut loop_environment,
                    template_file,
                    output,
                );
                if let Some(empty_body) = empty_body {
                    collect_markdown_projections(
                        empty_body,
                        cursor,
                        &mut environment.clone(),
                        template_file,
                        output,
                    );
                }
            }
            TeraSemanticNode::ComponentDefinition { body, .. } => {
                cursor.next(SourceNodeKind::ComponentDefinition);
                collect_markdown_projections(
                    body,
                    cursor,
                    &mut environment.clone(),
                    template_file,
                    output,
                );
            }
            TeraSemanticNode::ComponentCall { body, .. }
            | TeraSemanticNode::SetBlock { body, .. } => {
                collect_markdown_projections(
                    body,
                    cursor,
                    &mut environment.clone(),
                    template_file,
                    output,
                );
            }
            TeraSemanticNode::FilterSection { filter, body } => {
                let anchor = cursor.next(SourceNodeKind::Filter);
                if filter.name == "markdown" {
                    if let Some(anchor) = anchor {
                        output.push(projection_from_classification(
                            template_file,
                            anchor,
                            MarkdownClassification {
                                kind: MarkdownProjectionKind::Filter,
                                binding_kind: MarkdownSourceBindingKind::Unresolved,
                                static_content_path: None,
                                runtime_source_expression: None,
                            },
                        ));
                    }
                    let mut ignored = Vec::new();
                    collect_markdown_projections(
                        body,
                        cursor,
                        &mut environment.clone(),
                        template_file,
                        &mut ignored,
                    );
                    continue;
                }
                collect_markdown_projections(
                    body,
                    cursor,
                    &mut environment.clone(),
                    template_file,
                    output,
                );
            }
            TeraSemanticNode::Block { body, .. } => {
                cursor.next(SourceNodeKind::Block);
                collect_markdown_projections(
                    body,
                    cursor,
                    &mut environment.clone(),
                    template_file,
                    output,
                );
            }
            TeraSemanticNode::If {
                branches,
                otherwise,
            } => {
                cursor.next(SourceNodeKind::If);
                for branch in branches {
                    collect_markdown_projections(
                        &branch.body,
                        cursor,
                        &mut environment.clone(),
                        template_file,
                        output,
                    );
                }
                if let Some(otherwise) = otherwise {
                    collect_markdown_projections(
                        otherwise,
                        cursor,
                        &mut environment.clone(),
                        template_file,
                        output,
                    );
                }
            }
            _ => {}
        }
    }
}

fn classification_from_entity(
    kind: MarkdownProjectionKind,
    producer: ValueProducer,
) -> MarkdownClassification {
    match producer {
        ValueProducer::Entity {
            binding,
            static_content_path,
            runtime_owner,
            ..
        } => MarkdownClassification {
            kind,
            binding_kind: binding,
            static_content_path,
            runtime_source_expression: Some(format!("{runtime_owner}.relative_path")),
        },
        _ => MarkdownClassification {
            kind,
            binding_kind: MarkdownSourceBindingKind::Unresolved,
            static_content_path: None,
            runtime_source_expression: None,
        },
    }
}

#[derive(Clone, Debug)]
struct MarkdownClassification {
    kind: MarkdownProjectionKind,
    binding_kind: MarkdownSourceBindingKind,
    static_content_path: Option<String>,
    runtime_source_expression: Option<String>,
}

fn classify_expression(
    expression: &TeraSemanticExpression,
    environment: &HashMap<String, ValueProducer>,
) -> Option<MarkdownClassification> {
    let markdown_filter = expression
        .filters
        .iter()
        .any(|filter| filter.name == "markdown");
    let TeraSemanticValue::Identifier(identifier) = &expression.value else {
        return markdown_filter.then_some(MarkdownClassification {
            kind: MarkdownProjectionKind::Filter,
            binding_kind: MarkdownSourceBindingKind::Unresolved,
            static_content_path: None,
            runtime_source_expression: None,
        });
    };
    let field = identifier
        .rsplit_once('.')
        .map(|(_, field)| field)
        .unwrap_or(identifier.as_str());
    let producer = producer_for_identifier(identifier, environment);
    let semantic_kind = match (&producer, field) {
        (ValueProducer::Entity { .. }, "content") => Some(MarkdownProjectionKind::Body),
        (
            ValueProducer::Entity {
                kind: EntityKind::Page,
                ..
            },
            "summary",
        ) => Some(MarkdownProjectionKind::Summary),
        (ValueProducer::Entity { .. }, "toc") => Some(MarkdownProjectionKind::Toc),
        _ => None,
    };
    let kind = semantic_kind.or(markdown_filter.then_some(MarkdownProjectionKind::Filter))?;
    Some(classification_from_entity(kind, producer))
}

fn projection_from_classification(
    template_file: &str,
    anchor: MarkdownProjectionAnchor,
    classification: MarkdownClassification,
) -> MarkdownProjection {
    let _location_is_authoritative = anchor.location;
    MarkdownProjection {
        id: projection_id(&anchor.id, classification.kind),
        kind: classification.kind,
        template_source_node_id: anchor.id,
        template_file: template_file.to_string(),
        template_range: Some(anchor.range),
        binding_kind: classification.binding_kind,
        static_content_path: classification.static_content_path,
        runtime_source_expression: classification.runtime_source_expression,
    }
}

fn producer_for_expression(
    expression: &TeraSemanticExpression,
    environment: &HashMap<String, ValueProducer>,
    assigned_name: &str,
) -> ValueProducer {
    match &expression.value {
        TeraSemanticValue::Identifier(identifier) => {
            let mut producer = producer_for_identifier(identifier, environment);
            if let ValueProducer::Entity { runtime_owner, .. } = &mut producer {
                *runtime_owner = assigned_name.to_string();
            }
            producer
        }
        TeraSemanticValue::FunctionCall(call) if call.name == "get_page" => {
            entity_from_call(call, EntityKind::Page, assigned_name)
        }
        TeraSemanticValue::FunctionCall(call) if call.name == "get_section" => {
            entity_from_call(call, EntityKind::Section, assigned_name)
        }
        _ => ValueProducer::Unknown,
    }
}

fn producer_for_container_expression(
    expression: &TeraSemanticExpression,
    environment: &HashMap<String, ValueProducer>,
    loop_value: &str,
) -> ValueProducer {
    match &expression.value {
        TeraSemanticValue::Identifier(identifier) => {
            producer_for_identifier(identifier, environment)
        }
        TeraSemanticValue::FunctionCall(call) if call.name == "get_page" => {
            entity_from_call(call, EntityKind::Page, loop_value)
        }
        TeraSemanticValue::FunctionCall(call) if call.name == "get_section" => {
            entity_from_call(call, EntityKind::Section, loop_value)
        }
        _ => ValueProducer::Unknown,
    }
}

fn entity_from_call(
    call: &TeraSemanticCall,
    kind: EntityKind,
    runtime_owner: &str,
) -> ValueProducer {
    let static_content_path = call
        .arguments
        .get("path")
        .and_then(static_string_expression);
    let binding = match (kind, static_content_path.is_some()) {
        (EntityKind::Page, true) => MarkdownSourceBindingKind::StaticPage,
        (EntityKind::Section, true) => MarkdownSourceBindingKind::StaticSection,
        (EntityKind::Page, false) => MarkdownSourceBindingKind::RuntimePage,
        (EntityKind::Section, false) => MarkdownSourceBindingKind::RuntimeSection,
    };
    ValueProducer::Entity {
        kind,
        binding,
        static_content_path,
        runtime_owner: runtime_owner.to_string(),
    }
}

fn producer_for_identifier(
    identifier: &str,
    environment: &HashMap<String, ValueProducer>,
) -> ValueProducer {
    let root = root_identifier(identifier);
    let base = environment
        .get(root)
        .cloned()
        .unwrap_or_else(|| match root {
            "page" => ValueProducer::Entity {
                kind: EntityKind::Page,
                binding: MarkdownSourceBindingKind::CurrentPage,
                static_content_path: None,
                runtime_owner: root.to_string(),
            },
            "section" => ValueProducer::Entity {
                kind: EntityKind::Section,
                binding: MarkdownSourceBindingKind::CurrentSection,
                static_content_path: None,
                runtime_owner: root.to_string(),
            },
            _ => ValueProducer::Unknown,
        });
    if identifier == root {
        return base;
    }
    if identifier.ends_with(".pages") || identifier.ends_with(".pages[]") {
        return ValueProducer::PageCollection;
    }
    base
}

fn static_string_expression(expression: &TeraSemanticExpression) -> Option<String> {
    match &expression.value {
        TeraSemanticValue::String(value) => Some(value.trim_start_matches("@/").to_string()),
        _ => None,
    }
}

fn root_identifier(identifier: &str) -> &str {
    identifier.split(['.', '[']).next().unwrap_or(identifier)
}

fn projection_id(source_node_id: &str, kind: MarkdownProjectionKind) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "pana-markdown-projection-v1".hash(&mut hasher);
    source_node_id.hash(&mut hasher);
    format!("{kind:?}").hash(&mut hasher);
    format!("mdp_{:016x}", hasher.finish())
}

fn logical_template_name(path: &str) -> String {
    let normalized = path.trim_start_matches('/').replace('\\', "/");
    if let Some(theme_path) = normalized.strip_prefix("themes/") {
        if let Some((_, template)) = theme_path.split_once("/templates/") {
            return template.to_string();
        }
    }
    normalized
        .strip_prefix("templates/")
        .unwrap_or(normalized.as_str())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<MarkdownProjectionKind> {
        analyze_template_markdown("templates/page.html", source)
            .projections
            .into_iter()
            .map(|projection| projection.kind)
            .collect()
    }

    #[test]
    fn classifies_direct_body_summary_filter_and_toc() {
        let source = "{{ page.content | safe }}{{ page.summary | safe }}{{ page.toc }}{{ value | markdown | safe }}";
        assert_eq!(
            kinds(source),
            vec![
                MarkdownProjectionKind::Body,
                MarkdownProjectionKind::Summary,
                MarkdownProjectionKind::Toc,
                MarkdownProjectionKind::Filter,
            ]
        );
    }

    #[test]
    fn resolves_get_page_and_collection_aliases() {
        let source = r#"{% set article = get_page(path="blog/post.md") %}
{{ article.content | safe }}
{% for item in paginator.pages %}{{ item.summary | safe }}{% endfor %}"#;
        let analysis = analyze_template_markdown("templates/page.html", source);
        assert_eq!(analysis.projections.len(), 2);
        assert_eq!(
            analysis.projections[0].binding_kind,
            MarkdownSourceBindingKind::StaticPage
        );
        assert_eq!(
            analysis.projections[0].static_content_path.as_deref(),
            Some("blog/post.md")
        );
        assert_eq!(
            analysis.projections[1].binding_kind,
            MarkdownSourceBindingKind::RuntimePage
        );
        assert_eq!(
            analysis.projections[1].runtime_source_expression.as_deref(),
            Some("item.relative_path")
        );
    }

    #[test]
    fn classifies_markdown_filter_section_and_atomic_toc_loop() {
        let source = r#"{% filter markdown %}# Titlu{% endfilter %}
{% for heading in page.toc %}<a href="{{ heading.permalink }}">{{ heading.title }}</a>{% endfor %}"#;
        let analysis = analyze_template_markdown("templates/page.html", source);
        assert_eq!(
            analysis
                .projections
                .iter()
                .map(|projection| projection.kind)
                .collect::<Vec<_>>(),
            vec![MarkdownProjectionKind::Filter, MarkdownProjectionKind::Toc]
        );
        assert_eq!(
            analysis.projections[1].runtime_source_expression.as_deref(),
            Some("page.relative_path")
        );
    }

    #[test]
    fn preserves_theme_template_identity_and_dynamic_section_alias() {
        let source =
            r#"{% set selected = get_section(path=section_path) %}{{ selected.content | safe }}"#;
        let analysis =
            analyze_template_markdown("themes/anemone/templates/sections/list.html", source);
        assert_eq!(analysis.projections.len(), 1);
        let projection = &analysis.projections[0];
        assert_eq!(
            projection.template_file,
            "themes/anemone/templates/sections/list.html"
        );
        assert_eq!(
            projection.binding_kind,
            MarkdownSourceBindingKind::RuntimeSection
        );
        assert_eq!(
            projection.runtime_source_expression.as_deref(),
            Some("selected.relative_path")
        );
    }
}
