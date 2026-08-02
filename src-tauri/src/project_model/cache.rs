use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};

use crate::{
    kernel::{
        project_session::ProjectSessionSnapshot,
        project_workspace::{ProjectWorkspace, WorkspaceProjectionSnapshot},
    },
    project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    project_model::model::ProjectModel,
    state::AppState,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectModelBuildContext {
    projection: WorkspaceProjectionSnapshot,
    accepted_disk_generation: u64,
    accepted_disk_fingerprint: String,
}

impl ProjectModelBuildContext {
    pub(crate) fn projection(&self) -> &WorkspaceProjectionSnapshot {
        &self.projection
    }
}

pub(crate) fn capture_project_model_build_context(
    state: &AppState,
) -> Result<(PathBuf, ProjectSessionSnapshot, ProjectModelBuildContext), String> {
    let started = Instant::now();
    let (root, session, accepted_disk, projection) = {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut captura root-ul pentru ProjectModel context.".to_string())?;
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut captura ProjectWorkspace pentru ProjectModel context.".to_string()
        })?;

        let root = current_root
            .as_ref()
            .ok_or_else(|| "Nu există proiect deschis.".to_string())?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
        require_matching_root(root, &workspace.session)?;
        workspace.accepted_disk.require_identity(
            &workspace.session.runtime_instance_id(),
            &workspace.session.project_root,
        )?;
        workspace.accepted_disk.require_complete()?;

        (
            root.clone(),
            workspace.session.clone(),
            workspace.accepted_disk.clone(),
            workspace.capture_projection_snapshot()?,
        )
    };

    // Manifest traversal is filesystem work and may be slow on a large
    // project. It must not monopolize ProjectWorkspace: the immutable
    // authority snapshot is checked off-lock, while publish revalidates the
    // exact session, revision and accepted-disk generation before committing.
    require_accepted_disk_matches_live(&root, &session, &accepted_disk)?;
    let accepted_disk_generation = accepted_disk.generation;
    let accepted_disk_fingerprint = accepted_disk_fingerprint(&accepted_disk)?;

    let result = (
        root,
        session,
        ProjectModelBuildContext {
            projection,
            accepted_disk_generation,
            accepted_disk_fingerprint,
        },
    );
    #[cfg(debug_assertions)]
    eprintln!(
        "[Pană Studio][perf] project_model_context total_ms={}",
        started.elapsed().as_millis()
    );
    Ok(result)
}

pub(crate) fn publish_project_model_if_current(
    state: &AppState,
    context: &ProjectModelBuildContext,
    model: ProjectModel,
) -> Result<(), String> {
    let started = Instant::now();
    let result = publish_project_model_with_aliases_if_current(state, context, model, None);
    #[cfg(debug_assertions)]
    eprintln!(
        "[Pană Studio][perf] project_model_publish total_ms={} success={}",
        started.elapsed().as_millis(),
        result.is_ok()
    );
    result
}

pub(crate) fn publish_project_model_with_aliases_if_current(
    state: &AppState,
    context: &ProjectModelBuildContext,
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

    validate_live_context(&current_root, workspace, context)?;
    validate_model_root(&model, &context.projection.project_root)?;

    let live_source_ids = alias_updates
        .as_ref()
        .map(|_| project_model_source_ids(&model));
    workspace.publish_project_model(&context.projection, model)?;
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

fn validate_live_context(
    current_root: &Option<PathBuf>,
    workspace: &ProjectWorkspace,
    context: &ProjectModelBuildContext,
) -> Result<(), String> {
    let root = current_root.as_ref().ok_or_else(|| {
        "ProjectModel publish a devenit stale: proiectul a fost închis.".to_string()
    })?;
    require_matching_root(root, &workspace.session)?;
    if workspace.runtime_session_id() != context.projection.runtime_session_id
        || workspace.session.project_root != context.projection.project_root
    {
        return Err(
            "ProjectModel publish a devenit stale: instanța ProjectSession s-a schimbat."
                .to_string(),
        );
    }

    require_accepted_disk_matches_live(root, &workspace.session, &workspace.accepted_disk)?;
    if workspace.accepted_disk.generation != context.accepted_disk_generation
        || accepted_disk_fingerprint(&workspace.accepted_disk)? != context.accepted_disk_fingerprint
    {
        return Err(
            "ProjectModel publish a devenit stale: manifestul disk acceptat s-a schimbat."
                .to_string(),
        );
    }

    if workspace.revision != context.projection.revision {
        return Err(format!(
            "ProjectModel publish a devenit stale: generația context este {}, iar generația curentă este {}.",
            context.projection.revision, workspace.revision
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
        format!("AcceptedProjectDiskManifest nu poate fi serializat pentru context: {error}")
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
            "ProjectModel context a fost blocat: current_root este {}, iar ProjectSession aparține {}.",
            root.display(),
            session.project_root
        ));
    }
    Ok(())
}
