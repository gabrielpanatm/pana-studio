use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{Hash, Hasher},
};

use crate::{
    localization::LocalizedDiagnostic,
    source_graph::{
        model::{
            ComponentArgument, ComponentCapabilities, ComponentDefinition, ComponentDefinitionKind,
            ComponentDependency, ComponentDependencyKind, ComponentDiagnostic, ComponentGraph,
            ComponentInvocation, ComponentInvocationKind, ComponentOrigin, ComponentParameter,
            ComponentResolutionStatus, SourceDiagnosticSeverity, SourceGraph, SourceGraphTemplate,
            SourceNode, SourceNodeKind, SourceOrigin, SourceRange, SourceRelationKind,
        },
        tera_semantics::{
            TeraComponentArgument, TeraComponentCall, TeraComponentDefinition,
            TeraComponentParameter, TeraSemanticExpression, TeraSemanticValue, TeraSourceRange,
        },
        zola::{collect_zola_runtime_uses, normalize_zola_template_reference},
    },
};

pub(crate) const COMPONENT_GRAPH_SCHEMA_VERSION: u32 = 4;

pub(crate) fn build_component_graph(source_graph: &SourceGraph) -> ComponentGraph {
    ComponentGraphBuilder::new(source_graph).build()
}

pub(crate) fn upsert_component_graph_template(
    source_graph: &SourceGraph,
    previous: ComponentGraph,
    template_file: &str,
) -> ComponentGraph {
    let mut builder = ComponentGraphBuilder::from_previous(source_graph, previous, template_file);
    builder.project_template_definitions(Some(template_file));
    builder.reconcile_template_shadowing();
    builder.project_tera_component_definitions(Some(template_file));
    builder.project_structural_definitions(Some(template_file));
    builder.project_include_invocations(Some(template_file));
    // Component names are global in Tera 2. Definitions from unaffected files
    // are reusable, but every component call must be resolved again against the
    // resulting global symbol table.
    builder.project_tera_component_invocations();
    builder.project_structural_invocations(Some(template_file));
    builder.reconcile_consumers();
    builder.sort_output();
    ComponentGraph {
        schema_version: COMPONENT_GRAPH_SCHEMA_VERSION,
        definitions: builder.definitions,
        invocations: builder.invocations,
        rendered_instances: Vec::new(),
        diagnostics: builder.diagnostics,
    }
}

struct ComponentGraphBuilder<'a> {
    graph: &'a SourceGraph,
    nodes_by_file: HashMap<&'a str, Vec<&'a SourceNode>>,
    definitions: Vec<ComponentDefinition>,
    invocations: Vec<ComponentInvocation>,
    diagnostics: Vec<ComponentDiagnostic>,
    template_definition_by_node: HashMap<String, String>,
    effective_template_definition_by_name: HashMap<String, String>,
    component_definitions_by_name: HashMap<String, Vec<String>>,
}

impl<'a> ComponentGraphBuilder<'a> {
    fn new(graph: &'a SourceGraph) -> Self {
        let mut nodes_by_file = HashMap::<&str, Vec<&SourceNode>>::new();
        for node in &graph.nodes {
            nodes_by_file
                .entry(node.file.as_str())
                .or_default()
                .push(node);
        }
        Self {
            graph,
            nodes_by_file,
            definitions: Vec::new(),
            invocations: Vec::new(),
            diagnostics: Vec::new(),
            template_definition_by_node: HashMap::new(),
            effective_template_definition_by_name: HashMap::new(),
            component_definitions_by_name: HashMap::new(),
        }
    }

    fn from_previous(
        graph: &'a SourceGraph,
        previous: ComponentGraph,
        template_file: &str,
    ) -> Self {
        let mut definitions = previous
            .definitions
            .into_iter()
            .filter(|definition| definition.file.as_deref() != Some(template_file))
            .collect::<Vec<_>>();
        for definition in &mut definitions {
            definition
                .diagnostics
                .retain(|diagnostic| diagnostic.code != "duplicate_tera_component");
        }
        let invocations = previous
            .invocations
            .into_iter()
            .filter(|invocation| {
                invocation.file != template_file
                    && invocation.kind != ComponentInvocationKind::TeraComponent
            })
            .collect::<Vec<_>>();
        let template_definition_by_node = definitions
            .iter()
            .filter(|definition| definition.owner_definition_id.is_none())
            .filter_map(|definition| {
                Some((definition.source_node_id.clone()?, definition.id.clone()))
            })
            .collect();
        let mut component_definitions_by_name = HashMap::<String, Vec<String>>::new();
        for definition in definitions
            .iter()
            .filter(|definition| definition.kind == ComponentDefinitionKind::TeraComponent)
        {
            component_definitions_by_name
                .entry(definition.name.clone())
                .or_default()
                .push(definition.id.clone());
        }
        let mut builder = Self::new(graph);
        builder.definitions = definitions;
        builder.invocations = invocations;
        builder.template_definition_by_node = template_definition_by_node;
        builder.component_definitions_by_name = component_definitions_by_name;
        builder
    }

    fn build(mut self) -> ComponentGraph {
        self.project_template_definitions(None);
        self.reconcile_template_shadowing();
        self.project_tera_component_definitions(None);
        self.project_structural_definitions(None);
        self.project_include_invocations(None);
        self.project_tera_component_invocations();
        self.project_structural_invocations(None);
        self.reconcile_consumers();
        self.sort_output();
        ComponentGraph {
            schema_version: COMPONENT_GRAPH_SCHEMA_VERSION,
            definitions: self.definitions,
            invocations: self.invocations,
            rendered_instances: Vec::new(),
            diagnostics: self.diagnostics,
        }
    }

    fn project_template_definitions(&mut self, only_file: Option<&str>) {
        for template in self
            .graph
            .templates
            .iter()
            .filter(|template| only_file.is_none_or(|file| template.file == file))
        {
            let id = component_id("definition", &["template", template.node_id.as_str()]);
            self.template_definition_by_node
                .insert(template.node_id.clone(), id.clone());
            self.definitions.push(ComponentDefinition {
                id,
                kind: if template.is_partial {
                    ComponentDefinitionKind::Partial
                } else {
                    ComponentDefinitionKind::TemplateFile
                },
                name: template.name.clone(),
                display_name: display_name_for_template(&template.name),
                origin: component_origin(&template.origin),
                theme_name: template.theme_name.clone(),
                file: Some(template.file.clone()),
                template_name: Some(template.name.clone()),
                source_node_id: Some(template.node_id.clone()),
                owner_definition_id: None,
                symbol: None,
                range: self
                    .graph
                    .node_by_id(&template.node_id)
                    .and_then(|node| node.range.clone()),
                body_range: None,
                rest_parameter: None,
                parameters: Vec::new(),
                context_dependencies: Vec::new(),
                data_bindings: Vec::new(),
                dependencies: dependencies_for_template(self.graph, template),
                consumer_invocation_ids: Vec::new(),
                shadowed_by: None,
                active: true,
                capabilities: file_capabilities(matches!(template.origin, SourceOrigin::Local)),
                diagnostics: Vec::new(),
            });
        }
    }

    fn reconcile_template_shadowing(&mut self) {
        let mut grouped = BTreeMap::<String, Vec<usize>>::new();
        for (index, definition) in self.definitions.iter().enumerate() {
            if definition.owner_definition_id.is_none() {
                if let Some(name) = definition.template_name.as_deref() {
                    grouped
                        .entry(normalize_zola_template_reference(name))
                        .or_default()
                        .push(index);
                }
            }
        }
        for (name, indexes) in grouped {
            let active = indexes
                .iter()
                .copied()
                .find(|index| self.definitions[*index].origin == ComponentOrigin::Project)
                .or_else(|| indexes.first().copied());
            let Some(active) = active else {
                continue;
            };
            let active_id = self.definitions[active].id.clone();
            self.effective_template_definition_by_name
                .insert(name, active_id.clone());
            for index in indexes {
                self.definitions[index].active = index == active;
                self.definitions[index].shadowed_by = (index != active).then(|| active_id.clone());
            }
        }
    }

    fn project_tera_component_definitions(&mut self, only_file: Option<&str>) {
        for template in self
            .graph
            .templates
            .iter()
            .filter(|template| only_file.is_none_or(|file| template.file == file))
        {
            let Some(owner_id) = self
                .template_definition_by_node
                .get(&template.node_id)
                .cloned()
            else {
                continue;
            };
            let owner_active = self
                .definitions
                .iter()
                .find(|definition| definition.id == owner_id)
                .is_none_or(|definition| definition.active);
            for definition in &template.component_definitions {
                let source_node = source_node_for_range(
                    self.graph,
                    &template.file,
                    SourceNodeKind::ComponentDefinition,
                    &definition.range,
                );
                let id = component_id(
                    "definition",
                    &[
                        "tera-component",
                        definition.name.as_str(),
                        template.node_id.as_str(),
                        &definition.range.start.to_string(),
                    ],
                );
                let mut diagnostics = Vec::new();
                if !owner_active {
                    diagnostics.push(component_diagnostic(
                        "component_shadowed_template",
                        LocalizedDiagnostic::new("components-diagnostic-shadowed-component")
                            .with_argument("name", definition.name.clone()),
                        SourceDiagnosticSeverity::Warning,
                        Some(template.file.clone()),
                        source_node.map(|node| node.id.clone()),
                    ));
                }
                let component = ComponentDefinition {
                    id: id.clone(),
                    kind: ComponentDefinitionKind::TeraComponent,
                    name: definition.name.clone(),
                    display_name: definition.name.clone(),
                    origin: component_origin(&template.origin),
                    theme_name: template.theme_name.clone(),
                    file: Some(template.file.clone()),
                    template_name: Some(template.name.clone()),
                    source_node_id: source_node.map(|node| node.id.clone()),
                    owner_definition_id: Some(owner_id.clone()),
                    symbol: Some(definition.name.clone()),
                    range: Some(tera_range_to_source_range(&definition.range)),
                    body_range: definition
                        .body_range
                        .as_ref()
                        .map(tera_range_to_source_range),
                    rest_parameter: definition.rest_argument.clone(),
                    parameters: component_parameters(definition),
                    context_dependencies: Vec::new(),
                    data_bindings: Vec::new(),
                    dependencies: dependencies_for_template(self.graph, template),
                    consumer_invocation_ids: Vec::new(),
                    shadowed_by: None,
                    active: owner_active,
                    capabilities: symbol_capabilities(matches!(
                        template.origin,
                        SourceOrigin::Local
                    )),
                    diagnostics,
                };
                self.component_definitions_by_name
                    .entry(definition.name.clone())
                    .or_default()
                    .push(id);
                self.definitions.push(component);
            }
        }

        for definition_ids in self.component_definitions_by_name.values() {
            let active = definition_ids
                .iter()
                .filter(|id| {
                    self.definitions
                        .iter()
                        .find(|definition| definition.id == **id)
                        .is_some_and(|definition| definition.active)
                })
                .cloned()
                .collect::<Vec<_>>();
            if active.len() <= 1 {
                continue;
            }
            for id in active {
                if let Some(definition) = self
                    .definitions
                    .iter_mut()
                    .find(|definition| definition.id == id)
                {
                    let diagnostic = component_diagnostic(
                        "duplicate_tera_component",
                        LocalizedDiagnostic::new("components-diagnostic-duplicate-component")
                            .with_argument("name", definition.name.clone()),
                        SourceDiagnosticSeverity::Error,
                        definition.file.clone(),
                        definition.source_node_id.clone(),
                    );
                    definition.diagnostics.push(diagnostic.clone());
                    self.diagnostics.push(diagnostic);
                }
            }
        }
    }

    fn project_structural_definitions(&mut self, only_file: Option<&str>) {
        let structural = [
            (
                SourceNodeKind::Block,
                ComponentDefinitionKind::TemplateBlock,
            ),
            (SourceNodeKind::For, ComponentDefinitionKind::InlineRepeat),
            (
                SourceNodeKind::If,
                ComponentDefinitionKind::InlineConditional,
            ),
            (
                SourceNodeKind::Filter,
                ComponentDefinitionKind::InlineTransform,
            ),
        ];
        for template in self
            .graph
            .templates
            .iter()
            .filter(|template| only_file.is_none_or(|file| template.file == file))
        {
            let owner_id = self
                .template_definition_by_node
                .get(&template.node_id)
                .cloned();
            let template_nodes = self
                .nodes_by_file
                .get(template.file.as_str())
                .cloned()
                .unwrap_or_default();
            for (node_kind, definition_kind) in &structural {
                for node in template_nodes
                    .iter()
                    .copied()
                    .filter(|node| node.kind == *node_kind)
                {
                    self.definitions.push(ComponentDefinition {
                        id: component_id("definition", &["structural", node.id.as_str()]),
                        kind: definition_kind.clone(),
                        name: format!("{}#{}", template.name, node.label),
                        display_name: node.label.clone(),
                        origin: component_origin(&template.origin),
                        theme_name: template.theme_name.clone(),
                        file: Some(template.file.clone()),
                        template_name: Some(template.name.clone()),
                        source_node_id: Some(node.id.clone()),
                        owner_definition_id: owner_id.clone(),
                        symbol: Some(node.label.clone()),
                        range: node.range.clone(),
                        body_range: None,
                        rest_parameter: None,
                        parameters: Vec::new(),
                        context_dependencies: Vec::new(),
                        data_bindings: Vec::new(),
                        dependencies: dependencies_for_template(self.graph, template),
                        consumer_invocation_ids: Vec::new(),
                        shadowed_by: None,
                        active: true,
                        capabilities: symbol_capabilities(matches!(
                            template.origin,
                            SourceOrigin::Local
                        )),
                        diagnostics: Vec::new(),
                    });
                }
            }
        }
    }

    fn project_include_invocations(&mut self, only_file: Option<&str>) {
        for template in self
            .graph
            .templates
            .iter()
            .filter(|template| only_file.is_none_or(|file| template.file == file))
        {
            let owner = self
                .template_definition_by_node
                .get(&template.node_id)
                .cloned();
            let source_nodes = self
                .nodes_by_file
                .get(template.file.as_str())
                .into_iter()
                .flatten()
                .copied()
                .filter(|node| node.kind == SourceNodeKind::Include)
                .collect::<Vec<_>>();
            for (index, include) in template.include_groups.iter().enumerate() {
                let source_node = source_nodes.get(index).copied();
                let resolved = include
                    .targets
                    .iter()
                    .find_map(|target| {
                        self.effective_template_definition_by_name
                            .get(&normalize_zola_template_reference(target))
                            .cloned()
                    })
                    .into_iter()
                    .collect::<Vec<_>>();
                let mut diagnostics = Vec::new();
                if resolved.is_empty() && !include.ignore_missing {
                    diagnostics.push(component_diagnostic(
                        "unresolved_include",
                        LocalizedDiagnostic::new("components-diagnostic-unresolved-include")
                            .with_argument("targets", include.targets.join(", ")),
                        SourceDiagnosticSeverity::Error,
                        Some(template.file.clone()),
                        source_node.map(|node| node.id.clone()),
                    ));
                }
                self.invocations.push(ComponentInvocation {
                    id: component_id(
                        "invocation",
                        &[
                            "include",
                            template.node_id.as_str(),
                            source_node
                                .map(|node| node.id.as_str())
                                .unwrap_or("dynamic"),
                        ],
                    ),
                    kind: ComponentInvocationKind::Include,
                    name: format!("Include {}", include.targets.join(" | ")),
                    file: template.file.clone(),
                    source_node_id: source_node.map(|node| node.id.clone()),
                    owner_definition_id: owner.clone(),
                    parent_invocation_id: None,
                    target_reference: include.targets.first().cloned().unwrap_or_default(),
                    resolved_definition_ids: resolved.clone(),
                    fallback_references: include.targets.iter().skip(1).cloned().collect(),
                    range: source_node.and_then(|node| node.range.clone()),
                    call_range: source_node.and_then(|node| node.range.clone()),
                    body_range: None,
                    arguments: Vec::new(),
                    context_dependencies: Vec::new(),
                    data_bindings: Vec::new(),
                    status: if resolved.is_empty() {
                        if include.ignore_missing {
                            ComponentResolutionStatus::External
                        } else {
                            ComponentResolutionStatus::Unresolved
                        }
                    } else if include.targets.first().is_some_and(|target| {
                        self.effective_template_definition_by_name
                            .contains_key(&normalize_zola_template_reference(target))
                    }) {
                        ComponentResolutionStatus::Resolved
                    } else {
                        ComponentResolutionStatus::FallbackResolved
                    },
                    diagnostics,
                });
            }
        }
    }

    fn project_tera_component_invocations(&mut self) {
        for template in &self.graph.templates {
            let owner = self
                .template_definition_by_node
                .get(&template.node_id)
                .cloned();
            self.project_calls(
                &template.file,
                owner,
                &template.component_calls,
                Some(template),
            );
        }
        for page in &self.graph.pages {
            self.project_calls(&page.file, None, &page.component_calls, None);
        }
    }

    fn project_calls(
        &mut self,
        file: &str,
        owner: Option<String>,
        calls: &[TeraComponentCall],
        template: Option<&SourceGraphTemplate>,
    ) {
        let mut invocation_ids = Vec::with_capacity(calls.len());
        for call in calls {
            let source_node = source_node_for_range(
                self.graph,
                file,
                SourceNodeKind::ComponentCall,
                &call.call_range,
            );
            let resolved = self
                .component_definitions_by_name
                .get(&call.name)
                .into_iter()
                .flat_map(|ids| ids.iter())
                .filter(|id| {
                    self.definitions
                        .iter()
                        .find(|definition| definition.id == **id)
                        .is_some_and(|definition| definition.active)
                })
                .cloned()
                .collect::<Vec<_>>();
            let mut diagnostics =
                self.component_call_diagnostics(call, file, source_node, &resolved);
            let status = match resolved.len() {
                0 => ComponentResolutionStatus::Unresolved,
                1 => ComponentResolutionStatus::Resolved,
                _ => ComponentResolutionStatus::Ambiguous,
            };
            if resolved.is_empty() {
                diagnostics.push(component_diagnostic(
                    "unresolved_tera_component",
                    LocalizedDiagnostic::new("components-diagnostic-unresolved-component")
                        .with_argument("name", call.name.clone()),
                    SourceDiagnosticSeverity::Error,
                    Some(file.to_string()),
                    source_node.map(|node| node.id.clone()),
                ));
            }
            let id = component_id(
                "invocation",
                &[
                    "tera-component",
                    file,
                    call.name.as_str(),
                    &call.range.start.to_string(),
                ],
            );
            let parent_invocation_id = call
                .parent_call
                .and_then(|index| invocation_ids.get(index))
                .cloned();
            invocation_ids.push(id.clone());
            self.invocations.push(ComponentInvocation {
                id,
                kind: ComponentInvocationKind::TeraComponent,
                name: call.name.clone(),
                file: file.to_string(),
                source_node_id: source_node.map(|node| node.id.clone()),
                owner_definition_id: owner.clone(),
                parent_invocation_id,
                target_reference: call.name.clone(),
                resolved_definition_ids: resolved,
                fallback_references: Vec::new(),
                range: Some(tera_range_to_source_range(&call.range)),
                call_range: Some(tera_range_to_source_range(&call.call_range)),
                body_range: call.body_range.as_ref().map(tera_range_to_source_range),
                arguments: call.arguments.iter().map(component_argument).collect(),
                context_dependencies: context_dependencies_for_call(call),
                data_bindings: Vec::new(),
                status,
                diagnostics,
            });
        }

        if let Some(template) = template {
            for invocation in self
                .invocations
                .iter_mut()
                .filter(|invocation| invocation.file == template.file)
            {
                if invocation.owner_definition_id.is_none() {
                    invocation.owner_definition_id = owner.clone();
                }
            }
        }
    }

    fn component_call_diagnostics(
        &self,
        call: &TeraComponentCall,
        file: &str,
        source_node: Option<&SourceNode>,
        resolved: &[String],
    ) -> Vec<ComponentDiagnostic> {
        let Some(definition) = resolved.first().and_then(|id| {
            self.definitions
                .iter()
                .find(|definition| definition.id == *id)
        }) else {
            return Vec::new();
        };
        let supplied = call
            .arguments
            .iter()
            .filter_map(|argument| argument.name.as_deref())
            .collect::<BTreeSet<_>>();
        let has_spread = call.arguments.iter().any(|argument| argument.spread);
        let mut diagnostics = Vec::new();
        for parameter in &definition.parameters {
            if parameter.rest || !parameter.required || supplied.contains(parameter.name.as_str()) {
                continue;
            }
            diagnostics.push(component_diagnostic(
                "missing_component_argument",
                LocalizedDiagnostic::new("components-diagnostic-missing-argument")
                    .with_argument("component", call.name.clone())
                    .with_argument("argument", parameter.name.clone()),
                SourceDiagnosticSeverity::Error,
                Some(file.to_string()),
                source_node.map(|node| node.id.clone()),
            ));
        }
        if definition.rest_parameter.is_none() && !has_spread {
            for argument in &call.arguments {
                let Some(name) = argument.name.as_deref() else {
                    continue;
                };
                if !definition
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == name)
                {
                    diagnostics.push(component_diagnostic(
                        "unknown_component_argument",
                        LocalizedDiagnostic::new("components-diagnostic-unknown-argument")
                            .with_argument("component", call.name.clone())
                            .with_argument("argument", name.to_string()),
                        SourceDiagnosticSeverity::Error,
                        Some(file.to_string()),
                        source_node.map(|node| node.id.clone()),
                    ));
                }
            }
        }
        for argument in &call.arguments {
            let Some(name) = argument.name.as_deref() else {
                continue;
            };
            let Some(parameter) = definition
                .parameters
                .iter()
                .find(|parameter| parameter.name == name)
            else {
                continue;
            };
            if parameter
                .argument_type
                .as_deref()
                .is_some_and(|kind| !expression_matches_type(&argument.expression, kind))
            {
                diagnostics.push(component_diagnostic(
                    "incompatible_component_argument",
                    LocalizedDiagnostic::new("components-diagnostic-incompatible-argument")
                        .with_argument("component", call.name.clone())
                        .with_argument("argument", name.to_string())
                        .with_argument("type", parameter.argument_type.clone().unwrap_or_default()),
                    SourceDiagnosticSeverity::Error,
                    Some(file.to_string()),
                    source_node.map(|node| node.id.clone()),
                ));
            }
        }
        diagnostics
    }

    fn project_structural_invocations(&mut self, only_file: Option<&str>) {
        let mappings = [
            (
                ComponentDefinitionKind::InlineRepeat,
                ComponentInvocationKind::Repeat,
            ),
            (
                ComponentDefinitionKind::InlineConditional,
                ComponentInvocationKind::Conditional,
            ),
            (
                ComponentDefinitionKind::InlineTransform,
                ComponentInvocationKind::Transform,
            ),
        ];
        for (definition_kind, invocation_kind) in mappings {
            let definitions = self
                .definitions
                .iter()
                .filter(|definition| definition.kind == definition_kind)
                .filter(|definition| {
                    only_file.is_none_or(|file| definition.file.as_deref() == Some(file))
                })
                .cloned()
                .collect::<Vec<_>>();
            for definition in definitions {
                self.invocations.push(ComponentInvocation {
                    id: component_id("invocation", &["structural", definition.id.as_str()]),
                    kind: invocation_kind.clone(),
                    name: definition.display_name.clone(),
                    file: definition.file.clone().unwrap_or_default(),
                    source_node_id: definition.source_node_id.clone(),
                    owner_definition_id: definition.owner_definition_id.clone(),
                    parent_invocation_id: None,
                    target_reference: definition.name.clone(),
                    resolved_definition_ids: vec![definition.id],
                    fallback_references: Vec::new(),
                    range: definition.range.clone(),
                    call_range: definition.range,
                    body_range: definition.body_range,
                    arguments: Vec::new(),
                    context_dependencies: Vec::new(),
                    data_bindings: Vec::new(),
                    status: ComponentResolutionStatus::Resolved,
                    diagnostics: Vec::new(),
                });
            }
        }
    }

    fn reconcile_consumers(&mut self) {
        let consumers = self
            .invocations
            .iter()
            .flat_map(|invocation| {
                invocation
                    .resolved_definition_ids
                    .iter()
                    .map(move |definition| (definition.clone(), invocation.id.clone()))
            })
            .fold(
                HashMap::<String, Vec<String>>::new(),
                |mut result, (definition, invocation)| {
                    result.entry(definition).or_default().push(invocation);
                    result
                },
            );
        for definition in &mut self.definitions {
            definition.consumer_invocation_ids =
                consumers.get(&definition.id).cloned().unwrap_or_default();
            definition.consumer_invocation_ids.sort();
            definition.consumer_invocation_ids.dedup();
        }
        self.diagnostics.extend(
            self.definitions
                .iter()
                .flat_map(|definition| definition.diagnostics.iter().cloned()),
        );
        self.diagnostics.extend(
            self.invocations
                .iter()
                .flat_map(|invocation| invocation.diagnostics.iter().cloned()),
        );
        self.diagnostics.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.source_node_id.cmp(&right.source_node_id))
        });
        self.diagnostics.dedup_by(|left, right| {
            left.code == right.code
                && left.file == right.file
                && left.source_node_id == right.source_node_id
        });
    }

    fn sort_output(&mut self) {
        self.definitions.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| {
                    range_start(left.range.as_ref()).cmp(&range_start(right.range.as_ref()))
                })
                .then_with(|| left.id.cmp(&right.id))
        });
        self.invocations.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| {
                    range_start(left.range.as_ref()).cmp(&range_start(right.range.as_ref()))
                })
                .then_with(|| left.id.cmp(&right.id))
        });
    }
}

fn component_parameters(definition: &TeraComponentDefinition) -> Vec<ComponentParameter> {
    let mut parameters = definition
        .arguments
        .iter()
        .map(component_parameter)
        .collect::<Vec<_>>();
    if let Some(rest) = definition.rest_argument.as_ref() {
        parameters.push(ComponentParameter {
            name: rest.clone(),
            argument_type: Some("map".to_string()),
            required: false,
            rest: true,
            default_value: None,
            range: None,
        });
    }
    parameters
}

fn component_parameter(parameter: &TeraComponentParameter) -> ComponentParameter {
    ComponentParameter {
        name: parameter.name.clone(),
        argument_type: parameter.argument_type.clone(),
        required: parameter.required,
        rest: false,
        default_value: parameter.default_value.clone(),
        range: Some(tera_range_to_source_range(&parameter.range)),
    }
}

fn component_argument(argument: &TeraComponentArgument) -> ComponentArgument {
    ComponentArgument {
        name: argument.name.clone(),
        expression: argument.expression.clone(),
        spread: argument.spread,
        range: Some(tera_range_to_source_range(&argument.range)),
    }
}

fn expression_matches_type(expression: &TeraSemanticExpression, expected: &str) -> bool {
    matches!(
        (&expression.value, expected),
        (TeraSemanticValue::String(_), "string")
            | (TeraSemanticValue::Boolean(_), "bool")
            | (TeraSemanticValue::Integer(_), "integer" | "number")
            | (TeraSemanticValue::Float(_), "float" | "number")
            | (TeraSemanticValue::Array(_), "array")
            | (TeraSemanticValue::Map(_), "map")
            | (
                TeraSemanticValue::Identifier(_)
                    | TeraSemanticValue::FunctionCall(_)
                    | TeraSemanticValue::OptionalChain { .. }
                    | TeraSemanticValue::Ternary { .. }
                    | TeraSemanticValue::Raw(_),
                _,
            )
            | (_, "bytes")
    )
}

fn context_dependencies_for_call(call: &TeraComponentCall) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    for argument in &call.arguments {
        collect_identifiers(&argument.expression.value, &mut dependencies);
    }
    dependencies.into_iter().collect()
}

fn collect_identifiers(value: &TeraSemanticValue, output: &mut BTreeSet<String>) {
    match value {
        TeraSemanticValue::Identifier(identifier) => {
            output.insert(identifier.clone());
        }
        TeraSemanticValue::Math { left, right, .. }
        | TeraSemanticValue::Logic { left, right, .. }
        | TeraSemanticValue::In {
            needle: left,
            haystack: right,
            ..
        } => {
            collect_identifiers(&left.value, output);
            collect_identifiers(&right.value, output);
        }
        TeraSemanticValue::Test { arguments, .. } | TeraSemanticValue::Array(arguments) => {
            for argument in arguments {
                collect_identifiers(&argument.value, output);
            }
        }
        TeraSemanticValue::FunctionCall(call) => {
            for argument in call.arguments.values() {
                collect_identifiers(&argument.value, output);
            }
        }
        TeraSemanticValue::Map(values) => {
            for value in values.values() {
                collect_identifiers(&value.value, output);
            }
        }
        TeraSemanticValue::Spread(value) | TeraSemanticValue::OptionalChain { value, .. } => {
            collect_identifiers(&value.value, output);
        }
        TeraSemanticValue::Slice { value, start, end } => {
            collect_identifiers(&value.value, output);
            if let Some(start) = start {
                collect_identifiers(&start.value, output);
            }
            if let Some(end) = end {
                collect_identifiers(&end.value, output);
            }
        }
        TeraSemanticValue::Ternary {
            condition,
            truthy,
            falsy,
        } => {
            collect_identifiers(&condition.value, output);
            collect_identifiers(&truthy.value, output);
            collect_identifiers(&falsy.value, output);
        }
        TeraSemanticValue::Comprehension {
            value,
            binding,
            iterable,
            condition,
        } => {
            collect_identifiers(&value.value, output);
            collect_identifiers(&iterable.value, output);
            if let Some(condition) = condition {
                collect_identifiers(&condition.value, output);
            }
            output.retain(|identifier| identifier != binding);
        }
        TeraSemanticValue::StringConcat(values) => {
            for value in values {
                collect_identifiers(value, output);
            }
        }
        _ => {}
    }
}

fn source_node_for_range<'a>(
    graph: &'a SourceGraph,
    file: &str,
    kind: SourceNodeKind,
    range: &TeraSourceRange,
) -> Option<&'a SourceNode> {
    graph.nodes.iter().find(|node| {
        node.file == file
            && node.kind == kind
            && node
                .range
                .as_ref()
                .is_some_and(|node_range| node_range.start == range.start)
    })
}

fn dependencies_for_template(
    graph: &SourceGraph,
    template: &SourceGraphTemplate,
) -> Vec<ComponentDependency> {
    let mut dependencies = graph
        .relations
        .iter()
        .filter(|relation| relation.from == template.node_id)
        .filter_map(|relation| {
            let kind = match relation.kind {
                SourceRelationKind::Extends | SourceRelationKind::Includes => {
                    ComponentDependencyKind::Template
                }
                SourceRelationKind::GetsPage | SourceRelationKind::GetsSection => {
                    ComponentDependencyKind::Content
                }
                SourceRelationKind::UsesStyle => ComponentDependencyKind::Style,
                SourceRelationKind::UsesScript => ComponentDependencyKind::Script,
                SourceRelationKind::AssetReference
                | SourceRelationKind::AssetHash
                | SourceRelationKind::AssetUrl => ComponentDependencyKind::Asset,
                SourceRelationKind::DataLoad
                | SourceRelationKind::DataFileLoad
                | SourceRelationKind::ContentDataLoad => ComponentDependencyKind::Data,
                _ => return None,
            };
            Some(ComponentDependency {
                kind,
                reference: relation.label.clone(),
                source_node_id: Some(relation.from.clone()),
                target_node_id: Some(relation.to.clone()),
                resolved: true,
            })
        })
        .collect::<Vec<_>>();
    if let Some(semantics) = template.semantics.as_ref() {
        dependencies.extend(
            collect_zola_runtime_uses(semantics)
                .into_iter()
                .map(|runtime| ComponentDependency {
                    kind: ComponentDependencyKind::Runtime,
                    reference: runtime_dependency_reference(runtime),
                    source_node_id: Some(template.node_id.clone()),
                    target_node_id: None,
                    resolved: true,
                }),
        );
    }
    dependencies.sort_by(|left, right| {
        format!("{:?}:{}", left.kind, left.reference)
            .cmp(&format!("{:?}:{}", right.kind, right.reference))
    });
    dependencies
        .dedup_by(|left, right| left.kind == right.kind && left.reference == right.reference);
    dependencies
}

fn runtime_dependency_reference(
    runtime: crate::source_graph::zola::ZolaTeraRuntimeDescriptor,
) -> String {
    let mut arguments = runtime
        .required_arguments
        .iter()
        .map(|argument| (*argument).to_string())
        .collect::<Vec<_>>();
    arguments.extend(
        runtime
            .optional_arguments
            .iter()
            .map(|argument| format!("{argument}?")),
    );
    arguments.extend(
        runtime
            .deprecated_arguments
            .iter()
            .map(|(argument, replacement)| format!("{argument}->{replacement}")),
    );
    format!(
        "{:?}:{}({})@{:?}",
        runtime.kind,
        runtime.name,
        arguments.join(","),
        runtime.availability
    )
}

fn file_capabilities(editable: bool) -> ComponentCapabilities {
    if editable {
        ComponentCapabilities {
            can_create: true,
            can_edit: true,
            can_duplicate: true,
            can_move: true,
            can_rename: true,
            can_extract: true,
            can_delete: true,
            reason_diagnostic: None,
        }
    } else {
        ComponentCapabilities {
            can_create: false,
            can_edit: false,
            can_duplicate: true,
            can_move: false,
            can_rename: false,
            can_extract: false,
            can_delete: false,
            reason_diagnostic: Some(LocalizedDiagnostic::new(
                "components-capability-theme-definition-readonly",
            )),
        }
    }
}

fn symbol_capabilities(editable: bool) -> ComponentCapabilities {
    let mut capabilities = file_capabilities(editable);
    capabilities.can_create = false;
    // A Tera 2 component is a symbol inside a template, not the template file
    // itself. File duplication/move/extract would create duplicate global
    // symbols or operate at the wrong semantic level.
    capabilities.can_duplicate = false;
    capabilities.can_move = false;
    capabilities.can_extract = false;
    if !editable {
        capabilities.reason_diagnostic = Some(LocalizedDiagnostic::new(
            "components-capability-theme-symbol-readonly",
        ));
    }
    capabilities
}

fn component_origin(origin: &SourceOrigin) -> ComponentOrigin {
    match origin {
        SourceOrigin::Local => ComponentOrigin::Project,
        SourceOrigin::Theme => ComponentOrigin::Theme,
    }
}

fn display_name_for_template(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}

fn tera_range_to_source_range(range: &TeraSourceRange) -> SourceRange {
    SourceRange {
        start: range.start,
        end: range.end,
        line: range.line,
        column: range.column,
        end_line: range.end_line,
        end_column: range.end_column,
    }
}

fn component_diagnostic(
    code: &str,
    diagnostic: LocalizedDiagnostic,
    severity: SourceDiagnosticSeverity,
    file: Option<String>,
    source_node_id: Option<String>,
) -> ComponentDiagnostic {
    ComponentDiagnostic {
        code: code.to_string(),
        diagnostic,
        severity,
        file,
        source_node_id,
    }
}

fn component_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prefix.hash(&mut hasher);
    for part in parts {
        part.hash(&mut hasher);
    }
    format!("component:{prefix}:{:016x}", hasher.finish())
}

fn range_start(range: Option<&SourceRange>) -> usize {
    range.map(|range| range.start).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_type_validation_is_strict_but_dynamic_values_remain_deferred_to_tera() {
        assert!(expression_matches_type(
            &TeraSemanticExpression::parse("3"),
            "integer"
        ));
        assert!(!expression_matches_type(
            &TeraSemanticExpression::parse("3"),
            "string"
        ));
        assert!(expression_matches_type(
            &TeraSemanticExpression::parse("page.extra.value"),
            "string"
        ));
    }
}
