use std::{path::PathBuf, time::Instant};

use tauri::State;

use crate::{
    kernel::{
        preview_projection::PreviewStructuralCommandIdentity,
        project_session::ProjectSessionSnapshot,
    },
    project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    state::AppState,
};

pub(super) struct PreviewWriteCommandContext {
    pub(super) root: PathBuf,
    pub(super) session: ProjectSessionSnapshot,
    pub(super) accepted_disk: AcceptedProjectDiskManifest,
    pub(super) active_document_path: Option<String>,
    pub(super) workspace_revision: u64,
}

pub(super) fn prepare_preview_write_command(
    state: &State<AppState>,
    identity: &PreviewStructuralCommandIdentity,
) -> Result<PreviewWriteCommandContext, String> {
    let (root, session, accepted_disk, workspace_revision) =
        capture_preview_workspace_authority(state)?;
    require_preview_command_identity(&session, identity)?;
    let workbench = state.workbench.read(&session)?;
    let active_document_path = workbench
        .groups
        .iter()
        .find(|group| group.group_id == workbench.active_group_id)
        .and_then(|group| {
            group.active_document_id.as_deref().and_then(|document_id| {
                group
                    .documents
                    .iter()
                    .find(|document| document.document_id == document_id)
            })
        })
        .map(|document| document.relative_path.clone());

    Ok(PreviewWriteCommandContext {
        root,
        session,
        accepted_disk,
        active_document_path,
        workspace_revision,
    })
}

pub(super) fn require_preview_command_identity(
    session: &ProjectSessionSnapshot,
    identity: &PreviewStructuralCommandIdentity,
) -> Result<(), String> {
    let expected_root = identity.expected_project_root.trim();
    let expected_session_id = identity.expected_session_id.trim();
    if expected_root.is_empty() || expected_session_id.is_empty() {
        return Err("Preview Projection a refuzat o identitate root/session goală.".to_string());
    }
    let live_session_id = session.runtime_instance_id();
    if expected_root != session.project_root || expected_session_id != live_session_id {
        return Err(format!(
            "Preview Projection a refuzat un request stale: așteptat root/session {expected_root}/{expected_session_id}, activ {}/{live_session_id}.",
            session.project_root
        ));
    }
    Ok(())
}

pub(super) fn capture_preview_write_workspace_candidate(
    state: &State<AppState>,
    context: &PreviewWriteCommandContext,
) -> Result<(crate::kernel::project_workspace::ProjectWorkspace, u64), String> {
    let clone_started = Instant::now();
    let candidate = {
        let workspace = state
            .project_workspace
            .lock()
            .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
        require_preview_accepted_snapshot(
            Some(&workspace.accepted_disk),
            &context.session,
            &context.accepted_disk,
        )?;
        if workspace.revision != context.workspace_revision {
            return Err(format!(
                "Preview Projection a refuzat o mutație stale: revizia pregătită {}, revizia activă {}.",
                context.workspace_revision, workspace.revision
            ));
        }
        workspace.clone()
    };
    let clone_ms = clone_started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    require_accepted_disk_unchanged(&context.root, &context.accepted_disk)?;
    Ok((candidate, clone_ms))
}

pub(super) fn with_preview_write_workspace_cas<R>(
    state: &State<AppState>,
    context: &PreviewWriteCommandContext,
    execute: impl FnOnce(&mut crate::kernel::project_workspace::ProjectWorkspace) -> Result<R, String>,
) -> Result<R, String> {
    let mut workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru CAS.".to_string())?;
    let workspace = workspace
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru CAS.".to_string())?;
    require_preview_accepted_snapshot(
        Some(&workspace.accepted_disk),
        &context.session,
        &context.accepted_disk,
    )?;
    if workspace.revision != context.workspace_revision {
        return Err(format!(
            "Preview Projection CAS a refuzat candidatul stale: revizia pregătită {}, revizia activă {}.",
            context.workspace_revision, workspace.revision
        ));
    }
    execute(workspace)
}

fn capture_preview_workspace_authority(
    state: &State<AppState>,
) -> Result<
    (
        PathBuf,
        ProjectSessionSnapshot,
        AcceptedProjectDiskManifest,
        u64,
    ),
    String,
> {
    let root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul proiectului curent.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace.".to_string())?;
    let root = root
        .as_ref()
        .ok_or_else(|| "Nu există proiect deschis.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let session = &workspace.session;
    let accepted_disk = &workspace.accepted_disk;
    if root != std::path::Path::new(&session.project_root) {
        return Err("Preview Projection a blocat o sesiune cu root inconsistent.".to_string());
    }
    accepted_disk.require_identity(&session.runtime_instance_id(), &session.project_root)?;
    accepted_disk.require_complete()?;
    Ok((
        root.clone(),
        session.clone(),
        accepted_disk.clone(),
        workspace.revision,
    ))
}

fn require_preview_accepted_snapshot<'a>(
    live: Option<&'a AcceptedProjectDiskManifest>,
    session: &ProjectSessionSnapshot,
    expected: &AcceptedProjectDiskManifest,
) -> Result<&'a AcceptedProjectDiskManifest, String> {
    let live = live.ok_or_else(|| {
        "Preview Projection a devenit stale: manifestul disk acceptat lipsește.".to_string()
    })?;
    live.require_identity(&session.runtime_instance_id(), &session.project_root)?;
    if live != expected {
        return Err(
            "Preview Projection a devenit stale: manifestul disk acceptat s-a schimbat."
                .to_string(),
        );
    }
    Ok(live)
}

pub(super) fn require_accepted_disk_unchanged(
    root: &std::path::Path,
    accepted: &AcceptedProjectDiskManifest,
) -> Result<(), String> {
    let live = read_project_disk_manifest(root)?;
    if live != accepted.manifest {
        return Err(
            "Preview Projection a fost blocată: disk-ul live conține schimbări neacceptate de ProjectSession."
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use crate::kernel::project_session::{ProjectRootFingerprint, ProjectSessionScanSummary};

    fn session(opened_at_ms: u128) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "stable-project-session".to_string(),
            project_root: "/tmp/project".to_string(),
            zola_root: "/tmp/project".to_string(),
            session_dir: "/tmp/app/session".to_string(),
            manifest_path: "/tmp/app/session/manifest.json".to_string(),
            opened_at_ms,
            last_seen_at_ms: opened_at_ms,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: "/tmp/project".to_string(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 1,
                directory_count: 1,
            },
        }
    }

    #[test]
    fn preview_identity_accepts_only_the_live_runtime_instance() {
        let live = session(10);
        let identity = PreviewStructuralCommandIdentity {
            expected_project_root: live.project_root.clone(),
            expected_session_id: live.runtime_instance_id(),
            expected_selection: None,
        };
        require_preview_command_identity(&live, &identity).unwrap();
    }

    #[test]
    fn preview_identity_rejects_same_root_reopened_runtime() {
        let stale = session(10);
        let live = session(11);
        let identity = PreviewStructuralCommandIdentity {
            expected_project_root: stale.project_root.clone(),
            expected_session_id: stale.runtime_instance_id(),
            expected_selection: None,
        };
        let error = require_preview_command_identity(&live, &identity).unwrap_err();
        assert!(error.contains("request stale"));
        assert!(error.contains(&live.runtime_instance_id()));
    }

    #[test]
    fn preview_identity_rejects_another_project_root() {
        let live = session(10);
        let identity = PreviewStructuralCommandIdentity {
            expected_project_root: "/tmp/other".to_string(),
            expected_session_id: live.runtime_instance_id(),
            expected_selection: None,
        };
        assert!(require_preview_command_identity(&live, &identity).is_err());
    }
}
