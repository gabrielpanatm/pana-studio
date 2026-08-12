use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use crate::{
    preview::preprocess::annotate::{paths::is_template_relative_path, range::LineIndex},
    project_model::zola_image_engine::{inspect_zola_image_at, ZolaImagePresentation},
    source_graph::{
        html::should_project_html_tag,
        mixed_cst::{parse_mixed_cst, MixedCstKind},
        model::{MarkdownProjection, MarkdownProjectionKind, SourceGraph, SourceNodeKind},
        tera::{for_collection_root, parse_tera_items, set_assignment_name, TeraItemKind},
    },
};

#[cfg(test)]
use crate::source_graph::identity::ProvisionalSourceNodeIdAllocator;

#[derive(Default)]
pub struct SourceIdIndex {
    pub(super) by_source_location: HashMap<String, String>,
    pub(super) by_template_source_location: HashMap<String, String>,
    pub(super) template_source_by_html_location: HashMap<String, String>,
    pub(super) scope_start_marker_by_location: HashMap<String, String>,
    pub(super) external_scope_start_by_scope_location: HashSet<String>,
    pub(super) zola_image_by_source_location: HashMap<String, ZolaImagePresentation>,
    pub(super) markdown_projection_by_location: HashMap<String, MarkdownProjection>,
    pub(super) shortcode_projection_by_template: HashMap<String, MarkdownProjection>,
}

#[derive(Clone)]
pub(super) struct TeraScopeAnchor {
    pub(super) node_id: String,
    start: usize,
    end: usize,
}

struct SetPreludeAnchor {
    variable: String,
    start: usize,
    location: String,
    parent: Option<String>,
}

#[derive(Clone)]
struct CanonicalNodeAnchor {
    id: String,
    label: String,
    start: usize,
    end: usize,
}

#[derive(Default)]
struct CanonicalSourceNodeIndex {
    exact: HashMap<(String, SourceNodeKind, usize), Vec<String>>,
    ranged: HashMap<(String, SourceNodeKind), Vec<CanonicalNodeAnchor>>,
}

impl CanonicalSourceNodeIndex {
    fn from_graph(graph: &SourceGraph) -> Self {
        let mut index = Self::default();
        for node in &graph.nodes {
            let Some(range) = node.range.as_ref() else {
                continue;
            };
            let file = node.file.trim_start_matches('/').replace('\\', "/");
            index
                .exact
                .entry((file.clone(), node.kind.clone(), range.start))
                .or_default()
                .push(node.id.clone());
            index
                .ranged
                .entry((file, node.kind.clone()))
                .or_default()
                .push(CanonicalNodeAnchor {
                    id: node.id.clone(),
                    label: node.label.clone(),
                    start: range.start,
                    end: range.end,
                });
        }
        for anchors in index.ranged.values_mut() {
            anchors.sort_by_key(|anchor| (anchor.start, anchor.end));
        }
        index
    }

    fn exact_id(
        &self,
        file: &str,
        kind: &SourceNodeKind,
        start: usize,
    ) -> Result<Option<&str>, String> {
        let Some(ids) = self.exact.get(&(file.to_string(), kind.clone(), start)) else {
            return Ok(None);
        };
        match ids.as_slice() {
            [id] => Ok(Some(id.as_str())),
            _ => Err(format!(
                "SourceGraph conține mai multe noduri {:?} la aceeași poziție în {file}.",
                kind
            )),
        }
    }

    fn tera_id(
        &self,
        file: &str,
        kind: &SourceNodeKind,
        label: &str,
        start: usize,
        end: usize,
    ) -> Result<Option<&str>, String> {
        if let Some(id) = self.exact_id(file, kind, start)? {
            return Ok(Some(id));
        }
        let Some(anchors) = self.ranged.get(&(file.to_string(), kind.clone())) else {
            return Ok(None);
        };
        // A `for` may own a contiguous `set` prelude, so its canonical range
        // can begin before the opening tag. Select only a unique innermost
        // SourceGraph node which structurally contains this parser item.
        let mut candidates = anchors
            .iter()
            .filter(|anchor| anchor.label == label && anchor.start <= start && end <= anchor.end)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|anchor| (anchor.start, usize::MAX - anchor.end));
        let Some(candidate) = candidates.pop() else {
            return Ok(None);
        };
        if candidates
            .last()
            .is_some_and(|other| other.start == candidate.start && other.end == candidate.end)
        {
            return Err(format!(
                "SourceGraph nu poate rezolva univoc nodul Tera {:?} din {file}.",
                kind
            ));
        }
        Ok(Some(candidate.id.as_str()))
    }
}

impl SourceIdIndex {
    pub(crate) fn for_source_graph<'a>(
        graph: &SourceGraph,
        sources: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, String> {
        let mut index = Self::default();
        let canonical = CanonicalSourceNodeIndex::from_graph(graph);
        for projection in &graph.markdown_projections {
            if projection.kind == MarkdownProjectionKind::Shortcode {
                index.shortcode_projection_by_template.insert(
                    projection.template_file.trim_start_matches('/').to_string(),
                    projection.clone(),
                );
            } else if let Some(range) = projection.template_range.as_ref() {
                index.markdown_projection_by_location.insert(
                    format!(
                        "{}:{}:{}",
                        projection.template_file.trim_start_matches('/'),
                        range.line,
                        range.column
                    ),
                    projection.clone(),
                );
            }
        }
        for (relative_path, source) in sources {
            if is_template_relative_path(relative_path)
                && matches!(
                    Path::new(relative_path)
                        .extension()
                        .and_then(|extension| extension.to_str()),
                    Some("html" | "md")
                )
            {
                index.index_template_source_from_graph(source, relative_path, &canonical)?;
            }
        }
        Ok(index)
    }

    #[cfg(test)]
    pub fn for_template_source(relative_path: &str, source: &str) -> Self {
        let mut index = Self::default();
        index.index_template_source(source, relative_path);
        index
    }

    pub fn source_id_for(&self, source_location: &str) -> Option<&str> {
        self.by_source_location
            .get(source_location)
            .map(String::as_str)
    }

    pub fn template_source_id_for(&self, source_location: &str) -> Option<&str> {
        self.by_template_source_location
            .get(source_location)
            .map(String::as_str)
    }

    pub fn template_source_id_for_html(&self, source_location: &str) -> Option<&str> {
        self.template_source_by_html_location
            .get(source_location)
            .map(String::as_str)
    }

    pub(super) fn scope_start_marker_for(&self, source_location: &str) -> Option<&str> {
        self.scope_start_marker_by_location
            .get(source_location)
            .map(String::as_str)
    }

    pub(super) fn has_external_scope_start(&self, source_location: &str) -> bool {
        self.external_scope_start_by_scope_location
            .contains(source_location)
    }

    pub(super) fn zola_image_for(&self, source_location: &str) -> Option<&ZolaImagePresentation> {
        self.zola_image_by_source_location.get(source_location)
    }

    pub(super) fn markdown_projection_for(
        &self,
        source_location: &str,
    ) -> Option<&MarkdownProjection> {
        self.markdown_projection_by_location.get(source_location)
    }

    pub(super) fn shortcode_projection_for(
        &self,
        relative_path: &str,
    ) -> Option<&MarkdownProjection> {
        self.shortcode_projection_by_template
            .get(relative_path.trim_start_matches('/'))
    }

    #[cfg(test)]
    pub(super) fn index_template_source(&mut self, source: &str, relative_path: &str) {
        let graph_file = relative_path.trim_start_matches('/').to_string();
        let markdown =
            crate::source_graph::markdown::analyze_template_markdown(&graph_file, source);
        self.markdown_projection_by_location.extend(
            markdown
                .projection_by_location
                .into_iter()
                .filter(|(_, projection)| projection.kind != MarkdownProjectionKind::Shortcode),
        );
        if let Some(shortcode) = markdown.shortcode_projection {
            self.shortcode_projection_by_template
                .insert(graph_file.clone(), shortcode);
        }
        let mut identities = ProvisionalSourceNodeIdAllocator::default();
        let line_index = LineIndex::new(source);
        let tera_scopes =
            self.index_tera_source(source, relative_path, &line_index, &mut identities);
        let mixed = parse_mixed_cst(source, relative_path);
        debug_assert!(mixed.is_lossless());
        for element in &mixed.elements {
            let Some(opening) = mixed.nodes.get(element.opening_node) else {
                continue;
            };
            let MixedCstKind::StartTag(tag) = &opening.kind else {
                continue;
            };
            if !should_project_html_tag(&tag.name) {
                continue;
            }
            let (line, column) = line_index.line_column(source, opening.start);
            let source_location = format!("{}:{}:{}", relative_path, line, column);
            let source_id = identities.next();
            self.by_source_location.insert(source_location, source_id);
            if tag.name.eq_ignore_ascii_case("img") {
                if let Ok(Some(presentation)) = inspect_zola_image_at(source, opening.start) {
                    self.zola_image_by_source_location.insert(
                        format!("{}:{}:{}", relative_path, line, column),
                        presentation,
                    );
                }
            }
            if let Some(scope) = innermost_tera_scope(&tera_scopes, opening.start, opening.end) {
                self.template_source_by_html_location.insert(
                    format!("{}:{}:{}", relative_path, line, column),
                    scope.node_id.clone(),
                );
            }
        }
    }

    fn index_template_source_from_graph(
        &mut self,
        source: &str,
        relative_path: &str,
        canonical: &CanonicalSourceNodeIndex,
    ) -> Result<(), String> {
        let graph_file = relative_path.trim_start_matches('/').replace('\\', "/");
        let line_index = LineIndex::new(source);
        let tera_scopes = self.index_tera_source_from_graph(
            source,
            relative_path,
            &graph_file,
            &line_index,
            canonical,
        )?;
        let mixed = parse_mixed_cst(source, relative_path);
        debug_assert!(mixed.is_lossless());
        for element in &mixed.elements {
            let Some(opening) = mixed.nodes.get(element.opening_node) else {
                continue;
            };
            let MixedCstKind::StartTag(tag) = &opening.kind else {
                continue;
            };
            if !should_project_html_tag(&tag.name) {
                continue;
            }
            let Some(source_id) =
                canonical.exact_id(&graph_file, &SourceNodeKind::Html, opening.start)?
            else {
                // Some generated descendants (for example managed icon SVG)
                // are deliberately absent from SourceGraph and stay inert.
                continue;
            };
            let (line, column) = line_index.line_column(source, opening.start);
            let source_location = format!("{}:{}:{}", relative_path, line, column);
            self.by_source_location
                .insert(source_location.clone(), source_id.to_string());
            if tag.name.eq_ignore_ascii_case("img") {
                if let Ok(Some(presentation)) = inspect_zola_image_at(source, opening.start) {
                    self.zola_image_by_source_location
                        .insert(source_location.clone(), presentation);
                }
            }
            if let Some(scope) = innermost_tera_scope(&tera_scopes, opening.start, opening.end) {
                self.template_source_by_html_location
                    .insert(source_location, scope.node_id.clone());
            }
        }
        Ok(())
    }

    fn index_tera_source_from_graph(
        &mut self,
        source: &str,
        relative_path: &str,
        graph_file: &str,
        line_index: &LineIndex,
        canonical: &CanonicalSourceNodeIndex,
    ) -> Result<Vec<TeraScopeAnchor>, String> {
        let is_partial = is_partial_template_relative_path(relative_path);
        let mut scope_stack: Vec<TeraScopeAnchor> = Vec::new();
        let mut completed_scopes: Vec<TeraScopeAnchor> = Vec::new();
        let mut set_preludes: Vec<SetPreludeAnchor> = Vec::new();

        for item in parse_tera_items(source) {
            match item.kind {
                TeraItemKind::EndScope => {
                    if let Some(mut scope) = scope_stack.pop() {
                        scope.end = item.end;
                        completed_scopes.push(scope);
                    }
                }
                TeraItemKind::Node => {
                    let Some(kind) = item.node_kind.clone() else {
                        continue;
                    };
                    if is_partial && matches!(kind, SourceNodeKind::Block | SourceNodeKind::Extends)
                    {
                        continue;
                    }
                    let Some(node_id) =
                        canonical.tera_id(graph_file, &kind, &item.label, item.start, item.end)?
                    else {
                        return Err(format!(
                            "Preview-ul nu a găsit identitatea canonică {:?} din {} la byte {}.",
                            kind, graph_file, item.start
                        ));
                    };
                    let node_id = node_id.to_string();
                    let (line, column) = line_index.line_column(source, item.start);
                    let source_location = format!("{}:{}:{}", relative_path, line, column);
                    self.by_template_source_location
                        .insert(source_location.clone(), node_id.clone());
                    let parent = scope_stack.last().map(|scope| scope.node_id.clone());
                    if matches!(kind, SourceNodeKind::Set | SourceNodeKind::SetGlobal) {
                        if let Some(variable) = set_assignment_name(&item.label) {
                            set_preludes.push(SetPreludeAnchor {
                                variable,
                                start: item.start,
                                location: source_location.clone(),
                                parent: parent.clone(),
                            });
                        }
                    }
                    if item.opens_scope() {
                        let prelude = if kind == SourceNodeKind::For {
                            take_loop_prelude_for(&item.label, parent.as_ref(), &mut set_preludes)
                        } else {
                            None
                        };
                        if let Some(prelude) = prelude.as_ref() {
                            self.scope_start_marker_by_location
                                .insert(prelude.location.clone(), node_id.clone());
                            self.external_scope_start_by_scope_location
                                .insert(source_location.clone());
                        }
                        scope_stack.push(TeraScopeAnchor {
                            node_id,
                            start: prelude.map(|prelude| prelude.start).unwrap_or(item.start),
                            end: source.len(),
                        });
                    }
                }
            }
        }
        completed_scopes.extend(scope_stack);
        Ok(completed_scopes)
    }

    #[cfg(test)]
    fn index_tera_source(
        &mut self,
        source: &str,
        relative_path: &str,
        line_index: &LineIndex,
        identities: &mut ProvisionalSourceNodeIdAllocator,
    ) -> Vec<TeraScopeAnchor> {
        let is_partial = is_partial_template_relative_path(relative_path);
        let mut scope_stack: Vec<TeraScopeAnchor> = Vec::new();
        let mut completed_scopes: Vec<TeraScopeAnchor> = Vec::new();
        let mut set_preludes: Vec<SetPreludeAnchor> = Vec::new();

        for item in parse_tera_items(source) {
            match item.kind {
                TeraItemKind::EndScope => {
                    if let Some(mut scope) = scope_stack.pop() {
                        scope.end = item.end;
                        completed_scopes.push(scope);
                    }
                }
                TeraItemKind::Node => {
                    let Some(kind) = item.node_kind.clone() else {
                        continue;
                    };
                    if is_partial && matches!(kind, SourceNodeKind::Block | SourceNodeKind::Extends)
                    {
                        continue;
                    }
                    let node_id = identities.next();
                    let (line, column) = line_index.line_column(source, item.start);
                    let source_location = format!("{}:{}:{}", relative_path, line, column);
                    self.by_template_source_location
                        .insert(source_location.clone(), node_id.clone());
                    let parent = scope_stack.last().map(|scope| scope.node_id.clone());
                    if matches!(kind, SourceNodeKind::Set | SourceNodeKind::SetGlobal) {
                        if let Some(variable) = set_assignment_name(&item.label) {
                            set_preludes.push(SetPreludeAnchor {
                                variable,
                                start: item.start,
                                location: source_location.clone(),
                                parent: parent.clone(),
                            });
                        }
                    }
                    if item.opens_scope() {
                        let prelude = if kind == SourceNodeKind::For {
                            take_loop_prelude_for(&item.label, parent.as_ref(), &mut set_preludes)
                        } else {
                            None
                        };
                        if let Some(prelude) = prelude.as_ref() {
                            self.scope_start_marker_by_location
                                .insert(prelude.location.clone(), node_id.clone());
                            self.external_scope_start_by_scope_location
                                .insert(source_location.clone());
                        }
                        scope_stack.push(TeraScopeAnchor {
                            node_id,
                            start: prelude.map(|prelude| prelude.start).unwrap_or(item.start),
                            end: source.len(),
                        });
                    }
                }
            }
        }

        completed_scopes.extend(scope_stack);
        completed_scopes
    }
}

fn take_loop_prelude_for(
    for_label: &str,
    parent: Option<&String>,
    set_preludes: &mut Vec<SetPreludeAnchor>,
) -> Option<SetPreludeAnchor> {
    let collection_root = for_collection_root(for_label)?;
    let index = set_preludes.iter().rev().position(|candidate| {
        candidate.variable == collection_root && candidate.parent.as_ref() == parent
    })?;
    Some(set_preludes.remove(set_preludes.len() - 1 - index))
}

fn is_partial_template_relative_path(relative_path: &str) -> bool {
    let normalized = relative_path.trim_start_matches('/').replace('\\', "/");
    let logical = if let Some(after_themes) = normalized.strip_prefix("themes/") {
        after_themes
            .split_once("/templates/")
            .map(|(_theme, template_path)| template_path)
            .unwrap_or(normalized.as_str())
    } else {
        normalized
            .strip_prefix("templates/")
            .unwrap_or(normalized.as_str())
    };

    logical.starts_with("partials/")
        || logical.starts_with("macros/")
        || logical.starts_with("shortcodes/")
}

fn innermost_tera_scope(
    scopes: &[TeraScopeAnchor],
    start: usize,
    end: usize,
) -> Option<&TeraScopeAnchor> {
    scopes
        .iter()
        .filter(|scope| scope.start <= start && end <= scope.end)
        .max_by_key(|scope| (scope.start, usize::MAX - scope.end))
}

#[cfg(test)]
mod canonical_tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        project_model::test_support::ProjectModelTestFixture, source_graph::model::SourceNodeKind,
    };

    use super::SourceIdIndex;

    #[test]
    fn preview_index_uses_exact_source_graph_ids_for_identical_siblings() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pana-preview-canonical-source-ids-{}-{nonce}",
            std::process::id()
        ));
        let source = "<main>\n<section><span></span></section>\n<section><span></span></section>\n<section><span></span></section>\n</main>";
        let fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let graph = fixture.build_source_graph().unwrap();
        let index =
            SourceIdIndex::for_source_graph(&graph, [("templates/index.html", source)]).unwrap();

        let sections = graph
            .nodes
            .iter()
            .filter(|node| {
                node.file == "templates/index.html"
                    && node.kind == SourceNodeKind::Html
                    && node.label.starts_with("<section")
            })
            .collect::<Vec<_>>();
        assert_eq!(sections.len(), 3);
        for section in sections {
            let range = section.range.as_ref().unwrap();
            let location = format!("templates/index.html:{}:{}", range.line, range.column);
            assert_eq!(index.source_id_for(&location), Some(section.id.as_str()));
            assert!(section.id.starts_with("sgn_"));
        }
        fs::remove_dir_all(root).unwrap();
    }
}
