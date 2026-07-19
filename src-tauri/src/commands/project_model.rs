use std::collections::HashMap;

use tauri::State;

use crate::{
    commands::kernel_preview_context::require_preview_command_identity,
    kernel::preview_projection::PreviewStructuralCommandIdentity,
    project_model::{
        cache::{
            capture_project_model_build_lease, publish_project_model_if_current,
            publish_project_model_with_aliases_if_current,
        },
        move_engine::{html_identity_aliases, html_node_id_at_line, plan_html_move},
        template_workbench::{
            resolve_template_workbench_plan as resolve_workbench_plan, TemplateWorkbenchPlan,
            TemplateWorkbenchPlanInput,
        },
        ProjectHtmlMoveIntent, ProjectHtmlMovePlan, ProjectModelSnapshot,
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
    let (root, session, lease) = capture_project_model_build_lease(&state)?;
    require_preview_command_identity(&session, &identity)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let plan = resolve_workbench_plan(&model, &input)?;
    publish_project_model_if_current(&state, &lease, model)?;
    Ok(plan)
}

#[tauri::command]
pub fn read_project_model_with_drafts(
    _draft_sources: HashMap<String, String>,
    state: State<AppState>,
) -> Result<ProjectModelSnapshot, String> {
    let (root, _session, lease) = capture_project_model_build_lease(&state)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let snapshot = model.snapshot();
    publish_project_model_if_current(&state, &lease, model)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn plan_project_html_move(
    intent: ProjectHtmlMoveIntent,
    _draft_sources: HashMap<String, String>,
    state: State<AppState>,
) -> Result<ProjectHtmlMovePlan, String> {
    let (root, _session, lease) = capture_project_model_build_lease(&state)?;
    let model = crate::project_model::build_project_model_from_workspace_projection(
        &root,
        lease.projection(),
    )?;
    let aliases = state
        .project_workspace
        .lock()
        .map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru aliasurile Source Identity.".to_string()
        })?
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?
        .source_identity_aliases
        .clone();
    let plan = plan_html_move(&model, &intent, &aliases);

    if let Some(patch) = plan.patch.as_ref() {
        let mut candidate = lease.projection().clone();
        candidate.deleted_sources.remove(&patch.file);
        candidate.changed_paths.insert(patch.file.clone());
        candidate
            .source_texts
            .insert(patch.file.clone(), patch.contents.clone());
        let after_model =
            crate::project_model::build_project_model_from_workspace_projection(&root, &candidate)?;
        let next_aliases = html_identity_aliases(&model, &after_model);
        let moved_source_after_id = html_node_id_at_line(
            &after_model,
            &patch.file,
            &patch.source_label,
            patch.new_start_line,
        );
        let mut alias_updates = next_aliases.into_iter().collect::<Vec<_>>();
        if let Some(after_id) = moved_source_after_id {
            alias_updates.push((patch.resolved_source_id.clone(), after_id.clone()));
            if let Some(request_source_id) = intent.source_source_id.as_ref() {
                alias_updates.push((request_source_id.clone(), after_id));
            }
        }
        publish_project_model_with_aliases_if_current(
            &state,
            &lease,
            after_model,
            Some(alias_updates),
        )?;
    } else {
        publish_project_model_if_current(&state, &lease, model)?;
    }

    Ok(plan)
}
