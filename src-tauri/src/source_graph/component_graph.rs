use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{Hash, Hasher},
};

use crate::source_graph::{
    model::{
        ComponentArgument, ComponentCapabilities, ComponentDataBinding, ComponentDefinition,
        ComponentDefinitionKind, ComponentDependency, ComponentDependencyKind, ComponentDiagnostic,
        ComponentGraph, ComponentInvocation, ComponentInvocationKind, ComponentOrigin,
        ComponentParameter, ComponentResolutionStatus, SourceDiagnosticSeverity, SourceGraph,
        SourceGraphTemplate, SourceNode, SourceNodeKind, SourceOrigin, SourceRelationKind,
    },
    tera_semantics::{
        TeraSemanticCall, TeraSemanticDocument, TeraSemanticExpression, TeraSemanticNode,
        TeraSemanticValue,
    },
    zola::{
        collect_zola_runtime_uses, normalize_zola_template_reference, ZolaTeraRuntimeAvailability,
        ZolaTeraRuntimeKind, PINNED_ZOLA_REVISION,
    },
    zola_shortcode::{ZolaShortcodeInvocation, ZolaShortcodeValue},
};

pub(crate) const COMPONENT_GRAPH_SCHEMA_VERSION: u32 = 1;

pub(crate) fn build_component_graph(source_graph: &SourceGraph) -> ComponentGraph {
    let mut builder = ComponentGraphBuilder::new(source_graph);
    builder.project_template_file_definitions();
    builder.reconcile_template_shadowing();
    builder.project_template_symbol_definitions();
    builder.project_include_invocations();
    builder.project_macro_invocations();
    builder.project_shortcode_invocations();
    builder.project_repeat_invocations();
    builder.reconcile_consumers();
    builder.finish()
}

struct ComponentGraphBuilder<'a> {
    source_graph: &'a SourceGraph,
    definitions: Vec<ComponentDefinition>,
    invocations: Vec<ComponentInvocation>,
    diagnostics: Vec<ComponentDiagnostic>,
    file_definition_by_template_node: HashMap<String, String>,
    effective_file_definition_by_template_name: HashMap<String, String>,
    macro_definition_by_template_and_name: HashMap<(String, String), String>,
    shortcode_definition_by_name: HashMap<String, String>,
    repeat_definition_by_source_node: HashMap<String, String>,
}

impl<'a> ComponentGraphBuilder<'a> {
    fn new(source_graph: &'a SourceGraph) -> Self {
        Self {
            source_graph,
            definitions: Vec::new(),
            invocations: Vec::new(),
            diagnostics: Vec::new(),
            file_definition_by_template_node: HashMap::new(),
            effective_file_definition_by_template_name: HashMap::new(),
            macro_definition_by_template_and_name: HashMap::new(),
            shortcode_definition_by_name: HashMap::new(),
            repeat_definition_by_source_node: HashMap::new(),
        }
    }

    fn project_template_file_definitions(&mut self) {
        for template in &self.source_graph.templates {
            let kind = template_definition_kind(&template.name);
            let id = component_id("definition", &["template-file", template.node_id.as_str()]);
            let origin = component_origin(&template.origin);
            let editable = matches!(template.origin, SourceOrigin::Local);
            let dependencies = dependencies_for_template(self.source_graph, template);
            let diagnostics = zola_runtime_diagnostics_for_template(template, &kind);
            let context_dependencies = template
                .semantics
                .as_ref()
                .map(context_dependencies_for_document)
                .unwrap_or_default();
            let definition = ComponentDefinition {
                id: id.clone(),
                kind,
                name: template.name.clone(),
                display_name: display_name_for_template(&template.name),
                origin,
                theme_name: template.theme_name.clone(),
                file: Some(template.file.clone()),
                template_name: Some(template.name.clone()),
                source_node_id: Some(template.node_id.clone()),
                owner_definition_id: None,
                symbol: None,
                parameters: Vec::new(),
                context_dependencies,
                data_bindings: data_bindings_for_document(
                    template.semantics.as_ref(),
                    Some(template.node_id.as_str()),
                ),
                dependencies,
                consumer_invocation_ids: Vec::new(),
                shadowed_by: None,
                active: true,
                capabilities: file_capabilities(editable),
                diagnostics,
            };
            self.file_definition_by_template_node
                .insert(template.node_id.clone(), id);
            self.definitions.push(definition);
        }
    }

    fn reconcile_template_shadowing(&mut self) {
        let mut by_name = BTreeMap::<String, Vec<usize>>::new();
        for (index, definition) in self.definitions.iter().enumerate() {
            if definition.template_name.is_none() || definition.owner_definition_id.is_some() {
                continue;
            }
            let name = normalize_zola_template_reference(
                definition.template_name.as_deref().unwrap_or_default(),
            );
            by_name.entry(name).or_default().push(index);
        }

        for (name, indexes) in by_name {
            let active_index = indexes
                .iter()
                .copied()
                .find(|index| self.definitions[*index].origin == ComponentOrigin::Project)
                .or_else(|| indexes.first().copied());
            let Some(active_index) = active_index else {
                continue;
            };
            let active_id = self.definitions[active_index].id.clone();
            self.effective_file_definition_by_template_name
                .insert(name, active_id.clone());
            for index in indexes {
                let active = index == active_index;
                self.definitions[index].active = active;
                self.definitions[index].shadowed_by = (!active).then(|| active_id.clone());
            }
        }
        self.shortcode_definition_by_name = self
            .definitions
            .iter()
            .filter(|definition| {
                definition.active && definition.kind == ComponentDefinitionKind::Shortcode
            })
            .filter_map(|definition| {
                shortcode_name_from_template(
                    definition.template_name.as_deref().unwrap_or_default(),
                )
                .map(|name| (name, definition.id.clone()))
            })
            .collect();
    }

    fn project_template_symbol_definitions(&mut self) {
        for template in &self.source_graph.templates {
            let owner_definition_id = self
                .file_definition_by_template_node
                .get(&template.node_id)
                .cloned();
            let editable = matches!(template.origin, SourceOrigin::Local);
            let owner_active = owner_definition_id
                .as_deref()
                .and_then(|id| self.definition(id))
                .map(|definition| definition.active)
                .unwrap_or(true);
            let owner_data_bindings = owner_definition_id
                .as_deref()
                .and_then(|id| self.definition(id))
                .map(|definition| definition.data_bindings.clone())
                .unwrap_or_default();
            let base_dependencies = dependencies_for_template(self.source_graph, template);

            let macro_nodes =
                source_nodes_for_template(self.source_graph, template, SourceNodeKind::Macro);
            let mut macro_node_occurrences = HashMap::<String, usize>::new();
            if let Some(semantics) = template.semantics.as_ref() {
                for node in semantics.walk() {
                    let TeraSemanticNode::MacroDefinition {
                        name,
                        arguments,
                        body,
                    } = node
                    else {
                        continue;
                    };
                    let occurrence = macro_node_occurrences.entry(name.clone()).or_default();
                    let source_node = macro_nodes
                        .iter()
                        .filter(|node| node.label == *name)
                        .nth(*occurrence)
                        .copied();
                    *occurrence += 1;
                    let source_node_id = source_node.map(|node| node.id.clone());
                    let id = component_id(
                        "definition",
                        &[
                            "macro",
                            template.node_id.as_str(),
                            name.as_str(),
                            source_node_id.as_deref().unwrap_or("semantic-only"),
                        ],
                    );
                    let parameter_names = arguments.keys().cloned().collect::<BTreeSet<_>>();
                    let mut context_dependencies = context_dependencies_for_nodes(body);
                    context_dependencies.retain(|dependency| {
                        !parameter_names.contains(root_identifier(dependency))
                    });
                    let parameters = arguments
                        .iter()
                        .map(|(name, default_value)| ComponentParameter {
                            name: name.clone(),
                            required: default_value.is_none(),
                            default_value: default_value.clone(),
                        })
                        .collect();
                    let definition = ComponentDefinition {
                        id: id.clone(),
                        kind: ComponentDefinitionKind::Macro,
                        name: format!("{}::{name}", template.name),
                        display_name: name.clone(),
                        origin: component_origin(&template.origin),
                        theme_name: template.theme_name.clone(),
                        file: Some(template.file.clone()),
                        template_name: Some(template.name.clone()),
                        source_node_id,
                        owner_definition_id: owner_definition_id.clone(),
                        symbol: Some(name.clone()),
                        parameters,
                        context_dependencies,
                        data_bindings: data_bindings_for_nodes(
                            body,
                            source_node.map(|node| node.id.as_str()),
                        ),
                        dependencies: base_dependencies.clone(),
                        consumer_invocation_ids: Vec::new(),
                        shadowed_by: None,
                        active: owner_active,
                        capabilities: symbol_capabilities(editable),
                        diagnostics: Vec::new(),
                    };
                    self.macro_definition_by_template_and_name.insert(
                        (
                            normalize_zola_template_reference(&template.name),
                            name.clone(),
                        ),
                        id,
                    );
                    self.definitions.push(definition);
                }
            }

            for block_node in
                source_nodes_for_template(self.source_graph, template, SourceNodeKind::Block)
            {
                let id = component_id(
                    "definition",
                    &["block", template.node_id.as_str(), block_node.id.as_str()],
                );
                self.definitions.push(ComponentDefinition {
                    id,
                    kind: ComponentDefinitionKind::TemplateBlock,
                    name: format!("{}#{}", template.name, block_node.label),
                    display_name: block_node.label.clone(),
                    origin: component_origin(&template.origin),
                    theme_name: template.theme_name.clone(),
                    file: Some(template.file.clone()),
                    template_name: Some(template.name.clone()),
                    source_node_id: Some(block_node.id.clone()),
                    owner_definition_id: owner_definition_id.clone(),
                    symbol: Some(block_node.label.clone()),
                    parameters: Vec::new(),
                    context_dependencies: Vec::new(),
                    data_bindings: Vec::new(),
                    dependencies: base_dependencies.clone(),
                    consumer_invocation_ids: Vec::new(),
                    shadowed_by: None,
                    active: owner_active,
                    capabilities: symbol_capabilities(editable),
                    diagnostics: Vec::new(),
                });
            }

            let for_nodes =
                source_nodes_for_template(self.source_graph, template, SourceNodeKind::For);
            let semantic_loops = template
                .semantics
                .as_ref()
                .map(semantic_loops)
                .unwrap_or_default();
            for (index, source_node) in for_nodes.into_iter().enumerate() {
                let semantic = semantic_loops.get(index).copied();
                let (name, context_dependencies, data_bindings) = match semantic {
                    Some(TeraSemanticNode::For {
                        key,
                        value,
                        container,
                        body,
                        empty_body,
                    }) => {
                        let mut dependencies = context_dependencies_for_expression(container);
                        dependencies.extend(context_dependencies_for_nodes(body));
                        if let Some(empty_body) = empty_body {
                            dependencies.extend(context_dependencies_for_nodes(empty_body));
                        }
                        dependencies.sort();
                        dependencies.dedup();
                        dependencies.retain(|dependency| {
                            root_identifier(dependency) != value
                                && key
                                    .as_deref()
                                    .is_none_or(|key| root_identifier(dependency) != key)
                        });
                        let inherited_binding = owner_data_bindings
                            .iter()
                            .find(|binding| binding.name == *value);
                        let collection_path = expression_path(container);
                        let collection_producer =
                            producer_for_expression(container, &HashMap::new());
                        let mut bindings = vec![ComponentDataBinding {
                            name: value.clone(),
                            path: inherited_binding
                                .map(|binding| binding.path.clone())
                                .unwrap_or_else(|| format!("{collection_path}[]")),
                            producer: inherited_binding
                                .map(|binding| binding.producer.clone())
                                .unwrap_or_else(|| format!("{collection_producer}[]")),
                            source_node_id: Some(source_node.id.clone()),
                        }];
                        if let Some(key) = key {
                            bindings.push(ComponentDataBinding {
                                name: key.clone(),
                                path: format!("{collection_path}.__key"),
                                producer: format!("{collection_producer}.__key"),
                                source_node_id: Some(source_node.id.clone()),
                            });
                        }
                        (format!("Listă dinamică · {value}"), dependencies, bindings)
                    }
                    _ => ("Listă dinamică".to_string(), Vec::new(), Vec::new()),
                };
                let id = component_id(
                    "definition",
                    &["repeat", template.node_id.as_str(), source_node.id.as_str()],
                );
                self.repeat_definition_by_source_node
                    .insert(source_node.id.clone(), id.clone());
                self.definitions.push(ComponentDefinition {
                    id,
                    kind: ComponentDefinitionKind::InlineRepeat,
                    name: format!("{}#{}", template.name, source_node.id),
                    display_name: name,
                    origin: component_origin(&template.origin),
                    theme_name: template.theme_name.clone(),
                    file: Some(template.file.clone()),
                    template_name: Some(template.name.clone()),
                    source_node_id: Some(source_node.id.clone()),
                    owner_definition_id: owner_definition_id.clone(),
                    symbol: None,
                    parameters: Vec::new(),
                    context_dependencies,
                    data_bindings,
                    dependencies: base_dependencies.clone(),
                    consumer_invocation_ids: Vec::new(),
                    shadowed_by: None,
                    active: owner_active,
                    capabilities: symbol_capabilities(editable),
                    diagnostics: Vec::new(),
                });
            }
        }

        self.reconcile_symbol_shadowing();
    }

    fn reconcile_symbol_shadowing(&mut self) {
        let mut active_macros = HashMap::<(String, String), String>::new();
        for definition in &self.definitions {
            if definition.kind != ComponentDefinitionKind::Macro || !definition.active {
                continue;
            }
            let Some(template_name) = definition.template_name.as_ref() else {
                continue;
            };
            let Some(symbol) = definition.symbol.as_ref() else {
                continue;
            };
            active_macros.insert(
                (
                    normalize_zola_template_reference(template_name),
                    symbol.clone(),
                ),
                definition.id.clone(),
            );
        }
        for definition in &mut self.definitions {
            if definition.kind != ComponentDefinitionKind::Macro || definition.active {
                continue;
            }
            let Some(template_name) = definition.template_name.as_ref() else {
                continue;
            };
            let Some(symbol) = definition.symbol.as_ref() else {
                continue;
            };
            definition.shadowed_by = active_macros
                .get(&(
                    normalize_zola_template_reference(template_name),
                    symbol.clone(),
                ))
                .cloned();
        }
        self.macro_definition_by_template_and_name = active_macros;
    }

    fn project_include_invocations(&mut self) {
        for template in &self.source_graph.templates {
            let include_nodes =
                source_nodes_for_template(self.source_graph, template, SourceNodeKind::Include);
            for (index, group) in template.include_groups.iter().enumerate() {
                let source_node = include_nodes.get(index).copied();
                let mut resolved = Vec::new();
                let mut selected_index = None;
                for (fallback_index, target) in group.targets.iter().enumerate() {
                    let normalized = normalize_zola_template_reference(target);
                    if let Some(definition_id) = self
                        .effective_file_definition_by_template_name
                        .get(&normalized)
                    {
                        resolved.push(definition_id.clone());
                        if selected_index.is_none() {
                            selected_index = Some(fallback_index);
                        }
                    }
                }
                let status = match selected_index {
                    Some(0) => ComponentResolutionStatus::Resolved,
                    Some(_) => ComponentResolutionStatus::FallbackResolved,
                    None if group.ignore_missing => ComponentResolutionStatus::External,
                    None => ComponentResolutionStatus::Unresolved,
                };
                let mut invocation_diagnostics = Vec::new();
                if status == ComponentResolutionStatus::Unresolved {
                    invocation_diagnostics.push(component_diagnostic(
                        "unresolved_include",
                        format!(
                            "Niciun target al include-ului nu a fost rezolvat: {}.",
                            group.targets.join(", ")
                        ),
                        SourceDiagnosticSeverity::Error,
                        Some(template.file.clone()),
                        source_node.map(|node| node.id.clone()),
                    ));
                }
                let target_reference = group.targets.first().cloned().unwrap_or_default();
                let id = component_id(
                    "invocation",
                    &[
                        "include",
                        template.node_id.as_str(),
                        source_node
                            .map(|node| node.id.as_str())
                            .unwrap_or(target_reference.as_str()),
                    ],
                );
                self.invocations.push(ComponentInvocation {
                    id,
                    kind: ComponentInvocationKind::Include,
                    name: format!("Include {}", target_reference),
                    file: template.file.clone(),
                    source_node_id: source_node.map(|node| node.id.clone()),
                    owner_definition_id: self
                        .file_definition_by_template_node
                        .get(&template.node_id)
                        .cloned(),
                    parent_invocation_id: None,
                    target_reference,
                    resolved_definition_ids: resolved,
                    fallback_references: group.targets.iter().skip(1).cloned().collect(),
                    arguments: Vec::new(),
                    context_dependencies: template
                        .semantics
                        .as_ref()
                        .map(context_dependencies_for_document)
                        .unwrap_or_default(),
                    data_bindings: Vec::new(),
                    status,
                    diagnostics: invocation_diagnostics,
                });
            }
        }
    }

    fn project_macro_invocations(&mut self) {
        for template in &self.source_graph.templates {
            let Some(semantics) = template.semantics.as_ref() else {
                continue;
            };
            let import_bindings = import_bindings(semantics);
            let call_uses = semantic_macro_call_uses(self.source_graph, template, semantics);
            for (index, call_use) in call_uses.into_iter().enumerate() {
                let namespace = call_use.call.namespace.clone().unwrap_or_default();
                let target_template = if namespace == "self" {
                    Some(template.name.clone())
                } else {
                    import_bindings.get(&namespace).cloned()
                };
                let resolved_definition_id = target_template.as_ref().and_then(|target| {
                    self.macro_definition_by_template_and_name
                        .get(&(
                            normalize_zola_template_reference(target),
                            call_use.call.name.clone(),
                        ))
                        .cloned()
                });
                let status = if resolved_definition_id.is_some() {
                    ComponentResolutionStatus::Resolved
                } else if target_template.is_none() {
                    ComponentResolutionStatus::Unresolved
                } else {
                    ComponentResolutionStatus::Unresolved
                };
                let target_reference = format!("{namespace}::{}", call_use.call.name);
                let mut invocation_diagnostics = Vec::new();
                if status == ComponentResolutionStatus::Unresolved {
                    invocation_diagnostics.push(component_diagnostic(
                        "unresolved_macro_call",
                        format!(
                            "Apelul macro {target_reference} nu are o definiție activă rezolvată."
                        ),
                        SourceDiagnosticSeverity::Error,
                        Some(template.file.clone()),
                        call_use.source_node_id.clone(),
                    ));
                }
                let context_dependencies = context_dependencies_for_call(&call_use.call);
                let data_bindings = call_use
                    .call
                    .arguments
                    .iter()
                    .map(|(name, expression)| ComponentDataBinding {
                        name: name.clone(),
                        path: expression_path(expression),
                        producer: producer_for_expression(expression, &HashMap::new()),
                        source_node_id: call_use.source_node_id.clone(),
                    })
                    .collect();
                let arguments = call_use
                    .call
                    .arguments
                    .into_iter()
                    .map(|(name, expression)| ComponentArgument { name, expression })
                    .collect();
                let id = component_id(
                    "invocation",
                    &[
                        "macro",
                        template.node_id.as_str(),
                        call_use
                            .source_node_id
                            .as_deref()
                            .unwrap_or(target_reference.as_str()),
                        index.to_string().as_str(),
                    ],
                );
                self.invocations.push(ComponentInvocation {
                    id,
                    kind: ComponentInvocationKind::MacroCall,
                    name: format!("Macro {target_reference}"),
                    file: template.file.clone(),
                    source_node_id: call_use.source_node_id,
                    owner_definition_id: self
                        .file_definition_by_template_node
                        .get(&template.node_id)
                        .cloned(),
                    parent_invocation_id: None,
                    target_reference,
                    resolved_definition_ids: resolved_definition_id.into_iter().collect(),
                    fallback_references: Vec::new(),
                    arguments,
                    context_dependencies,
                    data_bindings,
                    status,
                    diagnostics: invocation_diagnostics,
                });
            }
        }
    }

    fn project_shortcode_invocations(&mut self) {
        let mut projected = Vec::new();
        for page in &self.source_graph.pages {
            project_shortcode_tree(
                &page.file,
                &page.shortcodes,
                None,
                &self.shortcode_definition_by_name,
                &mut projected,
            );
        }
        self.invocations.extend(projected);
    }

    fn project_repeat_invocations(&mut self) {
        for template in &self.source_graph.templates {
            for source_node in
                source_nodes_for_template(self.source_graph, template, SourceNodeKind::For)
            {
                let Some(definition_id) = self
                    .repeat_definition_by_source_node
                    .get(&source_node.id)
                    .cloned()
                else {
                    continue;
                };
                let definition = self.definition(&definition_id);
                let id = component_id(
                    "invocation",
                    &["repeat", template.node_id.as_str(), source_node.id.as_str()],
                );
                self.invocations.push(ComponentInvocation {
                    id,
                    kind: ComponentInvocationKind::Repeat,
                    name: definition
                        .map(|definition| definition.display_name.clone())
                        .unwrap_or_else(|| "Listă dinamică".to_string()),
                    file: template.file.clone(),
                    source_node_id: Some(source_node.id.clone()),
                    owner_definition_id: self
                        .file_definition_by_template_node
                        .get(&template.node_id)
                        .cloned(),
                    parent_invocation_id: None,
                    target_reference: source_node.label.clone(),
                    resolved_definition_ids: vec![definition_id],
                    fallback_references: Vec::new(),
                    arguments: Vec::new(),
                    context_dependencies: definition
                        .map(|definition| definition.context_dependencies.clone())
                        .unwrap_or_default(),
                    data_bindings: definition
                        .map(|definition| definition.data_bindings.clone())
                        .unwrap_or_default(),
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
                    .map(move |definition_id| (definition_id.clone(), invocation.id.clone()))
            })
            .collect::<Vec<_>>();
        for (definition_id, invocation_id) in consumers {
            if let Some(definition) = self
                .definitions
                .iter_mut()
                .find(|definition| definition.id == definition_id)
            {
                if !definition.consumer_invocation_ids.contains(&invocation_id) {
                    definition.consumer_invocation_ids.push(invocation_id);
                }
            }
        }
        self.reconcile_shortcode_parameters();

        for invocation in &self.invocations {
            self.diagnostics.extend(invocation.diagnostics.clone());
        }
        for definition in &self.definitions {
            self.diagnostics.extend(definition.diagnostics.clone());
        }
        self.diagnostics.sort_by(|left, right| {
            (
                left.file.as_deref().unwrap_or_default(),
                left.source_node_id.as_deref().unwrap_or_default(),
                left.code.as_str(),
            )
                .cmp(&(
                    right.file.as_deref().unwrap_or_default(),
                    right.source_node_id.as_deref().unwrap_or_default(),
                    right.code.as_str(),
                ))
        });
        self.diagnostics.dedup_by(|left, right| {
            left.code == right.code
                && left.file == right.file
                && left.source_node_id == right.source_node_id
                && left.message == right.message
        });
    }

    fn reconcile_shortcode_parameters(&mut self) {
        let arguments_by_definition = self
            .invocations
            .iter()
            .filter(|invocation| invocation.kind == ComponentInvocationKind::Shortcode)
            .flat_map(|invocation| {
                invocation
                    .resolved_definition_ids
                    .iter()
                    .flat_map(move |definition_id| {
                        invocation
                            .arguments
                            .iter()
                            .map(move |argument| (definition_id.clone(), argument.name.clone()))
                    })
            })
            .fold(
                HashMap::<String, BTreeSet<String>>::new(),
                |mut grouped, (definition_id, argument)| {
                    grouped.entry(definition_id).or_default().insert(argument);
                    grouped
                },
            );
        for definition in &mut self.definitions {
            if definition.kind != ComponentDefinitionKind::Shortcode {
                continue;
            }
            let mut names = arguments_by_definition
                .get(&definition.id)
                .cloned()
                .unwrap_or_default();
            for dependency in &definition.context_dependencies {
                let root = root_identifier(dependency);
                if !is_builtin_shortcode_context(root) {
                    names.insert(root.to_string());
                }
            }
            definition.parameters = names
                .into_iter()
                .map(|name| ComponentParameter {
                    name,
                    required: false,
                    default_value: None,
                })
                .collect();
        }
    }

    fn definition(&self, id: &str) -> Option<&ComponentDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.id == id)
    }

    fn finish(mut self) -> ComponentGraph {
        self.definitions.sort_by(|left, right| {
            (
                !left.active,
                component_origin_order(&left.origin),
                left.name.as_str(),
                left.id.as_str(),
            )
                .cmp(&(
                    !right.active,
                    component_origin_order(&right.origin),
                    right.name.as_str(),
                    right.id.as_str(),
                ))
        });
        self.invocations.sort_by(|left, right| {
            (
                left.file.as_str(),
                left.source_node_id.as_deref().unwrap_or_default(),
                left.id.as_str(),
            )
                .cmp(&(
                    right.file.as_str(),
                    right.source_node_id.as_deref().unwrap_or_default(),
                    right.id.as_str(),
                ))
        });
        ComponentGraph {
            schema_version: COMPONENT_GRAPH_SCHEMA_VERSION,
            definitions: self.definitions,
            invocations: self.invocations,
            rendered_instances: Vec::new(),
            diagnostics: self.diagnostics,
        }
    }
}

fn template_definition_kind(name: &str) -> ComponentDefinitionKind {
    let name = normalize_zola_template_reference(name);
    if name.starts_with("shortcodes/") {
        ComponentDefinitionKind::Shortcode
    } else if name.starts_with("macros/") {
        ComponentDefinitionKind::MacroLibrary
    } else if name.starts_with("partials/") {
        ComponentDefinitionKind::Partial
    } else {
        ComponentDefinitionKind::TemplateFile
    }
}

fn shortcode_name_from_template(template_name: &str) -> Option<String> {
    let normalized = normalize_zola_template_reference(template_name);
    let relative = normalized.strip_prefix("shortcodes/")?;
    relative
        .strip_suffix(".html")
        .or_else(|| relative.strip_suffix(".md"))
        .map(str::to_string)
        .filter(|name| !name.is_empty())
}

fn project_shortcode_tree(
    file: &str,
    shortcodes: &[ZolaShortcodeInvocation],
    parent_invocation_id: Option<String>,
    definitions: &HashMap<String, String>,
    output: &mut Vec<ComponentInvocation>,
) {
    for shortcode in shortcodes {
        let resolved = definitions.get(&shortcode.name).cloned();
        let status = if resolved.is_some() {
            ComponentResolutionStatus::Resolved
        } else {
            ComponentResolutionStatus::Unresolved
        };
        let mut diagnostics = Vec::new();
        if resolved.is_none() {
            diagnostics.push(component_diagnostic(
                "unresolved_shortcode",
                format!(
                    "Shortcode-ul {} nu are templates/shortcodes/{}.html sau .md activ.",
                    shortcode.name, shortcode.name
                ),
                SourceDiagnosticSeverity::Error,
                Some(file.to_string()),
                shortcode.source_node_id.clone(),
            ));
        }
        let id = component_id(
            "invocation",
            &[
                "shortcode",
                file,
                shortcode
                    .source_node_id
                    .as_deref()
                    .unwrap_or(shortcode.name.as_str()),
                shortcode.nth.to_string().as_str(),
            ],
        );
        let arguments = shortcode
            .arguments
            .iter()
            .map(|(name, value)| ComponentArgument {
                name: name.clone(),
                expression: shortcode_value_expression(value),
            })
            .collect::<Vec<_>>();
        let mut data_bindings = arguments
            .iter()
            .map(|argument| ComponentDataBinding {
                name: argument.name.clone(),
                path: expression_path(&argument.expression),
                producer: "shortcode_literal".to_string(),
                source_node_id: shortcode.source_node_id.clone(),
            })
            .collect::<Vec<_>>();
        if let Some(body) = shortcode.body_range.as_ref() {
            data_bindings.push(ComponentDataBinding {
                name: "body".to_string(),
                path: format!("{}:{}..{}", file, body.start, body.end),
                producer: "markdown_shortcode_body".to_string(),
                source_node_id: shortcode.source_node_id.clone(),
            });
        }
        output.push(ComponentInvocation {
            id: id.clone(),
            kind: ComponentInvocationKind::Shortcode,
            name: format!("Shortcode {}", shortcode.name),
            file: file.to_string(),
            source_node_id: shortcode.source_node_id.clone(),
            owner_definition_id: None,
            parent_invocation_id: parent_invocation_id.clone(),
            target_reference: shortcode.name.clone(),
            resolved_definition_ids: resolved.into_iter().collect(),
            fallback_references: Vec::new(),
            arguments,
            context_dependencies: Vec::new(),
            data_bindings,
            status,
            diagnostics,
        });
        project_shortcode_tree(file, &shortcode.inner, Some(id), definitions, output);
    }
}

fn shortcode_value_expression(value: &ZolaShortcodeValue) -> TeraSemanticExpression {
    let value = match value {
        ZolaShortcodeValue::String(value) => TeraSemanticValue::String(value.clone()),
        ZolaShortcodeValue::Integer(value) => TeraSemanticValue::Integer(*value),
        ZolaShortcodeValue::Float(value) => TeraSemanticValue::Float(*value),
        ZolaShortcodeValue::Boolean(value) => TeraSemanticValue::Boolean(*value),
        ZolaShortcodeValue::Array(values) => {
            TeraSemanticValue::Array(values.iter().map(shortcode_value_expression).collect())
        }
    };
    TeraSemanticExpression {
        value,
        negated: false,
        filters: Vec::new(),
    }
}

fn is_builtin_shortcode_context(name: &str) -> bool {
    matches!(
        name,
        "body" | "nth" | "config" | "page" | "section" | "lang" | "current_url"
    )
}

fn display_name_for_template(name: &str) -> String {
    name.rsplit('/')
        .next()
        .unwrap_or(name)
        .trim_end_matches(".html")
        .replace(['_', '-'], " ")
}

fn component_origin(origin: &SourceOrigin) -> ComponentOrigin {
    match origin {
        SourceOrigin::Local => ComponentOrigin::Project,
        SourceOrigin::Theme => ComponentOrigin::Theme,
    }
}

fn component_origin_order(origin: &ComponentOrigin) -> u8 {
    match origin {
        ComponentOrigin::Project => 0,
        ComponentOrigin::Theme => 1,
    }
}

fn file_capabilities(editable: bool) -> ComponentCapabilities {
    ComponentCapabilities {
        can_create: true,
        can_edit: editable,
        can_duplicate: true,
        can_move: editable,
        can_rename: editable,
        can_extract: false,
        can_delete: editable,
        reason: (!editable)
            .then(|| "Definiția provine din temă; creează un override local.".to_string()),
    }
}

fn symbol_capabilities(editable: bool) -> ComponentCapabilities {
    ComponentCapabilities {
        can_create: true,
        can_edit: editable,
        can_duplicate: editable,
        can_move: editable,
        can_rename: editable,
        can_extract: editable,
        can_delete: editable,
        reason: (!editable)
            .then(|| "Simbolul provine din temă; creează un override local.".to_string()),
    }
}

fn source_nodes_for_template<'a>(
    source_graph: &'a SourceGraph,
    template: &SourceGraphTemplate,
    kind: SourceNodeKind,
) -> Vec<&'a SourceNode> {
    let mut nodes = source_graph
        .nodes
        .iter()
        .filter(|node| node.file == template.file && node.kind == kind)
        .collect::<Vec<_>>();
    nodes.sort_by_key(|node| {
        (
            node.range
                .as_ref()
                .map(|range| range.start)
                .unwrap_or(usize::MAX),
            node.id.as_str(),
        )
    });
    nodes
}

fn dependencies_for_template(
    source_graph: &SourceGraph,
    template: &SourceGraphTemplate,
) -> Vec<ComponentDependency> {
    let mut dependencies = source_graph
        .relations
        .iter()
        .filter(|relation| relation.from == template.node_id)
        .filter_map(|relation| {
            let kind = match relation.kind {
                SourceRelationKind::PageTemplate
                | SourceRelationKind::SectionPageTemplate
                | SourceRelationKind::Extends
                | SourceRelationKind::Includes
                | SourceRelationKind::Imports
                | SourceRelationKind::DefinesBlock
                | SourceRelationKind::OverridesBlock => ComponentDependencyKind::Template,
                SourceRelationKind::DataLoad | SourceRelationKind::DataFileLoad => {
                    ComponentDependencyKind::Data
                }
                SourceRelationKind::ContentDataLoad
                | SourceRelationKind::GetsPage
                | SourceRelationKind::GetsSection
                | SourceRelationKind::InternalContentLink => ComponentDependencyKind::Content,
                SourceRelationKind::UsesStyle => ComponentDependencyKind::Style,
                SourceRelationKind::UsesScript => ComponentDependencyKind::Script,
                SourceRelationKind::AssetUrl
                | SourceRelationKind::AssetHash
                | SourceRelationKind::ImageMetadata
                | SourceRelationKind::ImageResize => ComponentDependencyKind::Asset,
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
    if let Some(document) = template.semantics.as_ref() {
        dependencies.extend(
            collect_zola_runtime_uses(document)
                .into_iter()
                .map(|runtime| ComponentDependency {
                    kind: ComponentDependencyKind::Runtime,
                    reference: format!(
                        "zola:{}:{}",
                        match runtime.kind {
                            ZolaTeraRuntimeKind::Function => "function",
                            ZolaTeraRuntimeKind::Filter => "filter",
                        },
                        runtime.name
                    ),
                    source_node_id: Some(template.node_id.clone()),
                    target_node_id: None,
                    resolved: true,
                }),
        );
    }
    dependencies.sort_by(|left, right| {
        (
            format!("{:?}", left.kind),
            left.reference.as_str(),
            left.target_node_id.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                format!("{:?}", right.kind),
                right.reference.as_str(),
                right.target_node_id.as_deref().unwrap_or_default(),
            ))
    });
    dependencies.dedup_by(|left, right| {
        left.kind == right.kind
            && left.reference == right.reference
            && left.target_node_id == right.target_node_id
    });
    dependencies
}

fn zola_runtime_diagnostics_for_template(
    template: &SourceGraphTemplate,
    kind: &ComponentDefinitionKind,
) -> Vec<ComponentDiagnostic> {
    if *kind != ComponentDefinitionKind::Shortcode {
        return Vec::new();
    }
    template
        .semantics
        .as_ref()
        .map(collect_zola_runtime_uses)
        .unwrap_or_default()
        .into_iter()
        .filter(|runtime| runtime.availability == ZolaTeraRuntimeAvailability::Late)
        .map(|runtime| {
            component_diagnostic(
                "zola_runtime_unavailable_in_shortcode",
                format!(
                    "`{}` este înregistrată de Zola numai după parsarea paginilor și secțiunilor; nu este disponibilă la randarea shortcode-urilor (revizia Zola {}).",
                    runtime.name, PINNED_ZOLA_REVISION
                ),
                SourceDiagnosticSeverity::Warning,
                Some(template.file.clone()),
                Some(template.node_id.clone()),
            )
        })
        .collect()
}

fn import_bindings(document: &TeraSemanticDocument) -> HashMap<String, String> {
    document
        .walk()
        .into_iter()
        .filter_map(|node| match node {
            TeraSemanticNode::Import {
                template,
                namespace,
            } => Some((namespace.clone(), template.clone())),
            _ => None,
        })
        .collect()
}

fn semantic_loops(document: &TeraSemanticDocument) -> Vec<&TeraSemanticNode> {
    document
        .walk()
        .into_iter()
        .filter(|node| matches!(node, TeraSemanticNode::For { .. }))
        .collect()
}

#[derive(Clone)]
struct SemanticMacroCallUse {
    call: TeraSemanticCall,
    source_node_id: Option<String>,
}

fn semantic_macro_call_uses(
    source_graph: &SourceGraph,
    template: &SourceGraphTemplate,
    document: &TeraSemanticDocument,
) -> Vec<SemanticMacroCallUse> {
    let mut cursor = SemanticSourceCursor::new(source_graph, template);
    let mut result = Vec::new();
    collect_macro_calls_from_nodes(&document.nodes, &mut cursor, &mut result);
    result
}

struct SemanticSourceCursor<'a> {
    nodes: HashMap<SourceNodeKind, Vec<&'a SourceNode>>,
    indexes: HashMap<SourceNodeKind, usize>,
}

impl<'a> SemanticSourceCursor<'a> {
    fn new(source_graph: &'a SourceGraph, template: &SourceGraphTemplate) -> Self {
        let mut nodes = HashMap::<SourceNodeKind, Vec<&SourceNode>>::new();
        for kind in [
            SourceNodeKind::TeraVariable,
            SourceNodeKind::Macro,
            SourceNodeKind::Set,
            SourceNodeKind::SetGlobal,
            SourceNodeKind::Filter,
            SourceNodeKind::Block,
            SourceNodeKind::For,
            SourceNodeKind::If,
        ] {
            nodes.insert(
                kind.clone(),
                source_nodes_for_template(source_graph, template, kind),
            );
        }
        Self {
            nodes,
            indexes: HashMap::new(),
        }
    }

    fn next(&mut self, kind: SourceNodeKind) -> Option<String> {
        let index = self.indexes.entry(kind.clone()).or_default();
        let node = self
            .nodes
            .get(&kind)
            .and_then(|nodes| nodes.get(*index))
            .map(|node| node.id.clone());
        *index += 1;
        node
    }
}

fn collect_macro_calls_from_nodes(
    nodes: &[TeraSemanticNode],
    cursor: &mut SemanticSourceCursor<'_>,
    result: &mut Vec<SemanticMacroCallUse>,
) {
    for node in nodes {
        match node {
            TeraSemanticNode::Variable { expression } => {
                let source_node_id = cursor.next(SourceNodeKind::TeraVariable);
                collect_macro_calls_from_expression(expression, source_node_id, result);
            }
            TeraSemanticNode::MacroDefinition {
                arguments, body, ..
            } => {
                let source_node_id = cursor.next(SourceNodeKind::Macro);
                for expression in arguments.values().flatten() {
                    collect_macro_calls_from_expression(expression, source_node_id.clone(), result);
                }
                collect_macro_calls_from_nodes(body, cursor, result);
            }
            TeraSemanticNode::Set { global, value, .. } => {
                let source_node_id = cursor.next(if *global {
                    SourceNodeKind::SetGlobal
                } else {
                    SourceNodeKind::Set
                });
                collect_macro_calls_from_expression(value, source_node_id, result);
            }
            TeraSemanticNode::FilterSection { filter, body } => {
                let source_node_id = cursor.next(SourceNodeKind::Filter);
                collect_macro_calls_from_call_arguments(filter, source_node_id, result);
                collect_macro_calls_from_nodes(body, cursor, result);
            }
            TeraSemanticNode::Block { body, .. } => {
                cursor.next(SourceNodeKind::Block);
                collect_macro_calls_from_nodes(body, cursor, result);
            }
            TeraSemanticNode::For {
                container,
                body,
                empty_body,
                ..
            } => {
                let source_node_id = cursor.next(SourceNodeKind::For);
                collect_macro_calls_from_expression(container, source_node_id, result);
                collect_macro_calls_from_nodes(body, cursor, result);
                if let Some(empty_body) = empty_body {
                    collect_macro_calls_from_nodes(empty_body, cursor, result);
                }
            }
            TeraSemanticNode::If {
                branches,
                otherwise,
            } => {
                let source_node_id = cursor.next(SourceNodeKind::If);
                for branch in branches {
                    collect_macro_calls_from_expression(
                        &branch.condition,
                        source_node_id.clone(),
                        result,
                    );
                    collect_macro_calls_from_nodes(&branch.body, cursor, result);
                }
                if let Some(otherwise) = otherwise {
                    collect_macro_calls_from_nodes(otherwise, cursor, result);
                }
            }
            _ => {}
        }
    }
}

fn collect_macro_calls_from_expression(
    expression: &TeraSemanticExpression,
    source_node_id: Option<String>,
    result: &mut Vec<SemanticMacroCallUse>,
) {
    for filter in &expression.filters {
        collect_macro_calls_from_call_arguments(filter, source_node_id.clone(), result);
    }
    match &expression.value {
        TeraSemanticValue::MacroCall(call) => {
            result.push(SemanticMacroCallUse {
                call: call.clone(),
                source_node_id: source_node_id.clone(),
            });
            collect_macro_calls_from_call_arguments(call, source_node_id, result);
        }
        TeraSemanticValue::FunctionCall(call) => {
            collect_macro_calls_from_call_arguments(call, source_node_id, result)
        }
        TeraSemanticValue::Math { left, right, .. }
        | TeraSemanticValue::Logic { left, right, .. } => {
            collect_macro_calls_from_expression(left, source_node_id.clone(), result);
            collect_macro_calls_from_expression(right, source_node_id, result);
        }
        TeraSemanticValue::Test { arguments, .. } | TeraSemanticValue::Array(arguments) => {
            for argument in arguments {
                collect_macro_calls_from_expression(argument, source_node_id.clone(), result);
            }
        }
        TeraSemanticValue::StringConcat(values) => {
            for value in values {
                collect_macro_calls_from_value(value, source_node_id.clone(), result);
            }
        }
        TeraSemanticValue::In {
            needle, haystack, ..
        } => {
            collect_macro_calls_from_expression(needle, source_node_id.clone(), result);
            collect_macro_calls_from_expression(haystack, source_node_id, result);
        }
        _ => {}
    }
}

fn collect_macro_calls_from_value(
    value: &TeraSemanticValue,
    source_node_id: Option<String>,
    result: &mut Vec<SemanticMacroCallUse>,
) {
    let expression = TeraSemanticExpression {
        value: value.clone(),
        negated: false,
        filters: Vec::new(),
    };
    collect_macro_calls_from_expression(&expression, source_node_id, result);
}

fn collect_macro_calls_from_call_arguments(
    call: &TeraSemanticCall,
    source_node_id: Option<String>,
    result: &mut Vec<SemanticMacroCallUse>,
) {
    for argument in call.arguments.values() {
        collect_macro_calls_from_expression(argument, source_node_id.clone(), result);
    }
}

fn context_dependencies_for_document(document: &TeraSemanticDocument) -> Vec<String> {
    context_dependencies_for_nodes(&document.nodes)
}

fn context_dependencies_for_nodes(nodes: &[TeraSemanticNode]) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    let mut locals = BTreeSet::new();
    collect_context_from_nodes(nodes, &mut dependencies, &mut locals);
    dependencies
        .into_iter()
        .filter(|dependency| !locals.contains(root_identifier(dependency)))
        .collect()
}

fn collect_context_from_nodes(
    nodes: &[TeraSemanticNode],
    dependencies: &mut BTreeSet<String>,
    locals: &mut BTreeSet<String>,
) {
    for node in nodes {
        match node {
            TeraSemanticNode::Variable { expression } => {
                collect_identifiers_from_expression(expression, dependencies);
            }
            TeraSemanticNode::MacroDefinition {
                arguments, body, ..
            } => {
                let mut macro_locals = locals.clone();
                macro_locals.extend(arguments.keys().cloned());
                for expression in arguments.values().flatten() {
                    collect_identifiers_from_expression(expression, dependencies);
                }
                collect_context_from_nodes(body, dependencies, &mut macro_locals);
            }
            TeraSemanticNode::Set { key, value, .. } => {
                collect_identifiers_from_expression(value, dependencies);
                locals.insert(key.clone());
            }
            TeraSemanticNode::FilterSection { filter, body } => {
                collect_identifiers_from_call(filter, dependencies);
                collect_context_from_nodes(body, dependencies, &mut locals.clone());
            }
            TeraSemanticNode::Block { body, .. } => {
                collect_context_from_nodes(body, dependencies, &mut locals.clone());
            }
            TeraSemanticNode::For {
                key,
                value,
                container,
                body,
                empty_body,
            } => {
                collect_identifiers_from_expression(container, dependencies);
                let mut loop_locals = locals.clone();
                loop_locals.insert(value.clone());
                if let Some(key) = key {
                    loop_locals.insert(key.clone());
                }
                collect_context_from_nodes(body, dependencies, &mut loop_locals);
                if let Some(empty_body) = empty_body {
                    collect_context_from_nodes(empty_body, dependencies, &mut locals.clone());
                }
            }
            TeraSemanticNode::If {
                branches,
                otherwise,
            } => {
                for branch in branches {
                    collect_identifiers_from_expression(&branch.condition, dependencies);
                    collect_context_from_nodes(&branch.body, dependencies, &mut locals.clone());
                }
                if let Some(otherwise) = otherwise {
                    collect_context_from_nodes(otherwise, dependencies, &mut locals.clone());
                }
            }
            _ => {}
        }
    }
}

fn context_dependencies_for_expression(expression: &TeraSemanticExpression) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    collect_identifiers_from_expression(expression, &mut dependencies);
    dependencies.into_iter().collect()
}

fn context_dependencies_for_call(call: &TeraSemanticCall) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    collect_identifiers_from_call(call, &mut dependencies);
    dependencies.into_iter().collect()
}

fn collect_identifiers_from_expression(
    expression: &TeraSemanticExpression,
    dependencies: &mut BTreeSet<String>,
) {
    for filter in &expression.filters {
        collect_identifiers_from_call(filter, dependencies);
    }
    collect_identifiers_from_value(&expression.value, dependencies);
}

fn collect_identifiers_from_value(value: &TeraSemanticValue, dependencies: &mut BTreeSet<String>) {
    match value {
        TeraSemanticValue::Identifier(identifier) => {
            dependencies.insert(identifier.clone());
        }
        TeraSemanticValue::Math { left, right, .. }
        | TeraSemanticValue::Logic { left, right, .. } => {
            collect_identifiers_from_expression(left, dependencies);
            collect_identifiers_from_expression(right, dependencies);
        }
        TeraSemanticValue::Test {
            identifier,
            arguments,
            ..
        } => {
            dependencies.insert(identifier.clone());
            for argument in arguments {
                collect_identifiers_from_expression(argument, dependencies);
            }
        }
        TeraSemanticValue::MacroCall(call) | TeraSemanticValue::FunctionCall(call) => {
            collect_identifiers_from_call(call, dependencies)
        }
        TeraSemanticValue::Array(values) => {
            for value in values {
                collect_identifiers_from_expression(value, dependencies);
            }
        }
        TeraSemanticValue::StringConcat(values) => {
            for value in values {
                collect_identifiers_from_value(value, dependencies);
            }
        }
        TeraSemanticValue::In {
            needle, haystack, ..
        } => {
            collect_identifiers_from_expression(needle, dependencies);
            collect_identifiers_from_expression(haystack, dependencies);
        }
        _ => {}
    }
}

fn collect_identifiers_from_call(call: &TeraSemanticCall, dependencies: &mut BTreeSet<String>) {
    for argument in call.arguments.values() {
        collect_identifiers_from_expression(argument, dependencies);
    }
}

fn data_bindings_for_document(
    document: Option<&TeraSemanticDocument>,
    source_node_id: Option<&str>,
) -> Vec<ComponentDataBinding> {
    document
        .map(|document| data_bindings_for_nodes(&document.nodes, source_node_id))
        .unwrap_or_default()
}

fn data_bindings_for_nodes(
    nodes: &[TeraSemanticNode],
    source_node_id: Option<&str>,
) -> Vec<ComponentDataBinding> {
    let mut environment = HashMap::<String, String>::new();
    let mut bindings = Vec::new();
    collect_data_bindings(nodes, source_node_id, &mut environment, &mut bindings);
    bindings.sort_by(|left, right| {
        (
            left.name.as_str(),
            left.path.as_str(),
            left.producer.as_str(),
        )
            .cmp(&(
                right.name.as_str(),
                right.path.as_str(),
                right.producer.as_str(),
            ))
    });
    bindings.dedup();
    bindings
}

fn collect_data_bindings(
    nodes: &[TeraSemanticNode],
    source_node_id: Option<&str>,
    environment: &mut HashMap<String, String>,
    bindings: &mut Vec<ComponentDataBinding>,
) {
    for node in nodes {
        match node {
            TeraSemanticNode::Set { key, value, .. } => {
                let producer = producer_for_expression(value, environment);
                environment.insert(key.clone(), producer.clone());
                bindings.push(ComponentDataBinding {
                    name: key.clone(),
                    path: expression_path(value),
                    producer,
                    source_node_id: source_node_id.map(str::to_string),
                });
            }
            TeraSemanticNode::For {
                key,
                value,
                container,
                body,
                empty_body,
            } => {
                let producer = producer_for_expression(container, environment);
                let path = expression_path(container);
                let mut loop_environment = environment.clone();
                loop_environment.insert(value.clone(), format!("{producer}[]"));
                bindings.push(ComponentDataBinding {
                    name: value.clone(),
                    path: format!("{path}[]"),
                    producer: format!("{producer}[]"),
                    source_node_id: source_node_id.map(str::to_string),
                });
                if let Some(key) = key {
                    loop_environment.insert(key.clone(), format!("{producer}.__key"));
                    bindings.push(ComponentDataBinding {
                        name: key.clone(),
                        path: format!("{path}.__key"),
                        producer: format!("{producer}.__key"),
                        source_node_id: source_node_id.map(str::to_string),
                    });
                }
                collect_data_bindings(body, source_node_id, &mut loop_environment, bindings);
                if let Some(empty_body) = empty_body {
                    collect_data_bindings(
                        empty_body,
                        source_node_id,
                        &mut environment.clone(),
                        bindings,
                    );
                }
            }
            TeraSemanticNode::MacroDefinition { body, .. }
            | TeraSemanticNode::FilterSection { body, .. }
            | TeraSemanticNode::Block { body, .. } => {
                collect_data_bindings(body, source_node_id, &mut environment.clone(), bindings);
            }
            TeraSemanticNode::If {
                branches,
                otherwise,
            } => {
                for branch in branches {
                    collect_data_bindings(
                        &branch.body,
                        source_node_id,
                        &mut environment.clone(),
                        bindings,
                    );
                }
                if let Some(otherwise) = otherwise {
                    collect_data_bindings(
                        otherwise,
                        source_node_id,
                        &mut environment.clone(),
                        bindings,
                    );
                }
            }
            _ => {}
        }
    }
}

fn producer_for_expression(
    expression: &TeraSemanticExpression,
    environment: &HashMap<String, String>,
) -> String {
    match &expression.value {
        TeraSemanticValue::Identifier(identifier) => {
            let root = root_identifier(identifier);
            environment
                .get(root)
                .map(|producer| {
                    identifier
                        .strip_prefix(root)
                        .map(|suffix| format!("{producer}{suffix}"))
                        .unwrap_or_else(|| producer.clone())
                })
                .unwrap_or_else(|| identifier.clone())
        }
        TeraSemanticValue::FunctionCall(call) => {
            let reference = ["path", "url", "literal"]
                .iter()
                .find_map(|name| call.arguments.get(*name))
                .and_then(static_string_expression);
            reference
                .map(|reference| format!("{}:{reference}", call.name))
                .unwrap_or_else(|| format!("{}:dynamic", call.name))
        }
        TeraSemanticValue::MacroCall(call) => format!(
            "macro:{}::{}",
            call.namespace.as_deref().unwrap_or_default(),
            call.name
        ),
        _ => expression_path(expression),
    }
}

fn expression_path(expression: &TeraSemanticExpression) -> String {
    match &expression.value {
        TeraSemanticValue::Identifier(identifier) => identifier.clone(),
        TeraSemanticValue::String(value) => value.clone(),
        TeraSemanticValue::Integer(value) => value.to_string(),
        TeraSemanticValue::Float(value) => value.to_string(),
        TeraSemanticValue::Boolean(value) => value.to_string(),
        TeraSemanticValue::FunctionCall(call) => {
            format!("{}(...)", call.name)
        }
        TeraSemanticValue::MacroCall(call) => format!(
            "{}::{}(...)",
            call.namespace.as_deref().unwrap_or_default(),
            call.name
        ),
        TeraSemanticValue::Array(_) => "[...]".to_string(),
        TeraSemanticValue::Math { operator, .. } | TeraSemanticValue::Logic { operator, .. } => {
            format!("expresie {operator}")
        }
        TeraSemanticValue::Test {
            identifier, name, ..
        } => format!("{identifier} is {name}"),
        TeraSemanticValue::StringConcat(_) => "concatenare".to_string(),
        TeraSemanticValue::In { .. } => "expresie in".to_string(),
    }
}

fn static_string_expression(expression: &TeraSemanticExpression) -> Option<String> {
    match &expression.value {
        TeraSemanticValue::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn root_identifier(identifier: &str) -> &str {
    identifier
        .split(|character| matches!(character, '.' | '['))
        .next()
        .unwrap_or(identifier)
}

fn component_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    severity: SourceDiagnosticSeverity,
    file: Option<String>,
    source_node_id: Option<String>,
) -> ComponentDiagnostic {
    ComponentDiagnostic {
        code: code.into(),
        message: message.into(),
        severity,
        file,
        source_node_id,
    }
}

fn component_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "pana-component-graph-v1".hash(&mut hasher);
    prefix.hash(&mut hasher);
    for part in parts {
        part.hash(&mut hasher);
    }
    format!("{prefix}_{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        kernel::project_workspace::WorkspaceProjectionLease,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        source_graph::build_source_graph_from_workspace_projection,
    };

    #[test]
    fn projects_definitions_invocations_shadowing_and_data_bindings() {
        let root = test_project_root("semantic-components");
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let session = "component-graph-test".to_string();
        let lease = WorkspaceProjectionLease {
            project_root: canonical.clone(),
            runtime_session_id: session.clone(),
            revision: 1,
            workspace_transaction_id: Some("component-graph-test-1".to_string()),
            source_texts: HashMap::from([
                (
                    "zola.toml".to_string(),
                    "base_url = '/'\ntheme = 'demo'\n".to_string(),
                ),
                (
                    "templates/index.html".to_string(),
                    r#"{% import "macros/cards.html" as cards %}
{% set items = load_data(path="date/items.toml") %}
{% include ["partials/card.html", "partials/fallback.html"] %}
{% for item in items %}
{{ cards::render(item=item) }}
{% endfor %}
<div data-pana-component="tabs"></div>
"#
                    .to_string(),
                ),
                (
                    "templates/partials/card.html".to_string(),
                    "<article>{{ title }}</article>".to_string(),
                ),
                (
                    "themes/demo/templates/partials/card.html".to_string(),
                    "<article>Theme</article>".to_string(),
                ),
                (
                    "templates/macros/cards.html".to_string(),
                    "{% macro render(item) %}<article>{{ item.title }}</article>{% endmacro %}"
                        .to_string(),
                ),
                (
                    "date/items.toml".to_string(),
                    "[[items]]\ntitle = 'Unu'\n".to_string(),
                ),
                (
                    "content/_index.md".to_string(),
                    "+++\ntitle = 'Acasă'\n+++\n{{ video(id='abc') }}\n".to_string(),
                ),
                (
                    "templates/shortcodes/video.md".to_string(),
                    "{% set article = get_page(path='blog/post.md') %}\n**Video {{ id }} ({{ nth }}) — {{ article.title }}**"
                        .to_string(),
                ),
            ]),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::new(),
            accepted_disk: AcceptedProjectDiskManifest::new(
                session,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 100,
                },
            )
            .unwrap(),
        };
        let graph = build_source_graph_from_workspace_projection(&root, &lease).unwrap();
        let components = &graph.component_graph;

        let local_partial = components
            .definitions
            .iter()
            .find(|definition| {
                definition.name == "partials/card.html"
                    && definition.origin == crate::source_graph::model::ComponentOrigin::Project
            })
            .unwrap();
        let theme_partial = components
            .definitions
            .iter()
            .find(|definition| {
                definition.name == "partials/card.html"
                    && definition.origin == crate::source_graph::model::ComponentOrigin::Theme
            })
            .unwrap();
        assert!(local_partial.active);
        assert_eq!(
            theme_partial.shadowed_by.as_deref(),
            Some(local_partial.id.as_str())
        );

        assert!(components.invocations.iter().any(|invocation| {
            invocation.kind == crate::source_graph::model::ComponentInvocationKind::Include
                && invocation.status
                    == crate::source_graph::model::ComponentResolutionStatus::Resolved
        }));
        assert!(components.invocations.iter().any(|invocation| {
            invocation.kind == crate::source_graph::model::ComponentInvocationKind::MacroCall
                && invocation.status
                    == crate::source_graph::model::ComponentResolutionStatus::Resolved
        }));
        assert!(components
            .invocations
            .iter()
            .all(|invocation| invocation.target_reference != "tabs"));
        assert!(graph.block_graph.source_instances.iter().any(|instance| {
            instance.provider_id == "tabs"
                && instance.status == crate::source_graph::model::BlockResolutionStatus::Resolved
        }));
        assert!(components.invocations.iter().any(|invocation| {
            invocation.kind == crate::source_graph::model::ComponentInvocationKind::Shortcode
                && invocation.target_reference == "video"
                && invocation.status
                    == crate::source_graph::model::ComponentResolutionStatus::Resolved
        }));
        let shortcode = components
            .definitions
            .iter()
            .find(|definition| {
                definition.kind == crate::source_graph::model::ComponentDefinitionKind::Shortcode
                    && definition.name == "shortcodes/video.md"
            })
            .unwrap();
        assert!(shortcode.dependencies.iter().any(|dependency| {
            dependency.kind == crate::source_graph::model::ComponentDependencyKind::Runtime
                && dependency.reference == "zola:function:get_page"
        }));
        assert!(shortcode.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "zola_runtime_unavailable_in_shortcode"
                && diagnostic.message.contains("get_page")
        }));
        let index_template = components
            .definitions
            .iter()
            .find(|definition| definition.name == "index.html")
            .unwrap();
        assert!(index_template.dependencies.iter().any(|dependency| {
            dependency.kind == crate::source_graph::model::ComponentDependencyKind::Runtime
                && dependency.reference == "zola:function:load_data"
        }));
        assert!(components
            .definitions
            .iter()
            .filter(|definition| {
                definition.kind == crate::source_graph::model::ComponentDefinitionKind::InlineRepeat
            })
            .any(|definition| definition.data_bindings.iter().any(|binding| {
                binding.name == "item" && binding.producer.contains("load_data")
            })));

        fs::remove_dir_all(root).unwrap();
    }

    fn test_project_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-component-graph-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
}
