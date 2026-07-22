use serde::{Deserialize, Serialize};

use crate::{
    js::PageJsConfig,
    kernel::file_buffer_store::{hash_text, FileBufferEntry},
};

use super::model::{
    WorkspaceBinaryResource, WorkspaceHistoryEntrySnapshot, WorkspaceHistorySnapshot,
};

const DEFAULT_HISTORY_ENTRY_LIMIT: usize = 200;
const DEFAULT_HISTORY_BYTES_LIMIT: u64 = 64 * 1024 * 1024;
const COALESCE_WINDOW_MS: u128 = 1_200;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct WorkspaceDocumentTransition {
    pub relative_path: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct WorkspacePageJsTransition {
    pub template_path: String,
    pub before: PageJsConfig,
    pub after: PageJsConfig,
    pub before_cachebust_assets: bool,
    pub after_cachebust_assets: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct WorkspaceResourceTransition {
    pub relative_path: String,
    pub before: Option<FileBufferEntry>,
    pub after: Option<FileBufferEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct WorkspaceBinaryResourceTransition {
    pub relative_path: String,
    pub before: Option<WorkspaceBinaryResource>,
    pub after: Option<WorkspaceBinaryResource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct WorkspaceHistoryEntry {
    pub transaction_id: String,
    pub label: String,
    pub source: String,
    pub coalesce_key: Option<String>,
    pub created_at_ms: u128,
    pub updated_at_ms: u128,
    pub mutation_count: u32,
    pub documents: Vec<WorkspaceDocumentTransition>,
    pub resources: Vec<WorkspaceResourceTransition>,
    pub binary_resources: Vec<WorkspaceBinaryResourceTransition>,
    pub page_js: Vec<WorkspacePageJsTransition>,
}

impl WorkspaceHistoryEntry {
    pub fn snapshot(&self) -> WorkspaceHistoryEntrySnapshot {
        WorkspaceHistoryEntrySnapshot {
            transaction_id: self.transaction_id.clone(),
            label: self.label.clone(),
            source: self.source.clone(),
            coalesce_key: self.coalesce_key.clone(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: self.updated_at_ms,
            mutation_count: self.mutation_count,
            document_paths: {
                let mut paths = self
                    .documents
                    .iter()
                    .map(|item| item.relative_path.clone())
                    .chain(self.resources.iter().map(|item| item.relative_path.clone()))
                    .chain(
                        self.binary_resources
                            .iter()
                            .map(|item| item.relative_path.clone()),
                    )
                    .collect::<Vec<_>>();
                paths.sort();
                paths.dedup();
                paths
            },
            topology_paths: {
                let mut paths = self
                    .resources
                    .iter()
                    .filter(|item| item.before.is_none() != item.after.is_none())
                    .map(|item| item.relative_path.clone())
                    .chain(
                        self.binary_resources
                            .iter()
                            .filter(|item| item.before.is_none() != item.after.is_none())
                            .map(|item| item.relative_path.clone()),
                    )
                    .collect::<Vec<_>>();
                paths.sort();
                paths.dedup();
                paths
            },
            page_js_paths: self
                .page_js
                .iter()
                .map(|item| item.template_path.clone())
                .collect(),
            retained_bytes: self.retained_bytes(),
        }
    }

    pub fn retained_bytes(&self) -> u64 {
        let document_bytes = self
            .documents
            .iter()
            .map(|item| {
                item.relative_path.len() as u64 + item.before.len() as u64 + item.after.len() as u64
            })
            .sum::<u64>();
        let page_js_bytes = self
            .page_js
            .iter()
            .map(|item| {
                item.template_path.len() as u64
                    + retained_page_js_entry_bytes(&item.before)
                    + retained_page_js_entry_bytes(&item.after)
            })
            .sum::<u64>();
        let resource_bytes = self
            .resources
            .iter()
            .map(|item| {
                item.relative_path.len() as u64
                    + retained_file_entry_bytes(item.before.as_ref())
                    + retained_file_entry_bytes(item.after.as_ref())
            })
            .sum::<u64>();
        let binary_resource_bytes = self
            .binary_resources
            .iter()
            .map(|item| {
                item.relative_path.len() as u64
                    + item
                        .before
                        .as_ref()
                        .map(|resource| resource.bytes.len() as u64)
                        .unwrap_or_default()
                    + item
                        .after
                        .as_ref()
                        .map(|resource| resource.bytes.len() as u64)
                        .unwrap_or_default()
            })
            .sum::<u64>();
        document_bytes
            .saturating_add(resource_bytes)
            .saturating_add(binary_resource_bytes)
            .saturating_add(page_js_bytes)
    }

    fn can_coalesce(&self, next: &Self) -> bool {
        let Some(key) = self.coalesce_key.as_deref() else {
            return false;
        };
        next.coalesce_key.as_deref() == Some(key)
            && self.source == next.source
            && (key.starts_with("preview.html.text.group:")
                || next.created_at_ms.saturating_sub(self.updated_at_ms) <= COALESCE_WINDOW_MS)
            && self.documents.len() == next.documents.len()
            && self.resources.is_empty()
            && next.resources.is_empty()
            && self.binary_resources.is_empty()
            && next.binary_resources.is_empty()
            && self.page_js.len() == next.page_js.len()
            && self
                .documents
                .iter()
                .zip(&next.documents)
                .all(|(left, right)| left.relative_path == right.relative_path)
            && self
                .page_js
                .iter()
                .zip(&next.page_js)
                .all(|(left, right)| left.template_path == right.template_path)
    }

    fn coalesce(&mut self, next: Self) {
        for (current, latest) in self.documents.iter_mut().zip(next.documents) {
            current.after = latest.after;
        }
        for (current, latest) in self.page_js.iter_mut().zip(next.page_js) {
            current.after = latest.after;
            current.after_cachebust_assets = latest.after_cachebust_assets;
        }
        self.transaction_id = next.transaction_id;
        self.label = next.label;
        self.updated_at_ms = next.updated_at_ms;
        self.mutation_count = self.mutation_count.saturating_add(next.mutation_count);
    }

    fn is_net_noop(&self) -> bool {
        let has_coalescible_transition = !self.documents.is_empty() || !self.page_js.is_empty();
        has_coalescible_transition
            && self.resources.is_empty()
            && self.binary_resources.is_empty()
            && self.documents.iter().all(transition_is_no_op)
            && self.page_js.iter().all(|transition| {
                transition.before == transition.after
                    && transition.before_cachebust_assets == transition.after_cachebust_assets
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct WorkspaceHistory {
    undo: Vec<WorkspaceHistoryEntry>,
    redo: Vec<WorkspaceHistoryEntry>,
    entry_limit: usize,
    retained_bytes_limit: u64,
    #[serde(default)]
    coalesce_barrier: bool,
}

impl Default for WorkspaceHistory {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            entry_limit: DEFAULT_HISTORY_ENTRY_LIMIT,
            retained_bytes_limit: DEFAULT_HISTORY_BYTES_LIMIT,
            coalesce_barrier: false,
        }
    }
}

impl WorkspaceHistory {
    pub fn recovery_paths(&self) -> Vec<&str> {
        self.undo
            .iter()
            .chain(&self.redo)
            .flat_map(|entry| {
                entry
                    .documents
                    .iter()
                    .map(|item| item.relative_path.as_str())
                    .chain(
                        entry
                            .resources
                            .iter()
                            .map(|item| item.relative_path.as_str()),
                    )
                    .chain(
                        entry
                            .binary_resources
                            .iter()
                            .map(|item| item.relative_path.as_str()),
                    )
            })
            .collect()
    }

    pub fn validate_recovery_limits(&self) -> Result<(), String> {
        if self.entry_limit == 0
            || self.entry_limit > DEFAULT_HISTORY_ENTRY_LIMIT
            || self.retained_bytes_limit == 0
            || self.retained_bytes_limit > DEFAULT_HISTORY_BYTES_LIMIT
            || self.undo.len() > self.entry_limit
            || self.redo.len() > self.entry_limit
            || self.retained_bytes() > self.retained_bytes_limit
        {
            return Err(
                "ProjectWorkspace recovery conține un History în afara limitelor nucleului."
                    .to_string(),
            );
        }
        Ok(())
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn next_undo_transaction_id(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.transaction_id.as_str())
    }

    pub fn next_redo_transaction_id(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.transaction_id.as_str())
    }

    pub fn merge_recorded_since(
        &mut self,
        start: usize,
        transaction_id: String,
        label: String,
        source: String,
        now_ms: u128,
    ) -> Result<Option<WorkspaceHistoryEntry>, String> {
        if start > self.undo.len() {
            return Err(
                "ProjectWorkspace nu poate compune history: ancora este după coada Undo."
                    .to_string(),
            );
        }
        let recorded = self.undo.drain(start..).collect::<Vec<_>>();
        if recorded.is_empty() {
            return Ok(None);
        }

        let created_at_ms = recorded
            .iter()
            .map(|entry| entry.created_at_ms)
            .min()
            .unwrap_or(now_ms);
        let mutation_count = recorded
            .iter()
            .map(|entry| entry.mutation_count)
            .fold(0_u32, u32::saturating_add)
            .max(1);
        let mut documents = Vec::new();
        let mut resources = Vec::new();
        let mut binary_resources = Vec::new();
        let mut page_js = Vec::new();
        for entry in recorded {
            documents.extend(entry.documents);
            resources.extend(entry.resources);
            binary_resources.extend(entry.binary_resources);
            page_js.extend(entry.page_js);
        }
        let entry = WorkspaceHistoryEntry {
            transaction_id,
            label,
            source,
            coalesce_key: None,
            created_at_ms,
            updated_at_ms: now_ms,
            mutation_count,
            documents,
            resources,
            binary_resources,
            page_js,
        };
        self.undo.push(entry.clone());
        self.trim();
        Ok(Some(entry))
    }

    pub fn record(&mut self, entry: WorkspaceHistoryEntry) {
        let can_coalesce = !self.coalesce_barrier && self.redo.is_empty();
        self.redo.clear();
        self.coalesce_barrier = false;
        if can_coalesce {
            if let Some(previous) = self.undo.last_mut() {
                if previous.can_coalesce(&entry) {
                    previous.coalesce(entry);
                    if previous.is_net_noop() {
                        self.undo.pop();
                    }
                    self.trim();
                    return;
                }
            }
        }
        self.undo.push(entry);
        self.trim();
    }

    pub fn pop_undo(&mut self) -> Result<WorkspaceHistoryEntry, String> {
        self.undo
            .pop()
            .ok_or_else(|| "ProjectWorkspace nu are o mutație disponibilă pentru Undo.".to_string())
    }

    pub fn complete_undo(&mut self, entry: WorkspaceHistoryEntry) {
        self.redo.push(entry);
        self.coalesce_barrier = true;
    }

    pub fn pop_redo(&mut self) -> Result<WorkspaceHistoryEntry, String> {
        self.redo
            .pop()
            .ok_or_else(|| "ProjectWorkspace nu are o mutație disponibilă pentru Redo.".to_string())
    }

    pub fn complete_redo(&mut self, entry: WorkspaceHistoryEntry) {
        self.undo.push(entry);
        self.coalesce_barrier = true;
    }

    pub fn break_coalescing_group(&mut self) {
        self.coalesce_barrier = true;
    }

    pub fn snapshot(&self) -> WorkspaceHistorySnapshot {
        WorkspaceHistorySnapshot {
            undo_count: self.undo.len(),
            redo_count: self.redo.len(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            retained_bytes: self.retained_bytes(),
            retained_bytes_limit: self.retained_bytes_limit,
            entry_limit: self.entry_limit,
            next_undo: self.undo.last().map(WorkspaceHistoryEntry::snapshot),
            next_redo: self.redo.last().map(WorkspaceHistoryEntry::snapshot),
            undo_entries: self
                .undo
                .iter()
                .rev()
                .map(WorkspaceHistoryEntry::snapshot)
                .collect(),
            redo_entries: self
                .redo
                .iter()
                .rev()
                .map(WorkspaceHistoryEntry::snapshot)
                .collect(),
        }
    }

    fn retained_bytes(&self) -> u64 {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(WorkspaceHistoryEntry::retained_bytes)
            .sum()
    }

    fn trim(&mut self) {
        while self.undo.len() > self.entry_limit {
            self.undo.remove(0);
        }
        while self.retained_bytes() > self.retained_bytes_limit && self.undo.len() > 1 {
            self.undo.remove(0);
        }
    }
}

pub(crate) fn new_history_entry(
    transaction_id: String,
    label: String,
    source: String,
    coalesce_key: Option<String>,
    now_ms: u128,
    documents: Vec<WorkspaceDocumentTransition>,
    resources: Vec<WorkspaceResourceTransition>,
    binary_resources: Vec<WorkspaceBinaryResourceTransition>,
    page_js: Vec<WorkspacePageJsTransition>,
) -> WorkspaceHistoryEntry {
    WorkspaceHistoryEntry {
        transaction_id,
        label,
        source,
        coalesce_key,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
        mutation_count: 1,
        documents,
        resources,
        binary_resources,
        page_js,
    }
}

fn retained_file_entry_bytes(entry: Option<&FileBufferEntry>) -> u64 {
    entry
        .map(|entry| {
            entry.relative_path.len() as u64
                + entry.absolute_path.len() as u64
                + entry.baseline_text.len() as u64
                + entry
                    .draft
                    .as_ref()
                    .map(|draft| draft.text.len() as u64)
                    .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn retained_page_js_entry_bytes(entry: &PageJsConfig) -> u64 {
    serde_json::to_vec(entry)
        .map(|bytes| bytes.len() as u64)
        .unwrap_or_default()
}

pub(crate) fn transition_is_no_op(transition: &WorkspaceDocumentTransition) -> bool {
    hash_text(&transition.before) == hash_text(&transition.after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_with_text(key: &str, at_ms: u128, before: &str, after: &str) -> WorkspaceHistoryEntry {
        new_history_entry(
            format!("transaction-{at_ms}"),
            "Edit text".to_string(),
            "preview.structural".to_string(),
            Some(key.to_string()),
            at_ms,
            vec![WorkspaceDocumentTransition {
                relative_path: "content/index.md".to_string(),
                before: before.to_string(),
                after: after.to_string(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn entry(key: &str, at_ms: u128) -> WorkspaceHistoryEntry {
        entry_with_text(key, at_ms, "before", &format!("after-{at_ms}"))
    }

    #[test]
    fn explicit_text_edit_group_coalesces_beyond_the_generic_time_window() {
        let key = "preview.html.text.group:text_session_1:preview.html.text:index:title";
        let mut history = WorkspaceHistory::default();
        history.record(entry(key, 100));
        history.record(entry(key, 100 + COALESCE_WINDOW_MS + 10_000));

        assert_eq!(history.undo.len(), 1);
        assert_eq!(history.undo[0].mutation_count, 2);
    }

    #[test]
    fn distinct_text_edit_groups_remain_distinct_undo_steps() {
        let mut history = WorkspaceHistory::default();
        history.record(entry(
            "preview.html.text.group:text_session_1:preview.html.text:index:title",
            100,
        ));
        history.record(entry(
            "preview.html.text.group:text_session_2:preview.html.text:index:title",
            101,
        ));

        assert_eq!(history.undo.len(), 2);
    }

    #[test]
    fn ordinary_coalesce_keys_still_obey_the_time_window() {
        let key = "preview.html.text:index:title";
        let mut history = WorkspaceHistory::default();
        history.record(entry(key, 100));
        history.record(entry(key, 100 + COALESCE_WINDOW_MS + 1));

        assert_eq!(history.undo.len(), 2);
    }

    #[test]
    fn edit_after_undo_starts_a_new_coalescing_group() {
        let key = "document:content/index.md";
        let mut history = WorkspaceHistory::default();
        history.record(entry_with_text(key, 100, "", "a"));
        history.record(entry_with_text("other-document", 101, "a", "other"));

        let undone = history.pop_undo().unwrap();
        history.complete_undo(undone);
        history.record(entry_with_text(key, 102, "a", "ab"));

        assert_eq!(history.undo.len(), 2);
        assert_eq!(history.redo.len(), 0);
        assert_eq!(history.undo[0].documents[0].after, "a");
        assert_eq!(history.undo[1].documents[0].before, "a");
    }

    #[test]
    fn coalescing_that_returns_to_the_original_text_drops_the_history_entry() {
        let key = "document:content/index.md";
        let mut history = WorkspaceHistory::default();
        history.record(entry_with_text(key, 100, "", "a"));
        history.record(entry_with_text(key, 101, "a", ""));

        assert!(history.undo.is_empty());
        assert!(history.redo.is_empty());
    }

    #[test]
    fn edit_after_redo_does_not_merge_into_the_replayed_entry() {
        let key = "document:content/index.md";
        let mut history = WorkspaceHistory::default();
        history.record(entry_with_text(key, 100, "", "a"));
        let undone = history.pop_undo().unwrap();
        history.complete_undo(undone);
        let redone = history.pop_redo().unwrap();
        history.complete_redo(redone);

        history.record(entry_with_text(key, 101, "a", "ab"));

        assert_eq!(history.undo.len(), 2);
        assert_eq!(history.undo[0].documents[0].after, "a");
        assert_eq!(history.undo[1].documents[0].before, "a");
    }
}
