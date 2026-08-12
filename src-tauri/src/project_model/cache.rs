use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[cfg(debug_assertions)]
use std::time::Instant;

use crate::{
    kernel::{
        project_session::ProjectSessionSnapshot,
        project_workspace::{ProjectWorkspace, WorkspaceProjectionSnapshot},
    },
    project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    project_model::{
        model::ProjectModel, rebuild_project_model_after_workspace_change,
        ProjectModelIncrementalIntent,
    },
    state::AppState,
};

#[derive(Clone)]
pub(crate) struct ProjectModelBuildContext {
    projection: WorkspaceProjectionSnapshot,
    accepted_disk_generation: u64,
    accepted_disk_fingerprint: String,
    previous_model: Option<ProjectModel>,
    previous_model_source_revision: Option<u64>,
}

impl ProjectModelBuildContext {
    pub(crate) fn projection(&self) -> &WorkspaceProjectionSnapshot {
        &self.projection
    }

    pub(crate) fn model_cache_hit(&self) -> bool {
        self.previous_model.is_some()
            && self.previous_model_source_revision == Some(self.projection.revision)
    }
}

pub(crate) fn capture_project_model_build_context(
    state: &AppState,
) -> Result<(PathBuf, ProjectSessionSnapshot, ProjectModelBuildContext), String> {
    #[cfg(debug_assertions)]
    let started = Instant::now();
    let (root, session, accepted_disk, projection, previous_model, previous_model_source_revision) = {
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
            workspace.project_model.clone(),
            workspace.project_model_source_revision,
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
            previous_model,
            previous_model_source_revision,
        },
    );
    #[cfg(debug_assertions)]
    eprintln!(
        "[Pană Studio][perf] project_model_context total_ms={}",
        started.elapsed().as_millis()
    );
    Ok(result)
}

/// Builds from the immutable context while retaining the previous canonical
/// SourceNode identities. Even a forced full parse is reconciled against the
/// captured model instead of bootstrapping a parallel identity namespace.
pub(crate) fn build_project_model_from_context(
    root: &Path,
    context: &ProjectModelBuildContext,
) -> Result<ProjectModel, String> {
    if context.previous_model_source_revision == Some(context.projection.revision) {
        if let Some(model) = context.previous_model.as_ref() {
            return Ok(model.clone());
        }
    }
    rebuild_project_model_from_previous_projection(
        root,
        context.previous_model.as_ref(),
        context.previous_model_source_revision,
        &context.projection,
    )
}

pub(crate) fn rebuild_project_model_from_previous_projection(
    root: &Path,
    previous: Option<&ProjectModel>,
    previous_model_source_revision: Option<u64>,
    projection: &WorkspaceProjectionSnapshot,
) -> Result<ProjectModel, String> {
    let Some(previous) = previous else {
        return super::build_project_model_from_workspace_projection(root, projection);
    };
    if previous_model_source_revision == Some(projection.revision) {
        return Ok(previous.clone());
    }
    let changed_paths = changed_paths_since_model(previous, projection);
    let intent = incremental_intent_for_paths(&changed_paths);
    rebuild_project_model_after_workspace_change(
        root,
        Some(previous),
        previous_model_source_revision,
        projection,
        &changed_paths,
        intent,
    )
    .map(|outcome| outcome.model)
}

fn changed_paths_since_model(
    previous: &ProjectModel,
    projection: &WorkspaceProjectionSnapshot,
) -> Vec<String> {
    let mut changed = BTreeSet::new();
    for file in &previous.files {
        if projection
            .source_texts
            .get(&file.relative_path)
            .is_none_or(|source| source != &file.contents)
        {
            changed.insert(file.relative_path.clone());
        }
    }
    for (path, source) in &projection.source_texts {
        if previous
            .files
            .iter()
            .find(|file| file.relative_path == *path)
            .is_none_or(|file| &file.contents != source)
        {
            changed.insert(path.clone());
        }
    }
    changed.extend(projection.changed_paths.iter().cloned());
    changed.into_iter().collect()
}

fn incremental_intent_for_paths(paths: &[String]) -> ProjectModelIncrementalIntent {
    if matches!(paths, [path] if path.starts_with("templates/") && path.ends_with(".html")) {
        ProjectModelIncrementalIntent::HtmlStructural
    } else if !paths.is_empty()
        && paths
            .iter()
            .all(|path| path.ends_with(".css") || path.ends_with(".scss"))
    {
        ProjectModelIncrementalIntent::StyleDeclaration
    } else {
        ProjectModelIncrementalIntent::Unsupported
    }
}

pub(crate) fn publish_project_model_if_current(
    state: &AppState,
    context: &ProjectModelBuildContext,
    model: ProjectModel,
) -> Result<(), String> {
    #[cfg(debug_assertions)]
    let started = Instant::now();
    let result = publish_project_model_current(state, context, model);
    #[cfg(debug_assertions)]
    eprintln!(
        "[Pană Studio][perf] project_model_publish total_ms={} success={}",
        started.elapsed().as_millis(),
        result.is_ok()
    );
    result
}

pub(crate) fn current_project_model_if_fresh(
    state: &AppState,
) -> Result<Option<ProjectModel>, String> {
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut consulta ProjectModel-ul canonic.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    if workspace.project_model_source_revision != Some(workspace.revision) {
        return Ok(None);
    }
    Ok(workspace.project_model.clone())
}

/// Revalidates an immutable analysis context without publishing a model into
/// ProjectWorkspace. Audit uses this after its provider run because a
/// best-effort model containing project defects must never replace the model
/// used by editing commands.
pub(crate) fn validate_project_model_build_context_current(
    state: &AppState,
    context: &ProjectModelBuildContext,
) -> Result<(), String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut valida root-ul după Audit.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut valida ProjectWorkspace după Audit.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "Audit a devenit stale: proiectul a fost închis.".to_string())?;
    validate_live_context(&current_root, workspace, context)
}

fn publish_project_model_current(
    state: &AppState,
    context: &ProjectModelBuildContext,
    model: ProjectModel,
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

    workspace.publish_project_model(&context.projection, model)?;

    Ok(())
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{FileBufferStore, FileBufferStoreLimits},
            project_session::{ProjectRootFingerprint, ProjectSessionScanSummary},
            project_workspace::ProjectWorkspace,
        },
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
    };

    use super::*;

    #[test]
    fn audit_context_revalidation_rejects_a_changed_workspace_revision() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("zola.toml"), "base_url = '/'\n").unwrap();
        let root = root.canonicalize().unwrap();
        let session = session(&root);
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(&root).unwrap(),
        )
        .unwrap();
        let documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 100,
                max_file_bytes: 1_048_576,
                max_total_bytes: 4_194_304,
            },
        );
        let page_js = PageJsDraftStore::new(&session);
        let workspace =
            ProjectWorkspace::new(session, accepted.clone(), documents, page_js).unwrap();
        let context = ProjectModelBuildContext {
            projection: workspace.capture_projection_snapshot().unwrap(),
            accepted_disk_generation: accepted.generation,
            accepted_disk_fingerprint: accepted_disk_fingerprint(&accepted).unwrap(),
            previous_model: None,
            previous_model_source_revision: None,
        };
        let state = AppState::default();
        *state.current_root.lock().unwrap() = Some(root.clone());
        *state.project_workspace.lock().unwrap() = Some(workspace);
        assert!(validate_project_model_build_context_current(&state, &context).is_ok());
        state
            .project_workspace
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .revision += 1;
        assert!(validate_project_model_build_context_current(&state, &context).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    fn session(root: &Path) -> ProjectSessionSnapshot {
        let root = root.to_string_lossy().to_string();
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "audit-cache-test".to_string(),
            project_root: root.clone(),
            zola_root: root.clone(),
            session_dir: format!("{root}/session"),
            manifest_path: format!("{root}/session.json"),
            opened_at_ms: 11,
            last_seen_at_ms: 11,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root,
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                active_theme: None,
                file_count: 1,
                directory_count: 0,
            },
        }
    }

    fn unique_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-audit-cache-{stamp}"))
    }
}
