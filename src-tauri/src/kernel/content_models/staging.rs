use crate::kernel::project_workspace::{
    ProjectWorkspace, ProjectWorkspaceIdentity, ProjectWorkspaceMutationReceipt,
    WorkspaceMutationMetadata,
};

use super::{ContentModelMutationPlan, PlannedContentModelMutation};

pub fn stage_content_model_mutation(
    workspace: &mut ProjectWorkspace,
    planned: PlannedContentModelMutation,
    now_ms: u128,
) -> Result<(ContentModelMutationPlan, ProjectWorkspaceMutationReceipt), String> {
    if planned.plan.blocked {
        return Err(format!(
            "Planul este blocat: {}",
            planned.plan.blockers.join(" ")
        ));
    }
    let identity = ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    };
    let plan = planned.plan;
    let mutation = workspace.stage_composite_changes(
        &identity,
        WorkspaceMutationMetadata {
            label: plan.label.clone(),
            source: "content_models.semantic".to_string(),
            coalesce_key: None,
            transaction_id: Some(format!("content-model-{}", plan.plan_id)),
        },
        planned.changes,
        planned.deletes,
        None,
        now_ms,
    )?;
    Ok((plan, mutation))
}
