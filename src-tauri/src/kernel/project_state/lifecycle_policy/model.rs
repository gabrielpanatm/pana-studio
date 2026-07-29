use serde::{Deserialize, Serialize};

use crate::{
    kernel::project_state::model::{
        KernelProjectStateReason, KernelProjectStateSnapshot, KernelProjectStateStatus,
    },
    localization::LocalizedDiagnostic,
};

pub const KERNEL_PROJECT_TRANSITION_POLICY_SCHEMA_VERSION: u32 = 4;
pub const KERNEL_PROJECT_TRANSITION_POLICY_MATRIX_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionAction {
    OpenProject,
    ReloadProject,
    CloseProject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionDecision {
    Allow,
    Confirm,
    Block,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelProjectTransitionReason {
    NoOpenProject,
    Clean,
    MetadataChanged,
    WorkspaceDirty,
    DiskConflict,
    BlockedProjectState,
    UnknownWarning,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionPolicy {
    pub schema_version: u32,
    pub action: KernelProjectTransitionAction,
    pub decision: KernelProjectTransitionDecision,
    pub reason: KernelProjectTransitionReason,
    pub project_state_status: KernelProjectStateStatus,
    pub project_state_reason: KernelProjectStateReason,
    pub project_root: Option<String>,
    pub session_id: Option<String>,
    pub requires_operator_confirmation: bool,
    pub blocks_transition: bool,
    pub workspace_dirty_resource_count: usize,
    pub workspace_revision: Option<u64>,
    pub workspace_undo_count: usize,
    pub workspace_redo_count: usize,
    pub disk_conflict_count: usize,
    pub disk_blocking_count: usize,
    pub metadata_changed_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KernelProjectTransitionPolicyMatrixSnapshot {
    pub schema_version: u32,
    pub project_state: KernelProjectStateSnapshot,
    pub policies: Vec<KernelProjectTransitionPolicy>,
}

impl KernelProjectTransitionPolicy {
    pub fn allows_without_operator(&self) -> bool {
        self.decision == KernelProjectTransitionDecision::Allow
    }

    pub fn guard_error(&self) -> String {
        let code = match self.decision {
            KernelProjectTransitionDecision::Confirm => "project-transition-confirmation-required",
            KernelProjectTransitionDecision::Block => "project-transition-blocked",
            KernelProjectTransitionDecision::Allow => "project-transition-allowed",
        };
        serde_json::to_string(&LocalizedDiagnostic::new(code))
            .unwrap_or_else(|_| format!(r#"{{"schemaVersion":1,"code":"{code}"}}"#))
    }
}
