use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

use crate::project::{ProjectDiskManifest, ProjectDiskManifestEntry};

use super::{
    model::{FileBufferBaseline, FileBufferStore, FileBufferStoreLimits, FileBufferTextSnapshot},
    reader::{read_project_disk_text_snapshot, ProjectDiskTextReadOutcome},
};

pub const KERNEL_EXTERNAL_DISK_RECONCILE_SCHEMA_VERSION: u32 = 1;
const MAX_RECONCILE_PATHS: usize = 1_000;
static RECONCILE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelExternalDiskReconcileInput {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub observed_manifest: ProjectDiskManifest,
    pub relative_paths: Vec<String>,
    #[serde(default)]
    pub active_relative_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelExternalDiskReconcileStatus {
    Applied,
    Noop,
    Blocked,
    ReloadRequired,
    StaleEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelExternalDiskReconcileItemOutcome {
    ContentRebased,
    MetadataRefreshed,
    Unchanged,
    Blocked,
    ReloadRequired,
    StaleEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelExternalDiskReconcileItemReceipt {
    pub relative_path: String,
    pub outcome: KernelExternalDiskReconcileItemOutcome,
    pub before_revision: Option<u64>,
    pub after_revision: Option<u64>,
    pub before_baseline: Option<FileBufferBaseline>,
    pub observed_disk_baseline: Option<FileBufferBaseline>,
    pub before_current_hash: Option<String>,
    pub after_current_hash: Option<String>,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelExternalDiskReconcileDiagnostic {
    pub code: String,
    pub relative_path: Option<String>,
    pub message: String,
    pub blocking: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelExternalDiskProjectionHints {
    pub project_rescan: bool,
    pub source_graph: bool,
    pub preview: bool,
    pub page_js: bool,
    pub scss: bool,
    pub history: bool,
    pub selection: bool,
}

impl KernelExternalDiskProjectionHints {
    fn none() -> Self {
        Self {
            project_rescan: false,
            source_graph: false,
            preview: false,
            page_js: false,
            scss: false,
            history: false,
            selection: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelExternalDiskReconcileReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub session_id: String,
    pub project_root: String,
    pub status: KernelExternalDiskReconcileStatus,
    pub verdict_reason: String,
    pub started_at_ms: u128,
    pub completed_at_ms: u128,
    pub requested_count: usize,
    pub target_count: usize,
    pub reconciled_count: usize,
    pub metadata_refreshed_count: usize,
    pub unchanged_count: usize,
    pub total_bytes_read: u64,
    pub requested_paths: Vec<String>,
    pub effective_paths: Vec<String>,
    pub invalidated_paths: Vec<String>,
    pub blocked_paths: Vec<String>,
    pub reload_required_paths: Vec<String>,
    pub history_invalidated: bool,
    pub source_graph_invalidated: bool,
    pub active_file: Option<FileBufferTextSnapshot>,
    pub accepted_manifest: Option<ProjectDiskManifest>,
    pub accepted_disk_generation: Option<u64>,
    /// ProjectWorkspace revision published by the reconcile commit. Derived
    /// consumers such as Preview must project exactly this revision.
    pub workspace_revision: Option<u64>,
    pub projection_hints: KernelExternalDiskProjectionHints,
    pub items: Vec<KernelExternalDiskReconcileItemReceipt>,
    pub diagnostics: Vec<KernelExternalDiskReconcileDiagnostic>,
}

impl KernelExternalDiskReconcileReceipt {
    pub(crate) fn mark_committed_runtime_effects(
        &mut self,
        history_invalidated: bool,
        source_graph_invalidated: bool,
    ) {
        self.history_invalidated = history_invalidated;
        self.source_graph_invalidated = source_graph_invalidated;
        self.projection_hints.history = history_invalidated;
    }

    pub(crate) fn blocked_by_runtime_guard(
        session_id: impl Into<String>,
        project_root: impl Into<String>,
        started_at_ms: u128,
        input: &KernelExternalDiskReconcileInput,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let project_root = project_root.into();
        let requested_paths = normalized_paths_best_effort(&input.relative_paths);
        let message = message.into();
        terminal_receipt(
            operation_id(&session_id, started_at_ms),
            session_id,
            project_root,
            started_at_ms,
            KernelExternalDiskReconcileStatus::Blocked,
            message.clone(),
            requested_paths.clone(),
            requested_paths.clone(),
            Vec::new(),
            requested_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::Blocked,
                        &message,
                    )
                })
                .collect(),
            vec![diagnostic(code, None, message)],
        )
    }

    pub(crate) fn stale_by_session_guard(
        session_id: impl Into<String>,
        project_root: impl Into<String>,
        started_at_ms: u128,
        input: &KernelExternalDiskReconcileInput,
        message: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let project_root = project_root.into();
        let requested_paths = normalized_paths_best_effort(&input.relative_paths);
        let message = message.into();
        terminal_receipt(
            operation_id(&session_id, started_at_ms),
            session_id,
            project_root,
            started_at_ms,
            KernelExternalDiskReconcileStatus::StaleEvidence,
            message.clone(),
            requested_paths.clone(),
            requested_paths.clone(),
            Vec::new(),
            requested_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::StaleEvidence,
                        &message,
                    )
                })
                .collect(),
            vec![diagnostic("session_instance_stale", None, message)],
        )
    }

    pub(crate) fn stale_manifest(
        plan: &CleanExternalReconcilePlan,
        message: impl Into<String>,
        completed_at_ms: u128,
    ) -> Self {
        let message = message.into();
        let mut receipt = terminal_receipt(
            plan.operation_id.clone(),
            plan.session_id.clone(),
            plan.project_root.clone(),
            plan.started_at_ms,
            KernelExternalDiskReconcileStatus::StaleEvidence,
            message.clone(),
            plan.requested_paths.clone(),
            plan.requested_paths.clone(),
            Vec::new(),
            plan.requested_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::StaleEvidence,
                        &message,
                    )
                })
                .collect(),
            vec![diagnostic("manifest_stale", None, message)],
        );
        receipt.completed_at_ms = completed_at_ms;
        receipt
    }
}

#[derive(Clone, Debug)]
pub(crate) enum CleanExternalReconcilePlanResult {
    Ready(CleanExternalReconcilePlan),
    Terminal(KernelExternalDiskReconcileReceipt),
}

#[derive(Clone, Debug)]
pub(crate) struct CleanExternalReconcilePlan {
    operation_id: String,
    session_id: String,
    store_session_id: String,
    project_root: String,
    started_at_ms: u128,
    requested_paths: Vec<String>,
    active_relative_path: Option<String>,
    observed_manifest: ProjectDiskManifest,
    targets: Vec<CleanExternalReconcileTarget>,
    store_version: Vec<StoreVersionEntry>,
    store_loaded_at_ms: u128,
    limits: FileBufferStoreLimits,
    baseline_total_bytes: u64,
}

impl CleanExternalReconcilePlan {
    pub(crate) fn observed_manifest(&self) -> &ProjectDiskManifest {
        &self.observed_manifest
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    pub(crate) fn project_root(&self) -> &str {
        &self.project_root
    }
}

#[derive(Clone, Debug)]
struct CleanExternalReconcileTarget {
    relative_path: String,
    revision: u64,
    baseline: FileBufferBaseline,
    current_hash: String,
    expected_manifest: ProjectDiskManifestEntry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoreVersionEntry {
    relative_path: String,
    revision: u64,
    baseline_hash: String,
    current_hash: String,
    has_draft: bool,
}

#[derive(Clone, Debug)]
pub(crate) enum CleanExternalReconcileReadResult {
    Staged(CleanExternalReconcileStaged),
    Terminal(KernelExternalDiskReconcileReceipt),
}

#[derive(Clone, Debug)]
pub(crate) struct CleanExternalReconcileStaged {
    plan: CleanExternalReconcilePlan,
    items: Vec<StagedItem>,
    total_bytes_read: u64,
    content_rebased_count: usize,
    metadata_refreshed_count: usize,
    unchanged_count: usize,
}

impl CleanExternalReconcileStaged {
    pub(crate) fn plan(&self) -> &CleanExternalReconcilePlan {
        &self.plan
    }

    pub(crate) fn invalidates_history(&self) -> bool {
        self.content_rebased_count > 0
    }
}

#[derive(Clone, Debug)]
struct StagedItem {
    target: CleanExternalReconcileTarget,
    text: String,
    disk_baseline: FileBufferBaseline,
    outcome: KernelExternalDiskReconcileItemOutcome,
}

pub(crate) fn plan_clean_external_reconcile(
    store: &FileBufferStore,
    input: KernelExternalDiskReconcileInput,
    started_at_ms: u128,
) -> CleanExternalReconcilePlanResult {
    let runtime_session_id = input.expected_session_id.clone();
    let operation_id = operation_id(&runtime_session_id, started_at_ms);
    let requested_paths = match normalize_requested_paths(&input.relative_paths) {
        Ok(paths) => paths,
        Err(message) => {
            return CleanExternalReconcilePlanResult::Terminal(terminal_receipt(
                operation_id,
                runtime_session_id.clone(),
                store.project_root.clone(),
                started_at_ms,
                KernelExternalDiskReconcileStatus::Blocked,
                message.clone(),
                normalized_paths_best_effort(&input.relative_paths),
                normalized_paths_best_effort(&input.relative_paths),
                Vec::new(),
                Vec::new(),
                vec![diagnostic("invalid_request_path", None, message)],
            ));
        }
    };
    if input.expected_project_root != store.project_root
        || input.observed_manifest.root != store.project_root
    {
        let message = format!(
            "Reconcilierea a refuzat un proiect stale: request={}, manifest={}, sesiune={}.",
            input.expected_project_root, input.observed_manifest.root, store.project_root
        );
        return CleanExternalReconcilePlanResult::Terminal(terminal_receipt(
            operation_id,
            runtime_session_id.clone(),
            store.project_root.clone(),
            started_at_ms,
            KernelExternalDiskReconcileStatus::StaleEvidence,
            message.clone(),
            requested_paths.clone(),
            requested_paths.clone(),
            Vec::new(),
            requested_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::StaleEvidence,
                        &message,
                    )
                })
                .collect(),
            vec![diagnostic("project_root_mismatch", None, message)],
        ));
    }

    if input.observed_manifest.truncated {
        let message = "Manifestul extern este trunchiat; reconcilierea automată nu poate demonstra batch-ul complet.".to_string();
        return CleanExternalReconcilePlanResult::Terminal(terminal_receipt(
            operation_id,
            runtime_session_id.clone(),
            store.project_root.clone(),
            started_at_ms,
            KernelExternalDiskReconcileStatus::Blocked,
            message.clone(),
            requested_paths.clone(),
            requested_paths.clone(),
            Vec::new(),
            requested_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::Blocked,
                        &message,
                    )
                })
                .collect(),
            vec![diagnostic("manifest_truncated", None, message)],
        ));
    }

    let max_paths = store.limits.max_files.min(MAX_RECONCILE_PATHS);
    if requested_paths.len() > max_paths {
        let message = format!(
            "Batch-ul extern are {} path-uri, peste limita de {}.",
            requested_paths.len(),
            max_paths
        );
        return CleanExternalReconcilePlanResult::Terminal(terminal_receipt(
            operation_id,
            runtime_session_id.clone(),
            store.project_root.clone(),
            started_at_ms,
            KernelExternalDiskReconcileStatus::Blocked,
            message.clone(),
            requested_paths.clone(),
            requested_paths.clone(),
            Vec::new(),
            Vec::new(),
            vec![diagnostic("batch_limit_exceeded", None, message)],
        ));
    }

    let dirty_paths = store
        .files
        .values()
        .filter(|entry| entry.draft.is_some())
        .map(|entry| entry.relative_path.clone())
        .collect::<Vec<_>>();
    if !dirty_paths.is_empty() {
        let message = format!(
            "FileBufferStore are {} draft(uri); auto-reconcile este all-or-nothing și rămâne blocat.",
            dirty_paths.len()
        );
        return CleanExternalReconcilePlanResult::Terminal(terminal_receipt(
            operation_id,
            runtime_session_id.clone(),
            store.project_root.clone(),
            started_at_ms,
            KernelExternalDiskReconcileStatus::Blocked,
            message.clone(),
            requested_paths.clone(),
            dirty_paths.clone(),
            Vec::new(),
            dirty_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::Blocked,
                        &message,
                    )
                })
                .collect(),
            vec![diagnostic("file_buffer_draft_present", None, message)],
        ));
    }

    let manifest_entries = input
        .observed_manifest
        .files
        .iter()
        .map(|entry| (entry.relative_path.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut reload_paths = Vec::new();
    let mut targets = Vec::with_capacity(requested_paths.len());
    for relative_path in &requested_paths {
        let Some(entry) = store.files.get(relative_path) else {
            reload_paths.push(relative_path.clone());
            continue;
        };
        let Some(expected_manifest) = manifest_entries.get(relative_path.as_str()) else {
            reload_paths.push(relative_path.clone());
            continue;
        };
        targets.push(CleanExternalReconcileTarget {
            relative_path: relative_path.clone(),
            revision: entry.revision,
            baseline: entry.baseline.clone(),
            current_hash: entry.current_hash(),
            expected_manifest: (*expected_manifest).clone(),
        });
    }

    if !reload_paths.is_empty() {
        let message = "Schimbarea externă adaugă, șterge sau atinge fișiere text neurmărite; este necesar full reload autoritar.".to_string();
        return CleanExternalReconcilePlanResult::Terminal(terminal_receipt(
            operation_id,
            runtime_session_id.clone(),
            store.project_root.clone(),
            started_at_ms,
            KernelExternalDiskReconcileStatus::ReloadRequired,
            message.clone(),
            requested_paths.clone(),
            Vec::new(),
            reload_paths.clone(),
            reload_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::ReloadRequired,
                        &message,
                    )
                })
                .collect(),
            vec![diagnostic("project_topology_changed", None, message)],
        ));
    }

    let baseline_total_bytes = store.files.values().fold(0u64, |total, entry| {
        total.saturating_add(entry.baseline_text.len() as u64)
    });
    CleanExternalReconcilePlanResult::Ready(CleanExternalReconcilePlan {
        operation_id,
        session_id: runtime_session_id,
        store_session_id: store.session_id.clone(),
        project_root: store.project_root.clone(),
        started_at_ms,
        requested_paths,
        active_relative_path: input.active_relative_path,
        observed_manifest: input.observed_manifest,
        targets,
        store_version: store_version(store),
        store_loaded_at_ms: store.loaded_at_ms,
        limits: store.limits.clone(),
        baseline_total_bytes,
    })
}

pub(crate) fn read_clean_external_reconcile_plan(
    plan: CleanExternalReconcilePlan,
    completed_at_ms: u128,
) -> CleanExternalReconcileReadResult {
    let mut staged_items = Vec::with_capacity(plan.targets.len());
    let mut total_bytes_read = 0u64;
    let mut next_total_bytes = plan.baseline_total_bytes;

    for target in &plan.targets {
        let read = read_project_disk_text_snapshot(
            std::path::Path::new(&plan.project_root),
            &target.relative_path,
            &plan.limits,
        );
        let snapshot = match read {
            ProjectDiskTextReadOutcome::Loaded(snapshot) => snapshot,
            ProjectDiskTextReadOutcome::Missing => {
                return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                    &plan,
                    completed_at_ms,
                    target,
                    KernelExternalDiskReconcileStatus::StaleEvidence,
                    KernelExternalDiskReconcileItemOutcome::StaleEvidence,
                    "disk_missing_after_manifest",
                    "Fișierul a dispărut după manifest; evidența externă este stale.",
                ));
            }
            ProjectDiskTextReadOutcome::NotFile => {
                return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                    &plan,
                    completed_at_ms,
                    target,
                    KernelExternalDiskReconcileStatus::Blocked,
                    KernelExternalDiskReconcileItemOutcome::Blocked,
                    "disk_target_not_file",
                    "Target-ul extern nu mai este fișier text.",
                ));
            }
            ProjectDiskTextReadOutcome::Oversized(bytes) => {
                return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                    &plan,
                    completed_at_ms,
                    target,
                    KernelExternalDiskReconcileStatus::Blocked,
                    KernelExternalDiskReconcileItemOutcome::Blocked,
                    "disk_target_oversized",
                    format!("Fișierul extern are {bytes} bytes și depășește limita bounded."),
                ));
            }
            ProjectDiskTextReadOutcome::InvalidPath(message)
            | ProjectDiskTextReadOutcome::UnsafePath(message) => {
                return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                    &plan,
                    completed_at_ms,
                    target,
                    KernelExternalDiskReconcileStatus::Blocked,
                    KernelExternalDiskReconcileItemOutcome::Blocked,
                    "disk_target_unsafe_path",
                    message,
                ));
            }
            ProjectDiskTextReadOutcome::Unstable(message) => {
                return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                    &plan,
                    completed_at_ms,
                    target,
                    KernelExternalDiskReconcileStatus::StaleEvidence,
                    KernelExternalDiskReconcileItemOutcome::StaleEvidence,
                    "disk_target_unstable",
                    message,
                ));
            }
            ProjectDiskTextReadOutcome::Unreadable(message) => {
                return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                    &plan,
                    completed_at_ms,
                    target,
                    KernelExternalDiskReconcileStatus::Blocked,
                    KernelExternalDiskReconcileItemOutcome::Blocked,
                    "disk_target_unreadable",
                    message,
                ));
            }
        };

        if snapshot.baseline.modified_ms != target.expected_manifest.modified_ms
            || snapshot.baseline.size != target.expected_manifest.size
            || snapshot.version_token != target.expected_manifest.version_token
        {
            return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                &plan,
                completed_at_ms,
                target,
                KernelExternalDiskReconcileStatus::StaleEvidence,
                KernelExternalDiskReconcileItemOutcome::StaleEvidence,
                "manifest_file_evidence_stale",
                "Metadata citită nu mai corespunde manifestului observat de monitor.",
            ));
        }

        total_bytes_read = total_bytes_read.saturating_add(snapshot.text.len() as u64);
        if total_bytes_read > plan.limits.max_total_bytes {
            return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                &plan,
                completed_at_ms,
                target,
                KernelExternalDiskReconcileStatus::Blocked,
                KernelExternalDiskReconcileItemOutcome::Blocked,
                "batch_bytes_exceeded",
                "Batch-ul extern depășește limita totală de memorie a FileBufferStore.",
            ));
        }

        next_total_bytes = next_total_bytes
            .saturating_sub(target.baseline.size)
            .saturating_add(snapshot.text.len() as u64);
        if next_total_bytes > plan.limits.max_total_bytes {
            return CleanExternalReconcileReadResult::Terminal(read_terminal_receipt(
                &plan,
                completed_at_ms,
                target,
                KernelExternalDiskReconcileStatus::Blocked,
                KernelExternalDiskReconcileItemOutcome::Blocked,
                "store_bytes_exceeded",
                "Baseline-urile reconciliate ar depăși limita totală FileBufferStore.",
            ));
        }

        let outcome = if snapshot.baseline.hash != target.baseline.hash {
            KernelExternalDiskReconcileItemOutcome::ContentRebased
        } else if snapshot.baseline != target.baseline {
            KernelExternalDiskReconcileItemOutcome::MetadataRefreshed
        } else {
            KernelExternalDiskReconcileItemOutcome::Unchanged
        };
        staged_items.push(StagedItem {
            target: target.clone(),
            text: snapshot.text,
            disk_baseline: snapshot.baseline,
            outcome,
        });
    }

    let content_rebased_count = staged_items
        .iter()
        .filter(|item| item.outcome == KernelExternalDiskReconcileItemOutcome::ContentRebased)
        .count();
    let metadata_refreshed_count = staged_items
        .iter()
        .filter(|item| item.outcome == KernelExternalDiskReconcileItemOutcome::MetadataRefreshed)
        .count();
    let unchanged_count = staged_items
        .iter()
        .filter(|item| item.outcome == KernelExternalDiskReconcileItemOutcome::Unchanged)
        .count();

    CleanExternalReconcileReadResult::Staged(CleanExternalReconcileStaged {
        plan,
        items: staged_items,
        total_bytes_read,
        content_rebased_count,
        metadata_refreshed_count,
        unchanged_count,
    })
}

pub(crate) fn commit_clean_external_reconcile(
    store: &mut FileBufferStore,
    staged: CleanExternalReconcileStaged,
    completed_at_ms: u128,
) -> KernelExternalDiskReconcileReceipt {
    let plan = &staged.plan;
    if store.session_id != plan.store_session_id
        || store.project_root != plan.project_root
        || store.loaded_at_ms != plan.store_loaded_at_ms
        || store_version(store) != plan.store_version
        || store.files.values().any(|entry| entry.draft.is_some())
    {
        let message =
            "FileBufferStore s-a modificat între plan și commit; CAS a blocat întregul batch.";
        let mut receipt = terminal_receipt(
            plan.operation_id.clone(),
            plan.session_id.clone(),
            store.project_root.clone(),
            plan.started_at_ms,
            KernelExternalDiskReconcileStatus::StaleEvidence,
            message.to_string(),
            plan.requested_paths.clone(),
            plan.requested_paths.clone(),
            Vec::new(),
            plan.requested_paths
                .iter()
                .map(|path| {
                    terminal_item(
                        path,
                        KernelExternalDiskReconcileItemOutcome::StaleEvidence,
                        message,
                    )
                })
                .collect(),
            vec![diagnostic("file_buffer_cas_failed", None, message)],
        );
        receipt.completed_at_ms = completed_at_ms;
        return receipt;
    }

    let mut items = Vec::with_capacity(staged.items.len());
    let mut effective_paths = Vec::new();
    let mut invalidated_paths = Vec::new();
    for staged_item in staged.items {
        let entry = store
            .files
            .get_mut(&staged_item.target.relative_path)
            .expect("CAS validated every staged FileBufferStore entry");
        let before_revision = entry.revision;
        let before_baseline = entry.baseline.clone();
        let before_current_hash = entry.current_hash();
        if staged_item.outcome != KernelExternalDiskReconcileItemOutcome::Unchanged {
            entry.baseline = staged_item.disk_baseline.clone();
            entry.baseline_text = staged_item.text;
            entry.draft = None;
            entry.revision = entry.revision.saturating_add(1);
            effective_paths.push(entry.relative_path.clone());
            if staged_item.outcome == KernelExternalDiskReconcileItemOutcome::ContentRebased {
                invalidated_paths.push(entry.relative_path.clone());
            }
        }
        items.push(KernelExternalDiskReconcileItemReceipt {
            relative_path: entry.relative_path.clone(),
            outcome: staged_item.outcome,
            before_revision: Some(before_revision),
            after_revision: Some(entry.revision),
            before_baseline: Some(before_baseline),
            observed_disk_baseline: Some(staged_item.disk_baseline),
            before_current_hash: Some(before_current_hash),
            after_current_hash: Some(entry.current_hash()),
            diagnostic: None,
        });
    }

    let status = if effective_paths.is_empty() {
        KernelExternalDiskReconcileStatus::Noop
    } else {
        KernelExternalDiskReconcileStatus::Applied
    };
    // A successful receipt also acts as an idempotent projection barrier. All
    // requested paths are invalidated so a frontend projection that failed
    // after a prior Rust commit can safely retry on the next monitor pass.
    let invalidated_paths = plan.requested_paths.clone();
    let preview = invalidated_paths.iter().any(|path| preview_relevant(path));
    let page_js = invalidated_paths.iter().any(|path| path.ends_with(".js"));
    let scss = invalidated_paths
        .iter()
        .any(|path| path.ends_with(".scss") || path.ends_with(".css"));
    let active_file = plan
        .active_relative_path
        .as_deref()
        .and_then(|active| store.text_snapshot(active));
    let selection = active_file.is_some();
    let content_changed = staged.content_rebased_count > 0;

    KernelExternalDiskReconcileReceipt {
        schema_version: KERNEL_EXTERNAL_DISK_RECONCILE_SCHEMA_VERSION,
        operation_id: plan.operation_id.clone(),
        session_id: plan.session_id.clone(),
        project_root: plan.project_root.clone(),
        status,
        verdict_reason: if status == KernelExternalDiskReconcileStatus::Applied {
            format!(
                "{} conținut(uri) și {} baseline-uri metadata au fost reconciliate atomic.",
                staged.content_rebased_count, staged.metadata_refreshed_count
            )
        } else {
            "FileBufferStore corespundea deja conținutului observat pe disk.".to_string()
        },
        started_at_ms: plan.started_at_ms,
        completed_at_ms,
        requested_count: plan.requested_paths.len(),
        target_count: plan.targets.len(),
        reconciled_count: staged.content_rebased_count,
        metadata_refreshed_count: staged.metadata_refreshed_count,
        unchanged_count: staged.unchanged_count,
        total_bytes_read: staged.total_bytes_read,
        requested_paths: plan.requested_paths.clone(),
        effective_paths,
        invalidated_paths,
        blocked_paths: Vec::new(),
        reload_required_paths: Vec::new(),
        history_invalidated: false,
        source_graph_invalidated: false,
        active_file,
        accepted_manifest: Some(plan.observed_manifest.clone()),
        accepted_disk_generation: None,
        workspace_revision: None,
        projection_hints: KernelExternalDiskProjectionHints {
            project_rescan: content_changed,
            source_graph: content_changed,
            preview,
            page_js,
            scss,
            history: false,
            selection,
        },
        items,
        diagnostics: Vec::new(),
    }
}

fn store_version(store: &FileBufferStore) -> Vec<StoreVersionEntry> {
    store
        .files
        .values()
        .map(|entry| StoreVersionEntry {
            relative_path: entry.relative_path.clone(),
            revision: entry.revision,
            baseline_hash: entry.baseline.hash.clone(),
            current_hash: entry.current_hash(),
            has_draft: entry.draft.is_some(),
        })
        .collect()
}

fn read_terminal_receipt(
    plan: &CleanExternalReconcilePlan,
    completed_at_ms: u128,
    target: &CleanExternalReconcileTarget,
    status: KernelExternalDiskReconcileStatus,
    outcome: KernelExternalDiskReconcileItemOutcome,
    code: &str,
    message: impl Into<String>,
) -> KernelExternalDiskReconcileReceipt {
    let message = message.into();
    let mut receipt = terminal_receipt(
        plan.operation_id.clone(),
        plan.session_id.clone(),
        plan.project_root.clone(),
        plan.started_at_ms,
        status,
        message.clone(),
        plan.requested_paths.clone(),
        vec![target.relative_path.clone()],
        Vec::new(),
        vec![KernelExternalDiskReconcileItemReceipt {
            relative_path: target.relative_path.clone(),
            outcome,
            before_revision: Some(target.revision),
            after_revision: Some(target.revision),
            before_baseline: Some(target.baseline.clone()),
            observed_disk_baseline: None,
            before_current_hash: Some(target.current_hash.clone()),
            after_current_hash: Some(target.current_hash.clone()),
            diagnostic: Some(message.clone()),
        }],
        vec![diagnostic(
            code,
            Some(target.relative_path.clone()),
            message,
        )],
    );
    receipt.completed_at_ms = completed_at_ms;
    receipt
}

#[allow(clippy::too_many_arguments)]
fn terminal_receipt(
    operation_id: String,
    session_id: String,
    project_root: String,
    started_at_ms: u128,
    status: KernelExternalDiskReconcileStatus,
    verdict_reason: String,
    requested_paths: Vec<String>,
    affected_paths: Vec<String>,
    reload_required_paths: Vec<String>,
    items: Vec<KernelExternalDiskReconcileItemReceipt>,
    diagnostics: Vec<KernelExternalDiskReconcileDiagnostic>,
) -> KernelExternalDiskReconcileReceipt {
    let blocked_paths = if status == KernelExternalDiskReconcileStatus::Blocked
        || status == KernelExternalDiskReconcileStatus::StaleEvidence
    {
        affected_paths
    } else {
        Vec::new()
    };
    KernelExternalDiskReconcileReceipt {
        schema_version: KERNEL_EXTERNAL_DISK_RECONCILE_SCHEMA_VERSION,
        operation_id,
        session_id,
        project_root,
        status,
        verdict_reason,
        started_at_ms,
        completed_at_ms: started_at_ms,
        requested_count: requested_paths.len(),
        target_count: items.len(),
        reconciled_count: 0,
        metadata_refreshed_count: 0,
        unchanged_count: 0,
        total_bytes_read: 0,
        requested_paths,
        effective_paths: Vec::new(),
        invalidated_paths: Vec::new(),
        blocked_paths,
        reload_required_paths,
        history_invalidated: false,
        source_graph_invalidated: false,
        active_file: None,
        accepted_manifest: None,
        accepted_disk_generation: None,
        workspace_revision: None,
        projection_hints: KernelExternalDiskProjectionHints::none(),
        items,
        diagnostics,
    }
}

fn terminal_item(
    relative_path: &str,
    outcome: KernelExternalDiskReconcileItemOutcome,
    message: &str,
) -> KernelExternalDiskReconcileItemReceipt {
    KernelExternalDiskReconcileItemReceipt {
        relative_path: relative_path.to_string(),
        outcome,
        before_revision: None,
        after_revision: None,
        before_baseline: None,
        observed_disk_baseline: None,
        before_current_hash: None,
        after_current_hash: None,
        diagnostic: Some(message.to_string()),
    }
}

fn diagnostic(
    code: impl Into<String>,
    relative_path: Option<String>,
    message: impl Into<String>,
) -> KernelExternalDiskReconcileDiagnostic {
    KernelExternalDiskReconcileDiagnostic {
        code: code.into(),
        relative_path,
        message: message.into(),
        blocking: true,
    }
}

fn normalize_requested_paths(paths: &[String]) -> Result<Vec<String>, String> {
    let mut normalized = BTreeSet::new();
    for path in paths {
        let path = path.trim();
        if path.is_empty()
            || path.contains('\0')
            || path.contains('\\')
            || path.starts_with('/')
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == ".." || part.contains(':'))
        {
            return Err(format!("Path extern invalid pentru reconcile: {path:?}."));
        }
        normalized.insert(path.to_string());
    }
    Ok(normalized.into_iter().collect())
}

fn normalized_paths_best_effort(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn operation_id(session_id: &str, started_at_ms: u128) -> String {
    let serial = RECONCILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("external-reconcile-{session_id}-{started_at_ms}-{serial}")
}

fn preview_relevant(path: &str) -> bool {
    path == "zola.toml"
        || path == "config.toml"
        || path.starts_with("content/")
        || path.starts_with("templates/")
        || path.starts_with("themes/")
        || path.starts_with("sass/")
        || path.starts_with("static/")
        || path.ends_with(".html")
        || path.ends_with(".css")
        || path.ends_with(".scss")
        || path.ends_with(".js")
        || path.ends_with(".md")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::UNIX_EPOCH};

    use crate::{
        kernel::file_buffer_store::{
            hash_text, FileBufferEntry, TextBufferLanguage, TextBufferRole,
        },
        project::{ProjectDiskManifest, ProjectDiskManifestEntry},
    };

    use super::*;

    #[test]
    fn clean_external_content_is_rebased_atomically() {
        let root = test_root("content-rebase");
        write_text(&root, "templates/index.html", "old");
        let mut store = store_with_file(&root, "templates/index.html", "old");
        write_text(&root, "templates/index.html", "new");
        let input = input_for(&root, &["templates/index.html"]);

        let plan = ready(plan_clean_external_reconcile(&store, input, 10));
        let staged = staged(read_clean_external_reconcile_plan(plan, 11));
        assert!(staged.invalidates_history());
        let receipt = commit_clean_external_reconcile(&mut store, staged, 12);

        assert_eq!(receipt.status, KernelExternalDiskReconcileStatus::Applied);
        assert_eq!(receipt.reconciled_count, 1);
        assert_eq!(receipt.invalidated_paths, vec!["templates/index.html"]);
        assert_eq!(
            store.text_for("templates/index.html").as_deref(),
            Some("new")
        );
        assert_eq!(store.files["templates/index.html"].revision, 2);
        assert_eq!(
            receipt.active_file.as_ref().map(|file| file.text.as_str()),
            Some("new")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn identical_disk_content_is_an_idempotent_noop_with_active_snapshot() {
        let root = test_root("noop");
        write_text(&root, "templates/index.html", "same");
        let mut store = store_with_file(&root, "templates/index.html", "same");
        let plan = ready(plan_clean_external_reconcile(
            &store,
            input_for(&root, &["templates/index.html"]),
            10,
        ));
        let staged = staged(read_clean_external_reconcile_plan(plan, 11));

        let receipt = commit_clean_external_reconcile(&mut store, staged, 12);

        assert_eq!(receipt.status, KernelExternalDiskReconcileStatus::Noop);
        assert_eq!(receipt.unchanged_count, 1);
        assert_eq!(store.files["templates/index.html"].revision, 1);
        assert_eq!(
            receipt.active_file.as_ref().map(|file| file.text.as_str()),
            Some("same")
        );
        assert_eq!(receipt.invalidated_paths, vec!["templates/index.html"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn readonly_metadata_is_refreshed_without_changing_text_or_history() {
        use std::os::unix::fs::PermissionsExt;

        let root = test_root("readonly-metadata");
        write_text(&root, "templates/index.html", "same");
        let mut store = store_with_file(&root, "templates/index.html", "same");
        let path = root.join("templates/index.html");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(&path, permissions).unwrap();
        let plan = ready(plan_clean_external_reconcile(
            &store,
            input_for(&root, &["templates/index.html"]),
            10,
        ));
        let staged = staged(read_clean_external_reconcile_plan(plan, 11));
        assert!(!staged.invalidates_history());

        let receipt = commit_clean_external_reconcile(&mut store, staged, 12);

        assert_eq!(receipt.status, KernelExternalDiskReconcileStatus::Applied);
        assert_eq!(receipt.metadata_refreshed_count, 1);
        assert!(store.files["templates/index.html"].baseline.readonly);
        assert_eq!(
            store.text_for("templates/index.html").as_deref(),
            Some("same")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manifest_changed_after_plan_returns_stale_evidence() {
        let root = test_root("manifest-stale");
        write_text(&root, "templates/index.html", "old");
        let store = store_with_file(&root, "templates/index.html", "old");
        let input = input_for(&root, &["templates/index.html"]);
        let plan = ready(plan_clean_external_reconcile(&store, input, 10));
        write_text(&root, "templates/index.html", "new-content");

        let receipt = terminal_read(read_clean_external_reconcile_plan(plan, 11));

        assert_eq!(
            receipt.status,
            KernelExternalDiskReconcileStatus::StaleEvidence
        );
        assert!(receipt
            .diagnostics
            .iter()
            .any(|item| item.code == "manifest_file_evidence_stale"));

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn version_token_rejects_same_size_rewrite_with_restored_mtime() {
        let root = test_root("manifest-version-token");
        write_text(&root, "templates/index.html", "old");
        let store = store_with_file(&root, "templates/index.html", "old");
        write_text(&root, "templates/index.html", "new");
        let path = root.join("templates/index.html");
        let observed_modified = fs::metadata(&path).unwrap().modified().unwrap();
        let input = input_for(&root, &["templates/index.html"]);
        let expected_entry = input.observed_manifest.files[0].clone();
        let plan = ready(plan_clean_external_reconcile(&store, input, 10));

        write_text(&root, "templates/index.html", "alt");
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_times(fs::FileTimes::new().set_modified(observed_modified))
            .unwrap();
        let current_entry = manifest_entry("templates/index.html", &fs::metadata(&path).unwrap());
        assert_eq!(current_entry.size, expected_entry.size);
        assert_eq!(current_entry.modified_ms, expected_entry.modified_ms);
        assert_ne!(current_entry.version_token, expected_entry.version_token);

        let receipt = terminal_read(read_clean_external_reconcile_plan(plan, 11));
        assert_eq!(
            receipt.status,
            KernelExternalDiskReconcileStatus::StaleEvidence
        );
        assert!(receipt
            .diagnostics
            .iter()
            .any(|item| item.code == "manifest_file_evidence_stale"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_and_non_utf8_files_block_without_partial_commit() {
        let root = test_root("bounded-read");
        write_text(&root, "templates/index.html", "old");
        let store = store_with_file(&root, "templates/index.html", "old");

        write_text(&root, "templates/index.html", &"x".repeat(1025));
        let oversized_plan = ready(plan_clean_external_reconcile(
            &store,
            input_for(&root, &["templates/index.html"]),
            10,
        ));
        let oversized = terminal_read(read_clean_external_reconcile_plan(oversized_plan, 11));
        assert_eq!(oversized.status, KernelExternalDiskReconcileStatus::Blocked);
        assert!(oversized
            .diagnostics
            .iter()
            .any(|item| item.code == "disk_target_oversized"));

        fs::write(root.join("templates/index.html"), [0xff, 0xfe]).unwrap();
        let utf8_plan = ready(plan_clean_external_reconcile(
            &store,
            input_for(&root, &["templates/index.html"]),
            12,
        ));
        let invalid_utf8 = terminal_read(read_clean_external_reconcile_plan(utf8_plan, 13));
        assert_eq!(
            invalid_utf8.status,
            KernelExternalDiskReconcileStatus::Blocked
        );
        assert!(invalid_utf8
            .diagnostics
            .iter()
            .any(|item| item.code == "disk_target_unreadable"));
        assert_eq!(
            store.text_for("templates/index.html").as_deref(),
            Some("old")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_paths_are_deduplicated_deterministically() {
        let root = test_root("dedupe");
        write_text(&root, "templates/index.html", "old");
        let mut store = store_with_file(&root, "templates/index.html", "old");
        write_text(&root, "templates/index.html", "new");
        let mut input = input_for(&root, &["templates/index.html"]);
        input.relative_paths = vec![
            "templates/index.html".to_string(),
            "templates/index.html".to_string(),
        ];
        let plan = ready(plan_clean_external_reconcile(&store, input, 10));
        let staged = staged(read_clean_external_reconcile_plan(plan, 11));

        let receipt = commit_clean_external_reconcile(&mut store, staged, 12);

        assert_eq!(receipt.requested_count, 1);
        assert_eq!(receipt.reconciled_count, 1);
        assert_eq!(receipt.requested_paths, vec!["templates/index.html"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn session_identity_change_between_read_and_commit_fails_cas() {
        let root = test_root("session-cas");
        write_text(&root, "templates/index.html", "old");
        let mut store = store_with_file(&root, "templates/index.html", "old");
        write_text(&root, "templates/index.html", "new");
        let plan = ready(plan_clean_external_reconcile(
            &store,
            input_for(&root, &["templates/index.html"]),
            10,
        ));
        let staged = staged(read_clean_external_reconcile_plan(plan, 11));
        store.session_id = "session-2".to_string();

        let receipt = commit_clean_external_reconcile(&mut store, staged, 12);

        assert_eq!(
            receipt.status,
            KernelExternalDiskReconcileStatus::StaleEvidence
        );
        assert_eq!(
            store.text_for("templates/index.html").as_deref(),
            Some("old")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mixed_batch_with_missing_file_requires_reload_without_mutation() {
        let root = test_root("missing-batch");
        write_text(&root, "templates/a.html", "old-a");
        write_text(&root, "templates/b.html", "old-b");
        let mut store = store_with_file(&root, "templates/a.html", "old-a");
        store.insert_loaded_file(entry(&root, "templates/b.html", "old-b"));
        write_text(&root, "templates/a.html", "new-a");
        fs::remove_file(root.join("templates/b.html")).unwrap();
        let input = input_for(&root, &["templates/a.html", "templates/b.html"]);

        let receipt = terminal(plan_clean_external_reconcile(&store, input, 10));

        assert_eq!(
            receipt.status,
            KernelExternalDiskReconcileStatus::ReloadRequired
        );
        assert_eq!(store.text_for("templates/a.html").as_deref(), Some("old-a"));
        assert_eq!(store.files["templates/a.html"].revision, 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn draft_anywhere_blocks_entire_batch() {
        let root = test_root("dirty-block");
        write_text(&root, "templates/a.html", "a");
        write_text(&root, "templates/b.html", "b");
        let mut store = store_with_file(&root, "templates/a.html", "a");
        store.insert_loaded_file(entry(&root, "templates/b.html", "b"));
        store
            .set_draft("templates/b.html", "draft".to_string(), 2)
            .unwrap();
        write_text(&root, "templates/a.html", "new-a");

        let receipt = terminal(plan_clean_external_reconcile(
            &store,
            input_for(&root, &["templates/a.html"]),
            10,
        ));

        assert_eq!(receipt.status, KernelExternalDiskReconcileStatus::Blocked);
        assert_eq!(store.text_for("templates/a.html").as_deref(), Some("a"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cas_blocks_draft_created_after_disk_read() {
        let root = test_root("cas-draft");
        write_text(&root, "templates/index.html", "old");
        let mut store = store_with_file(&root, "templates/index.html", "old");
        write_text(&root, "templates/index.html", "new");
        let plan = ready(plan_clean_external_reconcile(
            &store,
            input_for(&root, &["templates/index.html"]),
            10,
        ));
        let staged = staged(read_clean_external_reconcile_plan(plan, 11));
        store
            .set_draft("templates/index.html", "local".to_string(), 12)
            .unwrap();

        let receipt = commit_clean_external_reconcile(&mut store, staged, 13);

        assert_eq!(
            receipt.status,
            KernelExternalDiskReconcileStatus::StaleEvidence
        );
        assert_eq!(
            store.text_for("templates/index.html").as_deref(),
            Some("local")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_leaf_is_blocked_without_following_outside_project() {
        use std::os::unix::fs::symlink;

        let root = test_root("symlink-leaf");
        let outside = test_root("symlink-outside");
        write_text(&root, "templates/index.html", "old");
        write_text(&outside, "secret.html", "secret");
        let store = store_with_file(&root, "templates/index.html", "old");
        fs::remove_file(root.join("templates/index.html")).unwrap();
        symlink(
            outside.join("secret.html"),
            root.join("templates/index.html"),
        )
        .unwrap();
        let mut input = input_for(&root, &["templates/index.html"]);
        let metadata = fs::metadata(root.join("templates/index.html")).unwrap();
        input.observed_manifest.files = vec![manifest_entry("templates/index.html", &metadata)];
        let plan = ready(plan_clean_external_reconcile(&store, input, 10));

        let receipt = terminal_read(read_clean_external_reconcile_plan(plan, 11));

        assert_eq!(receipt.status, KernelExternalDiskReconcileStatus::Blocked);
        assert!(receipt
            .diagnostics
            .iter()
            .any(|item| item.code == "disk_target_unsafe_path"));

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    fn ready(result: CleanExternalReconcilePlanResult) -> CleanExternalReconcilePlan {
        match result {
            CleanExternalReconcilePlanResult::Ready(plan) => plan,
            CleanExternalReconcilePlanResult::Terminal(receipt) => {
                panic!("expected ready plan, got {:?}", receipt.status)
            }
        }
    }

    fn terminal(result: CleanExternalReconcilePlanResult) -> KernelExternalDiskReconcileReceipt {
        match result {
            CleanExternalReconcilePlanResult::Terminal(receipt) => receipt,
            CleanExternalReconcilePlanResult::Ready(_) => panic!("expected terminal receipt"),
        }
    }

    fn staged(result: CleanExternalReconcileReadResult) -> CleanExternalReconcileStaged {
        match result {
            CleanExternalReconcileReadResult::Staged(staged) => staged,
            CleanExternalReconcileReadResult::Terminal(receipt) => {
                panic!("expected staged read, got {:?}", receipt.status)
            }
        }
    }

    fn terminal_read(
        result: CleanExternalReconcileReadResult,
    ) -> KernelExternalDiskReconcileReceipt {
        match result {
            CleanExternalReconcileReadResult::Terminal(receipt) => receipt,
            CleanExternalReconcileReadResult::Staged(_) => panic!("expected terminal read"),
        }
    }

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "pana-external-reconcile-{label}-{}",
            super::RECONCILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root.canonicalize().unwrap()
    }

    fn store_with_file(root: &std::path::Path, relative_path: &str, text: &str) -> FileBufferStore {
        let mut store = FileBufferStore::new(
            "session-1",
            root.to_string_lossy(),
            1,
            FileBufferStoreLimits {
                max_files: 20,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        );
        store.insert_loaded_file(entry(root, relative_path, text));
        store
    }

    fn entry(root: &std::path::Path, relative_path: &str, text: &str) -> FileBufferEntry {
        let metadata = fs::metadata(root.join(relative_path)).unwrap();
        FileBufferEntry {
            relative_path: relative_path.to_string(),
            absolute_path: root.join(relative_path).to_string_lossy().to_string(),
            language: TextBufferLanguage::Html,
            role: TextBufferRole::Template,
            baseline: FileBufferBaseline {
                hash: hash_text(text),
                modified_ms: modified_ms(&metadata),
                size: metadata.len(),
                readonly: metadata.permissions().readonly(),
            },
            baseline_text: text.to_string(),
            draft: None,
            revision: 1,
        }
    }

    fn input_for(root: &std::path::Path, paths: &[&str]) -> KernelExternalDiskReconcileInput {
        let files = paths
            .iter()
            .filter_map(|relative_path| {
                fs::metadata(root.join(relative_path))
                    .ok()
                    .map(|metadata| manifest_entry(relative_path, &metadata))
            })
            .collect();
        KernelExternalDiskReconcileInput {
            expected_project_root: root.to_string_lossy().to_string(),
            expected_session_id: "session-1".to_string(),
            observed_manifest: ProjectDiskManifest {
                root: root.to_string_lossy().to_string(),
                files,
                truncated: false,
                max_files: 1000,
            },
            relative_paths: paths.iter().map(|path| (*path).to_string()).collect(),
            active_relative_path: paths.first().map(|path| (*path).to_string()),
        }
    }

    fn manifest_entry(relative_path: &str, metadata: &fs::Metadata) -> ProjectDiskManifestEntry {
        ProjectDiskManifestEntry {
            relative_path: relative_path.to_string(),
            modified_ms: modified_ms(metadata),
            size: metadata.len(),
            version_token: crate::project::project_disk_metadata_version_token(metadata),
        }
    }

    fn modified_ms(metadata: &fs::Metadata) -> u128 {
        metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    }

    fn write_text(root: &std::path::Path, relative_path: &str, text: &str) {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}
