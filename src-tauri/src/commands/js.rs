use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    js::{
        self, require_page_js_draft_session_identity, require_page_js_file_buffer_identity,
        MotionRuntimeContract, PageJsCommandReceipt, PageJsConfig, PageJsDraftStageInput,
        PageJsDraftStageReceipt, PageJsRequestIdentity,
    },
    kernel::{
        file_buffer_store::FileBufferStore,
        motion_graph::{
            apply_motion_mutation as apply_motion_mutation_kernel, MotionMutation,
            MotionMutationInput, MotionMutationReceipt,
        },
        project_session::ProjectSessionSnapshot,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspaceIdentity,
            WorkspaceMutationMetadata,
        },
    },
    project::strip_zola_root_prefix,
    state::AppState,
};

fn with_bound_page_js_file_buffer<T>(
    state: &AppState,
    identity: &PageJsRequestIdentity,
    operation: impl FnOnce(
        &std::path::Path,
        &ProjectSessionSnapshot,
        &FileBufferStore,
        &crate::js::PageJsDraftStore,
    ) -> Result<T, String>,
) -> Result<PageJsCommandReceipt<T>, String> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Page JS.".to_string())?;
    let project_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Page JS.".to_string())?;
    let current_root_string = project_root.to_string_lossy().into_owned();
    let project_workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page JS.".to_string())?;
    let workspace = project_workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page JS.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        project_root,
    )?;
    require_page_js_file_buffer_identity(
        &current_root_string,
        &workspace.session,
        &workspace.documents,
        identity,
    )?;
    let payload = operation(
        project_root,
        &workspace.session,
        &workspace.documents,
        &workspace.page_js,
    )?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        project_root,
    )?;
    Ok(PageJsCommandReceipt::new(&workspace.session, payload))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsWorkspaceState {
    pub template_path: String,
    pub motion_runtime: MotionRuntimeContract,
    pub accepted: PageJsConfig,
    pub current: PageJsConfig,
    pub dirty: bool,
    pub entry_revision: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPageMutationInput {
    pub template_path: String,
    pub expected_project_root: String,
    pub expected_session_id: String,
    #[serde(default)]
    pub expected_entry_revision: Option<u64>,
    pub mutation: MotionMutation,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionPageMutationReceipt {
    pub mutation: MotionMutationReceipt,
    pub page_js: PageJsDraftStageReceipt,
    pub workspace_revision: u64,
}

fn motion_mutation_coalesce_key(mutation: &MotionMutation) -> Option<String> {
    match mutation {
        MotionMutation::UpdateInteraction { interaction } => {
            Some(format!("motion.v2.interaction.{}", interaction.id))
        }
        MotionMutation::UpdateAction {
            interaction_id,
            action,
        } => Some(format!("motion.v2.action.{interaction_id}.{}", action.id())),
        MotionMutation::SetActionTiming {
            interaction_id,
            action_id,
            ..
        } => Some(format!("motion.v2.timing.{interaction_id}.{action_id}")),
        MotionMutation::UpsertBehavior { behavior } => {
            Some(format!("motion.v2.behavior.{}", behavior.id()))
        }
        MotionMutation::UpsertCustomCode { custom_code } => {
            Some(format!("motion.v2.custom.{}", custom_code.id))
        }
        _ => None,
    }
}

/// Returns one atomic, read-only projection of the Page JS resource owned by
/// ProjectWorkspace. UI clients must use `accepted` as the stable staging
/// baseline and `current` as the value to render; no frontend draft registry is
/// involved.
#[tauri::command(async)]
pub fn get_page_js_workspace_state(
    template_path: String,
    identity: PageJsRequestIdentity,
    state: State<AppState>,
) -> Result<PageJsCommandReceipt<PageJsWorkspaceState>, String> {
    with_bound_page_js_file_buffer(
        state.inner(),
        &identity,
        |project_root, _, store, drafts| {
            let template_path = strip_zola_root_prefix(&template_path).to_string();
            if let Some(draft) = drafts.drafts.get(&template_path) {
                return Ok(PageJsWorkspaceState {
                    template_path,
                    motion_runtime: MotionRuntimeContract::current(),
                    accepted: draft.base.clone(),
                    current: draft.current.clone(),
                    dirty: true,
                    entry_revision: Some(draft.revision),
                });
            }
            let accepted = js::read_page_motion_config(project_root, store, &template_path)?;
            Ok(PageJsWorkspaceState {
                template_path,
                motion_runtime: MotionRuntimeContract::current(),
                current: accepted.clone(),
                accepted,
                dirty: false,
                entry_revision: None,
            })
        },
    )
}

#[tauri::command(async)]
pub fn stage_page_js_draft(
    input: PageJsDraftStageInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PageJsDraftStageReceipt, String> {
    let mut input = input;
    input.template_path = strip_zola_root_prefix(&input.template_path).to_string();
    let identity = PageJsRequestIdentity {
        expected_project_root: input.expected_project_root.clone(),
        expected_session_id: input.expected_session_id.clone(),
    };
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Page JS.".to_string())?;
    let current_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Page JS.".to_string())?
        .to_string_lossy()
        .into_owned();
    let mut workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page JS.".to_string())?;
    let workspace = workspace
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page JS.".to_string())?;
    input.cachebust_assets =
        crate::commands::config::cachebust_assets_from_store(&workspace.documents)?;
    require_page_js_draft_session_identity(
        &current_root,
        &workspace.session,
        &workspace.page_js,
        &identity,
    )?;
    let metadata = WorkspaceMutationMetadata {
        label: "Editare Page JS".to_string(),
        source: input
            .source
            .clone()
            .unwrap_or_else(|| "inspector.js".to_string()),
        coalesce_key: input.coalesce_key.clone(),
        transaction_id: input.transaction_id.clone(),
    };
    let receipt = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.stage_page_js(
            &workspace_identity(candidate),
            metadata,
            input,
            crate::kernel::observability::now_ms(),
        )
    })?;
    receipt
        .page_js
        .ok_or_else(|| "ProjectWorkspace nu a returnat receipt Page JS.".to_string())
}

#[tauri::command(async)]
pub fn clear_page_js_draft(
    template_path: String,
    expected_revision: Option<u64>,
    expected_project_root: String,
    expected_session_id: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<PageJsDraftStageReceipt, String> {
    let template_path = strip_zola_root_prefix(&template_path);
    let identity = PageJsRequestIdentity {
        expected_project_root,
        expected_session_id,
    };
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Page JS.".to_string())?;
    let current_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Page JS.".to_string())?
        .to_string_lossy()
        .into_owned();
    let mut workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Page JS.".to_string())?;
    let workspace = workspace
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Page JS.".to_string())?;
    require_page_js_draft_session_identity(
        &current_root,
        &workspace.session,
        &workspace.page_js,
        &identity,
    )?;
    let receipt = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.clear_page_js(
            &workspace_identity(candidate),
            WorkspaceMutationMetadata {
                label: "Revenire Page JS la baseline".to_string(),
                source: "inspector.js".to_string(),
                coalesce_key: None,
                transaction_id: None,
            },
            template_path,
            expected_revision,
            crate::kernel::observability::now_ms(),
        )
    })?;
    receipt
        .page_js
        .ok_or_else(|| "ProjectWorkspace nu a returnat receipt Page JS.".to_string())
}

fn workspace_identity(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
) -> ProjectWorkspaceIdentity {
    ProjectWorkspaceIdentity {
        expected_project_root: workspace.session.project_root.clone(),
        expected_session_id: workspace.runtime_session_id(),
        expected_revision: workspace.revision,
    }
}

#[tauri::command(async)]
pub fn apply_motion_mutation(
    input: MotionPageMutationInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<MotionPageMutationReceipt, String> {
    let template_path = strip_zola_root_prefix(&input.template_path).to_string();
    let identity = PageJsRequestIdentity {
        expected_project_root: input.expected_project_root.clone(),
        expected_session_id: input.expected_session_id.clone(),
    };
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca root-ul curent pentru Motion v2.".to_string())?;
    let project_root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect curent pentru Motion v2.".to_string())?;
    let current_root_string = project_root.to_string_lossy().into_owned();
    let mut workspace_guard = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Motion v2.".to_string())?;
    let workspace = workspace_guard
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat pentru Motion v2.".to_string())?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        project_root,
    )?;
    require_page_js_draft_session_identity(
        &current_root_string,
        &workspace.session,
        &workspace.page_js,
        &identity,
    )?;

    let draft = workspace.page_js.drafts.get(&template_path);
    let current_entry_revision = draft.map(|entry| entry.revision);
    if current_entry_revision != input.expected_entry_revision {
        return Err(format!(
            "Motion v2 a refuzat mutația stale pentru {template_path}: entryRevision așteptat {:?}, curent {:?}.",
            input.expected_entry_revision, current_entry_revision
        ));
    }
    let accepted = draft
        .map(|entry| entry.base.clone())
        .or_else(|| workspace.accepted_page_js_config(&template_path).cloned())
        .unwrap_or(js::read_page_motion_config(
            project_root,
            &workspace.documents,
            &template_path,
        )?);
    let current = draft
        .map(|entry| entry.current.clone())
        .unwrap_or_else(|| accepted.clone());
    let mutation = apply_motion_mutation_kernel(MotionMutationInput {
        config: current,
        mutation: input.mutation,
    })?;
    let command = mutation.command.clone();
    let coalesce_key = mutation
        .transaction
        .as_ref()
        .and_then(|transaction| motion_mutation_coalesce_key(&transaction.forward));
    let stage_input = PageJsDraftStageInput {
        template_path: template_path.clone(),
        expected_project_root: input.expected_project_root,
        expected_session_id: input.expected_session_id,
        base_config: accepted,
        current_config: mutation.config.clone(),
        cachebust_assets: crate::commands::config::cachebust_assets_from_store(
            &workspace.documents,
        )?,
        source: Some("motion.v2".to_string()),
        coalesce_key,
        transaction_id: mutation
            .transaction
            .as_ref()
            .map(|transaction| transaction.id.clone()),
    };
    let receipt = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.stage_page_js(
            &workspace_identity(candidate),
            WorkspaceMutationMetadata {
                label: command.clone(),
                source: "motion.v2".to_string(),
                coalesce_key: stage_input.coalesce_key.clone(),
                transaction_id: stage_input.transaction_id.clone(),
            },
            stage_input,
            crate::kernel::observability::now_ms(),
        )
    })?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        project_root,
    )?;
    let page_js = receipt
        .page_js
        .ok_or_else(|| "ProjectWorkspace nu a returnat receipt pentru Motion v2.".to_string())?;
    Ok(MotionPageMutationReceipt {
        mutation,
        page_js,
        workspace_revision: receipt.revision_after,
    })
}

#[cfg(test)]
mod motion_command_tests {
    use super::*;

    #[test]
    fn timing_edits_coalesce_by_interaction_and_action() {
        let first = MotionMutation::SetActionTiming {
            interaction_id: "hero".to_string(),
            action_id: "fade".to_string(),
            start: Some(100.0),
            duration: None,
        };
        let second = MotionMutation::SetActionTiming {
            interaction_id: "hero".to_string(),
            action_id: "fade".to_string(),
            start: Some(150.0),
            duration: Some(600.0),
        };

        assert_eq!(
            motion_mutation_coalesce_key(&first),
            motion_mutation_coalesce_key(&second)
        );
        assert_eq!(
            motion_mutation_coalesce_key(&first).as_deref(),
            Some("motion.v2.timing.hero.fade")
        );
    }

    #[test]
    fn structural_edits_do_not_coalesce() {
        assert_eq!(
            motion_mutation_coalesce_key(&MotionMutation::DeleteInteraction {
                interaction_id: "hero".to_string(),
            }),
            None
        );
    }

    #[test]
    fn page_js_workspace_projects_the_current_motion_runtime_contract() {
        let state = PageJsWorkspaceState {
            template_path: "templates/index.html".to_string(),
            motion_runtime: MotionRuntimeContract::current(),
            accepted: PageJsConfig::default(),
            current: PageJsConfig::default(),
            dirty: false,
            entry_revision: None,
        };
        let value = serde_json::to_value(state).expect("serialize Page JS workspace");
        let current = MotionRuntimeContract::current();

        assert_eq!(
            value["motionRuntime"]["schemaVersion"],
            current.schema_version
        );
        assert_eq!(
            value["motionRuntime"]["animeVersion"],
            current.anime_version
        );
    }
}
