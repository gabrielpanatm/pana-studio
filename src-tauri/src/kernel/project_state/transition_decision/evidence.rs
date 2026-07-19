use crate::kernel::{
    disk_conflict::{KernelDiskConflictKind, KernelDiskConflictSnapshot},
    file_buffer_store::FileBufferStore,
    project_session::ProjectSessionSnapshot,
    project_workspace::ProjectWorkspaceSnapshot,
};

use super::super::{KernelProjectStateSnapshot, KernelProjectTransitionPolicy};
use super::{
    KernelProjectTransitionDecisionEvidence, KernelProjectTransitionDirtyFileEvidence,
    KernelProjectTransitionDiskFileEvidence, KernelProjectTransitionWorkspaceEvidence,
    KERNEL_PROJECT_TRANSITION_DECISION_EVIDENCE_SCHEMA_VERSION,
};

pub fn build_kernel_project_transition_decision_evidence(
    session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
    disk_conflicts: Option<&KernelDiskConflictSnapshot>,
    workspace: &ProjectWorkspaceSnapshot,
    project_state: &KernelProjectStateSnapshot,
    policy: &KernelProjectTransitionPolicy,
    target_project_root: &str,
) -> Result<KernelProjectTransitionDecisionEvidence, String> {
    let dirty_files = store
        .files
        .values()
        .filter(|entry| entry.is_dirty())
        .map(|entry| KernelProjectTransitionDirtyFileEvidence {
            relative_path: entry.relative_path.clone(),
            baseline_hash: entry.baseline.hash.clone(),
            current_hash: entry.current_hash(),
            current_bytes: entry.current_bytes(),
            revision: entry.revision,
        })
        .collect::<Vec<_>>();

    Ok(KernelProjectTransitionDecisionEvidence {
        schema_version: KERNEL_PROJECT_TRANSITION_DECISION_EVIDENCE_SCHEMA_VERSION,
        action: policy.action,
        target_project_root: target_project_root.to_string(),
        session_id: session.id.clone(),
        project_root: session.project_root.clone(),
        project_state_status: project_state.status,
        project_state_reason: project_state.reason,
        transition_decision: policy.decision,
        transition_reason: policy.reason,
        workspace_dirty_resource_count: project_state.workspace_dirty_resource_count,
        dirty_files,
        disk_files: disk_conflict_evidence(disk_conflicts),
        workspace: workspace_evidence(workspace)?,
    })
}

fn disk_conflict_evidence(
    snapshot: Option<&KernelDiskConflictSnapshot>,
) -> Vec<KernelProjectTransitionDiskFileEvidence> {
    snapshot
        .into_iter()
        .flat_map(|snapshot| snapshot.files.iter())
        .filter(|file| {
            !matches!(
                file.kind,
                KernelDiskConflictKind::Clean
                    | KernelDiskConflictKind::DirtyOnly
                    | KernelDiskConflictKind::MetadataChanged
            )
        })
        .map(|file| KernelProjectTransitionDiskFileEvidence {
            relative_path: file.relative_path.clone(),
            kind: disk_conflict_kind_code(file.kind).to_string(),
            baseline_hash: file.baseline.hash.clone(),
            disk_hash: file.disk.as_ref().map(|baseline| baseline.hash.clone()),
            revision: file.revision,
        })
        .collect()
}

fn disk_conflict_kind_code(kind: KernelDiskConflictKind) -> &'static str {
    match kind {
        KernelDiskConflictKind::Clean => "clean",
        KernelDiskConflictKind::DirtyOnly => "dirty_only",
        KernelDiskConflictKind::MetadataChanged => "metadata_changed",
        KernelDiskConflictKind::DiskChanged => "disk_changed",
        KernelDiskConflictKind::MissingOnDisk => "missing_on_disk",
        KernelDiskConflictKind::Readonly => "readonly",
        KernelDiskConflictKind::NotFile => "not_file",
        KernelDiskConflictKind::Oversized => "oversized",
        KernelDiskConflictKind::Unreadable => "unreadable",
        KernelDiskConflictKind::InvalidPath => "invalid_path",
    }
}

fn workspace_evidence(
    workspace: &ProjectWorkspaceSnapshot,
) -> Result<KernelProjectTransitionWorkspaceEvidence, String> {
    let serialized = serde_json::to_string(workspace)
        .map_err(|error| format!("ProjectWorkspace evidence nu poate fi serializată: {error}"))?;
    Ok(KernelProjectTransitionWorkspaceEvidence {
        revision: workspace.revision,
        disk_generation: workspace.disk_generation,
        dirty: workspace.dirty,
        dirty_document_count: workspace.dirty_document_count,
        created_document_count: workspace.created_document_count,
        deleted_document_count: workspace.deleted_document_count,
        dirty_page_js_count: workspace.dirty_page_js_count,
        undo_count: workspace.history.undo_count,
        redo_count: workspace.history.redo_count,
        fingerprint: crate::kernel::file_buffer_store::hash_text(&serialized),
    })
}

#[cfg(test)]
mod tests {
    use crate::kernel::{
        disk_conflict::{
            KernelDiskConflictFileSnapshot, KernelDiskConflictKind, KernelDiskConflictSnapshot,
            KernelDiskConflictStatus, KernelDiskConflictSummary,
            KERNEL_DISK_CONFLICT_SCHEMA_VERSION,
        },
        file_buffer_store::{FileBufferBaseline, TextBufferLanguage, TextBufferRole},
    };

    use super::disk_conflict_evidence;

    #[test]
    fn transition_evidence_captures_baseline_and_disk_hashes() {
        let baseline = FileBufferBaseline {
            hash: "baseline-hash".to_string(),
            modified_ms: 1,
            size: 3,
            readonly: false,
        };
        let disk = FileBufferBaseline {
            hash: "disk-hash".to_string(),
            modified_ms: 2,
            size: 3,
            readonly: false,
        };
        let snapshot = KernelDiskConflictSnapshot {
            schema_version: KERNEL_DISK_CONFLICT_SCHEMA_VERSION,
            session_id: "session-1".to_string(),
            project_root: "/project".to_string(),
            scanned_at_ms: 2,
            max_file_bytes: 1024,
            summary: KernelDiskConflictSummary {
                status: KernelDiskConflictStatus::Warning,
                verdict_reason: "changed".to_string(),
                tracked_file_count: 1,
                clean_count: 0,
                dirty_only_count: 0,
                metadata_changed_count: 0,
                disk_changed_count: 1,
                missing_on_disk_count: 0,
                readonly_count: 0,
                not_file_count: 0,
                oversized_count: 0,
                unreadable_count: 0,
                invalid_path_count: 0,
                conflict_count: 1,
                blocking_count: 1,
            },
            files: vec![KernelDiskConflictFileSnapshot {
                relative_path: "sursa/templates/index.html".to_string(),
                absolute_path: "/project/sursa/templates/index.html".to_string(),
                language: TextBufferLanguage::Html,
                role: TextBufferRole::Template,
                status: KernelDiskConflictStatus::Warning,
                kind: KernelDiskConflictKind::DiskChanged,
                message: "changed".to_string(),
                baseline,
                disk: Some(disk),
                has_draft: false,
                dirty: false,
                revision: 4,
            }],
        };

        let evidence = disk_conflict_evidence(Some(&snapshot));

        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].kind, "disk_changed");
        assert_eq!(evidence[0].baseline_hash, "baseline-hash");
        assert_eq!(evidence[0].disk_hash.as_deref(), Some("disk-hash"));
        assert_eq!(evidence[0].revision, 4);
    }
}
