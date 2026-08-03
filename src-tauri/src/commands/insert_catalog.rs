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
    let (root, projection, project_root, runtime_session_id, workspace_revision) = {
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
            workspace.capture_projection_snapshot()?,
            workspace.session.project_root.clone(),
            workspace.runtime_session_id(),
            workspace.revision,
        )
    };

    let model =
        crate::project_model::build_project_model_from_workspace_projection(&root, &projection)?;

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
