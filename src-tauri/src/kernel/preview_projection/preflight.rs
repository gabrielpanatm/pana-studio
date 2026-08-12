use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use super::model::{
    PreviewProjectionDiagnostic, PreviewProjectionEffect, PreviewProjectionIntentInput,
    PreviewProjectionIntentKind, PreviewProjectionIntentReceipt, PreviewProjectionIntentStatus,
};
use crate::{kernel::project_session::ProjectSessionSnapshot, localization::LocalizedDiagnostic};

const PREVIEW_PROJECTION_SCHEMA_VERSION: u32 = 2;
const DROP_POSITIONS: &[&str] = &["before", "after", "inside"];

pub fn preflight_preview_projection_intent(
    input: PreviewProjectionIntentInput,
    session: Option<&ProjectSessionSnapshot>,
) -> PreviewProjectionIntentReceipt {
    let kind = kind_from_message_type(&input.message_type);
    let requires_project_session = kind != PreviewProjectionIntentKind::Unsupported;
    let mut diagnostics = Vec::new();

    if kind == PreviewProjectionIntentKind::Unsupported {
        diagnostics.push(PreviewProjectionDiagnostic::blocking(
            "unsupported_preview_intent",
            LocalizedDiagnostic::new("preview-projection-unsupported-intent")
                .with_argument("type", input.message_type.trim()),
        ));
    }

    if requires_project_session && session.is_none() {
        diagnostics.push(PreviewProjectionDiagnostic::blocking(
            "missing_project_session",
            LocalizedDiagnostic::new("preview-projection-project-session-required"),
        ));
    }

    validate_intent_shape(kind, &input, &mut diagnostics);

    let has_blocking_diagnostic = diagnostics.iter().any(|diagnostic| diagnostic.blocking);
    let status = if kind == PreviewProjectionIntentKind::Unsupported {
        PreviewProjectionIntentStatus::Unsupported
    } else if has_blocking_diagnostic {
        PreviewProjectionIntentStatus::Blocked
    } else {
        PreviewProjectionIntentStatus::Accepted
    };
    let accepted = status == PreviewProjectionIntentStatus::Accepted;
    let effect = effect_for_kind(kind);
    let message_diagnostic = match status {
        PreviewProjectionIntentStatus::Accepted => {
            LocalizedDiagnostic::new("preview-projection-intent-accepted")
        }
        PreviewProjectionIntentStatus::Blocked => {
            LocalizedDiagnostic::new("preview-projection-intent-blocked")
        }
        PreviewProjectionIntentStatus::Unsupported => {
            LocalizedDiagnostic::new("preview-projection-intent-unsupported")
        }
    };

    PreviewProjectionIntentReceipt {
        schema_version: PREVIEW_PROJECTION_SCHEMA_VERSION,
        intent_id: intent_id(&input, kind),
        kind,
        status,
        effect,
        accepted,
        requires_project_session,
        project_session_id: session.map(|session| session.id.clone()),
        project_root: session.map(|session| session.project_root.clone()),
        runtime_session_id: session.map(ProjectSessionSnapshot::runtime_instance_id),
        preview_revision: input.preview_revision,
        message_diagnostic,
        diagnostics,
    }
}

fn kind_from_message_type(message_type: &str) -> PreviewProjectionIntentKind {
    match message_type.trim() {
        "preview-insert-drop" => PreviewProjectionIntentKind::HtmlInsertDrop,
        "preview-html-attributes" => PreviewProjectionIntentKind::HtmlAttributes,
        "preview-html-text" => PreviewProjectionIntentKind::HtmlText,
        "preview-html-tag" => PreviewProjectionIntentKind::HtmlTag,
        "preview-duplicate-selected" => PreviewProjectionIntentKind::HtmlDuplicate,
        "preview-tera-drop" => PreviewProjectionIntentKind::TeraInsertDrop,
        "preview-delete-selected" => PreviewProjectionIntentKind::HtmlDelete,
        "preview-template-delete-selected" => PreviewProjectionIntentKind::TemplateDelete,
        _ => PreviewProjectionIntentKind::Unsupported,
    }
}

fn effect_for_kind(kind: PreviewProjectionIntentKind) -> PreviewProjectionEffect {
    match kind {
        PreviewProjectionIntentKind::Unsupported => PreviewProjectionEffect::Unsupported,
        _ => PreviewProjectionEffect::KernelMutationPreflight,
    }
}

fn validate_intent_shape(
    kind: PreviewProjectionIntentKind,
    input: &PreviewProjectionIntentInput,
    diagnostics: &mut Vec<PreviewProjectionDiagnostic>,
) {
    match kind {
        PreviewProjectionIntentKind::HtmlInsertDrop => {
            require_text(diagnostics, "targetSourceId", &input.target_source_id);
            require_text(diagnostics, "targetTag", &input.target_tag);
            require_text(diagnostics, "elementTag", &input.element_tag);
            require_drop_position(diagnostics, &input.position);
        }
        PreviewProjectionIntentKind::HtmlAttributes => {
            require_text(diagnostics, "sourceId", &input.source_id);
            require_text(diagnostics, "sourceTag", &input.source_tag);
        }
        PreviewProjectionIntentKind::HtmlText => {
            require_text(diagnostics, "sourceId", &input.source_id);
            require_text(diagnostics, "sourceTag", &input.source_tag);
        }
        PreviewProjectionIntentKind::HtmlTag => {
            require_text(diagnostics, "sourceId", &input.source_id);
            require_text(diagnostics, "sourceTag", &input.source_tag);
            require_text(diagnostics, "elementTag", &input.element_tag);
        }
        PreviewProjectionIntentKind::HtmlDuplicate => {
            require_text(diagnostics, "sourceId", &input.source_id);
        }
        PreviewProjectionIntentKind::TeraInsertDrop => {
            require_text(diagnostics, "targetSourceId", &input.target_source_id);
            require_text(diagnostics, "targetTag", &input.target_tag);
            require_text(diagnostics, "itemKind", &input.item_kind);
            require_drop_position(diagnostics, &input.position);
        }
        PreviewProjectionIntentKind::HtmlDelete => {
            require_text(diagnostics, "sourceId", &input.source_id);
        }
        PreviewProjectionIntentKind::TemplateDelete => {
            require_text(diagnostics, "sourceId", &input.source_id);
        }
        PreviewProjectionIntentKind::Unsupported => {}
    }
}

fn require_text(
    diagnostics: &mut Vec<PreviewProjectionDiagnostic>,
    field: &str,
    value: &Option<String>,
) {
    if value
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        diagnostics.push(PreviewProjectionDiagnostic::blocking(
            format!("missing_{field}"),
            LocalizedDiagnostic::new("preview-projection-required-field-missing")
                .with_argument("field", field),
        ));
    }
}

fn require_drop_position(
    diagnostics: &mut Vec<PreviewProjectionDiagnostic>,
    value: &Option<String>,
) {
    let position = value.as_deref().map(str::trim).unwrap_or_default();
    if !DROP_POSITIONS.contains(&position) {
        diagnostics.push(PreviewProjectionDiagnostic::blocking(
            "invalid_position",
            LocalizedDiagnostic::new("preview-projection-position-invalid"),
        ));
    }
}

fn intent_id(input: &PreviewProjectionIntentInput, kind: PreviewProjectionIntentKind) -> String {
    let mut hasher = DefaultHasher::new();
    kind.operation_label().hash(&mut hasher);
    input.message_type.hash(&mut hasher);
    input.preview_revision.hash(&mut hasher);
    input.source_id.hash(&mut hasher);
    input.target_source_id.hash(&mut hasher);
    input.source_template_source_id.hash(&mut hasher);
    input.target_template_source_id.hash(&mut hasher);
    input.source_session_id.hash(&mut hasher);
    input.target_session_id.hash(&mut hasher);
    input.source_tag.hash(&mut hasher);
    input.target_tag.hash(&mut hasher);
    input.target_kind.hash(&mut hasher);
    input.position.hash(&mut hasher);
    input.item_kind.hash(&mut hasher);
    input.element_tag.hash(&mut hasher);
    format!("preview-intent-{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::project_session::ProjectSessionSnapshot;

    fn session() -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "session-1".to_string(),
            project_root: "/tmp/project".to_string(),
            zola_root: "/tmp/project".to_string(),
            session_dir: "/tmp/app/session-1".to_string(),
            manifest_path: "/tmp/app/session-1/manifest.json".to_string(),
            opened_at_ms: 10,
            last_seen_at_ms: 10,
            root_fingerprint: crate::kernel::project_session::ProjectRootFingerprint {
                canonical_path: "/tmp/project".to_string(),
                modified_ms: 10,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: crate::kernel::project_session::ProjectSessionScanSummary {
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        }
    }

    #[test]
    fn blocks_mutating_intent_without_session() {
        let receipt = preflight_preview_projection_intent(
            PreviewProjectionIntentInput {
                message_type: "preview-delete-selected".to_string(),
                source_id: Some("sgn_test_main_section".to_string()),
                ..PreviewProjectionIntentInput::default()
            },
            None,
        );

        assert!(!receipt.accepted);
        assert_eq!(receipt.status, PreviewProjectionIntentStatus::Blocked);
        assert!(receipt
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_project_session"));
    }

    #[test]
    fn blocks_invalid_drop_position() {
        let receipt = preflight_preview_projection_intent(
            PreviewProjectionIntentInput {
                message_type: "preview-insert-drop".to_string(),
                target_tag: Some("main".to_string()),
                element_tag: Some("section".to_string()),
                position: Some("around".to_string()),
                ..PreviewProjectionIntentInput::default()
            },
            Some(&session()),
        );

        assert!(!receipt.accepted);
        assert_eq!(receipt.status, PreviewProjectionIntentStatus::Blocked);
        assert!(receipt
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "invalid_position"));
    }

    #[test]
    fn rejects_unsupported_messages() {
        let receipt = preflight_preview_projection_intent(
            PreviewProjectionIntentInput {
                message_type: "preview-hover".to_string(),
                ..PreviewProjectionIntentInput::default()
            },
            Some(&session()),
        );

        assert!(!receipt.accepted);
        assert_eq!(receipt.status, PreviewProjectionIntentStatus::Unsupported);
        assert_eq!(receipt.kind, PreviewProjectionIntentKind::Unsupported);
    }
}
