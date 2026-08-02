use std::{collections::BTreeSet, path::Path, time::Instant};

use serde::Serialize;

use crate::{
    kernel::project_workspace::WorkspaceProjectionSnapshot,
    project::zola_project_root,
    project_model::{
        files::{model_revision, project_model_file},
        model::{ProjectModel, ProjectModelFileKind},
        tera_graph::build_tera_graph,
    },
    source_graph::{
        rebuild_local_template_graph, SourceGraphIncrementalFallback,
        SourceGraphIncrementalTemplateReport,
    },
};

pub(crate) const PROJECT_MODEL_INCREMENTAL_REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProjectModelRebuildMode {
    Incremental,
    FullFallback,
}

impl ProjectModelRebuildMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::FullFallback => "fullFallback",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectModelIncrementalIntent {
    HtmlStructural,
    StyleDeclaration,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectModelIncrementalFallback {
    OperationNotEligible,
    MissingPreviousModel,
    PreviousRevisionUnavailable,
    NonAdjacentWorkspaceRevision,
    MissingTransactionIdentity,
    EmptyChangeSet,
    MultipleChangedFiles,
    UnsafePath,
    UnsupportedFileKind,
    CreatedOrDeletedSource,
    RenamedSource,
    AmbiguousPreviousFile,
    SourceGraph(SourceGraphIncrementalFallback),
}

impl ProjectModelIncrementalFallback {
    fn code(&self) -> String {
        match self {
            Self::OperationNotEligible => "operation_not_eligible".to_string(),
            Self::MissingPreviousModel => "missing_previous_model".to_string(),
            Self::PreviousRevisionUnavailable => "previous_revision_unavailable".to_string(),
            Self::NonAdjacentWorkspaceRevision => "non_adjacent_workspace_revision".to_string(),
            Self::MissingTransactionIdentity => "missing_transaction_identity".to_string(),
            Self::EmptyChangeSet => "empty_change_set".to_string(),
            Self::MultipleChangedFiles => "multiple_changed_files".to_string(),
            Self::UnsafePath => "unsafe_changed_path".to_string(),
            Self::UnsupportedFileKind => "unsupported_file_kind".to_string(),
            Self::CreatedOrDeletedSource => "created_or_deleted_source".to_string(),
            Self::RenamedSource => "renamed_source".to_string(),
            Self::AmbiguousPreviousFile => "ambiguous_previous_file".to_string(),
            Self::SourceGraph(reason) => reason.code().to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectModelIncrementalBuildReport {
    pub(crate) schema_version: u32,
    pub(crate) mode: ProjectModelRebuildMode,
    pub(crate) workspace_revision: u64,
    pub(crate) workspace_transaction_id: Option<String>,
    pub(crate) changed_paths: Vec<String>,
    pub(crate) invalidated_template_files: Vec<String>,
    pub(crate) invalidated_page_files: Vec<String>,
    pub(crate) replaced_nodes: usize,
    pub(crate) reused_nodes: usize,
    pub(crate) reused_relations: usize,
    pub(crate) model_clone_ms: u64,
    pub(crate) template_parse_ms: u64,
    pub(crate) component_graph_ms: u64,
    pub(crate) block_graph_ms: u64,
    pub(crate) tera_graph_ms: u64,
    pub(crate) duration_ms: u64,
    pub(crate) fallback_reason: Option<String>,
}

pub(crate) struct ProjectModelIncrementalBuildOutcome {
    pub(crate) model: ProjectModel,
    pub(crate) report: ProjectModelIncrementalBuildReport,
}

pub(crate) fn rebuild_project_model_after_workspace_change(
    project_root: &Path,
    previous_model: Option<&ProjectModel>,
    previous_workspace_revision: Option<u64>,
    projection: &WorkspaceProjectionSnapshot,
    exact_changed_paths: &[String],
    intent: ProjectModelIncrementalIntent,
) -> Result<ProjectModelIncrementalBuildOutcome, String> {
    let started = Instant::now();
    match try_incremental_build(
        project_root,
        previous_model,
        previous_workspace_revision,
        projection,
        exact_changed_paths,
        intent,
    ) {
        Ok((model, graph_report, changed_paths, model_clone_ms, tera_graph_ms)) => {
            Ok(ProjectModelIncrementalBuildOutcome {
                model,
                report: report(
                    ProjectModelRebuildMode::Incremental,
                    projection,
                    changed_paths,
                    Some(graph_report),
                    model_clone_ms,
                    tera_graph_ms,
                    None,
                    elapsed_ms(started),
                ),
            })
        }
        Err(reason) => {
            let model =
                super::build_project_model_from_workspace_projection(project_root, projection)?;
            Ok(ProjectModelIncrementalBuildOutcome {
                model,
                report: report(
                    ProjectModelRebuildMode::FullFallback,
                    projection,
                    normalized_changed_paths(exact_changed_paths).unwrap_or_default(),
                    None,
                    0,
                    0,
                    Some(reason.code()),
                    elapsed_ms(started),
                ),
            })
        }
    }
}

fn try_incremental_build(
    project_root: &Path,
    previous_model: Option<&ProjectModel>,
    previous_workspace_revision: Option<u64>,
    projection: &WorkspaceProjectionSnapshot,
    exact_changed_paths: &[String],
    intent: ProjectModelIncrementalIntent,
) -> Result<
    (
        ProjectModel,
        SourceGraphIncrementalTemplateReport,
        Vec<String>,
        u64,
        u64,
    ),
    ProjectModelIncrementalFallback,
> {
    if intent == ProjectModelIncrementalIntent::Unsupported {
        return Err(ProjectModelIncrementalFallback::OperationNotEligible);
    }
    let previous = previous_model.ok_or(ProjectModelIncrementalFallback::MissingPreviousModel)?;
    let previous_revision = previous_workspace_revision
        .ok_or(ProjectModelIncrementalFallback::PreviousRevisionUnavailable)?;
    if previous_revision.checked_add(1) != Some(projection.revision) {
        return Err(ProjectModelIncrementalFallback::NonAdjacentWorkspaceRevision);
    }
    if projection
        .workspace_transaction_id
        .as_deref()
        .is_none_or(str::is_empty)
    {
        return Err(ProjectModelIncrementalFallback::MissingTransactionIdentity);
    }
    let changed_paths = normalized_changed_paths(exact_changed_paths)?;
    if looks_like_rename(previous, projection, &changed_paths) {
        return Err(ProjectModelIncrementalFallback::RenamedSource);
    }
    let root = project_root
        .canonicalize()
        .map_err(|_| ProjectModelIncrementalFallback::UnsafePath)?;
    if root != Path::new(&projection.project_root)
        || root != previous.project_root
        || zola_project_root(&root) != previous.zola_root
    {
        return Err(ProjectModelIncrementalFallback::UnsafePath);
    }
    if intent == ProjectModelIncrementalIntent::StyleDeclaration {
        return try_incremental_style_build(previous, projection, changed_paths);
    }
    let [relative_path] = changed_paths.as_slice() else {
        return Err(if changed_paths.is_empty() {
            ProjectModelIncrementalFallback::EmptyChangeSet
        } else {
            ProjectModelIncrementalFallback::MultipleChangedFiles
        });
    };
    if !relative_path.starts_with("templates/") || !relative_path.ends_with(".html") {
        return Err(ProjectModelIncrementalFallback::UnsupportedFileKind);
    }
    if projection.deleted_sources.contains(relative_path)
        || !projection.source_texts.contains_key(relative_path)
    {
        return Err(ProjectModelIncrementalFallback::CreatedOrDeletedSource);
    }
    let previous_files = previous
        .files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.relative_path == *relative_path)
        .collect::<Vec<_>>();
    let [(file_index, previous_file)] = previous_files.as_slice() else {
        return Err(if previous_files.is_empty() {
            ProjectModelIncrementalFallback::CreatedOrDeletedSource
        } else {
            ProjectModelIncrementalFallback::AmbiguousPreviousFile
        });
    };
    if previous_file.kind != ProjectModelFileKind::Template {
        return Err(ProjectModelIncrementalFallback::UnsupportedFileKind);
    }
    let model_clone_started = Instant::now();
    let mut next = previous.clone();
    let model_clone_ms = elapsed_ms(model_clone_started);
    let contents = projection
        .source_texts
        .get(relative_path)
        .expect("validated projected source")
        .clone();
    next.files[*file_index] = project_model_file(
        relative_path.clone(),
        contents,
        projection.changed_paths.contains(relative_path),
    );
    let (source_graph, graph_report) = rebuild_local_template_graph(
        next.source_graph,
        &root,
        &next.zola_root,
        relative_path,
        &projection.source_texts,
    )
    .map_err(ProjectModelIncrementalFallback::SourceGraph)?;
    next.source_graph = source_graph;
    let tera_graph_started = Instant::now();
    next.tera_graph = build_tera_graph(&next.source_graph, &next.files);
    let tera_graph_ms = elapsed_ms(tera_graph_started);
    next.revision = model_revision(&next.files);
    Ok((
        next,
        graph_report,
        changed_paths,
        model_clone_ms,
        tera_graph_ms,
    ))
}

fn try_incremental_style_build(
    previous: &ProjectModel,
    projection: &WorkspaceProjectionSnapshot,
    changed_paths: Vec<String>,
) -> Result<
    (
        ProjectModel,
        SourceGraphIncrementalTemplateReport,
        Vec<String>,
        u64,
        u64,
    ),
    ProjectModelIncrementalFallback,
> {
    if changed_paths.is_empty() {
        return Err(ProjectModelIncrementalFallback::EmptyChangeSet);
    }

    let mut replacements = Vec::with_capacity(changed_paths.len());
    for relative_path in &changed_paths {
        if projection.deleted_sources.contains(relative_path)
            || !projection.source_texts.contains_key(relative_path)
        {
            return Err(ProjectModelIncrementalFallback::CreatedOrDeletedSource);
        }
        let matching_files = previous
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| file.relative_path == *relative_path)
            .collect::<Vec<_>>();
        let [(file_index, previous_file)] = matching_files.as_slice() else {
            return Err(if matching_files.is_empty() {
                ProjectModelIncrementalFallback::CreatedOrDeletedSource
            } else {
                ProjectModelIncrementalFallback::AmbiguousPreviousFile
            });
        };
        if previous_file.kind != ProjectModelFileKind::Style {
            return Err(ProjectModelIncrementalFallback::UnsupportedFileKind);
        }
        replacements.push((*file_index, relative_path));
    }

    let model_clone_started = Instant::now();
    let mut next = previous.clone();
    let model_clone_ms = elapsed_ms(model_clone_started);
    for (file_index, relative_path) in replacements {
        let contents = projection
            .source_texts
            .get(relative_path)
            .expect("validated projected style source")
            .clone();
        next.files[file_index] = project_model_file(
            relative_path.clone(),
            contents,
            projection.changed_paths.contains(relative_path),
        );
    }
    next.revision = model_revision(&next.files);

    let graph_report = SourceGraphIncrementalTemplateReport {
        invalidated_template_files: Vec::new(),
        invalidated_page_files: Vec::new(),
        replaced_nodes: 0,
        reused_nodes: next.source_graph.nodes.len(),
        reused_relations: next.source_graph.relations.len(),
        template_parse_ms: 0,
        component_graph_ms: 0,
        block_graph_ms: 0,
    };
    Ok((next, graph_report, changed_paths, model_clone_ms, 0))
}

fn looks_like_rename(
    previous: &ProjectModel,
    projection: &WorkspaceProjectionSnapshot,
    changed_paths: &[String],
) -> bool {
    if changed_paths.len() != 2 {
        return false;
    }
    let removed = changed_paths.iter().filter(|path| {
        previous
            .files
            .iter()
            .any(|file| file.relative_path == ***path)
            && (projection.deleted_sources.contains(*path)
                || !projection.source_texts.contains_key(*path))
    });
    let created = changed_paths.iter().filter(|path| {
        !previous
            .files
            .iter()
            .any(|file| file.relative_path == ***path)
            && projection.source_texts.contains_key(*path)
            && !projection.deleted_sources.contains(*path)
    });
    removed.count() == 1 && created.count() == 1
}

fn normalized_changed_paths(
    paths: &[String],
) -> Result<Vec<String>, ProjectModelIncrementalFallback> {
    let mut normalized = BTreeSet::new();
    for path in paths {
        let value = path.trim().replace('\\', "/");
        let relative = Path::new(&value);
        if value.is_empty()
            || relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(ProjectModelIncrementalFallback::UnsafePath);
        }
        normalized.insert(value);
    }
    Ok(normalized.into_iter().collect())
}

fn report(
    mode: ProjectModelRebuildMode,
    projection: &WorkspaceProjectionSnapshot,
    changed_paths: Vec<String>,
    graph: Option<SourceGraphIncrementalTemplateReport>,
    model_clone_ms: u64,
    tera_graph_ms: u64,
    fallback_reason: Option<String>,
    duration_ms: u64,
) -> ProjectModelIncrementalBuildReport {
    let graph = graph.unwrap_or(SourceGraphIncrementalTemplateReport {
        invalidated_template_files: Vec::new(),
        invalidated_page_files: Vec::new(),
        replaced_nodes: 0,
        reused_nodes: 0,
        reused_relations: 0,
        template_parse_ms: 0,
        component_graph_ms: 0,
        block_graph_ms: 0,
    });
    ProjectModelIncrementalBuildReport {
        schema_version: PROJECT_MODEL_INCREMENTAL_REPORT_SCHEMA_VERSION,
        mode,
        workspace_revision: projection.revision,
        workspace_transaction_id: projection.workspace_transaction_id.clone(),
        changed_paths,
        invalidated_template_files: graph.invalidated_template_files,
        invalidated_page_files: graph.invalidated_page_files,
        replaced_nodes: graph.replaced_nodes,
        reused_nodes: graph.reused_nodes,
        reused_relations: graph.reused_relations,
        model_clone_ms,
        template_parse_ms: graph.template_parse_ms,
        component_graph_ms: graph.component_graph_ms,
        block_graph_ms: graph.block_graph_ms,
        tera_graph_ms,
        duration_ms,
        fallback_reason,
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        kernel::project_workspace::WorkspaceProjectionSnapshot,
        project::{AcceptedProjectDiskManifest, ProjectDiskManifest},
        project_model::build_project_model_from_workspace_projection,
        source_graph::model::{ComponentInvocationKind, SourceRelationKind},
    };

    use super::*;

    #[test]
    fn single_template_html_change_matches_the_full_builder_exactly() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_projection = projection(&root, 7, None, initial_sources(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let mut after_sources = initial_sources();
        after_sources.insert("templates/index.html".to_string(), moved_index_source());
        let after_projection = projection(
            &root,
            8,
            Some("incremental-8"),
            after_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );

        let outcome = rebuild_project_model_after_workspace_change(
            &root,
            Some(&before),
            Some(7),
            &after_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        let oracle =
            build_project_model_from_workspace_projection(&root, &after_projection).unwrap();

        assert_eq!(outcome.report.mode, ProjectModelRebuildMode::Incremental);
        assert_eq!(outcome.report.fallback_reason, None);
        assert!(outcome.report.reused_nodes > 0);
        assert!(outcome
            .report
            .invalidated_template_files
            .contains(&"templates/index.html".to_string()));
        assert_eq!(
            serde_json::to_value(outcome.model.snapshot()).unwrap(),
            serde_json::to_value(oracle.snapshot()).unwrap(),
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_style_change_reuses_the_semantic_graph_and_matches_the_full_builder() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_projection = projection(&root, 11, None, initial_sources(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let mut after_sources = initial_sources();
        after_sources.insert(
            "sass/pagini/index.scss".to_string(),
            ".lead { background-image: linear-gradient(red, blue); }\n".to_string(),
        );
        let after_projection = projection(
            &root,
            12,
            Some("style-12"),
            after_sources,
            HashSet::from(["sass/pagini/index.scss".to_string()]),
        );

        let outcome = rebuild_project_model_after_workspace_change(
            &root,
            Some(&before),
            Some(11),
            &after_projection,
            &["sass/pagini/index.scss".to_string()],
            ProjectModelIncrementalIntent::StyleDeclaration,
        )
        .unwrap();
        let oracle =
            build_project_model_from_workspace_projection(&root, &after_projection).unwrap();

        assert_eq!(outcome.report.mode, ProjectModelRebuildMode::Incremental);
        assert_eq!(outcome.report.fallback_reason, None);
        assert_eq!(outcome.report.replaced_nodes, 0);
        assert_eq!(outcome.report.template_parse_ms, 0);
        assert_eq!(
            serde_json::to_value(outcome.model.snapshot()).unwrap(),
            serde_json::to_value(oracle.snapshot()).unwrap(),
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_new_stylesheet_uses_the_full_builder_instead_of_reusing_stale_topology() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_projection = projection(&root, 21, None, initial_sources(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let mut after_sources = initial_sources();
        after_sources.insert(
            "sass/pagini/nou.scss".to_string(),
            ".nou { color: green; }\n".to_string(),
        );
        let after_projection = projection(
            &root,
            22,
            Some("style-create-22"),
            after_sources,
            HashSet::from(["sass/pagini/nou.scss".to_string()]),
        );

        let outcome = rebuild_project_model_after_workspace_change(
            &root,
            Some(&before),
            Some(21),
            &after_projection,
            &["sass/pagini/nou.scss".to_string()],
            ProjectModelIncrementalIntent::StyleDeclaration,
        )
        .unwrap();
        let oracle =
            build_project_model_from_workspace_projection(&root, &after_projection).unwrap();

        assert_eq!(outcome.report.mode, ProjectModelRebuildMode::FullFallback);
        assert_eq!(
            outcome.report.fallback_reason.as_deref(),
            Some("created_or_deleted_source")
        );
        assert_eq!(
            serde_json::to_value(outcome.model.snapshot()).unwrap(),
            serde_json::to_value(oracle.snapshot()).unwrap(),
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dependency_change_and_dynamic_load_data_fall_back_to_the_full_oracle() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_projection = projection(&root, 3, None, initial_sources(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();

        for (revision, replacement, expected_reason) in [
            (
                4,
                "{% extends \"base.html\" %}{% block content %}{% include \"partials/other.html\" %}{% endblock %}",
                "source_graph_dependency_contract_changed",
            ),
            (
                4,
                "{% extends \"base.html\" %}{% block content %}{% set data = load_data(path=data_path) %}<h1>Titlu</h1>{% endblock %}",
                "source_graph_dynamic_dependency",
            ),
        ] {
            let mut sources = initial_sources();
            sources.insert("templates/index.html".to_string(), replacement.to_string());
            if replacement.contains("partials/other.html") {
                sources.insert(
                    "templates/partials/other.html".to_string(),
                    "<footer>Altul</footer>".to_string(),
                );
            }
            let after_projection = projection(
                &root,
                revision,
                Some("fallback-4"),
                sources,
                HashSet::from(["templates/index.html".to_string()]),
            );
            let outcome = rebuild_project_model_after_workspace_change(
                &root,
                Some(&before),
                Some(3),
                &after_projection,
                &["templates/index.html".to_string()],
                ProjectModelIncrementalIntent::HtmlStructural,
            )
            .unwrap();
            let oracle = build_project_model_from_workspace_projection(&root, &after_projection)
                .unwrap();
            assert_eq!(outcome.report.mode, ProjectModelRebuildMode::FullFallback);
            assert_eq!(outcome.report.fallback_reason.as_deref(), Some(expected_reason));
            assert_eq!(
                serde_json::to_value(outcome.model.snapshot()).unwrap(),
                serde_json::to_value(oracle.snapshot()).unwrap(),
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn forward_undo_redo_sequence_uses_the_same_incremental_path() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let initial = initial_sources();
        let first_projection = projection(&root, 10, None, initial.clone(), HashSet::new());
        let first =
            build_project_model_from_workspace_projection(&root, &first_projection).unwrap();
        let mut moved_sources = initial.clone();
        moved_sources.insert("templates/index.html".to_string(), moved_index_source());
        let moved_projection = projection(
            &root,
            11,
            Some("forward-11"),
            moved_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );
        let moved = rebuild_project_model_after_workspace_change(
            &root,
            Some(&first),
            Some(10),
            &moved_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        assert_eq!(moved.report.mode, ProjectModelRebuildMode::Incremental);

        let restored_projection = projection(&root, 12, Some("undo-12"), initial, HashSet::new());
        let restored = rebuild_project_model_after_workspace_change(
            &root,
            Some(&moved.model),
            Some(11),
            &restored_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        let oracle =
            build_project_model_from_workspace_projection(&root, &restored_projection).unwrap();
        assert_eq!(restored.report.mode, ProjectModelRebuildMode::Incremental);
        assert_eq!(
            serde_json::to_value(restored.model.snapshot()).unwrap(),
            serde_json::to_value(oracle.snapshot()).unwrap(),
        );

        let mut redone_sources = initial_sources();
        redone_sources.insert("templates/index.html".to_string(), moved_index_source());
        let redone_projection = projection(
            &root,
            13,
            Some("redo-13"),
            redone_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );
        let redone = rebuild_project_model_after_workspace_change(
            &root,
            Some(&restored.model),
            Some(12),
            &redone_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        let oracle =
            build_project_model_from_workspace_projection(&root, &redone_projection).unwrap();
        assert_eq!(redone.report.mode, ProjectModelRebuildMode::Incremental);
        assert_eq!(
            serde_json::to_value(redone.model.snapshot()).unwrap(),
            serde_json::to_value(oracle.snapshot()).unwrap(),
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn active_theme_local_override_is_incremental_and_theme_edits_fall_back() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let mut initial = initial_sources();
        initial.insert(
            "zola.toml".to_string(),
            "base_url = '/'\ntheme = 'demo'\n".to_string(),
        );
        initial.insert(
            "themes/demo/templates/base.html".to_string(),
            "<body class=\"theme\">{% block content %}{% endblock %}</body>".to_string(),
        );
        let before_projection = projection(&root, 14, None, initial.clone(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();

        let mut local_override_sources = initial.clone();
        local_override_sources.insert(
            "templates/base.html".to_string(),
            "<body class=\"local changed\">{% block content %}{% endblock %}</body>".to_string(),
        );
        let local_override_projection = projection(
            &root,
            15,
            Some("theme-local-override-15"),
            local_override_sources,
            HashSet::from(["templates/base.html".to_string()]),
        );
        let local_override = rebuild_project_model_after_workspace_change(
            &root,
            Some(&before),
            Some(14),
            &local_override_projection,
            &["templates/base.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        let oracle =
            build_project_model_from_workspace_projection(&root, &local_override_projection)
                .unwrap();
        assert_eq!(
            local_override.report.mode,
            ProjectModelRebuildMode::Incremental,
        );
        assert!(local_override
            .report
            .invalidated_template_files
            .contains(&"templates/index.html".to_string()));
        assert_eq!(
            serde_json::to_value(local_override.model.snapshot()).unwrap(),
            serde_json::to_value(oracle.snapshot()).unwrap(),
        );

        let mut theme_sources = initial;
        theme_sources.insert(
            "themes/demo/templates/base.html".to_string(),
            "<body class=\"theme changed\">{% block content %}{% endblock %}</body>".to_string(),
        );
        assert_fallback_matches_oracle(
            &root,
            &before,
            14,
            projection(
                &root,
                15,
                Some("theme-source-15"),
                theme_sources,
                HashSet::from(["themes/demo/templates/base.html".to_string()]),
            ),
            &["themes/demo/templates/base.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            "unsupported_file_kind",
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn html_operation_matrix_matches_the_full_model_and_preserves_dependencies() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_projection = projection(&root, 20, None, initial_sources(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        for kind in [
            SourceRelationKind::PageTemplate,
            SourceRelationKind::SectionPageTemplate,
            SourceRelationKind::Extends,
            SourceRelationKind::Includes,
            SourceRelationKind::Imports,
            SourceRelationKind::DefinesBlock,
            SourceRelationKind::OverridesBlock,
            SourceRelationKind::DataFileLoad,
            SourceRelationKind::AssetUrl,
            SourceRelationKind::UsesStyle,
            SourceRelationKind::UsesScript,
        ] {
            assert!(before
                .source_graph
                .relations
                .iter()
                .any(|relation| relation.kind == kind));
        }
        assert!(before.source_graph.templates.iter().any(|template| {
            template.file == "templates/macros/ui.html"
                && template.macros.contains(&"badge".to_string())
        }));
        assert!(before
            .source_graph
            .component_graph
            .invocations
            .iter()
            .any(|invocation| {
                invocation.file == "templates/index.html"
                    && invocation.kind == ComponentInvocationKind::MacroCall
            }));

        let initial = initial_index_source();
        let variants = [
            ("move", moved_index_source()),
            (
                "insert",
                initial.replace("<h1>Titlu</h1>", "<h1>Titlu</h1><span>Nou</span>"),
            ),
            ("delete", initial.replace("<p class=\"lead\">Text</p>", "")),
            ("text", initial.replace(">Text</p>", ">Text nou</p>")),
            (
                "attributes",
                initial.replace("<h1>", "<h1 data-level=\"primary\">"),
            ),
            ("tag", initial.replace("<h1>Titlu</h1>", "<h2>Titlu</h2>")),
        ];
        for (index, (operation, source)) in variants.into_iter().enumerate() {
            let mut sources = initial_sources();
            sources.insert("templates/index.html".to_string(), source);
            let after_projection = projection(
                &root,
                21,
                Some(&format!("{operation}-21")),
                sources,
                HashSet::from(["templates/index.html".to_string()]),
            );
            let outcome = rebuild_project_model_after_workspace_change(
                &root,
                Some(&before),
                Some(20),
                &after_projection,
                &["templates/index.html".to_string()],
                ProjectModelIncrementalIntent::HtmlStructural,
            )
            .unwrap();
            let oracle =
                build_project_model_from_workspace_projection(&root, &after_projection).unwrap();
            assert_eq!(
                outcome.report.mode,
                ProjectModelRebuildMode::Incremental,
                "{operation} #{index}",
            );
            assert!(outcome
                .report
                .invalidated_page_files
                .contains(&"content/_index.md".to_string()));
            assert_eq!(
                serde_json::to_value(outcome.model.snapshot()).unwrap(),
                serde_json::to_value(oracle.snapshot()).unwrap(),
                "{operation} #{index}",
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsupported_topology_and_stale_inputs_use_explicit_full_fallbacks() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let initial = initial_sources();
        let before_projection = projection(&root, 30, None, initial.clone(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();

        let mut config_sources = initial.clone();
        config_sources.insert("zola.toml".to_string(), "base_url = '/nou'\n".to_string());
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            projection(
                &root,
                31,
                Some("config-31"),
                config_sources,
                HashSet::from(["zola.toml".to_string()]),
            ),
            &["zola.toml".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            "unsupported_file_kind",
        );

        let mut taxonomy_sources = initial.clone();
        taxonomy_sources.insert(
            "content/blog/post.md".to_string(),
            "+++\ntitle = 'Postare'\n[taxonomies]\ntaguri = ['rust']\n+++\nText\n".to_string(),
        );
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            projection(
                &root,
                31,
                Some("taxonomy-31"),
                taxonomy_sources,
                HashSet::from(["content/blog/post.md".to_string()]),
            ),
            &["content/blog/post.md".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            "unsupported_file_kind",
        );

        let mut multiple_sources = initial.clone();
        multiple_sources.insert("templates/index.html".to_string(), moved_index_source());
        multiple_sources.insert(
            "sass/pagini/index.scss".to_string(),
            ".lead { color: blue; }\n".to_string(),
        );
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            projection(
                &root,
                31,
                Some("multiple-31"),
                multiple_sources,
                HashSet::from([
                    "templates/index.html".to_string(),
                    "sass/pagini/index.scss".to_string(),
                ]),
            ),
            &[
                "templates/index.html".to_string(),
                "sass/pagini/index.scss".to_string(),
            ],
            ProjectModelIncrementalIntent::HtmlStructural,
            "multiple_changed_files",
        );

        let mut created_sources = initial.clone();
        created_sources.insert(
            "templates/new.html".to_string(),
            "<main>Nou</main>".to_string(),
        );
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            projection(
                &root,
                31,
                Some("create-31"),
                created_sources,
                HashSet::from(["templates/new.html".to_string()]),
            ),
            &["templates/new.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            "created_or_deleted_source",
        );

        let mut renamed_sources = initial.clone();
        let renamed_contents = renamed_sources.remove("templates/index.html").unwrap();
        renamed_sources.insert("templates/home.html".to_string(), renamed_contents);
        let mut renamed_projection = projection(
            &root,
            31,
            Some("rename-31"),
            renamed_sources,
            HashSet::from([
                "templates/index.html".to_string(),
                "templates/home.html".to_string(),
            ]),
        );
        renamed_projection
            .deleted_sources
            .insert("templates/index.html".to_string());
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            renamed_projection,
            &[
                "templates/index.html".to_string(),
                "templates/home.html".to_string(),
            ],
            ProjectModelIncrementalIntent::HtmlStructural,
            "renamed_source",
        );

        let mut deleted_sources = initial.clone();
        deleted_sources.remove("templates/index.html");
        let mut deleted_projection = projection(
            &root,
            31,
            Some("delete-31"),
            deleted_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );
        deleted_projection
            .deleted_sources
            .insert("templates/index.html".to_string());
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            deleted_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            "created_or_deleted_source",
        );

        let mut stale_sources = initial.clone();
        stale_sources.insert("templates/index.html".to_string(), moved_index_source());
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            projection(
                &root,
                99,
                Some("stale-99"),
                stale_sources.clone(),
                HashSet::from(["templates/index.html".to_string()]),
            ),
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            "non_adjacent_workspace_revision",
        );
        assert_fallback_matches_oracle(
            &root,
            &before,
            30,
            projection(
                &root,
                31,
                Some("unsupported-31"),
                stale_sources,
                HashSet::from(["templates/index.html".to_string()]),
            ),
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::Unsupported,
            "operation_not_eligible",
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_incremental_candidate_does_not_modify_the_previous_model() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let initial = initial_sources();
        let before_projection = projection(&root, 40, None, initial.clone(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let before_snapshot = serde_json::to_value(before.snapshot()).unwrap();
        let mut invalid_sources = initial;
        invalid_sources.insert(
            "templates/index.html".to_string(),
            "{% if broken %}<main>{% endblock %}".to_string(),
        );
        let invalid_projection = projection(
            &root,
            41,
            Some("invalid-41"),
            invalid_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );
        assert!(rebuild_project_model_after_workspace_change(
            &root,
            Some(&before),
            Some(40),
            &invalid_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .is_err());
        assert_eq!(
            serde_json::to_value(before.snapshot()).unwrap(),
            before_snapshot
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires PANA_INCREMENTAL_BENCH_PROJECT"]
    fn real_project_single_template_warm_p95_stays_within_budget() {
        let root = PathBuf::from(
            std::env::var("PANA_INCREMENTAL_BENCH_PROJECT")
                .expect("PANA_INCREMENTAL_BENCH_PROJECT"),
        )
        .canonicalize()
        .unwrap();
        let before = crate::project_model::build_project_model(&root, &HashMap::new()).unwrap();
        let target = before
            .files
            .iter()
            .find(|file| file.relative_path == "templates/index.html")
            .expect("templates/index.html");
        let mut changed = target.contents.clone();
        changed.push_str("\n<!-- pana incremental benchmark -->\n");
        let mut source_texts = before
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.contents.clone()))
            .collect::<HashMap<_, _>>();
        source_texts.insert(target.relative_path.clone(), changed);
        let manifest = crate::project::read_project_disk_manifest(&root).unwrap();
        let runtime_session_id = "incremental-real-benchmark".to_string();
        let projection = WorkspaceProjectionSnapshot {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: runtime_session_id.clone(),
            revision: 2,
            workspace_transaction_id: Some("incremental-real-benchmark-2".to_string()),
            source_texts,
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::from([target.relative_path.clone()]),
            accepted_disk: AcceptedProjectDiskManifest::new(
                runtime_session_id,
                root.to_string_lossy().to_string(),
                manifest,
            )
            .unwrap(),
        };
        let expected = build_project_model_from_workspace_projection(&root, &projection).unwrap();
        let expected_snapshot = serde_json::to_value(expected.snapshot()).unwrap();
        let mut full_samples = Vec::new();
        for _ in 0..10 {
            let full_started = Instant::now();
            let oracle = build_project_model_from_workspace_projection(&root, &projection).unwrap();
            full_samples.push(elapsed_ms(full_started));
            assert_eq!(
                serde_json::to_value(oracle.snapshot()).unwrap(),
                expected_snapshot,
            );
        }
        let mut samples = Vec::new();
        let mut last_report = None;
        for _ in 0..25 {
            let outcome = rebuild_project_model_after_workspace_change(
                &root,
                Some(&before),
                Some(1),
                &projection,
                &[target.relative_path.clone()],
                ProjectModelIncrementalIntent::HtmlStructural,
            )
            .unwrap();
            assert_eq!(outcome.report.mode, ProjectModelRebuildMode::Incremental);
            assert_eq!(
                serde_json::to_value(outcome.model.snapshot()).unwrap(),
                expected_snapshot,
            );
            samples.push(outcome.report.duration_ms);
            last_report = Some(outcome.report);
        }
        full_samples.sort_unstable();
        samples.sort_unstable();
        let full_p95 = full_samples[(full_samples.len() * 95).div_ceil(100).saturating_sub(1)];
        let p95 = samples[(samples.len() * 95).div_ceil(100).saturating_sub(1)];
        let report = last_report.unwrap();
        eprintln!(
            "project_model_incremental full_samples={full_samples:?} full_p95_ms={full_p95} incremental_samples={samples:?} incremental_p95_ms={p95} clone_ms={} parse_ms={} component_ms={} block_ms={} tera_ms={}",
            report.model_clone_ms,
            report.template_parse_ms,
            report.component_graph_ms,
            report.block_graph_ms,
            report.tera_graph_ms,
        );
        assert!(p95 <= 50, "incremental ProjectModel p95 {p95} ms > 50 ms");
    }

    fn initial_sources() -> HashMap<String, String> {
        HashMap::from([
            ("zola.toml".to_string(), "base_url = '/'\n".to_string()),
            (
                "content/_index.md".to_string(),
                "+++\ntitle = 'Acasă'\ntemplate = 'index.html'\n+++\n".to_string(),
            ),
            (
                "templates/base.html".to_string(),
                "<body>{% block content %}{% endblock %}</body>".to_string(),
            ),
            (
                "templates/index.html".to_string(),
                initial_index_source(),
            ),
            (
                "templates/partials/footer.html".to_string(),
                "<footer>Subsol</footer>".to_string(),
            ),
            (
                "templates/macros/ui.html".to_string(),
                "{% macro badge(text) %}<strong>{{ text }}</strong>{% endmacro %}".to_string(),
            ),
            (
                "templates/pagina.html".to_string(),
                "{% extends \"base.html\" %}{% block content %}<article>{{ page.content | safe }}</article>{% endblock %}".to_string(),
            ),
            (
                "content/blog/_index.md".to_string(),
                "+++\ntitle = 'Blog'\npage_template = 'pagina.html'\n+++\n".to_string(),
            ),
            (
                "content/blog/post.md".to_string(),
                "+++\ntitle = 'Postare'\n+++\nText\n".to_string(),
            ),
            (
                "static/data/catalog.toml".to_string(),
                "title = 'Catalog'\n".to_string(),
            ),
            (
                "static/app.js".to_string(),
                "console.log('app');\n".to_string(),
            ),
            (
                "static/icon.svg".to_string(),
                "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\n".to_string(),
            ),
            (
                "sass/pagini/index.scss".to_string(),
                ".lead { color: red; }\n".to_string(),
            ),
        ])
    }

    fn initial_index_source() -> String {
        concat!(
            "{% extends \"base.html\" %}",
            "{% import \"macros/ui.html\" as ui %}",
            "{% block content %}",
            "{% set catalog = load_data(path=\"data/catalog.toml\") %}",
            "<main><h1>Titlu</h1><p class=\"lead\">Text</p>",
            "{{ ui::badge(text=catalog.title) }}",
            "{% include \"partials/footer.html\" %}",
            "<img src=\"{{ get_url(path='icon.svg') }}\" alt=\"Icon\">",
            "<script src=\"{{ get_url(path='app.js') }}\"></script></main>",
            "{% endblock %}",
        )
        .to_string()
    }

    fn moved_index_source() -> String {
        initial_index_source().replace(
            "<h1>Titlu</h1><p class=\"lead\">Text</p>",
            "<p class=\"lead\">Text</p><h1>Titlu</h1>",
        )
    }

    fn projection(
        root: &Path,
        revision: u64,
        transaction_id: Option<&str>,
        source_texts: HashMap<String, String>,
        changed_paths: HashSet<String>,
    ) -> WorkspaceProjectionSnapshot {
        let canonical = root.canonicalize().unwrap().to_string_lossy().to_string();
        let runtime_session_id = "incremental-model-test".to_string();
        WorkspaceProjectionSnapshot {
            project_root: canonical.clone(),
            runtime_session_id: runtime_session_id.clone(),
            revision,
            workspace_transaction_id: transaction_id.map(str::to_string),
            source_texts,
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths,
            accepted_disk: AcceptedProjectDiskManifest::new(
                runtime_session_id,
                canonical.clone(),
                ProjectDiskManifest {
                    root: canonical,
                    files: Vec::new(),
                    truncated: false,
                    max_files: 10_000,
                },
            )
            .unwrap(),
        }
    }

    fn assert_fallback_matches_oracle(
        root: &Path,
        before: &ProjectModel,
        previous_revision: u64,
        projection: WorkspaceProjectionSnapshot,
        changed_paths: &[String],
        intent: ProjectModelIncrementalIntent,
        expected_reason: &str,
    ) {
        let outcome = rebuild_project_model_after_workspace_change(
            root,
            Some(before),
            Some(previous_revision),
            &projection,
            changed_paths,
            intent,
        )
        .unwrap();
        let oracle = build_project_model_from_workspace_projection(root, &projection).unwrap();
        assert_eq!(outcome.report.mode, ProjectModelRebuildMode::FullFallback);
        assert_eq!(
            outcome.report.fallback_reason.as_deref(),
            Some(expected_reason)
        );
        assert_eq!(
            serde_json::to_value(outcome.model.snapshot()).unwrap(),
            serde_json::to_value(oracle.snapshot()).unwrap(),
        );
    }

    fn unique_test_dir() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "pana-project-model-incremental-{}-{serial}",
            std::process::id()
        ))
    }
}
