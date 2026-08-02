use std::collections::HashMap;

use tauri::State;

use crate::{
    commands::kernel_preview_context::require_preview_command_identity,
    kernel::preview_projection::PreviewStructuralCommandIdentity,
    project_model::{
        cache::{capture_project_model_build_context, publish_project_model_if_current},
        template_workbench::{
            resolve_template_workbench_plan as resolve_workbench_plan, TemplateWorkbenchPlan,
            TemplateWorkbenchPlanInput,
        },
        ProjectModelSnapshot,
    },
    state::AppState,
};

#[tauri::command]
pub fn read_project_model(state: State<AppState>) -> Result<ProjectModelSnapshot, String> {
    read_project_model_with_drafts(HashMap::new(), state)
}

#[tauri::command(async)]
pub fn resolve_template_workbench_plan(
    input: TemplateWorkbenchPlanInput,
    identity: PreviewStructuralCommandIdentity,
    state: State<AppState>,
) -> Result<TemplateWorkbenchPlan, String> {
    let (root, session, context) = capture_project_model_build_context(&state)?;
    require_preview_command_identity(&session, &identity)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        context.projection(),
    )?;
    let plan = resolve_workbench_plan(&model, &input)?;
    publish_project_model_if_current(&state, &context, model)?;
    Ok(plan)
}

#[tauri::command]
pub fn read_project_model_with_drafts(
    _draft_sources: HashMap<String, String>,
    state: State<AppState>,
) -> Result<ProjectModelSnapshot, String> {
    let (root, _session, context) = capture_project_model_build_context(&state)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        context.projection(),
    )?;
    let snapshot = model.snapshot();
    publish_project_model_if_current(&state, &context, model)?;
    Ok(snapshot)
}
