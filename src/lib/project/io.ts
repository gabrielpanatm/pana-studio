import { CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION } from "$lib/types";
import type {
  CssInspectorContextResolution,
  CssMutationCommandReceipt,
  EditableStyles,
  FontInventory,
  FontManagerSnapshot,
  FontDeliveryMutationReceipt,
  FontFamilyRemovalPlan,
  FontFamilyRemovalReceipt,
  FontPreviewAsset,
  FontRoleAssignmentReceipt,
  FontRoleId,
  GoogleFontInstallReceipt,
  LocalFontImportPlan,
  LocalFontImportReceipt,
  FileBufferChangeSetInput,
  FileBufferChangeSetResult,
  FileBufferCommandReceipt,
  FileBufferFileSnapshot,
  FileBufferMutationExpectation,
  FileBufferRequestIdentity,
  FileBufferStoreSnapshot,
  FileBufferTextSnapshot,
  GoogleFontAxis,
  GoogleFontCatalogFamily,
  ThemeStyleCatalogSnapshot,
  ThemeStyleDraftPreview,
  ThemeStylePropertyInput,
  ThemeStyleTargetSnapshot,
  PageCssCleanupResult,
  PageCssWriteResult,
  ReusableCssWriteResult,
  UiContextProjection,
  AiContextStatus,
  AiCoordinationSnapshot,
  CodexMcpStatus,
  ContentModelCatalog,
  ContentModelMutationApplyReceipt,
  ContentModelMutationInput,
  ContentModelMutationPlan,
  ComponentMutationApplyReceipt,
  ComponentMutationInput,
  CanvasInteractionBindingReceipt,
  CanvasDragOverReceipt,
  CanvasDragOverResolveInput,
  CanvasHoverReceipt,
  CanvasInteractionIdentity,
  CanvasInteractionReceipt,
  CanvasInteractionResolveInput,
  SelectionCoordinatorSnapshot,
  SelectionIntent,
  SelectionMutationIdentity,
  SelectionObservationInput,
  SelectionObservationReceipt,
  DataMutationApplyReceipt,
  DataMutationInput,
  DataNodeEditorSnapshot,
  DesignTokenCatalogSnapshot,
  EditorNavigationSnapshot,
  EditScopeGrant,
  HoverSnapshot,
  EditorMoveCommitInput,
  EditorMoveExecutionReceipt,
  EditorMovePlan,
  EditorMovePlanInput,
  BlockRuntimeSnapshot,
  UiBlockGraphSnapshot,
  InsertCatalogContext,
  InsertCatalogSnapshot,
  DynamicWidgetSnapshot,
  DynamicWidgetSnapshotRequest,
  UpdateDynamicWidgetInput,
  DeleteDynamicWidgetInput,
  EditTransitionReceipt,
  PageAssetContractApplyInput,
  PageAssetContractInput,
  PageAssetContractApplyReceipt,
  PageAssetContractPlan,
  NativeBlockContractApplyReceipt,
  NativeBlockContractApplyInput,
  NativeBlockContractInput,
  NativeBlockContractPlan,
  NativeBlockRegistrySnapshot,
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
  MotionPageMutationInput,
  MotionPageMutationReceipt,
  PreviewHtmlInsertDropExecutionInput,
  PreviewHtmlInsertDropExecutionReceipt,
  PageJsDraftStageInput,
  PageJsDraftStageReceipt,
  PageJsDraftSessionIdentity,
  PageJsDraftStoreSnapshot,
  PageJsCommandReceipt,
  PageJsWorkspaceState,
  PageJsRequestIdentity,
  PreviewProjectionIntentInput,
  PreviewProjectionIntentReceipt,
  PreviewTeraDeleteExecutionInput,
  PreviewTeraDeleteExecutionReceipt,
  PreviewTeraInsertDropExecutionInput,
  PreviewTeraInsertDropExecutionReceipt,
  PreviewStructuralCommandIdentity,
  ProjectAppConfig,
  ProjectDiskManifest,
  ProjectAuditSnapshot,
  ProjectModelSnapshot,
  TemplateWorkbenchPlan,
  ProjectOpenRecoveryDecisionInput,
  ProjectLifecycleSnapshot,
  ProjectOpenBootstrapReceipt,
  ProjectOpenInspectionReceipt,
  ProjectScan,
  StartupCreationApplyRequest,
  StartupCreationCatalog,
  StartupCreationPlan,
  StartupCreationPlanRequest,
  StartupCreationReceipt,
  StartupFlowSnapshot,
  ProjectSessionSnapshot,
  ProjectWorkspaceHistoryIdentity,
  ProjectWorkspaceIdentity,
  ProjectWorkspaceSaveReceipt,
  ProjectWorkspaceSaveRecoveryAction,
  ProjectWorkspaceSaveRecoveryCommandResult,
  ProjectWorkspaceSnapshot,
  ThemeApplyReceipt,
  ThemeCatalogSnapshot,
  ThemePlan,
  ThemePlanRequest,
  ProjectWorkspaceUndoRedoCommandReceipt,
  KernelDiskConflictSnapshot,
  KernelExternalDiskReconcileInput,
  KernelExternalDiskReconcileReceipt,
  WorkspaceEntryMutationReceipt,
  DesignClassInventorySnapshot,
  DesignClassRenameReceipt,
  PublishOperationCancelReceipt,
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
  SourceGraphProjectionReceipt,
  TaxonomyCatalogSnapshot,
  TaxonomyMutationApplyReceipt,
  TaxonomyMutationInput,
  TaxonomyMutationPlan,
  TemplateCatalogSnapshot,
  CreateSemanticTemplateInput,
  CreateTemplateInput,
  CreateListingItemInput,
  DeleteListingItemInput,
  CreateTemplateCollectionInput,
  DeleteTemplateInput,
  DuplicateTemplateInput,
  OverrideThemeTemplateInput,
  RenameTemplateInput,
  SetTemplateAssignmentInput,
  SetTemplateParentInput,
  ZolaProjectSettings,
  UiQuiescenceAcknowledgement,
  WorkspaceCatalogProjectionReceipt,
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
import {
  GLOBAL_STATUS_SCHEMA_VERSION,
  type GlobalStatusInput,
  type GlobalStatusSnapshot,
} from "$lib/status/global-status";
import {
  DESIGN_CLASS_INVENTORY_SCHEMA_VERSION,
  DESIGN_CLASS_RENAME_SCHEMA_VERSION,
  CANVAS_INTERACTION_SCHEMA_VERSION,
  SELECTION_COORDINATOR_SCHEMA_VERSION,
  PROJECT_AUDIT_SCHEMA_VERSION,
  PROJECT_WORKSPACE_SCHEMA_VERSION,
  TAXONOMY_CATALOG_SCHEMA_VERSION,
  TAXONOMY_MUTATION_SCHEMA_VERSION,
  CONTENT_MODEL_SCHEMA_VERSION,
  TEMPLATE_CATALOG_SCHEMA_VERSION,
  EDITOR_NAVIGATION_SCHEMA_VERSION,
  EDIT_SCOPE_GRANT_SCHEMA_VERSION,
  EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
  EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION,
  EDITOR_MOVE_PLAN_SCHEMA_VERSION,
} from "$lib/types";
import { invoke } from "@tauri-apps/api/core";
import { homeDir } from "@tauri-apps/api/path";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  PageFrontmatterField,
  PageFrontmatterMutationValue,
} from "$lib/markdown/frontmatter";

function schemaMismatch(resource: string, actual: number, expected: number) {
  return new Error(t("io-schema-mismatch", { resource, actual, expected }));
}

export function openProject(
  path: string,
  operationId: string,
  candidateToken: string,
  operatorDecisionId?: string,
  recoveryDecision?: ProjectOpenRecoveryDecisionInput,
): Promise<ProjectOpenBootstrapReceipt> {
  return invoke<ProjectOpenBootstrapReceipt>("open_project", {
    path,
    operationId,
    candidateToken,
    operatorDecisionId,
    recoveryDecision,
  });
}

export function inspectProjectOpen(
  path: string,
  expectedSnapshotToken: string,
): Promise<ProjectOpenInspectionReceipt> {
  return invoke<ProjectOpenInspectionReceipt>("inspect_project_open", {
    path,
    expectedSnapshotToken,
  });
}

export function readProjectLifecycle(): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("read_project_lifecycle");
}

export function acknowledgeProjectFrontendHydrated(
  projectRoot: string,
  runtimeSessionId: string,
): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("acknowledge_project_frontend_hydrated", {
    projectRoot,
    runtimeSessionId,
  });
}

export function reportProjectCapabilityDegraded(
  projectRoot: string,
  runtimeSessionId: string,
  capability: "frontend" | "preview" | "canvas" | "source_graph",
  diagnostic: string,
): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("report_project_capability_degraded", {
    projectRoot,
    runtimeSessionId,
    capability,
    diagnostic,
  });
}

export function cancelProjectOpen(
  operationId: string,
  diagnostic: string,
): Promise<ProjectLifecycleSnapshot> {
  return invoke<ProjectLifecycleSnapshot>("cancel_project_open", {
    operationId,
    diagnostic,
  });
}

export function closeProject(operatorDecisionId?: string): Promise<void> {
  return invoke<void>("close_project", { operatorDecisionId });
}

export function readProjectSession(): Promise<ProjectSessionSnapshot | null> {
  return invoke<ProjectSessionSnapshot | null>("read_project_session");
}

export function reattachProjectSession(): Promise<ProjectOpenBootstrapReceipt | null> {
  return invoke<ProjectOpenBootstrapReceipt | null>("reattach_project_session");
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

export function publishKernelGlobalStatus(
  input: GlobalStatusInput,
): Promise<GlobalStatusSnapshot> {
  return invoke<GlobalStatusSnapshot>("publish_global_status", {
    input: {
      ...input,
      schemaVersion: GLOBAL_STATUS_SCHEMA_VERSION,
    },
  });
}

export function resolveKernelGlobalStatus(
  key: string,
): Promise<GlobalStatusSnapshot> {
  return invoke<GlobalStatusSnapshot>("resolve_global_status", { key });
}

export function readKernelGlobalStatus(): Promise<GlobalStatusSnapshot> {
  return invoke<GlobalStatusSnapshot>("read_global_status");
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

export function planNativeBlockContract(
  input: NativeBlockContractInput,
): Promise<NativeBlockContractPlan> {
  return invoke<NativeBlockContractPlan>("plan_native_block_contract", { input });
}

export function applyNativeBlockContract(
  input: NativeBlockContractApplyInput,
): Promise<NativeBlockContractApplyReceipt> {
  return invoke<NativeBlockContractApplyReceipt>("apply_native_block_contract", { input });
}

export function readNativeBlockRegistry(): Promise<NativeBlockRegistrySnapshot> {
  return invoke<NativeBlockRegistrySnapshot>("read_native_block_registry");
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

export function importProjectAsset(
  sourcePath: string,
  destinationDirectory: string,
  fileName: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "import_project_asset",
    { sourcePath, destinationDirectory, fileName, identity },
    identity,
  );
}

export async function readFileBufferStore(): Promise<FileBufferStoreSnapshot | null> {
  const snapshot = await invoke<FileBufferStoreSnapshot | null>("read_file_buffer_store");
  if (snapshot && snapshot.schemaVersion !== 4) {
    throw schemaMismatch("FileBufferStore", snapshot.schemaVersion, 4);
  }
  return snapshot;
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
      t("io-file-buffer-identity-invalid"),
    );
  }
  const receipt = await invoke<FileBufferCommandReceipt<T>>(command, args);
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
  ) {
    throw new Error(
      t("io-file-buffer-stale-receipt", {
        command,
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
    );
  }
  return receipt.payload;
}

export async function chooseProjectFolder(): Promise<string | null> {
  // The project chooser deliberately starts from the current account home on
  // every invocation. No previous project path is persisted or reused.
  const defaultPath = await homeDir().catch(() => undefined);
  const selected = await openDialog({
    directory: true,
    defaultPath,
    multiple: false,
    title: t("io-dialog-open-project"),
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export function readStartupFlow(): Promise<StartupFlowSnapshot> {
  return invoke<StartupFlowSnapshot>("read_startup_flow");
}

export function inspectStartupFolder(path: string): Promise<StartupFlowSnapshot> {
  return invoke<StartupFlowSnapshot>("inspect_startup_folder", { path });
}

export function readStartupCreationCatalog(
  expectedSnapshotToken: string,
): Promise<StartupCreationCatalog> {
  return invoke<StartupCreationCatalog>("read_startup_creation_catalog", {
    expectedSnapshotToken,
  });
}

export function planStartupCreation(
  request: StartupCreationPlanRequest,
): Promise<StartupCreationPlan> {
  return invoke<StartupCreationPlan>("plan_startup_creation", { request });
}

export function applyStartupCreation(
  request: StartupCreationApplyRequest,
): Promise<StartupCreationReceipt> {
  return invoke<StartupCreationReceipt>("apply_startup_creation", { request });
}

export async function chooseAssetFile(): Promise<string | null> {
  const selected = await openDialog({
    directory: false,
    multiple: false,
    title: t("io-dialog-import-asset"),
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export async function chooseFontFiles(): Promise<string[]> {
  const selected = await openDialog({
    directory: false,
    multiple: true,
    title: t("io-dialog-import-fonts"),
    filters: [{
      name: t("io-dialog-web-fonts"),
      extensions: ["woff2", "woff", "ttf", "otf"],
    }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export function scanProject(path: string): Promise<ProjectScan> {
  return invoke<ProjectScan>("scan_project", { path });
}

export function readSourceGraph(
  identity: PreviewStructuralCommandIdentity,
): Promise<SourceGraphProjectionReceipt> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    return Promise.reject(new Error(
      t("io-source-graph-identity-invalid"),
    ));
  }
  return invoke<SourceGraphProjectionReceipt>("read_source_graph", { identity });
}

export function readTemplateCatalog(
  identity: PreviewStructuralCommandIdentity,
  expectedWorkspaceRevision?: number,
): Promise<TemplateCatalogSnapshot> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    return Promise.reject(new Error(
      t("io-template-catalog-identity-invalid"),
    ));
  }
  return invoke<WorkspaceCatalogProjectionReceipt<TemplateCatalogSnapshot>>(
    "read_template_catalog",
    { identity },
  ).then((receipt) => {
    requireWorkspaceCatalogProjectionReceipt(
      "templates",
      identity,
      expectedWorkspaceRevision,
      receipt,
    );
    const snapshot = receipt.catalog;
    if (snapshot.schemaVersion !== TEMPLATE_CATALOG_SCHEMA_VERSION) {
      throw schemaMismatch(
        t("io-resource-template-catalog"),
        snapshot.schemaVersion,
        TEMPLATE_CATALOG_SCHEMA_VERSION,
      );
    }
    return snapshot;
  });
}

export function readTaxonomyCatalog(
  identity: PreviewStructuralCommandIdentity,
  expectedWorkspaceRevision?: number,
): Promise<TaxonomyCatalogSnapshot> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    return Promise.reject(new Error(
      t("io-taxonomy-catalog-identity-invalid"),
    ));
  }
  return invoke<WorkspaceCatalogProjectionReceipt<TaxonomyCatalogSnapshot>>(
    "read_taxonomy_catalog",
    { identity },
  ).then((receipt) => {
    requireWorkspaceCatalogProjectionReceipt(
      "taxonomies",
      identity,
      expectedWorkspaceRevision,
      receipt,
    );
    const snapshot = receipt.catalog;
    if (snapshot.schemaVersion !== TAXONOMY_CATALOG_SCHEMA_VERSION) {
      throw schemaMismatch(
        t("io-resource-taxonomy-catalog"),
        snapshot.schemaVersion,
        TAXONOMY_CATALOG_SCHEMA_VERSION,
      );
    }
    return snapshot;
  });
}

function requireWorkspaceCatalogProjectionReceipt<T>(
  kind: "templates" | "taxonomies",
  identity: PreviewStructuralCommandIdentity,
  expectedWorkspaceRevision: number | undefined,
  receipt: WorkspaceCatalogProjectionReceipt<T>,
) {
  const resource = kind === "templates"
    ? t("io-resource-template-catalog")
    : t("io-resource-taxonomy-catalog");
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
  ) {
    throw new Error(t("io-workspace-catalog-identity-invalid", { resource }));
  }
  if (
    expectedWorkspaceRevision !== undefined
    && receipt.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error(
      t("io-workspace-catalog-revision-mismatch", {
        resource,
        actual: receipt.workspaceRevision,
        expected: expectedWorkspaceRevision,
      }),
    );
  }
}

export async function planTaxonomyMutation(
  input: TaxonomyMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<TaxonomyMutationPlan> {
  const plan = await invoke<TaxonomyMutationPlan>("plan_taxonomy_mutation", { input, identity });
  if (plan.schemaVersion !== TAXONOMY_MUTATION_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-taxonomy-plan"),
      plan.schemaVersion,
      TAXONOMY_MUTATION_SCHEMA_VERSION,
    );
  }
  return plan;
}

export async function applyTaxonomyMutation(
  input: TaxonomyMutationInput,
  expectedPlanId: string,
  identity: FileBufferRequestIdentity,
): Promise<TaxonomyMutationApplyReceipt> {
  const receipt = await invoke<TaxonomyMutationApplyReceipt>("apply_taxonomy_mutation", {
    input,
    expectedPlanId,
    identity,
  });
  if (receipt.plan.schemaVersion !== TAXONOMY_MUTATION_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-taxonomy-receipt"),
      receipt.plan.schemaVersion,
      TAXONOMY_MUTATION_SCHEMA_VERSION,
    );
  }
  return receipt;
}

export async function readContentModelCatalog(
  identity: FileBufferRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<ContentModelCatalog> {
  const receipt = await invoke<FileBufferCommandReceipt<ContentModelCatalog>>(
    "read_content_model_catalog",
    { identity },
  );
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("io-workspace-catalog-identity-invalid", {
      resource: t("io-resource-content-model-catalog"),
    }));
  }
  if (
    expectedWorkspaceRevision !== undefined
    && receipt.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error(t("io-workspace-catalog-revision-mismatch", {
      resource: t("io-resource-content-model-catalog"),
      actual: receipt.workspaceRevision,
      expected: expectedWorkspaceRevision,
    }));
  }
  if (receipt.payload.schemaVersion !== CONTENT_MODEL_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-content-model-catalog"),
      receipt.payload.schemaVersion,
      CONTENT_MODEL_SCHEMA_VERSION,
    );
  }
  return receipt.payload;
}

export async function planContentModelMutation(
  input: ContentModelMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<ContentModelMutationPlan> {
  const plan = await invoke<ContentModelMutationPlan>("plan_content_model_mutation", {
    input,
    identity,
  });
  if (plan.schemaVersion !== CONTENT_MODEL_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-content-model-plan"),
      plan.schemaVersion,
      CONTENT_MODEL_SCHEMA_VERSION,
    );
  }
  return plan;
}

export async function applyContentModelMutation(
  input: ContentModelMutationInput,
  expectedPlanId: string,
  identity: FileBufferRequestIdentity,
): Promise<ContentModelMutationApplyReceipt> {
  const receipt = await invoke<ContentModelMutationApplyReceipt>(
    "apply_content_model_mutation",
    { input, expectedPlanId, identity },
  );
  if (receipt.plan.schemaVersion !== CONTENT_MODEL_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-content-model-receipt"),
      receipt.plan.schemaVersion,
      CONTENT_MODEL_SCHEMA_VERSION,
    );
  }
  return receipt;
}

export function readProjectModel(draftSources?: Record<string, string>): Promise<ProjectModelSnapshot> {
  if (draftSources && Object.keys(draftSources).length > 0) {
    return invoke<ProjectModelSnapshot>("read_project_model_with_drafts", { draftSources });
  }
  return invoke<ProjectModelSnapshot>("read_project_model");
}

export async function readEditorNavigationSnapshot(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null = null,
): Promise<EditorNavigationSnapshot> {
  const snapshot = await invoke<EditorNavigationSnapshot>(
    "read_editor_navigation_snapshot",
    {
      input: {
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
      },
    },
  );
  if (snapshot.schemaVersion !== EDITOR_NAVIGATION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "EditorNavigationSnapshot",
      snapshot.schemaVersion,
      EDITOR_NAVIGATION_SCHEMA_VERSION,
    );
  }
  if (
    snapshot.identity.projectRoot !== identity.projectRoot
    || snapshot.identity.runtimeSessionId !== identity.runtimeSessionId
    || snapshot.identity.workspaceRevision !== identity.workspaceRevision
    || snapshot.identity.transactionId !== identity.transactionId
    || snapshot.identity.previewRevision !== identity.previewRevision
  ) {
    throw new Error("EditorNavigationSnapshot a întors altă identitate Canvas.");
  }
  if (
    activeDocumentPath
    && snapshot.focusedView
    && snapshot.focusedView.activeDocumentPath !== activeDocumentPath
  ) {
    throw new Error("EditorNavigationSnapshot a întors alt document activ.");
  }
  return snapshot;
}

export async function bindCanvasInteractionAgent(
  identity: CanvasInteractionIdentity,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null = null,
): Promise<CanvasInteractionBindingReceipt> {
  const receipt = await invoke<CanvasInteractionBindingReceipt>(
    "bind_canvas_interaction_agent",
    {
      input: {
        schemaVersion: CANVAS_INTERACTION_SCHEMA_VERSION,
        identity,
        activeDocumentPath,
        previewContextRenderInstanceId,
      },
    },
  );
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasInteractionBindingReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  if (
    !sameCanvasInteractionIdentity(receipt.identity, identity)
    || !Number.isSafeInteger(receipt.lastAcceptedSequence)
    || receipt.lastAcceptedSequence < 0
    || (
      receipt.activeDocumentPath !== null
      && typeof receipt.activeDocumentPath !== "string"
    )
    || !Array.isArray(receipt.authoringSurfaces)
    || receipt.authoringSurfaces.some((surface) => (
      !surface
      || typeof surface.sourceNodeId !== "string"
      || surface.sourceNodeId.length === 0
      || typeof surface.boundaryInstanceId !== "string"
      || surface.boundaryInstanceId.length === 0
      || (
        surface.renderInstanceId !== null
        && (
          typeof surface.renderInstanceId !== "string"
          || surface.renderInstanceId.length === 0
        )
      )
    ))
  ) {
    throw new Error("CanvasAgent a întors alt binding sau o secvență invalidă.");
  }
  return receipt;
}

export async function resolveCanvasInteractionIntent(
  input: CanvasInteractionResolveInput,
): Promise<CanvasInteractionReceipt> {
  const receipt = await invoke<CanvasInteractionReceipt>(
    "resolve_canvas_interaction_intent",
    { input },
  );
  requireCanvasInteractionReceipt(receipt, input);
  return receipt;
}

export async function resolveCanvasDragOverIntent(
  input: CanvasDragOverResolveInput,
): Promise<CanvasDragOverReceipt> {
  if (input.request.gesture !== "dragOver") {
    throw new Error("Canvas DragOver acceptă numai gesturi dragOver.");
  }
  const receipt = await invoke<CanvasDragOverReceipt>(
    "resolve_canvas_drag_over_intent",
    { input },
  );
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasDragOverReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  requireCanvasInteractionReceipt(receipt.interaction, {
    request: input.request,
    editScopeGrant: input.editScopeGrant,
  });
  const plan = receipt.plan;
  const target = receipt.interaction.target;
  const position = receipt.interaction.dragPosition;
  const accepted = receipt.interaction.status === "resolved" && target && position;
  if (Boolean(plan) !== Boolean(accepted)) {
    throw new Error("CanvasDragOverReceipt are un plan inconsistent cu ținta Rust.");
  }
  if (plan && target && position) {
    if (
      plan.schemaVersion !== EDITOR_MOVE_PLAN_SCHEMA_VERSION
      || plan.sourceNodeId !== input.sourceNodeId
      || plan.targetNodeId !== target.editorNodeId
      || plan.position !== position
      || !sameCanvasProjectionIdentity(plan.identity, input.request.identity.canvas)
      || plan.allowed !== Boolean(plan.token && plan.operation)
    ) {
      throw new Error("CanvasDragOverReceipt a întors alt plan semantic.");
    }
    requireEditorMoveLiveProjection(plan);
  }
  const timings = receipt.timings;
  if (
    !timings
    || !Number.isSafeInteger(timings.emittedAtMs)
    || timings.emittedAtMs < 0
    || !Number.isSafeInteger(timings.rustReceivedAtMs)
    || timings.rustReceivedAtMs < 0
    || !Number.isSafeInteger(timings.rustCompletedAtMs)
    || timings.rustCompletedAtMs < timings.rustReceivedAtMs
    || !Number.isSafeInteger(timings.inputToPlanDurationMs)
    || timings.inputToPlanDurationMs < 0
    || (
      plan?.allowed
        ? !Number.isSafeInteger(timings.inputToFirstAllowedPlanMs)
          || (timings.inputToFirstAllowedPlanMs ?? -1) < 0
          || timings.inputToFirstAllowedPlanMs !== timings.inputToPlanDurationMs
        : timings.inputToFirstAllowedPlanMs !== null
    )
    || !Number.isSafeInteger(timings.rustDurationMs)
    || timings.rustDurationMs < 0
  ) {
    throw new Error("CanvasDragOverReceipt are telemetrie Rust invalidă.");
  }
  return receipt;
}

export async function resolveCanvasHoverIntent(
  input: CanvasInteractionResolveInput,
): Promise<CanvasHoverReceipt> {
  if (input.request.gesture !== "pointerMove") {
    throw new Error("CanvasHover acceptă numai gesturi pointerMove.");
  }
  const receipt = await invoke<CanvasHoverReceipt>(
    "resolve_canvas_hover_intent",
    { input },
  );
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasHoverReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  requireCanvasInteractionReceipt(receipt.interaction, input);
  const accepted = receipt.interaction.status === "resolved"
    || receipt.interaction.status === "noTarget";
  if (accepted !== Boolean(receipt.projection)) {
    throw new Error("CanvasHoverReceipt are o proiecție semantică inconsistentă.");
  }
  if (receipt.projection) {
    if (typeof receipt.projection.changed !== "boolean") {
      throw new Error("CanvasHoverReceipt nu declară starea proiecției.");
    }
    const hover = receipt.projection.hover;
    if (hover) {
      requireHoverSnapshot(hover, input.request.identity.canvas);
    }
    const target = receipt.interaction.target;
    if (
      (target && (
        !hover
        || hover.editorNodeId !== target.editorNodeId
        || hover.documentEpoch !== input.request.identity.documentEpoch
      ))
      || (!target && hover)
    ) {
      throw new Error("CanvasHoverReceipt nu proiectează ținta Rust rezolvată.");
    }
  }
  return receipt;
}

function requireCanvasInteractionReceipt(
  receipt: CanvasInteractionReceipt,
  input: CanvasInteractionResolveInput,
) {
  if (receipt.schemaVersion !== CANVAS_INTERACTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "CanvasInteractionReceipt",
      receipt.schemaVersion,
      CANVAS_INTERACTION_SCHEMA_VERSION,
    );
  }
  if (
    !sameCanvasInteractionIdentity(receipt.identity, input.request.identity)
    || receipt.gestureSequence !== input.request.gestureSequence
    || receipt.gesture !== input.request.gesture
  ) {
    throw new Error("CanvasInteractionReceipt nu aparține gestului solicitat.");
  }
  const expectedDragPosition = receipt.status === "resolved"
    && input.request.gesture === "dragOver"
    ? input.request.drag?.position ?? null
    : null;
  if (receipt.dragPosition !== expectedDragPosition) {
    throw new Error(
      "CanvasInteractionReceipt a întors o proiecție drag incompatibilă cu gestul.",
    );
  }
}

export async function applySelectionIntent(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null,
  intent: SelectionIntent,
): Promise<SelectionCoordinatorSnapshot> {
  const receipt = await invoke<SelectionCoordinatorSnapshot>(
    "apply_selection_intent",
    {
      input: {
        schemaVersion: SELECTION_COORDINATOR_SCHEMA_VERSION,
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
        intent,
      },
    },
  );
  requireSelectionCoordinatorSnapshot(receipt, identity);
  return receipt;
}

export async function readSelectionSnapshot(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string | null,
  previewContextRenderInstanceId: string | null,
): Promise<SelectionCoordinatorSnapshot> {
  const receipt = await invoke<SelectionCoordinatorSnapshot>(
    "read_selection_snapshot",
    {
      input: {
        schemaVersion: SELECTION_COORDINATOR_SCHEMA_VERSION,
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
      },
    },
  );
  requireSelectionCoordinatorSnapshot(receipt, identity);
  return receipt;
}

export async function acceptSelectionObservation(
  input: SelectionObservationInput,
): Promise<SelectionObservationReceipt> {
  const receipt = await invoke<SelectionObservationReceipt>(
    "accept_selection_observation",
    { input },
  );
  if (
    receipt.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || receipt.selectionRevision !== input.selectionRevision
    || receipt.documentEpoch !== input.documentEpoch
    || receipt.renderInstanceId !== input.renderInstanceId
    || !sameCanvasProjectionIdentity(receipt.canvasIdentity, input.canvasIdentity)
  ) {
    throw new Error("SelectionObservation nu aparține selecției solicitate.");
  }
  requireInspectorSelectionSummary(
    receipt.inspectorSummary,
    input.canvasIdentity,
    input.selectionRevision,
  );
  if (
    receipt.inspectorSummary.documentEpoch !== input.documentEpoch
    || receipt.inspectorSummary.renderInstanceId !== input.renderInstanceId
    || receipt.inspectorSummary.state !== "resolved"
  ) {
    throw new Error("InspectorSelectionSummary nu confirmă faptele fizice solicitate.");
  }
  return receipt;
}

function requireSelectionCoordinatorSnapshot(
  receipt: SelectionCoordinatorSnapshot,
  identity: CanvasProjectionIdentity,
) {
  if (
    receipt.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || receipt.selection.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
  ) {
    throw schemaMismatch(
      "SelectionCoordinator",
      receipt.schemaVersion,
      SELECTION_COORDINATOR_SCHEMA_VERSION,
    );
  }
  if (
    receipt.selection.projectRoot !== identity.projectRoot
    || receipt.selection.runtimeSessionId !== identity.runtimeSessionId
    || !sameCanvasProjectionIdentity(receipt.selection.canvasIdentity, identity)
    || !Number.isSafeInteger(receipt.selection.selectionRevision)
    || receipt.selection.selectionRevision <= 0
  ) {
    throw new Error("SelectionCoordinator a întors altă sesiune sau o revizie invalidă.");
  }
  const selection = receipt.selection;
  requireInspectorSelectionSummary(
    receipt.inspectorSummary,
    identity,
    selection.selectionRevision,
  );
  const expectedInspectorStates = {
    cleared: new Set(["empty"]),
    resolved: new Set(["resolving", "resolved", "uninspectable"]),
    notRendered: new Set(["notRendered"]),
    ambiguous: new Set(["ambiguous"]),
  } satisfies Record<
    SelectionCoordinatorSnapshot["selection"]["resolution"],
    Set<string>
  >;
  if (
    (selection.resolution === "resolved" && (!selection.subject || !selection.anchor))
    || (selection.resolution === "cleared" && (selection.subject || selection.anchor))
    || !expectedInspectorStates[selection.resolution].has(receipt.inspectorSummary.state)
    || (
      selection.projections.preview.primaryRenderInstanceId
      && !selection.projections.preview.renderInstanceIds.includes(
        selection.projections.preview.primaryRenderInstanceId,
      )
    )
  ) {
    throw new Error("SelectionCoordinator a întors o proiecție semantică inconsistentă.");
  }
  if (receipt.hover) requireHoverSnapshot(receipt.hover, identity);
}

function requireHoverSnapshot(
  hover: HoverSnapshot,
  identity: CanvasProjectionIdentity,
) {
  if (
    hover.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || !sameCanvasProjectionIdentity(hover.canvasIdentity, identity)
    || !Number.isSafeInteger(hover.hoverRevision)
    || hover.hoverRevision <= 0
    || !Number.isSafeInteger(hover.documentEpoch)
    || hover.documentEpoch <= 0
  ) {
    throw new Error("SelectionCoordinator a întors un HoverSnapshot invalid.");
  }
}

function requireInspectorSelectionSummary(
  summary: SelectionCoordinatorSnapshot["inspectorSummary"],
  identity: CanvasProjectionIdentity,
  selectionRevision: number,
) {
  const states = new Set([
    "empty",
    "resolving",
    "resolved",
    "notRendered",
    "ambiguous",
    "uninspectable",
  ]);
  const reasons = new Set([
    "noSelection",
    "awaitingPhysicalFacts",
    "selectionNotRendered",
    "selectionAmbiguous",
    "inspectionDisabled",
    "missingRenderInstance",
  ]);
  if (
    !summary
    || summary.schemaVersion !== SELECTION_COORDINATOR_SCHEMA_VERSION
    || summary.projectRoot !== identity.projectRoot
    || summary.runtimeSessionId !== identity.runtimeSessionId
    || summary.selectionRevision !== selectionRevision
    || !sameCanvasProjectionIdentity(summary.canvasIdentity, identity)
    || !states.has(summary.state)
    || (
      summary.documentEpoch !== null
      && (!Number.isSafeInteger(summary.documentEpoch) || summary.documentEpoch <= 0)
    )
    || !Array.isArray(summary.classes)
    || !Array.isArray(summary.diagnostics)
    || summary.classes.some((className) => (
      typeof className !== "string"
      || className.length === 0
      || /\s|[\u0000-\u001f\u007f]/u.test(className)
    ))
    || (
      summary.reason !== null
      && !reasons.has(summary.reason)
    )
    || summary.diagnostics.some((diagnostic) => (
      !diagnostic
      || !reasons.has(diagnostic.code)
      || typeof diagnostic.message !== "string"
      || diagnostic.message.length === 0
    ))
  ) {
    throw new Error("InspectorSelectionSummary a întors altă selecție sau o stare invalidă.");
  }
  if (
    (summary.state === "empty" && (
      summary.subjectKind !== null
      || summary.selector !== null
      || summary.classes.length > 0
    ))
    || (summary.state === "resolved" && summary.subjectKind === null)
    || (summary.state === "resolved" && summary.reason !== null)
    || (summary.state !== "resolved" && summary.reason === null)
    || (summary.reason === null && summary.diagnostics.length > 0)
    || (summary.reason !== null && summary.diagnostics[0]?.code !== summary.reason)
  ) {
    throw new Error("InspectorSelectionSummary conține o proiecție inconsistentă.");
  }
}

function sameCanvasProjectionIdentity(
  left: CanvasProjectionIdentity,
  right: CanvasProjectionIdentity,
) {
  return left.projectRoot === right.projectRoot
    && left.runtimeSessionId === right.runtimeSessionId
    && left.workspaceRevision === right.workspaceRevision
    && left.transactionId === right.transactionId
    && left.previewRevision === right.previewRevision;
}

function sameCanvasInteractionIdentity(
  left: CanvasInteractionIdentity,
  right: CanvasInteractionIdentity,
) {
  return left.route === right.route
    && left.documentEpoch === right.documentEpoch
    && left.agentInstanceId === right.agentInstanceId
    && left.canvas.projectRoot === right.canvas.projectRoot
    && left.canvas.runtimeSessionId === right.canvas.runtimeSessionId
    && left.canvas.workspaceRevision === right.canvas.workspaceRevision
    && left.canvas.transactionId === right.canvas.transactionId
    && left.canvas.previewRevision === right.canvas.previewRevision;
}

export async function requestEditorEditScope(
  identity: CanvasProjectionIdentity,
  route: string,
  activeDocumentPath: string,
  scopeId: string,
  previewContextRenderInstanceId: string | null = null,
): Promise<EditScopeGrant> {
  const grant = await invoke<EditScopeGrant>("request_editor_edit_scope", {
    input: {
      identity,
      route,
      activeDocumentPath,
      previewContextRenderInstanceId,
      scopeId,
    },
  });
  if (grant.schemaVersion !== EDIT_SCOPE_GRANT_SCHEMA_VERSION) {
    throw schemaMismatch(
      "EditScopeGrant",
      grant.schemaVersion,
      EDIT_SCOPE_GRANT_SCHEMA_VERSION,
    );
  }
  if (
    grant.projectRoot !== identity.projectRoot
    || grant.runtimeSessionId !== identity.runtimeSessionId
    || grant.workspaceRevision !== identity.workspaceRevision
    || grant.previewRevision !== identity.previewRevision
    || grant.canvasTransactionId !== identity.transactionId
    || grant.scopeId !== scopeId
    || grant.activeDocumentPath !== activeDocumentPath
  ) {
    throw new Error("EditScopeGrant a întors alt context Canvas.");
  }
  return grant;
}

export async function planEditorMove(
  input: EditorMovePlanInput,
): Promise<EditorMovePlan> {
  const plan = await invoke<EditorMovePlan>("plan_editor_move", { input });
  if (plan.schemaVersion !== EDITOR_MOVE_PLAN_SCHEMA_VERSION) {
    throw schemaMismatch(
      "PlanEditorMove",
      plan.schemaVersion,
      EDITOR_MOVE_PLAN_SCHEMA_VERSION,
    );
  }
  if (
    plan.identity.projectRoot !== input.identity.projectRoot
    || plan.identity.runtimeSessionId !== input.identity.runtimeSessionId
    || plan.identity.workspaceRevision !== input.identity.workspaceRevision
    || plan.identity.transactionId !== input.identity.transactionId
    || plan.identity.previewRevision !== input.identity.previewRevision
    || plan.sourceNodeId !== input.sourceNodeId
    || plan.targetNodeId !== input.targetNodeId
    || plan.position !== input.position
    || plan.activeDocumentPath !== input.activeDocumentPath
  ) {
    throw new Error("PlanEditorMove a întors altă intenție sau identitate Canvas.");
  }
  if (plan.allowed !== Boolean(plan.token && plan.operation)) {
    throw new Error("PlanEditorMove a întors o stare permis/refuz inconsistentă.");
  }
  requireEditorMoveLiveProjection(plan);
  return plan;
}

function requireEditorMoveLiveProjection(plan: EditorMovePlan) {
  const projection = plan.liveProjection;
  if (!projection) {
    if (plan.liveProjectionReason === "ready") {
      throw new Error("PlanEditorMove a omis proiecția live marcată ready.");
    }
    return;
  }
  if (
    !plan.allowed
    || !plan.token
    || plan.liveProjectionReason !== "ready"
    || projection.schemaVersion !== EDITOR_MOVE_LIVE_PROJECTION_SCHEMA_VERSION
    || projection.operation !== "move"
    || projection.scope !== "selectedInstance"
    || projection.planToken !== plan.token
    || projection.position !== plan.position
    || projection.sourceRenderInstanceId.length === 0
    || projection.targetRenderInstanceId.length === 0
    || projection.sourceRenderInstanceId === projection.targetRenderInstanceId
    || !sameCanvasProjectionIdentity(projection.identity, plan.identity)
  ) {
    throw new Error("PlanEditorMove a întors o proiecție live inconsistentă.");
  }
}

export async function commitEditorMove(
  input: EditorMoveCommitInput,
): Promise<EditorMoveExecutionReceipt> {
  const receipt = await invoke<EditorMoveExecutionReceipt>(
    "commit_editor_move",
    { input },
  );
  if (receipt.schemaVersion !== EDITOR_MOVE_EXECUTION_SCHEMA_VERSION) {
    throw schemaMismatch(
      "EditorMoveExecutionReceipt",
      receipt.schemaVersion,
      EDITOR_MOVE_EXECUTION_SCHEMA_VERSION,
    );
  }
  if (receipt.planToken !== input.planToken) {
    throw new Error("EditorMoveExecutionReceipt aparține altui plan.");
  }
  if (
    receipt.projectRoot !== input.identity.projectRoot
    || receipt.runtimeSessionId !== input.identity.runtimeSessionId
  ) {
    throw new Error("EditorMoveExecutionReceipt aparține altei sesiuni.");
  }
  const timings = receipt.timings;
  if (
    !timings
    || !Number.isSafeInteger(timings.inputEmittedAtMs)
    || timings.inputEmittedAtMs < 0
    || timings.inputEmittedAtMs !== (input.inputEmittedAtMs ?? 0)
    || !Number.isSafeInteger(timings.planIssuedAtMs)
    || timings.planIssuedAtMs <= 0
    || !Number.isSafeInteger(timings.rustReceivedAtMs)
    || timings.rustReceivedAtMs <= 0
    || !Number.isSafeInteger(timings.rustCompletedAtMs)
    || timings.rustCompletedAtMs < timings.rustReceivedAtMs
    || !Number.isSafeInteger(timings.inputToReceiptMs)
    || timings.inputToReceiptMs < 0
    || !Number.isSafeInteger(timings.pointerUpToCommitReceiptMs)
    || timings.pointerUpToCommitReceiptMs < 0
    || timings.pointerUpToCommitReceiptMs !== timings.inputToReceiptMs
    || !Number.isSafeInteger(timings.planToReceiptMs)
    || timings.planToReceiptMs < 0
    || !Number.isSafeInteger(timings.rustCommandMs)
    || timings.rustCommandMs < 0
    || !Number.isSafeInteger(timings.candidateCloneMs)
    || timings.candidateCloneMs < 0
    || !Number.isSafeInteger(timings.mutationMs)
    || timings.mutationMs < 0
    || !Number.isSafeInteger(timings.recoveryPersistMs)
    || timings.recoveryPersistMs < 0
    || !Number.isSafeInteger(timings.authorityPublishMs)
    || timings.authorityPublishMs < 0
    || !Number.isSafeInteger(timings.authorityTransactionMs)
    || timings.authorityTransactionMs < 0
    || !Number.isSafeInteger(timings.planRevalidationMs)
    || timings.planRevalidationMs < 0
    || !Number.isSafeInteger(timings.nativeBlockContractMs)
    || timings.nativeBlockContractMs < 0
    || !Number.isSafeInteger(timings.workspaceStageMs)
    || timings.workspaceStageMs < 0
    || !Number.isSafeInteger(timings.afterProjectModelBuildMs)
    || timings.afterProjectModelBuildMs < 0
    || !Number.isSafeInteger(timings.aliasCalculationMs)
    || timings.aliasCalculationMs < 0
    || (
      timings.patchIssuedToReceiptMs !== null
      && (
        !Number.isSafeInteger(timings.patchIssuedToReceiptMs)
        || timings.patchIssuedToReceiptMs < 0
      )
    )
  ) {
    throw new Error("EditorMoveExecutionReceipt are telemetrie Rust invalidă.");
  }
  return receipt;
}

export async function readProjectAudit(): Promise<ProjectAuditSnapshot> {
  const snapshot = await invoke<ProjectAuditSnapshot>("read_project_audit");
  if (snapshot.schemaVersion !== PROJECT_AUDIT_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-project-audit"),
      snapshot.schemaVersion,
      PROJECT_AUDIT_SCHEMA_VERSION,
    );
  }
  return snapshot;
}

export async function readDesignClassInventory(): Promise<DesignClassInventorySnapshot> {
  const snapshot = await invoke<DesignClassInventorySnapshot>("read_design_class_inventory");
  if (snapshot.schemaVersion !== DESIGN_CLASS_INVENTORY_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-design-class"),
      snapshot.schemaVersion,
      DESIGN_CLASS_INVENTORY_SCHEMA_VERSION,
    );
  }
  return snapshot;
}

export function createDesignClass(
  name: string,
  relativePath: string,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "create_design_class",
    { name, relativePath, identity },
    identity,
  );
}

export async function renameDesignClass(
  oldName: string,
  newName: string,
  identity: FileBufferRequestIdentity,
): Promise<DesignClassRenameReceipt> {
  const receipt = await invoke<DesignClassRenameReceipt>("rename_design_class", {
    oldName,
    newName,
    identity,
  });
  if (receipt.schemaVersion !== DESIGN_CLASS_RENAME_SCHEMA_VERSION) {
    throw schemaMismatch(
      t("io-resource-design-class-rename"),
      receipt.schemaVersion,
      DESIGN_CLASS_RENAME_SCHEMA_VERSION,
    );
  }
  return receipt;
}

export function cancelPublishOperation(
  identity: FileBufferRequestIdentity,
): Promise<PublishOperationCancelReceipt> {
  return invoke<PublishOperationCancelReceipt>("cancel_publish_operation", { identity });
}

export async function resolveTemplateWorkbenchPlan(
  input: {
    templatePath: string;
    preferredPagePath?: string | null;
    preferredRoute?: string | null;
  },
  identity: PreviewStructuralCommandIdentity,
): Promise<TemplateWorkbenchPlan> {
  const plan = await invoke<TemplateWorkbenchPlan>(
    "resolve_template_workbench_plan",
    { input, identity },
  );
  if (plan.schemaVersion !== 4) {
    throw schemaMismatch("Template Workbench", plan.schemaVersion, 4);
  }
  return plan;
}

export function readCurrentProjectDiskManifest(): Promise<ProjectDiskManifest> {
  return invoke<ProjectDiskManifest>("read_current_project_disk_manifest");
}

export type ProjectDiskWatchIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

export type ProjectDiskWatchReceipt = {
  projectRoot: string;
  runtimeSessionId: string;
  watchGeneration: number;
};

export type ProjectDiskWatchStopIdentity = ProjectDiskWatchIdentity & {
  expectedWatchGeneration: number;
};

export function startProjectDiskWatch(
  input: ProjectDiskWatchIdentity,
): Promise<ProjectDiskWatchReceipt> {
  return invoke<ProjectDiskWatchReceipt>("start_project_disk_watch", { input });
}

export function stopProjectDiskWatch(
  input: ProjectDiskWatchStopIdentity,
): Promise<void> {
  return invoke<void>("stop_project_disk_watch", { input });
}

export async function reconcileCleanExternalProjectFiles(
  input: KernelExternalDiskReconcileInput,
): Promise<KernelExternalDiskReconcileReceipt> {
  const receipt = await invoke<KernelExternalDiskReconcileReceipt>(
    "reconcile_clean_external_project_files",
    { input },
  );
  if (receipt.schemaVersion !== 2) {
    throw schemaMismatch("External disk reconcile", receipt.schemaVersion, 2);
  }
  return receipt;
}

export function createProjectContentPage(options: {
  section: string;
  slug: string;
  title: string;
}, identity: FileBufferRequestIdentity): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_create_content_page", { ...options, identity }, identity);
}

export function updateProjectPageFrontmatterField(
  input: {
    relativePath: string;
    field: PageFrontmatterField;
    value: PageFrontmatterMutationValue;
  },
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_update_page_frontmatter_field",
    { input, identity },
    identity,
  );
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

export function createTemplate(
  input: CreateTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_create_template", { input, identity }, identity);
}

export function createListingItem(
  input: CreateListingItemInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_create_listing_item",
    { input, identity },
    identity,
  );
}

export function deleteListingItem(
  input: DeleteListingItemInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_delete_listing_item",
    { input, identity },
    identity,
  );
}

export function createSemanticTemplate(
  input: CreateSemanticTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_create_semantic_template",
    { input, identity },
    identity,
  );
}

export function createTemplateCollection(
  input: CreateTemplateCollectionInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_create_template_collection",
    { input, identity },
    identity,
  );
}

export function duplicateTemplate(
  input: DuplicateTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_duplicate_template", { input, identity }, identity);
}

export function overrideThemeTemplate(
  input: OverrideThemeTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_override_theme_template",
    { input, identity },
    identity,
  );
}

export function renameTemplate(
  input: RenameTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_rename_template", { input, identity }, identity);
}

export function setTemplateParent(
  input: SetTemplateParentInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_set_template_parent",
    { input, identity },
    identity,
  );
}

export function setTemplateAssignment(
  input: SetTemplateAssignmentInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "workspace_set_template_assignment",
    { input, identity },
    identity,
  );
}

export function deleteTemplate(
  input: DeleteTemplateInput,
  identity: FileBufferRequestIdentity,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation("workspace_delete_template", { input, identity }, identity);
}

export async function applyComponentMutation(
  input: ComponentMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<ComponentMutationApplyReceipt> {
  requireProjectFileRequestIdentity(identity);
  const receipt = await invoke<ComponentMutationApplyReceipt>("apply_component_mutation", {
    input,
    identity,
  });
  requireProjectFileReceiptIdentity(receipt.workspace, identity, "apply_component_mutation");
  if (receipt.plan.schemaVersion !== 2) {
    throw schemaMismatch(
      t("io-resource-component-plan"),
      receipt.plan.schemaVersion,
      2,
    );
  }
  return receipt;
}

export async function applyDataMutation(
  input: DataMutationInput,
  identity: FileBufferRequestIdentity,
): Promise<DataMutationApplyReceipt> {
  requireProjectFileRequestIdentity(identity);
  const receipt = await invoke<DataMutationApplyReceipt>("apply_data_mutation", {
    input,
    identity,
  });
  requireProjectFileReceiptIdentity(receipt.workspace, identity, "apply_data_mutation");
  if (receipt.plan.schemaVersion !== 1) {
    throw schemaMismatch(
      t("io-resource-data-plan"),
      receipt.plan.schemaVersion,
      1,
    );
  }
  return receipt;
}

export async function readDataNodeEditor(
  file: string,
  nodeId: string,
  identity: FileBufferRequestIdentity,
): Promise<DataNodeEditorSnapshot> {
  requireProjectFileRequestIdentity(identity);
  const snapshot = await invoke<DataNodeEditorSnapshot>("read_data_node_editor", {
    file,
    nodeId,
    identity,
  });
  if (snapshot.schemaVersion !== 1 || snapshot.file !== file || snapshot.nodeId !== nodeId) {
    throw new Error(t("io-data-node-selection-mismatch"));
  }
  return snapshot;
}

export async function readBlockRuntimeSnapshot(
  identity: FileBufferRequestIdentity,
): Promise<BlockRuntimeSnapshot> {
  requireProjectFileRequestIdentity(identity);
  const snapshot = await invoke<BlockRuntimeSnapshot>("read_block_runtime_snapshot", {
    identity,
  });
  if (
    snapshot.schemaVersion !== 2
    || snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("io-block-runtime-session-mismatch"));
  }
  return snapshot;
}

export async function readUiBlockGraph(
  identity: FileBufferRequestIdentity,
): Promise<UiBlockGraphSnapshot> {
  requireProjectFileRequestIdentity(identity);
  const snapshot = await invoke<UiBlockGraphSnapshot>("read_ui_block_graph", {
    identity,
  });
  if (
    snapshot.schemaVersion !== 2
    || snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("io-ui-block-graph-session-mismatch"));
  }
  return snapshot;
}

export async function readInsertCatalog(
  identity: FileBufferRequestIdentity,
  expectedWorkspaceRevision: number,
  context: InsertCatalogContext,
): Promise<InsertCatalogSnapshot> {
  requireProjectFileRequestIdentity(identity);
  const snapshot = await invoke<InsertCatalogSnapshot>("read_insert_catalog", {
    request: {
      identity,
      expectedWorkspaceRevision,
      context,
    },
  });
  if (
    snapshot.schemaVersion !== 1
    || snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
    || snapshot.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error("Catalogul de inserare nu mai aparține reviziei active.");
  }
  return snapshot;
}

export async function readDynamicWidgetSnapshot(
  request: DynamicWidgetSnapshotRequest,
): Promise<DynamicWidgetSnapshot> {
  requireProjectFileRequestIdentity(request.identity);
  const snapshot = await invoke<DynamicWidgetSnapshot>("read_dynamic_widget_snapshot", {
    request,
  });
  if (
    snapshot.schemaVersion !== 1
    || snapshot.projectRoot !== request.identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== request.identity.expectedSessionId
    || snapshot.workspaceRevision !== request.expectedWorkspaceRevision
    || snapshot.modelRevision !== request.expectedModelRevision
    || snapshot.previewRevision !== request.previewRevision
    || snapshot.sourceInstance.id !== request.sourceInstanceId
  ) {
    throw new Error("Snapshotul widgetului dinamic nu mai aparține selecției active.");
  }
  return snapshot;
}

export function updateDynamicWidget(
  input: UpdateDynamicWidgetInput,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "update_dynamic_widget",
    { input },
    input.request.identity,
  );
}

export function deleteDynamicWidget(
  input: DeleteDynamicWidgetInput,
): Promise<WorkspaceEntryMutationReceipt> {
  return invokeWorkspaceEntryMutation(
    "delete_dynamic_widget",
    { input },
    input.request.identity,
  );
}

export function readProjectFile(relativePath: string): Promise<string> {
  return invoke<string>("read_project_file", { relativePath });
}

function requireProjectFileRequestIdentity(identity: FileBufferRequestIdentity) {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(
      t("io-project-file-identity-invalid"),
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
      t("io-project-file-stale-receipt", {
        operation,
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
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
  | "canvas_patch_applied"
  | "canvas_patch_refused"
  | "canvas_patch_rolled_back"
  | "canvas_drag_preview_applied"
  | "canvas_drag_preview_skipped"
  | "canvas_fallback"
  | "canvas_stylesheets_promoted"
  | "canvas_ack_timeout";

export type PreviewStylesheetPromotionMetrics = {
  reused: number;
  staged: number;
  retired: number;
  preloadsReused?: number;
  preloadsStaged?: number;
  preloadsRetired?: number;
  headNodesReused?: number;
  headNodesCreated?: number;
  headNodesRetired?: number;
  headNodesReordered?: number;
  stylesheetAttributeMutations?: number;
  preloadAttributeMutations?: number;
  fontInvalidationCount?: number;
  fontFallbackFrames?: number;
  maxTextMetricDelta?: number;
  fontActivationErrorCount?: number;
  fontActivationDiagnostic?: string | null;
  fontsReadyMs?: number;
  activationToStyledMs: number;
};

export type PreviewRuntimeEventInput = {
  schemaVersion: 1;
  identity: CanvasProjectionIdentity;
  kind: PreviewRuntimeEventKind;
  durationMs: number;
  diagnostic: string | null;
  stylesheetMetrics?: PreviewStylesheetPromotionMetrics | null;
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
  preferredRoute: string | null;
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
    throw new Error(t("io-preview-identity-invalid"));
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
    throw new Error(t("io-preview-start-receipt-mismatch"));
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
    throw new Error(t("workspace-preview-receipt-mismatch", {
      operation: receipt.operation,
    }));
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

export function acknowledgeCanvasProjectionPhases(
  inputs: PreviewPhaseReceipt[],
): Promise<CanvasProjectionPlan> {
  return invoke<CanvasProjectionPlan>("acknowledge_canvas_projection_phases", { inputs });
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
    throw new Error(t("io-css-identity-invalid"));
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
  expectedWorkspaceRevision?: number,
): Promise<T> {
  if (!identity.expectedProjectRoot.trim() || !identity.expectedSessionId.trim()) {
    throw new Error(t("io-css-identity-invalid"));
  }
  const receipt = await invoke<FileBufferCommandReceipt<T>>(command, { ...args, identity });
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
  ) {
    throw new Error(
      t("io-css-stale-receipt", {
        command,
        expectedRoot: identity.expectedProjectRoot,
        expectedSession: identity.expectedSessionId,
        actualRoot: receipt.projectRoot,
        actualSession: receipt.runtimeSessionId,
      }),
    );
  }
  if (
    expectedWorkspaceRevision !== undefined
    && receipt.workspaceRevision !== expectedWorkspaceRevision
  ) {
    throw new Error(
      t("io-css-workspace-revision-mismatch", {
        command,
        actual: receipt.workspaceRevision,
        expected: expectedWorkspaceRevision,
      }),
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
    throw new Error(t("io-css-identity-invalid"));
  }
  const receipt = await invoke<CssMutationCommandReceipt<T>>(command, { ...args, identity });
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || !Number.isSafeInteger(receipt.workspaceRevision)
    || receipt.workspaceRevision < 0
    || receipt.workspaceRevision !== receipt.authority.revisionAfter
    || receipt.authority.projectRoot !== identity.expectedProjectRoot
    || receipt.authority.sessionId !== identity.expectedSessionId
  ) {
    throw new Error(
      t("io-css-foreign-session-receipt", { command }),
    );
  }
  const authority = receipt.authority;
  if (
    !Array.isArray(authority.touchedFiles)
    || !Array.isArray(authority.writtenFiles)
    || !Array.isArray(authority.removedFiles)
    || !Array.isArray(authority.documents)
  ) {
    throw new Error(t("io-css-authority-manifests-invalid", { command }));
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
    throw new Error(t("io-css-authority-receipt-invalid", { command }));
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
    throw new Error(t("io-css-authority-noop-effects", { command }));
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
    throw new Error(t("io-css-authority-staged-mismatch", { command }));
  }
  if (authority.status !== "noop" && authority.status !== "staged") {
    throw new Error(t("io-css-authority-status-invalid", { command }));
  }
  for (const projection of authority.documents) {
    const written = authority.writtenFiles.find((file) => file.relativePath === projection.relativePath);
    const removed = authority.removedFiles.includes(projection.relativePath);
    if (projection.snapshot === null) {
      if (!removed || written) {
        throw new Error(t("io-css-authority-delete-projection-invalid", { command }));
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
      throw new Error(t("io-css-authority-file-buffer-mismatch", { command }));
    }
  }
  return receipt;
}

export function getScssVariables(
  identity: CssRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<ScssVariable[]> {
  return invokeBoundCss<ScssVariable[]>(
    "get_scss_variables",
    {},
    identity,
    expectedWorkspaceRevision,
  );
}

export function readDesignTokenCatalog(
  identity: CssRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<DesignTokenCatalogSnapshot> {
  return invokeBoundCss<DesignTokenCatalogSnapshot>(
    "read_design_token_catalog",
    {},
    identity,
    expectedWorkspaceRevision,
  );
}

export function readThemeStyleCatalog(
  identity: CssRequestIdentity,
  expectedWorkspaceRevision?: number,
): Promise<ThemeStyleCatalogSnapshot> {
  return invokeBoundCss<ThemeStyleCatalogSnapshot>(
    "read_theme_style_catalog",
    {},
    identity,
    expectedWorkspaceRevision,
  );
}

export function previewThemeStyleDraft(
  targetId: string,
  properties: ThemeStylePropertyInput[],
  expectedWorkspaceRevision: number,
  identity: CssRequestIdentity,
): Promise<ThemeStyleDraftPreview> {
  return invokeBoundCss<ThemeStyleDraftPreview>(
    "preview_theme_style_draft",
    { targetId, properties, expectedWorkspaceRevision },
    identity,
    expectedWorkspaceRevision,
  );
}

export function applyThemeStyleDraft(
  targetId: string,
  properties: ThemeStylePropertyInput[],
  expectedWorkspaceRevision: number,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<ThemeStyleTargetSnapshot>> {
  return invokeBoundCssMutation<ThemeStyleTargetSnapshot>(
    "apply_theme_style_draft",
    { targetId, properties, expectedWorkspaceRevision },
    identity,
  );
}

export function getFontInventory(): Promise<FontInventory> {
  return invoke<FontInventory>("get_font_inventory");
}

export function getFontManager(
  identity: ProjectWorkspaceIdentity,
): Promise<FontManagerSnapshot> {
  return invoke<FontManagerSnapshot>("get_font_manager", { identity });
}

export function getFontPreviewAsset(
  file: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontPreviewAsset> {
  return invoke<FontPreviewAsset>("get_font_preview_asset", { file, identity });
}

export function assignFontRole(
  roleId: FontRoleId,
  family: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontRoleAssignmentReceipt> {
  return invoke<FontRoleAssignmentReceipt>("assign_font_role", {
    roleId,
    family,
    identity,
  });
}

export function setFontDisplay(
  family: string,
  display: "auto" | "block" | "swap" | "fallback" | "optional",
  identity: ProjectWorkspaceIdentity,
): Promise<FontDeliveryMutationReceipt> {
  return invoke<FontDeliveryMutationReceipt>("set_font_display", {
    family,
    display,
    identity,
  });
}

export function setFontPreload(
  file: string,
  enabled: boolean,
  identity: ProjectWorkspaceIdentity,
): Promise<FontDeliveryMutationReceipt> {
  return invoke<FontDeliveryMutationReceipt>("set_font_preload", {
    file,
    enabled,
    identity,
  });
}

export function planFontFamilyRemoval(
  family: string,
  directory: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontFamilyRemovalPlan> {
  return invoke<FontFamilyRemovalPlan>("plan_font_family_removal", {
    family,
    directory,
    identity,
  });
}

export function removeFontFamily(
  family: string,
  directory: string,
  expectedPlanToken: string,
  identity: ProjectWorkspaceIdentity,
): Promise<FontFamilyRemovalReceipt> {
  return invoke<FontFamilyRemovalReceipt>("remove_font_family", {
    family,
    directory,
    expectedPlanToken,
    identity,
  });
}

export function downloadGoogleFontFamily(
  family: string,
  weights: number[],
  styles: string[],
  variable: boolean,
  axes: GoogleFontAxis[],
  characterSet: string | null,
  identity: ProjectWorkspaceIdentity,
): Promise<GoogleFontInstallReceipt> {
  return invoke<GoogleFontInstallReceipt>("download_google_font_family", {
    family,
    weights,
    styles,
    variable,
    axes,
    characterSet,
    identity,
  });
}

export function searchGoogleFonts(query: string, limit = 40, offset = 0): Promise<GoogleFontCatalogFamily[]> {
  return invoke<GoogleFontCatalogFamily[]>("search_google_fonts", { query, limit, offset });
}

export function planLocalFontImport(
  sourcePaths: string[],
  identity: ProjectWorkspaceIdentity,
): Promise<LocalFontImportPlan> {
  return invoke<LocalFontImportPlan>("plan_local_font_import", { sourcePaths, identity });
}

export function applyLocalFontImport(
  sourcePaths: string[],
  expectedPlanToken: string,
  identity: ProjectWorkspaceIdentity,
): Promise<LocalFontImportReceipt> {
  return invoke<LocalFontImportReceipt>("apply_local_font_import", {
    sourcePaths,
    expectedPlanToken,
    identity,
  });
}

export function setScssVariable(
  relativePath: string,
  name: string,
  value: string,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>("set_scss_variable", { relativePath, name, value }, identity);
}

export function createScssVariable(
  relativePath: string,
  name: string,
  value: string,
  identity: CssRequestIdentity,
): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>(
    "create_scss_variable",
    { relativePath, name, value },
    identity,
  );
}

export type CssViewport = "desktop" | "tablet" | "mobile";

function isCssBackgroundProjection(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const background = value as {
    schemaVersion?: unknown;
    color?: unknown;
    layers?: unknown;
    shorthand?: unknown;
    opaqueProperties?: unknown;
    structurallyEditable?: unknown;
  };
  return background.schemaVersion === 1
    && (background.color === null || typeof background.color === "string")
    && Array.isArray(background.layers)
    && (background.shorthand === null || typeof background.shorthand === "string")
    && Boolean(background.opaqueProperties)
    && typeof background.opaqueProperties === "object"
    && typeof background.structurallyEditable === "boolean";
}

function isCssGridProjection(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const grid = value as {
    schemaVersion?: unknown;
    templateColumns?: unknown;
    templateRows?: unknown;
    templateAreas?: unknown;
    opaqueProperties?: unknown;
    structurallyEditable?: unknown;
  };
  return grid.schemaVersion === 1
    && Boolean(grid.templateColumns) && typeof grid.templateColumns === "object"
    && Boolean(grid.templateRows) && typeof grid.templateRows === "object"
    && Boolean(grid.templateAreas) && typeof grid.templateAreas === "object"
    && Boolean(grid.opaqueProperties) && typeof grid.opaqueProperties === "object"
    && typeof grid.structurallyEditable === "boolean";
}

export async function resolveCssInspectorContext(options: {
  templatePath: string | null;
  selector: string;
  viewport: CssViewport;
  fallbackFile: string | null;
  expectedWorkspaceRevision: number;
  expectedSelection: SelectionMutationIdentity;
}, identity: CssRequestIdentity): Promise<CssInspectorContextResolution> {
  const resolution = await invokeBoundCss<CssInspectorContextResolution>(
    "resolve_css_inspector_context",
    options,
    identity,
    options.expectedWorkspaceRevision,
  );
  const expectedRevision = options.expectedSelection.selectionRevision;
  if (
    resolution.schemaVersion !== CSS_INSPECTOR_CONTEXT_SCHEMA_VERSION
    || resolution.selectionRevision !== expectedRevision
    || resolution.selector !== options.selector.trim()
    || resolution.viewport !== options.viewport
    || !["existing", "creation", "ambiguous"].includes(resolution.state)
    || !Array.isArray(resolution.candidates)
  ) {
    throw new Error("[css_inspector_invalid_receipt] Rust a returnat o rezoluție CSS inconsistentă.");
  }
  for (const candidate of resolution.candidates) {
    if (
      !candidate.file
      || candidate.ruleContext.file !== candidate.file
      || candidate.ruleContext.selector !== resolution.selector
      || candidate.ruleContext.viewport !== resolution.viewport
      || !isCssBackgroundProjection(candidate.ruleContext.background)
      || !isCssGridProjection(candidate.ruleContext.grid)
    ) {
      throw new Error("[css_inspector_invalid_receipt] Candidatul CSS nu corespunde rezoluției.");
    }
  }
  if (resolution.state === "ambiguous") {
    if (
      resolution.target !== null
      || resolution.ruleContext !== null
      || resolution.candidates.length < 2
    ) {
      throw new Error("[css_inspector_invalid_receipt] Ambiguitatea CSS nu este completă.");
    }
    return resolution;
  }
  if (
    !resolution.target
    || !resolution.ruleContext
    || !Array.isArray(resolution.target.consumerFiles)
    || !Array.isArray(resolution.target.consumerTemplates)
    || resolution.target.file !== resolution.ruleContext.file
    || resolution.target.selector !== resolution.selector
    || resolution.ruleContext.selector !== resolution.selector
    || resolution.ruleContext.viewport !== resolution.viewport
    || !isCssBackgroundProjection(resolution.ruleContext.background)
    || !isCssGridProjection(resolution.ruleContext.grid)
    || (resolution.state === "existing" && resolution.candidates.length !== 1)
    || (resolution.state === "creation" && resolution.candidates.length > 1)
  ) {
    throw new Error("[css_inspector_invalid_receipt] Ținta CSS nu corespunde contextului atomic.");
  }
  return resolution;
}

export function setCssRule(options: {
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  expectedSelection?: SelectionMutationIdentity | null;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<void>> {
  return invokeBoundCssMutation<void>("set_css_rule", options, identity);
}

export function setCssRuleAtViewport(options: {
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  viewport: CssViewport;
  expectedSelection?: SelectionMutationIdentity | null;
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
  expectedSelection?: SelectionMutationIdentity | null;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<PageCssWriteResult>> {
  return invokeBoundCssMutation<PageCssWriteResult>("set_page_css_rule_at_viewport", options, identity);
}

export function setReusableCssRuleAtViewport(options: {
  templatePath: string;
  relativePath: string;
  selector: string;
  properties: Partial<Record<keyof EditableStyles | string, string>>;
  viewport: CssViewport;
  cachebustAssets: boolean;
  expectedSelection?: SelectionMutationIdentity | null;
}, identity: CssRequestIdentity): Promise<CssMutationCommandReceipt<ReusableCssWriteResult>> {
  return invokeBoundCssMutation<ReusableCssWriteResult>(
    "set_reusable_css_rule_at_viewport",
    options,
    identity,
  );
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

export async function applyMotionMutation(
  input: MotionPageMutationInput,
): Promise<MotionPageMutationReceipt> {
  const receipt = await invoke<MotionPageMutationReceipt>("apply_motion_mutation", { input });
  if (
    receipt.mutation.schemaVersion !== 3
    || (receipt.mutation.transaction && receipt.mutation.transaction.schemaVersion !== 3)
  ) {
    throw schemaMismatch(
      "Motion mutation",
      receipt.mutation.schemaVersion,
      3,
    );
  }
  return receipt;
}

export function saveZolaBaseUrl(url: string): Promise<void> {
  return invoke("save_zola_base_url", { url });
}

export function readThemeCatalog(
  identity: ProjectWorkspaceIdentity | null,
): Promise<ThemeCatalogSnapshot> {
  return invoke<ThemeCatalogSnapshot>("read_theme_catalog", { identity });
}

export function planThemeChange(request: ThemePlanRequest): Promise<ThemePlan> {
  return invoke<ThemePlan>("plan_theme_change", { request });
}

export function applyThemeChange(
  plan: ThemePlanRequest,
  expectedPlanToken: string,
): Promise<ThemeApplyReceipt> {
  return invoke<ThemeApplyReceipt>("apply_theme_change", {
    request: { plan, expectedPlanToken },
  });
}

export function zolaBuild(): Promise<string> {
  return invoke<string>("zola_build");
}

export function zolaCheck(): Promise<string> {
  return invoke<string>("zola_check");
}

export function zolaCheckWorkspace(): Promise<string> {
  return invoke<string>("zola_check_workspace");
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
