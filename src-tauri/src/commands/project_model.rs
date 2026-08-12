use tauri::State;

use crate::{
    commands::kernel_preview_context::require_preview_command_identity,
    kernel::preview_projection::PreviewStructuralCommandIdentity,
    project_model::{
        cache::{
            build_project_model_from_context, capture_project_model_build_context,
            current_project_model_if_fresh, publish_project_model_if_current,
        },
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
    if let Some(model) = current_project_model_if_fresh(&state)? {
        return Ok(model.snapshot());
    }
    let (root, _session, context) = capture_project_model_build_context(&state)?;
    let model = build_project_model_from_context(&root, &context)?;
    let snapshot = model.snapshot();
    publish_project_model_if_current(&state, &context, model)?;
    Ok(snapshot)
}

#[tauri::command(async)]
pub fn resolve_template_workbench_plan(
    input: TemplateWorkbenchPlanInput,
    identity: PreviewStructuralCommandIdentity,
    state: State<AppState>,
) -> Result<TemplateWorkbenchPlan, String> {
    let (root, session, context) = capture_project_model_build_context(&state)?;
    require_preview_command_identity(&session, &identity)?;
    let model = build_project_model_from_context(&root, &context)?;
    let plan = resolve_workbench_plan(&model, &input)?;
    publish_project_model_if_current(&state, &context, model)?;
    Ok(plan)
}
