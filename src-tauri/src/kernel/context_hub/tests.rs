use super::{
    AiContextApp, AiContextCore, AiContextDirtyState, AiContextFileInventory, AiContextProject,
    ContextHubPublication, ContextHubRuntime, UiCenterView, UiCssContext, UiExternalDiskContext,
    UiMoodBoardContext, UiPreviewDevice, UiSelectionContext, UiSourceLanguage, UiWorkspaceContext,
};

fn core(active_file: Option<&str>) -> AiContextCore {
    AiContextCore {
        app: AiContextApp {
            name: "Pană Studio".to_string(),
            mode: "read_only_data_with_ram_coordination".to_string(),
        },
        project: AiContextProject {
            root: Some("/tmp/project".to_string()),
            session_id: Some("project-1".to_string()),
            is_open: true,
            is_zola: true,
            is_empty: false,
            project_revision: Some(7),
            disk_generation: Some(2),
            preview_base_url: Some("http://127.0.0.1:1111".to_string()),
            preview_warning: None,
        },
        workspace: UiWorkspaceContext {
            center_view: UiCenterView::Code,
            preview_device: UiPreviewDevice::Desktop,
            active_file: active_file.map(str::to_string),
            active_preview_path: None,
            source_language: UiSourceLanguage::Html,
        },
        selection: UiSelectionContext {
            has_selection: false,
            selector: None,
            css_selector: None,
            tag: None,
            id: None,
            classes: Vec::new(),
            text: None,
            image_src: None,
            source_location: None,
            source_id: None,
            template_source_id: None,
            session_id: None,
            rect: None,
        },
        css: UiCssContext {
            active_selector: None,
            target_file: None,
            variables_count: 0,
        },
        dirty_state: AiContextDirtyState {
            dirty: false,
            project_workspace_dirty: false,
            ui_dirty: false,
            can_save: false,
            dirty_files: Vec::new(),
            ui_areas: Vec::new(),
            blocked_reason: String::new(),
        },
        files: AiContextFileInventory {
            tracked_text_total: 0,
            pages: Vec::new(),
            templates: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
            config_and_data: Vec::new(),
            truncated: false,
        },
        mood_board: UiMoodBoardContext {
            available: true,
            items: 0,
            save_state: "idle".to_string(),
            tool: "select".to_string(),
        },
        external_disk: UiExternalDiskContext {
            changed: false,
            changed_files: Vec::new(),
            active_file_changed: false,
            preview_relevant_changed: false,
            blocked_by_dirty_session: false,
            last_detected_at: None,
            last_detected_files: Vec::new(),
            last_detected_active_file_changed: false,
            last_detected_preview_relevant_changed: false,
            last_applied_at: None,
            last_applied_files: Vec::new(),
            last_checked_at: None,
            checking: false,
            reconciling: false,
            workspace_projection_recovery_required: false,
            truncated: false,
        },
        guidance: Vec::new(),
    }
}

fn publication(ui_revision: u64, active_file: Option<&str>) -> ContextHubPublication {
    ContextHubPublication {
        project_session_id: Some("project-1".to_string()),
        ui_revision,
        core: core(active_file),
    }
}

#[test]
fn semantic_noop_advances_ui_seen_without_rewriting_context() {
    let runtime = ContextHubRuntime::default();
    let first = runtime.publish(publication(1, None), 100).unwrap();
    let second = runtime.publish(publication(2, None), 200).unwrap();

    assert!(first.changed);
    assert!(!second.changed);
    assert_eq!(first.context_revision, second.context_revision);
    assert_eq!(second.ui_revision_seen, 2);
    assert_eq!(second.updated_at_ms, 100);
}

#[test]
fn semantic_change_advances_context_revision() {
    let runtime = ContextHubRuntime::default();
    let first = runtime.publish(publication(1, None), 100).unwrap();
    let second = runtime
        .publish(publication(2, Some("sursa/templates/index.html")), 200)
        .unwrap();

    assert!(second.changed);
    assert_eq!(second.context_revision, first.context_revision + 1);
    assert_eq!(second.updated_at_ms, 200);
}

#[test]
fn stale_or_ambiguous_ui_revision_is_rejected() {
    let runtime = ContextHubRuntime::default();
    runtime.publish(publication(3, None), 100).unwrap();

    assert!(runtime.publish(publication(2, None), 200).is_err());
    assert!(runtime
        .publish(publication(3, Some("sursa/templates/index.html")), 200)
        .is_err());
}
