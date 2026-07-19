use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{preview::CanvasProjectionPlan, state::AppState};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPreviewRequestIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub expected_disk_generation: u64,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPreviewStartReceipt {
    pub url: String,
    pub project_root: String,
    pub runtime_session_id: String,
    pub accepted_disk_generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreviewRequestIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreviewStartReceipt {
    pub url: String,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub preview_revision: String,
    pub canvas_projection: CanvasProjectionPlan,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectPreviewMutationKind {
    WorkspaceProjection,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreviewMutationReceipt {
    pub operation: ProjectPreviewMutationKind,
    pub project_root: String,
    pub runtime_session_id: String,
    pub requested_paths: Vec<String>,
    pub preview_revision: Option<String>,
    pub canvas_projection: Option<CanvasProjectionPlan>,
    pub workspace_revision: u64,
}

impl ProjectPreviewMutationReceipt {
    pub fn new(
        identity: &ProjectPreviewRequestIdentity,
        operation: ProjectPreviewMutationKind,
        requested_paths: Vec<String>,
        canvas_projection: Option<CanvasProjectionPlan>,
        workspace_revision: u64,
    ) -> Self {
        Self {
            operation,
            project_root: identity.expected_project_root.clone(),
            runtime_session_id: identity.expected_session_id.clone(),
            requested_paths,
            preview_revision: canvas_projection
                .as_ref()
                .map(|plan| plan.identity.preview_revision.clone()),
            canvas_projection,
            workspace_revision,
        }
    }
}

pub fn require_browser_preview_session(
    state: &AppState,
    identity: &BrowserPreviewRequestIdentity,
) -> Result<PathBuf, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut valida root-ul pentru Browser preview.".to_string())?;
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut valida ProjectWorkspace pentru Browser preview.".to_string())?;
    require_browser_preview_session_from_parts(&current_root, &project_workspace, identity)
}

pub fn require_project_preview_session(
    state: &AppState,
    identity: &ProjectPreviewRequestIdentity,
) -> Result<PathBuf, String> {
    require_preview_session(
        state,
        &identity.expected_project_root,
        &identity.expected_session_id,
        "Preview embedded",
    )
}

pub fn require_project_preview_workspace_revision(
    state: &AppState,
    identity: &ProjectPreviewRequestIdentity,
    expected_workspace_revision: u64,
) -> Result<PathBuf, String> {
    let current_root = state.current_root.lock().map_err(|_| {
        "Nu am putut valida root-ul pentru proiecția ProjectWorkspace Preview.".to_string()
    })?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut valida ProjectWorkspace pentru proiecția Preview.".to_string())?;
    let project_session = workspace
        .as_ref()
        .map(|workspace| workspace.session.clone());
    let root = require_preview_session_from_parts(
        &current_root,
        &project_session,
        &identity.expected_project_root,
        &identity.expected_session_id,
        "Proiecția ProjectWorkspace Preview",
    )?;
    let live_revision = workspace
        .as_ref()
        .expect("session validation proves ProjectWorkspace exists")
        .revision;
    if live_revision != expected_workspace_revision {
        return Err(format!(
            "Proiecția Preview a devenit stale: revizia ProjectWorkspace așteptată este {expected_workspace_revision}, iar cea activă este {live_revision}."
        ));
    }
    Ok(root)
}

fn require_preview_session(
    state: &AppState,
    expected_project_root: &str,
    expected_session_id: &str,
    scope: &str,
) -> Result<PathBuf, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| format!("Nu am putut valida root-ul pentru {scope}."))?;
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| format!("Nu am putut valida ProjectWorkspace pentru {scope}."))?;
    let project_session = project_workspace
        .as_ref()
        .map(|workspace| workspace.session.clone());
    require_preview_session_from_parts(
        &current_root,
        &project_session,
        expected_project_root,
        expected_session_id,
        scope,
    )
}

fn require_preview_session_from_parts(
    current_root: &Option<PathBuf>,
    project_session: &Option<crate::kernel::project_session::ProjectSessionSnapshot>,
    expected_project_root: &str,
    expected_session_id: &str,
    scope: &str,
) -> Result<PathBuf, String> {
    let expected_project_root = expected_project_root.trim();
    let expected_session_id = expected_session_id.trim();
    if expected_project_root.is_empty() || expected_session_id.is_empty() {
        return Err(format!(
            "{scope} cere root-ul și identitatea runtime a ProjectSession."
        ));
    }

    let live_root = current_root
        .as_ref()
        .ok_or_else(|| format!("{scope} a fost anulat: proiectul a fost închis."))?;
    let session = project_session
        .as_ref()
        .ok_or_else(|| format!("{scope} a fost anulat: ProjectSession nu este inițializat."))?;
    let expected_root = Path::new(expected_project_root);
    let session_root = Path::new(&session.project_root);
    let live_session_id = session.runtime_instance_id();

    if live_root != expected_root
        || session_root != expected_root
        || live_session_id != expected_session_id
    {
        return Err(format!(
            "{scope} a refuzat un request stale: așteptat root/session {expected_project_root}/{expected_session_id}, activ {}/{}.",
            live_root.display(),
            live_session_id
        ));
    }

    Ok(live_root.clone())
}

fn require_browser_preview_session_from_parts(
    current_root: &Option<PathBuf>,
    project_workspace: &Option<crate::kernel::project_workspace::ProjectWorkspace>,
    identity: &BrowserPreviewRequestIdentity,
) -> Result<PathBuf, String> {
    let project_session = project_workspace
        .as_ref()
        .map(|workspace| workspace.session.clone());
    let root = require_preview_session_from_parts(
        current_root,
        &project_session,
        &identity.expected_project_root,
        &identity.expected_session_id,
        "Browser preview",
    )?;
    let workspace = project_workspace
        .as_ref()
        .expect("session validation proves ProjectWorkspace exists");
    let live_generation = workspace.accepted_disk.generation;
    if identity.expected_disk_generation == 0
        || live_generation != identity.expected_disk_generation
    {
        return Err(format!(
            "Browser preview a refuzat o generație AcceptedDisk stale: așteptat {}, activ {}.",
            identity.expected_disk_generation, live_generation
        ));
    }
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &root,
    )?;
    Ok(root)
}
