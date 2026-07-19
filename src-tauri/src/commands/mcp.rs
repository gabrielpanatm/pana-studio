use std::collections::BTreeSet;

use tauri::{AppHandle, State};

use crate::{
    kernel::{
        context_hub::{
            AiContextApp, AiContextCore, AiContextDirtyState, AiContextFileInventory,
            AiContextProject, CanonicalAiContextSnapshot, ContextHubPublication,
            UiContextProjection, UI_CONTEXT_PROJECTION_SCHEMA_VERSION,
        },
        file_buffer_store::TextBufferRole,
        observability::now_ms,
        write_authority::WriteAuthorityError,
    },
    mcp::{
        configure_codex, read_codex_status, read_context_status, save_context_snapshot,
        AiContextStatus, CodexMcpStatus,
    },
    state::AppState,
};

const MAX_UI_CONTEXT_PROJECTION_BYTES: usize = 256 * 1024;
const MAX_FILE_HINTS_PER_ROLE: usize = 24;

#[tauri::command]
pub fn read_ai_context_status(app: AppHandle) -> Result<AiContextStatus, String> {
    read_context_status(&app)
}

#[tauri::command]
pub fn save_ai_context_snapshot(
    app: AppHandle,
    state: State<AppState>,
    snapshot: UiContextProjection,
) -> Result<AiContextStatus, String> {
    publish_ai_context_projection(&app, state.inner(), snapshot)
}

#[tauri::command]
pub fn write_ai_context_snapshot(
    app: AppHandle,
    state: State<AppState>,
    snapshot: UiContextProjection,
) -> Result<AiContextStatus, String> {
    publish_ai_context_projection(&app, state.inner(), snapshot)
}

pub(crate) fn current_ai_context_snapshot(
    state: &AppState,
) -> Result<Option<CanonicalAiContextSnapshot>, String> {
    let captured_at_ms = now_ms();
    state
        .ai_coordination
        .expire(captured_at_ms)
        .map_err(|error| error.to_string())?;
    let coordination = state
        .ai_coordination
        .snapshot(captured_at_ms)
        .map_err(|error| error.to_string())?;
    state
        .context_hub
        .snapshot(coordination)
        .map_err(|error| error.to_string())
}

fn publish_ai_context_projection(
    app: &AppHandle,
    state: &AppState,
    projection: UiContextProjection,
) -> Result<AiContextStatus, String> {
    validate_ui_projection(&projection)?;
    let publication = build_context_publication(state, projection)?;
    let receipt = state
        .context_hub
        .publish(publication, now_ms())
        .map_err(|error| error.to_string())?;
    let snapshot = current_ai_context_snapshot(state)?
        .ok_or_else(|| "Context Hub nu a returnat snapshotul tocmai publicat.".to_string())?;
    if receipt.changed {
        save_context_snapshot(app, &snapshot)
    } else {
        read_context_status(app)
    }
}

fn validate_ui_projection(projection: &UiContextProjection) -> Result<(), String> {
    if projection.schema_version != UI_CONTEXT_PROJECTION_SCHEMA_VERSION {
        return Err(format!(
            "UiContextProjection schemaVersion {} nu este suportată; versiunea curentă este {}.",
            projection.schema_version, UI_CONTEXT_PROJECTION_SCHEMA_VERSION
        ));
    }
    let encoded = serde_json::to_vec(projection)
        .map_err(|error| format!("UiContextProjection nu poate fi validată: {error}"))?;
    if encoded.len() > MAX_UI_CONTEXT_PROJECTION_BYTES {
        return Err(format!(
            "UiContextProjection are {} bytes și depășește limita de {} bytes.",
            encoded.len(),
            MAX_UI_CONTEXT_PROJECTION_BYTES
        ));
    }
    Ok(())
}

fn build_context_publication(
    state: &AppState,
    projection: UiContextProjection,
) -> Result<ContextHubPublication, String> {
    let workspace_guard = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Context Hub.".to_string())?;

    let (project, dirty_files, files, workspace_dirty, project_session_id) = if let Some(
        workspace,
    ) =
        workspace_guard.as_ref()
    {
        let snapshot = workspace.snapshot();
        if projection.expected_project_session_id.as_deref()
            != Some(snapshot.runtime_session_id.as_str())
            || projection.expected_project_revision != Some(snapshot.revision)
        {
            return Err(format!(
                    "UiContextProjection este stale: aștepta sesiunea {:?}/revizia {:?}, iar Rust deține sesiunea {}/revizia {}.",
                    projection.expected_project_session_id,
                    projection.expected_project_revision,
                    snapshot.runtime_session_id,
                    snapshot.revision
                ));
        }
        let dirty_files = dirty_files_from_workspace_snapshot(&snapshot);
        let files = file_inventory(workspace);
        let project_session_id = Some(snapshot.runtime_session_id.clone());
        (
            AiContextProject {
                root: Some(snapshot.project_root),
                session_id: project_session_id.clone(),
                is_open: true,
                is_zola: projection.project.is_zola,
                is_empty: projection.project.is_empty,
                project_revision: Some(snapshot.revision),
                disk_generation: Some(snapshot.disk_generation),
                preview_base_url: projection.project.preview_base_url.clone(),
                preview_warning: projection.project.preview_warning.clone(),
            },
            dirty_files,
            files,
            snapshot.dirty,
            project_session_id,
        )
    } else {
        if projection.expected_project_session_id.is_some()
            || projection.expected_project_revision.is_some()
        {
            return Err(
                    "UiContextProjection revendică un proiect, dar Rust nu are ProjectWorkspace deschis."
                        .to_string(),
                );
        }
        (
            AiContextProject {
                root: None,
                session_id: None,
                is_open: false,
                is_zola: false,
                is_empty: true,
                project_revision: None,
                disk_generation: None,
                preview_base_url: None,
                preview_warning: None,
            },
            Vec::new(),
            empty_file_inventory(),
            false,
            None,
        )
    };
    drop(workspace_guard);

    let ui_dirty = projection.ui_dirty_state.dirty;
    let core = AiContextCore {
        app: AiContextApp {
            name: "Pană Studio".to_string(),
            mode: "read_only_data_with_ram_coordination".to_string(),
        },
        project,
        workspace: projection.workspace,
        selection: projection.selection,
        css: projection.css,
        dirty_state: AiContextDirtyState {
            dirty: workspace_dirty || ui_dirty,
            project_workspace_dirty: workspace_dirty,
            ui_dirty,
            can_save: projection.ui_dirty_state.can_save,
            dirty_files,
            ui_areas: projection.ui_dirty_state.areas,
            blocked_reason: projection.ui_dirty_state.blocked_reason,
        },
        files,
        mood_board: projection.mood_board,
        external_disk: projection.external_disk,
        guidance: vec![
            "Datele și fișierele expuse prin MCP sunt read-only.".to_string(),
            "Înainte de a modifica sursele prin filesystem, AI trebuie să obțină un edit lease."
                .to_string(),
            "Dacă dirtyState.dirty este true, utilizatorul trebuie să salveze sau să arunce modificările înainte de transferul autorității."
                .to_string(),
            "După release, Pană Studio reconciliază discul. După expirare, ambele părți rămân blocate până când utilizatorul adoptă explicit starea stabilă de pe disc."
                .to_string(),
        ],
    };

    Ok(ContextHubPublication {
        project_session_id,
        ui_revision: projection.ui_revision,
        core,
    })
}

fn dirty_files_from_workspace_snapshot(
    snapshot: &crate::kernel::project_workspace::ProjectWorkspaceSnapshot,
) -> Vec<String> {
    let mut dirty_files = BTreeSet::new();
    dirty_files.extend(
        snapshot
            .documents
            .files
            .iter()
            .filter(|file| file.dirty)
            .map(|file| file.relative_path.clone()),
    );
    dirty_files.extend(snapshot.created_documents.iter().cloned());
    dirty_files.extend(snapshot.deleted_documents.iter().cloned());
    dirty_files.extend(snapshot.staged_binary_resources.iter().cloned());
    dirty_files.extend(snapshot.deleted_binary_resources.iter().cloned());
    dirty_files.extend(
        snapshot
            .page_js
            .drafts
            .iter()
            .map(|draft| draft.template_path.clone()),
    );
    dirty_files.into_iter().collect()
}

fn file_inventory(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
) -> AiContextFileInventory {
    let snapshot = workspace.documents.snapshot();
    let mut pages = Vec::new();
    let mut templates = Vec::new();
    let mut styles = Vec::new();
    let mut scripts = Vec::new();
    let mut config_and_data = Vec::new();
    let mut truncated = false;

    for file in &snapshot.files {
        let bucket = match file.role {
            TextBufferRole::Page => &mut pages,
            TextBufferRole::Template => &mut templates,
            TextBufferRole::Style => &mut styles,
            TextBufferRole::Script => &mut scripts,
            TextBufferRole::Config | TextBufferRole::Data | TextBufferRole::Other => {
                &mut config_and_data
            }
        };
        if bucket.len() < MAX_FILE_HINTS_PER_ROLE {
            bucket.push(file.relative_path.clone());
        } else {
            truncated = true;
        }
    }

    AiContextFileInventory {
        tracked_text_total: snapshot.file_count,
        pages,
        templates,
        styles,
        scripts,
        config_and_data,
        truncated,
    }
}

fn empty_file_inventory() -> AiContextFileInventory {
    AiContextFileInventory {
        tracked_text_total: 0,
        pages: Vec::new(),
        templates: Vec::new(),
        styles: Vec::new(),
        scripts: Vec::new(),
        config_and_data: Vec::new(),
        truncated: false,
    }
}

#[tauri::command]
pub async fn read_codex_mcp_status(app: AppHandle) -> Result<CodexMcpStatus, String> {
    tauri::async_runtime::spawn_blocking(move || read_codex_status(&app))
        .await
        .map_err(|error| {
            format!("Citirea configurației Codex a căzut în task-ul de fundal: {error}")
        })?
}

#[tauri::command]
pub async fn configure_codex_mcp(app: AppHandle) -> Result<CodexMcpStatus, WriteAuthorityError> {
    tauri::async_runtime::spawn_blocking(move || configure_codex(&app))
        .await
        .map_err(|error| {
            WriteAuthorityError::from(format!(
                "Configurarea Codex a căzut în task-ul de fundal: {error}"
            ))
        })?
}
