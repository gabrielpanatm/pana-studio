use std::{collections::BTreeSet, path::Path, time::Instant};

use serde::Serialize;

use crate::{
    kernel::project_workspace::WorkspaceProjectionSnapshot,
    project::zola_project_root,
    project_model::{
        files::{model_revision, project_model_file},
        model::{ProjectModel, ProjectModelFileKind},
    },
    source_graph::{
        identity::{reconcile_project_source_node_ids, SourceChangeSet},
        rebuild_local_template_graph, SourceGraphIncrementalFallback,
        SourceGraphIncrementalTemplateReport,
    },
};

pub(crate) const PROJECT_MODEL_INCREMENTAL_REPORT_SCHEMA_VERSION: u32 = 4;

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
    pub(crate) model_clone_us: u64,
    pub(crate) template_parse_us: u64,
    pub(crate) component_graph_us: u64,
    pub(crate) block_graph_us: u64,
    pub(crate) content_model_us: u64,
    pub(crate) listing_items_us: u64,
    pub(crate) listing_items_reused: bool,
    pub(crate) dynamic_widget_us: u64,
    pub(crate) markdown_us: u64,
    pub(crate) node_index_us: u64,
    pub(crate) duration_ms: u64,
    pub(crate) duration_us: u64,
    pub(crate) fallback_reason: Option<String>,
}

pub(crate) struct ProjectModelIncrementalBuildOutcome {
    pub(crate) model: ProjectModel,
    pub(crate) report: ProjectModelIncrementalBuildReport,
}

pub(crate) struct ProjectModelComposedBuildOutcome {
    pub(crate) intermediate_model: ProjectModel,
    pub(crate) final_build: ProjectModelIncrementalBuildOutcome,
}

pub(crate) fn rebuild_project_model_after_workspace_change(
    project_root: &Path,
    previous_model: Option<&ProjectModel>,
    previous_workspace_revision: Option<u64>,
    projection: &WorkspaceProjectionSnapshot,
    exact_changed_paths: &[String],
    intent: ProjectModelIncrementalIntent,
) -> Result<ProjectModelIncrementalBuildOutcome, String> {
    rebuild_project_model_after_workspace_change_with_source_changes(
        project_root,
        previous_model,
        previous_workspace_revision,
        projection,
        exact_changed_paths,
        intent,
        None,
    )
}

pub(crate) fn rebuild_project_model_after_workspace_change_with_source_changes(
    project_root: &Path,
    previous_model: Option<&ProjectModel>,
    previous_workspace_revision: Option<u64>,
    projection: &WorkspaceProjectionSnapshot,
    exact_changed_paths: &[String],
    intent: ProjectModelIncrementalIntent,
    supplied_source_changes: Option<Vec<SourceChangeSet>>,
) -> Result<ProjectModelIncrementalBuildOutcome, String> {
    if let Some(changes) = supplied_source_changes.as_deref() {
        require_source_change_revisions(previous_model, projection, changes)?;
    }
    let started = Instant::now();
    match try_incremental_build(
        project_root,
        previous_model,
        previous_workspace_revision,
        projection,
        exact_changed_paths,
        intent,
        supplied_source_changes.as_deref(),
    ) {
        Ok((model, graph_report, changed_paths, model_clone_us)) => {
            Ok(ProjectModelIncrementalBuildOutcome {
                model,
                report: report(
                    ProjectModelRebuildMode::Incremental,
                    projection,
                    changed_paths,
                    Some(graph_report),
                    model_clone_us,
                    None,
                    elapsed_us(started),
                ),
            })
        }
        Err(reason) => {
            let mut model =
                super::build_project_model_from_workspace_projection(project_root, projection)?;
            if let Some(previous) = previous_model {
                let mut change_sets = supplied_source_changes.unwrap_or_else(|| {
                    source_change_sets(previous, projection, exact_changed_paths)
                });
                reconcile_project_source_node_ids(
                    &previous.source_graph,
                    &mut model.source_graph,
                    &mut change_sets,
                )?;
                rebuild_derived_graphs(project_root, projection, &mut model);
            }
            Ok(ProjectModelIncrementalBuildOutcome {
                model,
                report: report(
                    ProjectModelRebuildMode::FullFallback,
                    projection,
                    normalized_changed_paths(exact_changed_paths).unwrap_or_default(),
                    None,
                    0,
                    Some(reason.code()),
                    elapsed_us(started),
                ),
            })
        }
    }
}

/// Rebuilds one published workspace revision whose source was produced by two
/// ordered semantic transitions: the user structural edit, followed by a
/// native-block contract rewrite. Each transition owns its own SourceChangeSet
/// so identity reconciliation never attributes contract-owned topology to the
/// user's tree insertion.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_project_model_after_composed_workspace_change_with_source_changes(
    project_root: &Path,
    previous_model: Option<&ProjectModel>,
    previous_workspace_revision: Option<u64>,
    intermediate_projection: &WorkspaceProjectionSnapshot,
    final_projection: &WorkspaceProjectionSnapshot,
    intermediate_changed_paths: &[String],
    final_changed_paths: &[String],
    intent: ProjectModelIncrementalIntent,
    supplied_source_changes: Option<Vec<SourceChangeSet>>,
) -> Result<ProjectModelComposedBuildOutcome, String> {
    let mut intermediate = rebuild_project_model_after_workspace_change_with_source_changes(
        project_root,
        previous_model,
        previous_workspace_revision,
        intermediate_projection,
        intermediate_changed_paths,
        intent,
        supplied_source_changes,
    )?;

    let contract_started = Instant::now();
    let mut final_model =
        super::build_project_model_from_workspace_projection(project_root, final_projection)?;
    let mut contract_changes =
        source_change_sets(&intermediate.model, final_projection, final_changed_paths);
    reconcile_project_source_node_ids(
        &intermediate.model.source_graph,
        &mut final_model.source_graph,
        &mut contract_changes,
    )?;
    rebuild_derived_graphs(project_root, final_projection, &mut final_model);

    intermediate.report.changed_paths =
        normalized_changed_paths(final_changed_paths).map_err(|reason| {
            format!(
                "Tranziția structurală compusă a refuzat căile finale: {}.",
                reason.code()
            )
        })?;
    intermediate.report.workspace_revision = final_projection.revision;
    intermediate.report.workspace_transaction_id =
        final_projection.workspace_transaction_id.clone();
    intermediate.report.duration_us = intermediate
        .report
        .duration_us
        .saturating_add(elapsed_us(contract_started));
    intermediate.report.duration_ms = intermediate.report.duration_us / 1_000;
    Ok(ProjectModelComposedBuildOutcome {
        intermediate_model: intermediate.model,
        final_build: ProjectModelIncrementalBuildOutcome {
            model: final_model,
            report: intermediate.report,
        },
    })
}

fn require_source_change_revisions(
    previous_model: Option<&ProjectModel>,
    projection: &WorkspaceProjectionSnapshot,
    changes: &[SourceChangeSet],
) -> Result<(), String> {
    let previous = previous_model.ok_or_else(|| {
        "SourceChangeSet a fost furnizat fără ProjectModel-ul reviziei de bază.".to_string()
    })?;
    let mut files = BTreeSet::new();
    for change in changes {
        if !files.insert(change.file.as_str()) {
            return Err(format!(
                "SourceChangeSet conține două autorități pentru {}.",
                change.file
            ));
        }
        let before = previous
            .files
            .iter()
            .find(|file| file.relative_path == change.file)
            .ok_or_else(|| {
                format!(
                    "SourceChangeSet nu găsește {} în ProjectModel-ul de bază.",
                    change.file
                )
            })?;
        let after = projection.source_texts.get(&change.file).ok_or_else(|| {
            format!(
                "SourceChangeSet nu găsește {} în proiecția rezultată.",
                change.file
            )
        })?;
        change.require_sources(&before.contents, after)?;
    }
    Ok(())
}

fn source_change_sets(
    previous: &ProjectModel,
    projection: &WorkspaceProjectionSnapshot,
    changed_paths: &[String],
) -> Vec<SourceChangeSet> {
    changed_paths
        .iter()
        .filter_map(|path| {
            let before = previous
                .files
                .iter()
                .find(|file| file.relative_path == *path)?;
            let after = projection.source_texts.get(path)?;
            Some(SourceChangeSet::between(path, &before.contents, after))
        })
        .collect()
}

fn rebuild_derived_graphs(
    project_root: &Path,
    projection: &WorkspaceProjectionSnapshot,
    model: &mut ProjectModel,
) {
    model.source_graph.component_graph =
        crate::source_graph::component_graph::build_component_graph(&model.source_graph);
    model.source_graph.block_graph = crate::blocks::graph::build_block_graph(&model.source_graph);
    model.source_graph.content_models =
        crate::kernel::content_models::build_content_model_catalog_from_workspace_projection(
            project_root,
            &projection.source_texts,
            &projection.deleted_sources,
            &model.source_graph,
        );
    model.source_graph.listing_items =
        crate::kernel::listing_items::build_listing_item_catalog_from_workspace_projection(
            project_root,
            &projection.source_texts,
            &projection.deleted_sources,
            &model.source_graph,
        );
    model.source_graph.dynamic_widget_graph =
        crate::kernel::dynamic_widgets::build_dynamic_widget_graph_from_workspace_projection(
            project_root,
            &projection.source_texts,
            &projection.deleted_sources,
            &model.source_graph,
        );
    model.source_graph.markdown_projections =
        crate::source_graph::markdown::build_markdown_projections(&model.source_graph);
}

fn try_incremental_build(
    project_root: &Path,
    previous_model: Option<&ProjectModel>,
    previous_workspace_revision: Option<u64>,
    projection: &WorkspaceProjectionSnapshot,
    exact_changed_paths: &[String],
    intent: ProjectModelIncrementalIntent,
    supplied_source_changes: Option<&[SourceChangeSet]>,
) -> Result<
    (
        ProjectModel,
        SourceGraphIncrementalTemplateReport,
        Vec<String>,
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
    let model_clone_us = elapsed_us(model_clone_started);
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
        &previous_file.contents,
        &projection.source_texts,
        supplied_source_changes.and_then(|changes| {
            changes
                .iter()
                .find(|change| change.file == *relative_path)
                .cloned()
        }),
    )
    .map_err(ProjectModelIncrementalFallback::SourceGraph)?;
    next.source_graph = source_graph;
    next.revision = model_revision(&next.files);
    Ok((next, graph_report, changed_paths, model_clone_us))
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
    let model_clone_us = elapsed_us(model_clone_started);
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
        template_parse_us: 0,
        component_graph_us: 0,
        block_graph_us: 0,
        content_model_us: 0,
        listing_items_us: 0,
        listing_items_reused: true,
        dynamic_widget_us: 0,
        markdown_us: 0,
        node_index_us: 0,
    };
    Ok((next, graph_report, changed_paths, model_clone_us))
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
                || !projection.source_texts.contains_key(path))
    });
    let created = changed_paths.iter().filter(|path| {
        !previous
            .files
            .iter()
            .any(|file| file.relative_path == ***path)
            && projection.source_texts.contains_key(path)
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

// The report constructor mirrors the immutable incremental-build telemetry schema.
#[allow(clippy::too_many_arguments)]
fn report(
    mode: ProjectModelRebuildMode,
    projection: &WorkspaceProjectionSnapshot,
    changed_paths: Vec<String>,
    graph: Option<SourceGraphIncrementalTemplateReport>,
    model_clone_us: u64,
    fallback_reason: Option<String>,
    duration_us: u64,
) -> ProjectModelIncrementalBuildReport {
    let graph = graph.unwrap_or(SourceGraphIncrementalTemplateReport {
        invalidated_template_files: Vec::new(),
        invalidated_page_files: Vec::new(),
        replaced_nodes: 0,
        reused_nodes: 0,
        reused_relations: 0,
        template_parse_us: 0,
        component_graph_us: 0,
        block_graph_us: 0,
        content_model_us: 0,
        listing_items_us: 0,
        listing_items_reused: false,
        dynamic_widget_us: 0,
        markdown_us: 0,
        node_index_us: 0,
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
        model_clone_ms: model_clone_us / 1_000,
        model_clone_us,
        template_parse_us: graph.template_parse_us,
        component_graph_us: graph.component_graph_us,
        block_graph_us: graph.block_graph_us,
        content_model_us: graph.content_model_us,
        listing_items_us: graph.listing_items_us,
        listing_items_reused: graph.listing_items_reused,
        dynamic_widget_us: graph.dynamic_widget_us,
        markdown_us: graph.markdown_us,
        node_index_us: graph.node_index_us,
        duration_ms: duration_us / 1_000,
        duration_us,
        fallback_reason,
    }
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u64::MAX as u128) as u64
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
        project_model::{
            build_project_model_from_workspace_projection,
            move_engine::{plan_html_move, ProjectHtmlMoveIntent, ProjectMovePosition},
        },
        source_graph::identity::{SourceTextEdit, SourceTreeMovePosition},
        source_graph::model::{ComponentInvocationKind, SourceRelationKind},
    };

    use super::*;

    #[test]
    fn generated_class_preserves_the_exact_identity_of_three_identical_siblings() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_html = concat!(
            "{% extends \"base.html\" %}{% block content %}",
            "<main><div>unu</div><div>doi</div><div>trei</div></main>",
            "{% endblock %}",
        );
        let mut before_sources = initial_sources();
        before_sources.insert("templates/index.html".to_string(), before_html.to_string());
        let before_projection = projection(&root, 40, None, before_sources, HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let before_ids = html_node_ids_for_label(&before, "<div>");
        assert_eq!(before_ids.len(), 3);

        for (target_index, body) in [
            "<main><div class=\"ps-div-test1234\">unu</div><div>doi</div><div>trei</div></main>",
            "<main><div>unu</div><div class=\"ps-div-test1234\">doi</div><div>trei</div></main>",
            "<main><div>unu</div><div>doi</div><div class=\"ps-div-test1234\">trei</div></main>",
        ]
        .into_iter()
        .enumerate()
        {
            let after_html =
                format!("{{% extends \"base.html\" %}}{{% block content %}}{body}{{% endblock %}}");
            let mut after_sources = initial_sources();
            after_sources.insert("templates/index.html".to_string(), after_html);
            let after_projection = projection(
                &root,
                41,
                Some("generated-class-41"),
                after_sources,
                HashSet::from(["templates/index.html".to_string()]),
            );
            let outcome = rebuild_project_model_after_workspace_change(
                &root,
                Some(&before),
                Some(40),
                &after_projection,
                &["templates/index.html".to_string()],
                ProjectModelIncrementalIntent::HtmlStructural,
            )
            .unwrap();
            let mut after_divs = outcome
                .model
                .source_graph
                .nodes
                .iter()
                .filter(|node| {
                    node.file == "templates/index.html"
                        && node.kind == crate::source_graph::model::SourceNodeKind::Html
                        && node.label.starts_with("<div")
                })
                .collect::<Vec<_>>();
            after_divs.sort_by_key(|node| node.range.as_ref().map(|range| range.start));

            assert_eq!(
                after_divs
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<Vec<_>>(),
                before_ids,
                "ținta #{target_index}",
            );
            assert_eq!(after_divs[target_index].label, "<div .ps-div-test1234>");
            assert_eq!(
                after_divs
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<HashSet<_>>()
                    .len(),
                3,
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn move_change_set_preserves_exact_ids_for_identical_siblings_and_subtree() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_html = concat!(
            "{% extends \"base.html\" %}{% block content %}",
            "<main><section><span>A</span></section><section><span>B</span></section>",
            "<section><span>C</span></section></main>{% endblock %}",
        );
        let mut before_sources = initial_sources();
        before_sources.insert("templates/index.html".to_string(), before_html.to_string());
        let before_projection = projection(&root, 50, None, before_sources, HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let before_sections = html_node_ids_for_label(&before, "<section>");
        let before_spans = html_node_ids_for_label(&before, "<span>");
        assert_eq!(before_sections.len(), 3);
        assert_eq!(before_spans.len(), 3);

        let plan = plan_html_move(
            &before,
            &ProjectHtmlMoveIntent {
                source_source_id: Some(before_sections[0].clone()),
                target_source_id: Some(before_sections[2].clone()),
                source_tag: Some("section".to_string()),
                target_tag: Some("section".to_string()),
                position: ProjectMovePosition::After,
                native_block_slot: None,
            },
        );
        assert!(plan.allowed, "{:?}", plan.diagnostic);
        let patch = plan.patch.unwrap();
        let mut after_sources = initial_sources();
        after_sources.insert(patch.file.clone(), patch.contents.clone());
        let after_projection = projection(
            &root,
            51,
            Some("move-identical-siblings"),
            after_sources,
            HashSet::from([patch.file.clone()]),
        );
        let source_changes =
            vec![
                SourceChangeSet::between(&patch.file, before_html, &patch.contents).with_tree_move(
                    &patch.resolved_source_id,
                    &patch.resolved_target_id,
                    SourceTreeMovePosition::After,
                ),
            ];
        let outcome = rebuild_project_model_after_workspace_change_with_source_changes(
            &root,
            Some(&before),
            Some(50),
            &after_projection,
            &[patch.file],
            ProjectModelIncrementalIntent::HtmlStructural,
            Some(source_changes),
        )
        .unwrap();

        assert_eq!(
            html_node_ids_for_label(&outcome.model, "<section>"),
            vec![
                before_sections[1].clone(),
                before_sections[2].clone(),
                before_sections[0].clone(),
            ]
        );
        assert_eq!(
            html_node_ids_for_label(&outcome.model, "<span>"),
            vec![
                before_spans[1].clone(),
                before_spans[2].clone(),
                before_spans[0].clone(),
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disjoint_code_edits_do_not_replace_the_untouched_identical_sibling() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_html = concat!(
            "{% extends \"base.html\" %}{% block content %}",
            "<main><div>A</div><div>B</div><div>C</div></main>",
            "{% endblock %}",
        );
        let mut before_sources = initial_sources();
        before_sources.insert("templates/index.html".to_string(), before_html.to_string());
        let before_projection = projection(&root, 60, None, before_sources, HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let before_ids = html_node_ids_for_label(&before, "<div>");
        assert_eq!(before_ids.len(), 3);

        let first_start = before_html.find("<div>A").unwrap() + "<div".len();
        let last_start = before_html.rfind("<div>C").unwrap() + "<div".len();
        let first_insert = " data-edge=\"first\"";
        let last_insert = " data-edge=\"last\"";
        let mut after_html = before_html.to_string();
        after_html.insert_str(last_start, last_insert);
        after_html.insert_str(first_start, first_insert);
        let exact_edits = vec![
            SourceTextEdit {
                old_start: first_start,
                old_end: first_start,
                new_start: first_start,
                new_end: first_start + first_insert.len(),
            },
            SourceTextEdit {
                old_start: last_start,
                old_end: last_start,
                new_start: last_start + first_insert.len(),
                new_end: last_start + first_insert.len() + last_insert.len(),
            },
        ];
        let mut after_sources = initial_sources();
        after_sources.insert("templates/index.html".to_string(), after_html.clone());
        let after_projection = projection(
            &root,
            61,
            Some("code-disjoint-61"),
            after_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );
        let change = SourceChangeSet::between("templates/index.html", before_html, &after_html)
            .with_exact_text_edits(exact_edits);
        let outcome = rebuild_project_model_after_workspace_change_with_source_changes(
            &root,
            Some(&before),
            Some(60),
            &after_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            Some(vec![change]),
        )
        .unwrap();

        let after_ids = outcome
            .model
            .source_graph
            .nodes
            .iter()
            .filter(|node| {
                node.file == "templates/index.html"
                    && node.kind == crate::source_graph::model::SourceNodeKind::Html
                    && node.label.starts_with("<div")
            })
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        assert_eq!(after_ids, before_ids);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn supplied_source_change_set_rejects_another_result_revision() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let before_html = "<main><div>A</div></main>";
        let mut before_sources = initial_sources();
        before_sources.insert("templates/index.html".to_string(), before_html.to_string());
        let before_projection = projection(&root, 70, None, before_sources, HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let declared_after = "<main><div>B</div></main>";
        let actual_after = "<main><div>C</div></main>";
        let mut after_sources = initial_sources();
        after_sources.insert("templates/index.html".to_string(), actual_after.to_string());
        let after_projection = projection(
            &root,
            71,
            Some("stale-source-change-71"),
            after_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );
        let result = rebuild_project_model_after_workspace_change_with_source_changes(
            &root,
            Some(&before),
            Some(70),
            &after_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
            Some(vec![SourceChangeSet::between(
                "templates/index.html",
                before_html,
                declared_after,
            )]),
        );
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("SourceChangeSet stale a fost acceptat."),
        };
        assert!(error.contains("revizia rezultatului stale"), "{error}");
        assert!(error.contains("așteptată source_"), "{error}");
        assert!(error.contains("actuală source_"), "{error}");
        fs::remove_dir_all(root).unwrap();
    }

    fn html_node_ids_for_label(model: &ProjectModel, label: &str) -> Vec<String> {
        let mut nodes = model
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.file == "templates/index.html" && node.label == label)
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| node.range.as_ref().map(|range| range.start));
        nodes.into_iter().map(|node| node.id.clone()).collect()
    }

    fn assert_model_semantics_match(actual: &ProjectModel, expected: &ProjectModel) {
        let actual = canonical_model_semantics(actual);
        let expected = canonical_model_semantics(expected);
        if let Some(difference) = first_json_difference("$", &actual, &expected) {
            panic!("project model semantic mismatch: {difference}");
        }
    }

    fn first_json_difference(
        path: &str,
        actual: &serde_json::Value,
        expected: &serde_json::Value,
    ) -> Option<String> {
        match (actual, expected) {
            (serde_json::Value::Array(actual), serde_json::Value::Array(expected)) => {
                if actual.len() != expected.len() {
                    return Some(format!(
                        "{path}: array length {} != {}",
                        actual.len(),
                        expected.len()
                    ));
                }
                actual
                    .iter()
                    .zip(expected)
                    .enumerate()
                    .find_map(|(index, (actual, expected))| {
                        first_json_difference(&format!("{path}[{index}]"), actual, expected)
                    })
            }
            (serde_json::Value::Object(actual), serde_json::Value::Object(expected)) => {
                let mut keys = actual.keys().chain(expected.keys()).collect::<Vec<_>>();
                keys.sort_unstable();
                keys.dedup();
                keys.into_iter().find_map(|key| {
                    let next_path = format!("{path}.{key}");
                    match (actual.get(key), expected.get(key)) {
                        (Some(actual), Some(expected)) => {
                            first_json_difference(&next_path, actual, expected)
                        }
                        (Some(actual), None) => Some(format!("{next_path}: unexpected {actual:?}")),
                        (None, Some(expected)) => {
                            Some(format!("{next_path}: missing {expected:?}"))
                        }
                        (None, None) => None,
                    }
                })
            }
            _ if actual == expected => None,
            _ => Some(format!("{path}: {actual:?} != {expected:?}")),
        }
    }

    fn canonical_model_semantics(model: &ProjectModel) -> serde_json::Value {
        let runtime_ids = canonical_runtime_identities(model);
        let mut snapshot = serde_json::to_value(model.snapshot()).unwrap();
        canonicalize_runtime_identities(&mut snapshot, &runtime_ids);
        normalize_graph_collection_order(&mut snapshot);
        snapshot
    }

    fn canonical_runtime_identities(model: &ProjectModel) -> HashMap<String, String> {
        let mut identities = HashMap::new();
        let mut extend = |kind: &str, values: Vec<String>| {
            identities.extend(
                values
                    .into_iter()
                    .enumerate()
                    .map(|(index, id)| (id, format!("opaque-{kind}-{index}"))),
            );
        };
        extend(
            "source-node",
            model
                .source_graph
                .nodes
                .iter()
                .map(|node| node.id.clone())
                .collect(),
        );
        extend(
            "source-relation",
            model
                .source_graph
                .relations
                .iter()
                .map(|relation| relation.id.clone())
                .collect(),
        );
        extend(
            "component-definition",
            model
                .source_graph
                .component_graph
                .definitions
                .iter()
                .map(|definition| definition.id.clone())
                .collect(),
        );
        extend(
            "component-invocation",
            model
                .source_graph
                .component_graph
                .invocations
                .iter()
                .map(|invocation| invocation.id.clone())
                .collect(),
        );
        extend(
            "component-rendered-instance",
            model
                .source_graph
                .component_graph
                .rendered_instances
                .iter()
                .map(|instance| instance.id.clone())
                .collect(),
        );
        extend(
            "block-source-instance",
            model
                .source_graph
                .block_graph
                .source_instances
                .iter()
                .map(|instance| instance.id.clone())
                .collect(),
        );
        extend(
            "dynamic-source-instance",
            model
                .source_graph
                .dynamic_widget_graph
                .source_instances
                .iter()
                .map(|instance| instance.id.clone())
                .collect(),
        );
        extend(
            "markdown-projection",
            model
                .source_graph
                .markdown_projections
                .iter()
                .map(|projection| projection.id.clone())
                .collect(),
        );
        identities
    }

    fn normalize_graph_collection_order(snapshot: &mut serde_json::Value) {
        for graph_name in ["sourceGraph"] {
            let Some(graph) = snapshot
                .as_object_mut()
                .and_then(|root| root.get_mut(graph_name))
                .and_then(serde_json::Value::as_object_mut)
            else {
                continue;
            };
            for collection in ["nodes", "relations", "templates"] {
                let Some(values) = graph
                    .get_mut(collection)
                    .and_then(serde_json::Value::as_array_mut)
                else {
                    continue;
                };
                values.sort_by_cached_key(|value| serde_json::to_string(value).unwrap());
            }
        }
    }

    fn canonicalize_runtime_identities(
        value: &mut serde_json::Value,
        runtime_ids: &HashMap<String, String>,
    ) {
        match value {
            serde_json::Value::String(text) => {
                if let Some(canonical) = runtime_ids.get(text) {
                    *text = canonical.clone();
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    canonicalize_runtime_identities(value, runtime_ids);
                }
            }
            serde_json::Value::Object(object) => {
                for value in object.values_mut() {
                    canonicalize_runtime_identities(value, runtime_ids);
                }
            }
            _ => {}
        }
    }

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

        assert_eq!(
            outcome.report.mode,
            ProjectModelRebuildMode::Incremental,
            "{:?}",
            outcome.report.fallback_reason
        );
        assert_eq!(outcome.report.fallback_reason, None);
        assert!(outcome.report.reused_nodes > 0);
        assert!(outcome
            .report
            .invalidated_template_files
            .contains(&"templates/index.html".to_string()));
        assert_model_semantics_match(&outcome.model, &oracle);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rich_derived_projections_and_undo_match_the_full_builder() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let initial = rich_derived_sources();
        let before_projection = projection(&root, 70, None, initial.clone(), HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        assert!(!before.source_graph.component_graph.invocations.is_empty());
        assert!(!before.source_graph.block_graph.source_instances.is_empty());
        assert!(!before
            .source_graph
            .content_models
            .template_usages
            .is_empty());
        assert!(!before.source_graph.listing_items.items.is_empty());
        assert_eq!(
            before
                .source_graph
                .dynamic_widget_graph
                .source_instances
                .len(),
            2
        );
        assert!(before
            .source_graph
            .dynamic_widget_graph
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "dynamic_widget_duplicate_instance"));
        assert!(!before.source_graph.markdown_projections.is_empty());

        let mut changed_sources = initial.clone();
        changed_sources.insert(
            "templates/index.html".to_string(),
            rich_index_source().replace("Titlu</h1>", "Titlu actualizat</h1>"),
        );
        let changed_projection = projection(
            &root,
            71,
            Some("rich-derived-71"),
            changed_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );
        let changed = rebuild_project_model_after_workspace_change(
            &root,
            Some(&before),
            Some(70),
            &changed_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        let changed_oracle =
            build_project_model_from_workspace_projection(&root, &changed_projection).unwrap();
        assert_eq!(changed.report.mode, ProjectModelRebuildMode::Incremental);
        assert!(changed.report.listing_items_reused);
        assert_model_semantics_match(&changed.model, &changed_oracle);

        let undo_projection = projection(
            &root,
            72,
            Some("rich-derived-undo-72"),
            initial,
            HashSet::from(["templates/index.html".to_string()]),
        );
        let undone = rebuild_project_model_after_workspace_change(
            &root,
            Some(&changed.model),
            Some(71),
            &undo_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        let undo_oracle =
            build_project_model_from_workspace_projection(&root, &undo_projection).unwrap();
        assert_eq!(undone.report.mode, ProjectModelRebuildMode::Incremental);
        assert_model_semantics_match(&undone.model, &undo_oracle);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_icon_change_is_atomic_and_matches_the_full_builder_exactly() {
        let root = unique_test_dir();
        fs::create_dir_all(&root).unwrap();
        let home = crate::blocks::icons::render_icon_block_html(
            "home",
            "ps-icon-test",
            "ps-icon-test",
            "icon-test",
        )
        .unwrap();
        let star = crate::blocks::icons::render_icon_block_html(
            "star",
            "ps-icon-test",
            "ps-icon-test",
            "icon-test",
        )
        .unwrap();
        let mut before_sources = initial_sources();
        before_sources.insert(
            "templates/index.html".to_string(),
            initial_index_source().replace("</main>", &format!("{home}</main>")),
        );
        let before_projection = projection(&root, 30, None, before_sources, HashSet::new());
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let mut after_sources = initial_sources();
        after_sources.insert(
            "templates/index.html".to_string(),
            initial_index_source().replace("</main>", &format!("{star}</main>")),
        );
        let after_projection = projection(
            &root,
            31,
            Some("icon-change-31"),
            after_sources,
            HashSet::from(["templates/index.html".to_string()]),
        );

        let outcome = rebuild_project_model_after_workspace_change(
            &root,
            Some(&before),
            Some(30),
            &after_projection,
            &["templates/index.html".to_string()],
            ProjectModelIncrementalIntent::HtmlStructural,
        )
        .unwrap();
        let oracle =
            build_project_model_from_workspace_projection(&root, &after_projection).unwrap();
        let icon_markers = outcome
            .model
            .source_graph
            .nodes
            .iter()
            .filter(|node| {
                node.kind == crate::source_graph::model::SourceNodeKind::BlockMarker
                    && node.label == "icon"
            })
            .count();
        let svg_descendants = outcome
            .model
            .source_graph
            .nodes
            .iter()
            .filter(|node| node.label.starts_with("<path"))
            .count();

        assert_eq!(outcome.report.mode, ProjectModelRebuildMode::Incremental);
        assert_eq!(icon_markers, 1);
        assert_eq!(svg_descendants, 0);
        assert_model_semantics_match(&outcome.model, &oracle);
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
        assert_eq!(outcome.report.template_parse_us, 0);
        assert_model_semantics_match(&outcome.model, &oracle);
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
        assert_model_semantics_match(&outcome.model, &oracle);
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
            assert_model_semantics_match(&outcome.model, &oracle);
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
        assert_model_semantics_match(&restored.model, &oracle);

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
        assert_model_semantics_match(&redone.model, &oracle);
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
        assert_model_semantics_match(&local_override.model, &oracle);

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
            assert_model_semantics_match(&outcome.model, &oracle);
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
    #[ignore = "performance baseline; run through npm run performance:baseline"]
    fn performance_baseline_project_open_large_fixture() {
        let root = performance_project_root();
        let (sample_count, warmup_count) = performance_sample_configuration();
        let mut samples = Vec::with_capacity(sample_count);
        for sample in 0..(warmup_count + sample_count) {
            let started = Instant::now();
            let projection = performance_disk_projection(
                &root,
                u64::try_from(sample + 1).expect("revision benchmark"),
            );
            let model = build_project_model_from_workspace_projection(&root, &projection).unwrap();
            std::hint::black_box((model.files.len(), model.source_graph.nodes.len()));
            if sample >= warmup_count {
                samples.push(elapsed_us(started));
            }
        }
        emit_performance_baseline(
            "project_open",
            "kernel_ingest_and_model",
            &mut samples,
            None,
        );
    }

    #[test]
    #[ignore = "performance baseline; run through npm run performance:baseline"]
    fn performance_baseline_html_edit_large_fixture() {
        let root = performance_project_root();
        let (sample_count, warmup_count) = performance_sample_configuration();
        // This ignored real-project benchmark owns an explicit filesystem
        // ingestion boundary. ProjectModel still receives only the immutable
        // projection captured at that boundary.
        let manifest = crate::project::read_project_disk_manifest(&root).unwrap();
        let source_texts = manifest
            .files
            .iter()
            .filter_map(|entry| {
                fs::read_to_string(root.join(&entry.relative_path))
                    .ok()
                    .map(|source| (entry.relative_path.clone(), source))
            })
            .collect::<HashMap<_, _>>();
        let runtime_session_id = "incremental-real-benchmark".to_string();
        let before_projection = WorkspaceProjectionSnapshot {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: runtime_session_id.clone(),
            revision: 1,
            workspace_transaction_id: Some("incremental-real-benchmark-1".to_string()),
            source_texts: source_texts.into(),
            resource_bytes: HashMap::new().into(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::new(),
            accepted_disk: AcceptedProjectDiskManifest::new(
                runtime_session_id.clone(),
                root.to_string_lossy().to_string(),
                manifest.clone(),
            )
            .unwrap()
            .into(),
        };
        let before =
            build_project_model_from_workspace_projection(&root, &before_projection).unwrap();
        let target_path = before
            .source_graph
            .templates
            .iter()
            .max_by_key(|template| {
                before
                    .source_graph
                    .nodes
                    .iter()
                    .filter(|node| node.file == template.file)
                    .count()
            })
            .map(|template| template.file.clone())
            .expect("fixture-ul trebuie să conțină un template HTML eligibil");
        let target = before
            .files
            .iter()
            .find(|file| file.relative_path == target_path)
            .expect("template-ul SourceGraph trebuie să existe în ProjectModel");
        let changed = if target.contents.contains("Conținut determinist 0") {
            target.contents.replacen(
                "Conținut determinist 0",
                "Conținut determinist actualizat 0",
                1,
            )
        } else {
            format!("{}\n<!-- pana incremental benchmark -->\n", target.contents)
        };
        let mut source_texts = before
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.contents.clone()))
            .collect::<HashMap<_, _>>();
        source_texts.insert(target.relative_path.clone(), changed);
        let projection = WorkspaceProjectionSnapshot {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: runtime_session_id.clone(),
            revision: 2,
            workspace_transaction_id: Some("incremental-real-benchmark-2".to_string()),
            source_texts: source_texts.into(),
            resource_bytes: HashMap::new().into(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::from([target.relative_path.clone()]),
            accepted_disk: AcceptedProjectDiskManifest::new(
                runtime_session_id,
                root.to_string_lossy().to_string(),
                manifest,
            )
            .unwrap()
            .into(),
        };
        let expected = build_project_model_from_workspace_projection(&root, &projection).unwrap();
        let expected_snapshot = serde_json::to_value(expected.snapshot()).unwrap();
        let mut full_samples = Vec::with_capacity(sample_count);
        for sample in 0..(warmup_count + sample_count) {
            let full_started = Instant::now();
            let oracle = build_project_model_from_workspace_projection(&root, &projection).unwrap();
            if sample >= warmup_count {
                full_samples.push(elapsed_us(full_started));
            }
            if sample == 0 {
                assert_eq!(
                    serde_json::to_value(oracle.snapshot()).unwrap(),
                    expected_snapshot,
                );
            }
        }
        let mut samples = Vec::new();
        let mut template_parse_samples = Vec::new();
        let mut component_graph_samples = Vec::new();
        let mut block_graph_samples = Vec::new();
        let mut content_model_samples = Vec::new();
        let mut listing_items_samples = Vec::new();
        let mut dynamic_widget_samples = Vec::new();
        let mut markdown_samples = Vec::new();
        let mut node_index_samples = Vec::new();
        let mut last_report = None;
        for sample in 0..(warmup_count + sample_count) {
            let outcome = rebuild_project_model_after_workspace_change(
                &root,
                Some(&before),
                Some(1),
                &projection,
                std::slice::from_ref(&target.relative_path),
                ProjectModelIncrementalIntent::HtmlStructural,
            )
            .unwrap();
            assert_eq!(
                outcome.report.mode,
                ProjectModelRebuildMode::Incremental,
                "fallback: {:?}",
                outcome.report.fallback_reason,
            );
            if sample == 0 || sample + 1 == warmup_count + sample_count {
                assert_model_semantics_match(&outcome.model, &expected);
            }
            if sample >= warmup_count {
                samples.push(outcome.report.duration_us);
                template_parse_samples.push(outcome.report.template_parse_us);
                component_graph_samples.push(outcome.report.component_graph_us);
                block_graph_samples.push(outcome.report.block_graph_us);
                content_model_samples.push(outcome.report.content_model_us);
                listing_items_samples.push(outcome.report.listing_items_us);
                dynamic_widget_samples.push(outcome.report.dynamic_widget_us);
                markdown_samples.push(outcome.report.markdown_us);
                node_index_samples.push(outcome.report.node_index_us);
            }
            last_report = Some(outcome.report);
        }
        full_samples.sort_unstable();
        let full_p95 = full_samples[(full_samples.len() * 95).div_ceil(100).saturating_sub(1)];
        let report = last_report.unwrap();
        emit_performance_baseline(
            "html_edit",
            "project_model_incremental",
            &mut samples,
            Some(serde_json::json!({
                "fullP95Us": full_p95,
                "fullSamplesUs": full_samples,
                "projectModelCloneUs": report.model_clone_us,
                "projectModelBuildMode": report.mode.label(),
                "projectModelTemplateParseP95Us": performance_p95(&mut template_parse_samples),
                "projectModelComponentGraphP95Us": performance_p95(&mut component_graph_samples),
                "projectModelBlockGraphP95Us": performance_p95(&mut block_graph_samples),
                "projectModelContentModelP95Us": performance_p95(&mut content_model_samples),
                "projectModelListingItemsP95Us": performance_p95(&mut listing_items_samples),
                "projectModelListingItemsReused": report.listing_items_reused,
                "projectModelDynamicWidgetP95Us": performance_p95(&mut dynamic_widget_samples),
                "projectModelMarkdownP95Us": performance_p95(&mut markdown_samples),
                "projectModelNodeIndexP95Us": performance_p95(&mut node_index_samples),
            })),
        );
        emit_performance_baseline(
            "project_model_build",
            report.mode.label(),
            &mut samples,
            Some(serde_json::json!({
                "projectModelCloneUs": report.model_clone_us,
                "projectModelBuildMode": report.mode.label(),
            })),
        );
    }

    #[test]
    #[ignore = "performance baseline; run through npm run performance:baseline"]
    fn performance_baseline_css_edit_large_fixture() {
        let root = performance_project_root();
        let (sample_count, warmup_count) = performance_sample_configuration();
        let before_projection = performance_disk_projection(&root, 1);
        let before = build_project_model_from_workspace_projection(&root, &before_projection)
            .expect("performance baseline ProjectModel");
        let target = before
            .files
            .iter()
            .filter(|file| {
                file.relative_path.starts_with("sass/") && file.relative_path.ends_with(".scss")
            })
            .max_by_key(|file| file.contents.len())
            .expect("fixture-ul trebuie să conțină cel puțin un fișier SCSS");
        let mut source_texts = (*before_projection.source_texts).clone();
        source_texts.insert(
            target.relative_path.clone(),
            format!(
                "{}\n.performance-probe {{ color: red; }}\n",
                target.contents
            ),
        );
        let projection = WorkspaceProjectionSnapshot {
            project_root: before_projection.project_root.clone(),
            runtime_session_id: before_projection.runtime_session_id.clone(),
            revision: 2,
            workspace_transaction_id: Some("performance-css-2".to_string()),
            source_texts: source_texts.into(),
            resource_bytes: HashMap::new().into(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::from([target.relative_path.clone()]),
            accepted_disk: before_projection.accepted_disk.clone(),
        };
        let mut samples = Vec::with_capacity(sample_count);
        let mut last_report = None;
        for sample in 0..(warmup_count + sample_count) {
            let outcome = rebuild_project_model_after_workspace_change(
                &root,
                Some(&before),
                Some(1),
                &projection,
                std::slice::from_ref(&target.relative_path),
                ProjectModelIncrementalIntent::StyleDeclaration,
            )
            .unwrap();
            assert_eq!(outcome.report.mode, ProjectModelRebuildMode::Incremental);
            if sample >= warmup_count {
                samples.push(outcome.report.duration_us);
            }
            last_report = Some(outcome.report);
        }
        let report = last_report.unwrap();
        emit_performance_baseline(
            "css_edit",
            "project_model_incremental",
            &mut samples,
            Some(serde_json::json!({
                "projectModelCloneUs": report.model_clone_us,
                "projectModelBuildMode": report.mode.label(),
            })),
        );
    }

    fn performance_project_root() -> PathBuf {
        PathBuf::from(
            std::env::var("PANA_PERFORMANCE_BENCH_PROJECT")
                .or_else(|_| std::env::var("PANA_INCREMENTAL_BENCH_PROJECT"))
                .expect("PANA_PERFORMANCE_BENCH_PROJECT"),
        )
        .canonicalize()
        .unwrap()
    }

    fn performance_sample_configuration() -> (usize, usize) {
        let parse = |name: &str, default: usize| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(default)
        };
        let sample_count = parse("PANA_PERFORMANCE_SAMPLE_COUNT", 20).max(20);
        let warmup_count = parse("PANA_PERFORMANCE_WARMUP_COUNT", 3);
        (sample_count, warmup_count)
    }

    fn performance_disk_projection(root: &Path, revision: u64) -> WorkspaceProjectionSnapshot {
        let manifest = crate::project::read_project_disk_manifest(root).unwrap();
        assert!(
            !manifest.truncated,
            "fixture-ul de performanță depășește plafonul manifestului canonic"
        );
        let source_texts = manifest
            .files
            .iter()
            .filter_map(|entry| {
                fs::read_to_string(root.join(&entry.relative_path))
                    .ok()
                    .map(|source| (entry.relative_path.clone(), source))
            })
            .collect::<HashMap<_, _>>();
        let runtime_session_id = "performance-large-fixture".to_string();
        WorkspaceProjectionSnapshot {
            project_root: root.to_string_lossy().to_string(),
            runtime_session_id: runtime_session_id.clone(),
            revision,
            workspace_transaction_id: Some(format!("performance-{revision}")),
            source_texts: source_texts.into(),
            resource_bytes: HashMap::new().into(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::new(),
            accepted_disk: AcceptedProjectDiskManifest::new(
                runtime_session_id,
                root.to_string_lossy().to_string(),
                manifest,
            )
            .unwrap()
            .into(),
        }
    }

    fn emit_performance_baseline(
        operation: &str,
        variant: &str,
        samples: &mut [u64],
        extra: Option<serde_json::Value>,
    ) {
        samples.sort_unstable();
        assert!(samples.len() >= 20);
        let percentile =
            |percent: usize| samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)];
        let mut value = serde_json::json!({
            "schemaVersion": 1,
            "operation": operation,
            "variant": variant,
            "sampleCount": samples.len(),
            "samplesUs": samples,
            "p50Us": percentile(50),
            "p95Us": percentile(95),
            "maxUs": samples.last().copied().unwrap_or_default(),
        });
        if let (Some(target), Some(extra)) = (
            value.as_object_mut(),
            extra.and_then(|v| v.as_object().cloned()),
        ) {
            target.extend(extra);
        }
        eprintln!("[pana-performance] {value}");
    }

    fn performance_p95(samples: &mut [u64]) -> u64 {
        samples.sort_unstable();
        samples[(samples.len() * 95).div_ceil(100).saturating_sub(1)]
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

    fn rich_derived_sources() -> HashMap<String, String> {
        let mut sources = initial_sources();
        sources.insert(
            ".panastudio/project.toml".to_string(),
            "schema_version = 1\n".to_string(),
        );
        sources.insert(
            ".panastudio/assignments.toml".to_string(),
            "schema_version = 1\n\n[[assignments]]\nsectionPath = \"content/_index.md\"\nmodelId = \"service\"\n"
                .to_string(),
        );
        sources.insert(
            ".panastudio/content-models/service.toml".to_string(),
            "schemaVersion = 1\nid = \"service\"\nlabel = \"Serviciu\"\n\n[[fields]]\nid = \"field-title\"\nkey = \"title\"\nlabel = \"Titlu\"\nkind = \"text\"\n"
                .to_string(),
        );
        sources.insert(
            ".panastudio/listing-items.toml".to_string(),
            "schema_version = 1\n\n[[items]]\nid = \"service-card\"\nlabel = \"Card serviciu\"\ntemplateName = \"listing-items/service-card.html\"\nmodelId = \"service\"\npreviewPageFile = \"content/blog/post.md\"\n"
                .to_string(),
        );
        sources.insert(
            "templates/listing-items/service-card.html".to_string(),
            concat!(
                "{# pana:widget schema=2 provider=dynamic-field ",
                "instance=dynamic-field-rich01 props=00 #}",
                "<h2 data-pana-widget-instance=\"dynamic-field-rich01\">",
                "{{ item.extra.title }}</h2>",
                "{# /pana:widget instance=dynamic-field-rich01 #}",
            )
            .to_string(),
        );
        sources.insert("templates/index.html".to_string(), rich_index_source());
        sources
    }

    fn rich_index_source() -> String {
        initial_index_source().replace(
            "<main>",
            concat!(
                "<main>",
                "{% for item in [1, 2] %}<span>{{ item }}</span>{% endfor %}",
                "<span data-pana-block=\"counter\" data-pana-instance=\"counter-rich\">0</span>",
                "{{ page.content | safe }}{{ page.extra.title }}",
                "{# pana:widget schema=2 provider=dynamic-field ",
                "instance=dynamic-field-rich01 props=00 #}",
                "<h2 data-pana-widget-instance=\"dynamic-field-rich01\">",
                "{{ page.extra.title }}</h2>",
                "{# /pana:widget instance=dynamic-field-rich01 #}",
            ),
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
            source_texts: source_texts.into(),
            resource_bytes: HashMap::new().into(),
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
            .unwrap()
            .into(),
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
        assert_model_semantics_match(&outcome.model, &oracle);
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
