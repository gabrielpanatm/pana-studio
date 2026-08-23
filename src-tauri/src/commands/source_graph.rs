use serde::Serialize;
use tauri::State;

use super::kernel_preview_context::require_preview_command_identity;
use crate::{
    kernel::preview_projection::PreviewStructuralCommandIdentity,
    source_graph::{
        build_taxonomy_catalog, build_template_catalog_with_taxonomies, SourceGraph,
        TaxonomyCatalogSnapshot, TemplateCatalogSnapshot,
    },
    state::AppState,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceGraphProjectionReceipt {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub graph: SourceGraph,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCatalogProjectionReceipt {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub catalog: TemplateCatalogSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaxonomyCatalogProjectionReceipt {
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub catalog: TaxonomyCatalogSnapshot,
}

#[tauri::command(async)]
pub fn read_source_graph(
    identity: PreviewStructuralCommandIdentity,
    state: State<AppState>,
) -> Result<SourceGraphProjectionReceipt, String> {
    read_source_graph_from_accepted_project(&identity, &state)
}

#[tauri::command(async)]
pub fn read_template_catalog(
    identity: PreviewStructuralCommandIdentity,
    state: State<AppState>,
) -> Result<TemplateCatalogProjectionReceipt, String> {
    use crate::project_model::cache::{
        build_project_model_from_context, capture_project_model_build_context,
        publish_project_model_if_current,
    };

    let (root, session, context) = capture_project_model_build_context(&state)?;
    require_preview_command_identity(&session, &identity)?;
    let model = build_project_model_from_context(&root, &context)?;
    let graph = model.source_graph.clone();
    let taxonomy_catalog = ["zola.toml", "config.toml"].iter().find_map(|path| {
        context
            .projection()
            .source_texts
            .get(*path)
            .map(|source| build_taxonomy_catalog(&graph, path, source))
    });
    let catalog = build_template_catalog_with_taxonomies(&graph, taxonomy_catalog.as_ref());
    publish_project_model_if_current(&state, &context, model)?;
    Ok(TemplateCatalogProjectionReceipt {
        project_root: context.projection().project_root.clone(),
        runtime_session_id: context.projection().runtime_session_id.clone(),
        workspace_revision: context.projection().revision,
        catalog,
    })
}

#[tauri::command(async)]
pub fn read_taxonomy_catalog(
    identity: PreviewStructuralCommandIdentity,
    state: State<AppState>,
) -> Result<TaxonomyCatalogProjectionReceipt, String> {
    use crate::project_model::cache::{
        build_project_model_from_context, capture_project_model_build_context,
        publish_project_model_if_current,
    };

    let (root, session, context) = capture_project_model_build_context(&state)?;
    require_preview_command_identity(&session, &identity)?;
    let model = build_project_model_from_context(&root, &context)?;
    let graph = model.source_graph.clone();
    let (config_path, config_source) = ["zola.toml", "config.toml"]
        .iter()
        .find_map(|path| {
            context
                .projection()
                .source_texts
                .get(*path)
                .map(|source| ((*path).to_string(), source.as_str()))
        })
        .ok_or_else(|| {
            "Catalogul taxonomiilor cere un zola.toml sau config.toml urmărit de ProjectWorkspace."
                .to_string()
        })?;
    let catalog = build_taxonomy_catalog(&graph, &config_path, config_source);
    publish_project_model_if_current(&state, &context, model)?;
    Ok(TaxonomyCatalogProjectionReceipt {
        project_root: context.projection().project_root.clone(),
        runtime_session_id: context.projection().runtime_session_id.clone(),
        workspace_revision: context.projection().revision,
        catalog,
    })
}

pub(crate) fn read_source_graph_from_accepted_project(
    identity: &PreviewStructuralCommandIdentity,
    state: &State<AppState>,
) -> Result<SourceGraphProjectionReceipt, String> {
    use crate::project_model::cache::{
        build_project_model_from_context, capture_project_model_build_context,
        publish_project_model_if_current,
    };

    let (root, session, context) = capture_project_model_build_context(state)?;
    require_preview_command_identity(&session, identity)?;
    let cached_model = {
        let workspace = state.project_workspace.lock().map_err(|_| {
            "Nu am putut citi cache-ul ProjectModel pentru SourceGraph.".to_string()
        })?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace lipsește pentru SourceGraph.".to_string())?;
        if workspace.project_model_source_revision == Some(context.projection().revision) {
            workspace.project_model.clone()
        } else {
            None
        }
    };
    let graph = match cached_model {
        Some(model) => model.source_graph.clone(),
        None => {
            // Workspace mutations invalidate ProjectModel before every derived
            // frontend view. Preview normally republishes it first; activities
            // without a mounted Canvas still need an exact Rust projection.
            let model = build_project_model_from_context(&root, &context)?;
            let graph = model.source_graph.clone();
            publish_project_model_if_current(state, &context, model)?;
            graph
        }
    };
    Ok(SourceGraphProjectionReceipt {
        project_root: context.projection().project_root.clone(),
        runtime_session_id: context.projection().runtime_session_id.clone(),
        workspace_revision: context.projection().revision,
        graph,
    })
}
