use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    kernel::{
        project_session::ProjectSessionSnapshot,
        project_workspace::{ProjectWorkspace, WorkspaceProjectionLease},
    },
    project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    project_model::model::ProjectModel,
    state::AppState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectModelBuildLease {
    projection: WorkspaceProjectionLease,
    accepted_disk_generation: u64,
    accepted_disk_fingerprint: String,
}

impl ProjectModelBuildLease {
    pub(crate) fn projection(&self) -> &WorkspaceProjectionLease {
        &self.projection
    }
}

pub(crate) fn capture_project_model_build_lease(
    state: &AppState,
) -> Result<(PathBuf, ProjectSessionSnapshot, ProjectModelBuildLease), String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut captura root-ul pentru ProjectModel lease.".to_string())?;
    let workspace = state.project_workspace.lock().map_err(|_| {
        "Nu am putut captura ProjectWorkspace pentru ProjectModel lease.".to_string()
    })?;

    let root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect deschis.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    require_matching_root(root, &workspace.session)?;
    require_accepted_disk_matches_live(root, &workspace.session, &workspace.accepted_disk)?;

    Ok((
        root.clone(),
        workspace.session.clone(),
        ProjectModelBuildLease {
            projection: workspace.capture_projection_lease()?,
            accepted_disk_generation: workspace.accepted_disk.generation,
            accepted_disk_fingerprint: accepted_disk_fingerprint(&workspace.accepted_disk)?,
        },
    ))
}

pub(crate) fn publish_project_model_if_current(
    state: &AppState,
    lease: &ProjectModelBuildLease,
    model: ProjectModel,
) -> Result<(), String> {
    publish_project_model_with_aliases_if_current(state, lease, model, None)
}

pub(crate) fn publish_project_model_with_aliases_if_current(
    state: &AppState,
    lease: &ProjectModelBuildLease,
    model: ProjectModel,
    alias_updates: Option<Vec<(String, String)>>,
) -> Result<(), String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut valida root-ul pentru ProjectModel publish.".to_string())?;
    let mut workspace = state.project_workspace.lock().map_err(|_| {
        "Nu am putut bloca ProjectWorkspace pentru ProjectModel publish.".to_string()
    })?;
    let workspace = workspace.as_mut().ok_or_else(|| {
        "ProjectModel publish a devenit stale: proiectul a fost închis.".to_string()
    })?;

    validate_live_lease(&current_root, workspace, lease)?;
    validate_model_root(&model, &lease.projection.project_root)?;

    let live_source_ids = alias_updates
        .as_ref()
        .map(|_| project_model_source_ids(&model));
    workspace.publish_project_model(&lease.projection, model)?;
    if let (Some(updates), Some(live_source_ids)) = (alias_updates, live_source_ids) {
        reconcile_source_identity_aliases(
            &mut workspace.source_identity_aliases,
            &live_source_ids,
            updates,
        );
    }

    Ok(())
}

fn project_model_source_ids(model: &ProjectModel) -> HashSet<String> {
    model
        .source_graph
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect()
}

fn reconcile_source_identity_aliases(
    aliases: &mut HashMap<String, String>,
    live_source_ids: &HashSet<String>,
    alias_updates: Vec<(String, String)>,
) {
    // An identity can become live again when an edit is reversed (for example
    // add attribute A->B, remove attribute B->A). Any older outgoing edge from
    // that now-authoritative identity is stale and would otherwise form a
    // cycle. Prune stale edges first, then publish the aliases produced by the
    // current mutation; a current move may intentionally remap a reused
    // positional identity to the element that actually moved.
    aliases.retain(|from, _| !live_source_ids.contains(from));
    for (from, to) in alias_updates {
        if from != to {
            aliases.insert(from, to);
        }
    }
    if aliases.len() > 5000 {
        aliases.clear();
    }
}

pub(crate) fn invalidate_project_model_for_session(
    state: &AppState,
    expected_project_root: &str,
    expected_runtime_session_id: &str,
) -> Result<Option<u64>, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut valida root-ul pentru invalidarea ProjectModel.".to_string())?;
    let mut workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut invalida ProjectModel din ProjectWorkspace.".to_string())?;

    let (Some(root), Some(workspace)) = (current_root.as_ref(), workspace.as_mut()) else {
        return Ok(None);
    };
    if root.to_string_lossy() != expected_project_root
        || workspace.session.project_root != expected_project_root
        || workspace.runtime_session_id() != expected_runtime_session_id
    {
        return Ok(None);
    }

    Ok(Some(invalidate_project_model_locked(workspace)?))
}

pub(crate) fn invalidate_project_model_locked(
    workspace: &mut ProjectWorkspace,
) -> Result<u64, String> {
    workspace.project_model = None;
    workspace.project_model_source_revision = None;
    workspace.source_identity_aliases.clear();
    Ok(workspace.revision)
}

fn validate_live_lease(
    current_root: &Option<PathBuf>,
    workspace: &ProjectWorkspace,
    lease: &ProjectModelBuildLease,
) -> Result<(), String> {
    let root = current_root.as_ref().ok_or_else(|| {
        "ProjectModel publish a devenit stale: proiectul a fost închis.".to_string()
    })?;
    require_matching_root(root, &workspace.session)?;
    if workspace.runtime_session_id() != lease.projection.runtime_session_id
        || workspace.session.project_root != lease.projection.project_root
    {
        return Err(
            "ProjectModel publish a devenit stale: instanța ProjectSession s-a schimbat."
                .to_string(),
        );
    }

    require_accepted_disk_matches_live(root, &workspace.session, &workspace.accepted_disk)?;
    if workspace.accepted_disk.generation != lease.accepted_disk_generation
        || accepted_disk_fingerprint(&workspace.accepted_disk)? != lease.accepted_disk_fingerprint
    {
        return Err(
            "ProjectModel publish a devenit stale: manifestul disk acceptat s-a schimbat."
                .to_string(),
        );
    }

    if workspace.revision != lease.projection.revision {
        return Err(format!(
            "ProjectModel publish a devenit stale: generația lease este {}, iar generația curentă este {}.",
            lease.projection.revision, workspace.revision
        ));
    }
    Ok(())
}

fn require_accepted_disk_matches_live(
    root: &Path,
    session: &ProjectSessionSnapshot,
    accepted_disk: &AcceptedProjectDiskManifest,
) -> Result<(), String> {
    accepted_disk.require_identity(&session.runtime_instance_id(), &session.project_root)?;
    accepted_disk.require_complete()?;
    let live_manifest = read_project_disk_manifest(root)?;
    if live_manifest != accepted_disk.manifest {
        return Err(
            "ProjectModel a fost blocat: disk-ul live conține schimbări care nu au fost încă acceptate de ProjectSession."
                .to_string(),
        );
    }
    Ok(())
}

fn accepted_disk_fingerprint(accepted: &AcceptedProjectDiskManifest) -> Result<String, String> {
    serde_json::to_string(accepted).map_err(|error| {
        format!("AcceptedProjectDiskManifest nu poate fi serializat pentru lease: {error}")
    })
}

fn validate_model_root(model: &ProjectModel, expected_root: &str) -> Result<(), String> {
    let expected = Path::new(expected_root)
        .canonicalize()
        .map_err(|error| format!("ProjectModel publish nu poate valida root-ul: {error}"))?;
    if model.project_root != expected {
        return Err(format!(
            "ProjectModel publish a fost blocat: modelul aparține {}, nu {}.",
            model.project_root.display(),
            expected.display()
        ));
    }
    Ok(())
}

fn require_matching_root(root: &Path, session: &ProjectSessionSnapshot) -> Result<(), String> {
    if root != Path::new(&session.project_root) {
        return Err(format!(
            "ProjectModel lease a fost blocat: current_root este {}, iar ProjectSession aparține {}.",
            root.display(),
            session.project_root
        ));
    }
    Ok(())
}
