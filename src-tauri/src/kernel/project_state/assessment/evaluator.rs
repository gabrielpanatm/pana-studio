use crate::kernel::disk_conflict::{KernelDiskConflictSnapshot, KernelDiskConflictStatus};

use crate::kernel::project_state::model::{KernelProjectStateReason, KernelProjectStateStatus};

use super::{context::ProjectStateAssessmentContext, metrics::ProjectStateMetrics};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectStateAssessmentVerdict {
    pub(super) status: KernelProjectStateStatus,
    pub(super) reason: KernelProjectStateReason,
    pub(super) verdict_reason: String,
}

impl ProjectStateAssessmentVerdict {
    fn new(
        status: KernelProjectStateStatus,
        reason: KernelProjectStateReason,
        verdict_reason: impl Into<String>,
    ) -> Self {
        Self {
            status,
            reason,
            verdict_reason: verdict_reason.into(),
        }
    }
}

pub(super) fn evaluate_project_state_verdict(
    context: &ProjectStateAssessmentContext,
    disk_conflicts: Option<&KernelDiskConflictSnapshot>,
    metrics: &ProjectStateMetrics,
) -> ProjectStateAssessmentVerdict {
    if !context.project_open {
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Idle,
            KernelProjectStateReason::NoProject,
            "Nicio sesiune de proiect nu este deschisă în nucleu.",
        );
    }

    if !context.session_available {
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Blocked,
            KernelProjectStateReason::ProjectSessionMissing,
            "ProjectState nu poate fi autoritar fără ProjectSession.",
        );
    }

    if !context.project_workspace_available {
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Blocked,
            KernelProjectStateReason::ProjectWorkspaceMissing,
            "ProjectState nu poate evalua sesiunea fără ProjectWorkspace.",
        );
    }

    if metrics.workspace_dirty {
        let mut sources = Vec::new();
        if metrics.workspace_dirty_document_count > 0 {
            sources.push(format!(
                "{} documente modificate",
                metrics.workspace_dirty_document_count
            ));
        }
        if metrics.workspace_created_document_count > 0 {
            sources.push(format!(
                "{} documente create",
                metrics.workspace_created_document_count
            ));
        }
        if metrics.workspace_deleted_document_count > 0 {
            sources.push(format!(
                "{} documente șterse",
                metrics.workspace_deleted_document_count
            ));
        }
        if metrics.workspace_dirty_page_js_count > 0 {
            sources.push(format!(
                "{} resurse Page JS",
                metrics.workspace_dirty_page_js_count
            ));
        }
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Dirty,
            KernelProjectStateReason::WorkspaceDirty,
            format!(
                "ProjectWorkspace conține {} resurse nesalvate ({}).",
                metrics.workspace_dirty_resource_count,
                sources.join(", ")
            ),
        );
    }

    let Some(disk_conflicts) = disk_conflicts else {
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Blocked,
            KernelProjectStateReason::DiskConflictSnapshotMissing,
            "ProjectState nu poate confirma relația FileBufferStore ↔ disk fără Disk Conflict Snapshot.",
        );
    };

    if disk_conflicts.summary.status == KernelDiskConflictStatus::Error {
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Blocked,
            KernelProjectStateReason::DiskUnverifiable,
            format!(
                "{} fișiere urmărite nu pot fi verificate sigur față de disk.",
                metrics.unreadable_file_count
            ),
        );
    }

    if disk_conflicts.summary.conflict_count > 0 {
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Warning,
            KernelProjectStateReason::DiskConflict,
            format!(
                "{} fișiere urmărite diferă de baseline sau ar bloca Save Engine.",
                disk_conflicts.summary.conflict_count
            ),
        );
    }

    if metrics.metadata_changed_count > 0 {
        return ProjectStateAssessmentVerdict::new(
            KernelProjectStateStatus::Info,
            KernelProjectStateReason::MetadataChanged,
            format!(
                "{} fișiere au metadata schimbată, dar hash-ul text rămâne aliniat cu baseline-ul.",
                metrics.metadata_changed_count
            ),
        );
    }

    ProjectStateAssessmentVerdict::new(
        KernelProjectStateStatus::Clean,
        KernelProjectStateReason::Clean,
        "ProjectWorkspace și manifestul acceptat de pe disk sunt aliniate.",
    )
}
