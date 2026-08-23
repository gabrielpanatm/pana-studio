use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    kernel::{
        observability::{append_event, KernelEventKind, KernelLogEvent, KernelLogLevel},
        project_runtime_access::require_recovery_coordinator_clean_for_write,
        project_session::ProjectSessionSnapshot,
        project_workspace::{
            emit_project_workspace_mutated, save_project_workspace_with_recovery, ProjectWorkspace,
            ProjectWorkspaceIdentity, ProjectWorkspacePreviewProjection, ProjectWorkspaceSaveError,
            WorkspaceMutationMetadata,
        },
        write_authority::{ActiveProjectReadLease, WriteAuthorityRuntime},
    },
    preview::{
        preprocess::materialize_version_source_tree, start_version_source_browser,
        stop_version_source_browser,
    },
    state::AppState,
    versioning::{
        build_version_restore_plan, classify_network_publication_error,
        classify_network_runtime_error, execute_version_network_phases, network_progress_text,
        redact_network_text, validate_operation_id, PreparedVersionIntegration,
        PreparedVersionNetworkOperation, PreparedVersionRestore, VersionBranchInput,
        VersionBranchNameInput, VersionDiffInput, VersionDiffReceipt, VersionFetchInput,
        VersionHistoryPage, VersionIntegrationInput, VersionIntegrationKind,
        VersionIntegrationPlan, VersionIntegrationReceipt, VersionIntegrationRecoveryAction,
        VersionIntegrationRecoveryItem, VersionIntegrationRecoveryResolutionInput,
        VersionIntegrationRecoveryResolutionReceipt, VersionIntegrationRecoveryScan,
        VersionIntegrationRecoveryState, VersionIntegrationRelationship, VersionIntegrationStatus,
        VersionIntegrationTargetInput, VersionNetworkCancelInput, VersionNetworkCancelReceipt,
        VersionNetworkOperationKind, VersionNetworkOperationLease, VersionNetworkOperationStatus,
        VersionNetworkProgressEvent, VersionNetworkReceipt, VersionPreviewInput,
        VersionPreviewReceipt, VersionPushInput, VersionRemoteInput, VersionRemoteNameInput,
        VersionRepository, VersionRestoreExpectedFile, VersionRestoreInput, VersionRestoreReceipt,
        VersionRestoreRecoveryAction, VersionRestoreRecoveryItem,
        VersionRestoreRecoveryResolutionInput, VersionRestoreRecoveryResolutionReceipt,
        VersionRestoreRecoveryScan, VersionRestoreRecoveryState, VersionRestoreStatus,
        VersionSwitchBranchInput, VersionTree, VersionUpstreamInput, VersioningCommitInput,
        VersioningCommitReceipt, VersioningIdentityInput, VersioningMutationIdentity,
        VersioningMutationReceipt, VersioningPathsInput, VersioningSessionIdentity,
        VersioningSnapshot, VERSIONING_SCHEMA_VERSION,
    },
};

mod integration;
mod local;
mod network;
mod observability;
mod publication;
mod restore;
mod session;

pub use integration::{
    integrate_version_target, read_version_integration_plan, read_version_integration_recovery,
    resolve_version_integration_recovery, switch_version_branch,
};
pub use local::{
    clear_version_upstream, commit_versioning, configure_version_remote,
    configure_version_upstream, configure_versioning_identity, create_version_branch,
    delete_version_branch, initialize_versioning, preview_version, read_version_diff,
    read_version_history, read_versioning_snapshot, remove_version_remote, stage_all_versioning,
    stage_versioning_paths, stop_version_preview, unstage_all_versioning, unstage_versioning_paths,
};
pub use network::{cancel_version_network_operation, fetch_version_remote, push_version_branch};
pub use restore::{
    read_version_restore_recovery, resolve_version_restore_recovery, restore_version,
};
