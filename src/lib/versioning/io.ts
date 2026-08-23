import { invoke } from "@tauri-apps/api/core";
import type {
  VersionDiffInput,
  VersionDiffReceipt,
  VersionHistoryPage,
  VersioningCommitReceipt,
  VersioningMutationIdentity,
  VersioningMutationReceipt,
  VersioningSessionIdentity,
  VersioningSnapshot,
  VersionIntegrationMode,
  VersionIntegrationPlan,
  VersionIntegrationReceipt,
  VersionIntegrationRecoveryAction,
  VersionIntegrationRecoveryResolutionReceipt,
  VersionIntegrationRecoveryScan,
  VersionNetworkCancelReceipt,
  VersionNetworkReceipt,
  VersionPreviewReceipt,
  VersionRestoreReceipt,
  VersionRestoreRecoveryAction,
  VersionRestoreRecoveryResolutionReceipt,
  VersionRestoreRecoveryScan,
} from "$lib/versioning/contracts";

export function readVersioningSnapshot(identity: VersioningSessionIdentity): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("read_versioning_snapshot", { identity });
}

export function initializeVersioning(identity: VersioningMutationIdentity): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("initialize_versioning", { identity });
}

export function configureVersioningIdentity(
  identity: VersioningMutationIdentity,
  input: { name: string; email: string },
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("configure_versioning_identity", { identity, input });
}

export function configureVersionRemote(
  identity: VersioningMutationIdentity,
  input: { name: string; fetchUrl: string; pushUrl?: string | null },
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("configure_version_remote", { identity, input });
}

export function removeVersionRemote(
  identity: VersioningMutationIdentity,
  name: string,
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("remove_version_remote", { identity, input: { name } });
}

export function configureVersionUpstream(
  identity: VersioningMutationIdentity,
  input: { localBranch: string; remote: string; remoteBranch: string },
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("configure_version_upstream", { identity, input });
}

export function clearVersionUpstream(
  identity: VersioningMutationIdentity,
  name: string,
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("clear_version_upstream", { identity, input: { name } });
}

export function createVersionBranch(
  identity: VersioningMutationIdentity,
  name: string,
  startOid?: string | null,
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("create_version_branch", {
    identity,
    input: { name, startOid: startOid ?? null },
  });
}

export function deleteVersionBranch(
  identity: VersioningMutationIdentity,
  name: string,
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("delete_version_branch", { identity, input: { name } });
}

export function fetchVersionRemote(
  identity: VersioningMutationIdentity,
  input: { operationId: string; remote: string; prune: boolean },
): Promise<VersionNetworkReceipt> {
  return invoke<VersionNetworkReceipt>("fetch_version_remote", { identity, input });
}

export function pushVersionBranch(
  identity: VersioningMutationIdentity,
  input: { operationId: string; remote: string; remoteBranch: string; setUpstream: boolean },
): Promise<VersionNetworkReceipt> {
  return invoke<VersionNetworkReceipt>("push_version_branch", { identity, input });
}

export function cancelVersionNetworkOperation(
  identity: VersioningSessionIdentity,
  operationId: string,
): Promise<VersionNetworkCancelReceipt> {
  return invoke<VersionNetworkCancelReceipt>("cancel_version_network_operation", {
    identity,
    input: { operationId },
  });
}

export function readVersionIntegrationPlan(
  identity: VersioningSessionIdentity,
  targetRef: string,
  expectedTargetOid: string,
): Promise<VersionIntegrationPlan> {
  return invoke<VersionIntegrationPlan>("read_version_integration_plan", {
    identity,
    input: { targetRef, expectedTargetOid },
  });
}

export function integrateVersionTarget(
  identity: VersioningMutationIdentity,
  input: {
    targetRef: string;
    expectedTargetOid: string;
    mode: VersionIntegrationMode;
    message: string;
  },
): Promise<VersionIntegrationReceipt> {
  return invoke<VersionIntegrationReceipt>("integrate_version_target", { identity, input });
}

export function switchVersionBranch(
  identity: VersioningMutationIdentity,
  branch: string,
  expectedTargetOid: string,
): Promise<VersionIntegrationReceipt> {
  return invoke<VersionIntegrationReceipt>("switch_version_branch", {
    identity,
    input: { branch, expectedTargetOid },
  });
}

export function readVersionIntegrationRecovery(
  identity: VersioningSessionIdentity,
): Promise<VersionIntegrationRecoveryScan> {
  return invoke<VersionIntegrationRecoveryScan>("read_version_integration_recovery", { identity });
}

export function resolveVersionIntegrationRecovery(
  identity: VersioningMutationIdentity,
  recoveryRef: string,
  action: VersionIntegrationRecoveryAction,
): Promise<VersionIntegrationRecoveryResolutionReceipt> {
  return invoke<VersionIntegrationRecoveryResolutionReceipt>(
    "resolve_version_integration_recovery",
    { identity, input: { recoveryRef, action } },
  );
}

export function stageVersioningPaths(
  identity: VersioningMutationIdentity,
  paths: string[],
): Promise<VersioningMutationReceipt> {
  return invoke<VersioningMutationReceipt>("stage_versioning_paths", { identity, input: { paths } });
}

export function stageAllVersioning(
  identity: VersioningMutationIdentity,
): Promise<VersioningMutationReceipt> {
  return invoke<VersioningMutationReceipt>("stage_all_versioning", { identity });
}

export function unstageVersioningPaths(
  identity: VersioningMutationIdentity,
  paths: string[],
): Promise<VersioningMutationReceipt> {
  return invoke<VersioningMutationReceipt>("unstage_versioning_paths", { identity, input: { paths } });
}

export function unstageAllVersioning(
  identity: VersioningMutationIdentity,
): Promise<VersioningMutationReceipt> {
  return invoke<VersioningMutationReceipt>("unstage_all_versioning", { identity });
}

export function commitVersioning(
  identity: VersioningMutationIdentity,
  message: string,
): Promise<VersioningCommitReceipt> {
  return invoke<VersioningCommitReceipt>("commit_versioning", { identity, input: { message } });
}

export function readVersionHistory(
  identity: VersioningSessionIdentity,
  offset = 0,
  limit = 30,
): Promise<VersionHistoryPage> {
  return invoke<VersionHistoryPage>("read_version_history", { identity, offset, limit });
}

export function readVersionDiff(
  identity: VersioningSessionIdentity,
  input: VersionDiffInput,
): Promise<VersionDiffReceipt> {
  return invoke<VersionDiffReceipt>("read_version_diff", { identity, input });
}

export function previewVersion(
  identity: VersioningSessionIdentity,
  commitOid: string,
): Promise<VersionPreviewReceipt> {
  return invoke<VersionPreviewReceipt>("preview_version", { identity, input: { commitOid } });
}

export function stopVersionPreview(identity: VersioningSessionIdentity): Promise<void> {
  return invoke<void>("stop_version_preview", { identity });
}

export function restoreVersioning(
  identity: VersioningMutationIdentity,
  targetCommitOid: string,
  message: string,
): Promise<VersionRestoreReceipt> {
  return invoke<VersionRestoreReceipt>("restore_version", {
    identity,
    input: { targetCommitOid, message },
  });
}

export function readVersionRestoreRecovery(
  identity: VersioningSessionIdentity,
): Promise<VersionRestoreRecoveryScan> {
  return invoke<VersionRestoreRecoveryScan>("read_version_restore_recovery", { identity });
}

export function resolveVersionRestoreRecovery(
  identity: VersioningMutationIdentity,
  recoveryRef: string,
  action: VersionRestoreRecoveryAction,
): Promise<VersionRestoreRecoveryResolutionReceipt> {
  return invoke<VersionRestoreRecoveryResolutionReceipt>(
    "resolve_version_restore_recovery",
    { identity, input: { recoveryRef, action } },
  );
}
