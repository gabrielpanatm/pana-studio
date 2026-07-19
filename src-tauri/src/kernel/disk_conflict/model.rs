use serde::Serialize;

use crate::kernel::file_buffer_store::{FileBufferBaseline, TextBufferLanguage, TextBufferRole};

pub const KERNEL_DISK_CONFLICT_SCHEMA_VERSION: u32 = 1;
pub const KERNEL_DISK_CONFLICT_GATE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelDiskConflictStatus {
    Clean,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelDiskConflictKind {
    Clean,
    DirtyOnly,
    MetadataChanged,
    DiskChanged,
    MissingOnDisk,
    Readonly,
    NotFile,
    Oversized,
    Unreadable,
    InvalidPath,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelDiskConflictSummary {
    pub status: KernelDiskConflictStatus,
    pub verdict_reason: String,
    pub tracked_file_count: usize,
    pub clean_count: usize,
    pub dirty_only_count: usize,
    pub metadata_changed_count: usize,
    pub disk_changed_count: usize,
    pub missing_on_disk_count: usize,
    pub readonly_count: usize,
    pub not_file_count: usize,
    pub oversized_count: usize,
    pub unreadable_count: usize,
    pub invalid_path_count: usize,
    pub conflict_count: usize,
    pub blocking_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelDiskConflictFileSnapshot {
    pub relative_path: String,
    pub absolute_path: String,
    pub language: TextBufferLanguage,
    pub role: TextBufferRole,
    pub status: KernelDiskConflictStatus,
    pub kind: KernelDiskConflictKind,
    pub message: String,
    pub baseline: FileBufferBaseline,
    pub disk: Option<FileBufferBaseline>,
    pub has_draft: bool,
    pub dirty: bool,
    pub revision: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelDiskConflictSnapshot {
    pub schema_version: u32,
    pub session_id: String,
    pub project_root: String,
    pub scanned_at_ms: u128,
    pub max_file_bytes: u64,
    pub summary: KernelDiskConflictSummary,
    pub files: Vec<KernelDiskConflictFileSnapshot>,
}

#[derive(Clone, Debug)]
pub struct KernelDiskConflictGateRequest {
    pub operation_label: String,
    pub target_paths: Vec<String>,
    pub policy: KernelDiskConflictGatePolicy,
    pub allow_empty_target_paths: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelDiskConflictGatePolicy {
    ProjectFileSaveText,
    ProjectFileUndoCreatedText,
    ProjectFileDeleteText,
    WorkspaceMutationText,
    WorkspaceMutationCurrentBufferText,
    ProjectEntryMove,
    ProjectEntryRename,
    ProjectEntryTrash,
    ProjectEntryRestore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelDiskConflictGateAction {
    Allow,
    AllowWithInfo,
    Block,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelDiskConflictGateDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelDiskConflictGateDecision {
    pub relative_path: String,
    pub action: KernelDiskConflictGateAction,
    pub status: Option<KernelDiskConflictStatus>,
    pub kind: Option<KernelDiskConflictKind>,
    pub code: String,
    pub message: String,
    pub baseline_hash: Option<String>,
    pub disk_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelDiskConflictGateDiagnostic {
    pub severity: KernelDiskConflictGateDiagnosticSeverity,
    pub code: String,
    pub relative_path: Option<String>,
    pub message: String,
    pub blocking: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelDiskConflictGateResult {
    pub schema_version: u32,
    pub operation_label: String,
    pub policy: KernelDiskConflictGatePolicy,
    pub allowed: bool,
    pub target_count: usize,
    pub blocked_target_count: usize,
    pub info_count: usize,
    pub warning_count: usize,
    pub decisions: Vec<KernelDiskConflictGateDecision>,
    pub diagnostics: Vec<KernelDiskConflictGateDiagnostic>,
}

impl KernelDiskConflictGateRequest {
    pub fn project_file_save_text(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::ProjectFileSaveText,
            allow_empty_target_paths: false,
        }
    }

    pub fn project_file_undo_created_text(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::ProjectFileUndoCreatedText,
            allow_empty_target_paths: false,
        }
    }

    pub fn project_file_delete_text(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::ProjectFileDeleteText,
            allow_empty_target_paths: false,
        }
    }

    pub fn workspace_mutation_text(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::WorkspaceMutationText,
            allow_empty_target_paths: false,
        }
    }

    pub fn workspace_mutation_current_buffer_text(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::WorkspaceMutationCurrentBufferText,
            allow_empty_target_paths: false,
        }
    }

    pub fn project_entry_move(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::ProjectEntryMove,
            allow_empty_target_paths: true,
        }
    }

    pub fn project_entry_rename(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::ProjectEntryRename,
            allow_empty_target_paths: true,
        }
    }

    pub fn project_entry_trash(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::ProjectEntryTrash,
            allow_empty_target_paths: true,
        }
    }

    pub fn project_entry_restore(
        operation_label: impl Into<String>,
        target_paths: Vec<String>,
    ) -> Self {
        Self {
            operation_label: operation_label.into(),
            target_paths,
            policy: KernelDiskConflictGatePolicy::ProjectEntryRestore,
            allow_empty_target_paths: false,
        }
    }
}

impl KernelDiskConflictGateResult {
    pub fn blocking_message(&self) -> Option<String> {
        if self.allowed {
            return None;
        }

        let examples = self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.blocking)
            .take(3)
            .map(|diagnostic| match &diagnostic.relative_path {
                Some(relative_path) => format!("{relative_path}: {}", diagnostic.message),
                None => diagnostic.message.clone(),
            })
            .collect::<Vec<_>>();

        Some(format!(
            "Disk Conflict Gate a blocat {} pentru {} target-uri din {}. {}",
            self.operation_label,
            self.blocked_target_count,
            self.target_count,
            examples.join(" ")
        ))
    }
}
