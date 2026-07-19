use serde::{Deserialize, Serialize};

use crate::kernel::project_session::ProjectSessionSnapshot;

use super::FileBufferStore;

pub const FILE_BUFFER_IDENTITY_INVALID_CODE: &str = "file_buffer_identity_invalid";
pub const FILE_BUFFER_STALE_SESSION_CODE: &str = "file_buffer_stale_session";
pub const FILE_BUFFER_STORE_SESSION_MISMATCH_CODE: &str = "file_buffer_store_session_mismatch";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferRequestIdentity {
    pub expected_project_root: String,
    pub expected_session_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileBufferCommandReceipt<T> {
    pub project_root: String,
    pub runtime_session_id: String,
    pub payload: T,
}

impl<T> FileBufferCommandReceipt<T> {
    pub fn new(session: &ProjectSessionSnapshot, payload: T) -> Self {
        Self {
            project_root: session.project_root.clone(),
            runtime_session_id: session.runtime_instance_id(),
            payload,
        }
    }
}

pub fn require_file_buffer_session_binding(
    current_project_root: &str,
    session: &ProjectSessionSnapshot,
    store: &FileBufferStore,
    identity: &FileBufferRequestIdentity,
) -> Result<(), String> {
    if identity.expected_project_root.trim().is_empty()
        || identity.expected_session_id.trim().is_empty()
    {
        return Err(format!(
            "[{FILE_BUFFER_IDENTITY_INVALID_CODE}] FileBufferStore cere expectedProjectRoot și expectedSessionId nenule."
        ));
    }

    let live_runtime_session_id = session.runtime_instance_id();
    if identity.expected_project_root != session.project_root
        || identity.expected_session_id != live_runtime_session_id
    {
        return Err(format!(
            "[{FILE_BUFFER_STALE_SESSION_CODE}] FileBufferStore a refuzat requestul stale: așteptat root/session {}/{}, activ {}/{}.",
            identity.expected_project_root,
            identity.expected_session_id,
            session.project_root,
            live_runtime_session_id,
        ));
    }

    if current_project_root != session.project_root
        || store.project_root != session.project_root
        || store.session_id != session.id
        || store.runtime_session_id != live_runtime_session_id
    {
        return Err(format!(
            "[{FILE_BUFFER_STORE_SESSION_MISMATCH_CODE}] FileBufferStore nu aparține ProjectSession active: current root {}, store root/session/runtime {}/{}/{}, ProjectSession root/session/runtime {}/{}/{}.",
            current_project_root,
            store.project_root,
            store.session_id,
            store.runtime_session_id,
            session.project_root,
            session.id,
            live_runtime_session_id,
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::{
        file_buffer_store::FileBufferStoreLimits,
        project_session::{
            ProjectRootFingerprint, ProjectSessionScanSummary, ProjectSessionSnapshot,
        },
    };

    fn session(root: &str, opened_at_ms: u128) -> ProjectSessionSnapshot {
        ProjectSessionSnapshot {
            schema_version: 1,
            id: "stable-project".to_string(),
            project_root: root.to_string(),
            zola_root: format!("{root}/sursa"),
            session_dir: "/tmp/session".to_string(),
            manifest_path: "/tmp/session/manifest.json".to_string(),
            opened_at_ms,
            last_seen_at_ms: opened_at_ms,
            root_fingerprint: ProjectRootFingerprint {
                canonical_path: root.to_string(),
                modified_ms: 0,
                size: 0,
                readonly: false,
                unix_device: None,
                unix_inode: None,
            },
            scan_summary: ProjectSessionScanSummary {
                is_zola: true,
                is_empty: false,
                active_theme: None,
                file_count: 0,
                directory_count: 0,
            },
        }
    }

    fn store(session: &ProjectSessionSnapshot) -> FileBufferStore {
        FileBufferStore::for_project_session(
            session,
            session.opened_at_ms,
            FileBufferStoreLimits {
                max_files: 10,
                max_file_bytes: 1024,
                max_total_bytes: 4096,
            },
        )
    }

    #[test]
    fn same_root_reopen_rejects_previous_runtime_identity() {
        let runtime_a = session("/project", 1);
        let runtime_b = session("/project", 2);
        let identity_a = FileBufferRequestIdentity {
            expected_project_root: runtime_a.project_root.clone(),
            expected_session_id: runtime_a.runtime_instance_id(),
        };
        let store_b = store(&runtime_b);

        let error =
            require_file_buffer_session_binding("/project", &runtime_b, &store_b, &identity_a)
                .unwrap_err();
        assert!(error.contains(FILE_BUFFER_STALE_SESSION_CODE));
    }

    #[test]
    fn receipt_carries_the_exact_runtime_not_only_the_root() {
        let runtime = session("/project", 7);
        let receipt = FileBufferCommandReceipt::new(&runtime, "payload");
        assert_eq!(receipt.project_root, "/project");
        assert_eq!(receipt.runtime_session_id, runtime.runtime_instance_id());
        assert_eq!(receipt.payload, "payload");
    }
}
