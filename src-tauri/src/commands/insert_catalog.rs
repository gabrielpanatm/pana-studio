use serde::Deserialize;
use tauri::State;

use crate::{
    commands::workspace_entries::require_bound_workspace,
    kernel::{
        file_buffer_store::FileBufferRequestIdentity,
        insert_catalog::{
            build_insert_catalog, InsertCatalogContext, InsertCatalogSnapshot,
            INSERT_CATALOG_SCHEMA_VERSION,
        },
    },
    project_model::cache::{
        build_project_model_from_context, capture_project_model_build_context,
        publish_project_model_if_current,
    },
    state::AppState,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertCatalogRequest {
    pub identity: FileBufferRequestIdentity,
    pub expected_workspace_revision: u64,
    #[serde(default)]
    pub context: InsertCatalogContext,
}

#[tauri::command]
pub fn read_insert_catalog(
    request: InsertCatalogRequest,
    state: State<AppState>,
) -> Result<InsertCatalogSnapshot, String> {
    let (bound_root, project_root, runtime_session_id, workspace_revision) = {
        let (root, workspace) = require_bound_workspace(state.inner(), &request.identity)?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectWorkspace nu este inițializat pentru catalogul de inserare.".to_string()
        })?;
        if workspace.revision != request.expected_workspace_revision {
            return Err(stale_revision_error(
                request.expected_workspace_revision,
                workspace.revision,
            ));
        }
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
        return Err(stale_revision_error(
            request.expected_workspace_revision,
            context.projection().revision,
        ));
    }
    let model = build_project_model_from_context(&root, &context)?;

    // Building ProjectModel may overlap with a mutation. Rebind after the
    // projection work so a catalog can never be published for an obsolete
    // workspace generation or ProjectSession.
    {
        let (_root, workspace) = require_bound_workspace(state.inner(), &request.identity)?;
        let workspace = workspace.as_ref().ok_or_else(|| {
            "ProjectWorkspace a fost închis în timpul construirii catalogului.".to_string()
        })?;
        if workspace.revision != workspace_revision {
            return Err(stale_revision_error(workspace_revision, workspace.revision));
        }
    }
    publish_project_model_if_current(&state, &context, model.clone())?;

    let snapshot = build_insert_catalog(
        &model,
        project_root,
        runtime_session_id,
        workspace_revision,
        request.context,
    );
    debug_assert_eq!(snapshot.schema_version, INSERT_CATALOG_SCHEMA_VERSION);
    Ok(snapshot)
}

fn stale_revision_error(expected: u64, actual: u64) -> String {
    format!(
        "[insert_catalog_stale_revision] Catalogul de inserare a refuzat revizia stale {expected}; ProjectWorkspace este la revizia {actual}."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_error_is_machine_classifiable_and_precise() {
        let error = stale_revision_error(12, 13);
        assert!(error.contains("insert_catalog_stale_revision"));
        assert!(error.contains("12"));
        assert!(error.contains("13"));
    }
}
