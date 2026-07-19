use tauri::AppHandle;

use crate::kernel::{
    observability::{append_event, now_ms, KernelEventKind, KernelLogEvent, KernelLogLevel},
    project_session::ProjectSessionSnapshot,
};

use super::super::model::{
    PreviewHtmlAttributesExecutionReceipt, PreviewHtmlAttributesExecutionStatus,
    PreviewHtmlDeleteExecutionReceipt, PreviewHtmlDeleteExecutionStatus,
    PreviewHtmlDuplicateExecutionReceipt, PreviewHtmlDuplicateExecutionStatus,
    PreviewHtmlInsertDropExecutionReceipt, PreviewHtmlInsertDropExecutionStatus,
    PreviewHtmlTagExecutionReceipt, PreviewHtmlTagExecutionStatus, PreviewHtmlTextExecutionReceipt,
    PreviewHtmlTextExecutionStatus, PreviewLayerDropExecutionReceipt,
    PreviewLayerDropExecutionStatus, PreviewTemplateEditPermissionReceipt,
    PreviewTemplateEditPermissionStatus, PreviewTeraDeleteExecutionReceipt,
    PreviewTeraDeleteExecutionStatus, PreviewTeraInsertDropExecutionReceipt,
    PreviewTeraInsertDropExecutionStatus, PreviewTeraMoveDropExecutionReceipt,
    PreviewTeraMoveDropExecutionStatus,
};

pub(super) fn append_layer_drop_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewLayerDropExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewLayerDropExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionLayerDropCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewLayerDropExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionLayerDropBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.layer.drop.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("sourceStartLine", patch.source_start_line)
            .with_attribute("sourceEndLine", patch.source_end_line)
            .with_attribute("newStartLine", patch.new_start_line);
    }
    if let Some(projected_source_id) = receipt.projected_source_id.as_ref() {
        event = event.with_attribute("projectedSourceId", projected_source_id);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_html_insert_drop_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewHtmlInsertDropExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewHtmlInsertDropExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionHtmlInsertDropCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewHtmlInsertDropExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionHtmlInsertDropBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.html.insert_drop.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("tag", &patch.tag)
            .with_attribute("insertedStartLine", patch.inserted_start_line)
            .with_attribute("lineShift", patch.line_shift);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_html_attributes_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewHtmlAttributesExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewHtmlAttributesExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionHtmlAttributesCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewHtmlAttributesExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionHtmlAttributesBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.html.attributes.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("tag", &patch.tag)
            .with_attribute("sourceStartLine", patch.source_start_line)
            .with_attribute("attributeCount", patch.attributes.len());
    }

    let _ = append_event(app, event);
}

pub(super) fn append_html_text_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewHtmlTextExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewHtmlTextExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionHtmlTextCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewHtmlTextExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionHtmlTextBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.html.text.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("tag", &patch.tag)
            .with_attribute("sourceStartLine", patch.source_start_line)
            .with_attribute("lineShift", patch.line_shift);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_html_tag_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewHtmlTagExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewHtmlTagExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionHtmlTagCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewHtmlTagExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionHtmlTagBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.html.tag.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("oldTag", &patch.old_tag)
            .with_attribute("newTag", &patch.new_tag)
            .with_attribute("sourceStartLine", patch.source_start_line);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_html_delete_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewHtmlDeleteExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewHtmlDeleteExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionHtmlDeleteCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewHtmlDeleteExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionHtmlDeleteBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.html.delete.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("sourceStartLine", patch.source_start_line)
            .with_attribute("sourceEndLine", patch.source_end_line)
            .with_attribute("lineShift", patch.line_shift);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_html_duplicate_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewHtmlDuplicateExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewHtmlDuplicateExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionHtmlDuplicateCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewHtmlDuplicateExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionHtmlDuplicateBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.html.duplicate.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("tag", &patch.tag)
            .with_attribute("insertedStartLine", patch.inserted_start_line)
            .with_attribute("lineShift", patch.line_shift);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_tera_delete_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewTeraDeleteExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewTeraDeleteExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionTeraDeleteCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewTeraDeleteExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionTeraDeleteBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.tera.delete.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("deletedKind", &patch.deleted_kind)
            .with_attribute("sourceStartLine", patch.source_start_line)
            .with_attribute("sourceEndLine", patch.source_end_line)
            .with_attribute("lineShift", patch.line_shift);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_tera_insert_drop_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewTeraInsertDropExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewTeraInsertDropExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionTeraInsertDropCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewTeraInsertDropExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionTeraInsertDropBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.tera.insert_drop.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("insertedKind", &patch.inserted_kind)
            .with_attribute("insertedStartLine", patch.inserted_start_line)
            .with_attribute("lineShift", patch.line_shift);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_tera_move_drop_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewTeraMoveDropExecutionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewTeraMoveDropExecutionStatus::Committed => (
            KernelEventKind::PreviewProjectionTeraMoveDropCommitted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewTeraMoveDropExecutionStatus::Blocked => (
            KernelEventKind::PreviewProjectionTeraMoveDropBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = base_event(
        level,
        kind,
        "preview.tera.move_drop.execute",
        session,
        &receipt.intent.intent_id,
        receipt.status,
        receipt.diagnostics.len(),
        Some(receipt.touched_files.len()),
        message,
        diagnostic,
    );

    if let Some(workspace_mutation) = receipt.workspace_mutation.as_ref() {
        event = with_workspace_mutation(event, workspace_mutation);
    }
    if let Some(patch) = receipt.patch.as_ref() {
        event = event
            .with_attribute("file", &patch.file)
            .with_attribute("movedKind", &patch.moved_kind)
            .with_attribute("sourceStartLine", patch.source_start_line)
            .with_attribute("sourceEndLine", patch.source_end_line)
            .with_attribute("newStartLine", patch.new_start_line);
    }

    let _ = append_event(app, event);
}

pub(super) fn append_template_edit_permission_event(
    app: &AppHandle,
    session: &ProjectSessionSnapshot,
    receipt: &PreviewTemplateEditPermissionReceipt,
    diagnostic: Option<String>,
) {
    let (kind, level, message) = match receipt.status {
        PreviewTemplateEditPermissionStatus::Granted => (
            KernelEventKind::PreviewProjectionTemplateEditGranted,
            KernelLogLevel::Info,
            receipt.message.clone(),
        ),
        PreviewTemplateEditPermissionStatus::Blocked => (
            KernelEventKind::PreviewProjectionTemplateEditBlocked,
            KernelLogLevel::Warn,
            blocking_message(&receipt.diagnostics).unwrap_or_else(|| receipt.message.clone()),
        ),
    };

    let diagnostic = diagnostic.or_else(|| blocking_message(&receipt.diagnostics));
    let mut event = KernelLogEvent::new(
        level,
        kind,
        "preview_projection",
        "preview_projection",
        "preview.template_edit.permission",
        Some(session.id.clone()),
        message,
        diagnostic,
    )
    .with_attribute("intentId", &receipt.intent.intent_id)
    .with_attribute("permissionStatus", receipt.status)
    .with_attribute("diagnosticCount", receipt.diagnostics.len())
    .with_attribute("observedAtMs", now_ms());

    if let Some(grant) = receipt.grant.as_ref() {
        event = event
            .with_attribute("file", &grant.file)
            .with_attribute("targetKind", &grant.target_kind)
            .with_attribute("targetLabel", &grant.target_label)
            .with_attribute("scope", grant.scope);
    }

    let _ = append_event(app, event);
}

fn base_event<S>(
    level: KernelLogLevel,
    kind: KernelEventKind,
    operation: &str,
    session: &ProjectSessionSnapshot,
    intent_id: &str,
    status: S,
    diagnostic_count: usize,
    touched_file_count: Option<usize>,
    message: String,
    diagnostic: Option<String>,
) -> KernelLogEvent
where
    S: serde::Serialize,
{
    let mut event = KernelLogEvent::new(
        level,
        kind,
        "preview_projection",
        "preview_projection",
        operation,
        Some(session.id.clone()),
        message,
        diagnostic,
    )
    .with_attribute("intentId", intent_id)
    .with_attribute("executionStatus", status)
    .with_attribute("diagnosticCount", diagnostic_count)
    .with_attribute("observedAtMs", now_ms());

    if let Some(touched_file_count) = touched_file_count {
        event = event.with_attribute("touchedFileCount", touched_file_count);
    }

    event
}

fn with_workspace_mutation(
    event: KernelLogEvent,
    receipt: &crate::kernel::project_workspace::ProjectWorkspaceMutationReceipt,
) -> KernelLogEvent {
    let event = event
        .with_attribute("workspaceRevisionBefore", receipt.revision_before)
        .with_attribute("workspaceRevisionAfter", receipt.revision_after);
    match receipt.transaction_id.as_deref() {
        Some(transaction_id) => event.with_attribute("workspaceTransactionId", transaction_id),
        None => event,
    }
}

fn blocking_message(
    diagnostics: &[super::super::model::PreviewProjectionDiagnostic],
) -> Option<String> {
    diagnostics
        .iter()
        .find(|item| item.blocking)
        .map(|item| item.message.clone())
}
