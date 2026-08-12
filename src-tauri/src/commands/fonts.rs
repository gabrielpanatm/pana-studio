use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::Serialize;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    commands::project::{
        require_current_project_root, require_project_workspace_available_for_write,
    },
    fonts::{
        annotate_font_preloads, build_font_face_graph, font_delivery_diagnostics,
        normalize_font_family_name, plan_google_font_family_download, prepare_font_display_update,
        prepare_font_preload_update, prepare_font_role_assignment, prepare_local_font_import,
        read_font_roles, remove_managed_font_face_block,
        search_google_fonts as search_google_fonts_impl, select_font_face_stylesheet,
        select_font_preload_template, upsert_managed_font_face_block, FontCssRegistration,
        FontDeliveryDiagnostic, FontDisplayMode, FontFaceGraph, FontOrigin, FontOwnership,
        FontRoleAssignment, FontRoleId, GoogleFontAxis, GoogleFontCatalogFamily,
        GoogleFontDownloadResult, LocalFontImportFamilyPlan, LocalFontImportPlan,
        LocalFontImportPrepared, LOCAL_FONT_IMPORT_SCHEMA_VERSION,
    },
    kernel::{
        file_buffer_store::hash_bytes,
        observability::now_ms,
        project_path::normalize_project_relative_path,
        project_workspace::{
            commit_project_workspace_session_mutation, ProjectWorkspaceIdentity,
            ProjectWorkspaceMutationReceipt, ProjectWorkspaceSnapshot,
            WorkspaceBinaryRestoreChange, WorkspaceMutationMetadata, WorkspaceProjectionSnapshot,
            WorkspaceResourceDelete, WorkspaceResourceMutation,
            PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES,
        },
    },
    project::{resolve_project_write_path, zola_project_root},
    state::AppState,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleFontInstallReceipt {
    pub result: GoogleFontDownloadResult,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFontImportReceipt {
    pub plan: LocalFontImportPlan,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontManagerSnapshot {
    pub schema_version: u32,
    pub graph: FontFaceGraph,
    pub roles: Vec<FontRoleAssignment>,
    pub diagnostics: Vec<FontDeliveryDiagnostic>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontRoleAssignmentReceipt {
    pub role: FontRoleAssignment,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
    pub manager: FontManagerSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontDeliveryMutationReceipt {
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
    pub manager: FontManagerSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontPreviewAsset {
    pub file: String,
    pub format: String,
    pub data_url: String,
    pub content_hash: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFamilyRemovalPlan {
    pub schema_version: u32,
    pub plan_token: String,
    pub family_id: String,
    pub family: String,
    pub directories: Vec<String>,
    pub files: Vec<String>,
    pub stylesheet_paths: Vec<String>,
    pub license_files: Vec<String>,
    pub retained_resources: Vec<String>,
    pub blocked_reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFamilyRemovalReceipt {
    pub plan: FontFamilyRemovalPlan,
    pub mutation: ProjectWorkspaceMutationReceipt,
    pub workspace: ProjectWorkspaceSnapshot,
    pub manager: FontManagerSnapshot,
}

struct BuiltLocalFontImportPlan {
    public: LocalFontImportPlan,
    stylesheet_after: String,
    binary_changes: Vec<WorkspaceBinaryRestoreChange>,
}

struct BuiltFontFamilyRemovalPlan {
    public: FontFamilyRemovalPlan,
    text_changes: Vec<WorkspaceResourceMutation>,
    text_deletes: Vec<WorkspaceResourceDelete>,
    binary_changes: Vec<WorkspaceBinaryRestoreChange>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FontFaceGraphCacheKey {
    project_root: String,
    runtime_session_id: String,
    workspace_revision: u64,
    disk_generation: u64,
}

static FONT_FACE_GRAPH_CACHE: OnceLock<Mutex<HashMap<FontFaceGraphCacheKey, FontFaceGraph>>> =
    OnceLock::new();

#[tauri::command]
pub async fn get_font_manager(
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
) -> Result<FontManagerSnapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (project_root, projection) = capture_font_workspace_read_projection(&app, &identity)?;
        let zola_root = zola_project_root(&project_root);
        let graph = font_face_graph_for_projection(&zola_root, &projection);
        let manager = font_manager_snapshot_for_projection(&projection, graph);
        validate_font_workspace_read_projection(&app, &identity, &project_root, &projection)?;
        Ok(manager)
    })
    .await
    .map_err(|error| format!("Font Manager a căzut în task-ul Rust de fundal: {error}"))?
}

#[tauri::command]
pub async fn get_font_preview_asset(
    file: String,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
) -> Result<FontPreviewAsset, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (project_root, projection) = capture_font_workspace_read_projection(&app, &identity)?;
        let zola_root = zola_project_root(&project_root);
        let graph = font_face_graph_for_projection(&zola_root, &projection);
        let font_file = graph
            .families
            .iter()
            .flat_map(|family| family.files.iter())
            .find(|candidate| candidate.file == file)
            .ok_or_else(|| format!("Fișierul {file} nu există în FontFaceGraph."))?;
        let bytes = if let Some(staged) = projection.resource_bytes.get(&file) {
            staged.to_vec()
        } else {
            let target = resolve_project_write_path(&project_root, &file)?;
            let metadata = fs::symlink_metadata(&target)
                .map_err(|error| format!("Nu am putut citi mostra {file}: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "Mostra {file} a fost refuzată: ținta nu este un fișier obișnuit."
                ));
            }
            fs::read(&target).map_err(|error| format!("Nu am putut citi mostra {file}: {error}"))?
        };
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(format!(
                "Mostra {} depășește limita de 8 MB pentru transferul către interfață.",
                font_file.file_name
            ));
        }
        let mime = match font_file.extension.as_str() {
            "woff2" => "font/woff2",
            "woff" => "font/woff",
            "ttf" => "font/ttf",
            "otf" => "font/otf",
            _ => "application/octet-stream",
        };
        let asset = FontPreviewAsset {
            file,
            format: font_file.format.clone(),
            data_url: format!("data:{mime};base64,{}", BASE64_STANDARD.encode(&bytes)),
            content_hash: hash_bytes(&bytes),
        };
        validate_font_workspace_read_projection(&app, &identity, &project_root, &projection)?;
        Ok(asset)
    })
    .await
    .map_err(|error| format!("Pregătirea mostrei fontului a căzut în task-ul de fundal: {error}"))?
}

#[tauri::command]
pub fn assign_font_role(
    role_id: String,
    family_id: String,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FontRoleAssignmentReceipt, String> {
    let project_root = require_current_project_root(&state)?;
    let zola_root = zola_project_root(&project_root);
    require_project_workspace_available_for_write(&state)?;
    let role = FontRoleId::parse(&role_id)?;
    let mut workspace_slot = state.project_workspace.lock().map_err(|_| {
        "Nu am putut bloca ProjectWorkspace pentru rolurile fonturilor.".to_string()
    })?;
    let workspace = workspace_slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    workspace.require_identity(&identity)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    let graph = font_face_graph_for_workspace(&zola_root, workspace);
    let installed = graph
        .families
        .iter()
        .find(|candidate| candidate.id == family_id)
        .ok_or_else(|| {
            format!("Identitatea {family_id} nu există în FontFaceGraph-ul proiectului sau temei active.")
        })?;
    if !installed.registration.registered {
        return Err(format!(
            "Familia {} are fișiere, dar nu are declarații @font-face detectate; rolul nu poate folosi un font neînregistrat.",
            installed.family
        ));
    }
    if installed.delivery == crate::fonts::FontDeliveryKind::Missing {
        return Err(format!(
            "Familia {} are surse nerezolvate și nu poate primi un rol semantic.",
            installed.family
        ));
    }
    let (source_path, source_after, _) = prepare_font_role_assignment(
        workspace
            .documents
            .files
            .iter()
            .map(|(path, entry)| (path.as_str(), entry.current_text())),
        role,
        &installed.family,
    )?;
    let mutation = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.stage_project_bundle_changes(
            &identity,
            WorkspaceMutationMetadata {
                label: format!("Atribuie {} rolului {}", installed.family, role_id),
                source: "project_workspace.font_role.assign".to_string(),
                coalesce_key: None,
                transaction_id: None,
            },
            vec![WorkspaceResourceMutation {
                relative_path: source_path,
                contents: source_after,
                create_only: false,
            }],
            Vec::new(),
            Vec::new(),
            now_ms(),
        )
    })?;
    let graph = font_face_graph_for_workspace(&zola_root, workspace);
    let manager = font_manager_snapshot(workspace, graph);
    let assigned = manager
        .roles
        .iter()
        .find(|assignment| assignment.id == role)
        .cloned()
        .ok_or_else(|| "Rolul atribuit a dispărut din snapshot-ul Font Manager.".to_string())?;
    Ok(FontRoleAssignmentReceipt {
        role: assigned,
        mutation,
        workspace: workspace.snapshot(),
        manager,
    })
}

#[tauri::command]
pub fn set_font_display(
    family_id: String,
    display: String,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FontDeliveryMutationReceipt, String> {
    let project_root = require_current_project_root(&state)?;
    let zola_root = zola_project_root(&project_root);
    require_project_workspace_available_for_write(&state)?;
    let display = FontDisplayMode::parse(&display)?;
    let mut workspace_slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru font-display.".to_string())?;
    let workspace = workspace_slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    workspace.require_identity(&identity)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    let graph = font_face_graph_for_workspace(&zola_root, workspace);
    let installed = graph
        .families
        .iter()
        .find(|candidate| candidate.id == family_id)
        .ok_or_else(|| {
            format!("Identitatea {family_id} nu există în FontFaceGraph-ul proiectului.")
        })?;
    if !installed.registration.managed {
        return Err(format!(
            "Politica font-display pentru {} nu poate fi modificată: declarațiile @font-face nu sunt gestionate de Pană Studio.",
            installed.family
        ));
    }
    let (source_path, source_after) = prepare_font_display_update(
        workspace
            .documents
            .files
            .iter()
            .map(|(path, entry)| (path.as_str(), entry.current_text())),
        &installed.family,
        display,
    )?;
    let source_before = workspace
        .documents
        .text_for(&source_path)
        .ok_or_else(|| format!("ProjectWorkspace nu mai urmărește {source_path}."))?;
    if source_before == source_after {
        return Err(format!(
            "{} folosește deja font-display: {}.",
            installed.family,
            display.as_str()
        ));
    }
    let family_name = installed.family.clone();
    let mutation = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.stage_project_bundle_changes(
            &identity,
            WorkspaceMutationMetadata {
                label: format!(
                    "Setează font-display {} pentru {}",
                    display.as_str(),
                    family_name
                ),
                source: "project_workspace.font_delivery.display".to_string(),
                coalesce_key: None,
                transaction_id: None,
            },
            vec![WorkspaceResourceMutation {
                relative_path: source_path,
                contents: source_after,
                create_only: false,
            }],
            Vec::new(),
            Vec::new(),
            now_ms(),
        )
    })?;
    let graph = font_face_graph_for_workspace(&zola_root, workspace);
    Ok(FontDeliveryMutationReceipt {
        mutation,
        workspace: workspace.snapshot(),
        manager: font_manager_snapshot(workspace, graph),
    })
}

#[tauri::command]
pub fn set_font_preload(
    file: String,
    enabled: bool,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
    state: State<AppState>,
) -> Result<FontDeliveryMutationReceipt, String> {
    let project_root = require_current_project_root(&state)?;
    let zola_root = zola_project_root(&project_root);
    require_project_workspace_available_for_write(&state)?;
    let mut workspace_slot = state.project_workspace.lock().map_err(|_| {
        "Nu am putut bloca ProjectWorkspace pentru preload-ul fontului.".to_string()
    })?;
    let workspace = workspace_slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    workspace.require_identity(&identity)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    let graph = font_face_graph_for_workspace(&zola_root, workspace);
    let (family, target) = graph
        .families
        .iter()
        .find_map(|family| {
            family
                .files
                .iter()
                .find(|candidate| candidate.file == file)
                .map(|target| (family, target))
        })
        .ok_or_else(|| format!("Fișierul {file} nu există în biblioteca Rust a proiectului."))?;
    if enabled && !family.registration.registered {
        return Err(format!(
            "{} nu poate fi preîncărcat înainte ca familia {} să aibă @font-face.",
            target.file_name, family.family
        ));
    }
    let template_path = select_font_preload_template(
        workspace
            .documents
            .files
            .iter()
            .map(|(path, entry)| (path.as_str(), entry.current_text())),
    )
    .ok_or_else(|| {
        "Font Manager nu a găsit un template HTML urmărit care conține </head> pentru blocul preload."
            .to_string()
    })?;
    let template_source = workspace
        .documents
        .text_for(&template_path)
        .ok_or_else(|| format!("ProjectWorkspace nu mai urmărește {template_path}."))?;
    let template_after =
        prepare_font_preload_update(&template_source, &graph.families, &file, enabled)?;
    if template_after == template_source {
        return Err(format!(
            "Preload-ul pentru {} este deja {}.",
            target.file_name,
            if enabled { "activ" } else { "inactiv" }
        ));
    }
    let file_name = target.file_name.clone();
    let mutation = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
        candidate.stage_project_bundle_changes(
            &identity,
            WorkspaceMutationMetadata {
                label: format!(
                    "{} preload pentru {}",
                    if enabled {
                        "Activează"
                    } else {
                        "Dezactivează"
                    },
                    file_name
                ),
                source: "project_workspace.font_delivery.preload".to_string(),
                coalesce_key: None,
                transaction_id: None,
            },
            vec![WorkspaceResourceMutation {
                relative_path: template_path,
                contents: template_after,
                create_only: false,
            }],
            Vec::new(),
            Vec::new(),
            now_ms(),
        )
    })?;
    let graph = font_face_graph_for_workspace(&zola_root, workspace);
    Ok(FontDeliveryMutationReceipt {
        mutation,
        workspace: workspace.snapshot(),
        manager: font_manager_snapshot(workspace, graph),
    })
}

#[tauri::command]
pub async fn plan_font_family_removal(
    family_id: String,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
) -> Result<FontFamilyRemovalPlan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let project_root = require_font_workspace_identity(&app, &identity)?;
        let state = app.state::<AppState>();
        let workspace_slot = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru planul de eliminare a fontului.".to_string()
        })?;
        let workspace = workspace_slot
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
        workspace.require_identity(&identity)?;
        Ok(build_font_family_removal_plan(workspace, &project_root, &family_id, &identity)?.public)
    })
    .await
    .map_err(|error| {
        format!("Planificarea eliminării fontului a căzut în task-ul de fundal: {error}")
    })?
}

#[tauri::command]
pub async fn remove_font_family(
    family_id: String,
    expected_plan_token: String,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
) -> Result<FontFamilyRemovalReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let project_root = require_current_project_root(&state)?;
        let zola_root = zola_project_root(&project_root);
        require_project_workspace_available_for_write(&state)?;
        let mut workspace_slot = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru eliminarea fontului.".to_string()
        })?;
        let workspace = workspace_slot
            .as_mut()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
        workspace.require_identity(&identity)?;
        workspace.accepted_disk.require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            &project_root,
        )?;
        let built =
            build_font_family_removal_plan(workspace, &project_root, &family_id, &identity)?;
        if built.public.plan_token != expected_plan_token {
            return Err(
                "Planul eliminării fontului este stale. Reanalizează familia înainte de confirmare."
                    .to_string(),
            );
        }
        if !built.public.blocked_reasons.is_empty() {
            return Err(format!(
                "Eliminarea familiei este blocată:\n{}",
                built.public.blocked_reasons.join("\n")
            ));
        }
        if !built.public.changed {
            return Err(
                "Planul nu conține resurse administrate care pot fi eliminate.".to_string(),
            );
        }
        let family_name = built.public.family.clone();
        let mutation = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
            candidate.stage_project_bundle_changes(
                &identity,
                WorkspaceMutationMetadata {
                    label: format!("Elimină fontul {family_name}"),
                    source: "project_workspace.font_remove.family".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                built.text_changes,
                built.text_deletes,
                built.binary_changes,
                now_ms(),
            )
        })?;
        let graph = font_face_graph_for_workspace(&zola_root, workspace);
        Ok(FontFamilyRemovalReceipt {
            plan: built.public,
            mutation,
            workspace: workspace.snapshot(),
            manager: font_manager_snapshot(workspace, graph),
        })
    })
    .await
    .map_err(|error| format!("Eliminarea fontului a căzut în task-ul de fundal: {error}"))?
}

fn capture_font_workspace_read_projection<R: Runtime>(
    app: &AppHandle<R>,
    identity: &ProjectWorkspaceIdentity,
) -> Result<(std::path::PathBuf, WorkspaceProjectionSnapshot), String> {
    let state = app.state::<AppState>();
    let project_root = require_current_project_root(&state)?;
    let projection = {
        let workspace_slot = state.project_workspace.lock().map_err(|_| {
            "Nu am putut captura ProjectWorkspace pentru citirea fonturilor.".to_string()
        })?;
        let workspace = workspace_slot
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
        if workspace.session.project_root != project_root.to_string_lossy() {
            return Err(
                "Citirea fonturilor a refuzat un ProjectWorkspace din alt proiect.".to_string(),
            );
        }
        workspace.require_identity(identity)?;
        workspace.accepted_disk.require_identity(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
        )?;
        workspace.accepted_disk.require_complete()?;
        workspace.capture_projection_snapshot()?
    };

    // Traversarea fonturilor și auditul manifestului sunt filesystem work.
    // Proiecția imutabilă păstrează autoritatea exactă fără a bloca mutațiile
    // Workbench/ProjectWorkspace pe durata acestor operații.
    projection.accepted_disk.require_live_complete(
        &projection.runtime_session_id,
        &projection.project_root,
        &project_root,
    )?;
    Ok((project_root, projection))
}

fn validate_font_workspace_read_projection<R: Runtime>(
    app: &AppHandle<R>,
    identity: &ProjectWorkspaceIdentity,
    project_root: &Path,
    projection: &WorkspaceProjectionSnapshot,
) -> Result<(), String> {
    projection.accepted_disk.require_live_complete(
        &projection.runtime_session_id,
        &projection.project_root,
        project_root,
    )?;

    let state = app.state::<AppState>();
    if require_current_project_root(&state)? != project_root {
        return Err(
            "Citirea fonturilor a devenit stale: proiectul activ s-a schimbat.".to_string(),
        );
    }
    let workspace_slot = state.project_workspace.lock().map_err(|_| {
        "Nu am putut revalida ProjectWorkspace pentru citirea fonturilor.".to_string()
    })?;
    let workspace = workspace_slot.as_ref().ok_or_else(|| {
        "Citirea fonturilor a devenit stale: ProjectWorkspace a fost închis.".to_string()
    })?;
    workspace.require_identity(identity)?;
    if workspace.accepted_disk != projection.accepted_disk {
        return Err(
            "Citirea fonturilor a devenit stale: autoritatea disk s-a schimbat.".to_string(),
        );
    }
    Ok(())
}

fn font_face_graph_for_workspace(
    root: &Path,
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
) -> FontFaceGraph {
    let key = FontFaceGraphCacheKey {
        project_root: workspace.session.project_root.clone(),
        runtime_session_id: workspace.runtime_session_id(),
        workspace_revision: workspace.revision,
        disk_generation: workspace.accepted_disk.generation,
    };
    if let Some(graph) = cached_font_face_graph(&key) {
        return graph;
    }
    let documents = workspace
        .documents
        .files
        .iter()
        .map(|(path, entry)| (path.as_str(), entry.current_text()))
        .collect::<Vec<_>>();
    let mut graph = build_font_face_graph(
        root,
        documents.iter().copied(),
        workspace.staged_binary_resources(),
        workspace.deleted_binary_resources(),
        workspace
            .accepted_disk
            .manifest
            .files
            .iter()
            .map(|entry| (entry.relative_path.as_str(), entry.version_token.as_str())),
    );
    graph.families = annotate_font_preloads(graph.families, documents.iter().copied());
    publish_font_face_graph_cache(key, &graph);
    graph
}

fn font_face_graph_for_projection(
    root: &Path,
    projection: &WorkspaceProjectionSnapshot,
) -> FontFaceGraph {
    let key = FontFaceGraphCacheKey {
        project_root: projection.project_root.clone(),
        runtime_session_id: projection.runtime_session_id.clone(),
        workspace_revision: projection.revision,
        disk_generation: projection.accepted_disk.generation,
    };
    if let Some(graph) = cached_font_face_graph(&key) {
        return graph;
    }
    let documents = projection
        .source_texts
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect::<Vec<_>>();
    let mut graph = build_font_face_graph(
        root,
        documents.iter().copied(),
        projection
            .resource_bytes
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
        projection.deleted_sources.iter().map(String::as_str),
        projection
            .accepted_disk
            .manifest
            .files
            .iter()
            .map(|entry| (entry.relative_path.as_str(), entry.version_token.as_str())),
    );
    graph.families = annotate_font_preloads(graph.families, documents.iter().copied());
    publish_font_face_graph_cache(key, &graph);
    graph
}

fn cached_font_face_graph(key: &FontFaceGraphCacheKey) -> Option<FontFaceGraph> {
    FONT_FACE_GRAPH_CACHE
        .get_or_init(Default::default)
        .lock()
        .ok()
        .and_then(|cache| cache.get(key).cloned())
}

fn publish_font_face_graph_cache(key: FontFaceGraphCacheKey, graph: &FontFaceGraph) {
    if let Ok(mut cache) = FONT_FACE_GRAPH_CACHE.get_or_init(Default::default).lock() {
        if cache.len() >= 16 && !cache.contains_key(&key) {
            cache.clear();
        }
        cache.insert(key, graph.clone());
    }
}

fn font_manager_snapshot(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
    graph: FontFaceGraph,
) -> FontManagerSnapshot {
    font_manager_snapshot_for_sources(
        graph,
        workspace
            .documents
            .files
            .iter()
            .map(|(path, entry)| (path.as_str(), entry.current_text())),
    )
}

fn font_manager_snapshot_for_projection(
    projection: &WorkspaceProjectionSnapshot,
    graph: FontFaceGraph,
) -> FontManagerSnapshot {
    font_manager_snapshot_for_sources(
        graph,
        projection
            .source_texts
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    )
}

fn font_manager_snapshot_for_sources<'a>(
    graph: FontFaceGraph,
    documents: impl Iterator<Item = (&'a str, &'a str)>,
) -> FontManagerSnapshot {
    let roles = read_font_roles(documents, &graph.families);
    let diagnostics = font_delivery_diagnostics(&graph.families, &roles);
    FontManagerSnapshot {
        schema_version: 4,
        graph,
        roles,
        diagnostics,
    }
}

#[tauri::command]
pub async fn plan_local_font_import(
    source_paths: Vec<String>,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
) -> Result<LocalFontImportPlan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let project_root = require_font_workspace_identity(&app, &identity)?;
        let prepared = prepare_local_font_import(source_paths)?;
        let state = app.state::<AppState>();
        let workspace_slot = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru planul fonturilor.".to_string()
        })?;
        let workspace = workspace_slot
            .as_ref()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
        workspace.require_identity(&identity)?;
        workspace.accepted_disk.require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            &project_root,
        )?;
        Ok(build_local_font_import_plan(prepared, workspace, &project_root, &identity)?.public)
    })
    .await
    .map_err(|error| format!("Planificarea fonturilor a căzut în task-ul de fundal: {error}"))?
}

#[tauri::command]
pub async fn apply_local_font_import(
    source_paths: Vec<String>,
    expected_plan_token: String,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
) -> Result<LocalFontImportReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let project_root = require_font_workspace_identity(&app, &identity)?;
        let prepared = prepare_local_font_import(source_paths)?;
        let state = app.state::<AppState>();
        let mut workspace_slot = state.project_workspace.lock().map_err(|_| {
            "Nu am putut bloca ProjectWorkspace pentru importul fonturilor.".to_string()
        })?;
        let workspace = workspace_slot
            .as_mut()
            .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
        workspace.require_identity(&identity)?;
        workspace.accepted_disk.require_live_complete(
            &workspace.runtime_session_id(),
            &workspace.session.project_root,
            &project_root,
        )?;
        let built = build_local_font_import_plan(prepared, workspace, &project_root, &identity)?;
        if built.public.plan_token != expected_plan_token {
            return Err(
                "Planul importului local este stale. Reanalizează fișierele înainte de confirmare."
                    .to_string(),
            );
        }
        if !built.public.conflicts.is_empty() {
            return Err(format!(
                "Importul local este blocat de conflicte:\n{}",
                built.public.conflicts.join("\n")
            ));
        }
        let stylesheet_source = workspace
            .documents
            .text_for(&built.public.stylesheet_path)
            .ok_or_else(|| {
                format!(
                    "ProjectWorkspace nu mai urmărește stylesheet-ul {}.",
                    built.public.stylesheet_path
                )
            })?;
        let text_changes = (built.stylesheet_after != stylesheet_source)
            .then(|| WorkspaceResourceMutation {
                relative_path: built.public.stylesheet_path.clone(),
                contents: built.stylesheet_after.clone(),
                create_only: false,
            })
            .into_iter()
            .collect::<Vec<_>>();
        let family_names = built
            .public
            .families
            .iter()
            .map(|family| family.family.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let mutation = commit_project_workspace_session_mutation(&app, workspace, |candidate| {
            candidate.stage_project_bundle_changes(
                &identity,
                WorkspaceMutationMetadata {
                    label: format!("Importă fonturile {family_names}"),
                    source: "project_workspace.font_install.local".to_string(),
                    coalesce_key: None,
                    transaction_id: None,
                },
                text_changes,
                Vec::new(),
                built.binary_changes,
                now_ms(),
            )
        })?;
        Ok(LocalFontImportReceipt {
            plan: built.public,
            mutation,
            workspace: workspace.snapshot(),
        })
    })
    .await
    .map_err(|error| format!("Importul fonturilor a căzut în task-ul de fundal: {error}"))?
}

// Tauri derives the existing font-install IPC keys from this flat signature.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn download_google_font_family(
    family: String,
    weights: Vec<u16>,
    styles: Vec<String>,
    variable: bool,
    axes: Vec<GoogleFontAxis>,
    character_set: Option<String>,
    identity: ProjectWorkspaceIdentity,
    app: AppHandle,
) -> Result<GoogleFontInstallReceipt, String> {
    tauri::async_runtime::spawn_blocking(move || {
        download_google_font_family_blocking(
            &app,
            family,
            weights,
            styles,
            variable,
            axes,
            character_set,
            identity,
        )
    })
    .await
    .map_err(|error| format!("Instalarea fontului a căzut în task-ul de fundal: {error}"))?
}

// The blocking adapter mirrors the stable IPC contract and only adds the native app handle.
#[allow(clippy::too_many_arguments)]
fn download_google_font_family_blocking<R: Runtime>(
    app: &AppHandle<R>,
    family: String,
    weights: Vec<u16>,
    styles: Vec<String>,
    variable: bool,
    axes: Vec<GoogleFontAxis>,
    character_set: Option<String>,
    identity: ProjectWorkspaceIdentity,
) -> Result<GoogleFontInstallReceipt, String> {
    let state = app.state::<AppState>();
    let project_root = require_current_project_root(&state)?;
    require_project_workspace_available_for_write(&state)?;
    let mut plan = plan_google_font_family_download(
        &family,
        &weights,
        &styles,
        variable,
        &axes,
        character_set.as_deref(),
    )?;
    let mut workspace_slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru fonturi.".to_string())?;
    let workspace = workspace_slot
        .as_mut()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    if workspace.session.project_root != project_root.to_string_lossy() {
        return Err("Font Manager a refuzat un ProjectWorkspace din alt proiect.".to_string());
    }
    workspace.require_identity(&identity)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;

    let stylesheet_path = select_font_face_stylesheet(
        workspace
            .documents
            .files
            .iter()
            .map(|(path, entry)| (path.as_str(), entry.current_text())),
    )
    .ok_or_else(|| {
        "Font Manager nu a găsit un stylesheet de bază urmărit de ProjectWorkspace pentru @font-face."
            .to_string()
    })?;
    let stylesheet_source = workspace
        .documents
        .text_for(&stylesheet_path)
        .ok_or_else(|| {
            format!(
                "ProjectWorkspace nu mai urmărește stylesheet-ul ales pentru fonturi: {stylesheet_path}."
            )
        })?;
    let stylesheet_after = upsert_managed_font_face_block(
        &stylesheet_source,
        &plan.result.family.family,
        &plan.result.font_face_css,
    )?;
    let mut text_changes = (stylesheet_after != stylesheet_source)
        .then(|| WorkspaceResourceMutation {
            relative_path: stylesheet_path.clone(),
            contents: stylesheet_after,
            create_only: false,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let current_license = workspace.documents.text_for(&plan.result.license_file);
    if current_license.as_deref() != Some(plan.license_text.as_str()) {
        text_changes.push(WorkspaceResourceMutation {
            relative_path: plan.result.license_file.clone(),
            contents: plan.license_text.clone(),
            create_only: false,
        });
    }

    let binary_changes = plan
        .writes
        .iter()
        .filter_map(|write| {
            match font_resource_already_available(
                workspace,
                &project_root,
                &write.project_relative_path,
                &write.bytes,
            ) {
                Ok(true) => None,
                Ok(false) => Some(Ok(WorkspaceBinaryRestoreChange {
                    relative_path: write.project_relative_path.clone(),
                    before: None,
                    after: Some(write.bytes.clone()),
                })),
                Err(error) => Some(Err(error)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mutation = commit_project_workspace_session_mutation(app, workspace, |candidate| {
        candidate.stage_project_bundle_changes(
            &identity,
            WorkspaceMutationMetadata {
                label: format!("Instalează fontul {}", plan.result.family.family),
                source: "project_workspace.font_install.google".to_string(),
                coalesce_key: None,
                transaction_id: None,
            },
            text_changes,
            Vec::new(),
            binary_changes,
            now_ms(),
        )
    })?;

    plan.result.family.registration = FontCssRegistration {
        registered: true,
        managed: true,
        stylesheets: vec![stylesheet_path],
        display_modes: vec!["swap".to_string()],
    };
    Ok(GoogleFontInstallReceipt {
        result: plan.result,
        mutation,
        workspace: workspace.snapshot(),
    })
}

#[tauri::command]
pub async fn search_google_fonts(
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<GoogleFontCatalogFamily>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        search_google_fonts_impl(&query, limit.unwrap_or(40), offset.unwrap_or(0))
    })
    .await
    .map_err(|error| format!("Căutarea Google Fonts a căzut în task-ul de fundal: {error}"))?
}

fn font_resource_already_available(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
    project_root: &Path,
    relative_path: &str,
    bytes: &[u8],
) -> Result<bool, String> {
    if workspace
        .deleted_binary_resources()
        .any(|deleted| deleted == relative_path)
    {
        return Err(format!(
            "Font Manager a refuzat {relative_path}: resursa este marcată pentru ștergere în sesiunea curentă."
        ));
    }
    if let Some(staged) = workspace.staged_binary_resource(relative_path) {
        if hash_bytes(staged) == hash_bytes(bytes) {
            return Ok(true);
        }
        return Err(format!(
            "Font Manager a refuzat resursa staged divergentă {}.",
            relative_path
        ));
    }
    let target = resolve_project_write_path(project_root, relative_path)?;
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "Font Manager a blocat scrierea: {} este symlink.",
            relative_path
        )),
        Ok(metadata) if metadata.is_dir() => Err(format!(
            "Font Manager a blocat scrierea: {} este director.",
            relative_path
        )),
        Ok(_) => {
            let existing = fs::read(&target).map_err(|error| {
                format!(
                    "Nu am putut citi fontul existent {} înainte de conflict check: {}",
                    relative_path, error
                )
            })?;
            if hash_bytes(&existing) == hash_bytes(bytes) {
                return Ok(true);
            }
            Err(format!(
                "Font Manager a blocat suprascrierea fontului existent {}. Șterge sau redenumește fișierul înainte de re-descărcare.",
                relative_path
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "Nu am putut verifica fontul {} înainte de scriere: {}",
            relative_path, error
        )),
    }
}

fn require_font_workspace_identity<R: Runtime>(
    app: &AppHandle<R>,
    identity: &ProjectWorkspaceIdentity,
) -> Result<std::path::PathBuf, String> {
    let state = app.state::<AppState>();
    let project_root = require_current_project_root(&state)?;
    require_project_workspace_available_for_write(&state)?;
    let workspace_slot = state
        .project_workspace
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectWorkspace pentru Font Manager.".to_string())?;
    let workspace = workspace_slot
        .as_ref()
        .ok_or_else(|| "ProjectWorkspace nu este inițializat.".to_string())?;
    if workspace.session.project_root != project_root.to_string_lossy() {
        return Err("Font Manager a refuzat un ProjectWorkspace din alt proiect.".to_string());
    }
    workspace.require_identity(identity)?;
    workspace.accepted_disk.require_live_complete(
        &workspace.runtime_session_id(),
        &workspace.session.project_root,
        &project_root,
    )?;
    Ok(project_root)
}

fn build_local_font_import_plan(
    mut prepared: LocalFontImportPrepared,
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
    project_root: &Path,
    identity: &ProjectWorkspaceIdentity,
) -> Result<BuiltLocalFontImportPlan, String> {
    let stylesheet_path = select_font_face_stylesheet(
        workspace
            .documents
            .files
            .iter()
            .map(|(path, entry)| (path.as_str(), entry.current_text())),
    )
    .ok_or_else(|| {
        "Font Manager nu a găsit un stylesheet de bază urmărit de ProjectWorkspace pentru @font-face."
            .to_string()
    })?;
    let stylesheet_source = workspace
        .documents
        .text_for(&stylesheet_path)
        .ok_or_else(|| {
            format!(
                "ProjectWorkspace nu mai urmărește stylesheet-ul ales pentru fonturi: {stylesheet_path}."
            )
        })?;
    let mut stylesheet_after = stylesheet_source.clone();
    for family in &prepared.families {
        stylesheet_after = upsert_managed_font_face_block(
            &stylesheet_after,
            &family.family.family,
            &family.font_face_css,
        )?;
    }

    let mut binary_changes = Vec::new();
    for file in &prepared.files {
        match font_resource_already_available(
            workspace,
            project_root,
            &file.plan.destination_path,
            &file.bytes,
        ) {
            Ok(true) => prepared.warnings.push(format!(
                "{} există deja cu același conținut și nu va fi duplicat.",
                file.plan.destination_path
            )),
            Ok(false) => binary_changes.push(WorkspaceBinaryRestoreChange {
                relative_path: file.plan.destination_path.clone(),
                before: None,
                after: Some(file.bytes.clone()),
            }),
            Err(error) => prepared.conflicts.push(error),
        }
    }
    prepared.warnings.sort();
    prepared.warnings.dedup();
    prepared.conflicts.sort();
    prepared.conflicts.dedup();

    let mut token_material = format!(
        "{}|{}|{}|{}|{}",
        identity.expected_project_root,
        identity.expected_session_id,
        identity.expected_revision,
        stylesheet_path,
        hash_bytes(stylesheet_source.as_bytes())
    );
    for file in &prepared.files {
        token_material.push('|');
        token_material.push_str(&file.plan.destination_path);
        token_material.push(':');
        token_material.push_str(&file.content_hash);
    }
    for conflict in &prepared.conflicts {
        token_material.push('|');
        token_material.push_str(conflict);
    }
    let plan_token = hash_bytes(token_material.as_bytes());
    let families = prepared
        .families
        .iter()
        .map(|family| LocalFontImportFamilyPlan {
            id: family.family.id.clone(),
            family: family.family.family.clone(),
            directory: family
                .family
                .directories
                .first()
                .cloned()
                .unwrap_or_default(),
            file_count: family.family.files.len(),
            variable: family.family.files.iter().any(|file| !file.axes.is_empty()),
            license: family.family.license.clone(),
        })
        .collect();
    let files = prepared
        .files
        .iter()
        .map(|file| file.plan.clone())
        .collect();
    let changed = stylesheet_after != stylesheet_source || !binary_changes.is_empty();

    Ok(BuiltLocalFontImportPlan {
        public: LocalFontImportPlan {
            schema_version: LOCAL_FONT_IMPORT_SCHEMA_VERSION,
            plan_token,
            stylesheet_path,
            families,
            files,
            warnings: prepared.warnings,
            conflicts: prepared.conflicts,
            changed,
        },
        stylesheet_after,
        binary_changes,
    })
}

fn build_font_family_removal_plan(
    workspace: &crate::kernel::project_workspace::ProjectWorkspace,
    project_root: &Path,
    family_id: &str,
    identity: &ProjectWorkspaceIdentity,
) -> Result<BuiltFontFamilyRemovalPlan, String> {
    let zola_root = zola_project_root(project_root);
    let graph = font_face_graph_for_workspace(&zola_root, workspace);
    let installed = graph
        .families
        .iter()
        .find(|candidate| candidate.origin == FontOrigin::Local && candidate.id == family_id)
        .cloned()
        .ok_or_else(|| {
            format!("Familia locală cu identitatea {family_id} nu mai există în FontFaceGraph.")
        })?;
    let mut files = installed
        .files
        .iter()
        .filter(|file| file.file.starts_with("static/fonturi/"))
        .map(|file| normalize_project_relative_path(&file.file))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Err(format!(
            "Familia {} nu are resurse locale gestionabile în static/fonturi.",
            installed.family
        ));
    }
    let target_files = files.iter().cloned().collect::<HashSet<_>>();
    let mut directories = files
        .iter()
        .filter_map(|file| Path::new(file).parent())
        .map(|directory| normalize_project_relative_path(&directory.to_string_lossy()))
        .collect::<Result<Vec<_>, _>>()?;
    directories.sort();
    directories.dedup();
    if directories
        .iter()
        .any(|directory| !directory.starts_with("static/fonturi/"))
    {
        return Err(format!(
            "Font Manager a refuzat familia {}: numai resursele locale din static/fonturi pot fi eliminate.",
            installed.family
        ));
    }
    let retained_delivery = installed.faces.iter().any(|face| {
        face.ownership == FontOwnership::Detected
            && face.delivery != crate::fonts::FontDeliveryKind::Missing
            && face
                .resolved_file
                .as_ref()
                .is_none_or(|file| !target_files.contains(file))
    });
    let roles = read_font_roles(
        workspace
            .documents
            .files
            .iter()
            .map(|(path, entry)| (path.as_str(), entry.current_text())),
        &graph.families,
    );
    let mut blocked_reasons = Vec::new();
    let used_roles = roles
        .iter()
        .filter(|role| {
            role.family.as_deref().is_some_and(|assigned| {
                normalize_font_family_name(assigned)
                    == normalize_font_family_name(&installed.family)
            })
        })
        .map(|role| role.label.clone())
        .collect::<Vec<_>>();
    if !used_roles.is_empty() && !retained_delivery {
        blocked_reasons.push(format!(
            "Familia este încă atribuită rolurilor: {}. Atribuie mai întâi alte fonturi.",
            used_roles.join(", ")
        ));
    }
    let preloaded_files = installed
        .files
        .iter()
        .filter(|file| target_files.contains(&file.file) && file.preload.preloaded)
        .map(|file| file.file_name.clone())
        .collect::<Vec<_>>();
    if !preloaded_files.is_empty() {
        blocked_reasons.push(format!(
            "Dezactivează preload-ul pentru: {}.",
            preloaded_files.join(", ")
        ));
    }
    let managed_stylesheet_candidates = installed
        .faces
        .iter()
        .filter(|face| {
            face.ownership == FontOwnership::Managed
                && face
                    .resolved_file
                    .as_ref()
                    .is_some_and(|file| target_files.contains(file))
        })
        .map(|face| face.stylesheet.clone())
        .collect::<BTreeSet<_>>();
    let detected_local_references = installed
        .faces
        .iter()
        .filter(|face| {
            face.ownership == FontOwnership::Detected
                && face
                    .resolved_file
                    .as_ref()
                    .is_some_and(|file| target_files.contains(file))
        })
        .map(|face| face.stylesheet.clone())
        .collect::<BTreeSet<_>>();
    if !detected_local_references.is_empty() {
        blocked_reasons.push(format!(
            "Fișierele locale sunt încă referite de declarații @font-face detectate, dar negestionate, în: {}.",
            detected_local_references.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if managed_stylesheet_candidates.is_empty() {
        blocked_reasons.push(
            "Declarațiile @font-face nu sunt gestionate de Pană Studio; eliminarea automată ar putea rupe CSS extern."
                .to_string(),
        );
    }

    let mut text_changes = Vec::new();
    let mut managed_stylesheets = Vec::new();
    for stylesheet in managed_stylesheet_candidates {
        let Some(source) = workspace.documents.text_for(&stylesheet) else {
            blocked_reasons.push(format!(
                "Stylesheet-ul {stylesheet} nu mai este urmărit de ProjectWorkspace."
            ));
            continue;
        };
        match remove_managed_font_face_block(&source, &installed.family) {
            Ok(contents) => {
                managed_stylesheets.push(stylesheet.clone());
                if contents != source {
                    text_changes.push(WorkspaceResourceMutation {
                        relative_path: stylesheet.clone(),
                        contents,
                        create_only: false,
                    });
                }
            }
            Err(error) => blocked_reasons.push(format!(
                "Blocul @font-face gestionat din {stylesheet} nu mai poate fi eliminat: {error}"
            )),
        }
    }
    if managed_stylesheets.is_empty() {
        blocked_reasons.push(
            "Blocul @font-face administrat nu mai poate fi identificat în stylesheet.".to_string(),
        );
    }
    if files.iter().any(|file| {
        !directories
            .iter()
            .any(|directory| file.starts_with(&format!("{directory}/")))
    }) {
        return Err(format!(
            "FontFaceGraph conține un fișier al familiei {} în afara directoarelor locale demonstrate.",
            installed.family
        ));
    }

    let directory_prefixes = directories
        .iter()
        .map(|directory| format!("{directory}/"))
        .collect::<Vec<_>>();
    let mut all_directory_resources = workspace
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.clone())
        .chain(workspace.documents.files.keys().cloned())
        .chain(
            workspace
                .staged_binary_resources()
                .map(|(path, _)| path.to_string()),
        )
        .filter(|path| {
            directory_prefixes
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<HashSet<_>>();
    let shared_directories = graph
        .families
        .iter()
        .filter(|candidate| candidate.id != installed.id)
        .flat_map(|candidate| candidate.files.iter())
        .filter_map(|file| Path::new(&file.file).parent())
        .map(|directory| directory.to_string_lossy().replace('\\', "/"))
        .collect::<HashSet<_>>();
    let mut license_files = all_directory_resources
        .iter()
        .filter(|path| {
            directories.iter().any(|directory| {
                !shared_directories.contains(directory)
                    && is_managed_font_license_path(path, directory)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    license_files.sort();
    license_files.dedup();
    for removed in files.iter().chain(license_files.iter()) {
        all_directory_resources.remove(removed);
    }
    let mut retained_resources = all_directory_resources.into_iter().collect::<Vec<_>>();
    retained_resources.sort();
    let mut warnings = Vec::new();
    if !retained_resources.is_empty() {
        warnings.push(format!(
            "{} resurse necunoscute din director vor fi păstrate.",
            retained_resources.len()
        ));
    }

    let accepted_paths = workspace
        .accepted_disk
        .manifest
        .files
        .iter()
        .map(|entry| entry.relative_path.as_str())
        .collect::<HashSet<_>>();
    let staged_only_files = files
        .iter()
        .filter(|file| {
            !accepted_paths.contains(file.as_str())
                && workspace.staged_binary_resource(file).is_some()
        })
        .cloned()
        .collect::<Vec<_>>();
    if !staged_only_files.is_empty() {
        blocked_reasons.push(format!(
            "Familia conține {} fișiere nou instalate, încă nesalvate. Salvează proiectul înainte de dezinstalare sau folosește Undo pentru a anula instalarea.",
            staged_only_files.len()
        ));
    }
    let untracked_license_files = license_files
        .iter()
        .filter(|path| !workspace.documents.files.contains_key(*path))
        .cloned()
        .collect::<Vec<_>>();
    if !untracked_license_files.is_empty() {
        blocked_reasons.push(format!(
            "Licențele nu sunt urmărite ca documente text de ProjectWorkspace: {}.",
            untracked_license_files.join(", ")
        ));
    }
    let mut binary_changes = Vec::with_capacity(files.len());
    for file in &files {
        let before = if accepted_paths.contains(file.as_str()) {
            let target = resolve_project_write_path(project_root, file)?;
            let metadata = fs::symlink_metadata(&target).map_err(|error| {
                format!("Nu am putut valida {file} înainte de eliminare: {error}")
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "Eliminarea a fost refuzată: {file} nu este un fișier obișnuit."
                ));
            }
            if metadata.len() > PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES {
                return Err(format!(
                    "Eliminarea a fost refuzată: {file} depășește limita de {} bytes a istoricului binar.",
                    PROJECT_WORKSPACE_MAX_BINARY_RESOURCE_BYTES
                ));
            }
            Some(
                fs::read(&target).map_err(|error| {
                    format!("Nu am putut citi baseline-ul binar {file}: {error}")
                })?,
            )
        } else if workspace.staged_binary_resource(file).is_some() {
            None
        } else {
            return Err(format!(
                "ProjectWorkspace nu mai poate demonstra proveniența fișierului {file}."
            ));
        };
        binary_changes.push(WorkspaceBinaryRestoreChange {
            relative_path: file.clone(),
            before,
            after: None,
        });
    }

    let text_deletes = license_files
        .iter()
        .filter(|path| workspace.documents.files.contains_key(*path))
        .cloned()
        .map(|relative_path| WorkspaceResourceDelete { relative_path })
        .collect::<Vec<_>>();
    let mut token_material = format!(
        "{}|{}|{}|{}|{}",
        identity.expected_project_root,
        identity.expected_session_id,
        identity.expected_revision,
        installed.id,
        installed.family
    );
    for file in &files {
        token_material.push('|');
        token_material.push_str(file);
    }
    for change in &text_changes {
        token_material.push('|');
        token_material.push_str(&change.relative_path);
        token_material.push(':');
        token_material.push_str(&hash_bytes(change.contents.as_bytes()));
    }
    for path in &license_files {
        token_material.push('|');
        token_material.push_str(path);
    }
    for reason in &blocked_reasons {
        token_material.push('|');
        token_material.push_str(reason);
    }
    let changed =
        !text_changes.is_empty() || !text_deletes.is_empty() || !binary_changes.is_empty();
    Ok(BuiltFontFamilyRemovalPlan {
        public: FontFamilyRemovalPlan {
            schema_version: 2,
            plan_token: hash_bytes(token_material.as_bytes()),
            family_id: installed.id,
            family: installed.family,
            directories,
            files,
            stylesheet_paths: managed_stylesheets,
            license_files,
            retained_resources,
            blocked_reasons,
            warnings,
            changed,
        },
        text_changes,
        text_deletes,
        binary_changes,
    })
}

fn is_managed_font_license_path(path: &str, directory: &str) -> bool {
    let Some(parent) = Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
    else {
        return false;
    };
    if parent != directory {
        return false;
    }
    matches!(
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("licenta.txt" | "license.txt" | "ofl.txt" | "ufl.txt")
    )
}
