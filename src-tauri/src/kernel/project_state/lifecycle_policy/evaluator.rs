use crate::kernel::project_state::model::{
    KernelProjectStateReason, KernelProjectStateSnapshot, KernelProjectStateStatus,
};

use super::{
    policy::policy, KernelProjectTransitionAction, KernelProjectTransitionDecision,
    KernelProjectTransitionPolicy, KernelProjectTransitionReason,
};

pub fn evaluate_project_transition_policy(
    action: KernelProjectTransitionAction,
    project_state: &KernelProjectStateSnapshot,
) -> KernelProjectTransitionPolicy {
    let (decision, reason, title, message, recommended_action) =
        match (project_state.status, project_state.reason) {
            (KernelProjectStateStatus::Idle, _) => (
                KernelProjectTransitionDecision::Allow,
                KernelProjectTransitionReason::NoOpenProject,
                "Tranziție permisă",
                "Nu există o sesiune de proiect curentă care trebuie protejată.",
                "Continuă deschiderea proiectului ales.",
            ),
            (KernelProjectStateStatus::Clean, _) => (
                KernelProjectTransitionDecision::Allow,
                KernelProjectTransitionReason::Clean,
                "Tranziție permisă",
                "ProjectState raportează sesiunea curentă ca fiind clean.",
                "Continuă tranziția; ProjectSession și ProjectWorkspace sunt aliniate.",
            ),
            (KernelProjectStateStatus::Info, KernelProjectStateReason::MetadataChanged) => (
                KernelProjectTransitionDecision::Allow,
                KernelProjectTransitionReason::MetadataChanged,
                "Tranziție permisă cu audit informativ",
                "Există doar drift de metadata; hash-ul text rămâne aliniat cu baseline-ul.",
                "Continuă tranziția, dar păstrează drift-ul vizibil în Kernel Overview.",
            ),
            (KernelProjectStateStatus::Dirty, KernelProjectStateReason::WorkspaceDirty) => (
                KernelProjectTransitionDecision::Confirm,
                KernelProjectTransitionReason::WorkspaceDirty,
                "Tranziție oprită de ProjectState",
                "ProjectWorkspace conține modificări care nu au trecut prin granița Save.",
                "Salvează modificările sau confirmă explicit păstrarea/abandonarea sesiunii înainte de tranziție.",
            ),
            (KernelProjectStateStatus::Warning, KernelProjectStateReason::DiskConflict)
                if action == KernelProjectTransitionAction::ReloadProject => (
                    KernelProjectTransitionDecision::Confirm,
                    KernelProjectTransitionReason::DiskConflict,
                    "Reload de pe disk cere confirmare",
                    "Fișiere urmărite diferă de baseline. Reload Project va reconstrui FileBufferStore și va invalida istoricul sesiunii curente.",
                    "Confirmă explicit abandonarea proiecțiilor locale și reconstruirea autoritară din disk.",
                ),
            (KernelProjectStateStatus::Warning, KernelProjectStateReason::DiskConflict) => (
                KernelProjectTransitionDecision::Block,
                KernelProjectTransitionReason::DiskConflict,
                "Tranziție blocată de conflict disk",
                "Fișiere urmărite diferă de baseline sau ar bloca Save Engine.",
                "Folosește Reload Project explicit pentru discard/rebuild; close și open către alt proiect rămân blocate până la rezolvare.",
            ),
            (KernelProjectStateStatus::Blocked, _) => (
                KernelProjectTransitionDecision::Block,
                KernelProjectTransitionReason::BlockedProjectState,
                "Tranziție blocată de nucleu",
                "ProjectState este blocat; una dintre autoritățile critice ale sesiunii lipsește sau este neverificabilă.",
                "Deschide workspace-ul Nucleu și rezolvă diagnosticul blocant înainte de tranziție.",
            ),
            (KernelProjectStateStatus::Warning, _) => (
                KernelProjectTransitionDecision::Block,
                KernelProjectTransitionReason::UnknownWarning,
                "Tranziție blocată de avertisment necunoscut",
                "ProjectState raportează warning fără politică specializată.",
                "Inspectează Kernel Overview înainte de tranziție.",
            ),
            _ => (
                KernelProjectTransitionDecision::Block,
                KernelProjectTransitionReason::BlockedProjectState,
                "Tranziție blocată conservator",
                "ProjectState a produs o combinație status/reason pe care politica de lifecycle nu o tratează ca sigură.",
                "Extinde politica de lifecycle cu un contract explicit înainte de a permite tranziția.",
            ),
        };

    policy(
        action,
        project_state,
        decision,
        reason,
        title,
        message,
        recommended_action,
    )
}
