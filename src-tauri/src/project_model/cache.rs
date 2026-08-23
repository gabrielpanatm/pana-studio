use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

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
    previous_model: Option<Arc<ProjectModel>>,
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
    let (root, session, workspace_view, previous_model, previous_model_source_revision) = {
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
            workspace.fork_candidate(),
            workspace.project_model.clone(),
            workspace.project_model_source_revision,
        )
    };

    // Materializarea PageJS/runtime și traversarea manifestului sunt muncă
    // potențial voluminoasă. Captura COW a autorității este bounded sub lock;
    // proiecția completă se construiește off-lock, iar publish revalidează
    // exact sesiunea, revizia și AcceptedDisk Arc înainte de commit.
    let projection = workspace_view.capture_projection_snapshot()?;
    require_accepted_disk_matches_live(&root, &session, &projection.accepted_disk)?;

    let result = (
        root,
        session,
        ProjectModelBuildContext {
            projection,
            previous_model,
            previous_model_source_revision,
        },
    );
    Ok(result)
}

/// Builds from the immutable context while retaining the previous canonical
/// SourceNode identities. Even a forced full parse is reconciled against the
/// captured model instead of bootstrapping a parallel identity namespace.
pub(crate) fn build_project_model_from_context(
    root: &Path,
    context: &ProjectModelBuildContext,
) -> Result<Arc<ProjectModel>, String> {
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
    previous: Option<&Arc<ProjectModel>>,
    previous_model_source_revision: Option<u64>,
    projection: &WorkspaceProjectionSnapshot,
) -> Result<Arc<ProjectModel>, String> {
    let Some(previous) = previous else {
        return super::build_project_model_from_workspace_projection(root, projection)
            .map(Arc::new);
    };
    if previous_model_source_revision == Some(projection.revision) {
        return Ok((*previous).clone());
    }
    let changed_paths = changed_paths_since_model(previous.as_ref(), projection);
    let intent = incremental_intent_for_paths(&changed_paths);
    rebuild_project_model_after_workspace_change(
        root,
        Some(previous.as_ref()),
        previous_model_source_revision,
        projection,
        &changed_paths,
        intent,
    )
    .map(|outcome| Arc::new(outcome.model))
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
    model: Arc<ProjectModel>,
) -> Result<(), String> {
    publish_project_model_current(state, context, model)
}

/// Revalidates an immutable analysis context without publishing a model into
/// ProjectWorkspace. Audit uses this after its provider run because a
/// best-effort model containing project defects must never replace the model
/// used by editing commands.
pub(crate) fn validate_project_model_build_context_current(
    state: &AppState,
    context: &ProjectModelBuildContext,
) -> Result<(), String> {
    let (root, session, accepted_disk) = {
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
        validate_context_identity(&current_root, workspace, context)?;
        (
            current_root.as_ref().unwrap().clone(),
            workspace.session.clone(),
            Arc::clone(&workspace.accepted_disk),
        )
    };
    require_accepted_disk_matches_live(&root, &session, &accepted_disk)?;
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut finaliza CAS-ul Audit.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut finaliza CAS-ul ProjectWorkspace după Audit.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "Audit a devenit stale: proiectul a fost închis.".to_string())?;
    validate_context_identity(&current_root, workspace, context)
}

fn publish_project_model_current(
    state: &AppState,
    context: &ProjectModelBuildContext,
    model: Arc<ProjectModel>,
) -> Result<(), String> {
    if context.model_cache_hit() {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut valida root-ul pentru ProjectModel cache hit.".to_string())?;
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut valida ProjectWorkspace pentru ProjectModel cache hit.".to_string()
        })?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectModel cache hit a devenit stale: proiectul a fost închis.".to_string()
        })?;
        validate_context_identity(&current_root, workspace, context)?;
        validate_model_root(&model, &context.projection.project_root)?;
        let captured_model = context.previous_model.as_ref().ok_or_else(|| {
            "ProjectModel cache hit nu mai are modelul canonic capturat.".to_string()
        })?;
        let current_model = workspace.project_model.as_ref().ok_or_else(|| {
            "ProjectModel cache hit a devenit stale: modelul canonic a fost revocat.".to_string()
        })?;
        if workspace.project_model_source_revision != Some(workspace.revision)
            || !Arc::ptr_eq(captured_model, &model)
            || !Arc::ptr_eq(current_model, &model)
        {
            return Err(
                "ProjectModel cache hit a devenit stale: alocarea canonică s-a schimbat."
                    .to_string(),
            );
        }
        return Ok(());
    }

    // Miss-ul păstrează validarea live, dar traversarea filesystem-ului rulează
    // fără mutex-ul ProjectWorkspace. Un al doilea CAS verifică apoi exact
    // sesiunea, revizia și autoritatea Arc înainte de publicare.
    let (root, session, accepted_disk) = {
        let current_root = state
            .current_root
            .lock()
            .map_err(|_| "Nu am putut valida root-ul pentru ProjectModel publish.".to_string())?;
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut valida ProjectWorkspace pentru ProjectModel publish.".to_string()
        })?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectModel publish a devenit stale: proiectul a fost închis.".to_string()
        })?;
        validate_context_identity(&current_root, workspace, context)?;
        (
            current_root.as_ref().unwrap().clone(),
            workspace.session.clone(),
            Arc::clone(&workspace.accepted_disk),
        )
    };
    require_accepted_disk_matches_live(&root, &session, &accepted_disk)?;

    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut finaliza CAS-ul ProjectModel publish.".to_string())?;
    let mut workspace = state.project_workspace.lock().map_err(|_| {
        "Nu am putut finaliza CAS-ul ProjectWorkspace pentru ProjectModel publish.".to_string()
    })?;
    let workspace = workspace.as_mut().ok_or_else(|| {
        "ProjectModel publish a devenit stale: proiectul a fost închis.".to_string()
    })?;
    validate_context_identity(&current_root, workspace, context)?;
    validate_model_root(&model, &context.projection.project_root)?;
    workspace.publish_project_model(&context.projection, model)
}

fn validate_context_identity(
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

    if !Arc::ptr_eq(&workspace.accepted_disk, &context.projection.accepted_disk) {
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
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    use crate::{
        js::PageJsDraftStore,
        kernel::{
            file_buffer_store::{
                hash_text, FileBufferBaseline, FileBufferEntry, FileBufferStore,
                FileBufferStoreLimits, TextBufferLanguage, TextBufferRole,
            },
            project_session::{ProjectRootFingerprint, ProjectSessionScanSummary},
            project_workspace::{
                projection_deep_materializations, reset_projection_deep_materializations,
                ProjectWorkspace,
            },
        },
        project::{
            project_disk_manifest_traversals, read_project_disk_manifest,
            reset_project_disk_manifest_traversals, AcceptedProjectDiskManifest,
        },
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

    #[test]
    fn cache_hit_shares_model_keeps_sources_lazy_and_has_no_publish_io() {
        let (root, state) = benchmark_state("cache-hit", 0, 0);
        reset_project_disk_manifest_traversals();
        let (captured_root, _, miss_context) = capture_project_model_build_context(&state).unwrap();
        let model = build_project_model_from_context(&captured_root, &miss_context).unwrap();
        publish_project_model_if_current(&state, &miss_context, Arc::clone(&model)).unwrap();
        assert_eq!(project_disk_manifest_traversals(), 2);

        reset_project_disk_manifest_traversals();
        reset_projection_deep_materializations();
        let (captured_root, _, hit_context) = capture_project_model_build_context(&state).unwrap();
        assert!(hit_context.model_cache_hit());
        assert!(!hit_context
            .projection()
            .source_texts
            .owned_view_is_materialized());
        let cached = build_project_model_from_context(&captured_root, &hit_context).unwrap();
        assert!(Arc::ptr_eq(&model, &cached));
        publish_project_model_if_current(&state, &hit_context, Arc::clone(&cached)).unwrap();
        assert_eq!(project_disk_manifest_traversals(), 1);
        assert_eq!(projection_deep_materializations(), (0, 0));
        let workspace = state.project_workspace.lock().unwrap();
        assert!(Arc::ptr_eq(
            workspace.as_ref().unwrap().project_model.as_ref().unwrap(),
            &cached
        ));
        drop(workspace);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cache_hit_rejects_a_changed_workspace_without_publish_io() {
        let (root, state) = benchmark_state("cache-hit-stale", 0, 0);
        let (captured_root, _, miss_context) = capture_project_model_build_context(&state).unwrap();
        let model = build_project_model_from_context(&captured_root, &miss_context).unwrap();
        publish_project_model_if_current(&state, &miss_context, Arc::clone(&model)).unwrap();
        let (_, _, hit_context) = capture_project_model_build_context(&state).unwrap();
        state
            .project_workspace
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .revision += 1;
        reset_project_disk_manifest_traversals();
        assert!(publish_project_model_if_current(&state, &hit_context, model).is_err());
        assert_eq!(project_disk_manifest_traversals(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cache_miss_rejects_an_external_disk_change_before_publish() {
        let (root, state) = benchmark_state("cache-miss-external", 0, 0);
        let (captured_root, _, context) = capture_project_model_build_context(&state).unwrap();
        let model = build_project_model_from_context(&captured_root, &context).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = 'https://changed.example.com'\n",
        )
        .unwrap();
        assert!(publish_project_model_if_current(&state, &context, model).is_err());
        assert!(state
            .project_workspace
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .project_model
            .is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "probă manuală pentru traseul ProjectModel cache"]
    fn project_model_cache_pipeline_probe() {
        for (label, document_count, document_bytes) in
            [("small", 4usize, 1_024usize), ("large", 96, 32 * 1_024)]
        {
            let (root, state) = benchmark_state(label, document_count, document_bytes);
            let projection_started = Instant::now();
            let projection = state
                .project_workspace
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .capture_projection_snapshot()
                .unwrap();
            let projection_elapsed = projection_started.elapsed();
            let source_bytes = projection
                .source_texts
                .values()
                .map(String::len)
                .sum::<usize>();
            let accepted_disk_bytes = serde_json::to_vec(&projection.accepted_disk).unwrap().len();
            let accepted_disk_arc_shared = {
                let workspace = state.project_workspace.lock().unwrap();
                Arc::ptr_eq(
                    &projection.accepted_disk,
                    &workspace.as_ref().unwrap().accepted_disk,
                )
            };

            reset_project_disk_manifest_traversals();
            let miss_capture_started = Instant::now();
            let (captured_root, _, miss_context) =
                capture_project_model_build_context(&state).unwrap();
            let miss_capture_elapsed = miss_capture_started.elapsed();
            let miss_build_started = Instant::now();
            let model = build_project_model_from_context(&captured_root, &miss_context).unwrap();
            let miss_build_elapsed = miss_build_started.elapsed();
            let miss_publish_started = Instant::now();
            publish_project_model_if_current(&state, &miss_context, Arc::clone(&model)).unwrap();
            let miss_publish_elapsed = miss_publish_started.elapsed();
            let miss_manifest_traversals = project_disk_manifest_traversals();

            reset_project_disk_manifest_traversals();
            reset_projection_deep_materializations();
            let hit_capture_started = Instant::now();
            let (captured_root, _, hit_context) =
                capture_project_model_build_context(&state).unwrap();
            let hit_capture_elapsed = hit_capture_started.elapsed();
            let hit_sources_materialized = hit_context
                .projection()
                .source_texts
                .owned_view_is_materialized();
            let hit_build_started = Instant::now();
            let cached_model =
                build_project_model_from_context(&captured_root, &hit_context).unwrap();
            let hit_build_elapsed = hit_build_started.elapsed();
            let hit_publish_started = Instant::now();
            publish_project_model_if_current(&state, &hit_context, Arc::clone(&cached_model))
                .unwrap();
            let hit_publish_elapsed = hit_publish_started.elapsed();
            let hit_manifest_traversals = project_disk_manifest_traversals();
            let hit_deep_materializations = projection_deep_materializations();

            eprintln!(
                "PROJECT_MODEL_PIPELINE label={label} documents={} source_bytes={source_bytes} accepted_disk_bytes={accepted_disk_bytes} accepted_disk_arc_shared={accepted_disk_arc_shared} projection_us={} miss_capture_us={} miss_build_us={} miss_publish_us={} miss_manifest_traversals={miss_manifest_traversals} hit_capture_us={} hit_build_us={} hit_publish_us={} hit_manifest_traversals={hit_manifest_traversals} hit_deep_materializations={hit_deep_materializations:?} hit_sources_materialized={hit_sources_materialized} model_cache_ptr_shared={}",
                projection.source_texts.len(),
                projection_elapsed.as_micros(),
                miss_capture_elapsed.as_micros(),
                miss_build_elapsed.as_micros(),
                miss_publish_elapsed.as_micros(),
                hit_capture_elapsed.as_micros(),
                hit_build_elapsed.as_micros(),
                hit_publish_elapsed.as_micros(),
                Arc::ptr_eq(&model, &cached_model),
            );

            assert!(Arc::ptr_eq(&model, &cached_model));
            assert_eq!(miss_manifest_traversals, 2);
            assert_eq!(hit_manifest_traversals, 1);
            assert_eq!(hit_deep_materializations, (0, 0));
            assert!(!hit_sources_materialized);
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn benchmark_state(
        label: &str,
        document_count: usize,
        document_bytes: usize,
    ) -> (PathBuf, AppState) {
        let root = unique_test_dir().join(label);
        fs::create_dir_all(root.join("templates")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(root.join("zola.toml"), "base_url = 'https://example.com'\n").unwrap();
        fs::write(
            root.join("templates/index.html"),
            "<main>{% block content %}{% endblock content %}</main>\n",
        )
        .unwrap();
        let body = "x".repeat(document_bytes);
        for index in 0..document_count {
            fs::write(
                root.join(format!("content/page-{index}.md")),
                format!("+++\ntitle = 'Page {index}'\n+++\n\n{body}\n"),
            )
            .unwrap();
        }
        let root = root.canonicalize().unwrap();
        let session = session(&root);
        let mut documents = FileBufferStore::for_project_session(
            &session,
            1,
            FileBufferStoreLimits {
                max_files: 200,
                max_file_bytes: 2 * 1_024 * 1_024,
                max_total_bytes: 8 * 1_024 * 1_024,
            },
        );
        insert_baseline(&mut documents, &root, "zola.toml", TextBufferLanguage::Toml);
        insert_baseline(
            &mut documents,
            &root,
            "templates/index.html",
            TextBufferLanguage::Html,
        );
        for index in 0..document_count {
            insert_baseline(
                &mut documents,
                &root,
                &format!("content/page-{index}.md"),
                TextBufferLanguage::Markdown,
            );
        }
        let accepted = AcceptedProjectDiskManifest::new(
            session.runtime_instance_id(),
            session.project_root.clone(),
            read_project_disk_manifest(&root).unwrap(),
        )
        .unwrap();
        let page_js = PageJsDraftStore::new(&session);
        let workspace = ProjectWorkspace::new(session, accepted, documents, page_js).unwrap();
        let state = AppState::default();
        *state.current_root.lock().unwrap() = Some(root.clone());
        *state.project_workspace.lock().unwrap() = Some(workspace);
        (root, state)
    }

    fn insert_baseline(
        store: &mut FileBufferStore,
        root: &Path,
        relative_path: &str,
        language: TextBufferLanguage,
    ) {
        let path = root.join(relative_path);
        let text = fs::read_to_string(&path).unwrap();
        store.insert_loaded_file(FileBufferEntry {
            relative_path: relative_path.to_string(),
            absolute_path: path.to_string_lossy().into_owned(),
            language,
            role: TextBufferRole::Other,
            baseline: FileBufferBaseline {
                hash: hash_text(&text),
                modified_ms: 1,
                size: text.len() as u64,
                readonly: false,
            },
            baseline_text: text.into(),
            draft: None,
            revision: 1,
        });
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
