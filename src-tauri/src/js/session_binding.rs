use serde::{Deserialize, Serialize};

use crate::kernel::{
    file_buffer_store::{
        require_file_buffer_session_binding, FileBufferRequestIdentity, FileBufferStore,
    },
    project_session::ProjectSessionSnapshot,
};

use super::PageJsDraftStore;

pub const PAGE_JS_IDENTITY_INVALID_CODE: &str = "page_js_identity_invalid";
pub const PAGE_JS_STALE_SESSION_CODE: &str = "page_js_stale_session";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsRequestIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageJsCommandReceipt<T> {
    pub project_root: String,
    pub runtime_session_id: String,
    pub payload: T,
}

impl<T> PageJsCommandReceipt<T> {
    pub fn new(session: &ProjectSessionSnapshot, payload: T) -> Self {
        Self {
            project_root: session.project_root.clone(),
            runtime_session_id: session.runtime_instance_id(),
            payload,
        }
    }
}

pub fn require_page_js_session_identity(
    current_project_root: &str,
    session: &ProjectSessionSnapshot,
    identity: &PageJsRequestIdentity,
) -> Result<(), String> {
    if identity.expected_project_root.trim().is_empty()
        || identity.expected_session_id.trim().is_empty()
    {
        return Err(format!(
            "[{PAGE_JS_IDENTITY_INVALID_CODE}] Page JS cere expectedProjectRoot și expectedSessionId nenule."
        ));
    }

    let live_runtime_session_id = session.runtime_instance_id();
    if current_project_root != session.project_root
        || identity.expected_project_root != session.project_root
        || identity.expected_session_id != live_runtime_session_id
    {
        return Err(format!(
            "[{PAGE_JS_STALE_SESSION_CODE}] Page JS a refuzat request-ul stale: așteptat root/session {}/{}, activ {}/{} (current root {}).",
            identity.expected_project_root,
            identity.expected_session_id,
            session.project_root,
            live_runtime_session_id,
            current_project_root,
        ));
    }

    Ok(())
}

pub fn require_page_js_file_buffer_identity(
    current_project_root: &str,
    session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
    identity: &PageJsRequestIdentity,
) -> Result<(), String> {
    require_page_js_session_identity(current_project_root, session, identity)?;
    require_file_buffer_session_binding(
        current_project_root,
        session,
        store,
        &FileBufferRequestIdentity {
            expected_project_root: identity.expected_project_root.clone(),
            expected_session_id: identity.expected_session_id.clone(),
        },
    )
}

pub fn require_page_js_draft_session_identity(
    current_project_root: &str,
    session: &ProjectSessionSnapshot,
    draft_store: &PageJsDraftStore,
    identity: &PageJsRequestIdentity,
) -> Result<(), String> {
    require_page_js_session_identity(current_project_root, session, identity)?;
    draft_store.require_identity(
        &identity.expected_project_root,
        &identity.expected_session_id,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{
        require_page_js_file_buffer_identity, require_page_js_session_identity,
        PageJsRequestIdentity,
    };
    use crate::kernel::{
        file_buffer_store::{FileBufferStore, FileBufferStoreLimits},
        project_session::{
            ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
        },
    };

    fn session(opened_at_ms: u128) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "stable-project-session".to_string(),
            project_root: "/tmp/page-js-session-bound".to_string(),
            zola_root: "/tmp/page-js-session-bound/sursa".to_string(),
            session_dir: "/tmp/page-js-session-state".to_string(),
            manifest_path: "/tmp/page-js-session-state/session.json".to_string(),
            opened_at_ms,
            last_seen_at_ms: opened_at_ms,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: "/tmp/page-js-session-bound".to_string(),
                modified_ms: 1,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 1,
                directory_count: 1,
            },
        }
    }

    fn identity(session: &ProjectSessionSnapshot) -> PageJsRequestIdentity {
        PageJsRequestIdentity {
            expected_project_root: session.project_root.clone(),
            expected_session_id: session.runtime_instance_id(),
        }
    }

    #[test]
    fn same_root_previous_runtime_is_rejected_before_operation() {
        let previous = session(1);
        let current = session(2);
        let side_effect = Cell::new(false);

        let validation =
            require_page_js_session_identity(&current.project_root, &current, &identity(&previous));
        if validation.is_ok() {
            side_effect.set(true);
        }

        assert!(validation.unwrap_err().contains("page_js_stale_session"));
        assert!(!side_effect.get());
    }

    #[test]
    fn file_buffer_from_previous_same_root_runtime_is_rejected() {
        let previous = session(1);
        let current = session(2);
        let store = FileBufferStore::for_project_session(
            &previous,
            1,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        );

        let error = require_page_js_file_buffer_identity(
            &current.project_root,
            &current,
            &store,
            &identity(&current),
        )
        .unwrap_err();

        assert!(error.contains("file_buffer_store_session_mismatch"));
    }
}
