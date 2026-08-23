use super::*;
use super::{
    provenance::editor_source_provenance,
    snapshot::{
        boundary_effect_scope, is_document_fragment_root, is_document_wrapper_block,
        source_html_attributes, source_origin,
    },
};

struct EditorNavigationViewBuilder<'a> {
    model: &'a ProjectModel,
    template: &'a SourceGraphTemplate,
    source_nodes: HashMap<&'a str, &'a SourceNode>,
    relation_indices_by_from: HashMap<&'a str, Vec<usize>>,
    templates_by_node_id: HashMap<&'a str, &'a SourceGraphTemplate>,
    templates_by_file: HashMap<&'a str, &'a SourceGraphTemplate>,
    include_consumer_counts: HashMap<String, usize>,
    dynamic_widget_labels_by_source: HashMap<&'a str, String>,
    global_nodes_by_source: HashMap<&'a str, Vec<&'a EditorNavigationNode>>,
    markdown_nodes_by_template_source: HashMap<&'a str, Vec<&'a EditorNavigationNode>>,
    view_nodes: Vec<EditorNavigationViewNode>,
    view_ranges: HashMap<String, SourceRange>,
    editor_nodes: Vec<EditorNavigationNode>,
}

impl<'a> EditorNavigationViewBuilder<'a> {
    fn new(
        model: &'a ProjectModel,
        template: &'a SourceGraphTemplate,
        global_nodes: &'a [EditorNavigationNode],
    ) -> Self {
        let source_nodes = model
            .source_graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let mut relation_indices_by_from = HashMap::<&str, Vec<usize>>::new();
        for (index, relation) in model.source_graph.relations.iter().enumerate() {
            relation_indices_by_from
                .entry(relation.from.as_str())
                .or_default()
                .push(index);
        }
        let mut templates_by_node_id = HashMap::new();
        let mut templates_by_file = HashMap::new();
        for template in &model.source_graph.templates {
            templates_by_node_id
                .entry(template.node_id.as_str())
                .or_insert(template);
            templates_by_file
                .entry(template.file.as_str())
                .or_insert(template);
        }
        let mut include_consumer_counts = HashMap::<String, usize>::new();
        for group in model
            .source_graph
            .templates
            .iter()
            .flat_map(|template| template.include_groups.iter())
        {
            let normalized_targets = group
                .targets
                .iter()
                .map(|target| normalized_template_name(target))
                .collect::<HashSet<_>>();
            for target in normalized_targets {
                *include_consumer_counts.entry(target).or_default() += 1;
            }
        }
        let mut dynamic_widget_labels_by_source = HashMap::<&str, String>::new();
        for instance in &model.source_graph.dynamic_widget_graph.source_instances {
            let Some(label) = dynamic_widget_navigation_label(model, instance.properties.as_ref())
            else {
                continue;
            };
            for source_node_id in &instance.root_source_node_ids {
                dynamic_widget_labels_by_source.insert(source_node_id.as_str(), label.clone());
            }
        }
        let mut global_nodes_by_source = HashMap::<&str, Vec<&EditorNavigationNode>>::new();
        for node in global_nodes {
            if let Some(source_node_id) = node.source_node_id.as_deref() {
                global_nodes_by_source
                    .entry(source_node_id)
                    .or_default()
                    .push(node);
            }
        }
        for nodes in global_nodes_by_source.values_mut() {
            nodes.sort_by(|left, right| {
                left.order
                    .cmp(&right.order)
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
        let mut markdown_nodes_by_template_source =
            HashMap::<&str, Vec<&EditorNavigationNode>>::new();
        for node in global_nodes.iter().filter(|node| {
            node.kind == EditorNavigationNodeKind::Boundary
                && node
                    .boundary
                    .as_ref()
                    .is_some_and(|boundary| boundary.kind == EditorNavigationBoundaryKind::Markdown)
        }) {
            if let Some(template_source_node_id) = node
                .source_provenance
                .composition
                .as_ref()
                .and_then(|reference| reference.source_node_id.as_deref())
            {
                markdown_nodes_by_template_source
                    .entry(template_source_node_id)
                    .or_default()
                    .push(node);
            }
        }
        Self {
            model,
            template,
            source_nodes,
            relation_indices_by_from,
            templates_by_node_id,
            templates_by_file,
            include_consumer_counts,
            dynamic_widget_labels_by_source,
            global_nodes_by_source,
            markdown_nodes_by_template_source,
            view_nodes: Vec::new(),
            view_ranges: HashMap::new(),
            editor_nodes: Vec::new(),
        }
    }

    fn build(
        mut self,
    ) -> (
        Vec<String>,
        Vec<EditorNavigationViewNode>,
        Vec<EditorNavigationNode>,
    ) {
        let mut roots = self
            .source_nodes
            .get(self.template.node_id.as_str())
            .map(|root| root.children.clone())
            .unwrap_or_default();
        self.sort_source_ids(&mut roots);
        for source_node_id in roots {
            self.add_source_node(&source_node_id, None, None);
        }
        let mut root_node_ids = self.rebuild_visual_hierarchy();
        if root_node_ids.is_empty() {
            if let Some(authoring_root_id) = self.add_empty_document_authoring_root() {
                root_node_ids.push(authoring_root_id);
            }
        }
        (root_node_ids, self.view_nodes, self.editor_nodes)
    }

    /// Proiectează rădăcina goală a documentului activ ca suprafață de autor,
    /// nu ca gate Tera. Pentru pagini, ancora este block-ul local; pentru un
    /// fragment deschis direct, ancora este chiar rădăcina Template/Partial.
    /// Straturi primește aceeași identitate Rust pe care o folosește Canvas.
    fn add_empty_document_authoring_root(&mut self) -> Option<String> {
        let candidate = self
            .source_nodes
            .values()
            .copied()
            .filter(|source| {
                source.origin == SourceOrigin::Local
                    && (is_document_wrapper_block(source, self.template, &self.source_nodes)
                        || is_document_fragment_root(source, self.template))
            })
            .filter_map(|source| {
                let matches = self.global_nodes_by_source.get(source.id.as_str())?;
                let representative = matches.iter().copied().find(|node| {
                    node.kind == EditorNavigationNodeKind::Boundary
                        && is_empty_document_authoring_boundary(node, matches)
                        && node.capabilities.requires_edit_scope_id.is_none()
                })?;
                let order = source.range.as_ref().map(|range| range.start).unwrap_or(0);
                Some((
                    order,
                    source.id.clone(),
                    source.clone(),
                    representative.clone(),
                ))
            })
            .min_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))?;
        let (order, source_id, source, representative) = candidate;
        let view_node_id = format!("editor_view_authoring_root:{source_id}");
        let label = self
            .template
            .file
            .replace('\\', "/")
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(self.template.file.as_str())
            .to_string();
        let mut capabilities = representative.capabilities.clone();
        capabilities.can_select = true;
        capabilities.can_inspect = true;
        capabilities.can_enter_boundary = false;
        capabilities.can_move_atomic = false;
        capabilities.can_move = false;
        capabilities.can_edit_text = false;
        capabilities.can_edit_attributes = false;
        capabilities.read_only = false;
        capabilities.requires_edit_scope_id = None;

        self.view_nodes.push(EditorNavigationViewNode {
            id: view_node_id.clone(),
            editor_node_id: Some(representative.id),
            parent_id: None,
            children: Vec::new(),
            order,
            kind: EditorNavigationViewNodeKind::Slot,
            label,
            tag: None,
            source_node_id: Some(source_id),
            source_kind: Some(source.kind),
            file: source.file,
            origin: EditorNavigationOrigin::Project,
            theme_name: None,
            render_instance_ids: representative
                .boundary
                .as_ref()
                .map(|boundary| boundary.root_render_instance_ids.clone())
                .unwrap_or_default(),
            boundary: representative.boundary,
            relation: None,
            capabilities,
        });
        Some(view_node_id)
    }

    fn add_source_node(
        &mut self,
        source_node_id: &str,
        parent_view_id: Option<&str>,
        inherited_scope_id: Option<&str>,
    ) -> Vec<String> {
        let Some(source) = self.source_nodes.get(source_node_id).copied().cloned() else {
            return Vec::new();
        };
        if source.file != self.template.file {
            return Vec::new();
        }
        if source.kind == SourceNodeKind::BlockMarker {
            return Vec::new();
        }
        if let Some(markdown) = self.add_markdown_projection(&source, parent_view_id) {
            return markdown;
        }

        let document_wrapper_block =
            is_document_wrapper_block(&source, self.template, &self.source_nodes);
        if document_wrapper_block || !source_kind_is_visual_layer(&source.kind) {
            let mut children = source.children.clone();
            self.sort_source_ids(&mut children);
            let mut promoted = Vec::new();
            for child_id in children {
                promoted.extend(self.add_source_node(
                    &child_id,
                    parent_view_id,
                    inherited_scope_id,
                ));
            }
            return promoted;
        }

        let view_kind = view_node_kind(&source.kind);
        let view_node_id = editor_view_node_id(&source.id);
        let source_editor_node_id = editor_source_node_id(&source.id);
        let is_relation = matches!(
            source.kind,
            SourceNodeKind::Extends | SourceNodeKind::Import
        );
        let is_gate = source_kind_is_gate(&source.kind);
        let relation = self.navigation_relation(&source);
        let matches = self
            .global_nodes_by_source
            .get(source.id.as_str())
            .cloned()
            .unwrap_or_default();
        let representative = matches.first().copied();
        let mut render_instance_ids = matches
            .iter()
            .flat_map(|node| {
                node.render_instance_id.iter().cloned().chain(
                    node.boundary
                        .iter()
                        .flat_map(|boundary| boundary.root_render_instance_ids.iter().cloned()),
                )
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        render_instance_ids.sort();
        let include_consumer_count = source
            .tera_template_target()
            .map(normalized_template_name)
            .and_then(|target| self.include_consumer_counts.get(&target).copied());
        let boundary = view_boundary(self.model, &source, &matches, include_consumer_count);
        let local_source = source.origin == SourceOrigin::Local;
        let editor_node_id = (!is_relation).then_some(source_editor_node_id.clone());
        let requires_scope_id = inherited_scope_id.map(str::to_string);
        let can_move_atomic = local_source
            && source_kind_is_atomic(&source.kind)
            && !matches!(view_kind, EditorNavigationViewNodeKind::Relation);
        let can_move = match view_kind {
            EditorNavigationViewNodeKind::HtmlElement => {
                local_source && inherited_scope_id.is_none() && source.capabilities.can_move
            }
            EditorNavigationViewNodeKind::Boundary | EditorNavigationViewNodeKind::Slot => {
                can_move_atomic
            }
            EditorNavigationViewNodeKind::Source => {
                local_source && source_kind_is_atomic(&source.kind)
            }
            EditorNavigationViewNodeKind::Relation => false,
        };
        let capabilities = EditorNavigationCapabilities {
            can_select: representative.is_some(),
            can_inspect: representative.is_some_and(|node| node.capabilities.can_inspect),
            can_open_in_code: source.capabilities.can_open_in_code,
            can_enter_boundary: is_gate && local_source,
            can_move_atomic,
            can_move,
            can_edit_text: local_source
                && inherited_scope_id.is_none()
                && source.capabilities.can_edit_text,
            can_edit_attributes: local_source
                && inherited_scope_id.is_none()
                && source.capabilities.can_edit_attributes,
            read_only: !local_source || inherited_scope_id.is_some(),
            requires_edit_scope_id: if is_gate {
                Some(source_editor_node_id.clone())
            } else {
                requires_scope_id.clone()
            },
            reason_code: source.capabilities.reason_code,
        };
        let order = source.range.as_ref().map(|range| range.start).unwrap_or(0);
        let tag = (source.kind == SourceNodeKind::Html)
            .then(|| source_html_tag(&source.label))
            .flatten();
        let display_label = self
            .dynamic_widget_labels_by_source
            .get(source.id.as_str())
            .cloned()
            .unwrap_or_else(|| source.label.clone());
        if let Some(range) = source.range.clone() {
            self.view_ranges.insert(view_node_id.clone(), range);
        }

        let view_node_index = self.view_nodes.len();
        self.view_nodes.push(EditorNavigationViewNode {
            id: view_node_id.clone(),
            editor_node_id: editor_node_id.clone(),
            parent_id: parent_view_id.map(str::to_string),
            children: Vec::new(),
            order,
            kind: view_kind,
            label: display_label.clone(),
            tag: tag.clone(),
            source_node_id: Some(source.id.clone()),
            source_kind: Some(source.kind.clone()),
            file: source.file.clone(),
            origin: source_origin(Some(&source)),
            theme_name: source.theme_name.clone(),
            render_instance_ids: render_instance_ids.clone(),
            boundary: boundary.clone(),
            relation,
            capabilities: capabilities.clone(),
        });

        if let Some(editor_node_id) = editor_node_id {
            let component_definition_ids =
                union_editor_ids(&matches, |node| &node.component_definition_ids);
            let component_invocation_ids =
                union_editor_ids(&matches, |node| &node.component_invocation_ids);
            let source_provenance =
                editor_source_provenance(self.model, Some(&source), &component_invocation_ids);
            self.editor_nodes.push(EditorNavigationNode {
                id: editor_node_id,
                parent_id: None,
                children: Vec::new(),
                order,
                kind: if source.kind == SourceNodeKind::Html {
                    EditorNavigationNodeKind::HtmlElement
                } else {
                    EditorNavigationNodeKind::Boundary
                },
                label: display_label,
                tag,
                source_node_id: Some(source.id.clone()),
                render_instance_id: render_instance_ids.first().cloned(),
                source_kind: Some(source.kind.clone()),
                file: Some(source.file.clone()),
                range: source.range.clone(),
                origin: source_origin(Some(&source)),
                theme_name: source.theme_name.clone(),
                source_provenance,
                provenance_stack: representative
                    .map(|node| node.provenance_stack.clone())
                    .unwrap_or_else(|| vec![source.id.clone()]),
                component_definition_ids,
                component_invocation_ids,
                block_definition_ids: union_editor_ids(&matches, |node| &node.block_definition_ids),
                block_source_instance_ids: union_editor_ids(&matches, |node| {
                    &node.block_source_instance_ids
                }),
                dynamic_widget_provider_ids: union_editor_ids(&matches, |node| {
                    &node.dynamic_widget_provider_ids
                }),
                dynamic_widget_source_instance_ids: union_editor_ids(&matches, |node| {
                    &node.dynamic_widget_source_instance_ids
                }),
                binding_key: representative.and_then(|node| node.binding_key.clone()),
                binding_path: representative.and_then(|node| node.binding_path.clone()),
                boundary,
                capabilities,
                source_html_attributes: source_html_attributes(self.model, Some(&source)),
            });
        }

        let child_scope_id = if is_gate {
            Some(source_editor_node_id.as_str())
        } else {
            inherited_scope_id
        };
        let mut children = source.children.clone();
        self.sort_source_ids(&mut children);
        let mut child_view_ids = Vec::new();
        for child_id in children {
            child_view_ids.extend(self.add_source_node(
                &child_id,
                Some(&view_node_id),
                child_scope_id,
            ));
        }
        self.view_nodes[view_node_index].children = child_view_ids;
        vec![view_node_id]
    }

    fn add_markdown_projection(
        &mut self,
        template_source: &SourceNode,
        parent_view_id: Option<&str>,
    ) -> Option<Vec<String>> {
        let matches = self
            .markdown_nodes_by_template_source
            .get(template_source.id.as_str())?
            .clone();
        let representative = matches.first().copied()?;
        let view_node_id = format!("editor_view_markdown:{}", template_source.id);
        let mut render_instance_ids = matches
            .iter()
            .flat_map(|node| {
                node.boundary
                    .iter()
                    .flat_map(|boundary| boundary.root_render_instance_ids.iter().cloned())
            })
            .collect::<Vec<_>>();
        render_instance_ids.sort();
        render_instance_ids.dedup();
        let mut boundary = representative.boundary.clone();
        if let Some(boundary) = boundary.as_mut() {
            boundary.root_render_instance_ids = render_instance_ids.clone();
            boundary.rendered_instance_count = matches.len();
        }
        let order = template_source
            .range
            .as_ref()
            .map(|range| range.start)
            .unwrap_or(representative.order);
        if let Some(range) = template_source.range.clone() {
            self.view_ranges.insert(view_node_id.clone(), range);
        }
        self.view_nodes.push(EditorNavigationViewNode {
            id: view_node_id.clone(),
            editor_node_id: Some(representative.id.clone()),
            parent_id: parent_view_id.map(str::to_string),
            children: Vec::new(),
            order,
            kind: EditorNavigationViewNodeKind::Boundary,
            label: representative.label.clone(),
            tag: None,
            source_node_id: representative.source_node_id.clone(),
            source_kind: representative.source_kind.clone(),
            file: representative
                .file
                .clone()
                .unwrap_or_else(|| template_source.file.clone()),
            origin: representative.origin,
            theme_name: representative.theme_name.clone(),
            render_instance_ids,
            boundary,
            relation: None,
            capabilities: representative.capabilities.clone(),
        });
        Some(vec![view_node_id])
    }

    fn sort_source_ids(&self, source_ids: &mut [String]) {
        source_ids.sort_by(|left, right| {
            let left_node = self.source_nodes.get(left.as_str()).copied();
            let right_node = self.source_nodes.get(right.as_str()).copied();
            source_order(left_node)
                .cmp(&source_order(right_node))
                .then_with(|| left.cmp(right))
        });
    }

    fn rebuild_visual_hierarchy(&mut self) -> Vec<String> {
        #[derive(Clone, Copy)]
        struct RangedViewNode {
            node_index: usize,
            start: usize,
            end: usize,
            order: usize,
            can_parent: bool,
        }

        let mut ranged_nodes = self
            .view_nodes
            .iter()
            .enumerate()
            .filter_map(|(node_index, node)| {
                let range = self.view_ranges.get(&node.id)?;
                Some(RangedViewNode {
                    node_index,
                    start: range.start,
                    end: range.end,
                    order: node.order,
                    can_parent: matches!(
                        node.kind,
                        EditorNavigationViewNodeKind::HtmlElement
                            | EditorNavigationViewNodeKind::Boundary
                    ),
                })
            })
            .collect::<Vec<_>>();
        ranged_nodes.sort_by(|left, right| {
            left.start
                .cmp(&right.start)
                .then_with(|| right.end.cmp(&left.end))
                .then_with(|| left.order.cmp(&right.order))
                .then_with(|| {
                    self.view_nodes[left.node_index]
                        .id
                        .cmp(&self.view_nodes[right.node_index].id)
                })
        });

        // SourceGraph produce intervale vizuale disjuncte sau imbricate. Sortarea
        // costă O(n log n), iar stiva păstrează cel mai apropiat container activ;
        // fiecare interval intră și iese cel mult o dată. Gruparea intervalelor
        // identice păstrează regula veche: ele nu se pot adopta reciproc, iar
        // candidatul cu order maxim și id minim devine container pentru descendenți.
        let mut parent_by_index = vec![None; self.view_nodes.len()];
        let mut ancestor_stack = Vec::<RangedViewNode>::new();
        let mut group_start = 0;
        while group_start < ranged_nodes.len() {
            let start = ranged_nodes[group_start].start;
            let end = ranged_nodes[group_start].end;
            let mut group_end = group_start + 1;
            while group_end < ranged_nodes.len()
                && ranged_nodes[group_end].start == start
                && ranged_nodes[group_end].end == end
            {
                group_end += 1;
            }

            while ancestor_stack
                .last()
                .is_some_and(|candidate| candidate.end < end || candidate.start > start)
            {
                ancestor_stack.pop();
            }
            let parent_id = ancestor_stack
                .last()
                .map(|candidate| self.view_nodes[candidate.node_index].id.clone());
            for ranged in &ranged_nodes[group_start..group_end] {
                parent_by_index[ranged.node_index] = Some(parent_id.clone());
            }

            if let Some(representative) = ranged_nodes[group_start..group_end]
                .iter()
                .filter(|candidate| candidate.can_parent)
                .min_by(|left, right| {
                    right.order.cmp(&left.order).then_with(|| {
                        self.view_nodes[left.node_index]
                            .id
                            .cmp(&self.view_nodes[right.node_index].id)
                    })
                })
            {
                ancestor_stack.push(*representative);
            }
            group_start = group_end;
        }

        for (node, parent) in self.view_nodes.iter_mut().zip(parent_by_index) {
            if let Some(parent) = parent {
                node.parent_id = parent;
            }
            node.children.clear();
        }

        let mut children_by_parent = HashMap::<String, Vec<(usize, String)>>::new();
        for node in &self.view_nodes {
            if let Some(parent_id) = node.parent_id.as_ref() {
                children_by_parent
                    .entry(parent_id.clone())
                    .or_default()
                    .push((node.order, node.id.clone()));
            }
        }
        for children in children_by_parent.values_mut() {
            children.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        }
        for node in &mut self.view_nodes {
            if let Some(children) = children_by_parent.get(&node.id) {
                node.children = children.iter().map(|(_, child)| child.clone()).collect();
            }
        }

        let mut roots = self
            .view_nodes
            .iter()
            .filter(|node| node.parent_id.is_none())
            .map(|node| (node.order, node.id.clone()))
            .collect::<Vec<_>>();
        roots.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        roots.into_iter().map(|(_, id)| id).collect()
    }

    fn navigation_relation(&self, source: &SourceNode) -> Option<EditorNavigationRelation> {
        let relation_kind = match source.kind {
            SourceNodeKind::Extends => EditorNavigationRelationKind::Extends,
            SourceNodeKind::Include => EditorNavigationRelationKind::Include,
            SourceNodeKind::Import => EditorNavigationRelationKind::Import,
            SourceNodeKind::Block => {
                return self.block_override_relation(source);
            }
            _ => return None,
        };
        let target_template_name =
            source
                .tera_template_target()
                .map(str::to_string)
                .or_else(|| match source.kind {
                    SourceNodeKind::Extends => self.template.extends.clone(),
                    _ => None,
                });
        let target = self.resolved_template_target(
            &self.template.node_id,
            source_relation_kind(&source.kind)?,
            target_template_name.as_deref(),
        );
        Some(EditorNavigationRelation {
            kind: relation_kind,
            target_document_path: target.map(|template| template.file.clone()),
            target_source_node_id: target.map(|template| template.node_id.clone()),
            target_template_name: target_template_name
                .or_else(|| target.map(|template| template.name.clone())),
        })
    }

    fn resolved_template_target(
        &self,
        from_node_id: &str,
        kind: SourceRelationKind,
        target_name: Option<&str>,
    ) -> Option<&'a SourceGraphTemplate> {
        let target_name = target_name.map(normalized_template_name);
        let relation = self
            .relation_indices_by_from
            .get(from_node_id)?
            .iter()
            .filter_map(|index| self.model.source_graph.relations.get(*index))
            .find(|relation| {
                relation.kind == kind
                    && target_name
                        .as_ref()
                        .is_none_or(|target| normalized_template_name(&relation.label) == *target)
            })?;
        self.templates_by_node_id.get(relation.to.as_str()).copied()
    }

    fn block_override_relation(&self, source: &SourceNode) -> Option<EditorNavigationRelation> {
        let relation = self
            .relation_indices_by_from
            .get(source.id.as_str())?
            .iter()
            .filter_map(|index| self.model.source_graph.relations.get(*index))
            .find(|relation| relation.kind == SourceRelationKind::OverridesBlock)?;
        let target_node = self.source_nodes.get(relation.to.as_str()).copied();
        let target_template =
            target_node.and_then(|node| self.templates_by_file.get(node.file.as_str()).copied());
        Some(EditorNavigationRelation {
            kind: EditorNavigationRelationKind::BlockOverride,
            target_document_path: target_template.map(|template| template.file.clone()),
            target_source_node_id: Some(relation.to.clone()),
            target_template_name: target_template.map(|template| template.name.clone()),
        })
    }
}

fn is_empty_document_authoring_boundary(
    boundary_node: &EditorNavigationNode,
    source_matches: &[&EditorNavigationNode],
) -> bool {
    let Some(boundary) = boundary_node.boundary.as_ref() else {
        return false;
    };
    if boundary.empty {
        return true;
    }
    !boundary.root_render_instance_ids.is_empty()
        && boundary
            .root_render_instance_ids
            .iter()
            .all(|render_instance_id| {
                let render_node_id = format!("editor_render:{render_instance_id}");
                source_matches.iter().copied().any(|render_node| {
                    render_node.id == render_node_id
                        && render_node.parent_id.as_deref() == Some(boundary_node.id.as_str())
                        && render_node.source_node_id == boundary_node.source_node_id
                        && render_node.source_kind == Some(SourceNodeKind::Block)
                        && render_node.tag.as_deref() == Some("div")
                })
            })
}

pub(super) fn build_editor_navigation_view(
    model: &ProjectModel,
    global_nodes: &[EditorNavigationNode],
    active_document_path: &str,
    preview_context_render_instance_id: Option<&str>,
) -> Result<(EditorNavigationView, Vec<EditorNavigationNode>), String> {
    let active_document_path = normalize_editor_document_path(active_document_path)?;
    let template = model
        .source_graph
        .templates
        .iter()
        .find(|template| same_editor_document_path(&template.file, &active_document_path))
        .ok_or_else(|| {
            format!(
                "EditorNavigationView nu găsește documentul activ {active_document_path:?} în SourceGraph."
            )
        })?;
    let breadcrumbs = editor_navigation_breadcrumbs(model, template);
    let builder = EditorNavigationViewBuilder::new(model, template, global_nodes);
    let (root_node_ids, nodes, editor_nodes) = builder.build();
    let preview_context_render_instance_id = preview_context_render_instance_id
        .filter(|render_instance_id| {
            global_nodes.iter().any(|node| {
                node.render_instance_id.as_deref() == Some(*render_instance_id)
                    || node.boundary.as_ref().is_some_and(|boundary| {
                        boundary
                            .root_render_instance_ids
                            .iter()
                            .any(|candidate| candidate == render_instance_id)
                    })
            })
        })
        .map(str::to_string);
    Ok((
        EditorNavigationView {
            active_document_path,
            active_template_name: template.name.clone(),
            active_source_node_id: template.node_id.clone(),
            breadcrumbs,
            root_node_ids,
            nodes,
            preview_context_render_instance_id,
        },
        editor_nodes,
    ))
}

fn editor_navigation_breadcrumbs(
    model: &ProjectModel,
    active: &SourceGraphTemplate,
) -> Vec<EditorNavigationBreadcrumb> {
    let mut chain = vec![active];
    let mut current = active;
    let mut visited = HashSet::from([active.node_id.as_str()]);
    while let Some(parent) = resolved_template_target(
        model,
        current,
        SourceRelationKind::Extends,
        current.extends.as_deref(),
    ) {
        if !visited.insert(parent.node_id.as_str()) {
            break;
        }
        chain.push(parent);
        current = parent;
    }
    chain.reverse();
    chain
        .into_iter()
        .map(|template| EditorNavigationBreadcrumb {
            document_path: template.file.clone(),
            template_name: template.name.clone(),
            source_node_id: template.node_id.clone(),
            origin: match template.origin {
                SourceOrigin::Local => EditorNavigationOrigin::Project,
                SourceOrigin::Theme => EditorNavigationOrigin::Theme,
            },
            theme_name: template.theme_name.clone(),
            current: template.node_id == active.node_id,
        })
        .collect()
}

fn resolved_template_target<'a>(
    model: &'a ProjectModel,
    from: &SourceGraphTemplate,
    kind: SourceRelationKind,
    target_name: Option<&str>,
) -> Option<&'a SourceGraphTemplate> {
    let target_name = target_name.map(normalized_template_name);
    let relation = model.source_graph.relations.iter().find(|relation| {
        relation.from == from.node_id
            && relation.kind == kind
            && target_name
                .as_ref()
                .is_none_or(|target| normalized_template_name(&relation.label) == *target)
    })?;
    model
        .source_graph
        .templates
        .iter()
        .find(|template| template.node_id == relation.to)
}

fn source_relation_kind(kind: &SourceNodeKind) -> Option<SourceRelationKind> {
    match kind {
        SourceNodeKind::Extends => Some(SourceRelationKind::Extends),
        SourceNodeKind::Include => Some(SourceRelationKind::Includes),
        SourceNodeKind::Import => Some(SourceRelationKind::Imports),
        _ => None,
    }
}

fn view_boundary(
    model: &ProjectModel,
    source: &SourceNode,
    matching_nodes: &[&EditorNavigationNode],
    include_consumer_count: Option<usize>,
) -> Option<EditorNavigationBoundary> {
    if source.kind == SourceNodeKind::Html
        || matches!(
            source.kind,
            SourceNodeKind::Extends | SourceNodeKind::Import | SourceNodeKind::BlockMarker
        )
    {
        return None;
    }
    let boundaries = matching_nodes
        .iter()
        .filter_map(|node| node.boundary.as_ref())
        .collect::<Vec<_>>();
    let mut roots = boundaries
        .iter()
        .flat_map(|boundary| boundary.root_render_instance_ids.iter().cloned())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    roots.sort();
    let target = source.tera_template_target().map(str::to_string);
    let effect_scope = match source.kind {
        SourceNodeKind::Include | SourceNodeKind::Macro => {
            EditorNavigationEffectScope::SharedDefinition
        }
        _ => boundary_effect_scope(Some(&source.kind)),
    };
    let rendered_instance_count = if source.kind == SourceNodeKind::Include {
        include_consumer_count
            .unwrap_or(boundaries.len())
            .max(boundaries.len())
    } else {
        boundaries.len()
    };
    let (kind, component_kind) = editor_boundary_classification(model, Some(source), false);
    Some(EditorNavigationBoundary {
        kind,
        component_kind,
        boundary_instance_id: boundaries
            .first()
            .map(|boundary| boundary.boundary_instance_id.clone())
            .unwrap_or_else(|| format!("source_boundary:{}", source.id)),
        source_node_id: source.id.clone(),
        root_render_instance_ids: roots.clone(),
        atomic_when_closed: true,
        effect_scope,
        rendered_instance_count,
        target,
        empty: roots.is_empty(),
    })
}

fn union_editor_ids(
    nodes: &[&EditorNavigationNode],
    values: impl Fn(&EditorNavigationNode) -> &[String],
) -> Vec<String> {
    let mut result = nodes
        .iter()
        .flat_map(|node| values(node).iter().cloned())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn dynamic_widget_navigation_label(
    model: &ProjectModel,
    properties: Option<&DynamicWidgetProperties>,
) -> Option<String> {
    match properties? {
        DynamicWidgetProperties::DynamicField(properties) => {
            let label = model
                .source_graph
                .dynamic_widget_graph
                .value_catalog
                .iter()
                .find(|definition| {
                    definition.source == properties.binding.source
                        && definition.contexts.contains(&properties.binding.context)
                })
                .map(|definition| definition.label.as_str())
                .filter(|label| !label.trim().is_empty())
                .unwrap_or(properties.label.as_str());
            Some(if label.trim().is_empty() {
                "Câmp dinamic".to_string()
            } else {
                format!("Câmp dinamic · {label}")
            })
        }
        DynamicWidgetProperties::Listing(properties) => {
            Some(format!("Listing · {}", properties.listing_item_template))
        }
    }
}

fn view_node_kind(kind: &SourceNodeKind) -> EditorNavigationViewNodeKind {
    match kind {
        SourceNodeKind::Html => EditorNavigationViewNodeKind::HtmlElement,
        SourceNodeKind::Extends | SourceNodeKind::Import => EditorNavigationViewNodeKind::Relation,
        kind if source_kind_is_gate(kind) => EditorNavigationViewNodeKind::Boundary,
        _ => EditorNavigationViewNodeKind::Source,
    }
}

fn source_kind_is_visual_layer(kind: &SourceNodeKind) -> bool {
    *kind == SourceNodeKind::Html || source_kind_is_gate(kind)
}

fn source_kind_is_gate(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Filter
            | SourceNodeKind::Raw
    )
}

fn source_kind_is_atomic(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Include
            | SourceNodeKind::Import
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::Set
            | SourceNodeKind::SetGlobal
            | SourceNodeKind::Filter
            | SourceNodeKind::Break
            | SourceNodeKind::Continue
            | SourceNodeKind::Super
            | SourceNodeKind::TeraVariable
            | SourceNodeKind::TeraComment
            | SourceNodeKind::Raw
            | SourceNodeKind::Tera
    )
}

fn source_order(source: Option<&SourceNode>) -> usize {
    source
        .and_then(|source| source.range.as_ref())
        .map(|range| range.start)
        .unwrap_or(usize::MAX)
}

pub(super) fn source_html_tag(label: &str) -> Option<String> {
    label
        .strip_prefix('<')
        .and_then(|label| label.split([' ', '>', '.']).next())
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
}

fn editor_view_node_id(source_node_id: &str) -> String {
    format!("editor_view:{source_node_id}")
}

fn editor_source_node_id(source_node_id: &str) -> String {
    format!("editor_source:{source_node_id}")
}

fn normalized_template_name(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}

fn normalize_editor_document_path(value: &str) -> Result<String, String> {
    let value = value.trim().replace('\\', "/");
    if value.is_empty()
        || value.len() > 2_048
        || value.starts_with('/')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(
            "EditorNavigationView a refuzat calea documentului activ deoarece este invalidă."
                .to_string(),
        );
    }
    Ok(value)
}

pub(super) fn same_editor_document_path(left: &str, right: &str) -> bool {
    left.trim_start_matches('/').replace('\\', "/")
        == right.trim_start_matches('/').replace('\\', "/")
}

pub(super) fn same_preview_route(left: &str, right: &str) -> bool {
    normalize_preview_route(left) == normalize_preview_route(right)
}

fn normalize_preview_route(route: &str) -> String {
    let route = route.split(['?', '#']).next().unwrap_or(route).trim();
    let mut normalized = if route.is_empty() {
        "/".to_string()
    } else if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    };
    if normalized.len() > 1 && normalized.ends_with("/index.html") {
        normalized.truncate(normalized.len() - "index.html".len());
    }
    normalized
}
