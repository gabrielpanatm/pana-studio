mod model;
mod runtime;
mod state_machine;

pub use model::{
    AiClientIdentity, AiClientSessionSnapshot, AiCoordinationSnapshot, AiPresenceStatus,
    EditAuthority, EditCoordinationError, EditLease, EditLeaseRequest, EditLeaseStatus,
    EditTransitionReceipt, ProjectCoordinationBlocker, ProjectCoordinationBlockerKind,
    ProjectCoordinationEvidence, ReconciliationInput, ReleaseEditLeaseInput, RequiredUserAction,
    UiQuiescenceAcknowledgement, AI_COORDINATION_SCHEMA_VERSION, DEFAULT_EDIT_LEASE_TTL_MS,
};
pub use runtime::AiCoordinationRuntime;

#[cfg(test)]
mod tests;
