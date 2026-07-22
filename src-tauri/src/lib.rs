mod app_home;
mod commands;
mod css;
mod deploy;
mod fonts;
mod images;
mod js;
pub mod kernel;
mod mcp;
mod mood;
mod page_assets;
mod page_components;
mod preview;
mod project;
mod project_model;
mod source_graph;
mod state;
mod versioning;
#[macro_use]
#[allow(dead_code)]
mod tauri_command_registry;
mod zola_links;
mod zola_theme;

use serde::Serialize;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

macro_rules! generate_registered_tauri_handler {
    ($($command:ident),* $(,)?) => {
        tauri::generate_handler![$($command),*]
    };
}

use commands::{
    ai_coordination::{
        accept_ai_edit_conflict_for_reconciliation, acknowledge_ai_edit_quiescence,
        authorize_ai_reconciliation_recovery_reload, complete_ai_edit_reconciliation,
        complete_ai_reconciliation_recovery_reload, read_ai_coordination_state,
    },
    app_home::read_app_home,
    audit::read_project_audit,
    command_center::search_command_center,
    config::{
        read_project_app_config, read_project_env, read_zola_base_url, read_zola_project_settings,
        save_project_app_config, save_project_env, save_zola_base_url, save_zola_project_settings,
    },
    css::{
        cleanup_page_css_contract, find_class_in_scss, get_class_rules,
        get_class_rules_at_viewport, get_css_rule_context, get_scss_variables,
        resolve_page_css_target, set_css_rule, set_css_rule_at_viewport,
        set_page_css_rule_at_viewport, set_scss_variable,
    },
    deploy::{cancel_publish_operation, deploy_to_bunny, zola_build, zola_check},
    design_system::{read_design_class_inventory, rename_design_class},
    external_disk::reconcile_clean_external_project_files,
    fonts::{download_google_font_family, get_font_inventory, search_google_fonts},
    js::{
        apply_motion_timeline_step_timing, clear_page_js_draft, get_page_data_anims, get_page_js,
        get_page_js_workspace_state, read_page_js_drafts, stage_page_js_draft,
    },
    kernel::{
        execute_preview_html_attributes_intent, execute_preview_html_delete_intent,
        execute_preview_html_duplicate_intent, execute_preview_html_insert_drop_intent,
        execute_preview_html_tag_intent, execute_preview_html_text_intent,
        execute_preview_layer_drop_intent, execute_preview_template_edit_intent,
        execute_preview_tera_delete_intent, execute_preview_tera_insert_drop_intent,
        execute_preview_tera_move_drop_intent, normalize_preview_projection_intent,
        read_kernel_disk_conflicts, read_kernel_observability_log,
        read_kernel_project_transition_blocked_audit,
        read_kernel_project_transition_decision_journal,
        read_kernel_project_transition_decision_recovery_ack_journal,
        read_kernel_project_transition_decision_retention_hot_journals,
        read_kernel_project_transition_policy, read_kernel_project_transition_policy_matrix,
        read_write_authority_recovery_scan, resolve_write_authority_recovery,
    },
    mcp::{
        configure_codex_mcp, read_ai_context_status, read_codex_mcp_status,
        save_ai_context_snapshot, write_ai_context_snapshot,
    },
    mood::{
        export_mood_board_svg_asset, extract_mood_board_image_palette, read_mood_board,
        read_mood_board_image_data_url, read_mood_board_image_original_data_url,
        read_mood_board_svg_source, save_mood_board, write_mood_board,
    },
    page_assets::{apply_page_asset_contract, plan_page_asset_contract},
    page_components::{
        apply_page_component_contract, plan_page_component_contract, read_page_component_registry,
    },
    preview::{
        acknowledge_canvas_projection_phase, project_project_workspace_preview,
        project_template_workbench_preview, read_preview_document, record_preview_runtime_event,
        start_project_browser_preview, start_project_preview,
    },
    project::{
        acknowledge_project_transition_decision_recovery_plan, apply_file_buffer_changeset,
        clear_file_buffer_draft, close_project, execute_project_transition_decision_retention,
        export_project_asset_data_url, export_project_asset_webp_from_data_url,
        get_zola_binary_path, inspect_project_open_recovery, open_project,
        read_current_project_disk_manifest, read_file_buffer_store, read_file_buffer_text,
        read_project_file, read_project_session, read_project_workspace_history,
        read_project_workspace_state, read_recovery_coordinator_scan, reattach_project_session,
        record_project_transition_operator_decision,
        recover_project_transition_decision_retention_hot_journal, recover_project_workspace_save,
        redo_project_workspace, save_project_workspace, scan_project, set_file_buffer_draft,
        undo_project_workspace, zola_init,
    },
    project_model::{
        plan_project_html_move, read_project_model, read_project_model_with_drafts,
        resolve_template_workbench_plan,
    },
    source_graph::{
        create_site_archive_structure, create_site_page_structure, create_site_partial_structure,
        create_site_single_structure, include_site_partial, read_source_graph,
    },
    versioning::{
        cancel_version_network_operation, clear_version_upstream, commit_versioning,
        configure_version_remote, configure_version_upstream, configure_versioning_identity,
        create_version_branch, delete_version_branch, fetch_version_remote, initialize_versioning,
        integrate_version_target, preview_version, push_version_branch, read_version_diff,
        read_version_history, read_version_integration_plan, read_version_integration_recovery,
        read_version_restore_recovery, read_version_sync_comparison, read_versioning_snapshot,
        remove_version_remote, resolve_version_integration_recovery,
        resolve_version_restore_recovery, restore_version, stage_all_versioning,
        stage_versioning_paths, stop_version_preview, switch_version_branch,
        unstage_all_versioning, unstage_versioning_paths,
    },
    window::reset_main_webview_zoom,
    workbench::{apply_workbench_intent, read_workbench_state},
    workspace_entries::{
        workspace_create_content_page, workspace_create_project_text_file,
        workspace_delete_project_entry, workspace_move_project_entry,
        workspace_rename_project_entry,
    },
};
use kernel::ai_coordination::EditAuthority;
use kernel::observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel};
use mcp::{
    load_or_generate_access_token, mark_context_server_lifecycle, recorded_server_process_id,
    start_context_server,
};
use preview::{resolve_zola_binary_path, stop_project_preview, stop_source_browser};
use state::AppState;

const MAIN_WINDOW_LABEL: &str = "main";
const NATIVE_WINDOW_CLOSE_REQUESTED_EVENT: &str = "pana-native-window-close-requested";

fn apply_main_window_icon(app: &tauri::App) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let icon = match tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png")) {
        Ok(icon) => icon,
        Err(error) => {
            eprintln!("[Pană Studio] Nu am putut încărca icon-ul aplicației: {error}");
            return;
        }
    };
    if let Err(error) = window.set_icon(icon) {
        eprintln!("[Pană Studio] Nu am putut seta icon-ul ferestrei: {error}");
    }
}

#[derive(Clone, Serialize)]
struct NativeCanvasZoomPayload {
    x: f64,
    y: f64,
    scale: f64,
    phase: i32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeWindowCloseRequestPayload {
    project_root: String,
}

fn lock_main_webview_zoom(app: &tauri::App) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };

    if let Err(error) = window.set_zoom(1.0) {
        eprintln!("[Pană Studio] Nu am putut reseta zoom-ul WebView: {error}");
    }

    #[cfg(target_os = "linux")]
    {
        let native_zoom_window = window.clone();
        if let Err(error) = window.with_webview(move |webview| {
            use gdk::{EventMask, EventType, ModifierType};
            use gtk::{glib::Propagation, prelude::*};
            use webkit2gtk::WebViewExt;

            let inner = webview.inner();
            let pinch_event_window = native_zoom_window.clone();
            inner.add_events(
                EventMask::SCROLL_MASK
                    | EventMask::SMOOTH_SCROLL_MASK
                    | EventMask::TOUCHPAD_GESTURE_MASK,
            );
            inner.connect_event(move |view, event| {
                if event.event_type() == EventType::TouchpadPinch {
                    if let Some(pinch) = event.downcast_ref::<gdk::EventTouchpadPinch>() {
                        let (x, y) = pinch.position();
                        let _ = pinch_event_window.emit(
                            "native-canvas-zoom",
                            NativeCanvasZoomPayload {
                                x,
                                y,
                                scale: pinch.scale(),
                                phase: pinch.as_ref().phase as i32,
                            },
                        );
                    }
                    view.set_zoom_level(1.0);
                    return Propagation::Stop;
                }
                Propagation::Proceed
            });
            inner.connect_scroll_event(|view, event| {
                if event.state().contains(ModifierType::CONTROL_MASK) {
                    view.set_zoom_level(1.0);
                    return Propagation::Stop;
                }
                Propagation::Proceed
            });
            inner.set_zoom_level(1.0);
            inner.connect_zoom_level_notify(|view| {
                if (view.zoom_level() - 1.0).abs() > 0.001 {
                    view.set_zoom_level(1.0);
                }
            });
        }) {
            eprintln!("[Pană Studio] Nu am putut bloca zoom-ul WebKitGTK: {error}");
        }
    }
}

fn start_mcp_context_server(app: &tauri::App) {
    let state = app.state::<AppState>();
    let access_token = match load_or_generate_access_token() {
        Ok(access_token) => access_token,
        Err(error) => {
            eprintln!("[Pană Studio] Tokenul MCP nu a putut fi inițializat: {error}");
            return;
        }
    };
    match state.mcp_access_token.lock() {
        Ok(mut slot) => *slot = Some(access_token.clone()),
        Err(_) => {
            eprintln!("[Pană Studio] Tokenul MCP nu poate fi instalat în RAM.");
            return;
        }
    }
    match start_context_server(app.handle().clone(), access_token) {
        Ok(handle) => {
            match state.mcp_server.lock() {
                Ok(mut slot) => *slot = Some(handle),
                Err(_) => {
                    handle.stop();
                    eprintln!("[Pană Studio] Handle-ul MCP nu poate fi instalat în AppState.");
                    let _ =
                        mark_context_server_lifecycle(&app.handle(), false, "state_unavailable");
                    return;
                }
            }
            if let Err(error) =
                mark_context_server_lifecycle(&app.handle(), true, "awaiting_ui_context")
            {
                eprintln!("[Pană Studio] Descriptorul MCP de startup nu a fost scris: {error}");
            }
            println!("[Pană Studio] MCP read-only server pornit pe http://127.0.0.1:48731/mcp");
        }
        Err(error) => {
            let recorded_owner = recorded_server_process_id(&app.handle());
            if let Some(process_id) = recorded_owner {
                eprintln!(
                    "[Pană Studio] Serverul MCP read-only nu a pornit: {error}. Descriptorul anterior indică procesul PID {process_id}; verifică /health înainte de a opri procesul."
                );
            } else {
                eprintln!("[Pană Studio] Serverul MCP read-only nu a pornit: {error}");
            }
            if let Err(lifecycle_error) =
                mark_context_server_lifecycle(&app.handle(), false, "start_failed")
            {
                eprintln!(
                    "[Pană Studio] Descriptorul MCP de eșec nu a fost scris: {lifecycle_error}"
                );
            }
        }
    }
}

fn stop_mcp_context_server(app: &tauri::AppHandle, state: &AppState) {
    let handle = state
        .mcp_server
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(handle) = handle {
        handle.stop();
        if let Err(error) = mark_context_server_lifecycle(app, false, "stopped") {
            eprintln!("[Pană Studio] Tombstone-ul MCP de shutdown nu a fost scris: {error}");
        }
    }
}

fn current_project_root_for_shutdown_guard(state: &AppState) -> Result<Option<String>, String> {
    state
        .current_root
        .lock()
        .map_err(|_| "Nu am putut bloca ProjectSession pentru window close.".to_string())
        .map(|slot| slot.as_ref().map(|root| root.to_string_lossy().to_string()))
}

fn request_frontend_project_close<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    project_root: String,
    trigger: &'static str,
) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if let Err(error) = window.emit(
            NATIVE_WINDOW_CLOSE_REQUESTED_EVENT,
            NativeWindowCloseRequestPayload {
                project_root: project_root.clone(),
            },
        ) {
            eprintln!(
                "[Pană Studio] Nu am putut trimite cererea de native window close către frontend: {error}"
            );
        }
    }

    if let Err(error) = append_event(
        app,
        KernelLogEvent::new(
            KernelLogLevel::Info,
            KernelEventKind::NativeWindowCloseRequested,
            "project_lifecycle",
            "desktop_lifecycle",
            "native_window_close_requested",
            Some(project_root),
            "Native window close was routed through ProjectTransitionPolicy.",
            None,
        )
        .with_attribute("windowLabel", MAIN_WINDOW_LABEL)
        .with_attribute("trigger", trigger)
        .with_attribute("frontendEvent", NATIVE_WINDOW_CLOSE_REQUESTED_EVENT),
    ) {
        eprintln!("[Pană Studio] Nu am putut loga native window close request: {error}");
    }
}

fn project_session_is_open_for_shutdown_guard<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    trigger: &'static str,
) -> bool {
    let state = app.state::<AppState>();
    let timestamp = kernel::observability::now_ms();
    if state.ai_coordination.expire(timestamp).is_err() {
        return true;
    }
    let Ok(coordination) = state.ai_coordination.snapshot(timestamp) else {
        return true;
    };
    if !edit_authority_requires_project_close_guard(&coordination.authority) {
        // A non-user authority state must never route native shutdown through a
        // ProjectTransition which the same authority state prohibits. Closing
        // is the fail-safe: MCP is cancelled, the current disk is preserved and
        // the next boot performs normal disk/session recovery.
        return false;
    }
    match current_project_root_for_shutdown_guard(state.inner()) {
        Ok(Some(project_root)) => {
            request_frontend_project_close(app, project_root, trigger);
            true
        }
        Ok(None) => false,
        Err(error) => {
            eprintln!("[Pană Studio] {error}");
            true
        }
    }
}

fn edit_authority_requires_project_close_guard(authority: &EditAuthority) -> bool {
    matches!(authority, EditAuthority::UserActive)
}

#[cfg(test)]
mod desktop_lifecycle_tests {
    use super::edit_authority_requires_project_close_guard;
    use crate::kernel::ai_coordination::{EditAuthority, EditLease};

    #[test]
    fn only_user_authority_routes_close_through_project_transition() {
        assert!(edit_authority_requires_project_close_guard(
            &EditAuthority::UserActive
        ));
        assert!(!edit_authority_requires_project_close_guard(
            &EditAuthority::AiActive {
                lease: EditLease {
                    id: "lease-close".to_string(),
                    request_id: "request-close".to_string(),
                    client_session_id: "client-close".to_string(),
                    project_session_id: "project-close".to_string(),
                    basis_project_revision: 1,
                    intent: "test close escape".to_string(),
                    granted_at_ms: 1,
                    expires_at_ms: 2,
                },
            }
        ));
        assert!(!edit_authority_requires_project_close_guard(
            &EditAuthority::AiOrphaned {
                lease_id: "lease-close".to_string(),
                client_session_id: "client-close".to_string(),
                project_session_id: "project-close".to_string(),
                basis_project_revision: 1,
                expired_at_ms: 2,
                reason: "test".to_string(),
            }
        ));
        assert!(!edit_authority_requires_project_close_guard(
            &EditAuthority::Conflict {
                project_session_id: "project-close".to_string(),
                detected_at_ms: 2,
                files: vec!["sursa/content/test.md".to_string()],
                reason: "test".to_string(),
            }
        ));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .manage(AppState::default())
        .manage(kernel::write_authority::WriteAuthorityRuntime::default())
        .setup(|app| {
            let app_home = app_home::ensure_app_home(&app.handle())?;
            println!(
                "[Pană Studio] Application Home: config={}, data={}, cache={}, logs={}",
                app_home.config_dir, app_home.data_dir, app_home.cache_dir, app_home.log_dir
            );
            kernel::boot(&app.handle())?;
            apply_main_window_icon(app);
            lock_main_webview_zoom(app);
            start_mcp_context_server(app);
            let state = app.state::<AppState>();
            let zola_binary_path = resolve_zola_binary_path(&app.handle())?;
            let mut path_slot = state
                .zola_binary_path
                .lock()
                .map_err(|_| "Nu am putut bloca starea binarului Zola.")?;
            *path_slot = Some(zola_binary_path);
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_pty::init())
        .invoke_handler(pana_tauri_commands!(generate_registered_tauri_handler))
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } if label == MAIN_WINDOW_LABEL => {
            if project_session_is_open_for_shutdown_guard(app_handle, "window_close_requested") {
                api.prevent_close();
            }
        }
        RunEvent::ExitRequested { api, .. } => {
            if project_session_is_open_for_shutdown_guard(app_handle, "app_exit_requested") {
                api.prevent_exit();
                return;
            }
            let state = app_handle.state::<AppState>();
            stop_source_browser(app_handle, state.inner());
            stop_project_preview(app_handle, state.inner());
            stop_mcp_context_server(app_handle, state.inner());
        }
        RunEvent::Exit => {
            let state = app_handle.state::<AppState>();
            stop_source_browser(app_handle, state.inner());
            stop_project_preview(app_handle, state.inner());
            stop_mcp_context_server(app_handle, state.inner());
        }
        _ => {}
    });
}
