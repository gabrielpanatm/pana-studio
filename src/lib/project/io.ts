import type {
  CssProperty,
  CssRuleContext,
  CssMutationCommandReceipt,
  EditableStyles,
  FontInventory,
  FileBufferChangeSetInput,
  FileBufferChangeSetResult,
  FileBufferCommandReceipt,
  FileBufferFileSnapshot,
  FileBufferMutationExpectation,
  FileBufferRequestIdentity,
  FileBufferStoreSnapshot,
  FileBufferTextSnapshot,
  GoogleFontCatalogFamily,
  GoogleFontDownloadResult,
  PageCssTarget,
  PageCssCleanupResult,
  PageCssWriteResult,
  UiContextProjection,
  AiContextStatus,
  AiCoordinationSnapshot,
  CodexMcpStatus,
  EditTransitionReceipt,
  PageAssetContractApplyInput,
  PageAssetContractInput,
  PageAssetContractApplyReceipt,
  PageAssetContractPlan,
  PageComponentContractApplyReceipt,
  PageComponentContractApplyInput,
  PageComponentContractInput,
  PageComponentContractPlan,
  PageComponentRegistrySnapshot,
  PreviewHtmlDeleteExecutionInput,
  PreviewHtmlDeleteExecutionReceipt,
  PreviewHtmlAttributesExecutionInput,
  PreviewHtmlAttributesExecutionReceipt,
  PreviewHtmlTagExecutionInput,
  PreviewHtmlTagExecutionReceipt,
  PreviewHtmlTextExecutionInput,
  PreviewHtmlTextExecutionReceipt,
  PreviewHtmlDuplicateExecutionInput,
  PreviewHtmlDuplicateExecutionReceipt,
  PageJsConfig,
  MotionTimelineStepTimingInput,
  MotionTimelineStepTimingReceipt,
  PreviewHtmlInsertDropExecutionInput,
  PreviewHtmlInsertDropExecutionReceipt,
  PageJsDraftStageInput,
  PageJsDraftStageReceipt,
  PageJsDraftSessionIdentity,
  PageJsDraftStoreSnapshot,
  PageJsCommandReceipt,
  PageJsWorkspaceState,
  PageJsRequestIdentity,
  PreviewLayerDropExecutionInput,
  PreviewLayerDropExecutionReceipt,
  PreviewProjectionIntentInput,
  PreviewProjectionIntentReceipt,
  PreviewTemplateEditPermissionInput,
  PreviewTemplateEditPermissionReceipt,
  PreviewTeraDeleteExecutionInput,
  PreviewTeraDeleteExecutionReceipt,
  PreviewTeraInsertDropExecutionInput,
  PreviewTeraInsertDropExecutionReceipt,
  PreviewTeraMoveDropExecutionInput,
  PreviewTeraMoveDropExecutionReceipt,
  PreviewStructuralCommandIdentity,
  ProjectAppConfig,
  ProjectDiskManifest,
  ProjectHtmlMoveIntent,
  ProjectHtmlMovePlan,
  ProjectModelSnapshot,
  TemplateWorkbenchPlan,
  ProjectOpenRecoveryAssessment,
  ProjectOpenRecoveryDecisionInput,
  ProjectScan,
  ProjectSessionSnapshot,
  ProjectWorkspaceHistoryIdentity,
  ProjectWorkspaceIdentity,
  ProjectWorkspaceSaveReceipt,
  ProjectWorkspaceSaveRecoveryAction,
  ProjectWorkspaceSaveRecoveryCommandResult,
  ProjectWorkspaceSnapshot,
  ProjectWorkspaceUndoRedoCommandReceipt,
  WorkspaceHistorySnapshot,
  KernelDiskConflictSnapshot,
  KernelExternalDiskReconcileInput,
  KernelExternalDiskReconcileReceipt,
  WorkspaceEntryMutationReceipt,
  KernelLogLevel,
  KernelObservabilityLogSnapshot,
  KernelObservabilityLogSourceFilter,
  WriteAuthorityRecoveryResolutionInput,
  WriteAuthorityRecoveryResolutionReceipt,
  WriteAuthorityRecoveryScan,
  KernelProjectTransitionAction,
  KernelProjectTransitionBlockedAuditSnapshot,
  KernelProjectTransitionDecisionJournalSnapshot,
  KernelProjectTransitionDecisionRecoveryAckJournalSnapshot,
  KernelProjectTransitionDecisionRecoveryAckReceipt,
  KernelProjectTransitionDecisionRetentionHotJournal,
  KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult,
  KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
  KernelProjectTransitionDecisionRetentionReceipt,
  KernelProjectTransitionDecisionReceipt,
  KernelProjectTransitionPolicy,
  KernelProjectTransitionPolicyMatrixSnapshot,
  RecoveryCoordinatorScan,
  ScssVariable,
  SourceGraph,
  SiteArchiveStructureReceipt,
  SitePageStructureReceipt,
  SitePartialIncludeReceipt,
  SitePartialStructureReceipt,
  SiteSingleStructureReceipt,
  SiteStructureAuthorityReceipt,
  ZolaProjectSettings,
  UiQuiescenceAcknowledgement,
  VersionDiffInput,
  VersionDiffReceipt,
  VersionHistoryPage,
  VersioningCommitReceipt,
  VersioningMutationIdentity,
  VersioningMutationReceipt,
  VersioningSessionIdentity,
  VersioningSnapshot,
  VersionNetworkCancelReceipt,
  VersionNetworkReceipt,
  VersionSyncComparison,
  VersionIntegrationMode,
  VersionIntegrationPlan,
  VersionIntegrationReceipt,
  VersionIntegrationRecoveryAction,
  VersionIntegrationRecoveryResolutionReceipt,
  VersionIntegrationRecoveryScan,
  VersionPreviewReceipt,
  VersionRestoreReceipt,
  VersionRestoreRecoveryAction,
  VersionRestoreRecoveryResolutionReceipt,
  VersionRestoreRecoveryScan,
} from "$lib/types";
import { PROJECT_WORKSPACE_SCHEMA_VERSION } from "$lib/types";
import { invoke } from "@tauri-apps/api/core";
import { homeDir } from "@tauri-apps/api/path";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { MoodBoardAssetReceipt, MoodBoardRequestIdentity } from "$lib/mood-board/io";

export function openProject(
  path: string,
  operatorDecisionId?: string,
  recoveryDecision?: ProjectOpenRecoveryDecisionInput,
): Promise<ProjectScan> {
  return invoke<ProjectScan>("open_project", {
    path,
    operatorDecisionId,
    recoveryDecision,
  });
}

export function inspectProjectOpenRecovery(
  path: string,
): Promise<ProjectOpenRecoveryAssessment> {
  return invoke<ProjectOpenRecoveryAssessment>("inspect_project_open_recovery", { path });
}

export function closeProject(operatorDecisionId?: string): Promise<void> {
  return invoke<void>("close_project", { operatorDecisionId });
}

export function readProjectSession(): Promise<ProjectSessionSnapshot | null> {
  return invoke<ProjectSessionSnapshot | null>("read_project_session");
}

export function reattachProjectSession(): Promise<ProjectScan | null> {
  return invoke<ProjectScan | null>("reattach_project_session");
}

export function readVersioningSnapshot(
  identity: VersioningSessionIdentity,
): Promise<VersioningSnapshot> {
  return invoke<VersioningSnapshot>("read_versioning_snapshot", { identity });
}

export function initializeVersioning(
  identity: VersioningMutationIdentity,
): Promise<VersioningSnapshot> {
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
  return invoke<VersioningSnapshot>("remove_version_remote", {
    identity,
    input: { name },
  });
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
  return invoke<VersioningSnapshot>("clear_version_upstream", {
    identity,
    input: { name },
  });
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
  return invoke<VersioningSnapshot>("delete_version_branch", {
    identity,
    input: { name },
  });
}

export function fetchVersionRemote(
  identity: VersioningMutationIdentity,
  input: { operationId: string; remote: string; prune: boolean },
): Promise<VersionNetworkReceipt> {
  return invoke<VersionNetworkReceipt>("fetch_version_remote", { identity, input });
}

export function pushVersionBranch(
  identity: VersioningMutationIdentity,
  input: {
    operationId: string;
    remote: string;
    remoteBranch: string;
    setUpstream: boolean;
  },
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

export function readVersionSyncComparison(
  identity: VersioningSessionIdentity,
): Promise<VersionSyncComparison> {
  return invoke<VersionSyncComparison>("read_version_sync_comparison", { identity });
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
  return invoke<VersionIntegrationRecoveryScan>("read_version_integration_recovery", {
    identity,
  });
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
  return invoke<VersioningMutationReceipt>("stage_versioning_paths", {
    identity,
    input: { paths },
  });
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
  return invoke<VersioningMutationReceipt>("unstage_versioning_paths", {
    identity,
    input: { paths },
  });
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
  return invoke<VersioningCommitReceipt>("commit_versioning", {
    identity,
    input: { message },
  });
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
  return invoke<VersionPreviewReceipt>("preview_version", {
    identity,
    input: { commitOid },
  });
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

export function normalizePreviewProjectionIntent(
  input: PreviewProjectionIntentInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewProjectionIntentReceipt> {
  return invoke<PreviewProjectionIntentReceipt>("normalize_preview_projection_intent", { input, identity });
}

export function executePreviewTemplateEditIntent(
  input: PreviewTemplateEditPermissionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewTemplateEditPermissionReceipt> {
  return invoke<PreviewTemplateEditPermissionReceipt>("execute_preview_template_edit_intent", { input, identity });
}

export function executePreviewLayerDropIntent(
  input: PreviewLayerDropExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewLayerDropExecutionReceipt> {
  return invoke<PreviewLayerDropExecutionReceipt>("execute_preview_layer_drop_intent", { input, identity });
}

export function executePreviewHtmlInsertDropIntent(
  input: PreviewHtmlInsertDropExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlInsertDropExecutionReceipt> {
  return invoke<PreviewHtmlInsertDropExecutionReceipt>("execute_preview_html_insert_drop_intent", { input, identity });
}

export function executePreviewHtmlAttributesIntent(
  input: PreviewHtmlAttributesExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlAttributesExecutionReceipt> {
  return invoke<PreviewHtmlAttributesExecutionReceipt>("execute_preview_html_attributes_intent", { input, identity });
}

export function executePreviewHtmlTextIntent(
  input: PreviewHtmlTextExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlTextExecutionReceipt> {
  return invoke<PreviewHtmlTextExecutionReceipt>("execute_preview_html_text_intent", { input, identity });
}

export function executePreviewHtmlTagIntent(
  input: PreviewHtmlTagExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlTagExecutionReceipt> {
  return invoke<PreviewHtmlTagExecutionReceipt>("execute_preview_html_tag_intent", { input, identity });
}

export function executePreviewHtmlDuplicateIntent(
  input: PreviewHtmlDuplicateExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlDuplicateExecutionReceipt> {
  return invoke<PreviewHtmlDuplicateExecutionReceipt>("execute_preview_html_duplicate_intent", { input, identity });
}

export function executePreviewHtmlDeleteIntent(
  input: PreviewHtmlDeleteExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewHtmlDeleteExecutionReceipt> {
  return invoke<PreviewHtmlDeleteExecutionReceipt>("execute_preview_html_delete_intent", { input, identity });
}

export function executePreviewTeraDeleteIntent(
  input: PreviewTeraDeleteExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewTeraDeleteExecutionReceipt> {
  return invoke<PreviewTeraDeleteExecutionReceipt>("execute_preview_tera_delete_intent", { input, identity });
}

export function executePreviewTeraInsertDropIntent(
  input: PreviewTeraInsertDropExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewTeraInsertDropExecutionReceipt> {
  return invoke<PreviewTeraInsertDropExecutionReceipt>("execute_preview_tera_insert_drop_intent", { input, identity });
}

export function executePreviewTeraMoveDropIntent(
  input: PreviewTeraMoveDropExecutionInput,
  identity: PreviewStructuralCommandIdentity,
): Promise<PreviewTeraMoveDropExecutionReceipt> {
  return invoke<PreviewTeraMoveDropExecutionReceipt>("execute_preview_tera_move_drop_intent", { input, identity });
}

export function planPageComponentContract(
  input: PageComponentContractInput,
): Promise<PageComponentContractPlan> {
  return invoke<PageComponentContractPlan>("plan_page_component_contract", { input });
}

export function applyPageComponentContract(
  input: PageComponentContractApplyInput,
): Promise<PageComponentContractApplyReceipt> {
  return invoke<PageComponentContractApplyReceipt>("apply_page_component_contract", { input });
}

export function readPageComponentRegistry(): Promise<PageComponentRegistrySnapshot> {
  return invoke<PageComponentRegistrySnapshot>("read_page_component_registry");
}

export function planPageAssetContract(
  input: PageAssetContractInput,
): Promise<PageAssetContractPlan> {
  return invoke<PageAssetContractPlan>("plan_page_asset_contract", { input });
}

export function applyPageAssetContract(
  input: PageAssetContractApplyInput,
): Promise<PageAssetContractApplyReceipt> {
  return invoke<PageAssetContractApplyReceipt>("apply_page_asset_contract", { input });
}

export function readFileBufferStore(): Promise<FileBufferStoreSnapshot | null> {
  return invoke<FileBufferStoreSnapshot | null>("read_file_buffer_store");
}

export function readRecoveryCoordinator(): Promise<RecoveryCoordinatorScan | null> {
  return invoke<RecoveryCoordinatorScan | null>("read_recovery_coordinator_scan");
}

export function readKernelDiskConflicts(): Promise<KernelDiskConflictSnapshot | null> {
  return invoke<KernelDiskConflictSnapshot | null>("read_kernel_disk_conflicts");
}

export function readKernelObservabilityLog(
  limit = 80,
  recoveryOnly = true,
  includeArchives = false,
  levels: KernelLogLevel[] = ["info", "warn", "error"],
  sourceFilter: KernelObservabilityLogSourceFilter = "all",
): Promise<KernelObservabilityLogSnapshot> {
  return invoke<KernelObservabilityLogSnapshot>("read_kernel_observability_log", {
    limit,
    recoveryOnly,
    includeArchives,
    levels,
    sourceFilter,
  });
}

export function readWriteAuthorityRecoveryScan(): Promise<WriteAuthorityRecoveryScan> {
  return invoke<WriteAuthorityRecoveryScan>("read_write_authority_recovery_scan");
}

export function resolveWriteAuthorityRecovery(
  input: WriteAuthorityRecoveryResolutionInput,
): Promise<WriteAuthorityRecoveryResolutionReceipt> {
  return invoke<WriteAuthorityRecoveryResolutionReceipt>("resolve_write_authority_recovery", {
    input,
  });
}

export function readKernelProjectTransitionPolicy(
  action: KernelProjectTransitionAction,
): Promise<KernelProjectTransitionPolicy> {
  return invoke<KernelProjectTransitionPolicy>("read_kernel_project_transition_policy", { action });
}

export function readKernelProjectTransitionPolicyMatrix(): Promise<KernelProjectTransitionPolicyMatrixSnapshot> {
  return invoke<KernelProjectTransitionPolicyMatrixSnapshot>("read_kernel_project_transition_policy_matrix");
}

export function readKernelProjectTransitionBlockedAudit(
  limit = 40,
  includeArchives = false,
): Promise<KernelProjectTransitionBlockedAuditSnapshot> {
  return invoke<KernelProjectTransitionBlockedAuditSnapshot>("read_kernel_project_transition_blocked_audit", {
    limit,
    includeArchives,
  });
}

export function readKernelProjectTransitionDecisionJournal(
  limit = 80,
): Promise<KernelProjectTransitionDecisionJournalSnapshot | null> {
  return invoke<KernelProjectTransitionDecisionJournalSnapshot | null>(
    "read_kernel_project_transition_decision_journal",
    { limit },
  );
}

export function readKernelProjectTransitionDecisionRecoveryAckJournal(
  limit = 40,
): Promise<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot | null> {
  return invoke<KernelProjectTransitionDecisionRecoveryAckJournalSnapshot | null>(
    "read_kernel_project_transition_decision_recovery_ack_journal",
    { limit },
  );
}

export function readKernelProjectTransitionDecisionRetentionHotJournals(): Promise<
  KernelProjectTransitionDecisionRetentionHotJournal[] | null
> {
  return invoke<KernelProjectTransitionDecisionRetentionHotJournal[] | null>(
    "read_kernel_project_transition_decision_retention_hot_journals",
  );
}

export function recordProjectTransitionOperatorDecision(
  targetRoot: string,
  diagnostic: string,
  action?: KernelProjectTransitionAction,
): Promise<KernelProjectTransitionDecisionReceipt> {
  return invoke<KernelProjectTransitionDecisionReceipt>("record_project_transition_operator_decision", {
    targetRoot,
    diagnostic,
    action,
  });
}

export function acknowledgeProjectTransitionDecisionRecoveryPlan(
  recoveryPlanEvidenceHash: string,
  diagnostic: string,
): Promise<KernelProjectTransitionDecisionRecoveryAckReceipt> {
  return invoke<KernelProjectTransitionDecisionRecoveryAckReceipt>(
    "acknowledge_project_transition_decision_recovery_plan",
    {
      recoveryPlanEvidenceHash,
      diagnostic,
    },
  );
}

export function executeProjectTransitionDecisionRetention(
  recoveryPlanEvidenceHash: string,
  acknowledgementId: string,
  diagnostic: string,
): Promise<KernelProjectTransitionDecisionRetentionReceipt> {
  return invoke<KernelProjectTransitionDecisionRetentionReceipt>("execute_project_transition_decision_retention", {
    recoveryPlanEvidenceHash,
    acknowledgementId,
    diagnostic,
  });
}

export function recoverProjectTransitionDecisionRetentionHotJournal(
  retentionId: string,
  action: KernelProjectTransitionDecisionRetentionHotJournalRecoveryAction,
  diagnostic: string,
): Promise<KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult> {
  return invoke<KernelProjectTransitionDecisionRetentionHotJournalRecoveryCommandResult>(
    "recover_project_transition_decision_retention_hot_journal",
    {
      retentionId,
      action,
      diagnostic,
    },
  );
}

export function recoverProjectWorkspaceSave(
  transactionId: string,
  action: ProjectWorkspaceSaveRecoveryAction,
  diagnostic: string,
): Promise<ProjectWorkspaceSaveRecoveryCommandResult> {
  return invoke<ProjectWorkspaceSaveRecoveryCommandResult>("recover_project_workspace_save", {
    transactionId,
    action,
    diagnostic,
  });
}

export function readProjectWorkspaceState(): Promise<ProjectWorkspaceSnapshot | null> {
  return invoke<ProjectWorkspaceSnapshot | null>("read_project_workspace_state");
}

export function readProjectWorkspaceHistory(): Promise<WorkspaceHistorySnapshot | null> {
  return invoke<WorkspaceHistorySnapshot | null>("read_project_workspace_history");
}

export function saveProjectWorkspace(
  identity: ProjectWorkspaceIdentity,
): Promise<ProjectWorkspaceSaveReceipt> {
  return invoke<ProjectWorkspaceSaveReceipt>("save_project_workspace", { identity });
}

export function undoProjectWorkspace(
  identity: ProjectWorkspaceHistoryIdentity,
): Promise<ProjectWorkspaceUndoRedoCommandReceipt> {
  return invoke<ProjectWorkspaceUndoRedoCommandReceipt>("undo_project_workspace", { identity });
}

export function redoProjectWorkspace(
  identity: ProjectWorkspaceHistoryIdentity,
): Promise<ProjectWorkspaceUndoRedoCommandReceipt> {
  return invoke<ProjectWorkspaceUndoRedoCommandReceipt>("redo_project_workspace", { identity });
}

export function readFileBufferText(
  relativePath: string,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferTextSnapshot> {
  return invokeBoundFileBuffer<FileBufferTextSnapshot>(
    "read_file_buffer_text",
    { relativePath, identity },
    identity,
  );
}

export function setFileBufferDraft(
  relativePath: string,
  contents: string,
  expectation: FileBufferMutationExpectation,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferFileSnapshot> {
  return invokeBoundFileBuffer<FileBufferFileSnapshot>(
    "set_file_buffer_draft",
    { relativePath, contents, expectation, identity },
    identity,
  );
}

export function applyFileBufferChangeSet(
  input: FileBufferChangeSetInput,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferChangeSetResult> {
  return invokeBoundFileBuffer<FileBufferChangeSetResult>(
    "apply_file_buffer_changeset",
    { input, identity },
    identity,
  );
}

export function clearFileBufferDraft(
  relativePath: string,
  expectation: FileBufferMutationExpectation,
  identity: FileBufferRequestIdentity,
): Promise<FileBufferFileSnapshot> {
  return invokeBoundFileBuffer<FileBufferFileSnapshot>(
    "clear_file_buffer_draft",
    { relativePath, expectation, identity },
    identity,
  );
}

async function invokeBoundFileBuffer<T>(
  command: string,
  args: Record<string, unknown>,
  identity: FileBufferRequestIdentity,
): Promise<T> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(
      "[file_buffer_identity_invalid] FileBufferStore cere root-ul și runtime session ID.",
    );
  }
  const receipt = await invoke<FileBufferCommandReceipt<T>>(command, args);
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      `[file_buffer_stale_receipt] FileBufferStore a returnat receipt stale pentru ${command}: `
        + `așteptat ${identity.expectedProjectRoot}/${identity.expectedSessionId}, `
        + `primit ${receipt.projectRoot}/${receipt.runtimeSessionId}.`,
    );
  }
  return receipt.payload;
}

export async function chooseProjectFolder(): Promise<string | null> {
  const defaultPath = await homeDir().catch(() => undefined);
  const selected = await openDialog({
    directory: true,
    defaultPath,
    multiple: false,
    title: "Deschide dosar proiect",
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export function getZolaBinaryPath(): Promise<string> {
  return invoke<string>("get_zola_binary_path");
}

export function scanProject(path: string): Promise<ProjectScan> {
  return invoke<ProjectScan>("scan_project", { path });
}

export function readSourceGraph(
  identity: PreviewStructuralCommandIdentity,
): Promise<SourceGraph> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    return Promise.reject(new Error(
      "[source_graph_identity_invalid] Source Graph cere ProjectRoot și runtime session ID.",
    ));
  }
  return invoke<SourceGraph>("read_source_graph", { identity });
}

export function createSitePageStructure(input: {
  title: string;
  slug: string;
  pageTemplateName: string;
  draft: boolean;
  targetOrigin?: "local" | "theme";
  targetThemeName?: string | null;
}, identity: PreviewStructuralCommandIdentity): Promise<SitePageStructureReceipt> {
  return invokeBoundSiteStructure("create_site_page_structure", input, identity);
}

export function createSiteArchiveStructure(input: {
  title: string;
  slug: string;
  archiveTemplateName: string;
  targetOrigin?: "local" | "theme";
  targetThemeName?: string | null;
}, identity: PreviewStructuralCommandIdentity): Promise<SiteArchiveStructureReceipt> {
  return invokeBoundSiteStructure("create_site_archive_structure", input, identity);
}

export function createSiteSingleStructure(input: {
  sectionSlug: string;
  title: string;
  slug: string;
  singleTemplateName: string;
  targetOrigin?: "local" | "theme";
  targetThemeName?: string | null;
}, identity: PreviewStructuralCommandIdentity): Promise<SiteSingleStructureReceipt> {
  return invokeBoundSiteStructure("create_site_single_structure", input, identity);
}

export function createSitePartialStructure(input: {
  name: string;
  preset?: "cta" | "header" | "footer" | "generic";
  targetOrigin?: "local" | "theme";
  targetThemeName?: string | null;
}, identity: PreviewStructuralCommandIdentity): Promise<SitePartialStructureReceipt> {
  return invokeBoundSiteStructure("create_site_partial_structure", input, identity);
}

export function includeSitePartial(input: {
  targetFile: string;
  partialTemplateName: string;
  ensurePartial?: {
    name: string;
    preset?: "cta" | "header" | "footer" | "generic";
    targetOrigin?: "local" | "theme";
    targetThemeName?: string | null;
  } | null;
}, identity: PreviewStructuralCommandIdentity): Promise<SitePartialIncludeReceipt> {
  return invokeBoundSiteStructure("include_site_partial", input, identity);
}

type SiteStructureCommandReceipt = {
  workspaceMutation: SitePageStructureReceipt["workspaceMutation"];
  authority: SiteStructureAuthorityReceipt;
};

async function invokeBoundSiteStructure<T extends SiteStructureCommandReceipt>(
  command: string,
  input: Record<string, unknown>,
  identity: PreviewStructuralCommandIdentity,
): Promise<T> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(
      "[site_structure_identity_invalid] Site Structure cere ProjectRoot și runtime session ID.",
    );
  }
  const receipt = await invoke<T>(command, { input, identity });
  validateSiteStructureAuthorityReceipt(command, receipt, identity);
  return receipt;
}

function validateSiteStructureAuthorityReceipt(
  command: string,
  receipt: SiteStructureCommandReceipt,
  identity: PreviewStructuralCommandIdentity,
) {
  const authority = receipt.authority;
  const mutation = receipt.workspaceMutation;
  if (
    authority.schemaVersion !== 1
    || !authority.operationId.trim()
    || authority.projectRoot !== identity.expectedProjectRoot
    || authority.sessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(authority.revisionBefore)
    || !Number.isSafeInteger(authority.revisionAfter)
    || authority.revisionBefore < 0
    || authority.revisionAfter < authority.revisionBefore
    || !canonicalProjectPathsMatch(authority.touchedFiles)
  ) {
    throw new Error("[site_structure_invalid_authority_receipt] " + command + " a returnat autoritate ProjectWorkspace invalidă.");
  }
  if (authority.status === "noop") {
    if (
      mutation !== null
      || authority.touchedFiles.length !== 0
      || authority.revisionAfter !== authority.revisionBefore
      || authority.dirty
    ) {
      throw new Error("[site_structure_invalid_authority_receipt] " + command + " noop a declarat o mutație.");
    }
    return;
  }
  if (
    authority.status !== "staged"
    || !mutation
    || !mutation.changed
    || mutation.revisionBefore !== authority.revisionBefore
    || mutation.revisionAfter !== authority.revisionAfter
    || mutation.dirty !== authority.dirty
    || JSON.stringify(mutation.touchedFiles) !== JSON.stringify(authority.touchedFiles)
  ) {
    throw new Error("[site_structure_invalid_authority_receipt] " + command + " nu confirmă exact mutația ProjectWorkspace.");
  }
}

function canonicalProjectPathsMatch(paths: string[]) {
  const canonical = [...new Set(paths.map((path) => path.trim().replace(/^\/+/, "")))].sort();
  if (JSON.stringify(paths) !== JSON.stringify(canonical)) return false;
  return paths.every((path) =>
    Boolean(path)
    && !path.includes("\\")
    && path.split("/").every((segment) =>
      Boolean(segment) && segment !== "." && segment !== ".."));
}

export function readProjectModel(draftSources?: Record<string, string>): Promise<ProjectModelSnapshot> {
  if (draftSources && Object.keys(draftSources).length > 0) {
    return invoke<ProjectModelSnapshot>("read_project_model_with_drafts", { draftSources });
  }
  return invoke<ProjectModelSnapshot>("read_project_model");
}

export function resolveTemplateWorkbenchPlan(
  input: { templatePath: string; preferredPagePath?: string | null },
  identity: PreviewStructuralCommandIdentity,
): Promise<TemplateWorkbenchPlan> {
  return invoke<TemplateWorkbenchPlan>("resolve_template_workbench_plan", { input, identity });
}

export function planProjectHtmlMove(
  intent: ProjectHtmlMoveIntent,
  draftSources: Record<string, string> = {},
): Promise<ProjectHtmlMovePlan> {
  return invoke<ProjectHtmlMovePlan>("plan_project_html_move", { intent, draftSources });
}

export function readCurrentProjectDiskManifest(): Promise<ProjectDiskManifest> {
  return invoke<ProjectDiskManifest>("read_current_project_disk_manifest");
}

export function reconcileCleanExternalProjectFiles(
  input: KernelExternalDiskReconcileInput,
): Promise<KernelExternalDiskReconcileReceipt> {
  return invoke<KernelExternalDiskReconcileReceipt>("reconcile_clean_external_project_files", { input });
}

export function createProjectContentPage(options: {
  section: string;
  slug: string;
  title: string;
}, identity: FileBufferRequestIdentity): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_create_content_page", { ...options, identity }, identity);
}

export function createProjectTextFile(
  relativePath: string,
  contents: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_create_project_text_file",
    { relativePath, contents, identity },
    identity,
  );
}

export function readProjectFile(relativePath: string): Promise<string> {
  return invoke<string>("read_project_file", { relativePath });
}

export function exportProjectAssetDataUrl(
  identity: MoodBoardRequestIdentity,
  relativePath: string,
  dataUrl: string,
): Promise<MoodBoardAssetReceipt> {
  return invoke<MoodBoardAssetReceipt>("export_project_asset_data_url", {
    input: { ...identity, relativePath, dataUrl },
  });
}

export function exportProjectAssetWebpFromDataUrl(
  identity: MoodBoardRequestIdentity,
  relativePath: string,
  dataUrl: string,
): Promise<MoodBoardAssetReceipt> {
  return invoke<MoodBoardAssetReceipt>("export_project_asset_webp_from_data_url", {
    input: { ...identity, relativePath, dataUrl },
  });
}

export function semanticMoveProjectEntry(
  sourceRelativePath: string,
  targetDirectory: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_move_project_entry",
    { sourceRelativePath, targetDirectory, identity },
    identity,
  );
}

export function semanticRenameProjectEntry(
  sourceRelativePath: string,
  newName: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_rename_project_entry",
    { sourceRelativePath, newName, identity },
    identity,
  );
}

export function trashProjectEntry(
  relativePath: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_delete_project_entry",
    { relativePath, identity },
    identity,
  );
}

function requireProjectFileRequestIdentity(identity: FileBufferRequestIdentity) {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(
      "[project_file_identity_invalid] Operația de fișier cere root-ul și runtime session ID.",
    );
  }
}

export function requireProjectFileReceiptIdentity(
  receipt: { projectRoot: string; runtimeSessionId: string },
  identity: FileBufferRequestIdentity,
  operation: string,
) {
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      `[project_file_stale_receipt] Receipt stale pentru ${operation}: `
        + `așteptat ${identity.expectedProjectRoot}/${identity.expectedSessionId}, `
        + `primit ${receipt.projectRoot}/${receipt.runtimeSessionId}.`,
    );
  }
}

async function invokeWorkspaceEntryMutation(
  command: string,
  args: Record<string, unknown>,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  requireProjectFileRequestIdentity(identity);
  const receipt = await invoke<WorkspaceEntryMutationReceipt>(command, args);
  requireProjectFileReceiptIdentity(receipt, identity, command);
  return receipt;
}

export function readPreviewDocument(url: string): Promise<string> {
  return invoke<string>("read_preview_document", { url });
}

export type ProjectPreviewRequestIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type BrowserPreviewRequestIdentity = ProjectPreviewRequestIdentity & {
  expectedDiskGeneration: number;
};

export type BrowserPreviewStartReceipt = {
  url: string;
  projectRoot: string;
  runtimeSessionId: string;
  acceptedDiskGeneration: number;
};

export type CanvasProjectionPhase =
  | "prepared"
  | "resourcesReady"
  | "committed"
  | "styledReady"
  | "canonicalVerified"
  | "failed";

export type CanvasProjectionIdentity = {
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  transactionId: string;
  previewRevision: string;
};

export type CanvasResourceEntry = {
  url: string;
  contentHash: string;
  sizeBytes: number;
  contentType: string;
  kind: "stylesheet" | "script" | "font" | "image" | "media" | "other";
};

export type CanvasProjectionPlan = {
  schemaVersion: number;
  identity: CanvasProjectionIdentity;
  workspaceTransactionId: string | null;
  phase: CanvasProjectionPhase;
  impact: {
    kinds: string[];
    paths: string[];
    requiresFullDocument: boolean;
  };
  resources: {
    schemaVersion: number;
    previewRevision: string;
    totalBytes: number;
    entries: CanvasResourceEntry[];
  };
};

export type PreviewPhaseReceipt = {
  schemaVersion: number;
  identity: CanvasProjectionIdentity;
  phase: "resourcesReady" | "committed" | "styledReady" | "failed";
  phaseTimingsMs: Record<string, number>;
  diagnostic: string | null;
};

export type PreviewRuntimeEventKind =
  | "interactive_js_restarted"
  | "interactive_js_failed"
  | "canvas_patch_rolled_back"
  | "canvas_fallback";

export type PreviewRuntimeEventInput = {
  schemaVersion: 1;
  identity: CanvasProjectionIdentity;
  kind: PreviewRuntimeEventKind;
  durationMs: number;
  diagnostic: string | null;
};

export type PreviewRuntimeEventReceipt = {
  schemaVersion: 1;
  identity: CanvasProjectionIdentity;
  kind: PreviewRuntimeEventKind;
  accepted: boolean;
};

export type ProjectPreviewStartReceipt = {
  url: string;
  projectRoot: string;
  runtimeSessionId: string;
  workspaceRevision: number;
  previewRevision: string;
  canvasProjection: CanvasProjectionPlan;
};
export type ProjectWorkspacePreviewRequest = ProjectPreviewRequestIdentity & {
  expectedWorkspaceRevision: number;
  requestedPaths: string[];
};
export type TemplateWorkbenchPreviewRequest = ProjectPreviewRequestIdentity & {
  expectedWorkspaceRevision: number;
  templatePath: string;
  preferredPagePath: string | null;
};
export type TemplateWorkbenchPreviewReceipt = {
  plan: TemplateWorkbenchPlan;
  route: string;
  previewUrl: string;
  workspaceRevision: number;
  previewRevision: string;
  canvasProjection: CanvasProjectionPlan;
};
export type ProjectPreviewMutationReceipt = {
  operation: "workspace_projection";
  projectRoot: string;
  runtimeSessionId: string;
  requestedPaths: string[];
  previewRevision: string | null;
  canvasProjection: CanvasProjectionPlan | null;
  workspaceRevision: number;
};

export function createProjectPreviewRequestIdentity(
  projectRoot: string,
  runtimeSessionId: string,
): ProjectPreviewRequestIdentity {
  const expectedProjectRoot = projectRoot.trim();
  const expectedSessionId = runtimeSessionId.trim();
  if (!expectedProjectRoot || !expectedSessionId) {
    throw new Error("Preview workspace cere root-ul și identitatea runtime a ProjectSession.");
  }
  return { expectedProjectRoot, expectedSessionId };
}

export function projectPreviewRequestIdentityMatches(
  identity: ProjectPreviewRequestIdentity,
  projectRoot: string,
  runtimeSessionId: string,
) {
  return identity.expectedProjectRoot === projectRoot
    && identity.expectedSessionId === runtimeSessionId;
}

export function requireProjectPreviewStartReceipt(
  identity: ProjectPreviewRequestIdentity,
  receipt: ProjectPreviewStartReceipt,
) {
  const plan = receipt.canvasProjection;
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || receipt.workspaceRevision !== plan.identity.workspaceRevision
    || receipt.previewRevision !== plan.identity.previewRevision
    || plan.identity.projectRoot !== identity.expectedProjectRoot
    || plan.identity.runtimeSessionId !== identity.expectedSessionId
    || (plan.workspaceTransactionId !== null && (
      typeof plan.workspaceTransactionId !== "string"
      || !plan.workspaceTransactionId.trim()
    ))
    || (plan.phase !== "prepared" && plan.phase !== "canonicalVerified")
  ) {
    throw new Error(
      "Rust a returnat un start Preview pentru altă revizie Canvas sau ProjectSession.",
    );
  }
  return receipt;
}

export function requireProjectPreviewMutationReceipt(
  identity: ProjectWorkspacePreviewRequest,
  receipt: ProjectPreviewMutationReceipt,
) {
  if (
    receipt.operation !== "workspace_projection"
    || receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || receipt.workspaceRevision !== identity.expectedWorkspaceRevision
    || (receipt.previewRevision === null) !== (receipt.canvasProjection === null)
    || (receipt.canvasProjection !== null && (
      receipt.canvasProjection.identity.projectRoot !== identity.expectedProjectRoot
      || receipt.canvasProjection.identity.runtimeSessionId !== identity.expectedSessionId
      || receipt.canvasProjection.identity.workspaceRevision !== identity.expectedWorkspaceRevision
      || receipt.canvasProjection.identity.previewRevision !== receipt.previewRevision
      || (receipt.canvasProjection.workspaceTransactionId !== null && (
        typeof receipt.canvasProjection.workspaceTransactionId !== "string"
        || !receipt.canvasProjection.workspaceTransactionId.trim()
      ))
      || receipt.canvasProjection.phase !== "prepared"
    ))
  ) {
    throw new Error(
      `Preview workspace a returnat un receipt ${receipt.operation} pentru altă revizie ProjectWorkspace sau ProjectSession.`,
    );
  }
  return receipt;
}

export function startProjectBrowserPreview(
  identity: BrowserPreviewRequestIdentity,
): Promise<BrowserPreviewStartReceipt | null> {
  return invoke<BrowserPreviewStartReceipt | null>("start_project_browser_preview", {
    input: identity,
  });
}

export function startProjectPreview(
  identity: ProjectPreviewRequestIdentity,
): Promise<ProjectPreviewStartReceipt | null> {
  return invoke<ProjectPreviewStartReceipt | null>("start_project_preview", {
    input: identity,
  });
}

export function projectProjectWorkspacePreview(
  input: ProjectWorkspacePreviewRequest,
): Promise<ProjectPreviewMutationReceipt> {
  return invoke<ProjectPreviewMutationReceipt>("project_project_workspace_preview", {
    input,
  });
}

export function projectTemplateWorkbenchPreview(
  input: TemplateWorkbenchPreviewRequest,
): Promise<TemplateWorkbenchPreviewReceipt> {
  return invoke<TemplateWorkbenchPreviewReceipt>("project_template_workbench_preview", {
    input,
  });
}

export function acknowledgeCanvasProjectionPhase(
  input: PreviewPhaseReceipt,
): Promise<CanvasProjectionPlan> {
  return invoke<CanvasProjectionPlan>("acknowledge_canvas_projection_phase", { input });
}

export function recordPreviewRuntimeEvent(
  input: PreviewRuntimeEventInput,
): Promise<PreviewRuntimeEventReceipt> {
  return invoke<PreviewRuntimeEventReceipt>("record_preview_runtime_event", { input });
}

export type CssRequestIdentity = FileBufferRequestIdentity;

export function createCssRequestIdentity(
  projectRoot: string,
  runtimeSessionId: string,
): CssRequestIdentity {
  const expectedProjectRoot = projectRoot.trim();
  const expectedSessionId = runtimeSessionId.trim();
  if (!expectedProjectRoot || !expectedSessionId) {
    throw new Error("CSS/SCSS cere ProjectRoot și runtimeSessionId active.");
  }
  return { expectedProjectRoot, expectedSessionId };
}

export function cssRequestIdentityMatches(
  identity: CssRequestIdentity,
  projectRoot: string,
  runtimeSessionId: string,
): boolean {
  return identity.expectedProjectRoot === projectRoot
    && identity.expectedSessionId === runtimeSessionId;
}

async function invokeBoundCss<T>(
  command: string,
  args: Record<string, unknown>,
  identity: CssRequestIdentity,
): Promise<T> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error("[css_identity_invalid] CSS/SCSS cere root-ul și runtime session ID.");
  }
  const receipt = await invoke<FileBufferCommandReceipt<T>>(command, { ...args, identity });
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      `[css_stale_receipt] ${command} a returnat receipt pentru altă ProjectSession: `
        + `așteptat ${identity.expectedProjectRoot}/${identity.expectedSessionId}, `
        + `primit ${receipt.projectRoot}/${receipt.runtimeSessionId}.`,
    );
  }
  return receipt.payload;
}

async function invokeBoundCssMutation<T>(
  command: string,
  args: Record<string, unknown>,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<T>> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error("[css_identity_invalid] CSS/SCSS cere root-ul și runtime session ID.");
  }
  const receipt = await invoke<CssMutationCommandReceipt<T>>(command, { ...args, identity });
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || receipt.authority.projectRoot !== identity.expectedProjectRoot
    || receipt.authority.sessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      `[css_stale_receipt] ${command} a returnat receipt pentru altă ProjectSession.`,
    );
  }
  const authority = receipt.authority;
  if (
    !Array.isArray(authority.touchedFiles)
    || !Array.isArray(authority.writtenFiles)
    || !Array.isArray(authority.removedFiles)
    || !Array.isArray(authority.documents)
  ) {
    throw new Error(`[css_invalid_authority_receipt] ${command} nu conține manifestele CSS schema 2.`);
  }
  const sortedTouched = [...new Set(authority.touchedFiles)].sort();
  const projectedPaths = [
    ...authority.writtenFiles.map((file) => file.relativePath),
    ...authority.removedFiles,
  ].sort();
  const documentPaths = authority.documents.map((projection) => projection.relativePath);
  if (
    authority.schemaVersion !== 2
    || !authority.operationId.trim()
    || !Number.isSafeInteger(authority.revisionBefore)
    || !Number.isSafeInteger(authority.revisionAfter)
    || authority.revisionBefore < 0
    || authority.revisionAfter < 0
    || JSON.stringify(sortedTouched) !== JSON.stringify(authority.touchedFiles)
    || JSON.stringify(projectedPaths) !== JSON.stringify(authority.touchedFiles)
    || JSON.stringify(documentPaths) !== JSON.stringify(authority.touchedFiles)
  ) {
    throw new Error(`[css_invalid_authority_receipt] ${command} a returnat un receipt de sesiune invalid.`);
  }
  if (
    authority.status === "noop"
    && (
      authority.revisionAfter !== authority.revisionBefore
      || authority.touchedFiles.length !== 0
      || authority.writtenFiles.length !== 0
      || authority.removedFiles.length !== 0
      || authority.documents.length !== 0
      || authority.workspaceMutation !== null
    )
  ) {
    throw new Error(`[css_invalid_authority_receipt] ${command} noop a declarat efecte.`);
  }
  if (
    authority.status === "staged"
    && (
      authority.revisionAfter !== authority.revisionBefore + 1
      || authority.touchedFiles.length === 0
      || authority.workspaceMutation?.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION
      || !authority.workspaceMutation.changed
      || authority.workspaceMutation.revisionBefore !== authority.revisionBefore
      || authority.workspaceMutation.revisionAfter !== authority.revisionAfter
      || authority.workspaceMutation.dirty !== authority.dirty
      || JSON.stringify(authority.workspaceMutation.touchedFiles) !== JSON.stringify(authority.touchedFiles)
    )
  ) {
    throw new Error(`[css_invalid_authority_receipt] ${command} staged nu are confirmarea exactă ProjectWorkspace.`);
  }
  if (authority.status !== "noop" && authority.status !== "staged") {
    throw new Error(`[css_invalid_authority_receipt] ${command} are status necunoscut.`);
  }
  for (const projection of authority.documents) {
    const written = authority.writtenFiles.find((file) => file.relativePath === projection.relativePath);
    const removed = authority.removedFiles.includes(projection.relativePath);
    if (projection.snapshot === null) {
      if (!removed || written) {
        throw new Error(`[css_invalid_authority_receipt] ${command} are o proiecție de ștergere inconsistentă.`);
      }
      continue;
    }
    const snapshot = projection.snapshot;
    const file = authority.workspaceMutation?.files.find(
      (candidate) => candidate.relativePath === projection.relativePath,
    );
    if (
      removed
      || !written
      || written.contents !== snapshot.text
      || snapshot.relativePath !== projection.relativePath
      || !file
      || file.currentHash !== snapshot.hash
      || file.currentBytes !== snapshot.bytes
      || file.revision !== snapshot.revision
      || file.dirty !== snapshot.dirty
    ) {
      throw new Error(`[css_invalid_authority_receipt] ${command} nu leagă textul de snapshotul FileBuffer exact.`);
    }
  }
  return receipt;
}

export function getScssVariables(identity: CssRequestIdentity): Promise<ScssVariable[]> {
  return invokeBoundCss<ScssVariable[]>("get_scss_variables", {}, identity);
}

export function getFontInventory(): Promise<FontInventory> {
  return invoke<FontInventory>("get_font_inventory");
}

export function downloadGoogleFontFamily(
  family: string,
  weights: number[],
  variable: boolean,
): Promise<GoogleFontDownloadResult> {
  return invoke<GoogleFontDownloadResult>("download_google_font_family", { family, weights, variable });
}

export function searchGoogleFonts(query: string, limit = 40, offset = 0): Promise<GoogleFontCatalogFamily[]> {
  return invoke<GoogleFontCatalogFamily[]>("search_google_fonts", { query, limit, offset });
}

export function setScssVariable(
  relativePath: string,
  name: string,
  value: string,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>("set_scss_variable", { relativePath, name, value }, identity);
}

export function getClassRules(
  relativePath: string,
  selector: string,
  identity: CssRequestIdentity,
): Promise<CssProperty[]> {
  return invokeBoundCss<CssProperty[]>("get_class_rules", { relativePath, selector }, identity);
}

export type CssViewport = "desktop" | "tablet" | "mobile";

export function getClassRulesAtViewport(
  relativePath: string,
  selector: string,
  viewport: CssViewport,
  identity: CssRequestIdentity,
): Promise<CssProperty[]> {
  return invokeBoundCss<CssProperty[]>(
    "get_class_rules_at_viewport",
    { relativePath, selector, viewport },
    identity,
  );
}

export function getCssRuleContext(
  relativePath: string,
  selector: string,
  viewport: CssViewport,
  identity: CssRequestIdentity,
): Promise<CssRuleContext> {
  return invokeBoundCss<CssRuleContext>(
    "get_css_rule_context",
    { relativePath, selector, viewport },
    identity,
  );
}

export function findClassInScss(
  selector: string,
  scssFiles: string[],
  identity: CssRequestIdentity,
): Promise<{ file: string; rules: CssProperty[] } | null> {
  return invokeBoundCss("find_class_in_scss", { selector, scssFiles }, identity);
}

export function resolvePageCssTarget(options: {
  templatePath: string | null;
  selector: string;
  scssFiles: string[];
  fallbackFile: string | null;
}, identity: CssRequestIdentity): Promise<PageCssTarget> {
  return invokeBoundCss<PageCssTarget>("resolve_page_css_target", options, identity);
}

export function setCssRule(options: {
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>("set_css_rule", options, identity);
}

export function setCssRuleAtViewport(options: {
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  viewport: CssViewport;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>("set_css_rule_at_viewport", options, identity);
}

export function setPageCssRuleAtViewport(options: {
  templatePath: string;
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  viewport: CssViewport;
  cachebustAssets: boolean;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<PageCssWriteResult>> {
  return invokeBoundCssMutation<PageCssWriteResult>("set_page_css_rule_at_viewport", options, identity);
}

export function cleanupPageCssContract(
  templatePath: string,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<PageCssCleanupResult>> {
  return invokeBoundCssMutation<PageCssCleanupResult>(
    "cleanup_page_css_contract",
    { templatePath },
    identity,
  );
}

export function readProjectAppConfig(): Promise<ProjectAppConfig> {
  return invoke<ProjectAppConfig>("read_project_app_config");
}

export function saveProjectAppConfig(config: {
  cachebustAssets: boolean;
  optimizeImagesOnBuild: boolean;
  imageMaxDimension: number;
  imageExcludeSuffix: string;
  imageReplaceOnlyIfSmaller: boolean;
}): Promise<ProjectAppConfig> {
  return invoke<ProjectAppConfig>("save_project_app_config", { config });
}

export function readZolaProjectSettings(): Promise<ZolaProjectSettings> {
  return invoke<ZolaProjectSettings>("read_zola_project_settings");
}

export function saveZolaProjectSettings(settings: ZolaProjectSettings): Promise<ZolaProjectSettings> {
  return invoke<ZolaProjectSettings>("save_zola_project_settings", { settings });
}

export function readProjectEnv(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>("read_project_env");
}

export function saveProjectEnv(vars: Record<string, string>): Promise<void> {
  return invoke("save_project_env", { vars });
}

export function readZolaBaseUrl(): Promise<string> {
  return invoke<string>("read_zola_base_url");
}

export function getPageDataAnims(
  templatePath: string,
  identity: PageJsRequestIdentity,
): Promise<PageJsCommandReceipt<string[]>> {
  return invoke<PageJsCommandReceipt<string[]>>("get_page_data_anims", { templatePath, identity });
}

export function getPageJs(
  templatePath: string,
  identity: PageJsRequestIdentity,
): Promise<PageJsCommandReceipt<PageJsConfig>> {
  return invoke<PageJsCommandReceipt<PageJsConfig>>("get_page_js", { templatePath, identity });
}

export function getPageJsWorkspaceState(
  templatePath: string,
  identity: PageJsRequestIdentity,
): Promise<PageJsCommandReceipt<PageJsWorkspaceState>> {
  return invoke<PageJsCommandReceipt<PageJsWorkspaceState>>(
    "get_page_js_workspace_state",
    { templatePath, identity },
  );
}


export function stagePageJsDraft(
  input: PageJsDraftStageInput,
  identity: PageJsDraftSessionIdentity,
): Promise<PageJsDraftStageReceipt> {
  return invoke<PageJsDraftStageReceipt>("stage_page_js_draft", {
    input: { ...input, ...identity },
  });
}

export function readPageJsDrafts(
  identity: PageJsDraftSessionIdentity,
): Promise<PageJsDraftStoreSnapshot> {
  return invoke<PageJsDraftStoreSnapshot>("read_page_js_drafts", identity);
}

export function clearPageJsDraft(
  templatePath: string,
  expectedRevision: number | null,
  identity: PageJsDraftSessionIdentity,
): Promise<PageJsDraftStageReceipt> {
  return invoke<PageJsDraftStageReceipt>("clear_page_js_draft", {
    templatePath,
    expectedRevision,
    ...identity,
  });
}

export function applyMotionTimelineStepTiming(
  input: MotionTimelineStepTimingInput,
): Promise<MotionTimelineStepTimingReceipt> {
  return invoke("apply_motion_timeline_step_timing", { input });
}

export function saveZolaBaseUrl(url: string): Promise<void> {
  return invoke("save_zola_base_url", { url });
}

export function zolaInit(path: string): Promise<string> {
  return invoke<string>("zola_init", { path });
}

export function zolaBuild(): Promise<string> {
  return invoke<string>("zola_build");
}

export function zolaCheck(): Promise<string> {
  return invoke<string>("zola_check");
}

export function deployToBunny(): Promise<string> {
  return invoke<string>("deploy_to_bunny");
}

export function readAiContextStatus(): Promise<AiContextStatus> {
  return invoke<AiContextStatus>("read_ai_context_status");
}

export function readAiCoordinationState(): Promise<AiCoordinationSnapshot> {
  return invoke<AiCoordinationSnapshot>("read_ai_coordination_state");
}

export function acknowledgeAiEditQuiescence(
  clientSessionId: string,
  acknowledgement: UiQuiescenceAcknowledgement,
): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("acknowledge_ai_edit_quiescence", {
    clientSessionId,
    acknowledgement,
  });
}

export function completeAiEditReconciliation(
  leaseId: string,
  expectedProjectSessionId: string,
  expectedProjectRevision: number,
  observedChangedFiles: string[],
): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("complete_ai_edit_reconciliation", {
    leaseId,
    expectedProjectSessionId,
    expectedProjectRevision,
    observedChangedFiles,
  });
}

export function acceptAiEditConflictForReconciliation(): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("accept_ai_edit_conflict_for_reconciliation");
}

export function authorizeAiReconciliationRecoveryReload(): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("authorize_ai_reconciliation_recovery_reload");
}

export function completeAiReconciliationRecoveryReload(
  leaseId: string,
  expectedReplacementSessionId: string,
): Promise<EditTransitionReceipt> {
  return invoke<EditTransitionReceipt>("complete_ai_reconciliation_recovery_reload", {
    leaseId,
    expectedReplacementSessionId,
  });
}

export function saveAiContextSnapshot(snapshot: UiContextProjection): Promise<AiContextStatus> {
  return invoke<AiContextStatus>("save_ai_context_snapshot", { snapshot });
}

export function readCodexMcpStatus(): Promise<CodexMcpStatus> {
  return invoke<CodexMcpStatus>("read_codex_mcp_status");
}

export function configureCodexMcp(): Promise<CodexMcpStatus> {
  return invoke<CodexMcpStatus>("configure_codex_mcp");
}
