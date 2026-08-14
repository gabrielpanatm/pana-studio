use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::kernel::{
    observability::now_ms,
    project_session::{ProjectRootFingerprint, ProjectSessionSnapshot},
    project_workspace::{ProjectOpenRecoveryAssessment, ProjectOpenRecoveryStatus},
};

use super::{ProjectDiskManifest, StartupCandidateKind, StartupCandidateSnapshot};

pub const PROJECT_LIFECYCLE_SCHEMA_VERSION: u32 = 1;
pub const PROJECT_OPEN_INSPECTION_SCHEMA_VERSION: u32 = 1;
pub const PROJECT_OPEN_BOOTSTRAP_SCHEMA_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectTransitionState {
    Idle,
    Inspecting,
    AwaitingRecoveryDecision,
    Preparing,
    Committing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ActiveProjectReadiness {
    InitializingFrontend,
    PreparingPreview,
    AwaitingCanvas,
    FinalizingFrontend,
    Ready,
    Degraded {
        capability: String,
        diagnostic: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProjectLifecycleSession {
    pub project_root: String,
    pub runtime_session_id: String,
    pub readiness: ActiveProjectReadiness,
    pub committed_at_ms: u128,
    pub readiness_changed_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLifecycleSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub active_session: Option<ActiveProjectLifecycleSession>,
    pub transition: ProjectTransitionState,
    pub operation_id: Option<String>,
    pub transition_started_at_ms: Option<u128>,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectOpenInspectionContext {
    pub operation_id: String,
    pub operation_started_at_ms: u128,
    pub candidate: StartupCandidateSnapshot,
    pub manifest: ProjectDiskManifest,
    pub root_fingerprint: ProjectRootFingerprint,
    pub recovery: ProjectOpenRecoveryAssessment,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenInspectionReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub operation_started_at_ms: u128,
    pub candidate_token: String,
    pub recovery: ProjectOpenRecoveryAssessment,
    pub lifecycle: ProjectLifecycleSnapshot,
}

#[derive(Clone, Debug)]
struct ProjectLifecycleState {
    snapshot: ProjectLifecycleSnapshot,
    pending: Option<ProjectOpenInspectionContext>,
    preparation_claimed: bool,
}

#[derive(Clone)]
pub struct ProjectLifecycleRuntime {
    state: Arc<Mutex<ProjectLifecycleState>>,
}

impl Default for ProjectLifecycleRuntime {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(ProjectLifecycleState {
                snapshot: ProjectLifecycleSnapshot {
                    schema_version: PROJECT_LIFECYCLE_SCHEMA_VERSION,
                    revision: 1,
                    active_session: None,
                    transition: ProjectTransitionState::Idle,
                    operation_id: None,
                    transition_started_at_ms: None,
                    reason: "runtime_initialized".to_string(),
                },
                pending: None,
                preparation_claimed: false,
            })),
        }
    }
}

impl ProjectLifecycleRuntime {
    pub fn snapshot(&self) -> Result<ProjectLifecycleSnapshot, String> {
        self.state
            .lock()
            .map(|state| state.snapshot.clone())
            .map_err(|_| "ProjectLifecycle mutex este compromis.".to_string())
    }

    pub fn begin_inspection(&self, requested_root: &str) -> Result<String, String> {
        let mut state = self.lock()?;
        let next_revision = state.snapshot.revision.saturating_add(1);
        let operation_id = format!("project-open:{:032x}:{next_revision:016x}", now_ms());
        state.snapshot.revision = next_revision;
        state.snapshot.transition = ProjectTransitionState::Inspecting;
        state.snapshot.operation_id = Some(operation_id.clone());
        state.snapshot.transition_started_at_ms = Some(now_ms());
        state.snapshot.reason = format!("folder_selected:{requested_root}");
        state.pending = None;
        state.preparation_claimed = false;
        Ok(operation_id)
    }

    pub(crate) fn publish_inspection(
        &self,
        operation_id: &str,
        candidate: StartupCandidateSnapshot,
        manifest: ProjectDiskManifest,
        root_fingerprint: ProjectRootFingerprint,
        recovery: ProjectOpenRecoveryAssessment,
    ) -> Result<ProjectOpenInspectionReceipt, String> {
        if candidate.kind != StartupCandidateKind::ValidProject {
            return Err("ProjectLifecycle acceptă numai un candidat Zola valid.".to_string());
        }
        if candidate.root != manifest.root || candidate.root != root_fingerprint.canonical_path {
            return Err(
                "Inspecția proiectului nu descrie același root canonic în toate dovezile."
                    .to_string(),
            );
        }
        if candidate.root != recovery.project_root {
            return Err("Evaluarea recovery aparține altui proiect.".to_string());
        }

        let mut state = self.lock()?;
        require_operation(&state, operation_id, ProjectTransitionState::Inspecting)?;
        let operation_started_at_ms = state
            .snapshot
            .transition_started_at_ms
            .ok_or_else(|| "ProjectLifecycle nu are timestamp-ul inspecției.".to_string())?;
        let transition = if recovery.status == ProjectOpenRecoveryStatus::DecisionRequired {
            ProjectTransitionState::AwaitingRecoveryDecision
        } else {
            ProjectTransitionState::Preparing
        };
        state.pending = Some(ProjectOpenInspectionContext {
            operation_id: operation_id.to_string(),
            operation_started_at_ms,
            candidate: candidate.clone(),
            manifest,
            root_fingerprint,
            recovery: recovery.clone(),
        });
        state.preparation_claimed = false;
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.transition = transition;
        state.snapshot.transition_started_at_ms = Some(now_ms());
        state.snapshot.reason = match transition {
            ProjectTransitionState::AwaitingRecoveryDecision => "recovery_decision_required",
            _ => "inspection_accepted",
        }
        .to_string();
        Ok(ProjectOpenInspectionReceipt {
            schema_version: PROJECT_OPEN_INSPECTION_SCHEMA_VERSION,
            operation_id: operation_id.to_string(),
            operation_started_at_ms,
            candidate_token: candidate.snapshot_token,
            recovery,
            lifecycle: state.snapshot.clone(),
        })
    }

    pub(crate) fn published_inspection(
        &self,
        operation_id: &str,
        candidate_token: &str,
    ) -> Result<Option<ProjectOpenInspectionReceipt>, String> {
        let state = self.lock()?;
        require_current_operation(&state, operation_id)?;
        if state.snapshot.transition == ProjectTransitionState::Inspecting {
            return Ok(None);
        }
        let pending = state
            .pending
            .as_ref()
            .ok_or_else(|| "Contextul inspecției ProjectLifecycle lipsește.".to_string())?;
        if pending.candidate.snapshot_token != candidate_token {
            return Err("Tokenul candidatului este stale.".to_string());
        }
        Ok(Some(ProjectOpenInspectionReceipt {
            schema_version: PROJECT_OPEN_INSPECTION_SCHEMA_VERSION,
            operation_id: pending.operation_id.clone(),
            operation_started_at_ms: pending.operation_started_at_ms,
            candidate_token: pending.candidate.snapshot_token.clone(),
            recovery: pending.recovery.clone(),
            lifecycle: state.snapshot.clone(),
        }))
    }

    pub(crate) fn begin_preparing(
        &self,
        operation_id: &str,
        candidate_token: &str,
        recovery_required: bool,
        recovery_decision_token: Option<&str>,
    ) -> Result<ProjectOpenInspectionContext, String> {
        let mut state = self.lock()?;
        require_current_operation(&state, operation_id)?;
        if !matches!(
            state.snapshot.transition,
            ProjectTransitionState::Preparing | ProjectTransitionState::AwaitingRecoveryDecision
        ) {
            return Err(format!(
                "ProjectLifecycle nu poate începe pregătirea din {:?}.",
                state.snapshot.transition
            ));
        }
        if state.preparation_claimed {
            return Err(
                "ProjectLifecycle a refuzat pregătirea duplicată a aceleiași operații.".to_string(),
            );
        }
        let pending = state
            .pending
            .as_ref()
            .ok_or_else(|| "Contextul inspecției ProjectLifecycle lipsește.".to_string())?;
        if pending.candidate.snapshot_token != candidate_token {
            return Err("Tokenul candidatului este stale.".to_string());
        }
        if recovery_required
            && pending.recovery.status == ProjectOpenRecoveryStatus::DecisionRequired
        {
            let expected = pending
                .recovery
                .assessment_token
                .as_deref()
                .ok_or_else(|| "Evaluarea recovery nu are token autoritar.".to_string())?;
            if recovery_decision_token != Some(expected) {
                return Err(
                    "ProjectLifecycle așteaptă decizia recovery pentru aceeași evaluare."
                        .to_string(),
                );
            }
        } else if recovery_decision_token.is_some() {
            return Err("Decizia recovery este stale pentru inspecția curentă.".to_string());
        }
        let pending = pending.clone();
        state.preparation_claimed = true;
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.transition = ProjectTransitionState::Preparing;
        state.snapshot.transition_started_at_ms = Some(now_ms());
        state.snapshot.reason = "provisional_bootstrap_started".to_string();
        Ok(pending)
    }

    pub fn begin_commit(&self, operation_id: &str) -> Result<ProjectLifecycleSnapshot, String> {
        let mut state = self.lock()?;
        require_operation(&state, operation_id, ProjectTransitionState::Preparing)?;
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.transition = ProjectTransitionState::Committing;
        state.snapshot.transition_started_at_ms = Some(now_ms());
        state.snapshot.reason = "provisional_bootstrap_verified".to_string();
        Ok(state.snapshot.clone())
    }

    pub fn commit_session(
        &self,
        operation_id: &str,
        session: &ProjectSessionSnapshot,
    ) -> Result<ProjectLifecycleSnapshot, String> {
        let mut state = self.lock()?;
        require_operation(&state, operation_id, ProjectTransitionState::Committing)?;
        let changed_at = now_ms();
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.active_session = Some(ActiveProjectLifecycleSession {
            project_root: session.project_root.clone(),
            runtime_session_id: session.runtime_instance_id(),
            readiness: ActiveProjectReadiness::InitializingFrontend,
            committed_at_ms: changed_at,
            readiness_changed_at_ms: changed_at,
        });
        state.snapshot.transition = ProjectTransitionState::Idle;
        state.snapshot.operation_id = None;
        state.snapshot.transition_started_at_ms = None;
        state.snapshot.reason = "project_session_committed".to_string();
        state.pending = None;
        state.preparation_claimed = false;
        Ok(state.snapshot.clone())
    }

    pub fn fail_before_commit(
        &self,
        operation_id: &str,
        diagnostic: &str,
    ) -> Result<ProjectLifecycleSnapshot, String> {
        let mut state = self.lock()?;
        require_current_operation(&state, operation_id)?;
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.transition = ProjectTransitionState::Idle;
        state.snapshot.operation_id = None;
        state.snapshot.transition_started_at_ms = None;
        state.snapshot.reason = format!("precommit_failed:{diagnostic}");
        state.pending = None;
        state.preparation_claimed = false;
        Ok(state.snapshot.clone())
    }

    pub fn set_readiness(
        &self,
        project_root: &str,
        runtime_session_id: &str,
        readiness: ActiveProjectReadiness,
        reason: &str,
    ) -> Result<ProjectLifecycleSnapshot, String> {
        let mut state = self.lock()?;
        let active = state
            .snapshot
            .active_session
            .as_mut()
            .ok_or_else(|| "ProjectLifecycle nu are sesiune activă.".to_string())?;
        if active.project_root != project_root || active.runtime_session_id != runtime_session_id {
            return Err("Readiness-ul a fost refuzat pentru o sesiune stale.".to_string());
        }
        if !valid_readiness_transition(&active.readiness, &readiness) {
            return Err(format!(
                "ProjectLifecycle a refuzat tranziția readiness {:?} → {:?}.",
                active.readiness, readiness
            ));
        }
        active.readiness = readiness;
        active.readiness_changed_at_ms = now_ms();
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.reason = reason.to_string();
        Ok(state.snapshot.clone())
    }

    pub fn clear_active(&self, reason: &str) -> Result<ProjectLifecycleSnapshot, String> {
        let mut state = self.lock()?;
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.active_session = None;
        state.snapshot.transition = ProjectTransitionState::Idle;
        state.snapshot.operation_id = None;
        state.snapshot.transition_started_at_ms = None;
        state.snapshot.reason = reason.to_string();
        state.pending = None;
        state.preparation_claimed = false;
        Ok(state.snapshot.clone())
    }

    pub fn attach_existing_session(
        &self,
        session: &ProjectSessionSnapshot,
    ) -> Result<ProjectLifecycleSnapshot, String> {
        let mut state = self.lock()?;
        let runtime_session_id = session.runtime_instance_id();
        let changed_at = now_ms();
        state.snapshot.revision = state.snapshot.revision.saturating_add(1);
        state.snapshot.active_session = Some(ActiveProjectLifecycleSession {
            project_root: session.project_root.clone(),
            runtime_session_id,
            readiness: ActiveProjectReadiness::InitializingFrontend,
            committed_at_ms: changed_at,
            readiness_changed_at_ms: changed_at,
        });
        state.snapshot.transition = ProjectTransitionState::Idle;
        state.snapshot.operation_id = None;
        state.snapshot.transition_started_at_ms = None;
        state.snapshot.reason = "existing_session_attached".to_string();
        state.pending = None;
        state.preparation_claimed = false;
        Ok(state.snapshot.clone())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ProjectLifecycleState>, String> {
        self.state
            .lock()
            .map_err(|_| "ProjectLifecycle mutex este compromis.".to_string())
    }
}

fn valid_readiness_transition(
    current: &ActiveProjectReadiness,
    next: &ActiveProjectReadiness,
) -> bool {
    use ActiveProjectReadiness::*;
    matches!(
        (current, next),
        (
            InitializingFrontend,
            InitializingFrontend | PreparingPreview | Degraded { .. }
        ) | (
            PreparingPreview,
            PreparingPreview | AwaitingCanvas | FinalizingFrontend | Degraded { .. }
        ) | (
            AwaitingCanvas,
            AwaitingCanvas | FinalizingFrontend | Degraded { .. }
        ) | (
            FinalizingFrontend,
            FinalizingFrontend | Ready | Degraded { .. }
        ) | (
            Ready,
            Ready | PreparingPreview | AwaitingCanvas | FinalizingFrontend | Degraded { .. }
        ) | (
            Degraded { .. },
            Degraded { .. } | PreparingPreview | AwaitingCanvas | FinalizingFrontend
        )
    )
}

fn require_current_operation(
    state: &ProjectLifecycleState,
    operation_id: &str,
) -> Result<(), String> {
    if state.snapshot.operation_id.as_deref() != Some(operation_id) {
        return Err(format!(
            "ProjectLifecycle a refuzat operationId stale `{operation_id}`."
        ));
    }
    Ok(())
}

fn require_operation(
    state: &ProjectLifecycleState,
    operation_id: &str,
    expected: ProjectTransitionState,
) -> Result<(), String> {
    require_current_operation(state, operation_id)?;
    if state.snapshot.transition != expected {
        return Err(format!(
            "ProjectLifecycle aștepta {:?}, dar este {:?}.",
            expected, state.snapshot.transition
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::project_workspace::ProjectOpenRecoveryConflictReason;

    fn candidate(root: &str, token: &str) -> StartupCandidateSnapshot {
        StartupCandidateSnapshot {
            root: root.to_string(),
            display_name: "demo".to_string(),
            kind: StartupCandidateKind::ValidProject,
            snapshot_token: token.to_string(),
            entry_count: 3,
            truncated: false,
            diagnostics: Vec::new(),
        }
    }

    fn manifest(root: &str) -> ProjectDiskManifest {
        ProjectDiskManifest {
            root: root.to_string(),
            files: Vec::new(),
            truncated: false,
            max_files: 1_000,
        }
    }

    fn fingerprint(root: &str) -> ProjectRootFingerprint {
        ProjectRootFingerprint {
            canonical_path: root.to_string(),
            modified_ms: 1,
            size: 0,
            readonly: false,
            unix_device: None,
            unix_inode: None,
        }
    }

    fn recovery(root: &str, status: ProjectOpenRecoveryStatus) -> ProjectOpenRecoveryAssessment {
        ProjectOpenRecoveryAssessment {
            schema_version: 1,
            status,
            project_root: root.to_string(),
            assessment_token: (status == ProjectOpenRecoveryStatus::DecisionRequired)
                .then(|| "recovery-token".to_string()),
            conflict_reason: (status == ProjectOpenRecoveryStatus::DecisionRequired)
                .then_some(ProjectOpenRecoveryConflictReason::DiskBaselineChanged),
            root_identity_changed: None,
            recovery_revision: None,
            dirty_document_count: 0,
            staged_binary_resource_count: 0,
            deleted_binary_resource_count: 0,
            page_js_draft_count: 0,
            undo_count: 0,
            redo_count: 0,
            accepted_file_count: 0,
            current_file_count: 0,
            diagnostic: None,
        }
    }

    #[test]
    fn happy_path_reaches_initializing_frontend_only_after_commit() {
        let runtime = ProjectLifecycleRuntime::default();
        let root = "/tmp/project-lifecycle-happy";
        let operation = runtime.begin_inspection(root).unwrap();
        runtime
            .publish_inspection(
                &operation,
                candidate(root, "candidate"),
                manifest(root),
                fingerprint(root),
                recovery(root, ProjectOpenRecoveryStatus::Missing),
            )
            .unwrap();
        runtime
            .begin_preparing(&operation, "candidate", true, None)
            .unwrap();
        runtime.begin_commit(&operation).unwrap();
        let session = ProjectSessionSnapshot {
            schema_version: 2,
            id: "stable".to_string(),
            project_root: root.to_string(),
            zola_root: root.to_string(),
            session_dir: "/tmp/session".to_string(),
            manifest_path: "/tmp/session/manifest.json".to_string(),
            opened_at_ms: 7,
            last_seen_at_ms: 7,
            root_fingerprint: fingerprint(root),
            scan_summary: crate::kernel::project_session::ProjectSessionScanSummary {
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        };
        let committed = runtime.commit_session(&operation, &session).unwrap();
        assert_eq!(committed.transition, ProjectTransitionState::Idle);
        assert!(matches!(
            committed.active_session.unwrap().readiness,
            ActiveProjectReadiness::InitializingFrontend
        ));
    }

    #[test]
    fn stale_operation_cannot_consume_newer_inspection() {
        let runtime = ProjectLifecycleRuntime::default();
        let stale = runtime.begin_inspection("/tmp/a").unwrap();
        let current = runtime.begin_inspection("/tmp/b").unwrap();
        let error = runtime
            .publish_inspection(
                &stale,
                candidate("/tmp/a", "a"),
                manifest("/tmp/a"),
                fingerprint("/tmp/a"),
                recovery("/tmp/a", ProjectOpenRecoveryStatus::Missing),
            )
            .unwrap_err();
        assert!(error.contains("stale"));
        assert_eq!(
            runtime.snapshot().unwrap().operation_id.as_deref(),
            Some(current.as_str())
        );
    }

    #[test]
    fn repeated_inspection_is_idempotent_but_preparation_has_one_owner() {
        let runtime = ProjectLifecycleRuntime::default();
        let root = "/tmp/project-lifecycle-idempotent";
        let operation = runtime.begin_inspection(root).unwrap();
        let published = runtime
            .publish_inspection(
                &operation,
                candidate(root, "candidate"),
                manifest(root),
                fingerprint(root),
                recovery(root, ProjectOpenRecoveryStatus::Missing),
            )
            .unwrap();
        let repeated = runtime
            .published_inspection(&operation, "candidate")
            .unwrap()
            .unwrap();
        assert_eq!(repeated.operation_id, published.operation_id);
        assert_eq!(repeated.lifecycle.revision, published.lifecycle.revision);

        runtime
            .begin_preparing(&operation, "candidate", true, None)
            .unwrap();
        assert!(runtime
            .begin_preparing(&operation, "candidate", true, None)
            .unwrap_err()
            .contains("duplicată"));
    }

    #[test]
    fn recovery_decision_is_bound_to_the_inspected_assessment() {
        let runtime = ProjectLifecycleRuntime::default();
        let root = "/tmp/project-lifecycle-recovery";
        let operation = runtime.begin_inspection(root).unwrap();
        runtime
            .publish_inspection(
                &operation,
                candidate(root, "candidate"),
                manifest(root),
                fingerprint(root),
                recovery(root, ProjectOpenRecoveryStatus::DecisionRequired),
            )
            .unwrap();
        assert!(runtime
            .begin_preparing(&operation, "candidate", true, None)
            .unwrap_err()
            .contains("așteaptă decizia"));
        runtime
            .begin_preparing(&operation, "candidate", true, Some("recovery-token"))
            .unwrap();
    }

    #[test]
    fn precommit_failure_preserves_the_previous_active_session() {
        let runtime = ProjectLifecycleRuntime::default();
        {
            let mut state = runtime.state.lock().unwrap();
            state.snapshot.active_session = Some(ActiveProjectLifecycleSession {
                project_root: "/tmp/old".to_string(),
                runtime_session_id: "old-session".to_string(),
                readiness: ActiveProjectReadiness::Ready,
                committed_at_ms: 1,
                readiness_changed_at_ms: 1,
            });
        }
        let operation = runtime.begin_inspection("/tmp/new").unwrap();
        runtime.fail_before_commit(&operation, "invalid").unwrap();
        let snapshot = runtime.snapshot().unwrap();
        assert_eq!(snapshot.transition, ProjectTransitionState::Idle);
        assert_eq!(
            snapshot.active_session.unwrap().runtime_session_id,
            "old-session"
        );
    }

    #[test]
    fn cancellation_retires_only_the_matching_pending_operation() {
        let runtime = ProjectLifecycleRuntime::default();
        let operation = runtime.begin_inspection("/tmp/cancelled").unwrap();
        let cancelled = runtime
            .fail_before_commit(&operation, "user_cancelled")
            .unwrap();
        assert_eq!(cancelled.transition, ProjectTransitionState::Idle);
        assert!(cancelled.operation_id.is_none());
        assert!(runtime
            .fail_before_commit(&operation, "duplicate_cancel")
            .unwrap_err()
            .contains("stale"));
    }

    #[test]
    fn postcommit_failure_degrades_only_the_named_capability() {
        let runtime = ProjectLifecycleRuntime::default();
        {
            let mut state = runtime.state.lock().unwrap();
            state.snapshot.active_session = Some(ActiveProjectLifecycleSession {
                project_root: "/tmp/project".to_string(),
                runtime_session_id: "session".to_string(),
                readiness: ActiveProjectReadiness::AwaitingCanvas,
                committed_at_ms: 1,
                readiness_changed_at_ms: 1,
            });
        }
        let snapshot = runtime
            .set_readiness(
                "/tmp/project",
                "session",
                ActiveProjectReadiness::Degraded {
                    capability: "preview".to_string(),
                    diagnostic: "build failed".to_string(),
                },
                "preview_failed",
            )
            .unwrap();
        assert!(matches!(
            snapshot.active_session.unwrap().readiness,
            ActiveProjectReadiness::Degraded { capability, .. } if capability == "preview"
        ));
    }

    #[test]
    fn ready_cannot_bypass_frontend_and_preview_acknowledgements() {
        let runtime = ProjectLifecycleRuntime::default();
        {
            let mut state = runtime.state.lock().unwrap();
            state.snapshot.active_session = Some(ActiveProjectLifecycleSession {
                project_root: "/tmp/project".to_string(),
                runtime_session_id: "session".to_string(),
                readiness: ActiveProjectReadiness::InitializingFrontend,
                committed_at_ms: 1,
                readiness_changed_at_ms: 1,
            });
        }
        let error = runtime
            .set_readiness(
                "/tmp/project",
                "session",
                ActiveProjectReadiness::Ready,
                "invalid_shortcut",
            )
            .unwrap_err();
        assert!(error.contains("refuzat tranziția"));

        runtime
            .set_readiness(
                "/tmp/project",
                "session",
                ActiveProjectReadiness::PreparingPreview,
                "frontend_hydrated",
            )
            .unwrap();
        let error = runtime
            .set_readiness(
                "/tmp/project",
                "session",
                ActiveProjectReadiness::Ready,
                "invalid_preview_shortcut",
            )
            .unwrap_err();
        assert!(error.contains("refuzat tranziția"));
    }

    #[test]
    fn canonical_canvas_must_wait_for_the_final_frontend_surface() {
        let runtime = ProjectLifecycleRuntime::default();
        {
            let mut state = runtime.state.lock().unwrap();
            state.snapshot.active_session = Some(ActiveProjectLifecycleSession {
                project_root: "/tmp/project".to_string(),
                runtime_session_id: "session".to_string(),
                readiness: ActiveProjectReadiness::AwaitingCanvas,
                committed_at_ms: 1,
                readiness_changed_at_ms: 1,
            });
        }
        runtime
            .set_readiness(
                "/tmp/project",
                "session",
                ActiveProjectReadiness::FinalizingFrontend,
                "canvas_canonical_verified",
            )
            .unwrap();
        let ready = runtime
            .set_readiness(
                "/tmp/project",
                "session",
                ActiveProjectReadiness::Ready,
                "initial_surface_ready",
            )
            .unwrap();
        assert!(matches!(
            ready.active_session.unwrap().readiness,
            ActiveProjectReadiness::Ready
        ));
    }
}
