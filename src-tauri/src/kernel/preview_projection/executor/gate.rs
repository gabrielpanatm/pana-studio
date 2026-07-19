use crate::kernel::project_session::ProjectSessionSnapshot;

use super::{
    super::{
        model::{
            PreviewProjectionDiagnostic, PreviewProjectionIntentInput,
            PreviewProjectionIntentReceipt, PreviewProjectionIntentStatus,
        },
        preflight::preflight_preview_projection_intent,
    },
    spec::PreviewExecutorIntentSpec,
};

#[derive(Debug)]
pub(super) struct PreviewExecutorIntentBlocked {
    pub(super) intent_receipt: PreviewProjectionIntentReceipt,
    pub(super) diagnostic: Option<PreviewProjectionDiagnostic>,
}

pub(super) fn require_preview_executor_intent(
    input: PreviewProjectionIntentInput,
    session: &ProjectSessionSnapshot,
    spec: PreviewExecutorIntentSpec,
) -> Result<PreviewProjectionIntentReceipt, PreviewExecutorIntentBlocked> {
    let intent_receipt = preflight_preview_projection_intent(input, Some(session));
    let wrong_kind = intent_receipt.kind != spec.expected_kind;
    if intent_receipt.status == PreviewProjectionIntentStatus::Accepted && !wrong_kind {
        return Ok(intent_receipt);
    }

    let diagnostic = if wrong_kind {
        Some(PreviewProjectionDiagnostic::blocking(
            spec.wrong_kind_code,
            spec.wrong_kind_message,
        ))
    } else {
        None
    };

    Err(PreviewExecutorIntentBlocked {
        intent_receipt,
        diagnostic,
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::model::PreviewProjectionIntentKind;
    use super::super::spec::{HTML_DELETE_INTENT, LAYER_DROP_INTENT};
    use super::*;

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
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        }
    }

    #[test]
    fn require_preview_executor_intent_accepts_matching_preflight() {
        let input = PreviewProjectionIntentInput {
            message_type: "preview-delete-selected".to_string(),
            selector: Some(".hero".to_string()),
            ..Default::default()
        };

        let receipt = require_preview_executor_intent(input, &session(), HTML_DELETE_INTENT)
            .expect("matching accepted intent should pass executor gate");

        assert_eq!(receipt.kind, PreviewProjectionIntentKind::HtmlDelete);
        assert_eq!(receipt.status, PreviewProjectionIntentStatus::Accepted);
        assert!(receipt.diagnostics.is_empty());
    }

    #[test]
    fn require_preview_executor_intent_blocks_preflight_without_extra_diagnostic() {
        let input = PreviewProjectionIntentInput {
            message_type: "preview-delete-selected".to_string(),
            ..Default::default()
        };

        let blocked = require_preview_executor_intent(input, &session(), HTML_DELETE_INTENT)
            .expect_err("invalid shape should remain blocked by preflight");

        assert_eq!(
            blocked.intent_receipt.status,
            PreviewProjectionIntentStatus::Blocked
        );
        assert!(blocked.diagnostic.is_none());
        assert_eq!(blocked.intent_receipt.diagnostics.len(), 1);
    }

    #[test]
    fn require_preview_executor_intent_adds_wrong_kind_diagnostic() {
        let input = PreviewProjectionIntentInput {
            message_type: "preview-delete-selected".to_string(),
            selector: Some(".hero".to_string()),
            ..Default::default()
        };

        let blocked = require_preview_executor_intent(input, &session(), LAYER_DROP_INTENT)
            .expect_err("accepted preflight with wrong executor kind should be blocked");

        assert_eq!(
            blocked.intent_receipt.status,
            PreviewProjectionIntentStatus::Accepted
        );
        assert_eq!(
            blocked.diagnostic.expect("wrong kind diagnostic").code,
            "preview_layer_drop_wrong_intent_kind"
        );
    }
}
