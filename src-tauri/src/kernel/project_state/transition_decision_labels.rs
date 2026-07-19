use super::{
    lifecycle_policy::{KernelProjectTransitionAction, KernelProjectTransitionReason},
    transition_decision::KernelProjectTransitionDecisionKind,
};

pub(super) fn action_title(action: KernelProjectTransitionAction) -> &'static str {
    match action {
        KernelProjectTransitionAction::OpenProject => "Open Project",
        KernelProjectTransitionAction::ReloadProject => "Reload Project",
        KernelProjectTransitionAction::CloseProject => "Close Project",
    }
}

pub(super) fn action_decision_title(action: KernelProjectTransitionAction) -> &'static str {
    match action {
        KernelProjectTransitionAction::OpenProject => "Open Project confirmat",
        KernelProjectTransitionAction::ReloadProject => "Reload Project confirmat",
        KernelProjectTransitionAction::CloseProject => "Close Project confirmat",
    }
}

pub(super) fn action_code(action: KernelProjectTransitionAction) -> &'static str {
    match action {
        KernelProjectTransitionAction::OpenProject => "open_project",
        KernelProjectTransitionAction::ReloadProject => "reload_project",
        KernelProjectTransitionAction::CloseProject => "close_project",
    }
}

pub(super) fn decision_kind_title(kind: KernelProjectTransitionDecisionKind) -> &'static str {
    match kind {
        KernelProjectTransitionDecisionKind::DiscardLocalDraftsForTransition => {
            "Discard local drafts"
        }
        KernelProjectTransitionDecisionKind::AcknowledgeDirtyHistoryForTransition => {
            "Acknowledge dirty history"
        }
        KernelProjectTransitionDecisionKind::DiscardSessionForExternalReload => {
            "Discard session for external reload"
        }
    }
}

pub(super) fn decision_kind_code(kind: KernelProjectTransitionDecisionKind) -> &'static str {
    match kind {
        KernelProjectTransitionDecisionKind::DiscardLocalDraftsForTransition => {
            "discard_local_drafts_for_transition"
        }
        KernelProjectTransitionDecisionKind::AcknowledgeDirtyHistoryForTransition => {
            "acknowledge_dirty_history_for_transition"
        }
        KernelProjectTransitionDecisionKind::DiscardSessionForExternalReload => {
            "discard_session_for_external_reload"
        }
    }
}

pub(super) fn transition_reason_code(reason: KernelProjectTransitionReason) -> &'static str {
    match reason {
        KernelProjectTransitionReason::NoOpenProject => "no_open_project",
        KernelProjectTransitionReason::Clean => "clean",
        KernelProjectTransitionReason::MetadataChanged => "metadata_changed",
        KernelProjectTransitionReason::WorkspaceDirty => "workspace_dirty",
        KernelProjectTransitionReason::DiskConflict => "disk_conflict",
        KernelProjectTransitionReason::BlockedProjectState => "blocked_project_state",
        KernelProjectTransitionReason::UnknownWarning => "unknown_warning",
    }
}
