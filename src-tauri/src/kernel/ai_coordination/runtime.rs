use std::sync::Mutex;

use super::{
    model::{
        AiClientIdentity, AiCoordinationSnapshot, EditCoordinationError, EditLeaseRequest,
        EditTransitionReceipt, ProjectCoordinationEvidence, ReconciliationInput,
        ReleaseEditLeaseInput, UiQuiescenceAcknowledgement,
    },
    state_machine::AiCoordinationState,
};

#[derive(Default)]
pub struct AiCoordinationRuntime {
    state: Mutex<AiCoordinationState>,
}

impl AiCoordinationRuntime {
    pub fn snapshot(&self, now_ms: u128) -> Result<AiCoordinationSnapshot, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            Ok(state.snapshot(now_ms))
        })
    }

    pub fn register_client(
        &self,
        identity: AiClientIdentity,
        now_ms: u128,
    ) -> Result<AiCoordinationSnapshot, EditCoordinationError> {
        self.with_state(|state| state.register_client(identity, now_ms))
    }

    pub fn observe_client(
        &self,
        client_session_id: &str,
        context_revision_seen: Option<u64>,
        now_ms: u128,
    ) -> Result<(), EditCoordinationError> {
        self.with_state(|state| {
            state.observe_client(client_session_id, context_revision_seen, now_ms)
        })
    }

    pub fn bind_project(
        &self,
        project_session_id: Option<String>,
        now_ms: u128,
    ) -> Result<AiCoordinationSnapshot, EditCoordinationError> {
        self.with_state(|state| state.bind_project(project_session_id, now_ms))
    }

    pub fn require_user_source_mutation(&self) -> Result<(), EditCoordinationError> {
        self.with_state(|state| state.require_user_source_mutation())
    }

    pub fn require_project_transition(&self) -> Result<(), EditCoordinationError> {
        self.with_state(|state| state.require_project_transition())
    }

    pub fn require_external_reconciliation(&self) -> Result<(), EditCoordinationError> {
        self.with_state(|state| state.require_external_reconciliation())
    }

    pub fn request_edit_lease(
        &self,
        request: EditLeaseRequest,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.request_edit_lease(request, evidence, now_ms)
        })
    }

    pub fn acknowledge_ui_quiescence(
        &self,
        client_session_id: &str,
        acknowledgement: UiQuiescenceAcknowledgement,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.acknowledge_ui_quiescence(client_session_id, acknowledgement, evidence, now_ms)
        })
    }

    pub fn renew_edit_lease(
        &self,
        client_session_id: &str,
        lease_id: &str,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.renew_edit_lease(client_session_id, lease_id, evidence, now_ms)
        })
    }

    pub fn release_edit_lease(
        &self,
        input: ReleaseEditLeaseInput,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.release_edit_lease(input, evidence, now_ms)
        })
    }

    pub fn accept_conflict_for_reconciliation(
        &self,
        project_session_id: &str,
        project_revision: u64,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.accept_conflict_for_reconciliation(project_session_id, project_revision, now_ms)
        })
    }

    pub fn complete_reconciliation(
        &self,
        input: ReconciliationInput,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.complete_reconciliation(input, now_ms)
        })
    }

    pub fn authorize_reconciliation_recovery_reload(
        &self,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.authorize_reconciliation_recovery_reload(evidence, now_ms)
        })
    }

    pub fn complete_reconciliation_recovery_reload(
        &self,
        lease_id: &str,
        expected_replacement_session_id: &str,
        evidence: &ProjectCoordinationEvidence,
        now_ms: u128,
    ) -> Result<EditTransitionReceipt, EditCoordinationError> {
        self.with_state(|state| {
            state.expire(now_ms);
            state.complete_reconciliation_recovery_reload(
                lease_id,
                expected_replacement_session_id,
                evidence,
                now_ms,
            )
        })
    }

    pub fn expire(
        &self,
        now_ms: u128,
    ) -> Result<Option<EditTransitionReceipt>, EditCoordinationError> {
        self.with_state(|state| Ok(state.expire(now_ms)))
    }

    fn with_state<T>(
        &self,
        operation: impl FnOnce(&mut AiCoordinationState) -> Result<T, EditCoordinationError>,
    ) -> Result<T, EditCoordinationError> {
        let mut state = self.state.lock().map_err(|_| {
            EditCoordinationError::new("AiCoordinationRuntime mutex este compromis.")
        })?;
        operation(&mut state)
    }
}
