use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    blocks::{
        inspect_native_block_source, native_block_registry_snapshot,
        plan_native_block_contract as plan_contract, NativeBlockContractPlan,
        NativeBlockContractRequest, NativeBlockMarkerKind, NativeBlockOptionState,
        NativeBlockRegistrySnapshot,
    },
    commands::page_contracts::{
        apply_authoritative_page_contract, PageContractAuthorityReceipt, PageContractMutationPlan,
    },
    commands::project::require_current_project_root,
    commands::workspace_entries::require_bound_workspace,
    js::PageJsDraftStageReceipt,
    kernel::file_buffer_store::FileBufferRequestIdentity,
    kernel::project_workspace::ProjectWorkspaceMutationReceipt,
    project::strip_zola_root_prefix,
    project_model::move_engine::{parse_html_tag_at, ProjectSourceEditLocation},
    source_graph::model::{
        BlockDefinition, BlockDiagnostic, BlockResolutionStatus, RenderedBlockInstance,
        SourceNodeKind,
    },
    state::AppState,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockContractApplyReceipt {
    pub plan: NativeBlockContractPlan,
    pub workspace_mutation: Option<ProjectWorkspaceMutationReceipt>,
    pub page_js: Option<PageJsDraftStageReceipt>,
    pub authority: PageContractAuthorityReceipt,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeBlockContractApplyInput {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub template_path: String,
    pub ensure_block_id: Option<String>,
    pub cachebust_assets: Option<bool>,
}

#[tauri::command]
pub fn read_native_block_registry() -> NativeBlockRegistrySnapshot {
    native_block_registry_snapshot()
}

#[tauri::command]
pub fn plan_native_block_contract(
    mut input: NativeBlockContractRequest,
    state: State<AppState>,
) -> Result<NativeBlockContractPlan, String> {
    let _ = require_current_project_root(&state)?;
    input.template_path = strip_zola_root_prefix(&input.template_path).to_string();
    Ok(plan_contract(input))
}

#[tauri::command(async)]
pub fn apply_native_block_contract(
    input: NativeBlockContractApplyInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<NativeBlockContractApplyReceipt, String> {
    apply_native_block_contract_impl(input, &app, &state)
}

pub(crate) fn apply_native_block_contract_impl(
    input: NativeBlockContractApplyInput,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<NativeBlockContractApplyReceipt, String> {
    let cachebust_assets = input.cachebust_assets.unwrap_or(false);
    let expected_project_root = input.expected_project_root;
    let expected_session_id = input.expected_session_id;
    let template_path = strip_zola_root_prefix(&input.template_path).to_string();
    let ensure_block_id = input.ensure_block_id;
    let applied = apply_authoritative_page_contract(
        app,
        state,
        "Apply Native Block contract",
        &expected_project_root,
        &expected_session_id,
        &template_path,
        cachebust_assets,
        |sources| {
            plan_contract(NativeBlockContractRequest {
                template_path: sources.template_path.clone(),
                template_source: sources.template_source.clone(),
                stylesheet_source: Some(sources.stylesheet_source.clone()),
                page_js_config: Some(sources.page_js_config.clone()),
                ensure_block_id,
                cachebust_assets: Some(cachebust_assets),
            })
        },
        |plan| PageContractMutationPlan {
            template_changed: plan.template.changed,
            template_contents: plan.template.contents.clone(),
            stylesheet_changed: plan.stylesheet.changed,
            stylesheet_contents: plan.stylesheet.contents.clone(),
            page_js_changed: plan.page_js_changed,
            page_js_config: plan.page_js_config.clone(),
        },
    )?;

    Ok(NativeBlockContractApplyReceipt {
        plan: applied.plan,
        workspace_mutation: applied.workspace_mutation,
        page_js: applied.page_js,
        authority: applied.authority,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRuntimeSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub preview_revision: Option<String>,
    pub available: bool,
    pub instances: Vec<crate::source_graph::model::RenderedBlockInstance>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiBlockSourceInstance {
    pub id: String,
    pub definition_id: Option<String>,
    pub provider_id: String,
    pub file: String,
    pub marker_source_node_id: String,
    pub root_source_node_id: Option<String>,
    pub root_location: Option<ProjectSourceEditLocation>,
    pub status: BlockResolutionStatus,
    pub marker_kind: Option<NativeBlockMarkerKind>,
    pub editable: bool,
    pub diagnostic: Option<String>,
    pub options: Vec<NativeBlockOptionState>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiBlockGraphSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub model_revision: String,
    pub preview_revision: Option<String>,
    pub canvas_available: bool,
    pub definitions: Vec<BlockDefinition>,
    pub source_instances: Vec<UiBlockSourceInstance>,
    pub rendered_instances: Vec<RenderedBlockInstance>,
    pub diagnostics: Vec<String>,
}

#[tauri::command]
pub fn read_ui_block_graph(
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<UiBlockGraphSnapshot, String> {
    let (root, projection, project_root, runtime_session_id, workspace_revision) = {
        let (root, workspace) = require_bound_workspace(state.inner(), &identity)?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru blocuri.".to_string())?;
        (
            root,
            workspace.capture_projection_lease()?,
            workspace.session.project_root.clone(),
            workspace.runtime_session_id(),
            workspace.revision,
        )
    };
    let model =
        crate::project_model::build_project_model_from_workspace_projection(&root, &projection)?;
    let source_graph = &model.source_graph;
    let source_instances = source_graph
        .block_graph
        .source_instances
        .iter()
        .map(|instance| {
            let marker = source_graph
                .nodes
                .iter()
                .find(|node| node.id == instance.source_node_id);
            let root_node = marker
                .and_then(|node| node.parent.as_deref())
                .and_then(|parent| source_graph.nodes.iter().find(|node| node.id == parent))
                .filter(|node| node.kind == SourceNodeKind::Html);
            let root_location = root_node.and_then(|node| {
                node.range.as_ref().map(|range| ProjectSourceEditLocation {
                    file: node.file.clone(),
                    line: range.line,
                    column: range.column,
                })
            });
            let inspection = root_node
                .and_then(|node| {
                    let file = model
                        .files
                        .iter()
                        .find(|file| file.relative_path == node.file)?;
                    let range = node.range.as_ref()?;
                    let opening = parse_html_tag_at(&file.contents, range.start)?;
                    file.contents
                        .get(opening.start..opening.end)
                        .map(inspect_native_block_source)
                })
                .transpose();
            let (marker_kind, mut editable, diagnostic, options) = match inspection {
                Ok(Some(inspection)) => (
                    Some(inspection.marker_kind),
                    inspection.editable,
                    inspection.diagnostic,
                    inspection.options,
                ),
                Ok(None) => (
                    None,
                    false,
                    Some("Rădăcina blocului nu mai poate fi localizată în sursă.".to_string()),
                    Vec::new(),
                ),
                Err(error) => (None, false, Some(error), Vec::new()),
            };
            if root_node.is_some_and(|node| !node.capabilities.can_edit_attributes) {
                editable = false;
            }
            let status = if instance.status == BlockResolutionStatus::UnknownProvider {
                BlockResolutionStatus::UnknownProvider
            } else if marker_kind == Some(NativeBlockMarkerKind::Canonical) && diagnostic.is_some()
            {
                BlockResolutionStatus::InvalidContract
            } else if marker_kind.is_none() {
                BlockResolutionStatus::InvalidContract
            } else {
                BlockResolutionStatus::Resolved
            };
            UiBlockSourceInstance {
                id: instance.id.clone(),
                definition_id: instance.definition_id.clone(),
                provider_id: instance.provider_id.clone(),
                file: instance.file.clone(),
                marker_source_node_id: instance.source_node_id.clone(),
                root_source_node_id: root_node.map(|node| node.id.clone()),
                root_location,
                status,
                marker_kind,
                editable,
                diagnostic,
                options,
            }
        })
        .collect::<Vec<_>>();
    let runtime = block_runtime_snapshot(identity, state.inner())?;
    let mut diagnostics = source_graph
        .block_graph
        .diagnostics
        .iter()
        .map(block_diagnostic_message)
        .collect::<Vec<_>>();
    diagnostics.extend(runtime.diagnostics.iter().cloned());

    Ok(UiBlockGraphSnapshot {
        schema_version: 1,
        project_root,
        runtime_session_id,
        workspace_revision,
        model_revision: model.revision,
        preview_revision: runtime.preview_revision,
        canvas_available: runtime.available,
        definitions: source_graph.block_graph.definitions.clone(),
        source_instances,
        rendered_instances: runtime.instances,
        diagnostics,
    })
}

#[tauri::command]
pub fn read_block_runtime_snapshot(
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<BlockRuntimeSnapshot, String> {
    block_runtime_snapshot(identity, state.inner())
}

fn block_runtime_snapshot(
    identity: FileBufferRequestIdentity,
    state: &AppState,
) -> Result<BlockRuntimeSnapshot, String> {
    let (project_root, runtime_session_id, workspace_revision) = {
        let (_root, mut slot) = require_bound_workspace(state, &identity)?;
        let workspace = slot
            .as_mut()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru blocuri.".to_string())?;
        (
            workspace.session.project_root.clone(),
            workspace.runtime_session_id(),
            workspace.revision,
        )
    };
    let engine = state
        .preview_engine
        .lock()
        .map_err(|_| "Motorul Preview embedded este indisponibil.".to_string())?;
    let Some(engine) = engine.as_ref() else {
        return Ok(unavailable_runtime_snapshot(
            project_root,
            runtime_session_id,
            workspace_revision,
            None,
            "Preview-ul nu are încă o generație randată.",
        ));
    };
    let Some(generation) = engine.active_generation()? else {
        return Ok(unavailable_runtime_snapshot(
            project_root,
            runtime_session_id,
            workspace_revision,
            None,
            "Preview-ul nu are încă o generație activă.",
        ));
    };
    if !generation.owner_matches(&project_root, &runtime_session_id)
        || generation.workspace_revision != workspace_revision
    {
        return Ok(unavailable_runtime_snapshot(
            project_root,
            runtime_session_id,
            workspace_revision,
            Some(generation.preview_revision.clone()),
            &format!(
                "CanvasGraph aparține reviziei {}, nu reviziei ProjectWorkspace {}.",
                generation.workspace_revision, workspace_revision
            ),
        ));
    }
    Ok(BlockRuntimeSnapshot {
        schema_version: 1,
        project_root,
        runtime_session_id,
        workspace_revision,
        preview_revision: Some(generation.preview_revision.clone()),
        available: true,
        instances: generation.canvas_transaction.graph.block_instances.clone(),
        diagnostics: generation
            .canvas_transaction
            .graph
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    })
}

fn block_diagnostic_message(diagnostic: &BlockDiagnostic) -> String {
    match diagnostic.file.as_deref() {
        Some(file) => format!("{file}: {}", diagnostic.message),
        None => diagnostic.message.clone(),
    }
}

fn unavailable_runtime_snapshot(
    project_root: String,
    runtime_session_id: String,
    workspace_revision: u64,
    preview_revision: Option<String>,
    diagnostic: &str,
) -> BlockRuntimeSnapshot {
    BlockRuntimeSnapshot {
        schema_version: 1,
        project_root,
        runtime_session_id,
        workspace_revision,
        preview_revision,
        available: false,
        instances: Vec::new(),
        diagnostics: vec![diagnostic.to_string()],
    }
}
