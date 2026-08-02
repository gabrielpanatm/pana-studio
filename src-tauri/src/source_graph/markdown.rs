use std::{
    collections::{HashMap, VecDeque},
    hash::{Hash, Hasher},
};

use crate::source_graph::{
    identity::{stable_source_node_id, SourceIdentityAssigner},
    mixed_cst::parse_mixed_cst,
    model::{
        MarkdownProjection, MarkdownProjectionKind, MarkdownSourceBindingKind, SourceGraph,
        SourceNodeKind, SourceRange,
    },
    scan::ranges::source_range,
    tera::{parse_tera_items, TeraItemKind},
    tera_semantics::{
        TeraSemanticCall, TeraSemanticDocument, TeraSemanticExpression, TeraSemanticNode,
        TeraSemanticValue,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct MarkdownTemplateAnalysis {
    pub(crate) projections: Vec<MarkdownProjection>,
    pub(crate) projection_by_location: HashMap<String, MarkdownProjection>,
    pub(crate) shortcode_projection: Option<MarkdownProjection>,
}

#[derive(Clone, Debug)]
struct SourceAnchor {
    id: String,
    range: SourceRange,
    location: String,
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
    anchors: HashMap<SourceNodeKind, VecDeque<SourceAnchor>>,
}

impl SemanticSourceCursor {
    fn next(&mut self, kind: SourceNodeKind) -> Option<SourceAnchor> {
        self.anchors.get_mut(&kind).and_then(VecDeque::pop_front)
    }
}

pub(crate) fn build_markdown_projections(graph: &SourceGraph) -> Vec<MarkdownProjection> {
    let mut projections = graph
        .templates
        .iter()
        .flat_map(|template| template.markdown_projections.iter().cloned())
        .collect::<Vec<_>>();
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
    projections
}

pub(crate) fn analyze_template_markdown(
    relative_path: &str,
    source: &str,
) -> MarkdownTemplateAnalysis {
    let graph_file = relative_path.trim_start_matches('/').replace('\\', "/");
    let template_name = logical_template_name(&graph_file);
    let is_partial = is_partial_template_name(&template_name);
    let root_kind = if is_partial {
        SourceNodeKind::Partial
    } else {
        SourceNodeKind::Template
    };
    let root_id = stable_source_node_id(&graph_file, &root_kind, &template_name, 0);
    let mixed = parse_mixed_cst(source, &template_name);
    analyze_template(
        &graph_file,
        &template_name,
        &root_id,
        source,
        mixed.tera.semantics(),
    )
}

fn analyze_template(
    template_file: &str,
    template_name: &str,
    template_root_id: &str,
    source: &str,
    semantics: Option<&TeraSemanticDocument>,
) -> MarkdownTemplateAnalysis {
    let mut cursor = source_anchors(template_file, source);
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

    let shortcode_projection = template_name
        .strip_prefix("shortcodes/")
        .filter(|name| name.ends_with(".html") || name.ends_with(".md"))
        .map(|_| MarkdownProjection {
            id: projection_id(template_root_id, MarkdownProjectionKind::Shortcode),
            kind: MarkdownProjectionKind::Shortcode,
            template_source_node_id: template_root_id.to_string(),
            template_file: template_file.to_string(),
            template_range: (!source.is_empty()).then(|| source_range(source, 0, source.len())),
            binding_kind: MarkdownSourceBindingKind::ShortcodeInvocation,
            static_content_path: None,
            runtime_source_expression: None,
        });
    if let Some(shortcode) = shortcode_projection.as_ref() {
        projections.push(shortcode.clone());
    }
    let projection_by_location = projections
        .iter()
        .filter(|projection| projection.kind != MarkdownProjectionKind::Shortcode)
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
        projection_by_location,
        shortcode_projection,
    }
}

fn source_anchors(template_file: &str, source: &str) -> SemanticSourceCursor {
    let mut identities = SourceIdentityAssigner::default();
    let mut anchors = HashMap::<SourceNodeKind, VecDeque<SourceAnchor>>::new();
    for item in parse_tera_items(source) {
        if item.kind != TeraItemKind::Node {
            continue;
        }
        let Some(kind) = item.node_kind else {
            continue;
        };
        let id = identities.next(template_file, &kind, &item.label);
        let range = source_range(source, item.start, item.end);
        anchors.entry(kind).or_default().push_back(SourceAnchor {
            id,
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
            TeraSemanticNode::MacroDefinition { body, .. } => {
                cursor.next(SourceNodeKind::Macro);
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
        return markdown_filter.then(|| MarkdownClassification {
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
    anchor: SourceAnchor,
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
    identifier
        .split(|character| matches!(character, '.' | '['))
        .next()
        .unwrap_or(identifier)
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

fn is_partial_template_name(name: &str) -> bool {
    name.starts_with("partials/") || name.starts_with("macros/") || name.starts_with("shortcodes/")
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
    fn shortcode_is_a_distinct_atomic_projection() {
        let analysis = analyze_template_markdown(
            "templates/shortcodes/notice.html",
            "<aside>{{ body | markdown | safe }}</aside>",
        );
        assert!(analysis
            .projections
            .iter()
            .any(|projection| projection.kind == MarkdownProjectionKind::Shortcode));
        assert!(analysis.shortcode_projection.is_some());
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
