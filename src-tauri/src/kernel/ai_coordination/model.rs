use serde::{Deserialize, Serialize};

pub const AI_COORDINATION_SCHEMA_VERSION: u32 = 2;
/// A lease must be long enough to cover a normal CLI edit round-trip, while
/// remaining short-lived when the AI process disappears without releasing it.
/// Long-running edits are expected to renew before this deadline.
pub const DEFAULT_EDIT_LEASE_TTL_MS: u128 = 120_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiClientIdentity {
    pub session_id: String,
    pub client_name: String,
    #[serde(default)]
    pub client_version: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiPresenceStatus {
    Active,
    Idle,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiClientSessionSnapshot {
    pub session_id: String,
    pub client_name: String,
    pub client_version: Option<String>,
    pub initialized_at_ms: u128,
    pub last_seen_at_ms: u128,
    pub context_revision_seen: Option<u64>,
    pub presence: AiPresenceStatus,
    pub owns_edit_lease: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCoordinationEvidence {
    pub project_session_id: Option<String>,
    pub project_revision: Option<u64>,
    #[serde(default)]
    pub dirty_files: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<ProjectCoordinationBlocker>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCoordinationBlockerKind {
    RecoveryUnavailable,
    RecoveryNeedsAttention,
    RecoveryUnreadable,
    DiskConflict,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCoordinationBlocker {
    pub kind: ProjectCoordinationBlockerKind,
    pub reason: String,
    #[serde(default)]
    pub files: Vec<String>,
}

impl ProjectCoordinationEvidence {
    pub fn closed() -> Self {
        Self {
            project_session_id: None,
            project_revision: None,
            dirty_files: Vec::new(),
            blockers: Vec::new(),
        }
    }

    pub fn clean(project_session_id: impl Into<String>, project_revision: u64) -> Self {
        Self {
            project_session_id: Some(project_session_id.into()),
            project_revision: Some(project_revision),
            dirty_files: Vec::new(),
            blockers: Vec::new(),
        }
    }

    pub fn dirty(
        project_session_id: impl Into<String>,
        project_revision: u64,
        dirty_files: Vec<String>,
    ) -> Self {
        Self {
            project_session_id: Some(project_session_id.into()),
            project_revision: Some(project_revision),
            dirty_files,
            blockers: Vec::new(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        !self.dirty_files.is_empty()
    }

    pub fn is_blocked(&self) -> bool {
        !self.blockers.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditLeaseRequest {
    pub client_session_id: String,
    pub expected_project_session_id: String,
    pub expected_project_revision: u64,
    pub request_id: String,
    pub intent: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiQuiescenceAcknowledgement {
    pub request_id: String,
    pub project_session_id: String,
    pub project_revision: u64,
    pub ui_revision: u64,
    pub ui_quiescent: bool,
    #[serde(default)]
    pub blocker_reason: Option<String>,
    #[serde(default)]
    pub dirty_files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseEditLeaseInput {
    pub client_session_id: String,
    pub lease_id: String,
    #[serde(default)]
    pub expected_changed_files: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationInput {
    pub lease_id: String,
    pub project_session_id: String,
    pub project_revision: u64,
    #[serde(default)]
    pub observed_changed_files: Vec<String>,
    #[serde(default)]
    pub conflict_files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditLease {
    pub id: String,
    pub request_id: String,
    pub client_session_id: String,
    pub project_session_id: String,
    pub basis_project_revision: u64,
    pub intent: String,
    pub granted_at_ms: u128,
    pub expires_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "state",
    content = "detail",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum EditAuthority {
    UserActive,
    AiRequested {
        request: EditLeaseRequest,
        requested_at_ms: u128,
    },
    AiActive {
        lease: EditLease,
    },
    AiOrphaned {
        lease_id: String,
        client_session_id: String,
        project_session_id: String,
        basis_project_revision: u64,
        expired_at_ms: u128,
        reason: String,
    },
    Reconciling {
        lease_id: String,
        client_session_id: String,
        project_session_id: String,
        basis_project_revision: u64,
        released_at_ms: u128,
        expected_changed_files: Vec<String>,
        observed_changed_files: Vec<String>,
        declaration_reviewed_by_user: bool,
        recovery_reload_authorized: bool,
        recovery_reload_replacement_session_id: Option<String>,
        summary: Option<String>,
        reason: String,
    },
    Conflict {
        project_session_id: String,
        detected_at_ms: u128,
        files: Vec<String>,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EditLeaseStatus {
    PendingUiQuiescence,
    Granted,
    Blocked,
    Busy,
    Stale,
    Orphaned,
    Reconciling,
    ReleasedToUser,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredUserAction {
    SaveOrDiscard,
    WaitForAi,
    RecoverInterruptedAi,
    ResolveConflict,
    ReopenProject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditTransitionReceipt {
    pub status: EditLeaseStatus,
    pub coordination_revision: u64,
    pub authority: EditAuthority,
    pub lease: Option<EditLease>,
    pub reason: Option<String>,
    pub required_user_action: Option<RequiredUserAction>,
    pub dirty_files: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCoordinationSnapshot {
    pub schema_version: u32,
    pub coordination_revision: u64,
    pub project_session_id: Option<String>,
    pub authority: EditAuthority,
    pub clients: Vec<AiClientSessionSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditCoordinationError {
    pub diagnostic: String,
}

impl EditCoordinationError {
    pub fn new(diagnostic: impl Into<String>) -> Self {
        Self {
            diagnostic: diagnostic.into(),
        }
    }
}

impl std::fmt::Display for EditCoordinationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for EditCoordinationError {}
