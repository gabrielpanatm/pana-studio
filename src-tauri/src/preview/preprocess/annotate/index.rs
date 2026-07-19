use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use crate::{
    preview::preprocess::annotate::{
        paths::{is_template_relative_path, read_active_theme, zola_relative_path},
        range::line_column,
    },
    source_graph::{
        html::parse_html_opening_tags,
        identity::source_node_id,
        model::SourceNodeKind,
        tera::{for_collection_root, parse_tera_items, set_assignment_name, TeraItemKind},
    },
};

#[derive(Default)]
pub struct SourceIdIndex {
    pub(super) by_source_location: HashMap<String, String>,
    pub(super) by_template_source_location: HashMap<String, String>,
    pub(super) template_source_by_html_location: HashMap<String, String>,
    pub(super) scope_start_marker_by_location: HashMap<String, String>,
    pub(super) external_scope_start_by_scope_location: HashSet<String>,
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

impl SourceIdIndex {
    pub fn for_zola_root(zola_root: &Path) -> Result<Self, String> {
        let mut index = Self::default();
        let templates_root = zola_root.join("templates");
        if templates_root.is_dir() {
            index.collect_templates(zola_root, &templates_root)?;
        }
        if let Some(theme) = read_active_theme(zola_root) {
            let theme_templates_root = zola_root.join("themes").join(&theme).join("templates");
            if theme_templates_root.is_dir() {
                index.collect_templates(zola_root, &theme_templates_root)?;
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

    fn collect_templates(&mut self, zola_root: &Path, current: &Path) -> Result<(), String> {
        for entry in fs::read_dir(current).map_err(|error| {
            format!(
                "Nu am putut citi template-urile pentru Source Graph {}: {}",
                current.to_string_lossy(),
                error
            )
        })? {
            let entry = entry.map_err(|error| format!("Nu am putut citi o intrare: {}", error))?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_templates(zola_root, &path)?;
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("html") {
                self.index_template_file(zola_root, &path)?;
            }
        }
        Ok(())
    }

    fn index_template_file(&mut self, zola_root: &Path, path: &Path) -> Result<(), String> {
        let relative_path = zola_relative_path(zola_root, path);
        if !is_template_relative_path(&relative_path) {
            return Ok(());
        }

        let source = fs::read_to_string(path)
            .map_err(|error| format!("Nu am putut citi {}: {}", relative_path, error))?;
        self.index_template_source(&source, &relative_path);
        Ok(())
    }

    pub(super) fn index_template_source(&mut self, source: &str, relative_path: &str) {
        let graph_file = format!("sursa/{}", relative_path.trim_start_matches('/'));
        let tera_scopes = self.index_tera_source(source, relative_path, &graph_file);
        for item in parse_html_opening_tags(source) {
            let (line, column) = line_column(source, item.start);
            let source_location = format!("{}:{}:{}", relative_path, line, column);
            let source_id = source_node_id(
                &graph_file,
                &SourceNodeKind::Html,
                &item.label,
                Some(item.start),
                Some(item.end),
            );
            self.by_source_location.insert(source_location, source_id);
            if let Some(scope) = innermost_tera_scope(&tera_scopes, item.start, item.end) {
                self.template_source_by_html_location.insert(
                    format!("{}:{}:{}", relative_path, line, column),
                    scope.node_id.clone(),
                );
            }
        }
    }

    fn index_tera_source(
        &mut self,
        source: &str,
        relative_path: &str,
        graph_file: &str,
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
                    let node_id = source_node_id(
                        graph_file,
                        &kind,
                        &item.label,
                        Some(item.start),
                        Some(item.end),
                    );
                    let (line, column) = line_column(source, item.start);
                    let source_location = format!("{}:{}:{}", relative_path, line, column);
                    self.by_template_source_location
                        .insert(source_location.clone(), node_id.clone());
                    let parent = scope_stack.last().map(|scope| scope.node_id.clone());
                    if kind == SourceNodeKind::Set {
                        if let Some(variable) = set_assignment_name(&item.label) {
                            set_preludes.push(SetPreludeAnchor {
                                variable,
                                start: item.start,
                                location: source_location.clone(),
                                parent: parent.clone(),
                            });
                        }
                    }
                    if tera_item_opens_scope(&kind) {
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

    logical.starts_with("partials/") || logical.starts_with("macros/")
}

fn tera_item_opens_scope(kind: &SourceNodeKind) -> bool {
    matches!(
        kind,
        SourceNodeKind::Block
            | SourceNodeKind::Macro
            | SourceNodeKind::For
            | SourceNodeKind::If
            | SourceNodeKind::With
    )
}

fn innermost_tera_scope<'a>(
    scopes: &'a [TeraScopeAnchor],
    start: usize,
    end: usize,
) -> Option<&'a TeraScopeAnchor> {
    scopes
        .iter()
        .filter(|scope| scope.start <= start && end <= scope.end)
        .max_by_key(|scope| (scope.start, usize::MAX - scope.end))
}
