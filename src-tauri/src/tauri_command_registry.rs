//! Canonical registry for every custom Tauri command exposed by Pană Studio.
//!
//! This module is compiled both by `build.rs` and by the application crate. Keep
//! the command identifiers in exactly one place: the build manifest, the invoke
//! handler and the generated app permission set are all derived from this list.

macro_rules! pana_tauri_commands {
    ($consumer:ident) => {
        $consumer!(
            publish_global_status,
            resolve_global_status,
            read_global_status,
            read_ai_coordination_state,
            acknowledge_ai_edit_quiescence,
            accept_ai_edit_conflict_for_reconciliation,
            authorize_ai_reconciliation_recovery_reload,
            complete_ai_reconciliation_recovery_reload,
            complete_ai_edit_reconciliation,
            read_app_home,
            read_application_storage_inventory,
            clear_application_cache_storage,
            clear_application_log_storage,
            delete_application_session_storage,
            read_application_settings,
            save_application_settings,
            read_startup_flow,
            read_project_lifecycle,
            inspect_startup_folder,
            inspect_project_open,
            cancel_project_open,
            acknowledge_project_frontend_hydrated,
            report_project_capability_degraded,
            read_startup_creation_catalog,
            plan_startup_creation,
            apply_startup_creation,
            open_project,
            close_project,
            reattach_project_session,
            read_write_authority_recovery_scan,
            resolve_write_authority_recovery,
            normalize_preview_projection_intent,
            execute_preview_html_insert_drop_intent,
            execute_preview_html_attributes_intent,
            execute_preview_html_text_intent,
            execute_preview_html_tag_intent,
            execute_preview_html_duplicate_intent,
            execute_preview_html_delete_intent,
            execute_preview_selection_batch_intent,
            execute_preview_tera_insert_drop_intent,
            execute_preview_tera_delete_intent,
            read_kernel_disk_conflicts,
            read_kernel_project_transition_policy,
            read_kernel_project_transition_policy_matrix,
            read_kernel_project_transition_blocked_audit,
            read_kernel_project_transition_decision_journal,
            read_kernel_project_transition_decision_recovery_ack_journal,
            read_kernel_observability_log,
            read_recovery_coordinator_scan,
            record_project_transition_operator_decision,
            acknowledge_project_transition_decision_recovery_plan,
            execute_project_transition_decision_retention,
            recover_project_transition_decision_retention_hot_journal,
            recover_project_workspace_save,
            read_project_workspace_state,
            read_project_audit,
            apply_audit_fix,
            read_design_class_inventory,
            read_design_token_catalog,
            read_theme_style_catalog,
            preview_theme_style_draft,
            apply_theme_style_draft,
            create_design_class,
            rename_design_class,
            read_workbench_state,
            apply_workbench_intent,
            search_command_center,
            undo_project_workspace,
            redo_project_workspace,
            read_file_buffer_text,
            reconcile_clean_external_project_files,
            apply_file_buffer_changeset,
            set_file_buffer_draft,
            clear_file_buffer_draft,
            read_file_explorer_snapshot,
            select_file_explorer_entry,
            plan_file_explorer_operation,
            commit_file_explorer_operation,
            scan_project,
            read_source_graph,
            read_taxonomy_catalog,
            plan_taxonomy_mutation,
            apply_taxonomy_mutation,
            read_content_model_catalog,
            plan_content_model_mutation,
            apply_content_model_mutation,
            read_template_catalog,
            apply_component_mutation,
            read_data_node_editor,
            apply_data_mutation,
            read_ui_block_graph,
            read_icon_catalog,
            search_icon_catalog,
            read_insert_catalog,
            read_dynamic_widget_snapshot,
            update_dynamic_widget,
            delete_dynamic_widget,
            bind_canvas_interaction_agent,
            resolve_canvas_interaction_intent,
            resolve_canvas_drag_over_intent,
            resolve_canvas_hover_intent,
            apply_selection_intent,
            read_selection_snapshot,
            accept_selection_observation,
            read_editor_navigation_snapshot,
            request_editor_edit_scope,
            plan_editor_move,
            commit_editor_move,
            read_current_project_disk_manifest,
            start_project_disk_watch,
            stop_project_disk_watch,
            workspace_create_content_page,
            workspace_create_project_text_file,
            workspace_update_page_frontmatter_field,
            read_project_file,
            save_project_workspace,
            read_project_configuration,
            save_project_configuration,
            read_preview_document,
            start_project_browser_preview,
            start_project_preview,
            confirm_template_workbench_reuse,
            project_template_workbench_preview,
            project_project_workspace_preview,
            acknowledge_canvas_projection_phases,
            record_preview_runtime_event,
            workspace_create_semantic_template,
            workspace_create_listing_item,
            workspace_delete_listing_item,
            workspace_duplicate_template,
            workspace_override_theme_template,
            workspace_rename_template,
            workspace_set_template_parent,
            workspace_set_template_assignment,
            workspace_delete_template,
            set_css_rule_at_viewport,
            set_page_css_rule_at_viewport,
            set_reusable_css_rule_at_viewport,
            apply_local_font_import,
            assign_font_role,
            download_google_font_family,
            get_bundled_font_catalog,
            get_bundled_font_preview,
            get_font_manager,
            get_font_preview_asset,
            install_bundled_font_family,
            plan_font_family_removal,
            plan_local_font_import,
            remove_font_family,
            search_google_fonts,
            set_font_display,
            set_font_preload,
            get_scss_variables,
            create_scss_variable,
            set_scss_variable,
            resolve_css_inspector_context,
            get_page_js_workspace_state,
            stage_page_js_draft,
            clear_page_js_draft,
            apply_motion_mutation,
            read_ai_context_status,
            save_ai_context_snapshot,
            read_codex_mcp_status,
            configure_codex_mcp,
            apply_page_asset_contract,
            import_project_asset,
            zola_check,
            zola_check_workspace,
            zola_build,
            run_publish_preflight,
            current_publish_preflight_receipt,
            build_for_publish,
            current_publish_build_receipt,
            cancel_publish_operation,
            read_deploy_configuration,
            save_deploy_settings,
            save_deploy_credential,
            delete_deploy_credential,
            test_deploy_connection,
            plan_deploy,
            execute_deploy,
            read_versioning_snapshot,
            initialize_versioning,
            configure_versioning_identity,
            configure_version_remote,
            remove_version_remote,
            configure_version_upstream,
            clear_version_upstream,
            create_version_branch,
            delete_version_branch,
            fetch_version_remote,
            push_version_branch,
            cancel_version_network_operation,
            read_version_integration_plan,
            integrate_version_target,
            switch_version_branch,
            read_version_integration_recovery,
            resolve_version_integration_recovery,
            stage_versioning_paths,
            stage_all_versioning,
            unstage_versioning_paths,
            unstage_all_versioning,
            commit_versioning,
            read_version_history,
            read_version_diff,
            preview_version,
            stop_version_preview,
            restore_version,
            read_version_restore_recovery,
            resolve_version_restore_recovery,
            reset_main_webview_zoom,
        )
    };
}

macro_rules! collect_tauri_command_names {
    ($($command:ident),* $(,)?) => {
        &[$(stringify!($command)),*]
    };
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const APP_COMMAND_NAMES: &[&str] = pana_tauri_commands!(collect_tauri_command_names);

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn allow_permission_identifier(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn render_app_default_permission_toml() -> String {
    use std::fmt::Write as _;

    let mut output = String::from(
        "# Automatically generated from src/tauri_command_registry.rs - DO NOT EDIT!\n\
[default]\n\
description = \"Allows the main Pană Studio webview to invoke every registered application command.\"\n\
permissions = [\n",
    );

    for command in APP_COMMAND_NAMES {
        writeln!(output, "  \"{}\",", allow_permission_identifier(command))
            .expect("writing to String cannot fail");
    }
    output.push_str("]\n");
    output
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use super::*;

    macro_rules! collect_handler_registry_names {
        ($($command:ident),* $(,)?) => {
            &[$(stringify!($command)),*]
        };
    }

    #[test]
    fn manifest_handler_and_permissions_share_exact_registry() {
        let handler_names: &[&str] = pana_tauri_commands!(collect_handler_registry_names);
        assert_eq!(handler_names, APP_COMMAND_NAMES);

        let unique_names = APP_COMMAND_NAMES.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique_names.len(), APP_COMMAND_NAMES.len());

        let permission_document = render_app_default_permission_toml()
            .parse::<toml_edit::DocumentMut>()
            .expect("generated application permission TOML must parse");
        let actual_permissions = permission_document["default"]["permissions"]
            .as_array()
            .expect("generated default permission must contain an array")
            .iter()
            .map(|permission| {
                permission
                    .as_str()
                    .expect("generated permission identifiers must be strings")
                    .to_string()
            })
            .collect::<Vec<_>>();
        let expected_permissions = APP_COMMAND_NAMES
            .iter()
            .map(|command| allow_permission_identifier(command))
            .collect::<Vec<_>>();

        assert_eq!(actual_permissions, expected_permissions);
    }

    #[test]
    fn autogenerated_permission_files_match_the_exact_command_registry() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("permissions/autogenerated");
        let actual = fs::read_dir(&directory)
            .expect("autogenerated permission directory must exist")
            .map(|entry| entry.expect("permission entry must be readable").path())
            .filter(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("toml")
            })
            .map(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .expect("permission filename must be UTF-8")
                    .to_string()
            })
            .collect::<BTreeSet<_>>();
        let expected = APP_COMMAND_NAMES
            .iter()
            .map(|command| (*command).to_string())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            actual, expected,
            "stale or missing Tauri command permission"
        );
    }

    #[test]
    fn default_capability_is_scoped_only_to_main_webview() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("default capability must be valid JSON");

        assert!(capability.get("windows").is_none());
        assert_eq!(
            capability["webviews"],
            serde_json::json!(["main"]),
            "window-level matching would also grant permissions to child webviews"
        );
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "core:app:allow-version",
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "core:path:allow-resolve-directory",
                "core:window:allow-close",
                "core:window:allow-show",
                "dialog:allow-open",
                "opener:allow-open-url",
                "opener:allow-default-urls",
                "pty:default",
                "default"
            ])
        );
    }
}
