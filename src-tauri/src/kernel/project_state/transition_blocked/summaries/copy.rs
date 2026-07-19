use crate::kernel::project_state::lifecycle_policy::KernelProjectTransitionAction;

use super::super::{
    KernelProjectTransitionBlockedCause, KernelProjectTransitionBlockedHealthStatus,
    KernelProjectTransitionResolutionSurface,
};

pub(super) struct BlockedCauseProfile {
    pub(super) surface: KernelProjectTransitionResolutionSurface,
    pub(super) title: &'static str,
    pub(super) detail: &'static str,
    pub(super) recommended_action: &'static str,
}

pub(super) fn blocked_health_copy(
    status: KernelProjectTransitionBlockedHealthStatus,
    record_count: usize,
    repeated_action_count: usize,
    repeated_cause_count: usize,
    diagnostic_count: usize,
) -> (String, String, String) {
    match status {
        KernelProjectTransitionBlockedHealthStatus::Clean => (
            "Lifecycle curat".to_string(),
            "Nu există blocaje ProjectTransition în fereastra de audit scanată.".to_string(),
            "Continuă monitorizarea prin Policy Matrix și Observability.".to_string(),
        ),
        KernelProjectTransitionBlockedHealthStatus::RecentlyBlocked => (
            "Lifecycle blocat recent".to_string(),
            format!(
                "{} blocaj ProjectTransition apare în fereastra de audit scanată, fără repetiție pe acțiune sau cauză.",
                record_count
            ),
            "Deschide suprafața recomandată pentru blocajul recent și verifică evidența înainte de reluarea tranziției.".to_string(),
        ),
        KernelProjectTransitionBlockedHealthStatus::RepeatedlyBlocked => (
            "Lifecycle blocat repetat".to_string(),
            format!(
                "{} blocaje ProjectTransition; {} acțiuni și {} cauze se repetă în fereastra scanată.",
                record_count, repeated_action_count, repeated_cause_count
            ),
            "Tratează cauza repetată ca risc operațional: inspectează latestByAction, apoi rezolvă autoritatea indicată înainte de noi tranziții.".to_string(),
        ),
        KernelProjectTransitionBlockedHealthStatus::Degraded => (
            "Audit ProjectTransition degradat".to_string(),
            format!(
                "Auditul blocajelor are {} diagnostics la citire, deci health-ul lifecycle nu este complet de încredere.",
                diagnostic_count
            ),
            "Inspectează diagnostics și Observability Log înainte de a considera lifecycle-ul stabil.".to_string(),
        ),
    }
}

pub(super) fn blocked_action_title(action: KernelProjectTransitionAction) -> &'static str {
    match action {
        KernelProjectTransitionAction::OpenProject => "Open Project blocat",
        KernelProjectTransitionAction::ReloadProject => "Reload Project blocat",
        KernelProjectTransitionAction::CloseProject => "Close Project blocat",
    }
}

pub(super) fn blocked_cause_profile(
    cause: KernelProjectTransitionBlockedCause,
) -> BlockedCauseProfile {
    match cause {
        KernelProjectTransitionBlockedCause::DiskConflict => BlockedCauseProfile {
            surface: KernelProjectTransitionResolutionSurface::DiskConflict,
            title: "Disk Conflict",
            detail: "Tranziția a fost oprită deoarece disk-ul nu mai corespunde baseline-ului sigur.",
            recommended_action: "Deschide Disk Conflict în Nucleu, verifică fișierele schimbate extern și readu baseline-ul într-o stare verificabilă înainte de open/reload/close.",
        },
        KernelProjectTransitionBlockedCause::WorkspaceDirty => BlockedCauseProfile {
            surface: KernelProjectTransitionResolutionSurface::ProjectWorkspace,
            title: "ProjectWorkspace cu schimbări nesalvate",
            detail: "Tranziția cere decizie operator deoarece revizia autoritativă conține resurse nesalvate și istoric care nu trebuie pierdute implicit.",
            recommended_action: "Revizuiește ProjectWorkspace și History, salvează revizia sau folosește dialogul Project Transition pentru o decizie explicită.",
        },
        KernelProjectTransitionBlockedCause::BlockedProjectState => BlockedCauseProfile {
            surface: KernelProjectTransitionResolutionSurface::Overview,
            title: "ProjectState blocat",
            detail: "Tranziția a fost oprită deoarece o autoritate critică a nucleului lipsește sau nu poate fi verificată.",
            recommended_action: "Deschide Kernel Overview și urmează diagnosticul blocant către autoritatea indicată înainte de a relua tranziția.",
        },
        KernelProjectTransitionBlockedCause::Unknown => BlockedCauseProfile {
            surface: KernelProjectTransitionResolutionSurface::Observability,
            title: "Cauză nespecializată",
            detail: "Tranziția a fost oprită de o combinație pe care auditul nu o clasifică încă specializat.",
            recommended_action: "Inspectează Observability Log și extinde ProjectTransitionPolicy cu un contract explicit pentru combinația nouă.",
        },
    }
}
