use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::project::{
        require_project_workspace_available_for_write, require_recovery_coordinator_clean_for_write,
    },
    kernel::{
        file_buffer_store::FileBufferStoreLimits, project_session::ProjectSessionSnapshot,
        write_authority::WriteAuthorityRuntime,
    },
    mood,
    project_model::cache::invalidate_project_model_for_session,
    state::AppState,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardReadRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardReadReceipt {
    pub project_root: String,
    pub session_id: String,
    pub board: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardSaveRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub board: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardSaveReceipt {
    pub project_root: String,
    pub session_id: String,
    pub board: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardImageReadRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub relative_path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardImagePaletteRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub relative_path: String,
    pub max_colors: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardSvgAssetExportRequest {
    pub expected_project_root: String,
    pub expected_session_id: String,
    pub relative_path: String,
    pub svg: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardAssetWriteReceipt {
    pub project_root: String,
    pub session_id: String,
    pub relative_path: String,
    pub projection_invalidated: bool,
    pub projection_diagnostic: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardImageDataUrlReceipt {
    pub project_root: String,
    pub session_id: String,
    pub relative_path: String,
    pub data_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardImagePaletteReceipt {
    pub project_root: String,
    pub session_id: String,
    pub relative_path: String,
    pub colors: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodBoardSvgSourceReceipt {
    pub project_root: String,
    pub session_id: String,
    pub relative_path: String,
    pub source: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CapturedMoodBoardSession {
    pub(crate) root: PathBuf,
    pub(crate) session: ProjectSessionSnapshot,
}

impl CapturedMoodBoardSession {
    pub(crate) fn project_root(&self) -> &str {
        &self.session.project_root
    }

    pub(crate) fn session_id(&self) -> String {
        self.session.runtime_instance_id()
    }
}

pub(crate) fn capture_mood_board_request_session(
    state: &AppState,
    expected_project_root: &str,
    expected_session_id: &str,
) -> Result<CapturedMoodBoardSession, String> {
    // Keep ProjectTransition's current_root -> ProjectSession lock prefix so a
    // request can never combine the root of A with the session of B.
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut captura root-ul pentru Mood Board.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut captura ProjectWorkspace pentru Mood Board.".to_string())?;
    let root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect deschis.".to_string())?;
    let session = workspace
        .as_ref()
        .map(|workspace| &workspace.session)
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let root_label = root.to_string_lossy();
    if session.project_root != root_label {
        return Err(format!(
            "Mood Board a refuzat o stare incoerentă: root-ul curent este {root_label}, dar ProjectSession aparține {}.",
            session.project_root
        ));
    }
    require_mood_board_request_identity(
        &session.project_root,
        &session.runtime_instance_id(),
        expected_project_root,
        expected_session_id,
    )?;
    Ok(CapturedMoodBoardSession {
        root: root.clone(),
        session: session.clone(),
    })
}

pub(crate) fn require_mood_board_request_identity(
    live_project_root: &str,
    live_session_id: &str,
    expected_project_root: &str,
    expected_session_id: &str,
) -> Result<(), String> {
    if live_project_root == expected_project_root && live_session_id == expected_session_id {
        return Ok(());
    }
    Err(format!(
        "Mood Board a refuzat un request stale: așteptat root/session {expected_project_root}/{expected_session_id}, activ {live_project_root}/{live_session_id}."
    ))
}

fn with_mood_board_session_read_lease<R: Runtime, T>(
    app: &AppHandle<R>,
    captured: &CapturedMoodBoardSession,
    operation: impl FnOnce(&dyn Fn(&Path, u64, &str) -> Result<Vec<u8>, String>) -> Result<T, String>,
) -> Result<T, String> {
    let session_id = captured.session_id();
    let runtime = app
        .try_state::<WriteAuthorityRuntime>()
        .ok_or_else(|| "WriteAuthorityRuntime lipsește pentru citirea Mood Board.".to_string())?;
    let lease =
        runtime.acquire_active_project_read_lease_for_session(&captured.root, &session_id)?;
    let read_project_file = |relative_path: &Path, max_bytes: u64, public_label: &str| {
        lease
            .read_bounded_regular_file(relative_path, max_bytes, public_label)?
            .map(|snapshot| snapshot.bytes)
            .ok_or_else(|| {
                format!(
                    "Fișierul Mood Board {} nu mai există.",
                    relative_path.display()
                )
            })
    };
    // The lease remains alive through the entire read/decode and receipt build;
    // ProjectSession publication cannot change underneath the operation.
    operation(&read_project_file)
}

pub(crate) fn finalize_mood_board_asset_write<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    captured: &CapturedMoodBoardSession,
    relative_path: String,
) -> Result<MoodBoardAssetWriteReceipt, String> {
    let session_id = captured.session_id();
    let (projection_invalidated, mut diagnostics) =
        match invalidate_project_model_for_session(state, captured.project_root(), &session_id) {
            Ok(Some(_)) => (true, Vec::new()),
            Ok(None) => (
                false,
                vec![
                    "ProjectModel nu a fost invalidat deoarece sesiunea care a comis asset-ul nu mai este activă."
                        .to_string(),
                ],
            ),
            Err(error) => (
                false,
                vec![format!(
                    "ProjectModel nu a putut fi invalidat după commit: {error}"
                )],
            ),
        };
    match app.try_state::<WriteAuthorityRuntime>() {
        Some(runtime) => {
            if let Err(error) =
                runtime.acquire_active_project_read_lease_for_session(&captured.root, &session_id)
            {
                diagnostics.push(format!(
                    "Sesiunea care a comis asset-ul nu mai este activă la receipt: {error}"
                ));
            }
        }
        None => diagnostics.push(
            "WriteAuthorityRuntime lipsește la verificarea post-commit a receipt-ului.".to_string(),
        ),
    }
    // Once WriteAuthority returned success, the disk effect is committed. A
    // concurrent project publication may make the projection stale, but must
    // never turn that committed effect into a false command failure.
    Ok(MoodBoardAssetWriteReceipt {
        project_root: captured.project_root().to_string(),
        session_id,
        relative_path,
        projection_invalidated,
        projection_diagnostic: (!diagnostics.is_empty()).then(|| diagnostics.join(" ")),
    })
}

#[tauri::command]
pub async fn read_mood_board(
    input: MoodBoardReadRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MoodBoardReadReceipt, String> {
    read_mood_board_impl(input, app, state.inner()).await
}

async fn read_mood_board_impl<R: Runtime>(
    input: MoodBoardReadRequest,
    app: AppHandle<R>,
    state: &AppState,
) -> Result<MoodBoardReadReceipt, String> {
    let captured = capture_mood_board_request_session(
        state,
        &input.expected_project_root,
        &input.expected_session_id,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        with_mood_board_session_read_lease(&app, &captured, |_| {
            let board = mood::read_mood_board(&captured.root)?;
            Ok(MoodBoardReadReceipt {
                project_root: captured.project_root().to_string(),
                session_id: captured.session_id(),
                board,
            })
        })
    })
    .await
    .map_err(|error| format!("Nu am putut citi documentul Mood Board: {error}"))?
}

#[tauri::command]
pub fn save_mood_board(
    input: MoodBoardSaveRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<MoodBoardSaveReceipt, String> {
    save_mood_board_impl(input, &app, &state)
}

#[tauri::command]
pub fn write_mood_board(
    input: MoodBoardSaveRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<MoodBoardSaveReceipt, String> {
    save_mood_board_impl(input, &app, &state)
}

fn save_mood_board_impl(
    input: MoodBoardSaveRequest,
    app: &AppHandle,
    state: &State<AppState>,
) -> Result<MoodBoardSaveReceipt, String> {
    let captured = capture_mood_board_request_session(
        state.inner(),
        &input.expected_project_root,
        &input.expected_session_id,
    )?;
    require_project_workspace_available_for_write(state)?;
    require_recovery_coordinator_clean_for_write(state, &captured.session, "Mood Board")?;
    let session_id = captured.session_id();
    let board = mood::write_mood_board(app, &captured.root, input.board, &session_id)?;
    Ok(MoodBoardSaveReceipt {
        project_root: captured.project_root().to_string(),
        session_id,
        board,
    })
}

#[tauri::command]
pub async fn export_mood_board_svg_asset(
    input: MoodBoardSvgAssetExportRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MoodBoardAssetWriteReceipt, String> {
    export_mood_board_svg_asset_impl(input, app, &state).await
}

async fn export_mood_board_svg_asset_impl<R: Runtime>(
    input: MoodBoardSvgAssetExportRequest,
    app: AppHandle<R>,
    state: &State<'_, AppState>,
) -> Result<MoodBoardAssetWriteReceipt, String> {
    let captured = capture_mood_board_request_session(
        state.inner(),
        &input.expected_project_root,
        &input.expected_session_id,
    )?;
    require_project_workspace_available_for_write(&state)?;
    require_recovery_coordinator_clean_for_write(&state, &captured.session, "Mood Board")?;
    let worker_app = app.clone();
    let worker_session = captured.clone();
    let saved_path = tauri::async_runtime::spawn_blocking(move || {
        mood::export_mood_board_svg_asset(
            &worker_app,
            &worker_session.root,
            &input.relative_path,
            &input.svg,
            &worker_session.session_id(),
        )
    })
    .await
    .map_err(|error| format!("Exportul SVG a căzut în task-ul de fundal: {error}"))??;
    finalize_mood_board_asset_write(&app, state.inner(), &captured, saved_path)
}

#[tauri::command]
pub async fn read_mood_board_image_data_url(
    input: MoodBoardImageReadRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MoodBoardImageDataUrlReceipt, String> {
    read_mood_board_image_data_url_impl(input, app, state.inner()).await
}

async fn read_mood_board_image_data_url_impl<R: Runtime>(
    input: MoodBoardImageReadRequest,
    app: AppHandle<R>,
    state: &AppState,
) -> Result<MoodBoardImageDataUrlReceipt, String> {
    let relative_path = mood::normalize_mood_board_image_relative_path(&input.relative_path)?;
    let captured = capture_mood_board_request_session(
        state,
        &input.expected_project_root,
        &input.expected_session_id,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _resource_permit = mood::acquire_heavy_mood_asset_operation()?;
        with_mood_board_session_read_lease(&app, &captured, |read_project_file| {
            let data_url = mood::read_mood_board_image_data_url_with_reader(
                &relative_path,
                |max_bytes, operation| {
                    read_project_file(Path::new(&relative_path), max_bytes, operation)
                },
            )?;
            Ok(MoodBoardImageDataUrlReceipt {
                project_root: captured.project_root().to_string(),
                session_id: captured.session_id(),
                relative_path,
                data_url,
            })
        })
    })
    .await
    .map_err(|error| format!("Nu am putut finaliza preview-ul imaginii: {error}"))?
}

#[tauri::command]
pub async fn read_mood_board_image_original_data_url(
    input: MoodBoardImageReadRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MoodBoardImageDataUrlReceipt, String> {
    read_mood_board_image_original_data_url_impl(input, app, state.inner()).await
}

async fn read_mood_board_image_original_data_url_impl<R: Runtime>(
    input: MoodBoardImageReadRequest,
    app: AppHandle<R>,
    state: &AppState,
) -> Result<MoodBoardImageDataUrlReceipt, String> {
    let relative_path = mood::normalize_mood_board_image_relative_path(&input.relative_path)?;
    let captured = capture_mood_board_request_session(
        state,
        &input.expected_project_root,
        &input.expected_session_id,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _resource_permit = mood::acquire_heavy_mood_asset_operation()?;
        with_mood_board_session_read_lease(&app, &captured, |read_project_file| {
            let data_url = mood::read_mood_board_image_original_data_url_with_reader(
                &relative_path,
                |max_bytes, operation| {
                    read_project_file(Path::new(&relative_path), max_bytes, operation)
                },
            )?;
            Ok(MoodBoardImageDataUrlReceipt {
                project_root: captured.project_root().to_string(),
                session_id: captured.session_id(),
                relative_path,
                data_url,
            })
        })
    })
    .await
    .map_err(|error| format!("Nu am putut citi imaginea pentru export: {error}"))?
}

#[tauri::command]
pub async fn extract_mood_board_image_palette(
    input: MoodBoardImagePaletteRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MoodBoardImagePaletteReceipt, String> {
    extract_mood_board_image_palette_impl(input, app, state.inner()).await
}

async fn extract_mood_board_image_palette_impl<R: Runtime>(
    input: MoodBoardImagePaletteRequest,
    app: AppHandle<R>,
    state: &AppState,
) -> Result<MoodBoardImagePaletteReceipt, String> {
    let relative_path = mood::normalize_mood_board_image_relative_path(&input.relative_path)?;
    let max_colors = input.max_colors.unwrap_or(5).clamp(1, 12);
    let captured = capture_mood_board_request_session(
        state,
        &input.expected_project_root,
        &input.expected_session_id,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let _resource_permit = mood::acquire_heavy_mood_asset_operation()?;
        with_mood_board_session_read_lease(&app, &captured, |read_project_file| {
            let colors = mood::extract_mood_board_image_palette_with_reader(
                &relative_path,
                max_colors,
                |max_bytes, operation| {
                    read_project_file(Path::new(&relative_path), max_bytes, operation)
                },
            )?;
            Ok(MoodBoardImagePaletteReceipt {
                project_root: captured.project_root().to_string(),
                session_id: captured.session_id(),
                relative_path,
                colors,
            })
        })
    })
    .await
    .map_err(|error| format!("Nu am putut finaliza extragerea paletei: {error}"))?
}

fn capture_mood_board_svg_source(
    state: &AppState,
    expected_project_root: &str,
    expected_session_id: &str,
    relative_path: &str,
) -> Result<
    (
        CapturedMoodBoardSession,
        Option<String>,
        FileBufferStoreLimits,
    ),
    String,
> {
    let current_root = state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut captura root-ul pentru importul SVG.".to_string())?;
    let workspace = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut captura ProjectWorkspace pentru importul SVG.".to_string())?;
    let root = current_root
        .as_ref()
        .ok_or_else(|| "Nu există proiect deschis.".to_string())?;
    let workspace = workspace
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    let session = &workspace.session;
    if session.project_root != root.to_string_lossy() {
        return Err("Importul SVG a găsit root și ProjectSession incoerente.".to_string());
    }
    require_mood_board_request_identity(
        &session.project_root,
        &session.runtime_instance_id(),
        expected_project_root,
        expected_session_id,
    )?;
    let store = &workspace.documents;
    if store.project_root != session.project_root || store.session_id != session.id {
        return Err("Importul SVG a refuzat un FileBufferStore stale.".to_string());
    }
    let draft = store.text_for(relative_path);
    let svg_limit = store
        .limits
        .max_file_bytes
        .min(mood::MAX_MOOD_BOARD_SVG_BYTES as u64);
    if draft
        .as_ref()
        .is_some_and(|source| source.len() as u64 > svg_limit)
    {
        return Err(format!(
            "SVG-ul {relative_path} depășește limita Mood Board de {svg_limit} bytes."
        ));
    }
    let mut limits = store.limits.clone();
    limits.max_file_bytes = svg_limit;
    Ok((
        CapturedMoodBoardSession {
            root: root.clone(),
            session: session.clone(),
        },
        draft,
        limits,
    ))
}

#[tauri::command]
pub async fn read_mood_board_svg_source(
    input: MoodBoardImageReadRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<MoodBoardSvgSourceReceipt, String> {
    read_mood_board_svg_source_impl(input, app, state.inner()).await
}

async fn read_mood_board_svg_source_impl<R: Runtime>(
    input: MoodBoardImageReadRequest,
    app: AppHandle<R>,
    state: &AppState,
) -> Result<MoodBoardSvgSourceReceipt, String> {
    let relative_path = mood::normalize_mood_board_svg_source_relative_path(&input.relative_path)?;
    let (captured, draft, limits) = capture_mood_board_svg_source(
        state,
        &input.expected_project_root,
        &input.expected_session_id,
        &relative_path,
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        with_mood_board_session_read_lease(&app, &captured, |read_project_file| {
            let source = match draft {
                Some(source) => source,
                None => {
                    let bytes = read_project_file(
                        Path::new(&relative_path),
                        limits.max_file_bytes,
                        "import SVG editabil Mood Board",
                    )?;
                    String::from_utf8(bytes).map_err(|error| {
                        format!("SVG-ul {relative_path} nu este UTF-8 valid: {error}")
                    })?
                }
            };
            Ok(MoodBoardSvgSourceReceipt {
                project_root: captured.project_root().to_string(),
                session_id: captured.session_id(),
                relative_path,
                source,
            })
        })
    })
    .await
    .map_err(|error| format!("Nu am putut citi SVG-ul editabil: {error}"))?
}
