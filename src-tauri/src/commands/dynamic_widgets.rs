use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    commands::workspace_entries::{
        current_workspace_identity, finish_mutation, mutation_metadata, require_bound_workspace,
        WorkspaceEntryMutationReceipt,
    },
    kernel::{
        dynamic_widgets::{
            replace_dynamic_widget_source, validate_dynamic_widget_source_context,
            DynamicWidgetProperties, DynamicWidgetSourceInstance, RenderedDynamicWidgetInstance,
        },
        file_buffer_store::FileBufferRequestIdentity,
        observability::now_ms,
        project_workspace::WorkspaceResourceMutation,
    },
    project_model::cache::{
        build_project_model_from_context, capture_project_model_build_context,
        publish_project_model_if_current,
    },
    state::AppState,
};

pub const DYNAMIC_WIDGET_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWidgetSnapshotRequest {
    pub identity: FileBufferRequestIdentity,
    pub expected_workspace_revision: u64,
    pub expected_model_revision: String,
    pub preview_revision: String,
    pub source_instance_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicWidgetSnapshot {
    pub schema_version: u32,
    pub project_root: String,
    pub runtime_session_id: String,
    pub workspace_revision: u64,
    pub model_revision: String,
    pub preview_revision: String,
    pub source_instance: DynamicWidgetSourceInstance,
    pub rendered_instances: Vec<RenderedDynamicWidgetInstance>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDynamicWidgetInput {
    pub request: DynamicWidgetSnapshotRequest,
    pub expected_source_revision: String,
    pub properties: DynamicWidgetProperties,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteDynamicWidgetInput {
    pub request: DynamicWidgetSnapshotRequest,
    pub expected_source_revision: String,
}

#[tauri::command]
pub fn read_dynamic_widget_snapshot(
    request: DynamicWidgetSnapshotRequest,
    state: State<AppState>,
) -> Result<DynamicWidgetSnapshot, String> {
    let context = resolve_source_context(&request, state.inner())?;
    let rendered_instances = require_preview_instances(&request, state.inner())?;
    Ok(DynamicWidgetSnapshot {
        schema_version: DYNAMIC_WIDGET_SNAPSHOT_SCHEMA_VERSION,
        project_root: context.project_root,
        runtime_session_id: context.runtime_session_id,
        workspace_revision: context.workspace_revision,
        model_revision: context.model_revision,
        preview_revision: request.preview_revision,
        source_instance: context.source_instance,
        rendered_instances,
    })
}

#[tauri::command(async)]
pub fn update_dynamic_widget(
    input: UpdateDynamicWidgetInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let context = resolve_source_context(&input.request, state.inner())?;
    require_source_revision(&context.source_instance, &input.expected_source_revision)?;
    require_preview_instances(&input.request, state.inner())?;
    if input.properties.provider_kind().id() != context.source_instance.provider_id {
        return Err(format!(
            "[dynamic_widget_provider_mismatch] Instanța {} aparține providerului {}, nu {}.",
            context.source_instance.instance_id,
            context.source_instance.provider_id,
            input.properties.provider_kind().id()
        ));
    }
    if let DynamicWidgetProperties::DynamicField(field) = &input.properties {
        validate_dynamic_widget_source_context(
            &context.source_instance.file,
            field,
            &context.source_graph,
        )?;
    }
    let next_source = replace_dynamic_widget_source(
        &context.source,
        &context.source_instance,
        &input.properties,
        &context.source_graph,
    )?;
    commit_source_rewrite(
        &input.request,
        &context.source_instance.file,
        next_source,
        "Actualizare widget dinamic",
        "dynamic-widgets.update",
        &app,
        state.inner(),
    )
}

#[tauri::command(async)]
pub fn delete_dynamic_widget(
    input: DeleteDynamicWidgetInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let context = resolve_source_context(&input.request, state.inner())?;
    require_source_revision(&context.source_instance, &input.expected_source_revision)?;
    require_preview_instances(&input.request, state.inner())?;
    let next_source = remove_dynamic_widget_source(&context.source, &context.source_instance)?;
    commit_source_rewrite(
        &input.request,
        &context.source_instance.file,
        next_source,
        "Ștergere widget dinamic",
        "dynamic-widgets.delete",
        &app,
        state.inner(),
    )
}

struct DynamicWidgetSourceContext {
    project_root: String,
    runtime_session_id: String,
    workspace_revision: u64,
    model_revision: String,
    source: String,
    source_instance: DynamicWidgetSourceInstance,
    source_graph: crate::source_graph::SourceGraph,
}

fn resolve_source_context(
    request: &DynamicWidgetSnapshotRequest,
    state: &AppState,
) -> Result<DynamicWidgetSourceContext, String> {
    let (bound_root, project_root, runtime_session_id, workspace_revision) = {
        let (root, slot) = require_bound_workspace(state, &request.identity)?;
        let workspace = slot.as_ref().ok_or_else(|| {
            "ProjectWorkspace nu este inițializat pentru widgeturile dinamice.".to_string()
        })?;
        require_workspace_revision(request.expected_workspace_revision, workspace.revision)?;
        (
            root,
            workspace.session.project_root.clone(),
            workspace.runtime_session_id(),
            workspace.revision,
        )
    };
    let (root, session, context) = capture_project_model_build_context(state)?;
    if root != bound_root
        || session.runtime_instance_id() != runtime_session_id
        || context.projection().revision != workspace_revision
    {
        return Err("[dynamic_widget_stale_model] ProjectModel a devenit stale.".to_string());
    }
    let model = build_project_model_from_context(&root, &context)?;
    publish_project_model_if_current(state, &context, model.clone())?;
    if model.revision != request.expected_model_revision {
        return Err(format!(
            "[dynamic_widget_stale_model] Inspectorul aștepta ProjectModel {}, dar Rust a proiectat {}.",
            request.expected_model_revision, model.revision
        ));
    }
    let source_instance = model
        .source_graph
        .dynamic_widget_graph
        .source_instances
        .iter()
        .find(|instance| instance.id == request.source_instance_id)
        .cloned()
        .ok_or_else(|| {
            format!(
                "[dynamic_widget_source_missing] Instanța sursă {} nu mai există.",
                request.source_instance_id
            )
        })?;
    let source = context
        .projection()
        .source_texts
        .get(&source_instance.file)
        .cloned()
        .ok_or_else(|| {
            format!(
                "[dynamic_widget_source_file_missing] Sursa {} nu există în proiecția ProjectWorkspace.",
                source_instance.file
            )
        })?;

    // Model construction is intentionally outside the workspace lock. Rebind
    // before publishing the snapshot so an obsolete projection cannot escape.
    {
        let (_root, slot) = require_bound_workspace(state, &request.identity)?;
        let workspace = slot.as_ref().ok_or_else(|| {
            "ProjectWorkspace a fost închis în timpul rezolvării widgetului dinamic.".to_string()
        })?;
        require_workspace_revision(workspace_revision, workspace.revision)?;
    }

    Ok(DynamicWidgetSourceContext {
        project_root,
        runtime_session_id,
        workspace_revision,
        model_revision: model.revision,
        source,
        source_instance,
        source_graph: model.source_graph,
    })
}

fn require_preview_instances(
    request: &DynamicWidgetSnapshotRequest,
    state: &AppState,
) -> Result<Vec<RenderedDynamicWidgetInstance>, String> {
    if request.preview_revision.trim().is_empty() {
        return Err(
            "[dynamic_widget_preview_identity_invalid] previewRevision este obligatorie."
                .to_string(),
        );
    }
    let engine = state
        .preview_engine
        .lock()
        .map_err(|_| "Motorul Preview embedded este indisponibil.".to_string())?;
    let generation = engine
        .as_ref()
        .ok_or_else(|| {
            "[dynamic_widget_preview_missing] Preview-ul embedded nu este pornit.".to_string()
        })?
        .active_generation()?
        .ok_or_else(|| {
            "[dynamic_widget_preview_missing] Preview-ul nu are o generație activă.".to_string()
        })?;
    if !generation.owner_matches(
        &request.identity.expected_project_root,
        &request.identity.expected_session_id,
    ) || generation.workspace_revision != request.expected_workspace_revision
        || generation.preview_revision != request.preview_revision
    {
        return Err(format!(
            "[dynamic_widget_stale_preview] Inspectorul aștepta workspace/preview {}/{}, dar Canvas-ul activ este la {}/{}.",
            request.expected_workspace_revision,
            request.preview_revision,
            generation.workspace_revision,
            generation.preview_revision
        ));
    }
    Ok(generation
        .canvas_transaction
        .graph
        .dynamic_widget_instances
        .iter()
        .filter(|instance| instance.source_instance_id == request.source_instance_id)
        .cloned()
        .collect())
}

fn require_workspace_revision(expected: u64, actual: u64) -> Result<(), String> {
    if expected != actual {
        return Err(format!(
            "[dynamic_widget_stale_workspace] Inspectorul aștepta ProjectWorkspace {expected}, dar revizia activă este {actual}."
        ));
    }
    Ok(())
}

fn require_source_revision(
    instance: &DynamicWidgetSourceInstance,
    expected: &str,
) -> Result<(), String> {
    if expected.trim().is_empty() || instance.source_revision != expected {
        return Err(format!(
            "[dynamic_widget_stale_source] Inspectorul aștepta sursa {}, dar instanța {} este la {}.",
            expected, instance.instance_id, instance.source_revision
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit_source_rewrite(
    request: &DynamicWidgetSnapshotRequest,
    relative_path: &str,
    contents: String,
    label: &str,
    source: &str,
    app: &AppHandle,
    state: &AppState,
) -> Result<WorkspaceEntryMutationReceipt, String> {
    let (_root, mut slot) = require_bound_workspace(state, &request.identity)?;
    let workspace = slot.as_mut().ok_or_else(|| {
        "ProjectWorkspace nu este inițializat pentru widgeturile dinamice.".to_string()
    })?;
    require_workspace_revision(request.expected_workspace_revision, workspace.revision)?;
    let receipt_path = relative_path.to_string();
    let mutation_path = receipt_path.clone();
    finish_mutation(app, workspace, Some(receipt_path), |candidate| {
        candidate.stage_resource_texts(
            &current_workspace_identity(candidate),
            mutation_metadata(label, source),
            vec![WorkspaceResourceMutation {
                relative_path: mutation_path,
                contents,
                create_only: false,
            }],
            now_ms(),
        )
    })
}

fn remove_dynamic_widget_source(
    source: &str,
    instance: &DynamicWidgetSourceInstance,
) -> Result<String, String> {
    let mut start = instance.range.start;
    let mut end = instance.range.end;
    if start > end || end > source.len() {
        return Err(
            "[dynamic_widget_range_invalid] Limita widgetului nu mai aparține sursei.".to_string(),
        );
    }
    if source.get(end..end.saturating_add(2)) == Some("\r\n") {
        end += 2;
    } else if source.get(end..end.saturating_add(1)) == Some("\n") {
        end += 1;
    } else if source.get(start.saturating_sub(2)..start) == Some("\r\n") {
        start -= 2;
    } else if source.get(start.saturating_sub(1)..start) == Some("\n") {
        start -= 1;
    }
    Ok(format!("{}{}", &source[..start], &source[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kernel::dynamic_widgets::DynamicWidgetResolutionStatus, source_graph::model::SourceRange,
    };

    fn instance(start: usize, end: usize) -> DynamicWidgetSourceInstance {
        DynamicWidgetSourceInstance {
            id: "source:dynamic-widget:one".to_string(),
            instance_id: "dynamic-field-one".to_string(),
            provider_id: "dynamic-field".to_string(),
            provider_kind: None,
            file: "templates/index.html".to_string(),
            range: SourceRange {
                start,
                end,
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
            },
            start_marker_range: SourceRange {
                start,
                end: start,
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
            },
            end_marker_range: SourceRange {
                start: end,
                end,
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
            },
            source_node_ids: Vec::new(),
            root_source_node_ids: Vec::new(),
            status: DynamicWidgetResolutionStatus::Resolved,
            properties: None,
            canonical_binding_path: None,
            canonical_binding_expression: None,
            source_revision: "revision".to_string(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn delete_absorbs_the_following_line_break() {
        let source = "before\nwidget\nafter";
        let result = remove_dynamic_widget_source(source, &instance(7, 13)).unwrap();
        assert_eq!(result, "before\nafter");
    }

    #[test]
    fn delete_rejects_a_stale_range() {
        let error = remove_dynamic_widget_source("short", &instance(0, 20)).unwrap_err();
        assert!(error.contains("dynamic_widget_range_invalid"));
    }
}
