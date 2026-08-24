use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

use crate::source_graph::model::{SourceDataNode, SourceGraph, SourceNodeKind, SourceRelationKind};

static NEXT_RUNTIME_SOURCE_NODE_ID: AtomicU64 = AtomicU64::new(1);

/// Parser-local keys connect records assembled during one scan. They carry no
/// source meaning and are replaced before SourceGraph is published.
#[derive(Default)]
pub(crate) struct ProvisionalSourceNodeIdAllocator {
    next: u64,
}

impl ProvisionalSourceNodeIdAllocator {
    pub(crate) fn next(&mut self) -> String {
        self.next = self.next.saturating_add(1);
        format!("sg_p{:016x}", self.next)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceTextEdit {
    pub(crate) old_start: usize,
    pub(crate) old_end: usize,
    pub(crate) new_start: usize,
    pub(crate) new_end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceNodeLifecycle {
    Preserved {
        source_node_id: String,
    },
    Moved {
        source_node_id: String,
    },
    Inserted {
        source_node_id: String,
    },
    Duplicated {
        source_node_id: String,
        duplicated_from_source_node_id: String,
    },
    Deleted {
        source_node_id: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceTreeMovePosition {
    Before,
    After,
    Inside,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceTreeMove {
    pub(crate) source_node_id: String,
    pub(crate) target_node_id: String,
    pub(crate) position: SourceTreeMovePosition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceTreeInsert {
    pub(crate) target_node_id: String,
    pub(crate) position: SourceTreeMovePosition,
    pub(crate) inside_child_index: Option<usize>,
    pub(crate) inserted_start: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceTreeDuplicate {
    pub(crate) source_node_id: String,
    pub(crate) inserted_start: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceTreeDelete {
    pub(crate) source_node_ids: Vec<String>,
}

/// Compact, serializable identity of one SourceGraph subtree. History retains
/// this only for a tree that disappears and can later be restored by Undo/Redo.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub(crate) struct SourceTreeIdentityNode {
    pub(crate) source_node_id: String,
    pub(crate) kind: SourceNodeKind,
    pub(crate) child_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub(crate) struct SourceTreeIdentity {
    pub(crate) file: String,
    pub(crate) parent_source_node_id: String,
    pub(crate) sibling_index: usize,
    pub(crate) root_count: usize,
    pub(crate) nodes: Vec<SourceTreeIdentityNode>,
}

impl SourceTreeIdentity {
    pub(crate) fn root_source_node_ids(&self) -> Result<Vec<String>, String> {
        if self.root_count == 0 || self.nodes.is_empty() {
            return Err("History a primit o pădure SourceGraph fără rădăcini.".to_string());
        }
        let mut cursor = 0usize;
        let mut roots = Vec::with_capacity(self.root_count);
        for _ in 0..self.root_count {
            roots.push(
                self.nodes
                    .get(cursor)
                    .ok_or_else(|| "History a primit o pădure SourceGraph trunchiată.".to_string())?
                    .source_node_id
                    .clone(),
            );
            skip_source_tree_identity(&self.nodes, &mut cursor)?;
        }
        if cursor != self.nodes.len() {
            return Err("History a primit noduri în afara rădăcinilor declarate.".to_string());
        }
        Ok(roots)
    }
}

/// Exact transition between two immutable source revisions. Positions are
/// expressed in the before/after documents; SourceNode identity never depends
/// on labels, classes, selectors or sibling occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceChangeSet {
    pub(crate) file: String,
    pub(crate) base_revision: String,
    pub(crate) result_revision: String,
    pub(crate) edits: Vec<SourceTextEdit>,
    pub(crate) lifecycle: Vec<SourceNodeLifecycle>,
    pub(crate) tree_insert: Option<SourceTreeInsert>,
    pub(crate) tree_moves: Vec<SourceTreeMove>,
    pub(crate) tree_duplicates: Vec<SourceTreeDuplicate>,
    pub(crate) tree_delete: Option<SourceTreeDelete>,
    pub(crate) tree_restores: Vec<SourceTreeIdentity>,
}

impl SourceChangeSet {
    pub(crate) fn between(file: impl Into<String>, before: &str, after: &str) -> Self {
        let file = file.into();
        if before == after {
            return Self {
                file,
                base_revision: source_text_revision(before),
                result_revision: source_text_revision(after),
                edits: Vec::new(),
                lifecycle: Vec::new(),
                tree_insert: None,
                tree_moves: Vec::new(),
                tree_duplicates: Vec::new(),
                tree_delete: None,
                tree_restores: Vec::new(),
            };
        }
        let mut prefix = common_prefix_len(before.as_bytes(), after.as_bytes());
        while prefix > 0 && (!before.is_char_boundary(prefix) || !after.is_char_boundary(prefix)) {
            prefix -= 1;
        }
        let mut suffix =
            common_suffix_len(&before.as_bytes()[prefix..], &after.as_bytes()[prefix..]);
        while suffix > 0
            && (!before.is_char_boundary(before.len() - suffix)
                || !after.is_char_boundary(after.len() - suffix))
        {
            suffix -= 1;
        }
        Self {
            file,
            base_revision: source_text_revision(before),
            result_revision: source_text_revision(after),
            edits: vec![SourceTextEdit {
                old_start: prefix,
                old_end: before.len().saturating_sub(suffix),
                new_start: prefix,
                new_end: after.len().saturating_sub(suffix),
            }],
            lifecycle: Vec::new(),
            tree_insert: None,
            tree_moves: Vec::new(),
            tree_duplicates: Vec::new(),
            tree_delete: None,
            tree_restores: Vec::new(),
        }
    }

    pub(crate) fn with_tree_insert(
        mut self,
        target_node_id: impl Into<String>,
        position: SourceTreeMovePosition,
        inside_child_index: Option<usize>,
        inserted_start: Option<usize>,
    ) -> Self {
        self.tree_insert = Some(SourceTreeInsert {
            target_node_id: target_node_id.into(),
            position,
            inside_child_index,
            inserted_start,
        });
        self
    }

    pub(crate) fn with_tree_move(
        mut self,
        source_node_id: impl Into<String>,
        target_node_id: impl Into<String>,
        position: SourceTreeMovePosition,
    ) -> Self {
        self.tree_moves.push(SourceTreeMove {
            source_node_id: source_node_id.into(),
            target_node_id: target_node_id.into(),
            position,
        });
        self
    }

    pub(crate) fn with_tree_duplicate(
        mut self,
        source_node_id: impl Into<String>,
        inserted_start: usize,
    ) -> Self {
        self.tree_duplicates.push(SourceTreeDuplicate {
            source_node_id: source_node_id.into(),
            inserted_start,
        });
        self
    }

    pub(crate) fn with_tree_delete(mut self, source_node_id: impl Into<String>) -> Self {
        self.tree_delete = Some(SourceTreeDelete {
            source_node_ids: vec![source_node_id.into()],
        });
        self
    }

    pub(crate) fn with_tree_delete_many(mut self, source_node_ids: Vec<String>) -> Self {
        self.tree_delete
            .get_or_insert_with(|| SourceTreeDelete {
                source_node_ids: Vec::new(),
            })
            .source_node_ids
            .extend(source_node_ids);
        self
    }

    pub(crate) fn with_tree_restore(mut self, tree: SourceTreeIdentity) -> Self {
        self.tree_restores.push(tree);
        self
    }

    pub(crate) fn with_exact_text_edits(mut self, edits: Vec<SourceTextEdit>) -> Self {
        self.edits = edits;
        self
    }

    /// Proves that this transition belongs to the exact immutable source pair
    /// it is about to reconcile. This is deliberately stronger than checking
    /// lengths: the untouched spans must also be byte-for-byte identical.
    pub(crate) fn require_sources(&self, before: &str, after: &str) -> Result<(), String> {
        let actual_base_revision = source_text_revision(before);
        let actual_result_revision = source_text_revision(after);
        if self.base_revision != actual_base_revision {
            return Err(format!(
                "SourceChangeSet a refuzat revizia de bază stale pentru {} (așteptată {}, actuală {}).",
                self.file, self.base_revision, actual_base_revision
            ));
        }
        if self.result_revision != actual_result_revision {
            return Err(format!(
                "SourceChangeSet a refuzat revizia rezultatului stale pentru {} (așteptată {}, actuală {}).",
                self.file, self.result_revision, actual_result_revision
            ));
        }

        let mut old_cursor = 0usize;
        let mut new_cursor = 0usize;
        for edit in &self.edits {
            if edit.old_start < old_cursor
                || edit.old_start > edit.old_end
                || edit.old_end > before.len()
                || edit.new_start < new_cursor
                || edit.new_start > edit.new_end
                || edit.new_end > after.len()
                || !before.is_char_boundary(edit.old_start)
                || !before.is_char_boundary(edit.old_end)
                || !after.is_char_boundary(edit.new_start)
                || !after.is_char_boundary(edit.new_end)
            {
                return Err(format!(
                    "SourceChangeSet a refuzat maparea textuală invalidă pentru {}.",
                    self.file
                ));
            }
            if edit.new_start - new_cursor != edit.old_start - old_cursor
                || before[old_cursor..edit.old_start] != after[new_cursor..edit.new_start]
            {
                return Err(format!(
                    "SourceChangeSet a refuzat o mapare care nu păstrează regiunile neatinse din {}.",
                    self.file
                ));
            }
            old_cursor = edit.old_end;
            new_cursor = edit.new_end;
        }
        if before[old_cursor..] != after[new_cursor..] {
            return Err(format!(
                "SourceChangeSet a refuzat coada divergentă a sursei {}.",
                self.file
            ));
        }
        Ok(())
    }

    fn map_old_position(&self, position: usize) -> Option<usize> {
        let mut mapped = position as i128;
        for edit in &self.edits {
            if position < edit.old_start {
                continue;
            }
            if edit.old_start == edit.old_end {
                mapped += edit.new_end as i128 - edit.new_start as i128;
                continue;
            }
            if position < edit.old_end {
                return None;
            }
            mapped += (edit.new_end as i128 - edit.new_start as i128)
                - (edit.old_end as i128 - edit.old_start as i128);
        }
        usize::try_from(mapped).ok()
    }
}

fn source_text_revision(source: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    format!("source_{:016x}", hasher.finish())
}

pub(crate) fn capture_source_tree_identity(
    graph: &SourceGraph,
    root_source_node_id: &str,
) -> Result<SourceTreeIdentity, String> {
    capture_source_forest_identity(graph, &[root_source_node_id.to_string()])
}

pub(crate) fn capture_source_forest_identity(
    graph: &SourceGraph,
    root_source_node_ids: &[String],
) -> Result<SourceTreeIdentity, String> {
    if root_source_node_ids.is_empty() {
        return Err("SourceGraph nu poate reține o pădure History goală.".to_string());
    }
    let nodes_by_id = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let root = nodes_by_id
        .get(root_source_node_ids[0].as_str())
        .ok_or_else(|| {
            format!(
                "SourceGraph nu conține subarborele {} pentru History.",
                root_source_node_ids[0]
            )
        })?;
    let parent_source_node_id = root.parent.clone().ok_or_else(|| {
        format!(
            "SourceGraph nu poate reține pentru History rădăcina fără părinte {}.",
            root_source_node_ids[0]
        )
    })?;
    let parent = nodes_by_id
        .get(parent_source_node_id.as_str())
        .ok_or_else(|| {
            format!(
                "SourceGraph nu găsește părintele {parent_source_node_id} al subarborelui History."
            )
        })?;
    let sibling_index = parent
        .children
        .iter()
        .position(|child| child == &root_source_node_ids[0])
        .ok_or_else(|| {
            format!(
                "SourceGraph nu găsește {} în lista de copii History.",
                root_source_node_ids[0]
            )
        })?;
    if parent
        .children
        .get(sibling_index..sibling_index + root_source_node_ids.len())
        != Some(root_source_node_ids)
    {
        return Err(
            "SourceGraph poate reține numai rădăcini History contigue și ordonate.".to_string(),
        );
    }
    let mut nodes = Vec::new();
    let mut visiting = HashSet::new();
    for root_source_node_id in root_source_node_ids {
        let candidate = nodes_by_id
            .get(root_source_node_id.as_str())
            .ok_or_else(|| {
                format!("SourceGraph nu conține rădăcina History {root_source_node_id}.")
            })?;
        if candidate.file != root.file
            || candidate.parent.as_deref() != Some(parent_source_node_id.as_str())
        {
            return Err(
                "SourceGraph nu poate reține rădăcini History din părinți sau fișiere diferite."
                    .to_string(),
            );
        }
        collect_source_tree_identity(
            root_source_node_id,
            &root.file,
            &nodes_by_id,
            &mut visiting,
            &mut nodes,
        )?;
    }
    Ok(SourceTreeIdentity {
        file: root.file.clone(),
        parent_source_node_id,
        sibling_index,
        root_count: root_source_node_ids.len(),
        nodes,
    })
}

fn skip_source_tree_identity(
    identities: &[SourceTreeIdentityNode],
    cursor: &mut usize,
) -> Result<(), String> {
    let identity = identities
        .get(*cursor)
        .ok_or_else(|| "History a primit o identitate SourceGraph trunchiată.".to_string())?;
    *cursor += 1;
    for _ in 0..identity.child_count {
        skip_source_tree_identity(identities, cursor)?;
    }
    Ok(())
}

fn collect_source_tree_identity(
    source_node_id: &str,
    file: &str,
    nodes_by_id: &HashMap<&str, &crate::source_graph::model::SourceNode>,
    visiting: &mut HashSet<String>,
    output: &mut Vec<SourceTreeIdentityNode>,
) -> Result<(), String> {
    if !visiting.insert(source_node_id.to_string()) {
        return Err("SourceGraph conține un ciclu în subarborele reținut de History.".to_string());
    }
    let node = nodes_by_id.get(source_node_id).ok_or_else(|| {
        format!("SourceGraph a pierdut copilul {source_node_id} din subarborele History.")
    })?;
    if node.file != file {
        return Err("History nu poate reține un subarbore SourceGraph între fișiere.".to_string());
    }
    output.push(SourceTreeIdentityNode {
        source_node_id: node.id.clone(),
        kind: node.kind.clone(),
        child_count: node.children.len(),
    });
    for child in &node.children {
        collect_source_tree_identity(child, file, nodes_by_id, visiting, output)?;
    }
    visiting.remove(source_node_id);
    Ok(())
}

/// Reconciles a freshly parsed single-file fragment with the previous graph.
/// Every provisional parser ID is replaced either by the old logical ID or by
/// a new opaque runtime ID before any derived graph is built.
pub(crate) fn reconcile_fragment_source_node_ids(
    previous: &SourceGraph,
    fragment: &mut SourceGraph,
    change_set: &mut SourceChangeSet,
) -> Result<(), String> {
    let previous_nodes = previous
        .nodes
        .iter()
        .filter(|node| node.file == change_set.file)
        .collect::<Vec<_>>();
    let mut candidates = HashMap::<(SourceNodeKind, usize), Vec<usize>>::new();
    let mut rangeless_candidates = HashMap::<SourceNodeKind, Vec<usize>>::new();
    for (index, node) in fragment.nodes.iter().enumerate() {
        if node.file != change_set.file {
            continue;
        }
        if let Some(range) = node.range.as_ref() {
            candidates
                .entry((node.kind.clone(), range.start))
                .or_default()
                .push(index);
        } else {
            rangeless_candidates
                .entry(node.kind.clone())
                .or_default()
                .push(index);
        }
    }

    let mut claimed_new = HashSet::new();
    let mut provisional_to_final = HashMap::new();
    let mut preserved_old = HashSet::new();
    let previous_template = previous
        .templates
        .iter()
        .find(|template| template.file == change_set.file);
    let next_template = fragment
        .templates
        .iter()
        .find(|template| template.file == change_set.file);
    if let (Some(previous_template), Some(next_template)) = (previous_template, next_template) {
        provisional_to_final.insert(next_template.id.clone(), previous_template.id.clone());
        provisional_to_final.insert(
            next_template.node_id.clone(),
            previous_template.node_id.clone(),
        );
        if let Some(index) = fragment
            .nodes
            .iter()
            .position(|node| node.id == next_template.node_id)
        {
            claimed_new.insert(index);
        }
        preserved_old.insert(previous_template.node_id.clone());
        change_set.lifecycle.push(SourceNodeLifecycle::Preserved {
            source_node_id: previous_template.node_id.clone(),
        });
    }
    if let Some(tree_insert) = change_set.tree_insert.as_ref() {
        reconcile_source_tree_insert(
            previous,
            fragment,
            &change_set.file,
            tree_insert,
            &mut claimed_new,
            &mut preserved_old,
            &mut provisional_to_final,
            &mut change_set.lifecycle,
        )?;
    }
    for tree_move in &change_set.tree_moves {
        reconcile_source_tree_move(
            previous,
            fragment,
            &change_set.file,
            tree_move,
            &mut claimed_new,
            &mut preserved_old,
            &mut provisional_to_final,
            &mut change_set.lifecycle,
        )?;
    }
    for tree_duplicate in &change_set.tree_duplicates {
        reconcile_source_tree_duplicate(
            previous,
            fragment,
            &change_set.file,
            tree_duplicate,
            &mut claimed_new,
            &mut provisional_to_final,
            &mut change_set.lifecycle,
        )?;
    }
    for tree_restore in change_set.tree_restores.clone() {
        reconcile_source_tree_restore(
            previous,
            fragment,
            &change_set.file,
            &tree_restore,
            &mut claimed_new,
            &mut preserved_old,
            &mut provisional_to_final,
            &mut change_set.lifecycle,
        )?;
    }
    if let Some(tree_delete) = change_set.tree_delete.as_ref() {
        reconcile_source_tree_delete(
            previous,
            fragment,
            &change_set.file,
            tree_delete,
            &mut claimed_new,
            &mut preserved_old,
            &mut provisional_to_final,
            &mut change_set.lifecycle,
        )?;
    }
    for old in &previous_nodes {
        if preserved_old.contains(&old.id) {
            continue;
        }
        let indexes = if let Some(old_start) = old.range.as_ref().map(|range| range.start) {
            let Some(mapped_start) = change_set.map_old_position(old_start) else {
                continue;
            };
            candidates.get(&(old.kind.clone(), mapped_start))
        } else {
            rangeless_candidates.get(&old.kind)
        };
        let Some(indexes) = indexes else { continue };
        let available = indexes
            .iter()
            .copied()
            .filter(|index| !claimed_new.contains(index))
            .collect::<Vec<_>>();
        let [new_index] = available.as_slice() else {
            continue;
        };
        claimed_new.insert(*new_index);
        preserved_old.insert(old.id.clone());
        provisional_to_final.insert(fragment.nodes[*new_index].id.clone(), old.id.clone());
        change_set.lifecycle.push(SourceNodeLifecycle::Preserved {
            source_node_id: old.id.clone(),
        });
    }

    for (index, node) in fragment.nodes.iter().enumerate() {
        if provisional_to_final.contains_key(&node.id) {
            continue;
        }
        let id = next_runtime_source_node_id();
        if node.file == change_set.file {
            change_set.lifecycle.push(SourceNodeLifecycle::Inserted {
                source_node_id: id.clone(),
            });
        }
        provisional_to_final.insert(node.id.clone(), id);
        claimed_new.insert(index);
    }
    for old in previous_nodes {
        if !preserved_old.contains(&old.id) {
            change_set.lifecycle.push(SourceNodeLifecycle::Deleted {
                source_node_id: old.id.clone(),
            });
        }
    }

    remap_source_graph_base_ids(fragment, &provisional_to_final)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_tree_insert(
    previous: &SourceGraph,
    fragment: &SourceGraph,
    file: &str,
    tree_insert: &SourceTreeInsert,
    claimed_new: &mut HashSet<usize>,
    preserved_old: &mut HashSet<String>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    let previous_by_id = previous
        .nodes
        .iter()
        .filter(|node| node.file == file)
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let expected_kinds = previous_by_id
        .values()
        .map(|node| (node.id.clone(), node.kind.clone()))
        .collect::<HashMap<_, _>>();
    let expected_children = previous_by_id
        .values()
        .map(|node| {
            (
                node.id.clone(),
                node.children
                    .iter()
                    .filter(|child| previous_by_id.contains_key(child.as_str()))
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let fragment_by_id = fragment
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.file == file)
        .map(|(index, node)| (node.id.as_str(), (index, node)))
        .collect::<HashMap<_, _>>();
    let target = previous_by_id
        .get(tree_insert.target_node_id.as_str())
        .ok_or_else(|| {
            format!(
                "SourceChangeSet nu a găsit ținta inserării {}.",
                tree_insert.target_node_id
            )
        })?;
    let destination_parent = match tree_insert.position {
        SourceTreeMovePosition::Inside => tree_insert.target_node_id.clone(),
        SourceTreeMovePosition::Before | SourceTreeMovePosition::After => target
            .parent
            .as_deref()
            .filter(|parent| previous_by_id.contains_key(parent))
            .map(str::to_string)
            .ok_or_else(|| {
                "SourceChangeSet a refuzat inserarea lângă o rădăcină fără părinte structural."
                    .to_string()
            })?,
    };
    let destination_children = expected_children
        .get(&destination_parent)
        .ok_or_else(|| "SourceChangeSet nu a găsit copiii destinației inserării.".to_string())?;
    let insertion_index = match tree_insert.position {
        SourceTreeMovePosition::Inside => tree_insert.inside_child_index.ok_or_else(|| {
            "SourceChangeSet a refuzat inserarea Inside fără index structural exact.".to_string()
        })?,
        SourceTreeMovePosition::Before | SourceTreeMovePosition::After => {
            let target_index = destination_children
                .iter()
                .position(|candidate| candidate == &tree_insert.target_node_id)
                .ok_or_else(|| {
                    "SourceChangeSet nu a găsit ținta în lista de copii a destinației.".to_string()
                })?;
            target_index + usize::from(tree_insert.position == SourceTreeMovePosition::After)
        }
    };
    if insertion_index > destination_children.len() {
        return Err(
            "SourceChangeSet a refuzat indexul inserării în afara destinației.".to_string(),
        );
    }

    let previous_template_root = previous
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina veche pentru {file}."))?;
    let fragment_template_root = fragment
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina nouă pentru {file}."))?;
    let expected_roots = expected_children
        .get(previous_template_root)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let actual_roots = fragment_by_id
        .get(fragment_template_root)
        .map(|(_, root)| {
            root.children
                .iter()
                .filter(|child| fragment_by_id.contains_key(child.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        })
        .ok_or_else(|| format!("SourceChangeSet nu a găsit nodul rădăcină nou pentru {file}."))?;

    reconcile_source_insert_child_lists(
        previous_template_root,
        expected_roots,
        &actual_roots,
        &destination_parent,
        insertion_index,
        tree_insert.inserted_start,
        &expected_children,
        &expected_kinds,
        &fragment_by_id,
        claimed_new,
        preserved_old,
        provisional_to_final,
        lifecycle,
    )
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_insert_child_lists(
    old_parent_id: &str,
    expected: &[String],
    actual: &[String],
    destination_parent: &str,
    insertion_index: usize,
    inserted_start: Option<usize>,
    expected_children: &HashMap<String, Vec<String>>,
    expected_kinds: &HashMap<String, SourceNodeKind>,
    fragment_by_id: &HashMap<&str, (usize, &crate::source_graph::model::SourceNode)>,
    claimed_new: &mut HashSet<usize>,
    preserved_old: &mut HashSet<String>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    let actual_without_insert = if old_parent_id == destination_parent {
        if actual.len() <= expected.len() {
            return Err(format!(
                "SourceChangeSet a refuzat inserarea: destinația are {} copii vechi și {} copii noi.",
                expected.len(),
                actual.len()
            ));
        }
        let inserted_root_count = actual.len() - expected.len();
        let inserted_end = insertion_index.saturating_add(inserted_root_count);
        if insertion_index > expected.len() || inserted_end > actual.len() {
            return Err("SourceChangeSet a refuzat intervalul rădăcinilor inserate.".to_string());
        }
        let first_inserted_id = &actual[insertion_index];
        let (_, first_inserted) = fragment_by_id
            .get(first_inserted_id.as_str())
            .ok_or_else(|| "SourceChangeSet nu a găsit rădăcina inserată.".to_string())?;
        if inserted_start.is_some()
            && first_inserted.range.as_ref().map(|range| range.start) != inserted_start
        {
            return Err(
                "SourceChangeSet a refuzat rădăcina inserată la un offset divergent.".to_string(),
            );
        }
        for inserted_id in &actual[insertion_index..inserted_end] {
            claim_inserted_source_subtree(
                inserted_id,
                fragment_by_id,
                claimed_new,
                provisional_to_final,
                lifecycle,
            )?;
        }
        actual
            .iter()
            .enumerate()
            .filter(|(index, _)| *index < insertion_index || *index >= inserted_end)
            .map(|(_, id)| id.clone())
            .collect::<Vec<_>>()
    } else {
        if actual.len() != expected.len() {
            return Err(format!(
                "SourceChangeSet a refuzat inserarea: un părinte neatins are {} copii vechi și {} copii noi.",
                expected.len(),
                actual.len()
            ));
        }
        actual.to_vec()
    };

    for (old_id, new_id) in expected.iter().zip(&actual_without_insert) {
        let expected_kind = expected_kinds.get(old_id).ok_or_else(|| {
            format!("SourceChangeSet a pierdut kind-ul așteptat pentru {old_id}.")
        })?;
        let (new_index, new) = fragment_by_id
            .get(new_id.as_str())
            .ok_or_else(|| format!("SourceChangeSet a pierdut nodul nou {new_id}."))?;
        if expected_kind != &new.kind {
            return Err(format!(
                "SourceChangeSet a refuzat inserarea: kind divergent pentru {old_id}."
            ));
        }
        if !claimed_new.insert(*new_index) || !preserved_old.insert(old_id.clone()) {
            return Err(
                "SourceChangeSet a detectat o identitate revendicată de două ori la inserare."
                    .to_string(),
            );
        }
        provisional_to_final.insert(new.id.clone(), old_id.clone());
        lifecycle.push(SourceNodeLifecycle::Preserved {
            source_node_id: old_id.clone(),
        });
        let old_children = expected_children
            .get(old_id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let new_children = new
            .children
            .iter()
            .filter(|child| fragment_by_id.contains_key(child.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        reconcile_source_insert_child_lists(
            old_id,
            old_children,
            &new_children,
            destination_parent,
            insertion_index,
            inserted_start,
            expected_children,
            expected_kinds,
            fragment_by_id,
            claimed_new,
            preserved_old,
            provisional_to_final,
            lifecycle,
        )?;
    }
    Ok(())
}

fn claim_inserted_source_subtree(
    provisional_id: &str,
    fragment_by_id: &HashMap<&str, (usize, &crate::source_graph::model::SourceNode)>,
    claimed_new: &mut HashSet<usize>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    let (index, node) = fragment_by_id
        .get(provisional_id)
        .ok_or_else(|| format!("SourceChangeSet a pierdut nodul inserat {provisional_id}."))?;
    if !claimed_new.insert(*index) || provisional_to_final.contains_key(provisional_id) {
        return Err(
            "SourceChangeSet a detectat un nod inserat revendicat de două ori.".to_string(),
        );
    }
    let id = next_runtime_source_node_id();
    provisional_to_final.insert(node.id.clone(), id.clone());
    lifecycle.push(SourceNodeLifecycle::Inserted { source_node_id: id });
    for child in node
        .children
        .iter()
        .filter(|child| fragment_by_id.contains_key(child.as_str()))
    {
        claim_inserted_source_subtree(
            child,
            fragment_by_id,
            claimed_new,
            provisional_to_final,
            lifecycle,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_tree_move(
    previous: &SourceGraph,
    fragment: &SourceGraph,
    file: &str,
    tree_move: &SourceTreeMove,
    claimed_new: &mut HashSet<usize>,
    preserved_old: &mut HashSet<String>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    let previous_by_id = previous
        .nodes
        .iter()
        .filter(|node| node.file == file)
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let expected_kinds = previous_by_id
        .values()
        .map(|node| (node.id.clone(), node.kind.clone()))
        .collect::<HashMap<_, _>>();
    let fragment_by_id = fragment
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.file == file)
        .map(|(index, node)| (node.id.as_str(), (index, node)))
        .collect::<HashMap<_, _>>();
    let source = previous_by_id
        .get(tree_move.source_node_id.as_str())
        .ok_or_else(|| {
            format!(
                "SourceChangeSet nu a găsit sursa mutării {}.",
                tree_move.source_node_id
            )
        })?;
    let target = previous_by_id
        .get(tree_move.target_node_id.as_str())
        .ok_or_else(|| {
            format!(
                "SourceChangeSet nu a găsit ținta mutării {}.",
                tree_move.target_node_id
            )
        })?;

    let source_parent_for = |node: &crate::source_graph::model::SourceNode| {
        node.parent
            .as_deref()
            .filter(|parent| previous_by_id.contains_key(parent))
            .map(str::to_string)
    };
    let source_parent = source_parent_for(source);
    let target_parent = source_parent_for(target);
    let mut expected_children = HashMap::<Option<String>, Vec<String>>::new();
    for node in previous_by_id.values() {
        expected_children.insert(
            Some(node.id.clone()),
            node.children
                .iter()
                .filter(|child| previous_by_id.contains_key(child.as_str()))
                .cloned()
                .collect(),
        );
    }
    let source_siblings = expected_children
        .get_mut(&source_parent)
        .ok_or_else(|| "SourceChangeSet nu a găsit părintele sursei mutate.".to_string())?;
    let source_index = source_siblings
        .iter()
        .position(|candidate| candidate == &tree_move.source_node_id)
        .ok_or_else(|| "SourceChangeSet nu a găsit sursa în lista de copii.".to_string())?;
    source_siblings.remove(source_index);

    let destination_parent = match tree_move.position {
        SourceTreeMovePosition::Inside => Some(tree_move.target_node_id.clone()),
        SourceTreeMovePosition::Before | SourceTreeMovePosition::After => target_parent,
    };
    let destination = expected_children
        .get_mut(&destination_parent)
        .ok_or_else(|| "SourceChangeSet nu a găsit părintele destinației.".to_string())?;
    let insertion_index = match tree_move.position {
        SourceTreeMovePosition::Inside => destination.len(),
        SourceTreeMovePosition::Before | SourceTreeMovePosition::After => {
            let target_index = destination
                .iter()
                .position(|candidate| candidate == &tree_move.target_node_id)
                .ok_or_else(|| "SourceChangeSet nu a găsit ținta în lista de copii.".to_string())?;
            target_index + usize::from(tree_move.position == SourceTreeMovePosition::After)
        }
    };
    destination.insert(insertion_index, tree_move.source_node_id.clone());

    let previous_template_root = previous
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina veche pentru {file}."))?;
    let fragment_template_root = fragment
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina nouă pentru {file}."))?;
    let expected_roots = expected_children
        .get(&Some(previous_template_root.to_string()))
        .cloned()
        .unwrap_or_default();
    let fragment_roots = fragment_by_id
        .get(fragment_template_root)
        .map(|(_, root)| {
            root.children
                .iter()
                .filter(|child| fragment_by_id.contains_key(child.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        })
        .ok_or_else(|| format!("SourceChangeSet nu a găsit nodul rădăcină nou pentru {file}."))?;
    let moved_subtree = source_subtree_ids(previous, &tree_move.source_node_id);
    reconcile_source_child_lists(
        &expected_roots,
        &fragment_roots,
        &expected_children,
        &expected_kinds,
        &fragment_by_id,
        &moved_subtree,
        &HashSet::new(),
        claimed_new,
        preserved_old,
        provisional_to_final,
        lifecycle,
    )
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_child_lists(
    expected: &[String],
    actual: &[String],
    expected_children: &HashMap<Option<String>, Vec<String>>,
    expected_kinds: &HashMap<String, SourceNodeKind>,
    fragment_by_id: &HashMap<&str, (usize, &crate::source_graph::model::SourceNode)>,
    moved_subtree: &HashSet<String>,
    inserted_subtree: &HashSet<String>,
    claimed_new: &mut HashSet<usize>,
    preserved_old: &mut HashSet<String>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    if expected.len() != actual.len() {
        return Err(format!(
            "SourceChangeSet a refuzat mutarea: arborele sursă proiectat are {} copii, dar parserul a produs {}.",
            expected.len(),
            actual.len()
        ));
    }
    for (old_id, new_id) in expected.iter().zip(actual) {
        let expected_kind = expected_kinds.get(old_id).ok_or_else(|| {
            format!("SourceChangeSet a pierdut kind-ul așteptat pentru {old_id}.")
        })?;
        let (new_index, new) = fragment_by_id
            .get(new_id.as_str())
            .ok_or_else(|| format!("SourceChangeSet a pierdut nodul nou {new_id}."))?;
        if expected_kind != &new.kind {
            return Err(format!(
                "SourceChangeSet a refuzat tranziția structurală: kind divergent pentru {old_id}."
            ));
        }
        if !claimed_new.insert(*new_index) || !preserved_old.insert(old_id.clone()) {
            return Err(
                "SourceChangeSet a detectat o identitate revendicată de două ori.".to_string(),
            );
        }
        provisional_to_final.insert(new.id.clone(), old_id.clone());
        lifecycle.push(if inserted_subtree.contains(old_id) {
            SourceNodeLifecycle::Inserted {
                source_node_id: old_id.clone(),
            }
        } else if moved_subtree.contains(old_id) {
            SourceNodeLifecycle::Moved {
                source_node_id: old_id.clone(),
            }
        } else {
            SourceNodeLifecycle::Preserved {
                source_node_id: old_id.clone(),
            }
        });

        let old_children = expected_children
            .get(&Some(old_id.clone()))
            .map(Vec::as_slice)
            .unwrap_or_default();
        let new_children = new
            .children
            .iter()
            .filter(|child| fragment_by_id.contains_key(child.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        reconcile_source_child_lists(
            old_children,
            &new_children,
            expected_children,
            expected_kinds,
            fragment_by_id,
            moved_subtree,
            inserted_subtree,
            claimed_new,
            preserved_old,
            provisional_to_final,
            lifecycle,
        )?;
    }
    Ok(())
}

fn source_subtree_ids(graph: &SourceGraph, root: &str) -> HashSet<String> {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let mut pending = vec![root.to_string()];
    let mut subtree = HashSet::new();
    while let Some(id) = pending.pop() {
        if !subtree.insert(id.clone()) {
            continue;
        }
        if let Some(node) = nodes.get(id.as_str()) {
            pending.extend(node.children.iter().cloned());
        }
    }
    subtree
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_tree_delete(
    previous: &SourceGraph,
    next: &SourceGraph,
    file: &str,
    tree_delete: &SourceTreeDelete,
    claimed_new: &mut HashSet<usize>,
    preserved_old: &mut HashSet<String>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    let previous_by_id = previous
        .nodes
        .iter()
        .filter(|node| node.file == file)
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let expected_kinds = previous_by_id
        .values()
        .map(|node| (node.id.clone(), node.kind.clone()))
        .collect::<HashMap<_, _>>();
    let deleted_roots = tree_delete
        .source_node_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    if deleted_roots.is_empty()
        || deleted_roots.len() != tree_delete.source_node_ids.len()
        || deleted_roots
            .iter()
            .any(|source_node_id| !previous_by_id.contains_key(source_node_id))
    {
        return Err("SourceChangeSet nu a găsit toate rădăcinile șterse.".to_string());
    }
    let next_by_id = next
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.file == file)
        .map(|(index, node)| (node.id.as_str(), (index, node)))
        .collect::<HashMap<_, _>>();
    let deleted_subtree = tree_delete
        .source_node_ids
        .iter()
        .flat_map(|source_node_id| source_subtree_ids(previous, source_node_id))
        .collect::<HashSet<_>>();
    if deleted_subtree
        .iter()
        .any(|source_id| !previous_by_id.contains_key(source_id.as_str()))
    {
        return Err("SourceChangeSet a refuzat un subarbore șters între fișiere.".to_string());
    }

    let mut expected_children = HashMap::<Option<String>, Vec<String>>::new();
    for node in previous_by_id.values() {
        if deleted_subtree.contains(&node.id) {
            continue;
        }
        expected_children.insert(
            Some(node.id.clone()),
            node.children
                .iter()
                .filter(|child| {
                    previous_by_id.contains_key(child.as_str())
                        && !deleted_subtree.contains(child.as_str())
                })
                .cloned()
                .collect(),
        );
    }

    let previous_template_root = previous
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina veche pentru {file}."))?;
    if deleted_subtree.contains(previous_template_root) {
        return Err("SourceChangeSet nu poate șterge rădăcina template-ului.".to_string());
    }
    let next_template_root = next
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina nouă pentru {file}."))?;
    let expected_roots = expected_children
        .get(&Some(previous_template_root.to_string()))
        .cloned()
        .unwrap_or_default();
    let actual_roots = next_by_id
        .get(next_template_root)
        .map(|(_, root)| {
            root.children
                .iter()
                .filter(|child| next_by_id.contains_key(child.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        })
        .ok_or_else(|| format!("SourceChangeSet nu a găsit nodul rădăcină nou pentru {file}."))?;

    reconcile_source_child_lists(
        &expected_roots,
        &actual_roots,
        &expected_children,
        &expected_kinds,
        &next_by_id,
        &HashSet::new(),
        &HashSet::new(),
        claimed_new,
        preserved_old,
        provisional_to_final,
        lifecycle,
    )
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_tree_duplicate(
    previous: &SourceGraph,
    next: &SourceGraph,
    file: &str,
    duplicate: &SourceTreeDuplicate,
    claimed_new: &mut HashSet<usize>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    let previous_by_id = previous
        .nodes
        .iter()
        .filter(|node| node.file == file)
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let next_by_id = next
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.file == file)
        .map(|(index, node)| (node.id.as_str(), (index, node)))
        .collect::<HashMap<_, _>>();
    let source = previous_by_id
        .get(duplicate.source_node_id.as_str())
        .ok_or_else(|| {
            format!(
                "SourceChangeSet nu a găsit sursa duplicării {}.",
                duplicate.source_node_id
            )
        })?;
    let candidates = next
        .nodes
        .iter()
        .enumerate()
        .filter(|(index, node)| {
            node.file == file
                && node.kind == source.kind
                && node
                    .range
                    .as_ref()
                    .is_some_and(|range| range.start == duplicate.inserted_start)
                && !claimed_new.contains(index)
        })
        .collect::<Vec<_>>();
    let [(duplicate_index, duplicate_root)] = candidates.as_slice() else {
        return Err(format!(
            "SourceChangeSet a găsit {} rădăcini pentru duplicarea {} la byte {}.",
            candidates.len(),
            duplicate.source_node_id,
            duplicate.inserted_start
        ));
    };
    reconcile_duplicated_subtree(
        source,
        *duplicate_index,
        duplicate_root,
        &previous_by_id,
        &next_by_id,
        claimed_new,
        provisional_to_final,
        lifecycle,
    )
}

#[allow(clippy::too_many_arguments)]
fn reconcile_duplicated_subtree(
    source: &crate::source_graph::model::SourceNode,
    duplicate_index: usize,
    duplicate: &crate::source_graph::model::SourceNode,
    previous_by_id: &HashMap<&str, &crate::source_graph::model::SourceNode>,
    next_by_id: &HashMap<&str, (usize, &crate::source_graph::model::SourceNode)>,
    claimed_new: &mut HashSet<usize>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    if source.kind != duplicate.kind || !claimed_new.insert(duplicate_index) {
        return Err(format!(
            "SourceChangeSet a refuzat structura duplicată pentru {}.",
            source.id
        ));
    }
    let duplicate_id = next_runtime_source_node_id();
    provisional_to_final.insert(duplicate.id.clone(), duplicate_id.clone());
    lifecycle.push(SourceNodeLifecycle::Duplicated {
        source_node_id: duplicate_id,
        duplicated_from_source_node_id: source.id.clone(),
    });

    let source_children = source
        .children
        .iter()
        .filter_map(|id| previous_by_id.get(id.as_str()).copied())
        .collect::<Vec<_>>();
    let duplicate_children = duplicate
        .children
        .iter()
        .filter_map(|id| next_by_id.get(id.as_str()).copied())
        .collect::<Vec<_>>();
    if source_children.len() != duplicate_children.len() {
        return Err(format!(
            "SourceChangeSet a refuzat duplicarea {}: subarborele are structură divergentă.",
            source.id
        ));
    }
    for (source_child, (duplicate_child_index, duplicate_child)) in
        source_children.into_iter().zip(duplicate_children)
    {
        reconcile_duplicated_subtree(
            source_child,
            duplicate_child_index,
            duplicate_child,
            previous_by_id,
            next_by_id,
            claimed_new,
            provisional_to_final,
            lifecycle,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reconcile_source_tree_restore(
    previous: &SourceGraph,
    next: &SourceGraph,
    file: &str,
    restored: &SourceTreeIdentity,
    claimed_new: &mut HashSet<usize>,
    preserved_old: &mut HashSet<String>,
    provisional_to_final: &mut HashMap<String, String>,
    lifecycle: &mut Vec<SourceNodeLifecycle>,
) -> Result<(), String> {
    if restored.file != file || restored.root_count == 0 || restored.nodes.is_empty() {
        return Err("SourceChangeSet a primit un subarbore History invalid.".to_string());
    }
    let previous_by_id = previous
        .nodes
        .iter()
        .filter(|node| node.file == file)
        .map(|node| (node.id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let next_by_id = next
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.file == file)
        .map(|(index, node)| (node.id.as_str(), (index, node)))
        .collect::<HashMap<_, _>>();
    let desired_ids = restored
        .nodes
        .iter()
        .map(|node| node.source_node_id.as_str())
        .collect::<HashSet<_>>();
    if desired_ids.len() != restored.nodes.len()
        || desired_ids.iter().any(|desired| {
            provisional_to_final
                .values()
                .any(|assigned| assigned == *desired)
        })
    {
        return Err("SourceChangeSet a refuzat identități History duplicate.".to_string());
    }
    if desired_ids
        .iter()
        .any(|desired| previous_by_id.contains_key(*desired))
    {
        return Err(
            "SourceChangeSet a refuzat restaurarea peste o identitate încă activă.".to_string(),
        );
    }

    let mut expected_children = HashMap::<Option<String>, Vec<String>>::new();
    let mut expected_kinds = HashMap::<String, SourceNodeKind>::new();
    for node in previous_by_id.values() {
        expected_kinds.insert(node.id.clone(), node.kind.clone());
        expected_children.insert(
            Some(node.id.clone()),
            node.children
                .iter()
                .filter(|child| previous_by_id.contains_key(child.as_str()))
                .cloned()
                .collect(),
        );
    }

    if !previous_by_id.contains_key(restored.parent_source_node_id.as_str()) {
        return Err(format!(
            "SourceChangeSet nu a găsit părintele History {}.",
            restored.parent_source_node_id
        ));
    }
    let mut cursor = 0usize;
    let mut inserted_subtree = HashSet::new();
    let mut restored_roots = Vec::with_capacity(restored.root_count);
    for _ in 0..restored.root_count {
        restored_roots.push(append_restored_tree_expectation(
            &restored.nodes,
            &mut cursor,
            &mut expected_children,
            &mut expected_kinds,
            &mut inserted_subtree,
        )?);
    }
    if cursor != restored.nodes.len() {
        return Err("SourceChangeSet a refuzat o pădure History inconsistentă.".to_string());
    }
    let parent_children = expected_children
        .get_mut(&Some(restored.parent_source_node_id.clone()))
        .ok_or_else(|| {
            "SourceChangeSet a pierdut lista de copii a părintelui History.".to_string()
        })?;
    if restored.sibling_index > parent_children.len() {
        return Err(format!(
            "SourceChangeSet a refuzat indexul History {} pentru {} copii.",
            restored.sibling_index,
            parent_children.len()
        ));
    }
    parent_children.splice(
        restored.sibling_index..restored.sibling_index,
        restored_roots,
    );

    let previous_template_root = previous
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina veche pentru {file}."))?;
    let next_template_root = next
        .templates
        .iter()
        .find(|template| template.file == file)
        .map(|template| template.node_id.as_str())
        .ok_or_else(|| format!("SourceChangeSet nu a găsit rădăcina nouă pentru {file}."))?;
    let expected_roots = expected_children
        .get(&Some(previous_template_root.to_string()))
        .cloned()
        .unwrap_or_default();
    let actual_roots = next_by_id
        .get(next_template_root)
        .map(|(_, root)| {
            root.children
                .iter()
                .filter(|child| next_by_id.contains_key(child.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        })
        .ok_or_else(|| format!("SourceChangeSet nu a găsit nodul rădăcină nou pentru {file}."))?;
    reconcile_source_child_lists(
        &expected_roots,
        &actual_roots,
        &expected_children,
        &expected_kinds,
        &next_by_id,
        &HashSet::new(),
        &inserted_subtree,
        claimed_new,
        preserved_old,
        provisional_to_final,
        lifecycle,
    )
}

fn append_restored_tree_expectation(
    identities: &[SourceTreeIdentityNode],
    cursor: &mut usize,
    expected_children: &mut HashMap<Option<String>, Vec<String>>,
    expected_kinds: &mut HashMap<String, SourceNodeKind>,
    inserted_subtree: &mut HashSet<String>,
) -> Result<String, String> {
    let identity = identities
        .get(*cursor)
        .ok_or_else(|| "SourceChangeSet a primit un subarbore History trunchiat.".to_string())?;
    *cursor += 1;
    if !inserted_subtree.insert(identity.source_node_id.clone())
        || expected_kinds
            .insert(identity.source_node_id.clone(), identity.kind.clone())
            .is_some()
    {
        return Err("SourceChangeSet a primit identități duplicate în History.".to_string());
    }
    let mut children = Vec::with_capacity(identity.child_count);
    for _ in 0..identity.child_count {
        children.push(append_restored_tree_expectation(
            identities,
            cursor,
            expected_children,
            expected_kinds,
            inserted_subtree,
        )?);
    }
    expected_children.insert(Some(identity.source_node_id.clone()), children);
    Ok(identity.source_node_id.clone())
}

pub(crate) fn initialize_runtime_source_node_ids(
    graph: &mut SourceGraph,
    runtime_session_id: &str,
) -> Result<(), String> {
    let mut namespace_hasher = std::collections::hash_map::DefaultHasher::new();
    "pana-source-runtime-v1".hash(&mut namespace_hasher);
    runtime_session_id.hash(&mut namespace_hasher);
    let namespace = namespace_hasher.finish();
    let ids = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            (
                node.id.clone(),
                format!("sgn_s{namespace:016x}_{:016x}", index.saturating_add(1)),
            )
        })
        .collect::<HashMap<_, _>>();
    remap_source_graph_base_ids(graph, &ids)
}

pub(crate) fn reconcile_project_source_node_ids(
    previous: &SourceGraph,
    next: &mut SourceGraph,
    change_sets: &mut [SourceChangeSet],
) -> Result<(), String> {
    let change_indexes = change_sets
        .iter()
        .enumerate()
        .map(|(index, change)| (change.file.clone(), index))
        .collect::<HashMap<_, _>>();
    let next_files = next
        .nodes
        .iter()
        .map(|node| node.file.as_str())
        .collect::<HashSet<_>>();
    let mut ranged = HashMap::<(String, SourceNodeKind, usize), Vec<usize>>::new();
    let mut rangeless = HashMap::<(String, SourceNodeKind), Vec<usize>>::new();
    for (index, node) in next.nodes.iter().enumerate() {
        if let Some(range) = node.range.as_ref() {
            ranged
                .entry((node.file.clone(), node.kind.clone(), range.start))
                .or_default()
                .push(index);
        } else {
            rangeless
                .entry((node.file.clone(), node.kind.clone()))
                .or_default()
                .push(index);
        }
    }

    let mut claimed_new = HashSet::new();
    let mut preserved_old = HashSet::new();
    let mut ids = HashMap::new();
    for next_template in &next.templates {
        let Some(previous_template) = previous
            .templates
            .iter()
            .find(|template| template.file == next_template.file)
        else {
            continue;
        };
        ids.insert(next_template.id.clone(), previous_template.id.clone());
        ids.insert(
            next_template.node_id.clone(),
            previous_template.node_id.clone(),
        );
        if let Some(index) = next
            .nodes
            .iter()
            .position(|node| node.id == next_template.node_id)
        {
            claimed_new.insert(index);
        }
        preserved_old.insert(previous_template.node_id.clone());
        if let Some(change_index) = change_indexes.get(&next_template.file) {
            change_sets[*change_index]
                .lifecycle
                .push(SourceNodeLifecycle::Preserved {
                    source_node_id: previous_template.node_id.clone(),
                });
        }
    }

    for change_set in change_sets.iter_mut() {
        let Some(tree_insert) = change_set.tree_insert.clone() else {
            continue;
        };
        reconcile_source_tree_insert(
            previous,
            next,
            &change_set.file,
            &tree_insert,
            &mut claimed_new,
            &mut preserved_old,
            &mut ids,
            &mut change_set.lifecycle,
        )?;
    }
    for change_set in change_sets.iter_mut() {
        for tree_move in change_set.tree_moves.clone() {
            reconcile_source_tree_move(
                previous,
                next,
                &change_set.file,
                &tree_move,
                &mut claimed_new,
                &mut preserved_old,
                &mut ids,
                &mut change_set.lifecycle,
            )?;
        }
    }
    for change_set in change_sets.iter_mut() {
        for tree_restore in change_set.tree_restores.clone() {
            reconcile_source_tree_restore(
                previous,
                next,
                &change_set.file,
                &tree_restore,
                &mut claimed_new,
                &mut preserved_old,
                &mut ids,
                &mut change_set.lifecycle,
            )?;
        }
    }
    for change_set in change_sets.iter_mut() {
        for tree_duplicate in change_set.tree_duplicates.clone() {
            reconcile_source_tree_duplicate(
                previous,
                next,
                &change_set.file,
                &tree_duplicate,
                &mut claimed_new,
                &mut ids,
                &mut change_set.lifecycle,
            )?;
        }
    }
    for change_set in change_sets.iter_mut() {
        let Some(tree_delete) = change_set.tree_delete.clone() else {
            continue;
        };
        reconcile_source_tree_delete(
            previous,
            next,
            &change_set.file,
            &tree_delete,
            &mut claimed_new,
            &mut preserved_old,
            &mut ids,
            &mut change_set.lifecycle,
        )?;
    }

    for old in previous
        .nodes
        .iter()
        .filter(|node| next_files.contains(node.file.as_str()))
    {
        if preserved_old.contains(&old.id) {
            continue;
        }
        let candidates = if let Some(old_start) = old.range.as_ref().map(|range| range.start) {
            let mapped_start = if let Some(change_index) = change_indexes.get(&old.file) {
                let Some(mapped) = change_sets[*change_index].map_old_position(old_start) else {
                    continue;
                };
                mapped
            } else {
                old_start
            };
            ranged.get(&(old.file.clone(), old.kind.clone(), mapped_start))
        } else {
            rangeless.get(&(old.file.clone(), old.kind.clone()))
        };
        let Some(candidates) = candidates else {
            continue;
        };
        let available = candidates
            .iter()
            .copied()
            .filter(|index| !claimed_new.contains(index))
            .collect::<Vec<_>>();
        let [new_index] = available.as_slice() else {
            continue;
        };
        claimed_new.insert(*new_index);
        preserved_old.insert(old.id.clone());
        ids.insert(next.nodes[*new_index].id.clone(), old.id.clone());
        if let Some(change_index) = change_indexes.get(&old.file) {
            change_sets[*change_index]
                .lifecycle
                .push(SourceNodeLifecycle::Preserved {
                    source_node_id: old.id.clone(),
                });
        }
    }

    for node in &next.nodes {
        if ids.contains_key(&node.id) {
            continue;
        }
        let id = next_runtime_source_node_id();
        if let Some(change_index) = change_indexes.get(&node.file) {
            change_sets[*change_index]
                .lifecycle
                .push(SourceNodeLifecycle::Inserted {
                    source_node_id: id.clone(),
                });
        }
        ids.insert(node.id.clone(), id);
    }
    for old in &previous.nodes {
        if preserved_old.contains(&old.id) {
            continue;
        }
        if let Some(change_index) = change_indexes.get(&old.file) {
            change_sets[*change_index]
                .lifecycle
                .push(SourceNodeLifecycle::Deleted {
                    source_node_id: old.id.clone(),
                });
        }
    }

    remap_source_graph_base_ids(next, &ids)
}

fn next_runtime_source_node_id() -> String {
    let sequence = NEXT_RUNTIME_SOURCE_NODE_ID.fetch_add(1, Ordering::Relaxed);
    format!("sgn_r{:016x}_{:016x}", std::process::id(), sequence)
}

fn remap_source_graph_base_ids(
    graph: &mut SourceGraph,
    ids: &HashMap<String, String>,
) -> Result<(), String> {
    if ids.values().collect::<HashSet<_>>().len() != ids.len() {
        return Err("SourceChangeSet a produs identități SourceNode duplicate.".to_string());
    }
    for node in &mut graph.nodes {
        remap_required(&mut node.id, ids);
        remap_optional(&mut node.parent, ids);
        remap_many(&mut node.children, ids);
    }
    for relation in &mut graph.relations {
        remap_required(&mut relation.from, ids);
        remap_required(&mut relation.to, ids);
        relation.id = source_relation_id(
            &relation.from,
            &relation.to,
            &relation.kind,
            &relation.label,
        );
    }
    for page in &mut graph.pages {
        remap_required(&mut page.id, ids);
        remap_required(&mut page.content_node_id, ids);
        remap_optional(&mut page.template_node_id, ids);
        remap_optional(&mut page.page_template_node_id, ids);
        remap_data_nodes(&mut page.frontmatter_nodes, ids);
        remap_shortcodes(&mut page.shortcodes, ids);
    }
    for template in &mut graph.templates {
        remap_required(&mut template.id, ids);
        remap_required(&mut template.node_id, ids);
        for projection in &mut template.markdown_projections {
            remap_required(&mut projection.template_source_node_id, ids);
        }
    }
    for style in &mut graph.styles {
        remap_required(&mut style.id, ids);
        remap_required(&mut style.node_id, ids);
    }
    for script in &mut graph.scripts {
        remap_required(&mut script.id, ids);
        remap_required(&mut script.node_id, ids);
    }
    for asset in &mut graph.assets {
        remap_required(&mut asset.id, ids);
        remap_required(&mut asset.node_id, ids);
    }
    for data_file in &mut graph.data_files {
        remap_required(&mut data_file.id, ids);
        remap_required(&mut data_file.node_id, ids);
        remap_data_nodes(&mut data_file.nodes, ids);
    }
    for document in &mut graph.structured_documents {
        remap_required(&mut document.id, ids);
        remap_required(&mut document.node_id, ids);
        remap_data_nodes(&mut document.nodes, ids);
    }
    graph.rebuild_node_index()
}

fn remap_required(value: &mut String, ids: &HashMap<String, String>) {
    if let Some(replacement) = ids.get(value) {
        *value = replacement.clone();
    }
}

fn remap_optional(value: &mut Option<String>, ids: &HashMap<String, String>) {
    if let Some(value) = value.as_mut() {
        remap_required(value, ids);
    }
}

fn remap_many(values: &mut [String], ids: &HashMap<String, String>) {
    for value in values {
        remap_required(value, ids);
    }
}

fn remap_data_nodes(nodes: &mut [SourceDataNode], ids: &HashMap<String, String>) {
    for node in nodes {
        remap_required(&mut node.id, ids);
        remap_optional(&mut node.parent_id, ids);
        remap_many(&mut node.children, ids);
    }
}

fn remap_shortcodes(
    invocations: &mut [crate::source_graph::zola_shortcode::ZolaShortcodeInvocation],
    ids: &HashMap<String, String>,
) {
    for invocation in invocations {
        remap_optional(&mut invocation.source_node_id, ids);
        remap_shortcodes(&mut invocation.inner, ids);
    }
}

fn common_prefix_len(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn common_suffix_len(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .rev()
        .zip(right.iter().rev())
        .take_while(|(left, right)| left == right)
        .count()
}

pub(crate) fn source_relation_id(
    from: &str,
    to: &str,
    kind: &SourceRelationKind,
    label: &str,
) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    from.hash(&mut hasher);
    to.hash(&mut hasher);
    relation_kind_key(kind).hash(&mut hasher);
    label.hash(&mut hasher);
    format!("rel_{:016x}", hasher.finish())
}

fn relation_kind_key(kind: &SourceRelationKind) -> &'static str {
    match kind {
        SourceRelationKind::PageTemplate => "page_template",
        SourceRelationKind::SectionPageTemplate => "section_page_template",
        SourceRelationKind::GetsPage => "gets_page",
        SourceRelationKind::GetsSection => "gets_section",
        SourceRelationKind::InternalContentLink => "internal_content_link",
        SourceRelationKind::AssetUrl => "asset_url",
        SourceRelationKind::AssetHash => "asset_hash",
        SourceRelationKind::AssetReference => "asset_reference",
        SourceRelationKind::DataLoad => "data_load",
        SourceRelationKind::DataFileLoad => "data_file_load",
        SourceRelationKind::ContentDataLoad => "content_data_load",
        SourceRelationKind::ImageMetadata => "image_metadata",
        SourceRelationKind::ImageResize => "image_resize",
        SourceRelationKind::Extends => "extends",
        SourceRelationKind::Includes => "includes",
        SourceRelationKind::Imports => "imports",
        SourceRelationKind::DefinesBlock => "defines_block",
        SourceRelationKind::OverridesBlock => "overrides_block",
        SourceRelationKind::UsesStyle => "uses_style",
        SourceRelationKind::UsesScript => "uses_script",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        hint::black_box,
        time::Instant,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        kernel::preview_projection::{CanvasPatch, CanvasPatchAnchor, CanvasPatchOperation},
        project_model::{
            delete_engine::{plan_html_delete, ProjectHtmlDeleteIntent},
            duplicate_engine::{plan_html_duplicate, ProjectHtmlDuplicateIntent},
            move_engine::ProjectMovePosition,
            tera_delete_engine::{plan_tera_delete, ProjectTeraDeleteIntent},
            tera_move_engine::{plan_tera_move, ProjectTeraMoveIntent},
            test_support::ProjectModelTestFixture,
        },
        source_graph::model::SourceNodeKind,
    };

    use super::*;

    fn unique_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "pana-source-change-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn duplicate_transition_preserves_originals_and_records_new_subtree_provenance() {
        let root = unique_root("duplicate");
        let source = "<main><section><span></span></section><section><span></span></section><section><span></span></section></main>";
        let mut fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let before = fixture.build_model().unwrap();
        let sections = before
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.kind == SourceNodeKind::Html && node.label == "<section>")
            .collect::<Vec<_>>();
        let source_id = sections[1].id.clone();
        let patch = plan_html_duplicate(
            &before,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(source_id.clone()),
                source_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        )
        .patch
        .expect("duplicate patch");
        fixture.draft(&patch.file, &patch.contents);
        let mut after = fixture.build_model().unwrap().source_graph;
        let mut changes = vec![
            SourceChangeSet::between(&patch.file, source, &patch.contents)
                .with_tree_duplicate(&source_id, patch.inserted_offset),
        ];
        reconcile_project_source_node_ids(&before.source_graph, &mut after, &mut changes).unwrap();

        for node in &before.source_graph.nodes {
            assert!(after.nodes.iter().any(|candidate| candidate.id == node.id));
        }
        let duplicated = changes[0]
            .lifecycle
            .iter()
            .filter_map(|transition| match transition {
                SourceNodeLifecycle::Duplicated {
                    source_node_id,
                    duplicated_from_source_node_id,
                } => Some((source_node_id, duplicated_from_source_node_id)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(duplicated.len(), 2);
        assert!(duplicated
            .iter()
            .any(|(_, duplicated_from)| *duplicated_from == &source_id));
        assert!(!changes[0]
            .lifecycle
            .iter()
            .any(|transition| matches!(transition, SourceNodeLifecycle::Deleted { .. })));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_undo_redo_restores_every_source_node_id_in_the_subtree() {
        let root = unique_root("duplicate-history");
        let source = "<main><section><span></span></section><section><span></span></section><section><span></span></section></main>";
        let mut fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let before = fixture.build_model().unwrap();
        let source_id = before
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.kind == SourceNodeKind::Html && node.label == "<section>")
            .nth(1)
            .unwrap()
            .id
            .clone();
        let patch = plan_html_duplicate(
            &before,
            &ProjectHtmlDuplicateIntent {
                source_source_id: Some(source_id.clone()),
                source_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        )
        .patch
        .expect("duplicate patch");
        fixture.draft(&patch.file, &patch.contents);
        let mut after_duplicate = fixture.build_model().unwrap().source_graph;
        let mut duplicate_changes =
            vec![
                SourceChangeSet::between(&patch.file, source, &patch.contents)
                    .with_tree_duplicate(&source_id, patch.inserted_offset),
            ];
        reconcile_project_source_node_ids(
            &before.source_graph,
            &mut after_duplicate,
            &mut duplicate_changes,
        )
        .unwrap();
        let duplicate_root_id = after_duplicate
            .nodes
            .iter()
            .find(|node| {
                node.kind == SourceNodeKind::Html
                    && node.label == "<section>"
                    && node
                        .range
                        .as_ref()
                        .is_some_and(|range| range.start == patch.inserted_offset)
            })
            .unwrap()
            .id
            .clone();
        let retained_tree =
            capture_source_tree_identity(&after_duplicate, &duplicate_root_id).unwrap();

        fixture.draft(&patch.file, source);
        let mut after_undo = fixture.build_model().unwrap().source_graph;
        let mut undo_changes = vec![
            SourceChangeSet::between(&patch.file, &patch.contents, source)
                .with_tree_delete(&duplicate_root_id),
        ];
        reconcile_project_source_node_ids(&after_duplicate, &mut after_undo, &mut undo_changes)
            .unwrap();
        assert!(retained_tree.nodes.iter().all(|identity| {
            !after_undo
                .nodes
                .iter()
                .any(|node| node.id == identity.source_node_id)
        }));

        fixture.draft(&patch.file, &patch.contents);
        let mut after_redo = fixture.build_model().unwrap().source_graph;
        let mut redo_changes = vec![
            SourceChangeSet::between(&patch.file, source, &patch.contents)
                .with_tree_restore(retained_tree.clone()),
        ];
        reconcile_project_source_node_ids(&after_undo, &mut after_redo, &mut redo_changes).unwrap();
        let restored_tree = capture_source_tree_identity(&after_redo, &duplicate_root_id).unwrap();
        assert_eq!(restored_tree, retained_tree);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn multi_root_insert_undo_redo_restores_the_exact_source_forest() {
        let root = unique_root("insert-forest-history");
        let source = "<main><p>A</p></main>";
        let inserted_source = "<main><span></span><em></em><p>A</p></main>";
        let mut fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let before = fixture.build_model().unwrap();
        let target_id = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<p>")
            .unwrap()
            .id
            .clone();
        let before_ids = before
            .source_graph
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<HashSet<_>>();

        fixture.draft("templates/index.html", inserted_source);
        let mut after_insert = fixture.build_model().unwrap().source_graph;
        let mut insert_changes =
            vec![
                SourceChangeSet::between("templates/index.html", source, inserted_source)
                    .with_tree_insert(
                        &target_id,
                        SourceTreeMovePosition::Before,
                        None,
                        Some("<main>".len()),
                    ),
            ];
        reconcile_project_source_node_ids(
            &before.source_graph,
            &mut after_insert,
            &mut insert_changes,
        )
        .unwrap();
        let parent = after_insert
            .node_by_id(
                before
                    .source_graph
                    .node_by_id(&target_id)
                    .unwrap()
                    .parent
                    .as_deref()
                    .unwrap(),
            )
            .unwrap();
        let inserted_roots = parent
            .children
            .iter()
            .filter(|id| !before_ids.contains(id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(inserted_roots.len(), 2);
        let retained_forest =
            capture_source_forest_identity(&after_insert, &inserted_roots).unwrap();
        assert_eq!(retained_forest.root_count, 2);

        fixture.draft("templates/index.html", source);
        let mut after_undo = fixture.build_model().unwrap().source_graph;
        let mut undo_changes =
            vec![
                SourceChangeSet::between("templates/index.html", inserted_source, source)
                    .with_tree_delete_many(inserted_roots.clone()),
            ];
        reconcile_project_source_node_ids(&after_insert, &mut after_undo, &mut undo_changes)
            .unwrap();
        assert!(inserted_roots
            .iter()
            .all(|id| after_undo.node_by_id(id).is_none()));

        fixture.draft("templates/index.html", inserted_source);
        let mut after_redo = fixture.build_model().unwrap().source_graph;
        let mut redo_changes =
            vec![
                SourceChangeSet::between("templates/index.html", source, inserted_source)
                    .with_tree_restore(retained_forest.clone()),
            ];
        reconcile_project_source_node_ids(&after_undo, &mut after_redo, &mut redo_changes).unwrap();
        assert_eq!(
            capture_source_forest_identity(&after_redo, &inserted_roots).unwrap(),
            retained_forest
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_undo_restores_the_exact_middle_subtree_without_stealing_a_sibling_id() {
        let root = unique_root("delete-history");
        let source = "<main><section><span></span></section><section><span></span></section><section><span></span></section></main>";
        let mut fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let before = fixture.build_model().unwrap();
        let section_ids = before
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.kind == SourceNodeKind::Html && node.label == "<section>")
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let deleted_tree =
            capture_source_tree_identity(&before.source_graph, &section_ids[1]).unwrap();
        let patch = plan_html_delete(
            &before,
            &ProjectHtmlDeleteIntent {
                target_source_id: Some(section_ids[1].clone()),
                target_render_instance_id: None,
                target_tag: Some("section".to_string()),
                native_block_slot: None,
            },
        )
        .patch
        .expect("delete patch");

        fixture.draft(&patch.file, &patch.contents);
        let mut after_delete = fixture.build_model().unwrap().source_graph;
        let mut delete_changes =
            vec![
                SourceChangeSet::between(&patch.file, source, &patch.contents)
                    .with_tree_delete(&section_ids[1]),
            ];
        reconcile_project_source_node_ids(
            &before.source_graph,
            &mut after_delete,
            &mut delete_changes,
        )
        .unwrap();
        let leaked_deleted_ids = deleted_tree
            .nodes
            .iter()
            .filter_map(|identity| {
                after_delete
                    .node_by_id(&identity.source_node_id)
                    .map(|node| (identity.source_node_id.clone(), node.range.clone()))
            })
            .collect::<Vec<_>>();
        assert!(
            leaked_deleted_ids.is_empty(),
            "identitățile subarborelui șters au fost realocate: {leaked_deleted_ids:?}; edits={:?}; lifecycle={:?}",
            delete_changes[0].edits,
            delete_changes[0].lifecycle,
        );
        assert!(after_delete.node_by_id(&section_ids[0]).is_some());
        assert!(after_delete.node_by_id(&section_ids[2]).is_some());

        fixture.draft(&patch.file, source);
        let mut after_undo = fixture.build_model().unwrap().source_graph;
        let mut undo_changes = vec![
            SourceChangeSet::between(&patch.file, &patch.contents, source)
                .with_tree_restore(deleted_tree.clone()),
        ];
        reconcile_project_source_node_ids(&after_delete, &mut after_undo, &mut undo_changes)
            .unwrap();
        assert_eq!(
            capture_source_tree_identity(&after_undo, &section_ids[1]).unwrap(),
            deleted_tree
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn tera_delete_undo_restores_the_exact_middle_identity() {
        let root = unique_root("tera-delete-history");
        let source = concat!(
            "{% block content %}\n",
            "{{ same }}\n",
            "{{ same }}\n",
            "{{ same }}\n",
            "{% endblock %}\n",
        );
        let mut fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let before = fixture.build_model().unwrap();
        let variable_ids = before
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.kind == SourceNodeKind::TeraVariable && node.label == "same")
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        assert_eq!(variable_ids.len(), 3);
        let deleted_tree =
            capture_source_tree_identity(&before.source_graph, &variable_ids[1]).unwrap();
        let patch = plan_tera_delete(
            &before,
            &ProjectTeraDeleteIntent {
                target_source_id: Some(variable_ids[1].clone()),
                target_kind: Some("teraVariable".to_string()),
                target_label: Some("same".to_string()),
            },
        )
        .patch
        .expect("tera delete patch");

        fixture.draft(&patch.file, &patch.contents);
        let mut after_delete = fixture.build_model().unwrap().source_graph;
        let mut delete_changes =
            vec![
                SourceChangeSet::between(&patch.file, source, &patch.contents)
                    .with_tree_delete(&variable_ids[1]),
            ];
        reconcile_project_source_node_ids(
            &before.source_graph,
            &mut after_delete,
            &mut delete_changes,
        )
        .unwrap();
        assert!(after_delete.node_by_id(&variable_ids[0]).is_some());
        assert!(after_delete.node_by_id(&variable_ids[1]).is_none());
        assert!(after_delete.node_by_id(&variable_ids[2]).is_some());

        fixture.draft(&patch.file, source);
        let mut after_undo = fixture.build_model().unwrap().source_graph;
        let mut undo_changes = vec![
            SourceChangeSet::between(&patch.file, &patch.contents, source)
                .with_tree_restore(deleted_tree.clone()),
        ];
        reconcile_project_source_node_ids(&after_delete, &mut after_undo, &mut undo_changes)
            .unwrap();
        assert_eq!(
            capture_source_tree_identity(&after_undo, &variable_ids[1]).unwrap(),
            deleted_tree
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mixed_tree_move_preserves_exact_tera_sibling_ids() {
        let root = unique_root("tera-move");
        let source = concat!(
            "{% block content %}\n",
            "{{ same }}\n",
            "{{ same }}\n",
            "<section></section>\n",
            "{% endblock %}\n",
        );
        let mut fixture = ProjectModelTestFixture::standard_zola(root.clone(), source).unwrap();
        let before = fixture.build_model().unwrap();
        let variables = before
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.kind == SourceNodeKind::TeraVariable && node.label == "same")
            .collect::<Vec<_>>();
        let target = before
            .source_graph
            .nodes
            .iter()
            .find(|node| node.kind == SourceNodeKind::Html && node.label == "<section>")
            .unwrap();
        let moved_id = variables[0].id.clone();
        let untouched_id = variables[1].id.clone();
        let patch = plan_tera_move(
            &before,
            &ProjectTeraMoveIntent {
                source_source_id: Some(moved_id.clone()),
                target_source_id: Some(target.id.clone()),
                source_kind: Some("teraVariable".to_string()),
                target_kind: Some("html".to_string()),
                source_label: Some("same".to_string()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::Before,
            },
        )
        .patch
        .expect("tera move patch");
        fixture.draft(&patch.file, &patch.contents);
        let mut after = fixture.build_model().unwrap().source_graph;
        let mut changes = vec![
            SourceChangeSet::between(&patch.file, source, &patch.contents).with_tree_move(
                &moved_id,
                &patch.resolved_target_id,
                SourceTreeMovePosition::Before,
            ),
        ];
        reconcile_project_source_node_ids(&before.source_graph, &mut after, &mut changes).unwrap();
        let after_variables = after
            .nodes
            .iter()
            .filter(|node| node.kind == SourceNodeKind::TeraVariable && node.label == "same")
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            after_variables,
            vec![untouched_id.as_str(), moved_id.as_str()]
        );
        assert!(changes[0].lifecycle.iter().any(|transition| matches!(
            transition,
            SourceNodeLifecycle::Moved { source_node_id } if source_node_id == &moved_id
        )));
        assert!(!changes[0].lifecycle.iter().any(|transition| matches!(
            transition,
            SourceNodeLifecycle::Inserted { .. } | SourceNodeLifecycle::Deleted { .. }
        )));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "release performance budget"]
    fn source_reconcile_to_canvas_patch_warm_p95_is_below_fifty_milliseconds() {
        for node_count in [1_000usize, 10_000usize] {
            let root = unique_root(&format!("reconcile-benchmark-{node_count}"));
            let mut source = String::with_capacity(node_count * 36);
            source.push_str("<main>\n");
            for index in 0..node_count {
                source.push_str(&format!("<div data-i=\"{index}\">x</div>\n"));
            }
            source.push_str("</main>\n");
            let mut fixture =
                ProjectModelTestFixture::standard_zola(root.clone(), &source).unwrap();
            let before = fixture.build_model().unwrap();
            let target_index = node_count / 2;
            let needle = format!("<div data-i=\"{target_index}\">x</div>");
            let replacement =
                format!("<div data-i=\"{target_index}\" data-benchmark=\"1\">x</div>");
            let after_source = source.replacen(&needle, &replacement, 1);
            let target_start = source.find(&needle).unwrap();
            let target_id = before
                .source_graph
                .nodes
                .iter()
                .find(|node| {
                    node.kind == SourceNodeKind::Html
                        && node
                            .range
                            .as_ref()
                            .is_some_and(|range| range.start == target_start)
                })
                .unwrap()
                .id
                .clone();
            fixture.draft("templates/index.html", &after_source);
            let parsed_after = fixture.build_model().unwrap().source_graph;
            let base_change =
                SourceChangeSet::between("templates/index.html", &source, &after_source);
            let mut samples = Vec::with_capacity(32);
            for sample in 0..36u64 {
                let started = Instant::now();
                let mut after = parsed_after.clone();
                let mut changes = vec![base_change.clone()];
                reconcile_project_source_node_ids(&before.source_graph, &mut after, &mut changes)
                    .unwrap();
                assert!(after.node_by_id(&target_id).is_some());
                let patch = CanvasPatch::issued_for_history(
                    "/benchmark/project",
                    "benchmark-session",
                    sample + 1,
                    sample + 2,
                    &format!("benchmark-transaction-{node_count}-{sample}"),
                    "model-before",
                    "model-after",
                    CanvasPatchOperation::SetAttributes {
                        target: CanvasPatchAnchor::source(&target_id, Some("div")),
                        attributes: BTreeMap::from([(
                            "data-benchmark".to_string(),
                            Some("1".to_string()),
                        )]),
                    },
                )
                .unwrap();
                let elapsed = started.elapsed().as_nanos();
                black_box(patch.patch_id);
                if sample >= 4 {
                    samples.push(elapsed);
                }
            }
            samples.sort_unstable();
            let percentile =
                |percent: usize| samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)];
            let p50 = percentile(50);
            let p95 = percentile(95);
            let p99 = percentile(99);
            eprintln!("rust_to_patch nodes={node_count} p50_ns={p50} p95_ns={p95} p99_ns={p99}");
            assert!(
                p95 < 50_000_000,
                "Rust→patch warm p95 pentru {node_count} noduri este {} ms, peste bugetul de 50 ms",
                p95 as f64 / 1_000_000.0
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
}
