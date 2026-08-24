use serde::Serialize;
use tauri::State;

use crate::{
    blocks::{
        inspect_native_block_slots, inspect_native_block_source, inspect_native_icon_source,
        IconCatalogPage, IconCatalogSearchInput, IconCatalogSummary, NativeBlockOptionState,
        NativeBlockSlotState, NativeIconState,
    },
    commands::workspace_entries::require_bound_workspace,
    kernel::file_buffer_store::FileBufferRequestIdentity,
    localization::LocalizedDiagnostic,
    project_model::cache::{
        build_project_model_from_context, capture_project_model_build_context,
        publish_project_model_if_current,
    },
    project_model::move_engine::{parse_html_tag_at, ProjectSourceEditLocation},
    source_graph::model::{
        BlockDefinition, BlockResolutionStatus, RenderedBlockInstance, SourceNodeKind,
    },
    state::AppState,
};

#[tauri::command]
pub fn read_icon_catalog() -> Result<IconCatalogSummary, String> {
    crate::blocks::read_icon_catalog()
}

#[tauri::command]
pub fn search_icon_catalog(input: IconCatalogSearchInput) -> Result<IconCatalogPage, String> {
    crate::blocks::search_icon_catalog(input)
}

#[derive(Debug)]
struct BlockRuntimeSnapshot {
    preview_revision: Option<String>,
    available: bool,
    instances: Vec<crate::source_graph::model::RenderedBlockInstance>,
    diagnostics: Vec<LocalizedDiagnostic>,
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
    pub editable: bool,
    pub diagnostic: Option<LocalizedDiagnostic>,
    pub options: Vec<NativeBlockOptionState>,
    pub slots: Vec<NativeBlockSlotState>,
    pub icon: Option<NativeIconState>,
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
    pub diagnostics: Vec<LocalizedDiagnostic>,
}

#[tauri::command]
pub fn read_ui_block_graph(
    identity: FileBufferRequestIdentity,
    state: State<AppState>,
) -> Result<UiBlockGraphSnapshot, String> {
    let (bound_root, project_root, runtime_session_id, workspace_revision) = {
        let (root, workspace) = require_bound_workspace(state.inner(), &identity)?;
        let workspace = workspace
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru blocuri.".to_string())?;
        (
            root,
            workspace.session.project_root.clone(),
            workspace.runtime_session_id(),
            workspace.revision,
        )
    };
    let (root, session, context) = capture_project_model_build_context(&state)?;
    if root != bound_root
        || session.runtime_instance_id() != runtime_session_id
        || context.projection().revision != workspace_revision
    {
        return Err("BlockGraph a refuzat un ProjectModel stale.".to_string());
    }
    let model = build_project_model_from_context(&root, &context)?;
    publish_project_model_if_current(&state, &context, model.clone())?;
    let source_graph = &model.source_graph;
    let source_instances = source_graph
        .block_graph
        .source_instances
        .iter()
        .map(|instance| {
            let marker = source_graph.node_by_id(&instance.source_node_id);
            let root_node = marker
                .and_then(|node| node.parent.as_deref())
                .and_then(|parent| source_graph.node_by_id(parent))
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
                    file.contents.get(opening.start..opening.end).map(|source| {
                        Ok::<_, String>((
                            inspect_native_block_source(source)?,
                            inspect_native_icon_source(source)?,
                        ))
                    })
                })
                .transpose();
            let (mut editable, mut diagnostic, options, icon) = match inspection {
                Ok(Some((inspection, icon))) => (
                    inspection.editable,
                    inspection.diagnostic.map(|details| {
                        LocalizedDiagnostic::new("blocks-diagnostic-contract-invalid")
                            .with_argument("details", details)
                    }),
                    inspection.options,
                    icon,
                ),
                Ok(None) => (
                    false,
                    Some(LocalizedDiagnostic::new(
                        "blocks-diagnostic-source-root-missing",
                    )),
                    Vec::new(),
                    None,
                ),
                Err(details) => (
                    false,
                    Some(
                        LocalizedDiagnostic::new("blocks-diagnostic-inspection-failed")
                            .with_argument("details", details),
                    ),
                    Vec::new(),
                    None,
                ),
            };
            let slots = root_node
                .map(|root| inspect_native_block_slots(&model, root, &instance.provider_id))
                .unwrap_or_default();
            if let Some(details) = slots
                .iter()
                .find(|slot| !slot.editable && instance.provider_id == "slider")
                .and_then(|slot| slot.diagnostic.clone())
            {
                editable = false;
                if diagnostic.is_none() {
                    diagnostic = Some(
                        LocalizedDiagnostic::new("blocks-diagnostic-contract-invalid")
                            .with_argument("details", details),
                    );
                }
            }
            if root_node.is_some_and(|node| !node.capabilities.can_edit_attributes) {
                editable = false;
            }
            let status = ui_block_resolution_status(&instance.status, diagnostic.is_some());
            UiBlockSourceInstance {
                id: instance.id.clone(),
                definition_id: instance.definition_id.clone(),
                provider_id: instance.provider_id.clone(),
                file: instance.file.clone(),
                marker_source_node_id: instance.source_node_id.clone(),
                root_source_node_id: root_node.map(|node| node.id.clone()),
                root_location,
                status,
                editable,
                diagnostic,
                options,
                slots,
                icon,
            }
        })
        .collect::<Vec<_>>();
    let runtime = block_runtime_snapshot(identity, state.inner())?;
    let mut diagnostics = source_graph
        .block_graph
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.diagnostic.clone())
        .collect::<Vec<_>>();
    diagnostics.extend(runtime.diagnostics.iter().cloned());

    Ok(UiBlockGraphSnapshot {
        schema_version: 4,
        project_root,
        runtime_session_id,
        workspace_revision,
        model_revision: model.revision.clone(),
        preview_revision: runtime.preview_revision,
        canvas_available: runtime.available,
        definitions: source_graph.block_graph.definitions.clone(),
        source_instances,
        rendered_instances: runtime.instances,
        diagnostics,
    })
}

fn ui_block_resolution_status(
    source_status: &BlockResolutionStatus,
    has_diagnostic: bool,
) -> BlockResolutionStatus {
    if source_status == &BlockResolutionStatus::UnknownProvider {
        BlockResolutionStatus::UnknownProvider
    } else if has_diagnostic {
        BlockResolutionStatus::InvalidContract
    } else {
        BlockResolutionStatus::Resolved
    }
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
            None,
            LocalizedDiagnostic::new("blocks-runtime-not-rendered"),
        ));
    };
    let Some(generation) = engine.active_generation()? else {
        return Ok(unavailable_runtime_snapshot(
            None,
            LocalizedDiagnostic::new("blocks-runtime-no-active-generation"),
        ));
    };
    if !generation.owner_matches(&project_root, &runtime_session_id)
        || generation.workspace_revision != workspace_revision
    {
        return Ok(unavailable_runtime_snapshot(
            Some(generation.preview_revision.clone()),
            LocalizedDiagnostic::new("blocks-runtime-workspace-revision-mismatch")
                .with_argument("canvasRevision", generation.workspace_revision)
                .with_argument("workspaceRevision", workspace_revision),
        ));
    }
    Ok(BlockRuntimeSnapshot {
        preview_revision: Some(generation.preview_revision.clone()),
        available: true,
        instances: generation.canvas_transaction.graph.block_instances.clone(),
        diagnostics: generation
            .canvas_transaction
            .graph
            .diagnostics
            .iter()
            .map(|diagnostic| {
                LocalizedDiagnostic::new("blocks-canvas-diagnostic")
                    .with_argument("code", diagnostic.code.clone())
                    .with_argument("details", diagnostic.message.clone())
            })
            .collect(),
    })
}

fn unavailable_runtime_snapshot(
    preview_revision: Option<String>,
    diagnostic: LocalizedDiagnostic,
) -> BlockRuntimeSnapshot {
    BlockRuntimeSnapshot {
        preview_revision,
        available: false,
        instances: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_block_status_preserves_unknown_provider_and_rejects_invalid_contracts() {
        assert_eq!(
            ui_block_resolution_status(&BlockResolutionStatus::UnknownProvider, false),
            BlockResolutionStatus::UnknownProvider
        );
        assert_eq!(
            ui_block_resolution_status(&BlockResolutionStatus::Resolved, true),
            BlockResolutionStatus::InvalidContract
        );
        assert_eq!(
            ui_block_resolution_status(&BlockResolutionStatus::Resolved, false),
            BlockResolutionStatus::Resolved
        );
    }
}
