use std::collections::{BTreeMap, BTreeSet};

use super::model::{
    AiClientIdentity, AiClientSessionSnapshot, AiCoordinationSnapshot, AiPresenceStatus,
    EditAuthority, EditCoordinationError, EditLease, EditLeaseRequest, EditLeaseStatus,
    EditTransitionReceipt, ProjectCoordinationBlockerKind, ProjectCoordinationEvidence,
    ReconciliationInput, ReleaseEditLeaseInput, RequiredUserAction, UiQuiescenceAcknowledgement,
    AI_COORDINATION_SCHEMA_VERSION, DEFAULT_EDIT_LEASE_TTL_MS,
};

const ACTIVE_CLIENT_WINDOW_MS: u128 = 15_000;
const IDLE_CLIENT_WINDOW_MS: u128 = 90_000;
const MAX_CLIENTS: usize = 16;
const MAX_ID_BYTES: usize = 256;
const MAX_INTENT_BYTES: usize = 4 * 1024;
const MAX_SUMMARY_BYTES: usize = 8 * 1024;
const MAX_BLOCKER_REASON_BYTES: usize = 8 * 1024;
const MAX_PATHS: usize = 1_000;
const MAX_PATH_BYTES: usize = 4 * 1024;
const MAX_PATH_TOTAL_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct AiClientSession {
    identity: AiClientIdentity,
    initialized_at_ms: u128,
    last_seen_at_ms: u128,
    context_revision_seen: Option<u64>,
}

#[derive(Clone, Debug)]
pub(super) struct AiCoordinationState {
    project_session_id: Option<String>,
    coordination_revision: u64,
    authority: EditAuthority,
    clients: BTreeMap<String, AiClientSession>,
    next_lease_nonce: u64,
}

impl Default for AiCoordinationState {
    fn default() -> Self {
        Self {
            project_session_id: None,
            coordination_revision: 0,
            authority: EditAuthority::UserActive,
            clients: BTreeMap::new(),
            next_lease_nonce: 1,
        }
    }
}

impl AiCoordinationState {
    pub(super) fn snapshot(&self, now_ms: u128) -> AiCoordinationSnapshot {
        let active_lease_owner = match &self.authority {
            EditAuthority::AiActive { lease } => Some(lease.client_session_id.as_str()),
            _ => None,
        };
        AiCoordinationSnapshot {
            schema_version: AI_COORDINATION_SCHEMA_VERSION,
            coordination_revision: self.coordination_revision,
            project_session_id: self.project_session_id.clone(),
            authority: self.authority.clone(),
            clients: self
                .clients
                .values()
                .map(|client| AiClientSessionSnapshot {
                    session_id: client.identity.session_id.clone(),
                    client_name: client.identity.client_name.clone(),
                    client_version: client.identity.client_version.clone(),
                    initialized_at_ms: client.initialized_at_ms,
                    last_seen_at_ms: client.last_seen_at_ms,
                    context_revision_seen: client.context_revision_seen,
                    presence: presence_status(client.last_seen_at_ms, now_ms),
                    owns_edit_lease: active_lease_owner
                        == Some(client.identity.session_id.as_str()),
                })
                .collect(),
        }
    }

    pub(super) fn register_client(
        &mut self,
        identity: AiClientIdentity,
        now_ms: u128,
    ) -> Result<AiCoordinationSnapshot, EditCoordinationError> {
        validate_identifier("MCP session id", &identity.session_id)?;
        validate_identifier("MCP client name", &identity.client_name)?;
        if let Some(version) = identity.client_version.as_deref() {
            validate_identifier("MCP client version", version)?;
        }

        let changed = match self.clients.get_mut(&identity.session_id) {
            Some(client) => {
                let identity_changed = client.identity != identity;
                client.identity = identity;
                client.last_seen_at_ms = now_ms;
                identity_changed
            }
            None => {
                self.remove_expired_clients(now_ms);
                if self.clients.len() >= MAX_CLIENTS {
                    return Err(EditCoordinationError::new(format!(
                        "Registrul AI a atins limita de {MAX_CLIENTS} sesiuni."
                    )));
                }
                self.clients.insert(
                    identity.session_id.clone(),
                    AiClientSession {
                        identity,
                        initialized_at_ms: now_ms,
                        last_seen_at_ms: now_ms,
                        context_revision_seen: None,
                    },
                );
                true
            }
        };
        if changed {
            self.bump_revision();
        }
        Ok(self.snapshot(now_ms))
    }

    pub(super) fn observe_client(
        &mut self,
        client_session_id: &str,
        context_revision_seen: Option<u64>,
        now_ms: u128,
    ) -> Result<(), EditCoordinationError> {
        let client = self.clients.get_mut(client_session_id).ok_or_else(|| {
            EditCoordinationError::new("Sesiunea AI nu a fost inițializată în Context Hub.")
        })?;
        client.last_seen_at_ms = now_ms;
        if let Some(revision) = context_revision_seen {
            client.context_revision_seen = Some(
                client
                    .context_revision_seen
                    .map_or(revision, |current| current.max(revision)),
            );
        }
        Ok(())
    }

    pub(super) fn bind_project(
        &mut self,
        project_session_id: Option<String>,
        now_ms: u128,
    ) -> Result<AiCoordinationSnapshot, EditCoordinationError> {
        // Publishing the same ProjectSession is an observational refresh. It
        // must remain possible while recovery is blocking transitions, but it
        // must never clear that recovery barrier.
        if self.project_session_id == project_session_id {
            return Ok(self.snapshot(now_ms));
        }

        // Every actual ProjectSession replacement is fenced by the authority
        // state. The one exception is an explicitly-authorized recovery reload,
        // which `require_project_transition` already models as allowed.
        self.require_project_transition()?;

        if let (
            Some(replacement_session_id),
            EditAuthority::Reconciling {
                project_session_id: authority_session_id,
                recovery_reload_authorized: true,
                recovery_reload_replacement_session_id,
                reason,
                ..
            },
        ) = (project_session_id.as_ref(), &mut self.authority)
        {
            let changed = self.project_session_id.as_deref()
                != Some(replacement_session_id.as_str())
                || authority_session_id != replacement_session_id
                || recovery_reload_replacement_session_id.as_deref()
                    != Some(replacement_session_id.as_str());
            self.project_session_id = Some(replacement_session_id.clone());
            authority_session_id.clone_from(replacement_session_id);
            *recovery_reload_replacement_session_id = Some(replacement_session_id.clone());
            *reason = "ProjectSession a fost înlocuită; controlul rămâne blocat până când frontend-ul confirmă proiecția surselor.".to_string();
            if changed {
                self.bump_revision();
            }
            return Ok(self.snapshot(now_ms));
        }

        self.project_session_id = project_session_id;
        self.authority = EditAuthority::UserActive;
        self.bump_revision();
        Ok(self.snapshot(now_ms))
    }

    pub(super) fn require_user_source_mutation(&self) -> Result<(), EditCoordinationError> {
        match &self.authority {
            EditAuthority::UserActive => Ok(()),
            // AiRequested is a two-phase reservation, not yet AI authority.
            // Existing frontend mutations may drain; the final acknowledgement
            // revalidates the exact ProjectWorkspace revision before grant.
            EditAuthority::AiRequested { .. } => Ok(()),
            EditAuthority::AiActive { lease } => Err(EditCoordinationError::new(format!(
                "Editarea aparține temporar sesiunii AI {} (lease {}).",
                lease.client_session_id, lease.id
            ))),
            EditAuthority::AiOrphaned { .. } => Err(EditCoordinationError::new(
                "Editarea este blocată: lease-ul AI s-a întrerupt în timpul unei tranzacții filesystem.",
            )),
            EditAuthority::Reconciling { .. } => Err(EditCoordinationError::new(
                "Editarea este blocată până la reconcilierea modificărilor AI.",
            )),
            EditAuthority::Conflict { .. } => Err(EditCoordinationError::new(
                "Editarea este blocată până la rezolvarea conflictului de disc.",
            )),
        }
    }

    pub(super) fn require_project_transition(&self) -> Result<(), EditCoordinationError> {
        match &self.authority {
            EditAuthority::UserActive => Ok(()),
            EditAuthority::AiRequested { .. } => Err(EditCoordinationError::new(
                "Proiectul nu poate fi schimbat cât timp Pană Studio negociază autoritatea cu AI.",
            )),
            EditAuthority::AiActive { lease } => Err(EditCoordinationError::new(format!(
                "Proiectul nu poate fi schimbat cât timp sesiunea AI {} deține lease-ul {}.",
                lease.client_session_id, lease.id
            ))),
            EditAuthority::AiOrphaned { .. } => Err(EditCoordinationError::new(
                "Proiectul nu poate fi schimbat până la recuperarea tranzacției AI întrerupte.",
            )),
            EditAuthority::Reconciling {
                recovery_reload_authorized: true,
                ..
            } => Ok(()),
            EditAuthority::Reconciling { .. } => Err(EditCoordinationError::new(
                "Proiectul nu poate fi schimbat înainte de reconcilierea modificărilor AI.",
            )),
            EditAuthority::Conflict { .. } => Err(EditCoordinationError::new(
                "Proiectul nu poate fi schimbat înainte de rezolvarea conflictului de coordonare AI.",
            )),
        }
    }

    pub(super) fn require_external_reconciliation(&self) -> Result<(), EditCoordinationError> {
        match &self.authority {
            EditAuthority::UserActive
            | EditAuthority::AiOrphaned { .. }
            | EditAuthority::Reconciling { .. } => Ok(()),
            EditAuthority::AiRequested { .. } => Err(EditCoordinationError::new(
                "Reconcilierea externă este suspendată în timpul transferului de autoritate către AI.",
            )),
            EditAuthority::AiActive { lease } => Err(EditCoordinationError::new(format!(
                "Reconcilierea externă este suspendată cât timp AI deține lease-ul {}.",
                lease.id
            ))),
            EditAuthority::Conflict { .. } => Err(EditCoordinationError::new(
                "Reconcilierea automată este blocată de conflictul de coordonare AI.",
            )),
        }
    }

    pub(super) fn request_edit_lease(
        &mut self,
        request: EditLeaseRequest,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        validate_request(&request)?;
        self.observe_client(&request.client_session_id, None, now_ms)?;
        if let Some(receipt) = self.idempotent_request_receipt(&request) {
            return Ok(receipt);
        }
        if let Some(receipt) = self.validate_request_evidence(&request, evidence) {
            return Ok(receipt);
        }
        if evidence.is_blocked() {
            let files =
                normalized_paths(evidence.blockers.iter().flat_map(|blocker| &blocker.files))?;
            let has_disk_conflict = evidence
                .blockers
                .iter()
                .any(|blocker| blocker.kind == ProjectCoordinationBlockerKind::DiskConflict);
            let reason = evidence
                .blockers
                .iter()
                .map(|blocker| blocker.reason.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            return Ok(self.blocked_receipt(
                if has_disk_conflict {
                    EditLeaseStatus::Conflict
                } else {
                    EditLeaseStatus::Blocked
                },
                reason,
                Some(RequiredUserAction::ResolveConflict),
                files,
            ));
        }
        if evidence.is_dirty() {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                "ProjectWorkspace are modificări nesalvate în RAM.",
                Some(RequiredUserAction::SaveOrDiscard),
                evidence.dirty_files.clone(),
            ));
        }

        match &self.authority {
            EditAuthority::UserActive => {
                self.authority = EditAuthority::AiRequested {
                    request,
                    requested_at_ms: now_ms,
                };
                self.bump_revision();
                Ok(self.receipt(EditLeaseStatus::PendingUiQuiescence, None, None, Vec::new()))
            }
            EditAuthority::AiRequested { .. } | EditAuthority::AiActive { .. } => Ok(self
                .blocked_receipt(
                    EditLeaseStatus::Busy,
                    "O altă sesiune AI solicită sau deține deja autoritatea de editare.",
                    Some(RequiredUserAction::WaitForAi),
                    Vec::new(),
                )),
            EditAuthority::AiOrphaned { .. } => Ok(self.blocked_receipt(
                EditLeaseStatus::Orphaned,
                "Lease-ul AI anterior s-a întrerupt; discul trebuie adoptat explicit sau restaurat înaintea unei noi sesiuni AI.",
                Some(RequiredUserAction::RecoverInterruptedAi),
                Vec::new(),
            )),
            EditAuthority::Reconciling { .. } => Ok(self.blocked_receipt(
                EditLeaseStatus::Reconciling,
                "Pană Studio reconciliază modificările sesiunii AI anterioare.",
                Some(RequiredUserAction::WaitForAi),
                Vec::new(),
            )),
            EditAuthority::Conflict { files, .. } => Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                "Există un conflict de disc care trebuie rezolvat de utilizator.",
                Some(RequiredUserAction::ResolveConflict),
                files.clone(),
            )),
        }
    }

    pub(super) fn acknowledge_ui_quiescence(
        &mut self,
        client_session_id: &str,
        acknowledgement: UiQuiescenceAcknowledgement,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        validate_identifier("MCP client session id", client_session_id)?;
        validate_identifier("edit request id", &acknowledgement.request_id)?;
        validate_identifier("project session id", &acknowledgement.project_session_id)?;
        if acknowledgement
            .blocker_reason
            .as_ref()
            .is_some_and(|reason| reason.len() > MAX_BLOCKER_REASON_BYTES)
        {
            return Err(EditCoordinationError::new(format!(
                "Motivul de blocare UI depășește limita de {MAX_BLOCKER_REASON_BYTES} bytes."
            )));
        }
        self.observe_client(client_session_id, None, now_ms)?;
        let EditAuthority::AiRequested {
            request,
            requested_at_ms: _,
        } = &self.authority
        else {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Nu există o cerere AI care așteaptă quiescence din frontend.",
                None,
                Vec::new(),
            ));
        };
        let request = request.clone();
        if request.client_session_id != client_session_id
            || request.request_id != acknowledgement.request_id
        {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Busy,
                "Confirmarea UI nu aparține cererii AI curente.",
                Some(RequiredUserAction::WaitForAi),
                Vec::new(),
            ));
        }

        let project_identity_matches = evidence.project_session_id.as_deref()
            == Some(request.expected_project_session_id.as_str())
            && acknowledgement.project_session_id == request.expected_project_session_id;
        if !project_identity_matches {
            self.authority = EditAuthority::UserActive;
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Proiectul s-a schimbat în timpul transferului de autoritate.",
                None,
                Vec::new(),
            ));
        }

        if evidence.is_blocked() {
            self.authority = EditAuthority::UserActive;
            self.bump_revision();
            let files =
                normalized_paths(evidence.blockers.iter().flat_map(|blocker| &blocker.files))?;
            let reason = evidence
                .blockers
                .iter()
                .map(|blocker| blocker.reason.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                reason,
                Some(RequiredUserAction::ResolveConflict),
                files,
            ));
        }

        let dirty_files = normalized_paths(
            evidence
                .dirty_files
                .iter()
                .chain(&acknowledgement.dirty_files),
        )?;
        if !dirty_files.is_empty() {
            self.authority = EditAuthority::UserActive;
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                "Frontendul sau ProjectWorkspace au devenit dirty înainte de acordarea lease-ului.",
                Some(RequiredUserAction::SaveOrDiscard),
                dirty_files,
            ));
        }

        if !acknowledgement.ui_quiescent {
            self.authority = EditAuthority::UserActive;
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                acknowledgement.blocker_reason.unwrap_or_else(|| {
                    "Frontendul nu a putut ajunge la o graniță quiescentă.".to_string()
                }),
                Some(RequiredUserAction::SaveOrDiscard),
                Vec::new(),
            ));
        }

        let revision_matches = evidence.project_revision == Some(request.expected_project_revision)
            && acknowledgement.project_revision == request.expected_project_revision;
        if !revision_matches {
            self.authority = EditAuthority::UserActive;
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Revizia proiectului s-a schimbat în timpul transferului de autoritate.",
                None,
                Vec::new(),
            ));
        }

        let lease = EditLease {
            id: self.next_lease_id(now_ms),
            request_id: request.request_id,
            client_session_id: request.client_session_id,
            project_session_id: request.expected_project_session_id,
            basis_project_revision: request.expected_project_revision,
            intent: request.intent,
            granted_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(DEFAULT_EDIT_LEASE_TTL_MS),
        };
        self.authority = EditAuthority::AiActive {
            lease: lease.clone(),
        };
        self.bump_revision();
        Ok(self.receipt(EditLeaseStatus::Granted, Some(lease), None, Vec::new()))
    }

    pub(super) fn renew_edit_lease(
        &mut self,
        client_session_id: &str,
        lease_id: &str,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.observe_client(client_session_id, None, now_ms)?;
        let EditAuthority::AiActive { lease } = &self.authority else {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Nu există un lease AI activ care poate fi reînnoit.",
                None,
                Vec::new(),
            ));
        };
        if lease.client_session_id != client_session_id || lease.id != lease_id {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Busy,
                "Lease-ul aparține altei sesiuni AI.",
                Some(RequiredUserAction::WaitForAi),
                Vec::new(),
            ));
        }
        if evidence.project_session_id.as_deref() != Some(lease.project_session_id.as_str()) {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "ProjectSession nu mai corespunde lease-ului AI.",
                Some(RequiredUserAction::ReopenProject),
                Vec::new(),
            ));
        }
        if evidence.is_dirty() {
            let lease = lease.clone();
            self.authority = EditAuthority::Conflict {
                project_session_id: lease.project_session_id,
                detected_at_ms: now_ms,
                files: evidence.dirty_files.clone(),
                reason: "ProjectWorkspace a devenit dirty în timpul lease-ului AI.".to_string(),
            };
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                "ProjectWorkspace a devenit dirty în timpul lease-ului AI.",
                Some(RequiredUserAction::ResolveConflict),
                evidence.dirty_files.clone(),
            ));
        }

        let mut renewed = lease.clone();
        renewed.expires_at_ms = now_ms.saturating_add(DEFAULT_EDIT_LEASE_TTL_MS);
        self.authority = EditAuthority::AiActive {
            lease: renewed.clone(),
        };
        self.bump_revision();
        Ok(self.receipt(EditLeaseStatus::Granted, Some(renewed), None, Vec::new()))
    }

    pub(super) fn release_edit_lease(
        &mut self,
        input: ReleaseEditLeaseInput,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        validate_identifier("MCP client session id", &input.client_session_id)?;
        validate_identifier("edit lease id", &input.lease_id)?;
        if input
            .summary
            .as_ref()
            .is_some_and(|summary| summary.len() > MAX_SUMMARY_BYTES)
        {
            return Err(EditCoordinationError::new(format!(
                "Rezumatul AI depășește limita de {MAX_SUMMARY_BYTES} bytes."
            )));
        }
        let expected_changed_files = normalized_paths(input.expected_changed_files.iter())?;
        self.observe_client(&input.client_session_id, None, now_ms)?;
        let EditAuthority::AiActive { lease } = &self.authority else {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Nu există un lease AI activ care poate fi eliberat.",
                None,
                Vec::new(),
            ));
        };
        if lease.client_session_id != input.client_session_id || lease.id != input.lease_id {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Busy,
                "Lease-ul activ aparține altei sesiuni AI.",
                Some(RequiredUserAction::WaitForAi),
                Vec::new(),
            ));
        }
        let lease = lease.clone();
        if evidence.project_session_id.as_deref() != Some(lease.project_session_id.as_str()) {
            self.authority = EditAuthority::Conflict {
                project_session_id: lease.project_session_id,
                detected_at_ms: now_ms,
                files: Vec::new(),
                reason: "ProjectSession s-a schimbat înainte de release-ul lease-ului AI."
                    .to_string(),
            };
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                "ProjectSession s-a schimbat înainte de release-ul lease-ului AI.",
                Some(RequiredUserAction::ResolveConflict),
                Vec::new(),
            ));
        }
        if evidence.is_dirty() {
            self.authority = EditAuthority::Conflict {
                project_session_id: lease.project_session_id,
                detected_at_ms: now_ms,
                files: evidence.dirty_files.clone(),
                reason: "ProjectWorkspace a devenit dirty înainte de release-ul lease-ului AI."
                    .to_string(),
            };
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                "ProjectWorkspace a devenit dirty înainte de release-ul lease-ului AI.",
                Some(RequiredUserAction::ResolveConflict),
                evidence.dirty_files.clone(),
            ));
        }
        let non_disk_blockers = evidence
            .blockers
            .iter()
            .filter(|blocker| blocker.kind != ProjectCoordinationBlockerKind::DiskConflict)
            .collect::<Vec<_>>();
        if !non_disk_blockers.is_empty() {
            let reason = non_disk_blockers
                .iter()
                .map(|blocker| blocker.reason.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let files = normalized_paths(
                non_disk_blockers
                    .iter()
                    .flat_map(|blocker| blocker.files.iter()),
            )?;
            self.authority = EditAuthority::Conflict {
                project_session_id: lease.project_session_id,
                detected_at_ms: now_ms,
                files: files.clone(),
                reason: reason.clone(),
            };
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                reason,
                Some(RequiredUserAction::ResolveConflict),
                files,
            ));
        }
        let observed_changed_files = normalized_paths(
            evidence
                .blockers
                .iter()
                .filter(|blocker| blocker.kind == ProjectCoordinationBlockerKind::DiskConflict)
                .flat_map(|blocker| blocker.files.iter()),
        )?;
        let expected_set = expected_changed_files
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let observed_set = observed_changed_files
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mismatch = expected_set
            .symmetric_difference(&observed_set)
            .cloned()
            .collect::<Vec<_>>();
        if !mismatch.is_empty() {
            self.authority = EditAuthority::Conflict {
                project_session_id: lease.project_session_id,
                detected_at_ms: now_ms,
                files: mismatch.clone(),
                reason:
                    "Fișierele schimbate pe disc nu corespund setului declarat de AI la release."
                        .to_string(),
            };
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                "Fișierele schimbate pe disc nu corespund setului declarat de AI la release.",
                Some(RequiredUserAction::ResolveConflict),
                mismatch,
            ));
        }
        self.authority = EditAuthority::Reconciling {
            lease_id: lease.id,
            client_session_id: lease.client_session_id,
            project_session_id: lease.project_session_id,
            basis_project_revision: lease.basis_project_revision,
            released_at_ms: now_ms,
            expected_changed_files,
            observed_changed_files,
            declaration_reviewed_by_user: false,
            recovery_reload_authorized: false,
            recovery_reload_replacement_session_id: None,
            summary: input.summary,
            reason: "Sesiunea AI a eliberat lease-ul; discul trebuie reconciliat.".to_string(),
        };
        self.bump_revision();
        Ok(self.receipt(EditLeaseStatus::Reconciling, None, None, Vec::new()))
    }

    pub(super) fn accept_conflict_for_reconciliation(
        &mut self,
        project_session_id: &str,
        project_revision: u64,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        validate_identifier("project session id", project_session_id)?;
        let EditAuthority::Conflict {
            project_session_id: conflicted_project_session_id,
            files,
            reason,
            ..
        } = &self.authority
        else {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Nu există un conflict AI care poate fi acceptat pentru reconciliere.",
                None,
                Vec::new(),
            ));
        };
        if conflicted_project_session_id != project_session_id
            || self.project_session_id.as_deref() != Some(project_session_id)
        {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Conflictul AI nu aparține ProjectSession curente.",
                Some(RequiredUserAction::ReopenProject),
                Vec::new(),
            ));
        }
        let files = files.clone();
        let previous_reason = reason.clone();
        self.authority = EditAuthority::Reconciling {
            lease_id: format!("ai-conflict-review-{now_ms:032x}"),
            client_session_id: "user-conflict-review".to_string(),
            project_session_id: project_session_id.to_string(),
            basis_project_revision: project_revision,
            released_at_ms: now_ms,
            expected_changed_files: files.clone(),
            observed_changed_files: files,
            declaration_reviewed_by_user: true,
            recovery_reload_authorized: false,
            recovery_reload_replacement_session_id: None,
            summary: Some(previous_reason),
            reason: "Utilizatorul a acceptat reconcilierea setului de fișiere necunoscut."
                .to_string(),
        };
        self.bump_revision();
        Ok(self.receipt(EditLeaseStatus::Reconciling, None, None, Vec::new()))
    }

    pub(super) fn authorize_reconciliation_recovery_reload(
        &mut self,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        if let EditAuthority::AiOrphaned {
            lease_id,
            client_session_id,
            project_session_id,
            basis_project_revision,
            expired_at_ms,
            reason,
        } = self.authority.clone()
        {
            if evidence.project_session_id.as_deref() != Some(project_session_id.as_str()) {
                return Ok(self.blocked_receipt(
                    EditLeaseStatus::Stale,
                    "Recuperarea lease-ului întrerupt nu aparține ProjectSession curente.",
                    Some(RequiredUserAction::ReopenProject),
                    Vec::new(),
                ));
            }
            if evidence.is_dirty() {
                return Ok(self.blocked_receipt(
                    EditLeaseStatus::Blocked,
                    "ProjectWorkspace este dirty; discul unei tranzacții AI întrerupte nu poate fi adoptat automat.",
                    Some(RequiredUserAction::SaveOrDiscard),
                    evidence.dirty_files.clone(),
                ));
            }
            if evidence
                .blockers
                .iter()
                .any(|blocker| blocker.kind != ProjectCoordinationBlockerKind::DiskConflict)
            {
                return Ok(self.blocked_receipt(
                    EditLeaseStatus::Blocked,
                    "RecoveryCoordinator are blocaje care nu provin din modificările de disc ale AI.",
                    Some(RequiredUserAction::ResolveConflict),
                    Vec::new(),
                ));
            }
            let adopted_files = normalized_paths(
                evidence
                    .blockers
                    .iter()
                    .filter(|blocker| blocker.kind == ProjectCoordinationBlockerKind::DiskConflict)
                    .flat_map(|blocker| blocker.files.iter()),
            )?;
            self.authority = EditAuthority::Reconciling {
                lease_id,
                client_session_id,
                project_session_id,
                basis_project_revision,
                released_at_ms: expired_at_ms,
                expected_changed_files: adopted_files.clone(),
                observed_changed_files: adopted_files,
                declaration_reviewed_by_user: true,
                recovery_reload_authorized: true,
                recovery_reload_replacement_session_id: None,
                summary: Some(reason),
                reason: "Utilizatorul a autorizat adoptarea stării stabile de pe disc după întreruperea lease-ului AI.".to_string(),
            };
            self.bump_revision();
            return Ok(self.receipt(
                EditLeaseStatus::Reconciling,
                None,
                Some(
                    "Tranzacția AI întreruptă a fost înghețată; este autorizată reconstruirea exactă din disc."
                        .to_string(),
                ),
                Vec::new(),
            ));
        }
        if matches!(
            self.authority,
            EditAuthority::Reconciling {
                recovery_reload_authorized: true,
                recovery_reload_replacement_session_id: Some(_),
                ..
            }
        ) {
            return Ok(self.receipt(
                EditLeaseStatus::Reconciling,
                None,
                Some(
                    "ProjectSession a fost deja înlocuită; se așteaptă confirmarea proiecției frontend."
                        .to_string(),
                ),
                Vec::new(),
            ));
        }
        let (project_session_id, expected_changed_files, observed_changed_files_at_release) =
            match &self.authority {
                EditAuthority::Reconciling {
                    project_session_id,
                    expected_changed_files,
                    observed_changed_files,
                    ..
                } => (
                    project_session_id.clone(),
                    expected_changed_files.clone(),
                    observed_changed_files.clone(),
                ),
                _ => {
                    return Ok(self.blocked_receipt(
                    EditLeaseStatus::Stale,
                    "Nu există o reconciliere AI pentru care poate fi autorizat recovery reload.",
                    None,
                    Vec::new(),
                ));
                }
            };
        if evidence.project_session_id.as_deref() != Some(project_session_id.as_str()) {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Recovery reload nu aparține ProjectSession curente.",
                Some(RequiredUserAction::ReopenProject),
                Vec::new(),
            ));
        }
        if evidence.is_dirty() {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                "ProjectWorkspace este dirty; recovery reload nu poate fi autorizat.",
                Some(RequiredUserAction::SaveOrDiscard),
                evidence.dirty_files.clone(),
            ));
        }
        let non_disk_blockers = evidence
            .blockers
            .iter()
            .filter(|blocker| blocker.kind != ProjectCoordinationBlockerKind::DiskConflict)
            .collect::<Vec<_>>();
        if !non_disk_blockers.is_empty() {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                "RecoveryCoordinator nu este clean; recovery reload rămâne blocat.",
                Some(RequiredUserAction::ResolveConflict),
                Vec::new(),
            ));
        }

        // Release-ul validează manifestul o dată, dar disk-ul se poate schimba
        // din nou înaintea full reload-ului. Autorizația de Project Transition
        // se acordă numai dacă evidența Rust curentă confirmă încă exact aceeași
        // tranzacție declarată de AI.
        let observed_changed_files_now = normalized_paths(
            evidence
                .blockers
                .iter()
                .filter(|blocker| blocker.kind == ProjectCoordinationBlockerKind::DiskConflict)
                .flat_map(|blocker| blocker.files.iter()),
        )?;
        let expected_set = normalized_paths(expected_changed_files.iter())?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let release_set = normalized_paths(observed_changed_files_at_release.iter())?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let current_set = observed_changed_files_now
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mismatch = expected_set
            .symmetric_difference(&current_set)
            .chain(release_set.symmetric_difference(&current_set))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !mismatch.is_empty() {
            let mismatch = mismatch.into_iter().collect::<Vec<_>>();
            let reason =
                "Disk-ul s-a schimbat după release; full reload-ul AI nu mai corespunde manifestului autorizat.";
            self.authority = EditAuthority::Conflict {
                project_session_id,
                detected_at_ms: now_ms,
                files: mismatch.clone(),
                reason: reason.to_string(),
            };
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                reason,
                Some(RequiredUserAction::ResolveConflict),
                mismatch,
            ));
        }

        let EditAuthority::Reconciling {
            recovery_reload_authorized,
            ..
        } = &mut self.authority
        else {
            unreachable!("autoritatea a fost verificată drept Reconciling");
        };
        if !*recovery_reload_authorized {
            *recovery_reload_authorized = true;
            self.bump_revision();
        }
        Ok(self.receipt(
            EditLeaseStatus::Reconciling,
            None,
            Some(
                "Este autorizată numai reconstruirea sigură a ProjectSession din disk.".to_string(),
            ),
            Vec::new(),
        ))
    }

    pub(super) fn complete_reconciliation_recovery_reload(
        &mut self,
        lease_id: &str,
        expected_replacement_session_id: &str,
        evidence: &ProjectCoordinationEvidence,
        _now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        validate_identifier("AI edit lease id", lease_id)?;
        validate_identifier(
            "replacement project session id",
            expected_replacement_session_id,
        )?;
        let EditAuthority::Reconciling {
            lease_id: authority_lease_id,
            project_session_id,
            recovery_reload_authorized: true,
            recovery_reload_replacement_session_id: Some(replacement_session_id),
            ..
        } = &self.authority
        else {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Nu există un recovery reload publicat care așteaptă proiecția frontend.",
                None,
                Vec::new(),
            ));
        };
        if authority_lease_id != lease_id
            || replacement_session_id != expected_replacement_session_id
            || project_session_id != expected_replacement_session_id
            || self.project_session_id.as_deref() != Some(expected_replacement_session_id)
            || evidence.project_session_id.as_deref() != Some(expected_replacement_session_id)
        {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Confirmarea frontend nu aparține exact ProjectSession rezultate din recovery reload.",
                Some(RequiredUserAction::ReopenProject),
                Vec::new(),
            ));
        }
        if evidence.project_revision.is_none() {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "ProjectWorkspace nu are o revizie confirmabilă după recovery reload.",
                Some(RequiredUserAction::ReopenProject),
                Vec::new(),
            ));
        }
        if evidence.is_dirty() {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                "ProjectWorkspace a devenit dirty înaintea confirmării proiecției frontend.",
                Some(RequiredUserAction::SaveOrDiscard),
                evidence.dirty_files.clone(),
            ));
        }
        if evidence.is_blocked() {
            let files =
                normalized_paths(evidence.blockers.iter().flat_map(|blocker| &blocker.files))?;
            let reason = evidence
                .blockers
                .iter()
                .map(|blocker| blocker.reason.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                reason,
                Some(RequiredUserAction::ResolveConflict),
                files,
            ));
        }

        self.authority = EditAuthority::UserActive;
        self.bump_revision();
        Ok(self.receipt(EditLeaseStatus::ReleasedToUser, None, None, Vec::new()))
    }

    pub(super) fn complete_reconciliation(
        &mut self,
        input: ReconciliationInput,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        let EditAuthority::Reconciling {
            lease_id,
            project_session_id,
            basis_project_revision,
            expected_changed_files,
            observed_changed_files,
            declaration_reviewed_by_user,
            ..
        } = &self.authority
        else {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Nu există o reconciliere AI activă.",
                None,
                Vec::new(),
            ));
        };
        if lease_id != &input.lease_id || project_session_id != &input.project_session_id {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Confirmarea reconcilierii nu aparține lease-ului curent.",
                None,
                Vec::new(),
            ));
        }
        if input.project_revision < *basis_project_revision {
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Revizia reconciliată este mai veche decât baza lease-ului AI.",
                None,
                Vec::new(),
            ));
        }
        let expected_changed_files = normalized_paths(expected_changed_files.iter())?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let observed_at_release = normalized_paths(observed_changed_files.iter())?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let observed_changed_files = normalized_paths(input.observed_changed_files.iter())?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let declared_set_mismatch = expected_changed_files
            .symmetric_difference(&observed_changed_files)
            .chain(observed_at_release.symmetric_difference(&observed_changed_files))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut conflict_files = normalized_paths(input.conflict_files.iter())?
            .into_iter()
            .collect::<BTreeSet<_>>();
        if !declaration_reviewed_by_user {
            conflict_files.extend(declared_set_mismatch.iter().cloned());
        }
        if !conflict_files.is_empty() {
            let reason = if declared_set_mismatch.is_empty() || *declaration_reviewed_by_user {
                "Reconcilierea modificărilor AI a detectat un conflict."
            } else {
                "Fișierele reconciliate nu corespund exact setului declarat de AI la release."
            };
            self.authority = EditAuthority::Conflict {
                project_session_id: input.project_session_id,
                detected_at_ms: now_ms,
                files: conflict_files.iter().cloned().collect(),
                reason: reason.to_string(),
            };
            self.bump_revision();
            return Ok(self.blocked_receipt(
                EditLeaseStatus::Conflict,
                reason,
                Some(RequiredUserAction::ResolveConflict),
                conflict_files.into_iter().collect(),
            ));
        }
        self.authority = EditAuthority::UserActive;
        self.bump_revision();
        Ok(self.receipt(EditLeaseStatus::ReleasedToUser, None, None, Vec::new()))
    }

    pub(super) fn expire(&mut self, now_ms: u128) -> Option<EditTransitionReceipt> {
        match &self.authority {
            EditAuthority::AiActive { lease } if lease.expires_at_ms <= now_ms => {
                let lease = lease.clone();
                self.authority = EditAuthority::AiOrphaned {
                    lease_id: lease.id,
                    client_session_id: lease.client_session_id,
                    project_session_id: lease.project_session_id,
                    basis_project_revision: lease.basis_project_revision,
                    expired_at_ms: now_ms,
                    reason:
                        "Lease-ul AI a expirat în afara unui release confirmat. Ambele părți rămân blocate până la recuperarea explicită a discului."
                            .to_string(),
                };
                self.bump_revision();
                Some(self.receipt(
                    EditLeaseStatus::Orphaned,
                    None,
                    Some(
                        "Tranzacția AI este întreruptă; utilizatorul nu primește automat autoritatea."
                            .to_string(),
                    ),
                    Vec::new(),
                ))
            }
            EditAuthority::AiRequested {
                requested_at_ms, ..
            } if requested_at_ms.saturating_add(DEFAULT_EDIT_LEASE_TTL_MS) <= now_ms => {
                self.authority = EditAuthority::UserActive;
                self.bump_revision();
                Some(self.receipt(EditLeaseStatus::ReleasedToUser, None, None, Vec::new()))
            }
            _ => None,
        }
    }

    fn validate_request_evidence(
        &self,
        request: &EditLeaseRequest,
        evidence: &ProjectCoordinationEvidence,
    ) -> Option<EditTransitionReceipt> {
        let Some(bound_project_session) = self.project_session_id.as_deref() else {
            return Some(self.blocked_receipt(
                EditLeaseStatus::Blocked,
                "Pană Studio nu are un proiect deschis.",
                Some(RequiredUserAction::ReopenProject),
                Vec::new(),
            ));
        };
        let matches = bound_project_session == request.expected_project_session_id
            && evidence.project_session_id.as_deref() == Some(bound_project_session)
            && evidence.project_revision == Some(request.expected_project_revision);
        (!matches).then(|| {
            self.blocked_receipt(
                EditLeaseStatus::Stale,
                "Cererea AI folosește o sesiune sau revizie de proiect stale.",
                None,
                Vec::new(),
            )
        })
    }

    fn idempotent_request_receipt(
        &self,
        request: &EditLeaseRequest,
    ) -> Option<EditTransitionReceipt> {
        match &self.authority {
            EditAuthority::AiRequested {
                request: current, ..
            } if current.client_session_id == request.client_session_id
                && current.request_id == request.request_id =>
            {
                Some(self.receipt(EditLeaseStatus::PendingUiQuiescence, None, None, Vec::new()))
            }
            EditAuthority::AiActive { lease }
                if lease.client_session_id == request.client_session_id
                    && lease.request_id == request.request_id =>
            {
                Some(self.receipt(
                    EditLeaseStatus::Granted,
                    Some(lease.clone()),
                    None,
                    Vec::new(),
                ))
            }
            _ => None,
        }
    }

    fn receipt(
        &self,
        status: EditLeaseStatus,
        lease: Option<EditLease>,
        reason: Option<String>,
        dirty_files: Vec<String>,
    ) -> EditTransitionReceipt {
        EditTransitionReceipt {
            status,
            coordination_revision: self.coordination_revision,
            authority: self.authority.clone(),
            lease,
            reason,
            required_user_action: None,
            dirty_files,
        }
    }

    fn blocked_receipt(
        &self,
        status: EditLeaseStatus,
        reason: impl Into<String>,
        required_user_action: Option<RequiredUserAction>,
        dirty_files: Vec<String>,
    ) -> EditTransitionReceipt {
        EditTransitionReceipt {
            status,
            coordination_revision: self.coordination_revision,
            authority: self.authority.clone(),
            lease: None,
            reason: Some(reason.into()),
            required_user_action,
            dirty_files,
        }
    }

    fn next_lease_id(&mut self, now_ms: u128) -> String {
        let nonce = self.next_lease_nonce;
        self.next_lease_nonce = self.next_lease_nonce.saturating_add(1);
        format!("ai-lease-{now_ms:032x}-{nonce:016x}")
    }

    fn bump_revision(&mut self) {
        self.coordination_revision = self.coordination_revision.saturating_add(1);
    }

    fn remove_expired_clients(&mut self, now_ms: u128) {
        let lease_owner = match &self.authority {
            EditAuthority::AiActive { lease } => Some(lease.client_session_id.as_str()),
            EditAuthority::AiOrphaned {
                client_session_id, ..
            } => Some(client_session_id.as_str()),
            _ => None,
        };
        self.clients.retain(|session_id, client| {
            lease_owner == Some(session_id.as_str())
                || presence_status(client.last_seen_at_ms, now_ms) != AiPresenceStatus::Expired
        });
    }
}

fn presence_status(last_seen_at_ms: u128, now_ms: u128) -> AiPresenceStatus {
    let age = now_ms.saturating_sub(last_seen_at_ms);
    if age <= ACTIVE_CLIENT_WINDOW_MS {
        AiPresenceStatus::Active
    } else if age <= IDLE_CLIENT_WINDOW_MS {
        AiPresenceStatus::Idle
    } else {
        AiPresenceStatus::Expired
    }
}

fn validate_request(request: &EditLeaseRequest) -> Result<(), EditCoordinationError> {
    validate_identifier("MCP client session id", &request.client_session_id)?;
    validate_identifier("project session id", &request.expected_project_session_id)?;
    validate_identifier("edit request id", &request.request_id)?;
    if request.intent.trim().is_empty() {
        return Err(EditCoordinationError::new(
            "Cererea de edit lease trebuie să declare intenția AI.",
        ));
    }
    if request.intent.len() > MAX_INTENT_BYTES {
        return Err(EditCoordinationError::new(format!(
            "Intenția AI depășește limita de {MAX_INTENT_BYTES} bytes."
        )));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), EditCoordinationError> {
    if value.trim().is_empty() || value.len() > MAX_ID_BYTES || value.chars().any(char::is_control)
    {
        return Err(EditCoordinationError::new(format!(
            "{label} este gol, prea lung sau conține caractere de control."
        )));
    }
    Ok(())
}

fn normalized_paths<'a>(
    paths: impl Iterator<Item = &'a String>,
) -> Result<Vec<String>, EditCoordinationError> {
    let mut normalized = BTreeSet::new();
    let mut total_bytes = 0_usize;
    for path in paths {
        if normalized.len() >= MAX_PATHS {
            return Err(EditCoordinationError::new(format!(
                "Lista de fișiere depășește limita de {MAX_PATHS}."
            )));
        }
        total_bytes = total_bytes.saturating_add(path.len());
        if path.len() > MAX_PATH_BYTES || total_bytes > MAX_PATH_TOTAL_BYTES {
            return Err(EditCoordinationError::new(format!(
                "Lista de căi depășește bugetul de {MAX_PATH_TOTAL_BYTES} bytes sau o cale depășește {MAX_PATH_BYTES} bytes."
            )));
        }
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path
                .split('/')
                .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(EditCoordinationError::new(format!(
                "Calea de coordonare nu este relativă și normalizată: {path}"
            )));
        }
        normalized.insert(path.clone());
    }
    Ok(normalized.into_iter().collect())
}
