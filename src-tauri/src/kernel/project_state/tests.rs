use crate::{
    js::PageJsDraftStore,
    kernel::{
        disk_conflict::{
            KernelDiskConflictSnapshot, KernelDiskConflictStatus, KernelDiskConflictSummary,
            KERNEL_DISK_CONFLICT_SCHEMA_VERSION,
        },
        file_buffer_store::{
            hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore, FileBufferStoreLimits,
            TextBufferLanguage, TextBufferRole,
        },
        project_session::{
            ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
        },
        project_workspace::{
            ProjectWorkspace, ProjectWorkspaceIdentity, WorkspaceDocumentMutation,
            WorkspaceMutationMetadata,
        },
    },
    project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
};

use super::{
    build_kernel_project_state_snapshot, evaluate_project_transition_policy,
    KernelProjectStateReason, KernelProjectStateStatus, KernelProjectTransitionAction,
    KernelProjectTransitionDecision, KernelProjectTransitionReason,
};

#[test]
fn no_project_is_idle_and_allows_open_without_operator() {
    let state = build_kernel_project_state_snapshot(None, None, None, None);
    let policy =
        evaluate_project_transition_policy(KernelProjectTransitionAction::OpenProject, &state);

    assert_eq!(state.status, KernelProjectStateStatus::Idle);
    assert_eq!(state.reason, KernelProjectStateReason::NoProject);
    assert_eq!(policy.decision, KernelProjectTransitionDecision::Allow);
    assert_eq!(policy.reason, KernelProjectTransitionReason::NoOpenProject);
}

#[test]
fn dirty_workspace_is_the_transition_authority_even_before_disk_scan() {
    let mut workspace = workspace();
    workspace
        .stage_document_texts(
            &identity(&workspace),
            mutation_metadata("Edit title"),
            vec![WorkspaceDocumentMutation {
                relative_path: "templates/index.html".to_string(),
                contents: "<h1>Draft</h1>".to_string(),
            }],
            10,
        )
        .unwrap();
    let snapshot = workspace.snapshot();
    let state = build_kernel_project_state_snapshot(
        Some(&workspace.session.project_root),
        Some(&workspace.session),
        Some(&snapshot),
        None,
    );

    assert_eq!(state.status, KernelProjectStateStatus::Dirty);
    assert_eq!(state.reason, KernelProjectStateReason::WorkspaceDirty);
    assert_eq!(state.workspace_revision, Some(1));
    assert_eq!(state.workspace_dirty_resource_count, 1);
    assert_eq!(state.workspace_undo_count, 1);
    assert_eq!(state.workspace_redo_count, 0);
    assert!(!state.write_blocked);

    for action in [
        KernelProjectTransitionAction::OpenProject,
        KernelProjectTransitionAction::ReloadProject,
        KernelProjectTransitionAction::CloseProject,
    ] {
        let policy = evaluate_project_transition_policy(action, &state);
        assert_eq!(policy.decision, KernelProjectTransitionDecision::Confirm);
        assert_eq!(policy.reason, KernelProjectTransitionReason::WorkspaceDirty);
        assert_eq!(policy.workspace_revision, Some(1));
        assert_eq!(policy.workspace_dirty_resource_count, 1);
    }
}

#[test]
fn missing_workspace_blocks_an_open_session() {
    let session = session();
    let state = build_kernel_project_state_snapshot(
        Some(&session.project_root),
        Some(&session),
        None,
        None,
    );
    let policy =
        evaluate_project_transition_policy(KernelProjectTransitionAction::CloseProject, &state);

    assert_eq!(state.status, KernelProjectStateStatus::Blocked);
    assert_eq!(
        state.reason,
        KernelProjectStateReason::ProjectWorkspaceMissing
    );
    assert!(state.write_blocked);
    assert_eq!(policy.decision, KernelProjectTransitionDecision::Block);
    assert_eq!(
        policy.reason,
        KernelProjectTransitionReason::BlockedProjectState
    );
}

#[test]
fn clean_workspace_and_clean_disk_allow_every_transition() {
    let workspace = workspace();
    let snapshot = workspace.snapshot();
    let disk = disk_snapshot(&workspace.session, KernelDiskConflictStatus::Clean, 0, 0);
    let state = build_kernel_project_state_snapshot(
        Some(&workspace.session.project_root),
        Some(&workspace.session),
        Some(&snapshot),
        Some(&disk),
    );

    assert_eq!(state.status, KernelProjectStateStatus::Clean);
    assert_eq!(state.reason, KernelProjectStateReason::Clean);
    assert!(state.is_clean);
    for action in [
        KernelProjectTransitionAction::OpenProject,
        KernelProjectTransitionAction::ReloadProject,
        KernelProjectTransitionAction::CloseProject,
    ] {
        assert_eq!(
            evaluate_project_transition_policy(action, &state).decision,
            KernelProjectTransitionDecision::Allow
        );
    }
}

#[test]
fn disk_conflict_allows_only_explicit_reload_confirmation() {
    let workspace = workspace();
    let snapshot = workspace.snapshot();
    let disk = disk_snapshot(&workspace.session, KernelDiskConflictStatus::Warning, 1, 1);
    let state = build_kernel_project_state_snapshot(
        Some(&workspace.session.project_root),
        Some(&workspace.session),
        Some(&snapshot),
        Some(&disk),
    );

    assert_eq!(state.status, KernelProjectStateStatus::Warning);
    assert_eq!(state.reason, KernelProjectStateReason::DiskConflict);
    assert!(state.write_blocked);
    assert_eq!(
        evaluate_project_transition_policy(KernelProjectTransitionAction::ReloadProject, &state)
            .decision,
        KernelProjectTransitionDecision::Confirm
    );
    for action in [
        KernelProjectTransitionAction::OpenProject,
        KernelProjectTransitionAction::CloseProject,
    ] {
        assert_eq!(
            evaluate_project_transition_policy(action, &state).decision,
            KernelProjectTransitionDecision::Block
        );
    }
}

fn workspace() -> ProjectWorkspace {
    let session = session();
    let text = "<h1>Disk</h1>";
    let relative_path = "templates/index.html";
    let mut documents = FileBufferStore::for_project_session(
        &session,
        1,
        FileBufferStoreLimits {
            max_files: 16,
            max_file_bytes: 1024 * 1024,
            max_total_bytes: 4 * 1024 * 1024,
        },
    );
    documents.insert_loaded_file(FileBufferEntry {
        relative_path: relative_path.to_string(),
        absolute_path: format!("{}/{relative_path}", session.project_root),
        language: TextBufferLanguage::Html,
        role: TextBufferRole::Template,
        baseline: FileBufferBaseline {
            hash: hash_text(text),
            modified_ms: 1,
            size: text.len() as u64,
            readonly: false,
        },
        baseline_text: text.to_string(),
        draft: None,
        revision: 1,
    });
    let accepted = AcceptedProjectDiskManifest::new(
        session.runtime_instance_id(),
        session.project_root.clone(),
        ProjectDiskManifest {
            root: session.project_root.clone(),
            files: Vec::new(),
            truncated: false,
            max_files: 100,
        },
    )
    .unwrap();
    let page_js = PageJsDraftStore::new(&session);
    ProjectWorkspace::new(session, accepted, documents, page_js).unwrap()
}

fn session() -> ProjectSessionSnapshot {
    let project_root = "/tmp/pana-project-state-authority".to_string();
    ProjectSessionSnapshot {
        schema_version: 1,
        id: "project-state-test".to_string(),
        project_root: project_root.clone(),
        zola_root: project_root.to_string(),
        session_dir: "/tmp/pana-project-state-session".to_string(),
        manifest_path: "/tmp/pana-project-state-session/manifest.json".to_string(),
        opened_at_ms: 7,
        last_seen_at_ms: 7,
        root_fingerprint: ProjectRootFingerprint {
            canonical_path: project_root,
            modified_ms: 1,
            size: 0,
            readonly: false,
            unix_device: None,
            unix_inode: None,
        },
        scan_summary: ProjectSessionScanSummary {
            is_zola: true,
            is_empty: false,
            active_theme: None,
            file_count: 1,
            directory_count: 2,
        },
    }
}

fn identity(workspace: &ProjectWorkspace) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

fn mutation_metadata(label: &str) -> WorkspaceMutationMetadata {
    WorkspaceMutationMetadata {
        label: label.to_string(),
        source: "project_state.test".to_string(),
        coalesce_key: None,
        transaction_id: None,
    }
}

fn disk_snapshot(
    session: &ProjectSessionSnapshot,
    status: KernelDiskConflictStatus,
    conflict_count: usize,
    blocking_count: usize,
) -> KernelDiskConflictSnapshot {
    KernelDiskConflictSnapshot {
        schema_version: KERNEL_DISK_CONFLICT_SCHEMA_VERSION,
        session_id: session.runtime_instance_id(),
        project_root: session.project_root.clone(),
        scanned_at_ms: 1,
        max_file_bytes: 1024 * 1024,
        summary: KernelDiskConflictSummary {
            status,
            verdict_reason: "test".to_string(),
            tracked_file_count: 1,
            clean_count: usize::from(conflict_count == 0),
            dirty_only_count: 0,
            metadata_changed_count: 0,
            disk_changed_count: conflict_count,
            missing_on_disk_count: 0,
            readonly_count: 0,
            not_file_count: 0,
            oversized_count: 0,
            unreadable_count: 0,
            invalid_path_count: 0,
            conflict_count,
            blocking_count,
        },
        files: Vec::new(),
    }
}
