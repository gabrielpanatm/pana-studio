//! Canonical registry for every custom Tauri command exposed by Pană Studio.
//!
//! This module is compiled both by `build.rs` and by the application crate. Keep
//! the command identifiers in exactly one place: the build manifest, the invoke
//! handler and the generated app permission set are all derived from this list.

macro_rules! pana_tauri_commands {
    ($consumer:ident) => {
        $consumer!(
            read_ai_coordination_state,
            acknowledge_ai_edit_quiescence,
            accept_ai_edit_conflict_for_reconciliation,
            authorize_ai_reconciliation_recovery_reload,
            complete_ai_reconciliation_recovery_reload,
            complete_ai_edit_reconciliation,
            read_app_home,
            inspect_project_open_recovery,
            open_project,
            close_project,
            read_project_session,
            reattach_project_session,
            read_write_authority_recovery_scan,
            resolve_write_authority_recovery,
            normalize_preview_projection_intent,
            execute_preview_layer_drop_intent,
            execute_preview_html_insert_drop_intent,
            execute_preview_html_attributes_intent,
            execute_preview_html_text_intent,
            execute_preview_html_tag_intent,
            execute_preview_html_duplicate_intent,
            execute_preview_html_delete_intent,
            execute_preview_template_edit_intent,
            execute_preview_tera_insert_drop_intent,
            execute_preview_tera_move_drop_intent,
            execute_preview_tera_delete_intent,
            read_kernel_disk_conflicts,
            read_kernel_project_transition_policy,
            read_kernel_project_transition_policy_matrix,
            read_kernel_project_transition_blocked_audit,
            read_kernel_project_transition_decision_journal,
            read_kernel_project_transition_decision_recovery_ack_journal,
            read_kernel_project_transition_decision_retention_hot_journals,
            read_kernel_observability_log,
            read_recovery_coordinator_scan,
            record_project_transition_operator_decision,
            acknowledge_project_transition_decision_recovery_plan,
            execute_project_transition_decision_retention,
            recover_project_transition_decision_retention_hot_journal,
            recover_project_workspace_save,
            read_project_workspace_state,
            read_project_workspace_history,
            read_project_audit,
            read_design_class_inventory,
            rename_design_class,
            read_workbench_state,
            apply_workbench_intent,
            search_command_center,
            undo_project_workspace,
            redo_project_workspace,
            read_file_buffer_store,
            read_file_buffer_text,
            reconcile_clean_external_project_files,
            apply_file_buffer_changeset,
            set_file_buffer_draft,
            clear_file_buffer_draft,
            scan_project,
            create_site_archive_structure,
            create_site_page_structure,
            create_site_partial_structure,
            create_site_single_structure,
            include_site_partial,
            read_source_graph,
            read_project_model,
            read_project_model_with_drafts,
            resolve_template_workbench_plan,
            plan_project_html_move,
            read_current_project_disk_manifest,
            workspace_create_content_page,
            workspace_create_project_text_file,
            read_project_file,
            save_project_workspace,
            read_project_app_config,
            read_zola_project_settings,
            save_project_app_config,
            save_zola_project_settings,
            read_preview_document,
            start_project_browser_preview,
            start_project_preview,
            project_template_workbench_preview,
            project_project_workspace_preview,
            acknowledge_canvas_projection_phase,
            record_preview_runtime_event,
            workspace_move_project_entry,
            workspace_rename_project_entry,
            workspace_delete_project_entry,
            set_css_rule,
            set_css_rule_at_viewport,
            set_page_css_rule_at_viewport,
            cleanup_page_css_contract,
            download_google_font_family,
            get_font_inventory,
            search_google_fonts,
            get_scss_variables,
            get_class_rules,
            get_class_rules_at_viewport,
            get_css_rule_context,
            set_scss_variable,
            find_class_in_scss,
            resolve_page_css_target,
            get_page_js,
            get_page_js_workspace_state,
            get_page_data_anims,
            stage_page_js_draft,
            read_page_js_drafts,
            clear_page_js_draft,
            apply_motion_timeline_step_timing,
            read_ai_context_status,
            save_ai_context_snapshot,
            write_ai_context_snapshot,
            read_codex_mcp_status,
            configure_codex_mcp,
            apply_page_asset_contract,
            plan_page_asset_contract,
            read_page_component_registry,
            apply_page_component_contract,
            plan_page_component_contract,
            read_project_env,
            save_project_env,
            read_zola_base_url,
            save_zola_base_url,
            zola_init,
            zola_check,
            zola_check_workspace,
            zola_build,
            cancel_publish_operation,
            deploy_to_bunny,
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
            read_version_sync_comparison,
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

pub(crate) const APP_COMMAND_NAMES: &[&str] = pana_tauri_commands!(collect_tauri_command_names);

pub(crate) fn allow_permission_identifier(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

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
                "core:default",
                "core:window:allow-close",
                "dialog:default",
                "opener:default",
                "pty:default",
                "default"
            ])
        );
    }
}
