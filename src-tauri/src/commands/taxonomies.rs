use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    commands::workspace_entries::{
        require_bound_workspace, WorkspaceEntryMutationReceipt,
        WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
    },
    kernel::{
        file_buffer_store::FileBufferRequestIdentity,
        observability::now_ms,
        project_workspace::{commit_project_workspace_session_mutation, ProjectWorkspace},
        taxonomy_mutation::{
            plan_taxonomy_mutation as build_mutation_plan, stage_taxonomy_mutation,
            PlannedTaxonomyMutation, TaxonomyMutationInput, TaxonomyMutationPlan,
        },
    },
    source_graph::{
        build_source_graph_from_workspace_projection, build_taxonomy_catalog,
        TaxonomyCatalogSnapshot,
    },
    state::AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyMutationApplyReceipt {
    pub plan: TaxonomyMutationPlan,
    pub workspace: WorkspaceEntryMutationReceipt,
}

#[tauri::command(async)]
pub fn plan_taxonomy_mutation(
    input: TaxonomyMutationInput,
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<TaxonomyMutationPlan, String> {
    let (root, slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru taxonomii.".to_string())?;
    Ok(plan_for_workspace(&root, workspace, &input)?.plan)
}

#[tauri::command(async)]
pub fn apply_taxonomy_mutation(
    input: TaxonomyMutationInput,
    expected_plan_id: String,
    identity: FileBufferRequestIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<TaxonomyMutationApplyReceipt, String> {
    let (root, mut slot) = require_bound_workspace(state.inner(), &identity)?;
    let workspace = slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru taxonomii.".to_string())?;
    let planned = plan_for_workspace(&root, workspace, &input)?;
    if planned.plan.plan_id != expected_plan_id {
        return Err(format!(
            "Planul taxonomiei este stale: UI a confirmat {}, Rust a recalculat {}.",
            expected_plan_id, planned.plan.plan_id
        ));
    }
    let (plan, mutation) =
        commit_project_workspace_session_mutation(&app, workspace, |candidate| {
            stage_taxonomy_mutation(candidate, planned, now_ms())
        })?;
    let relative_path = plan.touched_files.first().cloned();
    let workspace_receipt = WorkspaceEntryMutationReceipt {
        schema_version: WORKSPACE_ENTRY_MUTATION_SCHEMA_VERSION,
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        relative_path,
        mutation,
        workspace: workspace.snapshot(),
    };
    Ok(TaxonomyMutationApplyReceipt {
        plan,
        workspace: workspace_receipt,
    })
}

fn plan_for_workspace(
    root: &std::path::Path,
    workspace: &ProjectWorkspace,
    input: &TaxonomyMutationInput,
) -> Result<PlannedTaxonomyMutation, String> {
    let projection = workspace.capture_projection_snapshot()?;
    let graph = build_source_graph_from_workspace_projection(root, &projection)?;
    let catalog = catalog_for_projection(&graph, &projection.source_texts)?;
    build_mutation_plan(&graph, &catalog, &projection.source_texts, input)
}

fn catalog_for_projection(
    graph: &crate::source_graph::SourceGraph,
    source_texts: &std::collections::HashMap<String, String>,
) -> Result<TaxonomyCatalogSnapshot, String> {
    let (config_path, config_source) = ["zola.toml", "config.toml"]
        .iter()
        .find_map(|path| {
            source_texts
                .get(*path)
                .map(|source| ((*path).to_string(), source.as_str()))
        })
        .ok_or_else(|| {
            "Taxonomiile cer un zola.toml sau config.toml urmărit de ProjectWorkspace.".to_string()
        })?;
    Ok(build_taxonomy_catalog(graph, &config_path, config_source))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::{
        kernel::project_workspace::WorkspaceProjectionSnapshot,
        project::{read_project_disk_manifest, AcceptedProjectDiskManifest},
        source_graph::build_source_graph_from_workspace_projection,
    };

    fn fixture_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pana-taxonomy-command-{name}-{nonce}"))
    }

    #[test]
    fn rename_plan_updates_config_and_all_affected_frontmatter_atomically() {
        let root = fixture_root("rename");
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(
            root.join("zola.toml"),
            "base_url = \"https://example.com\"\ntaxonomies = [{ name = \"tags\" }]\n",
        )
        .unwrap();
        fs::write(
            root.join("content/a.md"),
            "+++\ntitle = \"A\"\n[taxonomies]\ntags = [\"Rust\"]\n+++\n",
        )
        .unwrap();
        fs::write(
            root.join("content/b.md"),
            "---\ntitle: B\ntaxonomies:\n  tags: [Web]\n---\n",
        )
        .unwrap();
        let source_texts = ["zola.toml", "content/a.md", "content/b.md"]
            .into_iter()
            .map(|path| {
                (
                    path.to_string(),
                    fs::read_to_string(root.join(path)).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();
        let disk = read_project_disk_manifest(&root).unwrap();
        let projection = WorkspaceProjectionSnapshot {
            project_root: root.canonicalize().unwrap().to_string_lossy().into_owned(),
            runtime_session_id: "test".to_string(),
            revision: 0,
            workspace_transaction_id: None,
            source_texts: source_texts.clone(),
            resource_bytes: HashMap::new(),
            deleted_sources: HashSet::new(),
            changed_paths: HashSet::new(),
            accepted_disk: AcceptedProjectDiskManifest::new(
                "test",
                root.canonicalize().unwrap().to_string_lossy().into_owned(),
                disk,
            )
            .unwrap(),
        };
        let graph = build_source_graph_from_workspace_projection(&root, &projection).unwrap();
        let catalog = catalog_for_projection(&graph, &source_texts).unwrap();
        let planned = build_mutation_plan(
            &graph,
            &catalog,
            &source_texts,
            &TaxonomyMutationInput {
                operation:
                    crate::kernel::taxonomy_mutation::TaxonomyMutationOperation::UpsertDefinition {
                        original_name: Some("tags".to_string()),
                        original_language: Some("en".to_string()),
                        definition: crate::kernel::taxonomy_mutation::TaxonomyDefinitionInput {
                            name: "topics".to_string(),
                            language: "en".to_string(),
                            render: true,
                            feed: false,
                            paginate_by: None,
                            paginate_path: None,
                        },
                    },
            },
        )
        .unwrap();
        assert_eq!(planned.plan.affected_pages.len(), 2);
        assert_eq!(planned.changes.len(), 3);
        assert!(planned
            .changes
            .iter()
            .find(|change| change.relative_path == "zola.toml")
            .unwrap()
            .contents
            .contains("name = \"topics\""));
        assert!(planned
            .changes
            .iter()
            .filter(|change| change.relative_path.starts_with("content/"))
            .all(|change| change.contents.contains("topics")));
        let _ = fs::remove_dir_all(root);
    }
}
