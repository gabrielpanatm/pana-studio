import type { CodeEditorContextMenuRequest, CodeEditorController } from "$lib/editor/controller";
import { tick } from "svelte";
import { contextMenu } from "$lib/context-menu/store.svelte";
import {
  createEditorRuntime,
  type EditorRuntime,
  type EditorRuntimeHost,
} from "$lib/editor-runtime/runtime";
import {
  createPreviewRuntime,
  type CanvasPatchPerformanceSnapshot,
  type PreviewRuntime,
  type PreviewRuntimeHost,
} from "$lib/editor-runtime/preview-runtime";
import {
  captureCanvasElementObservation,
  htmlTargetFromPageSection,
  htmlTargetFromCoordinatedSelection,
  type EditorHtmlTarget,
  type EditorTeraTarget,
} from "$lib/editor-runtime/commands";
import {
  blockedAction,
  committedAction,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import { createDefaultEditableStyles } from "$lib/editor/defaults";
import type { AppNotification } from "$lib/notifications/center";
import {
  readApplicationSettings,
  saveApplicationSettings,
} from "$lib/application/io";
import type {
  CanvasProjectionIdentity,
  CanvasProjectionPlan,
} from "$lib/project/io";
import {
  acceptSelectionObservation,
  applySelectionIntent as applySelectionIntentInRust,
  readEditorNavigationSnapshot,
  readSelectionSnapshot,
  readStartupFlow,
  stopVersionPreview,
} from "$lib/project/io";
import {
  buildInteractivePreviewUrl,
  type InteractivePreviewDomNode,
} from "$lib/preview/interactive";
import { createDiskState, type DiskState } from "$lib/session/disk-state";
import {
  registerEditFlushHandler,
  type EditFlushReason,
} from "$lib/session/edit-flush-registry";
import {
  createLatestWinsAsyncQueue,
  type LatestWinsAsyncQueue,
} from "$lib/session/latest-wins-async-queue";
import { pageJsRelativePath } from "$lib/js/page-path";
import { dispatchExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";
import {
  flushWorkspaceMutationInputs,
  settleProjectWorkspaceMutation,
  type WorkspaceDerivedProjectionStatus,
} from "$lib/session/workspace-mutation-coordinator";
import {
  drainPreviewStructuralLanes,
  requireCurrentPreviewStructuralSession,
  runInPreviewStructuralLane,
} from "$lib/kernel/preview-structural-lane";
import {
  applyTagChange as applyHtmlTagChange,
  changeElementTag as changeHtmlElementTag,
  type HtmlEditControllerHost,
} from "$lib/state/html-edit-controller";
import {
  saveActiveFile as saveActiveDocument,
  saveSessionDrafts as saveSessionDraftsFromController,
  savePendingHtmlChanges as savePendingHtmlChangesFromController,
  saveSourceFile as saveSourceFileFromController,
  type SaveControllerHost,
} from "$lib/state/save-controller";
import {
  currentGlobalStatus as currentGlobalStatusFromController,
  type StatusControllerHost,
} from "$lib/state/status-controller";
import type {
  GlobalStatusEvent,
  GlobalStatusEscalationRequest,
  GlobalStatusKind,
  GlobalStatusPublishOptions,
} from "$lib/status/global-status";
import type { InsertPosition } from "$lib/html/mutations";
import {
  isLatestHtmlAttributeDraftSettlement,
  liveProjectableHtmlAttributeDraft,
} from "$lib/html/live-attribute-draft";
import type { TeraDropRequest, TeraPaletteItem } from "$lib/tera/model";
import {
  applyAttributesToHtml as applyAttributesToHtmlFromController,
  applyAttributesToCapturedHtmlTarget,
  applyClassesToHtml as applyClassesToHtmlFromController,
  generateClassForSelectedHtml as generateClassForSelectedHtmlFromController,
  generateDataAnimForSelectedHtml as generateDataAnimForSelectedHtmlFromController,
  applyImageSourceToHtml as applyImageSourceToHtmlFromController,
  applyZolaImageProcessingToHtml as applyZolaImageProcessingToHtmlFromController,
  applyNativeBlockOptionToHtml as applyNativeBlockOptionToHtmlFromController,
  applyTextContentToCapturedHtmlTarget,
  captureHtmlActionTarget,
  insertPaletteElementAtTarget as insertPaletteElementAtTargetFromController,
  insertNodeRelative as insertNodeRelativeFromController,
  openSourceLocation as openSourceLocationFromController,
  type HtmlActionTarget,
  type HtmlActionsControllerHost,
  type ApplyNativeBlockOptionRequest,
} from "$lib/state/html-actions-controller";
import type { HtmlPaletteElement } from "$lib/project/html-palette";
import type { ResizeKind } from "$lib/ui/resize";
import {
  DEFAULT_PREVIEW_ZOOM,
  resetPreviewZoom as resetPreviewZoomFromController,
  resetResize as resetResizeFromController,
  setUiTheme as setUiThemeFromController,
  setPreviewZoom as setPreviewZoomFromController,
  startResizeDrag as startResizeDragFromController,
  stopResizeDrag as stopResizeDragFromController,
  toggleUiTheme as toggleUiThemeFromController,
  type UiControllerHost,
} from "$lib/state/ui-controller";
import type {
  CanvasPatch,
  CssRuleContext,
  CssMutationAuthorityReceipt,
  EditableAttributes,
  EditableStyles,
  HtmlPendingArea,
  InspectorPendingArea,
  InspectorTab,
  AiCoordinationSnapshot,
  AiContextStatus,
  ApplicationSettingsPatch,
  ApplicationSettingsSnapshot,
  ApplicationSurface,
  ApplicationTheme,
  ApplicationThemePreference,
  CenterView,
  DesignClassInventorySnapshot,
  EditScopeGrant,
  EditorMovePlan,
  EditorNavigationNode,
  EditorNavigationSnapshot,
  ExternalDiskState,
  FileExplorerCommitReceipt,
  FileExplorerOperationPlan,
  FileExplorerOperationRequest,
  FileExplorerSnapshot,
  PageSection,
  ProjectDiskManifest,
  ProjectAuditSnapshot,
  ProjectFile,
  ProjectMovePosition,
  ProjectScan,
  StartupCreationCatalog,
  StartupCreationPlan,
  StartupFlowSnapshot,
  ProjectZolaImageIntent,
  ProjectWorkspaceSnapshot,
  WorkbenchActivity,
  WorkbenchBottomPanelView,
  WorkbenchCanvasMode,
  WorkbenchCanvasPreset,
  WorkbenchCanvasViewportSnapshot,
  WorkbenchIntent,
  WorkbenchSnapshot,
  WorkbenchSplit,
  WorkbenchSurface,
  VersionPreviewReceipt,
  ScssVariable,
  HoverSnapshot,
  AcceptedCanvasElementObservation,
  CoordinatedElementSelection,
  InspectorHtmlPhysicalFacts,
  InspectorSelectionSummarySnapshot,
  BlockSelectionContext,
  CanvasElementObservation,
  SelectionIntent,
  SelectionObservationInput,
  SelectionSnapshot,
  SourceEditLocation,
  SourceGraph,
  TemplateWorkbenchPlan,
  SourceGraphNode,
} from "$lib/types";
import {
  commitFileExplorerOperation as commitFileExplorerOperationInRust,
  planFileExplorerOperation as planFileExplorerOperationInRust,
  readFileExplorerSnapshot,
  selectFileExplorerEntry as selectFileExplorerEntryInRust,
} from "$lib/project/file-explorer";
import {
  WorkbenchProjectionController,
  type WorkbenchProjectionHost,
} from "$lib/workbench/controller";
import {
  createInspectorPendingSourceRegistry,
  type InspectorPendingSource,
} from "$lib/state/inspector-pending";
import {
  MotionWorkspaceState,
  type MotionPreviewMode,
} from "$lib/state/motion-workspace.svelte";
import { TerminalController } from "$lib/terminal/controller";
import {
  defaultTerminalPaneHeight,
  terminalQuickTasks as defaultTerminalQuickTasks,
  type TerminalQuickTask,
  type TerminalTab,
} from "$lib/terminal/runtime";
import {
  closeTerminalTab as closeTerminalTabFromController,
  initialTerminalTabs,
  openTerminalTab as openTerminalTabFromController,
  selectTerminalTab as selectTerminalTabFromController,
  type TerminalTabsHost,
} from "$lib/state/terminal-tabs-controller";
import {
  clearActiveTerminal as clearActiveTerminalFromController,
  runTerminalQuickTask as runTerminalQuickTaskFromController,
  type TerminalQuickTaskHost,
} from "$lib/state/terminal-quick-task-controller";
import {
  cancelCanvasProjectionConfirmation,
  cancelPreviewSync as cancelPreviewSyncFromController,
  fetchDomTreeFromPreview as fetchDomTreeFromPreviewFromController,
  getPreviewDocument as getPreviewDocumentFromController,
  hasMountedCanvasProjectionSurface as hasMountedCanvasProjectionSurfaceFromController,
  invalidatePreviewDomTreeProjection,
  invalidatePreviewRefreshLease,
  mountCanvasProjectionSurface as mountCanvasProjectionSurfaceFromController,
  postPreviewMessage as postPreviewMessageFromController,
  prepareCanvasProjectionNavigation as prepareCanvasProjectionNavigationFromController,
  previewReloadUrl as previewReloadUrlFromController,
  reconcileTemplateWorkbenchPreviewDocument as reconcileTemplateWorkbenchPreviewDocumentFromController,
  refreshRenderedPreviewDocument as refreshRenderedPreviewDocumentFromController,
  reloadPreview as reloadPreviewFromController,
  sendPreviewOperation as sendPreviewOperationFromController,
  unmountCanvasProjectionSurface as unmountCanvasProjectionSurfaceFromController,
  type CanvasProjectionConfirmation,
  type PreviewControllerHost,
  type PreviewRefreshLease,
} from "$lib/state/preview-controller";
import type { SelectionControllerHost } from "$lib/state/selection-controller";
import {
  projectSelectionSnapshotOnCanvas,
  selectCanvasPreviewElement,
} from "$lib/state/canvas-interaction-controller";
import {
  createSourceEditor as createSourceEditorFromController,
  handleCodeCursorSelection as handleCodeCursorSelectionFromController,
  syncCodeSelectionHighlight as syncCodeSelectionHighlightFromController,
  updateMarkdownSource as updateMarkdownSourceFromController,
  withSyncingCode as withSyncingCodeFromController,
  type SourceEditorControllerHost,
} from "$lib/state/source-editor-controller";
import {
  removeAttribute as removeAttributeFromController,
  htmlTextSelectionKey,
  updateAttributeValue as updateAttributeValueFromController,
  updateTextContentValue as updateTextContentValueFromController,
  type HtmlDraftControllerHost,
} from "$lib/state/html-draft-controller";
import {
  updatePageFrontmatterSource as updatePageFrontmatterSourceFromController,
  type PageSettingsControllerHost,
} from "$lib/state/page-settings-controller";

import {
  requestControlledPreviewRefresh as requestControlledPreviewRefreshFromController,
  runZolaValidation as runZolaValidationFromController,
  scheduleZolaValidation as scheduleZolaValidationFromController,
  type ControlledPreviewControllerHost,
} from "$lib/state/controlled-preview-controller";
import {
  createControlledPreviewState,
  markPreviewLive,
  markPreviewSaved,
  type ControlledPreviewState,
  type PreviewRefreshReason,
  type ZolaValidationReason,
} from "$lib/preview/controlled";
import {
  applyInspectorLiveProperties as applyInspectorLivePropertiesFromController,
  applyInspectorLivePropertyDrafts as applyInspectorLivePropertyDraftsFromController,
  bindInspectorLiveCssTransaction,
  breakpointValue as breakpointValueFromController,
  captureInspectorLiveCssIdentity,
  clearInspectorLiveProperties as clearInspectorLivePropertiesFromController,
  injectRawCss as injectRawCssFromController,
  restoreLiveCssLayersToPreview as restoreLiveCssLayersToPreviewFromController,
  type InspectorCssDraft,
  type InspectorLiveCssIdentity,
  type PreviewLiveControllerHost,
} from "$lib/state/preview-live-controller";
import {
  cancelProjectOpenRecoveryDecision as cancelProjectOpenRecoveryDecisionFromController,
  closeCurrentProject as closeCurrentProjectFromController,
  continueProjectOpenWithRecoveryAbandonment as continueProjectOpenWithRecoveryAbandonmentFromController,
  createContentPageFromInput as createContentPageFromInputFromController,
  continueProjectTransitionWithOperatorDecision as continueProjectTransitionWithOperatorDecisionFromController,
  applyStartupProject as applyStartupProjectFromController,
  cancelStartupCreationPlan as cancelStartupCreationPlanFromController,
  discardSessionAndReloadFromDisk as discardSessionAndReloadFromDiskFromController,
  loadScannedProjectFile as loadScannedProjectFileFromController,
  openCurrentProjectInBrowser as openCurrentProjectInBrowserFromController,
  openProjectFolder as openProjectFolderFromController,
  planStartupProject as planStartupProjectFromController,
  reattachCurrentProjectSession as reattachCurrentProjectSessionFromController,
  reconcileWorkspaceDerivedState as reconcileWorkspaceDerivedStateFromController,
  rescanCurrentProject as rescanCurrentProjectFromController,
  rescanCurrentProjectWithinKernelUndoRedoLease as rescanCurrentProjectWithinKernelUndoRedoLeaseFromController,
  resetProjectScopedState as resetProjectScopedStateFromController,
  selectStartupCreationOption as selectStartupCreationOptionFromController,
  exitTemplateWorkbench as exitTemplateWorkbenchFromController,
  updateTemplateWorkbenchContext as updateTemplateWorkbenchContextFromController,
  type ProjectControllerHost,
} from "$lib/state/project-controller";
import type { ProjectOpenRecoveryDecisionRequest } from "$lib/project/open-recovery";
import type { KernelUndoRedoProjectionLease } from "$lib/kernel/undo-redo-projection-lease";
import {
  PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
  type ProjectTransitionDecisionRequest,
} from "$lib/project/transition-decision";
import {
  stageKernelPlannedTemplateDraft as stageKernelPlannedTemplateDraftFromController,
  type HtmlMutationControllerHost,
} from "$lib/state/html-mutation-controller";
import {
  editorNavigationNodeSelector,
  editorNavigationDropTargetStatus as editorNavigationDropTargetStatusFromController,
  enterEditorNavigationScope as enterEditorNavigationScopeFromController,
  exitEditorNavigationScope as exitEditorNavigationScopeFromController,
  hoverEditorNavigationNode as hoverEditorNavigationNodeFromController,
  moveEditorNavigationNode as moveEditorNavigationNodeFromController,
  previewEditorNavigationMove as previewEditorNavigationMoveFromController,
  selectEditorNavigationNode as selectEditorNavigationNodeFromController,
  type EditorNavigationControllerHost,
} from "$lib/state/editor-navigation-controller";
import {
  type PreviewInsertControllerHost,
  type PreviewInsertDropRequest,
} from "$lib/state/preview-insert-controller";
import {
  type PreviewTeraInsertControllerHost,
} from "$lib/state/preview-tera-insert-controller";
import {
  startElementPaletteDrag as startElementPaletteDragFromController,
  type ElementPaletteDragHost,
} from "$lib/state/element-palette-drag-controller";
import {
  startTeraPaletteDrag as startTeraPaletteDragFromController,
  type TeraPaletteDragHost,
} from "$lib/state/tera-palette-drag-controller";
import {
  deleteSelectedTeraNode as deleteSelectedTeraNodeFromController,
  insertTeraPaletteItemAtTarget as insertTeraPaletteItemAtTargetFromController,
  type TeraActionsControllerHost,
} from "$lib/state/tera-actions-controller";
import {
  type AiContextControllerHost,
} from "$lib/state/ai-context-controller";
import {
  hydratePageSections as hydratePageSectionsFromController,
  resetPageSections as resetPageSectionsFromController,
  setPageSections as setPageSectionsFromController,
  type PageSectionsHost,
} from "$lib/state/page-sections-controller";
import {
  acceptProjectWorkspaceSaveBaseline as acceptProjectWorkspaceSaveBaselineFromController,
  createExternalDiskState,
  establishExternalDiskBaseline as establishExternalDiskBaselineFromController,
  invalidateExternalReconcileForProjectTransition as invalidateExternalReconcileForProjectTransitionFromController,
  markWorkspaceProjectionRecoveryRequired as markWorkspaceProjectionRecoveryRequiredFromController,
  resumeExternalMonitoringAfterFailedTransition as resumeExternalMonitoringAfterFailedTransitionFromController,
  resetExternalDiskState as resetExternalDiskStateFromController,
  resumeExternalDiskMonitoringAfterSave as resumeExternalDiskMonitoringAfterSaveFromController,
  resumeExternalDiskMonitoringAfterTransitionLease as resumeExternalDiskMonitoringAfterTransitionLeaseFromController,
  startExternalDiskPolling as startExternalDiskPollingFromController,
  suspendAndDrainExternalDiskMonitoring as suspendAndDrainExternalDiskMonitoringFromController,
  type ExternalDiskControllerHost,
} from "$lib/state/external-disk-controller";
import {
  flushFileBufferDraftSync,
  rebaseFileBufferDraftSyncProjection,
} from "$lib/session/file-buffer-draft-sync";
import { projectLatestProjectWorkspacePreview } from "$lib/kernel/project-workspace-preview-coordinator";
import {
  createDesignClass as createDesignClassCommand,
  createScssVariable,
  createCssRequestIdentity,
  getScssVariables,
  readDesignClassInventory,
  readProjectAudit,
  readProjectFile,
  readProjectWorkspaceState,
  recordPreviewRuntimeEvent,
  renameDesignClass as renameDesignClassCommand,
  setScssVariable,
  type PreviewRuntimeEventKind,
} from "$lib/project/io";
import { scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import {
  cssRuleContextFromSource,
  type CssViewport,
} from "$lib/css/source-sync";
import { errorMessage } from "$lib/util";
import { l10n, t } from "$lib/i18n/runtime.svelte";
import {
  applyApplicationBootProjection,
  storeApplicationBootProjection,
} from "$lib/system-preferences/boot-projection";
import {
  createEmptyHtmlPending,
  createEmptyInspectorPending,
  contrastingTextColor,
  initialUiTheme,
  type PreviewTeraSelectionTarget,
} from "$lib/state/app-helpers";
import { registerAppEffects } from "$lib/state/app-effects.svelte";
import {
  applySelectionState as applySelectionStateFromAppSelectionController,
  clearPreviewSelection as clearPreviewSelectionFromController,
  openSelectedTeraSource as openSelectedTeraSourceFromController,
  selectTeraLayerSource as selectTeraLayerSourceFromController,
  setPreviewTeraSelection as setPreviewTeraSelectionFromController,
} from "$lib/state/app-selection-controller";
import {
  applyStagedOverrideStylesToPreview as applyStagedOverrideStylesToPreviewFromController,
  attachPreviewInspector as attachPreviewInspectorFromController,
  handlePreviewMessage as handlePreviewMessageFromController,
  previewUrlForScannedFile as previewUrlForScannedFileFromController,
  refreshSourceGraph as refreshSourceGraphFromController,
  resolveSourceEditLocationForSourceId as resolveSourceEditLocationForSourceIdFromController,
  resolveSourceEditTargetForSourceId as resolveSourceEditTargetForSourceIdFromController,
  syncHtmlCodeToPreview as syncHtmlCodeToPreviewFromController,
} from "$lib/state/app-preview-runtime-controller";
import {
  clearHtmlPending as clearHtmlPendingFromController,
  clearNotification as clearNotificationFromController,
  dismissNotification as dismissNotificationFromController,
  handleNotificationAction as handleNotificationActionFromController,
  escalateGlobalStatus as escalateGlobalStatusFromController,
  refreshCurrentSession as refreshCurrentSessionFromController,
  refreshGlobalStatusFromKernel as refreshGlobalStatusFromKernelFromController,
  setGlobalStatus as setGlobalStatusFromAppSessionController,
  setHtmlPending as setHtmlPendingFromController,
  setInspectorPending as setInspectorPendingFromController,
} from "$lib/state/app-session-controller";
import {
  deriveActiveRenderedPreviewPageFile,
  deriveActiveRenderedTemplatePath,
  deriveActiveTemplateFile,
  deriveActiveTerminalTab,
  deriveAppDirtyState,
  deriveCanAddChildToSelectedElement,
  deriveCanEditHtml,
  deriveCanPreviewCurrentSource,
  deriveCurrentHtmlRelativePath,
  deriveCurrentProjectPath,
  deriveCurrentSourceCacheKey,
  deriveCurrentSourcePath,
  deriveCurrentSourceRelativePath,
  deriveHtmlSourceMutationBlockedReason,
  deriveHtmlSourceNodes,
  deriveIsActiveRenderedPreviewPage,
  deriveIsActivePreviewHtmlSource,
  deriveScannedFilesByRole,
  deriveSelectedEditorNavigationNode,
  deriveSelectedSemanticSourceLocation,
  deriveSelectedSourceEditTarget,
  deriveSelectedTemplateSourceNode,
  deriveSessionHasPending,
  deriveSourceLanguage,
  deriveWorkbenchSourceStatus,
} from "$lib/state/app-derived";
import {
  destroyApp as destroyAppFromController,
  initFromStorage as initFromStorageFromController,
} from "$lib/state/app-lifecycle-controller";
import {
  cancelPendingNativeWindowClose,
  closeNativeWindowIfProjectClosed,
} from "$lib/state/native-window-close-controller";
// ── Constants ────────────────────────────────────────────────────────────────

const DEFAULT_LEFT_PANE_WIDTH = 260;
const DEFAULT_RIGHT_PANE_WIDTH = 320;
const HTML_TEXT_RECOVERY_INTERVAL_MS = 200;
const HTML_TEXT_CANONICAL_IDLE_MS = 650;
const HTML_TEXT_HISTORY_IDLE_MS = 1_800;

type CodeRevealTarget =
  | { kind: "html" }
  | { kind: "css"; selector: string; file: string };

// ── AppState class ───────────────────────────────────────────────────────────

type HtmlTextDraftCommitTask = Readonly<{
  key: string;
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  target: HtmlActionTarget;
  text: string;
  editSessionId: string;
}>;

type ActiveHtmlTextEditSession = {
  id: string;
  key: string;
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  target: HtmlActionTarget;
  text: string;
  projectedText: string | null;
};

type ActiveHtmlAttributeEditSession = {
  id: string;
  key: string;
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  target: HtmlActionTarget;
  attributes: EditableAttributes;
  baselineAttributes: EditableAttributes;
  baselineNames: string[];
  latestLiveEpoch: number;
  latestLiveProjection: Promise<void> | null;
  finishPromise: Promise<EditorActionOutcome | null> | null;
};

function canvasIdentityEquals(
  left: CanvasProjectionIdentity | null | undefined,
  right: CanvasProjectionIdentity | null | undefined,
) {
  return Boolean(
    left
    && right
    && left.projectRoot === right.projectRoot
    && left.runtimeSessionId === right.runtimeSessionId
    && left.workspaceRevision === right.workspaceRevision
    && left.transactionId === right.transactionId
    && left.previewRevision === right.previewRevision,
  );
}

function editorNavigationRoute(previewUrl: string, fallbackRoute: string) {
  if (previewUrl && previewUrl !== "about:blank") {
    try {
      return new URL(previewUrl, "http://pana.local/").pathname || "/";
    } catch {
      // The route falls back to the project browser route below.
    }
  }
  const fallback = fallbackRoute.trim() || "/";
  return fallback.startsWith("/") ? fallback : `/${fallback}`;
}

export class AppState {
  // Expose constants for template access
  readonly motionWorkspace = new MotionWorkspaceState();

  // ── DOM refs (set by component via $effect) ──
  previewFrame = $state<HTMLIFrameElement | undefined>(undefined);
  canvasSurfaceElement: HTMLIFrameElement | null = null;
  canvasSurfaceGeneration = 0;
  canvasSurfaceResumeRequired = false;
  private canvasSurfaceResumePromise: Promise<void> | null = null;
  codeEditorHost = $state<HTMLDivElement | undefined>(undefined);
  terminalHost = $state<HTMLDivElement | undefined>(undefined);

  // ── Editor / source state ──
  source = $state("");
  sourceCache = $state<Record<string, string>>({});
  /** Local UI edits only; used to reject stale asynchronous UI settlements. */
  editorMutationEpoch = $state(0);
  /** Durable Rust authority notifications; drives read-only workspace mirrors. */
  projectWorkspaceMutationEpoch = $state(0);
  acceptedSelectionObservation = $state<AcceptedCanvasElementObservation | null>(null);

  // ── Element editor values ──
  attributeValues = $state<EditableAttributes>({});
  attributeStatus = $state("");
  textContentValue = $state("");
  activeHtmlTextEditKey = $state<string | null>(null);
  activeHtmlTextEditValue = $state<string | null>(null);
  textEditOriginalKey = $state<string | null>(null);
  textEditOriginalText = $state<string | null>(null);
  textStatus = $state("");
  classEditorValue = $state("");
  classStatus = $state("");
  imageSourceValue = $state("");
  imageStatus = $state("");
  pendingTag = $state<string | null>(null);
  pendingTagOriginal = $state<string | null>(null);
	  pendingTagSourceLocation = $state<SourceEditLocation | null>(null);
  tagStatus = $state("");
  structureStatus = $state("");
  htmlPending = $state<Record<HtmlPendingArea, boolean>>(createEmptyHtmlPending());
  inspectorPending = $state<Record<InspectorPendingArea, boolean>>(createEmptyInspectorPending());
  inspectorPendingSources = createInspectorPendingSourceRegistry();
  private readonly htmlTextDraftCommitQueue: LatestWinsAsyncQueue<HtmlTextDraftCommitTask> =
    createLatestWinsAsyncQueue<HtmlTextDraftCommitTask>({
      key: (task) => task.key,
      delayMs: HTML_TEXT_RECOVERY_INTERVAL_MS,
      delayMode: "throttle",
      run: async (task) => {
        if (
          task.projectRoot !== this.sessionProjectRoot
          || task.runtimeSessionId !== this.kernelProjectSessionId
          || task.projectSessionEpoch !== this.projectSessionEpoch
        ) return;
        const result = await applyTextContentToCapturedHtmlTarget(
          this.htmlActionsControllerHost(),
          task.target,
          task.text,
          {
            deferCanonicalProjection: true,
            editSessionId: task.editSessionId,
          },
        );
        if (result.status !== "committed" && result.status !== "noop") {
          throw new Error(result.reason ?? t("workbench-text-draft-status", {
            status: result.status,
          }));
        }
      },
      onError: (error, task) => {
        if (
          task.projectRoot !== this.sessionProjectRoot
          || task.runtimeSessionId !== this.kernelProjectSessionId
          || task.projectSessionEpoch !== this.projectSessionEpoch
        ) return;
        this.setGlobalStatus(
          t("workbench-text-draft-kernel-failed", {
            message: error instanceof Error ? error.message : String(error),
          }),
          "error",
        );
      },
    });
  private activeHtmlTextEditSession: ActiveHtmlTextEditSession | null = null;
  private activeHtmlAttributeEditSession: ActiveHtmlAttributeEditSession | null = null;
  private htmlAttributeEditSessionSerial = 0;
  private htmlTextEditSessionSerial = 0;
  private htmlTextCanonicalTimer: ReturnType<typeof setTimeout> | null = null;
  private htmlTextHistoryTimer: ReturnType<typeof setTimeout> | null = null;
  private htmlTextProjectionTail: Promise<void> = Promise.resolve();
  private unregisterHtmlDraftCommitFlush: () => void = () => {};

  // ── CSS / override state ──
  variableValues = $state<Record<string, string>>({});
  editableStyles = $state<EditableStyles>(createDefaultEditableStyles());
  overrideRules = $state<Record<string, EditableStyles>>({});
  variableOverrides = $state<Record<string, string>>({});
  targetCssFile = $state<string>("styles.css");
  liveCssById = $state<Record<string, string>>({});
  inspectorLiveCssEpoch = $state(0);
  inspectorLiveCssIdentity = $state<InspectorLiveCssIdentity | null>(null);
  sessionProjectRoot = $state("");
  kernelProjectSessionId = $state("");
  projectSessionEpoch = $state(0);
  diskState = $state<DiskState>(createDiskState());
  notifications = $state<AppNotification[]>([]);
  dismissedNotificationIds = $state<Set<string>>(new Set());
  private saveOperationPromise: Promise<boolean> | null = null;
  private projectSessionReattachPromise: Promise<boolean> | null = null;
  projectTransitionFrontendLeaseActive = $state(false);
  kernelUndoRedoFrontendLeaseActive = $state(false);
  htmlMutationRevision = 0;

  // ── Global application status / project state ──
  globalStatusEvents = $state<GlobalStatusEvent[]>([]);
  globalStatusRevision = 0;
  globalStatusSequence = 0;
  globalStatusExpiryTimer: number | null = null;
  globalStatusKernelTail: Promise<void> = Promise.resolve();
  projectWorkspaceSnapshot = $state<ProjectWorkspaceSnapshot | null>(null);
  projectAuditSnapshot = $state<ProjectAuditSnapshot | null>(null);
  projectAuditLoading = $state(false);
  projectAuditError = $state("");
  auditWorkspaceView = $state<"overview" | "runtime">("overview");
  auditObservabilityFocusSerial = $state(0);
  private projectAuditRequestSerial = 0;
  private projectAuditRequestKey = "";
  private projectAuditRequest: Promise<ProjectAuditSnapshot | null> | null = null;
  designClassInventory = $state<DesignClassInventorySnapshot | null>(null);
  designClassInventoryLoading = $state(false);
  designClassInventoryError = $state("");
  private designClassInventorySerial = 0;
  private designClassInventoryRequestKey = "";
  private designClassInventoryRequest: Promise<DesignClassInventorySnapshot | null> | null = null;
  workbenchSnapshot = $state<WorkbenchSnapshot | null>(null);
  fileExplorerSnapshot = $state<FileExplorerSnapshot | null>(null);
  fileExplorerLoading = $state(false);
  fileExplorerError = $state("");
  private fileExplorerRequestSerial = 0;
  jsRefreshToken = $state(0);
  scannedProject = $state<ProjectScan | null>(null);
  startupFlow = $state<StartupFlowSnapshot>({
    schemaVersion: 1,
    revision: 1,
    stage: "idle",
    candidate: null,
    diagnostics: [],
  });
  startupCreationCatalog = $state<StartupCreationCatalog | null>(null);
  startupCreationPlan = $state<StartupCreationPlan | null>(null);
  startupSelectedOptionId = $state<string | null>(null);
  startupPending = $state(false);
  startupError = $state("");
  projectOpenRecoveryDecisionRequest = $state<ProjectOpenRecoveryDecisionRequest | null>(null);
  projectTransitionDecisionRequest = $state<ProjectTransitionDecisionRequest | null>(null);
  sourceGraph = $state<SourceGraph | null>(null);
  sourceGraphLoadSerial = 0;
  sourceGraphProjectionStatus = $state<WorkspaceDerivedProjectionStatus>("deferred");
  sourceGraphWorkspaceRevision = $state<number | null>(null);
  scssVariables = $state<ScssVariable[]>([]);
  projectStatus = $state("");
  cachebustAssets = $state(false);

  // ── Preview state ──
  previewSrc = $state("about:blank");
  previewRefreshSerial = 0;
  previewDomTreeSerial = 0;
  previewReloadSerial = 0;
  previewWorkspaceRevision = $state<string | null>(null);
  pendingCanvasProjection = $state<CanvasProjectionPlan | null>(null);
  activeCanvasIdentity = $state<CanvasProjectionIdentity | null>(null);
  editorNavigationSnapshot = $state<EditorNavigationSnapshot | null>(null);
  editorNavigationLoading = $state(false);
  editorNavigationError = $state("");
  editorEditScopeGrant = $state<EditScopeGrant | null>(null);
  editorEditScopeId = $state<string | null>(null);
  selectionSnapshot = $state<SelectionSnapshot | null>(null);
  inspectorSelectionSummary = $state<InspectorSelectionSummarySnapshot | null>(null);
  hoverSnapshot = $state<HoverSnapshot | null>(null);
  private editorNavigationRequestSerial = 0;
  private selectionCoordinatorRequestSerial = 0;
  private hoverCoordinatorRequestSerial = 0;
  activeCanvasUrl = $state("about:blank");
  interactivePreviewEnabled = $state(false);
  interactivePreviewDomNodes = $state<InteractivePreviewDomNode[]>([]);
  canvasProjectionConfirmation: CanvasProjectionConfirmation | null = null;
  activeScannedPath = $state<string | null>(null);
  activePreviewPath = $state("about:blank");
  browserPreviewRoute = $state("/");
  previewDocumentMarkup = $state<string | null>(null);
  pageSections = $state<PageSection[]>([]);
  latestPreviewMessageRevision = $state(0);
  controlledPreview = $state<ControlledPreviewState>(createControlledPreviewState());
  zolaValidationTimer: number | null = null;
  zolaValidationSerial = 0;
  templateWorkbenchPlan = $state<TemplateWorkbenchPlan | null>(null);
  templateWorkbenchPreferredPagePath = $state<string | null>(null);
  templateWorkbenchPreferredRoute = $state<string | null>(null);
  templateWorkbenchActive = $state(false);
  templateWorkbenchTarget = $state<string | null>(null);
  templateWorkbenchReturnPreviewPath = $state<string | null>(null);
  templateWorkbenchRequestSerial = 0;

  // ── UI state ──
  centerView = $state<CenterView>("preview");
  codeRevealTarget = $state<CodeRevealTarget>({ kind: "html" });
  cssSourceRevision = $state(0);
  codeSelectionRevealRequestId = $state(0);
  codeSelectionRevealConsumedId = 0;
  previewDevice = $state<"desktop" | "tablet" | "mobile">("desktop");
  previewZoom = $state(DEFAULT_PREVIEW_ZOOM);
  previewCanvasMode = $state<WorkbenchCanvasMode>("fit");
  previewCanvasPreset = $state<WorkbenchCanvasPreset>("desktop");
  previewWidthPx = $state(1_440);
  previewRulers = $state(true);
  uiTheme = $state<"dark" | "light">(initialUiTheme());
  uiLocale = $state("en-US");
  uiDirection = $state<"ltr" | "rtl">("ltr");
  uiAccent = $state("#1d7f6a");
  leftPaneWidth = $state(DEFAULT_LEFT_PANE_WIDTH);
  rightPaneWidth = $state(DEFAULT_RIGHT_PANE_WIDTH);
  terminalPaneHeight = $state(defaultTerminalPaneHeight);
  leftPaneCollapsed = $state(false);
  rightPaneCollapsed = $state(false);
  applicationSurface = $state<ApplicationSurface>("workbench");
  applicationSettings = $state<ApplicationSettingsSnapshot | null>(null);
  applicationSettingsLoading = $state(false);
  activeVersionPreview = $state<VersionPreviewReceipt | null>(null);
  activeInspectorTab = $state<InspectorTab>("html");
  activeResizeKind = $state<ResizeKind | null>(null);
  private workbenchController: WorkbenchProjectionController;
  private workbenchHydratedRuntimeSessionId = "";
  private applicationSettingsSaveTail: Promise<void> = Promise.resolve();
  private applicationSettingsRefreshTail: Promise<void> = Promise.resolve();
  systemPreferencesUnlisten: (() => void) | null = null;

  // ── Terminal state ──
  terminalPaneOpen = $state(false);
  terminalQuickTasks = defaultTerminalQuickTasks;
  terminalTabs = $state<TerminalTab[]>(initialTerminalTabs());
  activeTerminalTabId = $state("terminal-shell-1");
  terminalTabSerial = $state(1);

  // ── AI / MCP context bridge ──
  aiContextStatus = $state<AiContextStatus | null>(null);
  aiContextSaveTimer: number | null = null;
  aiContextUiRevision = Date.now();
  aiCoordinationSnapshot = $state<AiCoordinationSnapshot | null>(null);
  aiCoordinationTimer: number | null = null;
  aiCoordinationOperationInFlight = false;
  aiCoordinationHandledRequestId: string | null = null;
  aiCoordinationReconciliationLeaseId: string | null = null;
  aiCoordinationAutomaticReloadLeaseId: string | null = null;
  aiEditLeaseFrontendLockActive = $state(false);
  aiReconciliationRecoveryReloadAuthorized = false;

  // ── External disk change awareness ──
  externalDiskState = $state<ExternalDiskState>(createExternalDiskState());
  externalDiskTimer: number | null = null;
  externalDiskSuspended = $state(false);
  externalDiskCheckInFlight: ExternalDiskControllerHost["externalDiskCheckInFlight"] = null;
  externalDiskCheckGeneration = 0;
  previewStructuralWriteBoundaryActive = false;
  previewStructuralWriteBoundaryResumesMonitoring = false;

  // ── Internal non-reactive flags ──
  syncingSourceFromEditor = false;
  syncingSelectionFromCode = false;
  pendingRestoredSelectionTag: string | null = null;
  pendingRestoredSelectionTimer: number | null = null;
  previewSyncTimer: number | null = null;
  domTreeFetchTimer: number | null = null;
  activeResizeCleanup: (() => void) | null = null;
  appliedTerminalSessionRuntimeVersion = $state(0);
  nativeWindowClosePending = false;
  nativeWindowCloseInProgress = false;

  // ── Controllers ──
  codeEditorController = $state<CodeEditorController | null>(null);
  readonly editorRuntime: EditorRuntime;
  readonly previewRuntime: PreviewRuntime;
  canvasPatchPerformance = $state<CanvasPatchPerformanceSnapshot>({
    sampleCount: 0,
    receiptToCommitP50Ms: null,
    receiptToCommitP95Ms: null,
    receiptToCommitMaxMs: null,
    bridgeCommitP95Ms: null,
    budgetMs: 50,
    budgetMet: null,
  });
  readonly terminalController = new TerminalController();

  // ── Derived: source / language ──
  currentSourcePath = $derived(deriveCurrentSourcePath(this));
  sourceLanguage = $derived(deriveSourceLanguage(this));
  currentSourceCacheKey = $derived(deriveCurrentSourceCacheKey(this));
  currentHtmlRelativePath = $derived(deriveCurrentHtmlRelativePath(this));
  currentSourceRelativePath = $derived(deriveCurrentSourceRelativePath(this));
  htmlSourceNodes = $derived(deriveHtmlSourceNodes(this));

  // ── Derived: project ──
  scannedPages = $derived(deriveScannedFilesByRole(this, "page"));
  scannedTemplates = $derived(deriveScannedFilesByRole(this, "template"));
  scannedStyles = $derived(deriveScannedFilesByRole(this, "style"));
  scannedScripts = $derived(deriveScannedFilesByRole(this, "script"));
  scannedAssets = $derived(deriveScannedFilesByRole(this, "asset"));
  currentProjectPath = $derived(deriveCurrentProjectPath(this));
  activeTemplateFile = $derived(deriveActiveTemplateFile(this));
  activeRenderedPreviewPageFile = $derived(deriveActiveRenderedPreviewPageFile(this));
  activeRenderedTemplatePath = $derived(deriveActiveRenderedTemplatePath(this));

  // ── Derived: preview / source mode ──
  isActiveRenderedPreviewPage = $derived(deriveIsActiveRenderedPreviewPage(this));
  isActivePreviewHtmlSource = $derived(deriveIsActivePreviewHtmlSource(this));
  selectedSourceEditTarget = $derived(deriveSelectedSourceEditTarget(this));
  selectedTemplateSourceNode = $derived(deriveSelectedTemplateSourceNode(this));
  selectedEditorNavigationNode = $derived(deriveSelectedEditorNavigationNode(this));
  coordinatedElementSelection = $derived.by<CoordinatedElementSelection | null>(() => {
    const accepted = this.acceptedSelectionObservation;
    const semantic = this.selectionSnapshot;
    if (
      !accepted
      || !semantic
      || semantic.resolution !== "resolved"
      || (
        semantic.subject?.kind !== "htmlElement"
        && semantic.subject?.kind !== "runtimeElement"
      )
      || accepted.selectionRevision !== semantic.selectionRevision
      || !canvasIdentityEquals(accepted.canvasIdentity, semantic.canvasIdentity)
      || !canvasIdentityEquals(this.activeCanvasIdentity, semantic.canvasIdentity)
      || accepted.renderInstanceId !== semantic.anchor?.renderInstanceId
    ) return null;
    const sourceReference =
      semantic.provenance?.definition
      ?? semantic.provenance?.composition
      ?? null;
    return {
      snapshot: semantic,
      documentEpoch: accepted.documentEpoch,
      renderInstanceId: accepted.renderInstanceId,
      sourceNodeId: semantic.anchor?.sourceNodeId ?? null,
      sourceLocation: sourceReference?.range
        ? {
            file: sourceReference.file,
            line: sourceReference.range.line,
            column: sourceReference.range.column,
          }
        : semantic.anchor?.file && semantic.anchor.range
          ? {
              file: semantic.anchor.file,
              line: semantic.anchor.range.line,
              column: semantic.anchor.range.column,
            }
        : null,
      observation: accepted.observation,
    };
  });
  inspectorHtmlPhysicalFacts = $derived.by<InspectorHtmlPhysicalFacts | null>(() => {
    const coordinated = this.coordinatedElementSelection;
    const summary = this.inspectorSelectionSummary;
    if (
      !coordinated
      || summary?.state !== "resolved"
      || summary.selectionRevision !== coordinated.snapshot.selectionRevision
      || summary.renderInstanceId !== coordinated.renderInstanceId
    ) return null;
    const observation = coordinated.observation;
    return {
      selectionRevision: summary.selectionRevision,
      renderInstanceId: coordinated.renderInstanceId,
      rect: { ...observation.rect },
      hasChildElements: observation.hasChildElements,
      childElementCount: observation.childNodes.length,
      zolaImage: observation.zolaImage ? { ...observation.zolaImage } : null,
    };
  });
  inspectorBlockSelectionContext = $derived.by<BlockSelectionContext | null>(() => {
    const coordinated = this.coordinatedElementSelection;
    const summary = this.inspectorSelectionSummary;
    if (
      !coordinated
      || summary?.state !== "resolved"
      || summary.selectionRevision !== coordinated.snapshot.selectionRevision
      || summary.renderInstanceId !== coordinated.renderInstanceId
    ) return null;
    const physical = coordinated.observation.blockContext;
    const bounded = summary?.blockContext ?? null;
    if (
      !physical
      || !bounded
      || bounded.providerId !== physical.providerId
      || bounded.markerKind !== physical.markerKind
      || bounded.rootTag !== physical.rootTag
    ) return null;
    return {
      ...physical,
      rootSourceId: null,
      rootTemplateSourceId: null,
      rootSessionId: coordinated.snapshot.runtimeSessionId,
    };
  });
  selectionEpoch = $derived(this.selectionSnapshot?.selectionRevision ?? 0);
  activeCssSelector = $derived(
    this.selectionSnapshot?.focus.kind === "cssRule"
      || this.selectionSnapshot?.focus.kind === "cssProperty"
      ? this.selectionSnapshot.focus.selector
      : "",
  );
  selectedSemanticSourceLocation = $derived(deriveSelectedSemanticSourceLocation(this));
  workbenchSourceStatus = $derived(deriveWorkbenchSourceStatus(this));
  canEditHtmlStructure = $derived(deriveCanEditHtml(this));
  canEditHtml = $derived(deriveCanEditHtml(this));
  saveRequest = $state(0);
  refreshToken = $state(0);
  globalDirtyState = $derived(deriveAppDirtyState(this));
  sessionHasPending = $derived(deriveSessionHasPending(this));
  inspectorHasPending = $derived(this.globalDirtyState.dirty);
  saveHasPending = $derived(this.globalDirtyState.canSave);
  immediateDiskOperationBlockedReason = $derived(
    this.aiEditLeaseFrontendLockActive
      ? t("workbench-source-operations-ai-blocked")
      : this.externalDiskState.workspaceProjectionRecoveryRequired
      ? t("workbench-disk-operations-projection-blocked")
      : this.globalDirtyState.immediateDiskOperationBlockedReason,
  );
  canAddChildToSelectedElement = $derived(deriveCanAddChildToSelectedElement(this));
  canPreviewCurrentSource = $derived(deriveCanPreviewCurrentSource(this));
  htmlSourceMutationBlockedReason = $derived(deriveHtmlSourceMutationBlockedReason(this));

  // ── Derived: terminal ──
  activeTerminalTab = $derived(deriveActiveTerminalTab(this));

  // ── Constructor: reactive effects ────────────────────────────────────────

  constructor() {
    this.workbenchController = new WorkbenchProjectionController(() => this.workbenchProjectionHost());
    this.editorRuntime = createEditorRuntime(this.editorRuntimeHost());
    this.previewRuntime = createPreviewRuntime(this.previewRuntimeHost());
    this.unregisterHtmlDraftCommitFlush = registerEditFlushHandler(
      "html-draft-project-workspace",
      async () => {
        await this.finishActiveHtmlAttributeEditSession();
        await this.finishActiveHtmlTextEditSession();
      },
    );
    registerEditFlushHandler("motion-v2-project-workspace", async () => {
      await this.motionWorkspace.flush();
    });
    registerAppEffects(this);
  }

  editorRuntimeHost(): EditorRuntimeHost {
    return this;
  }

  previewRuntimeHost(): PreviewRuntimeHost {
    return this;
  }

  workbenchProjectionHost(): WorkbenchProjectionHost {
    return this;
  }

  refreshWorkbenchState() {
    return this.workbenchController.refresh();
  }

  async refreshFileExplorerSnapshot() {
    const workspace = this.projectWorkspaceSnapshot;
    if (
      !workspace
      || workspace.projectRoot !== this.sessionProjectRoot
      || workspace.runtimeSessionId !== this.kernelProjectSessionId
    ) {
      this.fileExplorerRequestSerial += 1;
      this.fileExplorerSnapshot = null;
      this.fileExplorerLoading = false;
      this.fileExplorerError = "";
      return null;
    }
    const serial = ++this.fileExplorerRequestSerial;
    const identity = {
      expectedProjectRoot: workspace.projectRoot,
      expectedSessionId: workspace.runtimeSessionId,
      expectedRevision: workspace.revision,
    };
    this.fileExplorerLoading = true;
    try {
      const snapshot = await readFileExplorerSnapshot(identity);
      if (
        serial !== this.fileExplorerRequestSerial
        || this.sessionProjectRoot !== identity.expectedProjectRoot
        || this.kernelProjectSessionId !== identity.expectedSessionId
        || this.projectWorkspaceSnapshot?.revision !== identity.expectedRevision
      ) return this.fileExplorerSnapshot;
      this.fileExplorerSnapshot = snapshot;
      this.fileExplorerError = "";
      if (this.workbenchSnapshot?.revision !== snapshot.workbenchRevision) {
        await this.refreshWorkbenchState();
        if (
          serial !== this.fileExplorerRequestSerial
          || this.sessionProjectRoot !== identity.expectedProjectRoot
          || this.kernelProjectSessionId !== identity.expectedSessionId
        ) return this.fileExplorerSnapshot;
      }
      return snapshot;
    } catch (error) {
      if (serial !== this.fileExplorerRequestSerial) return this.fileExplorerSnapshot;
      this.fileExplorerError = errorMessage(error);
      return null;
    } finally {
      if (serial === this.fileExplorerRequestSerial) this.fileExplorerLoading = false;
    }
  }

  async selectFileExplorerEntry(entryId: string) {
    const explorer = this.fileExplorerSnapshot;
    const workspace = this.projectWorkspaceSnapshot;
    if (
      !explorer
      || !workspace
      || explorer.projectRoot !== workspace.projectRoot
      || explorer.runtimeSessionId !== workspace.runtimeSessionId
      || explorer.workspaceRevision !== workspace.revision
    ) {
      await this.refreshFileExplorerSnapshot();
      return;
    }
    try {
      const receipt = await selectFileExplorerEntryInRust({
        identity: {
          expectedProjectRoot: explorer.projectRoot,
          expectedSessionId: explorer.runtimeSessionId,
          expectedRevision: explorer.workspaceRevision,
        },
        expectedWorkbenchRevision: explorer.workbenchRevision,
        entryId,
      });
      if (
        this.sessionProjectRoot !== receipt.projectRoot
        || this.kernelProjectSessionId !== receipt.runtimeSessionId
        || this.projectWorkspaceSnapshot?.revision !== receipt.workspaceRevision
      ) return;
      this.fileExplorerSnapshot = receipt.snapshot;
      this.workbenchSnapshot = receipt.workbench.snapshot;
      this.fileExplorerError = "";
      const selection = receipt.snapshot.selectedEntry;
      if (!selection || selection.kind !== "text") return;
      const file = this.scannedProject?.files.find(
        (candidate) => candidate.relativePath === selection.relativePath,
      );
      if (file) {
        await this.loadScannedProjectFile(file, { syncWorkbench: false });
      }
    } catch (error) {
      this.fileExplorerError = errorMessage(error);
      this.setGlobalStatus(this.fileExplorerError, "error");
    }
  }

  async planFileExplorerOperation(
    operation: FileExplorerOperationRequest,
  ): Promise<FileExplorerOperationPlan> {
    const explorer = this.fileExplorerSnapshot;
    const workspace = this.projectWorkspaceSnapshot;
    if (
      !explorer
      || !workspace
      || explorer.projectRoot !== workspace.projectRoot
      || explorer.runtimeSessionId !== workspace.runtimeSessionId
      || explorer.workspaceRevision !== workspace.revision
    ) {
      throw new Error(t("project-files-projection-unavailable"));
    }
    return planFileExplorerOperationInRust({
      identity: {
        expectedProjectRoot: explorer.projectRoot,
        expectedSessionId: explorer.runtimeSessionId,
        expectedRevision: explorer.workspaceRevision,
      },
      expectedWorkbenchRevision: explorer.workbenchRevision,
      operation,
    });
  }

  async commitFileExplorerOperation(
    plan: FileExplorerOperationPlan,
  ): Promise<FileExplorerCommitReceipt> {
    if (!plan.allowed || !plan.commitToken) {
      throw new Error(plan.diagnostic ?? t("project-files-plan-blocked"));
    }
    const workspace = this.projectWorkspaceSnapshot;
    if (
      !workspace
      || workspace.projectRoot !== plan.projectRoot
      || workspace.runtimeSessionId !== plan.runtimeSessionId
      || workspace.revision !== plan.workspaceRevision
      || workspace.diskGeneration !== plan.acceptedDiskGeneration
    ) {
      throw new Error(t("project-files-plan-stale"));
    }
    const receipt = await commitFileExplorerOperationInRust({
      identity: {
        expectedProjectRoot: plan.projectRoot,
        expectedSessionId: plan.runtimeSessionId,
        expectedRevision: plan.workspaceRevision,
      },
      expectedAcceptedDiskGeneration: plan.acceptedDiskGeneration,
      commitToken: plan.commitToken,
    });
    if (
      this.sessionProjectRoot !== receipt.projectRoot
      || this.kernelProjectSessionId !== receipt.runtimeSessionId
    ) {
      return receipt;
    }
    const selected = receipt.snapshot.selectedEntry;
    await settleProjectWorkspaceMutation(this, receipt.mutation, {
      preferredRelativePath: selected?.kind === "text"
        ? selected.relativePath
        : this.activeScannedPath,
      warningLabel: t("project-files-operation"),
    });
    if (
      this.sessionProjectRoot === receipt.projectRoot
      && this.kernelProjectSessionId === receipt.runtimeSessionId
      && this.projectWorkspaceSnapshot?.revision === receipt.mutation.workspace.revision
    ) {
      this.workbenchSnapshot = receipt.workbench.snapshot;
      this.fileExplorerSnapshot = receipt.snapshot;
      this.fileExplorerError = "";
    }
    this.setGlobalStatus(t("project-files-mutation-staged"), "unsaved");
    return receipt;
  }

  async restoreWorkbenchState() {
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    let snapshot = await this.workbenchController.refresh();
    if (
      !snapshot
      || !projectRoot
      || !runtimeSessionId
      || snapshot.projectRoot !== projectRoot
      || snapshot.runtimeSessionId !== runtimeSessionId
      || this.scannedProject?.root !== projectRoot
    ) return snapshot;

    if (snapshot.bottomPanel.activeView !== "terminal") {
      const receipt = await this.workbenchController.apply({
        kind: "set_bottom_panel",
        open: false,
        activeView: "terminal",
      });
      if (
        this.sessionProjectRoot !== projectRoot
        || this.kernelProjectSessionId !== runtimeSessionId
      ) return this.workbenchSnapshot;
      snapshot = receipt.snapshot;
    }

    this.projectWorkbenchCanvas(snapshot.canvasViewport);

    const group = snapshot.groups.find(
      (candidate) => candidate.groupId === snapshot.activeGroupId,
    );
    const document = group?.documents.find(
      (candidate) => candidate.documentId === group.activeDocumentId,
    );
    const file = document
      ? this.scannedProject.files.find(
          (candidate) => candidate.relativePath === document.relativePath,
        )
      : null;

    if (document && !file) {
      this.escalateGlobalStatus({
        id: "workbench.restore.missing-document",
        level: "warning",
        title: t("workbench-restored-document-missing"),
        message: document.relativePath,
      });
    } else if (file) {
      await this.loadScannedProjectFile(file, {
        strict: true,
        skipDraftFlush: true,
        activateTemplateWorkbench: snapshot.activeActivity === "editor"
          && !this.canvasSurfaceResumeRequired
          && this.hasMountedCanvasProjectionSurface(),
        syncWorkbench: false,
      });
      if (
        this.sessionProjectRoot !== projectRoot
        || this.kernelProjectSessionId !== runtimeSessionId
      ) return this.workbenchSnapshot;
      this.centerView = document?.surface === "code"
        ? "code"
        : document?.surface === "markdown"
          ? "markdown"
          : "preview";
      this.clearNotification("workbench.restore.missing-document");
    }

    this.workbenchHydratedRuntimeSessionId = runtimeSessionId;
    this.terminalPaneOpen = snapshot.bottomPanel.open
      && snapshot.bottomPanel.activeView === "terminal";
    this.projectWorkbenchActivity(snapshot.activeActivity, document?.surface ?? "visual");
    this.clearNotification("workbench.restore");
    if (!file && this.activeScannedPath) {
      const fallbackFile = this.scannedProject.files.find(
        (candidate) => candidate.relativePath === this.activeScannedPath,
      );
      if (fallbackFile) {
        try {
          const receipt = await this.workbenchController.openDocument(
            fallbackFile,
            this.centerView,
          );
          return receipt.snapshot;
        } catch (error) {
          this.workbenchHydratedRuntimeSessionId = "";
          throw error;
        }
      }
    }
    return snapshot;
  }

  applyWorkbenchIntent(intent: WorkbenchIntent) {
    return this.workbenchController.apply(intent);
  }

  private projectWorkbenchCanvas(viewport: WorkbenchCanvasViewportSnapshot) {
    this.previewCanvasMode = viewport.mode;
    this.previewCanvasPreset = viewport.preset;
    this.previewWidthPx = viewport.widthPx;
    this.previewZoom = viewport.zoomPercent;
    this.previewRulers = viewport.showRulers;
    this.previewDevice = viewport.mode === "fit"
      ? "desktop"
      : viewport.preset === "mobile"
      ? "mobile"
      : viewport.preset === "tablet"
        ? "tablet"
        : viewport.preset === "custom" && viewport.widthPx <= 600
          ? "mobile"
          : viewport.preset === "custom" && viewport.widthPx <= 1_100
            ? "tablet"
            : "desktop";
  }

  async setWorkbenchCanvasViewport(
    viewport: Partial<WorkbenchCanvasViewportSnapshot>,
  ) {
    const current = this.workbenchSnapshot?.canvasViewport ?? {
      mode: this.previewCanvasMode,
      preset: this.previewCanvasPreset,
      widthPx: this.previewWidthPx,
      zoomPercent: this.previewZoom,
      showRulers: this.previewRulers,
    } satisfies WorkbenchCanvasViewportSnapshot;
    const next: WorkbenchCanvasViewportSnapshot = {
      ...current,
      ...viewport,
      widthPx: Math.round(viewport.widthPx ?? current.widthPx),
      zoomPercent: Math.round(viewport.zoomPercent ?? current.zoomPercent),
    };
    try {
      const receipt = await this.workbenchController.apply({
        kind: "set_canvas_viewport",
        viewport: next,
      });
      this.projectWorkbenchCanvas(receipt.snapshot.canvasViewport);
      this.clearNotification("workbench.canvas-viewport");
      return receipt;
    } catch (error) {
      this.escalateGlobalStatus({
        id: "workbench.canvas-viewport",
        level: "warning",
        title: t("workbench-canvas-viewport-failed"),
        message: errorMessage(error),
      });
      return null;
    }
  }

  async setSynchronizedWorkbenchSplit(split: WorkbenchSplit) {
    try {
      if (split === "none") {
        const receipt = await this.workbenchController.apply({
          kind: "set_split",
          split,
        });
        if (this.activeScannedPath) {
          await this.workbenchController.setActiveDocumentSurface(
            this.activeScannedPath,
            this.centerView,
          );
        }
        this.clearNotification("workbench.split");
        return receipt;
      }

      if (!this.activeScannedPath) {
        throw new Error(t("workbench-split-document-required"));
      }
      const secondarySurface: WorkbenchSurface = this.sourceLanguage === "markdown"
        ? "markdown"
        : "code";
      const receipt = await this.workbenchController.apply({
        kind: "configure_synchronized_split",
        split,
        relativePath: this.activeScannedPath,
        secondarySurface,
      });
      this.clearNotification("workbench.split");
      return receipt;
    } catch (error) {
      this.escalateGlobalStatus({
        id: "workbench.split",
        level: "warning",
        title: t("workbench-split-update-failed"),
        message: errorMessage(error),
      });
      return null;
    }
  }

  async setWorkbenchSplitRatio(ratioBasisPoints: number) {
    try {
      const receipt = await this.workbenchController.apply({
        kind: "set_split_ratio",
        ratioBasisPoints: Math.round(ratioBasisPoints),
      });
      this.clearNotification("workbench.split-ratio");
      return receipt;
    } catch (error) {
      this.escalateGlobalStatus({
        id: "workbench.split-ratio",
        level: "warning",
        title: t("workbench-split-ratio-save-failed"),
        message: errorMessage(error),
      });
      return null;
    }
  }

  async setWorkbenchBottomPanel(
    open: boolean,
    activeView: WorkbenchBottomPanelView = "terminal",
  ) {
    try {
      const receipt = await this.workbenchController.apply({
        kind: "set_bottom_panel",
        open,
        activeView,
      });
      this.terminalPaneOpen = receipt.snapshot.bottomPanel.open
        && receipt.snapshot.bottomPanel.activeView === "terminal";
      this.clearNotification("workbench.bottom-panel");
      return true;
    } catch (error) {
      this.escalateGlobalStatus({
        id: "workbench.bottom-panel",
        level: "warning",
        title: t("workbench-bottom-panel-update-failed"),
        message: errorMessage(error),
      });
      return false;
    }
  }

  toggleTerminalPane() {
    return this.setWorkbenchBottomPanel(!this.terminalPaneOpen, "terminal");
  }

  async setWorkbenchActivity(activity: WorkbenchActivity) {
    const receipt = await this.workbenchController.apply({
      kind: "set_activity",
      activity,
    });
    const group = receipt.snapshot.groups.find(
      (candidate) => candidate.groupId === receipt.snapshot.activeGroupId,
    );
    const document = group?.documents.find(
      (candidate) => candidate.documentId === group.activeDocumentId,
    );
    if (activity === "editor" && document && this.scannedProject) {
      const file = this.scannedProject.files.find(
        (candidate) => candidate.relativePath === document.relativePath,
      );
      if (file && this.activeScannedPath !== file.relativePath) {
        await this.loadScannedProjectFile(file, {
          strict: true,
          syncWorkbench: false,
        });
      }
    }
    this.projectWorkbenchActivity(activity, document?.surface ?? "visual");
    return receipt;
  }

  async openAuditWorkspace(
    view: "overview" | "runtime",
    focusObservability = false,
  ) {
    this.auditWorkspaceView = view;
    if (focusObservability) this.auditObservabilityFocusSerial += 1;
    if (this.terminalPaneOpen) {
      await this.setWorkbenchBottomPanel(false, "terminal");
    }
    return this.setWorkbenchActivity("audit");
  }

  private projectWorkbenchActivity(
    activity: WorkbenchActivity,
    surface: "visual" | "code" | "markdown",
  ) {
    if (activity === "editor") {
      this.centerView = surface === "code"
        ? "code"
        : surface === "markdown"
          ? "markdown"
          : "preview";
    } else if (activity === "audit") {
      this.centerView = "kernel";
    }
  }

  // ── Lifecycle (called from onMount) ──────────────────────────────────────

  async initFromStorage(storage: Storage) {
    await initFromStorageFromController(this, storage);
  }

  destroy() {
    this.unregisterHtmlDraftCommitFlush();
    this.cancelActiveHtmlAttributeEditSession();
    this.cancelActiveHtmlTextEditSession();
    this.htmlTextDraftCommitQueue.reset();
    destroyAppFromController(this);
  }

  aiContextControllerHost(): AiContextControllerHost {
    return this;
  }

  externalDiskControllerHost(): ExternalDiskControllerHost {
    return this;
  }

  markEditorMutation() {
    this.editorMutationEpoch += 1;
  }

  markProjectWorkspaceMutation() {
    this.projectWorkspaceMutationEpoch += 1;
  }

  quiesceExternalReconcileInteractions() {
    dispatchExternalReconcileInteractionBarrier();
  }

  async waitForExternalReconcileInteractionLock() {
    await tick();
  }

  async establishExternalDiskBaseline() {
    await establishExternalDiskBaselineFromController(this.externalDiskControllerHost());
  }

  acceptProjectWorkspaceSaveBaseline(
    acceptedManifest: ProjectDiskManifest,
    acceptedDiskGeneration: number,
  ) {
    acceptProjectWorkspaceSaveBaselineFromController(
      this.externalDiskControllerHost(),
      acceptedManifest,
      acceptedDiskGeneration,
    );
  }

  /**
   * Structural mutations share one serialized ProjectWorkspace lane. Drain
   * the external monitor before entering it so external reconcile cannot
   * replace the accepted baseline while the mutation is being projected.
   */
  async beginPreviewStructuralWriteBoundary() {
    if (this.previewStructuralWriteBoundaryActive) {
      throw new Error(t("workbench-structural-boundary-busy"));
    }
    const resumesMonitoring = !this.externalDiskSuspended;
    try {
      await suspendAndDrainExternalDiskMonitoringFromController(
        this.externalDiskControllerHost(),
      );
      if (
        this.externalDiskState.checking
        || this.externalDiskState.reconciling
        || this.externalDiskState.changed
        || this.externalDiskState.blockedByDirtySession
        || this.externalDiskState.workspaceProjectionRecoveryRequired
      ) {
        throw new Error(
          t("workbench-structural-boundary-disk-dirty"),
        );
      }
      this.previewStructuralWriteBoundaryResumesMonitoring = resumesMonitoring;
      this.previewStructuralWriteBoundaryActive = true;
    } catch (error) {
      if (resumesMonitoring) {
        resumeExternalDiskMonitoringAfterSaveFromController(
          this.externalDiskControllerHost(),
        );
      }
      throw error;
    }
  }

  endPreviewStructuralWriteBoundary() {
    if (!this.previewStructuralWriteBoundaryActive) return;
    const resumesMonitoring = this.previewStructuralWriteBoundaryResumesMonitoring;
    this.previewStructuralWriteBoundaryActive = false;
    this.previewStructuralWriteBoundaryResumesMonitoring = false;
    if (resumesMonitoring) {
      resumeExternalDiskMonitoringAfterSaveFromController(
        this.externalDiskControllerHost(),
      );
    }
  }

  startExternalDiskPolling() {
    startExternalDiskPollingFromController(this.externalDiskControllerHost());
  }

  resetExternalDiskState() {
    resetExternalDiskStateFromController(this.externalDiskControllerHost());
  }

  async invalidateExternalReconcileForProjectTransition() {
    invalidateExternalReconcileForProjectTransitionFromController(this.externalDiskControllerHost());
    await tick();
  }

  resumeExternalMonitoringAfterFailedTransition() {
    resumeExternalMonitoringAfterFailedTransitionFromController(this.externalDiskControllerHost());
  }

  markWorkspaceProjectionRecoveryRequired(message: string) {
    markWorkspaceProjectionRecoveryRequiredFromController(this.externalDiskControllerHost(), message);
  }

  setGlobalStatus(
    text: string,
    kind: GlobalStatusKind,
    options: GlobalStatusPublishOptions = {},
  ) {
    setGlobalStatusFromAppSessionController(this, text, kind, options);
  }

  refreshGlobalStatusFromKernel() {
    return refreshGlobalStatusFromKernelFromController(this);
  }

  escalateGlobalStatus(notification: GlobalStatusEscalationRequest) {
    escalateGlobalStatusFromController(this, notification);
  }

  clearNotification(id: string) {
    clearNotificationFromController(this, id);
  }

  dismissNotification(id: string) {
    dismissNotificationFromController(this, id);
  }

  async handleNotificationAction(notification: AppNotification, actionId: string) {
    try {
      await handleNotificationActionFromController(this, notification, actionId);
    } catch (error) {
      // Notification actions are launched from a void UI event. Terminate every
      // rejected command here so recovery failures remain visible instead of
      // becoming an unhandled promise that looks like a dead button.
      this.setGlobalStatus(
        t("workbench-notification-action-failed", {
          action: notification.actionLabel ?? actionId,
          message: errorMessage(error),
        }),
        "error",
      );
    }
  }

  setSessionProjectRoot(projectRoot = "") {
    if (this.sessionProjectRoot !== projectRoot) {
      this.projectAuditRequestSerial += 1;
      this.projectAuditRequestKey = "";
      this.projectAuditRequest = null;
      this.projectAuditSnapshot = null;
      this.projectAuditLoading = false;
      this.projectAuditError = "";
      this.designClassInventorySerial += 1;
      this.designClassInventoryRequestKey = "";
      this.designClassInventoryRequest = null;
      this.designClassInventory = null;
      this.designClassInventoryLoading = false;
      this.designClassInventoryError = "";
    }
    this.sessionProjectRoot = projectRoot;
  }

  async refreshProjectAudit(force = false): Promise<ProjectAuditSnapshot | null> {
    const projectRoot = this.sessionProjectRoot.trim();
    const runtimeSessionId = this.kernelProjectSessionId.trim();
    const workspaceRevision = this.projectWorkspaceSnapshot?.revision ?? null;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === null) {
      this.projectAuditSnapshot = null;
      this.projectAuditError = "";
      return null;
    }

    const requestKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}`;
    const current = this.projectAuditSnapshot;
    if (
      !force
      && current?.projectRoot === projectRoot
      && current.runtimeSessionId === runtimeSessionId
      && current.workspaceRevision === workspaceRevision
    ) {
      return current;
    }
    if (!force && this.projectAuditRequest && this.projectAuditRequestKey === requestKey) {
      return await this.projectAuditRequest;
    }

    const serial = ++this.projectAuditRequestSerial;
    this.projectAuditRequestKey = requestKey;
    this.projectAuditLoading = true;
    this.projectAuditError = "";
    const request = (async () => {
      try {
        const snapshot = await readProjectAudit();
        if (
          serial !== this.projectAuditRequestSerial
          || this.sessionProjectRoot !== projectRoot
          || this.kernelProjectSessionId !== runtimeSessionId
          || this.projectWorkspaceSnapshot?.revision !== workspaceRevision
        ) return null;
        if (
          snapshot.projectRoot !== projectRoot
          || snapshot.runtimeSessionId !== runtimeSessionId
          || snapshot.workspaceRevision !== workspaceRevision
        ) {
          throw new Error(t("workbench-audit-session-mismatch"));
        }
        this.projectAuditSnapshot = snapshot;
        return snapshot;
      } catch (error) {
        if (serial !== this.projectAuditRequestSerial) return null;
        this.projectAuditError = errorMessage(error);
        return null;
      } finally {
        if (serial === this.projectAuditRequestSerial) {
          this.projectAuditLoading = false;
          this.projectAuditRequest = null;
          this.projectAuditRequestKey = "";
        }
      }
    })();
    this.projectAuditRequest = request;
    return await request;
  }

  async refreshDesignClassInventory(
    force = false,
  ): Promise<DesignClassInventorySnapshot | null> {
    const projectRoot = this.sessionProjectRoot.trim();
    const runtimeSessionId = this.kernelProjectSessionId.trim();
    const workspaceRevision = this.projectWorkspaceSnapshot?.revision ?? null;
    if (!projectRoot || !runtimeSessionId || workspaceRevision === null) {
      this.designClassInventory = null;
      this.designClassInventoryError = "";
      return null;
    }
    const requestKey = `${projectRoot}\u0000${runtimeSessionId}\u0000${workspaceRevision}`;
    const current = this.designClassInventory;
    if (
      !force
      && current?.projectRoot === projectRoot
      && current.runtimeSessionId === runtimeSessionId
      && current.workspaceRevision === workspaceRevision
    ) return current;
    if (
      !force
      && this.designClassInventoryRequest
      && this.designClassInventoryRequestKey === requestKey
    ) return await this.designClassInventoryRequest;

    const serial = ++this.designClassInventorySerial;
    this.designClassInventoryRequestKey = requestKey;
    this.designClassInventoryLoading = true;
    this.designClassInventoryError = "";
    const request = (async () => {
      try {
        const snapshot = await readDesignClassInventory();
        if (
          serial !== this.designClassInventorySerial
          || this.sessionProjectRoot !== projectRoot
          || this.kernelProjectSessionId !== runtimeSessionId
          || this.projectWorkspaceSnapshot?.revision !== workspaceRevision
        ) return null;
        if (
          snapshot.projectRoot !== projectRoot
          || snapshot.runtimeSessionId !== runtimeSessionId
          || snapshot.workspaceRevision !== workspaceRevision
        ) throw new Error(t("workbench-class-inventory-revision-mismatch"));
        this.designClassInventory = snapshot;
        return snapshot;
      } catch (error) {
        if (serial !== this.designClassInventorySerial) return null;
        this.designClassInventoryError = errorMessage(error);
        return null;
      } finally {
        if (serial === this.designClassInventorySerial) {
          this.designClassInventoryLoading = false;
          this.designClassInventoryRequest = null;
          this.designClassInventoryRequestKey = "";
        }
      }
    })();
    this.designClassInventoryRequest = request;
    return await request;
  }

  statusControllerHost(): StatusControllerHost {
    return this;
  }

  get currentGlobalStatus() {
    return currentGlobalStatusFromController(this.statusControllerHost());
  }

  setPreviewZoom(value: number) {
    setPreviewZoomFromController(this.uiControllerHost(), value);
  }

  resetPreviewZoom() {
    resetPreviewZoomFromController(this.uiControllerHost());
    void this.setWorkbenchCanvasViewport({ zoomPercent: this.previewZoom });
  }

  commitPreviewZoom(value = this.previewZoom) {
    setPreviewZoomFromController(this.uiControllerHost(), value);
    return this.setWorkbenchCanvasViewport({ zoomPercent: this.previewZoom });
  }

  setInspectorPending(
    area: InspectorPendingArea,
    pending: boolean,
    source: InspectorPendingSource = "session",
  ) {
    setInspectorPendingFromController(this, area, pending, source);
  }

  resetInspectorPendingSources() {
    this.inspectorPendingSources = createInspectorPendingSourceRegistry();
  }

  async flushInteractiveEditorDrafts(reason: EditFlushReason = "manual") {
    await flushWorkspaceMutationInputs(reason);
  }

  setHtmlPending(area: HtmlPendingArea, pending: boolean) {
    setHtmlPendingFromController(this, area, pending);
  }

  clearHtmlPending() {
    clearHtmlPendingFromController(this);
  }

  cancelPendingHtmlMutations() {
    this.cancelActiveHtmlAttributeEditSession();
    this.cancelActiveHtmlTextEditSession();
    this.htmlTextDraftCommitQueue.reset();
    this.htmlMutationRevision += 1;
  }

  // ── Project management ────────────────────────────────────────────────────

  async reattachCurrentProjectSession(): Promise<boolean> {
    if (this.scannedProject) return true;
    if (this.projectSessionReattachPromise) return await this.projectSessionReattachPromise;
    const operation = reattachCurrentProjectSessionFromController(this.projectControllerHost());
    this.projectSessionReattachPromise = operation;
    try {
      return await operation;
    } finally {
      if (this.projectSessionReattachPromise === operation) {
        this.projectSessionReattachPromise = null;
      }
    }
  }

  async openProjectFolder() {
    await openProjectFolderFromController(this.projectControllerHost());
  }

  async refreshStartupFlow() {
    this.startupFlow = await readStartupFlow();
    return this.startupFlow;
  }

  selectStartupCreationOption(optionId: string) {
    selectStartupCreationOptionFromController(this.projectControllerHost(), optionId);
  }

  async planStartupProject() {
    await planStartupProjectFromController(this.projectControllerHost());
  }

  cancelStartupCreationPlan() {
    cancelStartupCreationPlanFromController(this.projectControllerHost());
  }

  async applyStartupProject() {
    await applyStartupProjectFromController(this.projectControllerHost());
  }

  cancelProjectOpenRecoveryDecision(requestId: string) {
    cancelProjectOpenRecoveryDecisionFromController(this.projectControllerHost(), requestId);
  }

  async confirmProjectOpenRecoveryAbandonment(requestId: string) {
    await continueProjectOpenWithRecoveryAbandonmentFromController(
      this.projectControllerHost(),
      requestId,
    );
  }

  cancelProjectTransitionOperatorDecision(requestId: string) {
    if (this.projectTransitionDecisionRequest?.id !== requestId) return;
    this.projectTransitionDecisionRequest = null;
    cancelPendingNativeWindowClose(this);
    this.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
    this.setGlobalStatus(t("workbench-transition-cancelled"), "idle");
  }

  async confirmProjectTransitionOperatorDecision(requestId: string, diagnostic: string) {
    await continueProjectTransitionWithOperatorDecisionFromController(
      this.projectControllerHost(),
      requestId,
      diagnostic,
    );
    if (!this.scannedProject) {
      this.clearClosedProjectRuntimeState();
      await closeNativeWindowIfProjectClosed(this);
    }
  }

  async closeCurrentProject(detachedProjectRoot: string | null = null) {
    const closed = await closeCurrentProjectFromController(
      this.projectControllerHost(),
      { detachedProjectRoot },
    );
    if (closed && !this.scannedProject) {
      this.clearClosedProjectRuntimeState();
      await this.refreshStartupFlow();
      await closeNativeWindowIfProjectClosed(this);
    }
    return closed;
  }

  async openCurrentProjectInBrowser(route: string | null = null) {
    await openCurrentProjectInBrowserFromController(
      this.projectControllerHost(),
      undefined,
      { route: route?.trim() || this.browserPreviewRoute },
    );
  }

  clearClosedProjectRuntimeState() {
    this.applicationSurface = "workbench";
    this.terminalController.destroyAll();
    this.terminalTabs = initialTerminalTabs();
    this.activeTerminalTabId = "terminal-shell-1";
    this.terminalTabSerial = 1;
    this.terminalPaneOpen = false;
    this.auditWorkspaceView = "overview";
    this.auditObservabilityFocusSerial = 0;
    this.startupCreationCatalog = null;
    this.startupCreationPlan = null;
    this.startupSelectedOptionId = null;
    this.startupPending = false;
    this.startupError = "";
  }

  resetProjectScopedState() {
    resetProjectScopedStateFromController(this.projectControllerHost());
    this.workbenchController.reset();
    this.workbenchHydratedRuntimeSessionId = "";
  }

  async rescanCurrentProject(
    preferredRelativePath: string | null = this.activeScannedPath,
    options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
  ) {
    await rescanCurrentProjectFromController(
      this.projectControllerHost(),
      preferredRelativePath,
      options,
    );
  }

  async reconcileWorkspaceDerivedState(
    options: import("$lib/state/project-controller").ReconcileWorkspaceDerivedStateOptions,
  ) {
    return await reconcileWorkspaceDerivedStateFromController(
      this.projectControllerHost(),
      options,
    );
  }

  async rescanCurrentProjectWithinKernelUndoRedoLease(
    lease: KernelUndoRedoProjectionLease,
    preferredRelativePath: string | null = this.activeScannedPath,
    options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
  ) {
    await rescanCurrentProjectWithinKernelUndoRedoLeaseFromController(
      this.projectControllerHost(),
      lease,
      preferredRelativePath,
      options,
    );
  }

  async discardSessionAndReloadFromDisk(preferredRelativePath: string | null = this.activeScannedPath) {
    return await discardSessionAndReloadFromDiskFromController(
      this.projectControllerHost(),
      preferredRelativePath,
    );
  }

  async refreshCurrentSession() {
    await refreshCurrentSessionFromController(this);
  }

  async createContentPageFromInput(input: {
    title: string;
    slug?: string | null;
    section?: string | null;
  }) {
    return await createContentPageFromInputFromController(this.projectControllerHost(), input);
  }

  // ── File loading ──────────────────────────────────────────────────────────

  async loadScannedProjectFile(
    file: ProjectFile,
    options: {
      strict?: boolean;
      skipDraftFlush?: boolean;
      deferPreviewRefresh?: boolean;
      activateTemplateWorkbench?: boolean;
      preferredTemplatePagePath?: string | null;
      preferredTemplateRoute?: string | null;
      syncWorkbench?: boolean;
    } = {},
  ) {
    const workbenchSessionId = this.kernelProjectSessionId;
    const shouldSyncWorkbench = options.syncWorkbench !== false
      && this.workbenchHydratedRuntimeSessionId === workbenchSessionId;
    await loadScannedProjectFileFromController(this.projectControllerHost(), file, options);
    if (
      shouldSyncWorkbench
      && this.kernelProjectSessionId === workbenchSessionId
      && this.activeScannedPath === file.relativePath
      && this.sessionProjectRoot
      && this.kernelProjectSessionId
    ) {
      try {
        await this.workbenchController.openDocument(file, this.centerView);
        this.clearNotification("workbench.document-sync");
        if (this.activeCanvasIdentity) {
          await this.refreshEditorNavigationSnapshot();
        }
      } catch (error) {
        this.escalateGlobalStatus({
          id: "workbench.document-sync",
          level: "warning",
          title: t("workbench-document-sync-failed"),
          message: errorMessage(error),
        });
      }
    }
  }

  async updateTemplateWorkbenchContext(
    project: ProjectScan,
    templateFile: ProjectFile,
    preferredPagePath: string | null = null,
    options: {
      deferPreviewRefresh?: boolean;
      minimumWorkspaceRevision?: number;
      preferredRoute?: string | null;
      strict?: boolean;
    } = {},
  ) {
    return await updateTemplateWorkbenchContextFromController(
      this.projectControllerHost(),
      project,
      templateFile,
      preferredPagePath,
      options,
    );
  }

  async reprojectActiveTemplateWorkbench(minimumWorkspaceRevision: number) {
    if (!this.templateWorkbenchActive) return false;
    const project = this.scannedProject;
    const target = this.templateWorkbenchTarget;
    const templateFile = project && target
      ? project.files.find(
        (file) => file.relativePath === target && file.role === "template",
      ) ?? null
      : null;
    if (!project || !templateFile || this.activeScannedPath !== target) {
      throw new Error(
        t("workbench-template-context-missing"),
      );
    }
    await this.updateTemplateWorkbenchContext(
      project,
      templateFile,
      this.templateWorkbenchPreferredPagePath,
      {
        minimumWorkspaceRevision,
        preferredRoute: this.templateWorkbenchPreferredRoute,
        strict: true,
      },
    );
    return this.templateWorkbenchActive
      && this.activeCanvasIdentity?.projectRoot === this.sessionProjectRoot
      && this.activeCanvasIdentity?.runtimeSessionId === this.kernelProjectSessionId
      && this.activeCanvasIdentity?.workspaceRevision === minimumWorkspaceRevision;
  }

  async exitTemplateWorkbench(options: { deferPreviewRefresh?: boolean } = {}) {
    await exitTemplateWorkbenchFromController(this.projectControllerHost(), options);
  }

  projectControllerHost(): ProjectControllerHost {
    return this;
  }

  // ── Preview ───────────────────────────────────────────────────────────────

  previewUrlForScannedFile(file: ProjectFile) {
    return previewUrlForScannedFileFromController(this, file);
  }

  previewReloadUrl(url: string) {
    return previewReloadUrlFromController(this.previewControllerHost(), url);
  }

  cancelPreviewSync() {
    cancelPreviewSyncFromController(this.previewControllerHost());
  }

  getPreviewDocument(): Document | undefined {
    return getPreviewDocumentFromController(this.previewControllerHost());
  }

  postPreviewMessage(payload: Record<string, unknown>) {
    postPreviewMessageFromController(this.previewControllerHost(), payload);
  }

  sendPreviewOperation(payload: Record<string, unknown> & { type: string }) {
    return sendPreviewOperationFromController(this.previewControllerHost(), payload);
  }

  async applyCanvasPatchToPreview(patch: CanvasPatch) {
    const receipt = await this.previewRuntime.applyCanvasPatch(patch);
    this.canvasPatchPerformance = this.previewRuntime.canvasPatchPerformance();
    return receipt;
  }

  async rollbackCanvasPatchInPreview(patch: CanvasPatch) {
    const identity = this.pendingCanvasProjection?.identity ?? null;
    const startedAt = performance.now();
    const receipt = await this.previewRuntime.rollbackCanvasPatch(patch);
    if (identity?.workspaceRevision === patch.workspaceRevision) {
      void this.recordCanvasProjectionRuntimeEvent(
        "canvas_patch_rolled_back",
        identity,
        Math.max(0, performance.now() - startedAt),
        t("workbench-canvas-patch-withdrawn", { patch: patch.patchId }),
      );
    }
    return receipt;
  }

  async refreshRenderedPreviewDocument(lease?: PreviewRefreshLease) {
    return await refreshRenderedPreviewDocumentFromController(this.previewControllerHost(), lease);
  }

  prepareCanvasProjectionNavigation(plan: CanvasProjectionPlan) {
    return prepareCanvasProjectionNavigationFromController(this.previewControllerHost(), plan);
  }

  hasMountedCanvasProjectionSurface() {
    return hasMountedCanvasProjectionSurfaceFromController(this.previewControllerHost());
  }

  mountCanvasProjectionSurface(frame: HTMLIFrameElement) {
    const replacedSurface = Boolean(
      this.canvasSurfaceElement && this.canvasSurfaceElement !== frame,
    );
    mountCanvasProjectionSurfaceFromController(this.previewControllerHost(), frame);
    if (replacedSurface) this.deferWorkspacePreviewProjection();
  }

  unmountCanvasProjectionSurface(frame: HTMLIFrameElement) {
    if (!unmountCanvasProjectionSurfaceFromController(this.previewControllerHost(), frame)) return;
    this.deferWorkspacePreviewProjection();
  }

  deferWorkspacePreviewProjection() {
    if (this.scannedProject) this.canvasSurfaceResumeRequired = true;
  }

  markCanvasProjectionSurfaceCurrent() {
    this.canvasSurfaceResumeRequired = false;
  }

  onCanvasProjectionSurfaceLoaded(frame: HTMLIFrameElement) {
    if (
      this.canvasSurfaceElement !== frame
      || !this.canvasSurfaceResumeRequired
      || this.canvasProjectionConfirmation
      || this.pendingCanvasProjection
      || !this.canProjectWorkspacePreview()
      || this.activeVersionPreview
    ) return;
    if (this.canvasSurfaceResumePromise) return;

    const surfaceGeneration = this.canvasSurfaceGeneration;
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    const resume = (async () => {
      const outcome = await projectLatestProjectWorkspacePreview(this, {
        reason: "session-refresh",
      });
      if (
        this.canvasSurfaceGeneration !== surfaceGeneration
        || this.canvasSurfaceElement !== frame
        || this.sessionProjectRoot !== projectRoot
        || this.kernelProjectSessionId !== runtimeSessionId
      ) return;
      if (outcome.status !== "published" && outcome.status !== "already_current") return;
      this.markCanvasProjectionSurfaceCurrent();
      const activeFile = this.scannedProject?.files.find(
        (file) => file.relativePath === this.activeScannedPath,
      );
      if (activeFile?.role === "template" && !this.templateWorkbenchActive) {
        await this.loadScannedProjectFile(activeFile, {
          strict: true,
          skipDraftFlush: true,
          activateTemplateWorkbench: true,
          syncWorkbench: false,
        });
      }
    })()
      .catch((error) => {
        if (
          this.canvasSurfaceGeneration === surfaceGeneration
          && this.canvasSurfaceElement === frame
          && this.sessionProjectRoot === projectRoot
          && this.kernelProjectSessionId === runtimeSessionId
        ) {
          this.setGlobalStatus(
            t("workbench-preview-resume-failed", { message: errorMessage(error) }),
            "error",
          );
        }
      })
      .finally(() => {
        if (this.canvasSurfaceResumePromise === resume) {
          this.canvasSurfaceResumePromise = null;
          const currentSurface = this.canvasSurfaceElement;
          if (
            currentSurface
            && this.canvasSurfaceResumeRequired
            && this.canvasSurfaceGeneration !== surfaceGeneration
          ) {
            queueMicrotask(() => this.onCanvasProjectionSurfaceLoaded(currentSurface));
          }
        }
      });
    this.canvasSurfaceResumePromise = resume;
  }

  async reconcileTemplateWorkbenchPreviewDocument(
    previewUrl: string,
    plan: CanvasProjectionPlan,
  ) {
    return await reconcileTemplateWorkbenchPreviewDocumentFromController(
      this.previewControllerHost(),
      previewUrl,
      plan,
    );
  }

  async reloadPreview(lease?: PreviewRefreshLease) {
    return await reloadPreviewFromController(this.previewControllerHost(), lease);
  }

  async requestPreviewRefresh(reason: PreviewRefreshReason = "manual") {
    const refreshed = await requestControlledPreviewRefreshFromController(
      this.controlledPreviewControllerHost(),
      reason,
    );
    if (refreshed && this.scannedProject?.previewWarning) {
      this.scannedProject = { ...this.scannedProject, previewWarning: null };
      this.clearNotification("project.preview.warning");
    }
    return refreshed;
  }

  async requestWorkspaceProjectionPreviewRefresh(reason: PreviewRefreshReason) {
    const refreshed = await requestControlledPreviewRefreshFromController(
      this.controlledPreviewControllerHost(),
      reason,
      { publishFailure: false },
    );
    if (refreshed) {
      if (this.scannedProject?.previewWarning) {
        this.scannedProject = { ...this.scannedProject, previewWarning: null };
        this.clearNotification("project.preview.warning");
      }
      return true;
    }
    throw new Error(
      this.projectStatus || t("workbench-preview-generation-unconfirmed"),
    );
  }

  canProjectWorkspacePreview() {
    return Boolean(
      this.hasMountedCanvasProjectionSurface()
      && this.scannedProject?.previewBaseUrl
      && this.previewSrc
      && this.previewSrc !== "about:blank"
      && this.previewDocumentMarkup === null,
    );
  }

  markPreviewLive(message?: string) {
    this.controlledPreview = markPreviewLive(this.controlledPreview, message);
  }

  markPreviewSavedToDisk(message?: string) {
    this.controlledPreview = markPreviewSaved(this.controlledPreview, message);
  }

  resetControlledPreviewState() {
    this.previewRuntime.reset();
    this.canvasPatchPerformance = this.previewRuntime.canvasPatchPerformance();
    cancelCanvasProjectionConfirmation(this.previewControllerHost());
    this.canvasSurfaceResumeRequired = false;
    this.canvasSurfaceResumePromise = null;
    this.pendingCanvasProjection = null;
    this.activeCanvasIdentity = null;
    this.acceptedSelectionObservation = null;
    this.inspectorSelectionSummary = null;
    this.hoverSnapshot = null;
    this.editorNavigationRequestSerial += 1;
    this.editorNavigationSnapshot = null;
    this.editorNavigationLoading = false;
    this.editorNavigationError = "";
    this.editorEditScopeGrant = null;
    this.editorEditScopeId = null;
    this.activeCanvasUrl = "about:blank";
    this.interactivePreviewEnabled = false;
    this.motionWorkspace.previewMode = "design";
    this.interactivePreviewDomNodes = [];
    invalidatePreviewRefreshLease(this.previewControllerHost());
    invalidatePreviewDomTreeProjection(this.previewControllerHost());
    this.sourceGraphLoadSerial += 1;
    if (this.zolaValidationTimer !== null && typeof window !== "undefined") {
      window.clearTimeout(this.zolaValidationTimer);
    }
    this.zolaValidationTimer = null;
    this.zolaValidationSerial += 1;
    this.controlledPreview = createControlledPreviewState();
  }

  get interactivePreviewUrl() {
    if (!this.activeCanvasIdentity) return "";
    const sourceUrl = this.pendingCanvasProjection
      ? this.activeCanvasUrl
      : this.previewSrc;
    return buildInteractivePreviewUrl(sourceUrl, this.activeCanvasIdentity);
  }

  setInteractivePreviewEnabled(enabled: boolean) {
    this.interactivePreviewEnabled = Boolean(
      enabled
      && this.activeCanvasIdentity
      && this.previewSrc
      && this.previewSrc !== "about:blank",
    );
    if (!this.interactivePreviewEnabled) this.interactivePreviewDomNodes = [];
  }

  setPreviewExecutionMode(mode: MotionPreviewMode) {
    this.motionWorkspace.previewMode = mode;
    if (mode !== "motion") this.motionWorkspace.previewStatus = null;
    this.setInteractivePreviewEnabled(mode !== "design");
    if (mode !== "design" && !this.interactivePreviewEnabled) {
      this.motionWorkspace.previewMode = "design";
      this.motionWorkspace.previewStatus = null;
    }
  }

  acceptInteractivePreviewDomSnapshot(nodes: InteractivePreviewDomNode[]) {
    if (!this.interactivePreviewEnabled || !this.activeCanvasIdentity) return;
    this.interactivePreviewDomNodes = nodes.slice(0, 5000);
  }

  async recordInteractivePreviewRealmEvent(
    kind: PreviewRuntimeEventKind,
    previewRevision: string,
    durationMs: number,
    diagnostic: string | null = null,
  ) {
    const identity = this.activeCanvasIdentity;
    if (
      !identity
      || identity.previewRevision !== previewRevision
      || !Number.isFinite(durationMs)
      || durationMs < 0
    ) return;
    await this.recordCanvasProjectionRuntimeEvent(
      kind,
      identity,
      durationMs,
      diagnostic,
    );
  }

  async recordCanvasProjectionRuntimeEvent(
    kind: PreviewRuntimeEventKind,
    identity: CanvasProjectionIdentity,
    durationMs: number,
    diagnostic: string | null,
  ) {
    try {
      const receipt = await recordPreviewRuntimeEvent({
        schemaVersion: 1,
        identity,
        kind,
        durationMs: Math.min(600_000, Math.round(durationMs)),
        diagnostic,
      });
      if (
        !receipt.accepted
        || receipt.identity.projectRoot !== identity.projectRoot
        || receipt.identity.runtimeSessionId !== identity.runtimeSessionId
        || receipt.identity.workspaceRevision !== identity.workspaceRevision
        || receipt.identity.transactionId !== identity.transactionId
        || receipt.identity.previewRevision !== identity.previewRevision
        || receipt.kind !== kind
      ) {
        throw new Error(t("workbench-canvas-event-mismatch"));
      }
    } catch (error) {
      if (this.activeCanvasIdentity?.transactionId !== identity.transactionId) return;
      this.setGlobalStatus(
        t("workbench-canvas-observability-failed", { message: errorMessage(error) }),
        "error",
      );
    }
  }

  scheduleZolaValidation(reason: ZolaValidationReason = "save") {
    scheduleZolaValidationFromController(this.controlledPreviewControllerHost(), reason);
  }

  async runZolaValidation(reason: ZolaValidationReason = "manual") {
    return await runZolaValidationFromController(this.controlledPreviewControllerHost(), reason);
  }

  controlledPreviewControllerHost(): ControlledPreviewControllerHost {
    return this;
  }

  previewControllerHost(): PreviewControllerHost {
    return this;
  }

  hydratePageSections(sections: PageSection[]) {
    return hydratePageSectionsFromController(this.pageSectionsHost(), sections);
  }

  setPageSections(sections: PageSection[]) {
    setPageSectionsFromController(this.pageSectionsHost(), sections);
  }

  resetPageSections() {
    resetPageSectionsFromController(this.pageSectionsHost());
  }

  pageSectionsHost(): PageSectionsHost {
    return this;
  }

  async refreshEditorNavigationSnapshot(
    identity = this.activeCanvasIdentity ?? undefined,
    previewUrl = this.activeCanvasUrl || this.previewSrc,
  ) {
    if (!identity) {
      this.editorNavigationRequestSerial += 1;
      this.editorNavigationSnapshot = null;
      this.editorNavigationLoading = false;
      this.editorNavigationError = "";
      this.editorEditScopeGrant = null;
      this.editorEditScopeId = null;
      this.hoverSnapshot = null;
      return;
    }
    const serial = ++this.editorNavigationRequestSerial;
    const route = editorNavigationRoute(previewUrl, this.browserPreviewRoute);
    this.editorNavigationLoading = true;
    this.editorNavigationError = "";
    try {
      const activeDocumentPath = this.activeScannedPath;
      const previewContextRenderInstanceId =
        this.coordinatedElementSelection?.renderInstanceId ?? null;
      const snapshot = await readEditorNavigationSnapshot(
        identity,
        route,
        activeDocumentPath,
        previewContextRenderInstanceId,
      );
      if (
        serial !== this.editorNavigationRequestSerial
        || !canvasIdentityEquals(this.activeCanvasIdentity, identity)
      ) return;
      this.editorNavigationSnapshot = snapshot;
      if (
        this.editorEditScopeGrant
        && (
          this.editorEditScopeGrant.projectRoot !== identity.projectRoot
          || this.editorEditScopeGrant.runtimeSessionId !== identity.runtimeSessionId
          || this.editorEditScopeGrant.workspaceRevision !== identity.workspaceRevision
          || this.editorEditScopeGrant.previewRevision !== identity.previewRevision
          || this.editorEditScopeGrant.canvasTransactionId !== identity.transactionId
          || this.editorEditScopeGrant.activeDocumentPath
            !== snapshot.focusedView?.activeDocumentPath
        )
      ) {
        this.editorEditScopeGrant = null;
        this.editorEditScopeId = null;
      }
      await this.rebaseSelectionSnapshot(identity, route);
    } catch (error) {
      if (
        serial !== this.editorNavigationRequestSerial
        || !canvasIdentityEquals(this.activeCanvasIdentity, identity)
      ) return;
      this.editorNavigationSnapshot = null;
      this.editorNavigationError = errorMessage(error);
      this.editorEditScopeGrant = null;
      this.editorEditScopeId = null;
    } finally {
      if (serial === this.editorNavigationRequestSerial) {
        this.editorNavigationLoading = false;
      }
    }
  }

  async applySelectionIntent(intent: SelectionIntent): Promise<SelectionSnapshot | null> {
    const identity = this.activeCanvasIdentity;
    if (!identity) return null;
    const route = editorNavigationRoute(
      this.activeCanvasUrl || this.previewSrc,
      this.browserPreviewRoute,
    );
    const serial = ++this.selectionCoordinatorRequestSerial;
    const receipt = await applySelectionIntentInRust(
      identity,
      route,
      this.activeScannedPath,
      this.selectionSnapshot?.anchor?.renderInstanceId
        ?? this.coordinatedElementSelection?.renderInstanceId
        ?? null,
      intent,
    );
    if (
      serial !== this.selectionCoordinatorRequestSerial
      || !canvasIdentityEquals(this.activeCanvasIdentity, identity)
    ) return null;
    this.projectSelectionCoordinatorSnapshot(
      receipt.selection,
      receipt.hover,
      receipt.inspectorSummary,
    );
    if (intent.kind === "setFocus") {
      projectSelectionSnapshotOnCanvas(this, receipt.selection);
    }
    return receipt.selection;
  }

  async applyHoverIntent(intent: Extract<SelectionIntent, {
    kind: "setHover" | "clearHover";
  }>): Promise<HoverSnapshot | null> {
    const identity = this.activeCanvasIdentity;
    if (!identity) return null;
    const route = editorNavigationRoute(
      this.activeCanvasUrl || this.previewSrc,
      this.browserPreviewRoute,
    );
    const serial = ++this.hoverCoordinatorRequestSerial;
    const receipt = await applySelectionIntentInRust(
      identity,
      route,
      this.activeScannedPath,
      this.selectionSnapshot?.anchor?.renderInstanceId
        ?? this.coordinatedElementSelection?.renderInstanceId
        ?? null,
      intent,
    );
    if (
      serial !== this.hoverCoordinatorRequestSerial
      || !canvasIdentityEquals(this.activeCanvasIdentity, identity)
    ) return null;
    this.hoverSnapshot = receipt.hover;
    this.inspectorSelectionSummary = receipt.inspectorSummary;
    return receipt.hover;
  }

  async rebaseSelectionSnapshot(
    identity = this.activeCanvasIdentity ?? undefined,
    route = editorNavigationRoute(
      this.activeCanvasUrl || this.previewSrc,
      this.browserPreviewRoute,
    ),
  ): Promise<SelectionSnapshot | null> {
    if (!identity) return null;
    const serial = ++this.selectionCoordinatorRequestSerial;
    const receipt = await readSelectionSnapshot(
      identity,
      route,
      this.activeScannedPath,
      this.selectionSnapshot?.anchor?.renderInstanceId
        ?? this.coordinatedElementSelection?.renderInstanceId
        ?? null,
    );
    if (
      serial !== this.selectionCoordinatorRequestSerial
      || !canvasIdentityEquals(this.activeCanvasIdentity, identity)
    ) return null;
    this.projectSelectionCoordinatorSnapshot(
      receipt.selection,
      receipt.hover,
      receipt.inspectorSummary,
    );
    projectSelectionSnapshotOnCanvas(this, receipt.selection);
    return receipt.selection;
  }

  async acceptSelectionObservation(
    input: SelectionObservationInput,
    observation: CanvasElementObservation,
  ): Promise<AcceptedCanvasElementObservation | null> {
    const selection = this.selectionSnapshot;
    if (
      !selection
      || selection.selectionRevision !== input.selectionRevision
      || !canvasIdentityEquals(selection.canvasIdentity, input.canvasIdentity)
    ) return null;
    const receipt = await acceptSelectionObservation(input);
    if (this.selectionSnapshot?.selectionRevision !== input.selectionRevision) return null;
    const accepted = {
      selectionRevision: receipt.selectionRevision,
      canvasIdentity: receipt.canvasIdentity,
      documentEpoch: receipt.documentEpoch,
      renderInstanceId: receipt.renderInstanceId,
      observation: captureCanvasElementObservation(observation) ?? observation,
    };
    this.acceptedSelectionObservation = accepted;
    this.inspectorSelectionSummary = receipt.inspectorSummary;
    return accepted;
  }

  private projectSelectionCoordinatorSnapshot(
    selection: SelectionSnapshot,
    hover: HoverSnapshot | null,
    inspectorSummary: InspectorSelectionSummarySnapshot,
  ) {
    const previousRenderInstanceId = this.selectionSnapshot?.anchor?.renderInstanceId ?? null;
    this.selectionSnapshot = selection;
    this.inspectorSelectionSummary = inspectorSummary;
    this.hoverSnapshot = hover;

    if (selection.resolution === "cleared" || !selection.subject) {
      this.acceptedSelectionObservation = null;
      return;
    }

    if (selection.subject.kind === "teraBoundary") {
      this.acceptedSelectionObservation = null;
    } else {
      const accepted = this.acceptedSelectionObservation;
      const renderInstanceId = selection.anchor?.renderInstanceId ?? null;
      if (
        !accepted
        || previousRenderInstanceId !== renderInstanceId
        || accepted.renderInstanceId !== renderInstanceId
        || inspectorSummary.state !== "resolved"
        || inspectorSummary.renderInstanceId !== renderInstanceId
      ) {
        this.acceptedSelectionObservation = null;
      } else if (accepted.selectionRevision !== selection.selectionRevision) {
        this.acceptedSelectionObservation = {
          ...accepted,
          selectionRevision: selection.selectionRevision,
          canvasIdentity: selection.canvasIdentity,
        };
      }
    }

    if (selection.focus.kind === "cssRule" || selection.focus.kind === "cssProperty") {
      this.targetCssFile = selection.focus.file;
    }
  }

  async refreshSourceGraph(options: { strict?: boolean } = {}) {
    await refreshSourceGraphFromController(this, options);
  }

  resolveSourceEditTargetForSourceId(sourceId: string | null | undefined) {
    return resolveSourceEditTargetForSourceIdFromController(this, sourceId);
  }

  resolveSourceEditLocationForSourceId(sourceId: string | null | undefined) {
    return resolveSourceEditLocationForSourceIdFromController(this, sourceId);
  }

  syncHtmlCodeToPreview(sourceText: string, cursorPosition: number) {
    syncHtmlCodeToPreviewFromController(this, sourceText, cursorPosition);
  }

  attachPreviewInspector() {
    attachPreviewInspectorFromController(this);
  }

  fetchDomTreeFromPreview() {
    fetchDomTreeFromPreviewFromController(this.previewControllerHost());
  }

  applyStagedOverrideStylesToPreview(css: string) {
    applyStagedOverrideStylesToPreviewFromController(this, css);
  }

  breakpointValue(name: string, fallback: string) {
    return breakpointValueFromController(this.previewLiveControllerHost(), name, fallback);
  }

  applyInspectorLiveProperties(
    selector: string | null,
    properties: Record<string, string>,
    viewport: "desktop" | "tablet" | "mobile" = this.previewDevice,
  ) {
    return applyInspectorLivePropertiesFromController(
      this.previewLiveControllerHost(),
      selector,
      properties,
      viewport,
    );
  }

  applyInspectorLivePropertyDrafts(entries: InspectorCssDraft[]) {
    return applyInspectorLivePropertyDraftsFromController(this.previewLiveControllerHost(), entries);
  }

  clearInspectorLiveProperties(expectedEpoch?: number) {
    let expectedIdentity: InspectorLiveCssIdentity | undefined;
    if (expectedEpoch !== undefined) {
      const captured = captureInspectorLiveCssIdentity(
        this.previewLiveControllerHost(),
        expectedEpoch,
      );
      if (!captured) return false;
      expectedIdentity = captured;
    }
    return clearInspectorLivePropertiesFromController(
      this.previewLiveControllerHost(),
      expectedIdentity,
    );
  }

  async projectCommittedInspectorCssMutation(
    authority: CssMutationAuthorityReceipt,
    liveEpoch: number | null,
  ) {
    const projectRoot = this.sessionProjectRoot;
    const sessionId = this.kernelProjectSessionId;
    if (
      authority.projectRoot !== projectRoot
      || authority.sessionId !== sessionId
    ) {
      throw new Error(t("workbench-css-live-session-mismatch"));
    }
    if (
      authority.schemaVersion !== 2
      || authority.documents.map((projection) => projection.relativePath).join("\u0000")
        !== authority.touchedFiles.join("\u0000")
      || (authority.status === "noop" && authority.documents.length !== 0)
    ) {
      throw new Error(t("workbench-css-documents-mismatch"));
    }

    const mutation = authority.workspaceMutation;
    const transactionId = mutation?.transactionId?.trim() ?? "";
    if (
      authority.status === "staged"
      && (
        !mutation?.changed
        || mutation.revisionBefore !== authority.revisionBefore
        || mutation.revisionAfter !== authority.revisionAfter
        || !transactionId
      )
    ) {
      throw new Error(t("workbench-css-transaction-missing"));
    }

    let localProjectionWarning = "";
    try {
      await flushFileBufferDraftSync({ throwOnFailure: true });
      for (const projection of authority.documents) {
        rebaseFileBufferDraftSyncProjection(projection.relativePath, projection.snapshot);
        const cacheKey = scannedCacheKey({ relativePath: projection.relativePath });
        if (projection.snapshot) {
          this.sourceCache = {
            ...this.sourceCache,
            [cacheKey]: projection.snapshot.text,
          };
          if (this.activeScannedPath === projection.relativePath) {
            this.source = projection.snapshot.text;
          }
        } else {
          const nextCache = { ...this.sourceCache };
          delete nextCache[cacheKey];
          this.sourceCache = nextCache;
          if (this.activeScannedPath === projection.relativePath) {
            this.source = "";
          }
        }
      }
      if (
        authority.status === "noop"
        || authority.documents.some((projection) => /\.(?:css|scss)$/i.test(projection.relativePath))
      ) {
        // Un no-op poate proveni dintr-un control rămas în urma snapshot-ului
        // canonic (de exemplu după Undo). Recitirea sursei deschise repară
        // starea toggle-ului chiar dacă Rust nu are documente schimbate de emis.
        this.notifyCssSourceChanged();
      }
    } catch (error) {
      localProjectionWarning = errorMessage(error);
    }

    const draftIdentity = liveEpoch === null
      ? null
      : captureInspectorLiveCssIdentity(this.previewLiveControllerHost(), liveEpoch);

    if (authority.status === "noop") {
      if (draftIdentity) {
        clearInspectorLivePropertiesFromController(
          this.previewLiveControllerHost(),
          draftIdentity,
        );
      }
      return;
    }
    if (!mutation) {
      throw new Error(t("workbench-css-staged-mutation-missing"));
    }

    let boundIdentity: InspectorLiveCssIdentity | null = null;
    try {
      const workspace = await readProjectWorkspaceState();
      if (
        !workspace
        || workspace.projectRoot !== projectRoot
        || workspace.runtimeSessionId !== sessionId
        || workspace.revision !== authority.revisionAfter
      ) {
        throw new Error(
          t("workbench-css-revision-unconfirmed"),
        );
      }
      await settleProjectWorkspaceMutation(this, {
        projectRoot,
        runtimeSessionId: sessionId,
        mutation,
        workspace,
      }, {
        warningLabel: "Modificarea CSS",
        onCanvasPlanPrepared: (plan) => {
          if (plan.workspaceTransactionId !== transactionId) {
            throw new Error(t("workbench-css-canvas-plan-mismatch"));
          }
          if (!draftIdentity) return;
          boundIdentity = bindInspectorLiveCssTransaction(
            this.previewLiveControllerHost(),
            draftIdentity,
            {
              workspaceRevision: plan.identity.workspaceRevision,
              workspaceTransactionId: transactionId,
              canvasTransactionId: plan.identity.transactionId,
              previewRevision: plan.identity.previewRevision,
            },
          );
        },
      });
    } catch (error) {
      if (
        this.sessionProjectRoot === projectRoot
        && this.kernelProjectSessionId === sessionId
      ) {
        this.setGlobalStatus(
          t("workbench-css-resync", { message: errorMessage(error) }),
          "unsaved",
        );
      }
    }
    if (
      localProjectionWarning
      && this.sessionProjectRoot === projectRoot
      && this.kernelProjectSessionId === sessionId
    ) {
      this.setGlobalStatus(
        t("workbench-css-editor-resync", { message: localProjectionWarning }),
        "unsaved",
      );
    }
    if (
      this.sessionProjectRoot !== projectRoot
      || this.kernelProjectSessionId !== sessionId
    ) return;

    const exactIdentity = boundIdentity ?? draftIdentity;
    if (exactIdentity) {
      clearInspectorLivePropertiesFromController(
        this.previewLiveControllerHost(),
        exactIdentity,
      );
    }
  }

  async updateDesignSystemVariable(
    variable: ScssVariable,
    value: string,
  ): Promise<boolean> {
    const nextValue = value.trim();
    if (!nextValue || nextValue === variable.value) return false;
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    const identity = createCssRequestIdentity(projectRoot, runtimeSessionId);
    const receipt = await setScssVariable(
      variable.file,
      variable.name,
      nextValue,
      identity,
    );
    if (
      this.sessionProjectRoot !== projectRoot
      || this.kernelProjectSessionId !== runtimeSessionId
    ) return false;
    await this.projectCommittedInspectorCssMutation(receipt.authority, null);
    if (
      this.sessionProjectRoot !== projectRoot
      || this.kernelProjectSessionId !== runtimeSessionId
    ) return false;
    this.scssVariables = await getScssVariables(
      identity,
      this.projectWorkspaceSnapshot?.revision,
    ).catch(() => (
      this.scssVariables.map((entry) => (
        entry.file === variable.file && entry.name === variable.name
          ? { ...entry, value: nextValue }
          : entry
      ))
    ));
    this.setGlobalStatus(t("workbench-token-updated", { name: variable.name }), "unsaved");
    return true;
  }

  async createDesignSystemVariable(
    relativePath: string,
    name: string,
    value: string,
  ): Promise<boolean> {
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    const identity = createCssRequestIdentity(projectRoot, runtimeSessionId);
    const receipt = await createScssVariable(relativePath, name, value, identity);
    if (
      this.sessionProjectRoot !== projectRoot
      || this.kernelProjectSessionId !== runtimeSessionId
    ) return false;
    await this.projectCommittedInspectorCssMutation(receipt.authority, null);
    if (
      this.sessionProjectRoot !== projectRoot
      || this.kernelProjectSessionId !== runtimeSessionId
    ) return false;
    let scssProjectionCurrent = true;
    this.scssVariables = await getScssVariables(
      identity,
      this.projectWorkspaceSnapshot?.revision,
    ).catch(() => {
      scssProjectionCurrent = false;
      return this.scssVariables;
    });
    this.setGlobalStatus(
      scssProjectionCurrent
        ? t("workbench-token-created", { name: name.replace(/^\$/, "") })
        : t("workbench-token-created-resync", { name: name.replace(/^\$/, "") }),
      "unsaved",
    );
    return true;
  }

  async createDesignSystemClass(name: string, relativePath: string): Promise<boolean> {
    const outcome = await runInPreviewStructuralLane(this, async (lease) => {
      const receipt = await createDesignClassCommand(name, relativePath, {
        expectedProjectRoot: lease.projectRoot,
        expectedSessionId: lease.sessionId,
      });
      requireCurrentPreviewStructuralSession(this, lease);
      const settlement = await settleProjectWorkspaceMutation(this, receipt, {
        preferredRelativePath: relativePath,
        warningLabel: t("workbench-class-create-operation"),
      });
      requireCurrentPreviewStructuralSession(this, lease);
      try {
        await this.refreshDesignClassInventory(true);
      } catch (error) {
        settlement.warnings.push(
          t("workbench-class-inventory-resync", { message: errorMessage(error) }),
        );
      }
      requireCurrentPreviewStructuralSession(this, lease);
      this.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("workbench-class-created-resync", {
              name: name.replace(/^\./, ""),
              path: relativePath,
            })
          : t("workbench-class-created", {
              name: name.replace(/^\./, ""),
              path: relativePath,
            }),
        "unsaved",
      );
      return true;
    });
    return outcome ?? false;
  }

  async renameDesignSystemClass(oldName: string, newName: string): Promise<boolean> {
    const outcome = await runInPreviewStructuralLane(this, async (lease) => {
      const receipt = await renameDesignClassCommand(oldName, newName, {
        expectedProjectRoot: lease.projectRoot,
        expectedSessionId: lease.sessionId,
      });
      requireCurrentPreviewStructuralSession(this, lease);
      const settlement = await settleProjectWorkspaceMutation(this, receipt.workspace, {
        preferredRelativePath: this.activeScannedPath,
        warningLabel: t("workbench-class-rename-operation"),
      });
      requireCurrentPreviewStructuralSession(this, lease);
      try {
        await this.refreshDesignClassInventory(true);
      } catch (error) {
        settlement.warnings.push(
          t("workbench-class-inventory-resync", { message: errorMessage(error) }),
        );
      }
      requireCurrentPreviewStructuralSession(this, lease);
      this.setGlobalStatus(
        settlement.warnings.length > 0
          ? t("workbench-class-renamed-resync", {
              oldName: receipt.oldName,
              newName: receipt.newName,
            })
          : t("workbench-class-renamed", {
              oldName: receipt.oldName,
              newName: receipt.newName,
              files: receipt.changedFiles.length,
              references: receipt.replacementCount,
            }),
        "unsaved",
      );
      return true;
    });
    return outcome ?? false;
  }

  handlePreviewMessage = (event: MessageEvent) => {
    handlePreviewMessageFromController(this, event);
  };

  closeContextMenu() {
    contextMenu.close();
  }

  previewInsertControllerHost(): PreviewInsertControllerHost {
    return this;
  }

  previewTeraInsertControllerHost(): PreviewTeraInsertControllerHost {
    return this;
  }

  elementPaletteDragHost(): ElementPaletteDragHost {
    return this;
  }

  teraPaletteDragHost(): TeraPaletteDragHost {
    return this;
  }

  teraActionsControllerHost(): TeraActionsControllerHost {
    return this;
  }

  // ── Code editor ───────────────────────────────────────────────────────────

  async setCenterView(view: CenterView) {
    if (view !== this.centerView && this.centerView === "preview") {
      try {
        await this.flushInteractiveEditorDrafts("template-switch");
      } catch (error) {
        this.setGlobalStatus(t("workbench-activity-switch-blocked", {
          message: errorMessage(error),
        }), "error");
        return false;
      }
    }
    const enteringCode = view === "code" && this.centerView !== "code";
    const enteringPreview = view === "preview" && this.centerView !== "preview";
    if (enteringCode) {
      await this.prepareHtmlCodeRevealTargetForCodeEntry();
      this.requestCodeSelectionReveal();
    }
    const targetActivity: WorkbenchActivity = view === "kernel"
      ? "audit"
      : "editor";
    if (
      this.workbenchHydratedRuntimeSessionId === this.kernelProjectSessionId
      && this.workbenchSnapshot
      && this.workbenchSnapshot.activeActivity !== targetActivity
    ) {
      try {
        await this.workbenchController.apply({
          kind: "set_activity",
          activity: targetActivity,
        });
        this.clearNotification("workbench.activity-sync");
      } catch (error) {
        this.escalateGlobalStatus({
          id: "workbench.activity-sync",
          level: "warning",
          title: t("workbench-activity-switch-failed"),
          message: errorMessage(error),
        });
        return false;
      }
    }
    this.centerView = view;
    if (
      this.activeScannedPath
      && (view === "preview" || view === "code" || view === "markdown")
      && this.workbenchSnapshot?.split === "none"
    ) {
      try {
        await this.workbenchController.setActiveDocumentSurface(this.activeScannedPath, view);
        this.clearNotification("workbench.surface-sync");
      } catch (error) {
        this.escalateGlobalStatus({
          id: "workbench.surface-sync",
          level: "warning",
          title: t("workbench-document-surface-save-failed"),
          message: errorMessage(error),
        });
      }
    }
    if (enteringPreview && this.scannedProject) {
      const projectRoot = this.sessionProjectRoot;
      const sessionId = this.kernelProjectSessionId;
      const sessionEpoch = this.projectSessionEpoch;
      await tick();
      if (
        this.centerView === "preview"
        && this.sessionProjectRoot === projectRoot
        && this.kernelProjectSessionId === sessionId
        && this.projectSessionEpoch === sessionEpoch
      ) {
        try {
          await projectLatestProjectWorkspacePreview(this, { reason: "manual" });
        } catch (error) {
          if (
            this.centerView === "preview"
            && this.sessionProjectRoot === projectRoot
            && this.kernelProjectSessionId === sessionId
            && this.projectSessionEpoch === sessionEpoch
          ) {
            this.setGlobalStatus(
              t("workbench-preview-project-failed", { message: errorMessage(error) }),
              "error",
            );
          }
        }
      }
    }
    return true;
  }

  async showVersionPreview(receipt: VersionPreviewReceipt) {
    if (
      receipt.projectRoot !== this.sessionProjectRoot
      || receipt.sessionId !== this.kernelProjectSessionId
    ) {
      throw new Error(t("workbench-version-preview-session-stale"));
    }
    await this.flushInteractiveEditorDrafts("template-switch");
    invalidatePreviewRefreshLease(this);
    this.interactivePreviewEnabled = false;
    this.motionWorkspace.previewMode = "design";
    this.templateWorkbenchActive = false;
    this.activeVersionPreview = receipt;
    this.centerView = "preview";
    this.previewSrc = receipt.previewUrl;
    this.previewDocumentMarkup = null;
  }

  async returnToLivePreview() {
    if (!this.activeVersionPreview) return;
    await stopVersionPreview({
      expectedProjectRoot: this.sessionProjectRoot,
      expectedSessionId: this.kernelProjectSessionId,
    });
    this.activeVersionPreview = null;
    this.centerView = "preview";
    await projectLatestProjectWorkspacePreview(this, {
      reason: "manual",
      force: true,
    });
  }

  setCssCodeRevealTarget(target: { selector: string; file: string }) {
    if (!target.selector || !target.file) return;
    this.targetCssFile = target.file;
    if (
      this.codeRevealTarget.kind === "css" &&
      this.codeRevealTarget.selector === target.selector &&
      this.codeRevealTarget.file === target.file
    ) {
      return;
    }
    this.codeRevealTarget = { kind: "css", selector: target.selector, file: target.file };
  }

  async selectCssFocusFromInspector(target: {
    selector: string;
    file: string;
    property?: string | null;
    expectedSelectionRevision?: number | null;
  }): Promise<boolean> {
    if (!target.selector || !target.file) return false;
    if (
      target.expectedSelectionRevision
      && this.selectionSnapshot?.selectionRevision !== target.expectedSelectionRevision
    ) {
      this.setGlobalStatus(t("inspector-css-focus-blocked"), "error");
      return false;
    }
    const property = target.property?.trim() || null;
    const focus = this.selectionSnapshot?.focus;
    if (
      (
        focus?.kind === "cssRule"
        || focus?.kind === "cssProperty"
      )
      && focus.file === target.file
      && focus.selector === target.selector
      && (
        (!property && focus.kind === "cssRule")
        || (
          property
          && focus.kind === "cssProperty"
          && focus.property === property
        )
      )
    ) return true;
    try {
      const selection = await this.applySelectionIntent({
        kind: "setFocus",
        focus: property
          ? {
              kind: "cssProperty",
              selector: target.selector,
              file: target.file,
              property,
              viewport: this.previewDevice,
            }
          : {
              kind: "cssRule",
              selector: target.selector,
              file: target.file,
              viewport: this.previewDevice,
            },
        expectedSelectionRevision: target.expectedSelectionRevision ?? null,
      });
      if (
        !selection
        || (
          selection.focus.kind !== "cssRule"
          && selection.focus.kind !== "cssProperty"
        )
        || selection.focus.file !== target.file
        || selection.focus.selector !== target.selector
      ) return false;
      return property
        ? selection.focus.kind === "cssProperty" && selection.focus.property === property
        : selection.focus.kind === "cssRule";
    } catch (error) {
      this.setGlobalStatus(
        `${t("inspector-css-focus-blocked")} ${errorMessage(error)}`,
        "error",
      );
      return false;
    }
  }

  selectJsBehaviorFromCode(target: { file: string; behaviorId?: string | null }) {
    if (!target.file) return;
    const focus = this.selectionSnapshot?.focus;
    if (
      focus?.kind === "jsBehavior"
      && focus.file === target.file
      && focus.behaviorId === (target.behaviorId ?? null)
    ) return;
    void this.applySelectionIntent({
      kind: "setFocus",
      focus: {
        kind: "jsBehavior",
        file: target.file,
        behaviorId: target.behaviorId ?? null,
      },
    });
  }

  selectInspectorTab(tab: InspectorTab) {
    this.activeInspectorTab = tab;
    if (tab === "html") {
      if (this.selectionSnapshot?.focus.kind !== "element") {
        void this.applySelectionIntent({
          kind: "setFocus",
          focus: { kind: "element" },
        });
      }
      return;
    }
    if (tab !== "js") return;
    const templatePath =
      this.selectionSnapshot?.provenance?.definition?.file
      ?? this.selectionSnapshot?.provenance?.composition?.file
      ?? this.activeRenderedTemplatePath;
    if (templatePath) {
      this.selectJsBehaviorFromCode({ file: pageJsRelativePath(templatePath) });
    }
  }

  setHtmlCodeRevealTarget() {
    if (this.codeRevealTarget.kind === "html") return;
    this.codeRevealTarget = { kind: "html" };
  }

  requestCodeSelectionReveal() {
    this.codeSelectionRevealRequestId += 1;
  }

  consumeCodeSelectionRevealRequest() {
    if (this.codeSelectionRevealConsumedId === this.codeSelectionRevealRequestId) return false;
    this.codeSelectionRevealConsumedId = this.codeSelectionRevealRequestId;
    return true;
  }

  async openCssCodeRevealTarget(target: { selector: string; file: string }) {
    if (!this.scannedProject || !target.selector || !target.file) return;
    this.setCssCodeRevealTarget(target);
    const targetPath = zolaRelativePath(target.file);
    const file = this.scannedProject.files.find(
      (item) => item.relativePath === target.file || zolaRelativePath(item.relativePath) === targetPath,
    );
    if (file && this.activeScannedPath !== file.relativePath) {
      await this.loadScannedProjectFile(file);
    }
    await this.setCenterView("code");
    this.requestCodeSelectionReveal();
  }

  async prepareHtmlCodeRevealTargetForCodeEntry() {
    const sourceFile = this.selectionSnapshot?.projections.code.file;
    if (!this.scannedProject || !sourceFile) return;
    const targetPath = zolaRelativePath(sourceFile);
    const file = this.scannedProject.files.find(
      (item) => item.relativePath === sourceFile || zolaRelativePath(item.relativePath) === targetPath,
    );
    if (!file || this.activeScannedPath === file.relativePath) return;

    await this.loadScannedProjectFile(file);
  }

  async createCodeEditor() {
    await createSourceEditorFromController(this.sourceEditorControllerHost());
  }

  handleCodeCursorSelection(position: number, sourceText: string) {
    handleCodeCursorSelectionFromController(this.sourceEditorControllerHost(), position, sourceText);
  }

  async selectSourcePositionFromCode(file: string, offset: number) {
    const selection = await this.applySelectionIntent({
      kind: "selectSourcePosition",
      file,
      offset,
      viewport: this.previewDevice,
    });
    if (selection) {
      projectSelectionSnapshotOnCanvas(this, selection, { revealCode: false });
    }
  }

  updateMarkdownSource(nextSource: string, relativePath = this.currentSourceRelativePath) {
    updateMarkdownSourceFromController(this.sourceEditorControllerHost(), nextSource, relativePath);
  }

  syncCodeSelectionHighlight(reveal = false) {
    syncCodeSelectionHighlightFromController(this.sourceEditorControllerHost(), reveal);
  }

  notifyCssSourceChanged() {
    this.cssSourceRevision += 1;
  }

  cssRuleContextFromOpenSource(file: string, selector: string, viewport: CssViewport): CssRuleContext | null {
    if (!this.isOpenCssSource(file) || !selector) return null;
    return cssRuleContextFromSource(this.source, file, selector, viewport);
  }

  isOpenCssSource(file: string) {
    if (this.sourceLanguage !== "css" && this.sourceLanguage !== "scss") return false;
    if (!file || !this.currentSourceRelativePath) return false;
    return zolaRelativePath(file) === zolaRelativePath(this.currentSourceRelativePath);
  }

  withSyncingCode(fn: () => void) {
    withSyncingCodeFromController(this.sourceEditorControllerHost(), fn);
  }

  openCodeEditorContextMenu(request: CodeEditorContextMenuRequest) {
    contextMenu.open({
      source: "code",
      x: request.event.clientX,
      y: request.event.clientY,
      title: this.currentSourcePath || t("workbench-source-code"),
      subtitle: t("workbench-code-position", {
        line: request.line,
        column: request.column,
      }),
      items: [
        {
          id: "save-source",
          label: t("workbench-save"),
          shortcut: "Ctrl+S",
          disabled: !this.saveHasPending,
          action: async () => {
            await this.saveActiveFile();
          },
        },
        {
          id: "select-html-at-cursor",
          label: t("workbench-select-html-at-cursor"),
          disabled: this.sourceLanguage !== "html",
          separatorBefore: true,
          action: () => this.handleCodeCursorSelection(request.position, request.docText),
        },
        {
          id: "reveal-current-selection",
          label: t("workbench-reveal-selection-code"),
          disabled: !this.selectionSnapshot?.subject,
          action: () => this.syncCodeSelectionHighlight(true),
        },
        {
          id: "copy-code-selection",
          label: t("workbench-copy-code-selection"),
          disabled: !request.hasSelection,
          separatorBefore: true,
          action: async () => {
            if (!request.selectedText) return;
            await navigator.clipboard?.writeText(request.selectedText);
            this.setGlobalStatus(t("workbench-code-selection-copied"), "idle");
          },
        },
      ],
    });
  }

  sourceEditorControllerHost(): SourceEditorControllerHost {
    return this;
  }

  // ── Selection ─────────────────────────────────────────────────────────────

  clearPreviewSelection(options: { clearCanvasOverlay?: boolean } = {}) {
    clearPreviewSelectionFromController(this, options);
  }

  setPreviewTeraSelection(
    target: PreviewTeraSelectionTarget,
    options: { status?: string } = {},
  ) {
    setPreviewTeraSelectionFromController(this, target, options);
  }

  applySelectionState(
    selection: CanvasElementObservation,
    resolvedStyles?: EditableStyles,
  ) {
    applySelectionStateFromAppSelectionController(this, selection, resolvedStyles);
  }

  previewDropTargetStatus(target: {
    targetRenderInstanceId?: string | null;
    targetBoundarySourceId?: string | null;
  }) {
    return editorNavigationDropTargetStatusFromController(this, target);
  }

  async openSelectedTeraSource() {
    await openSelectedTeraSourceFromController(this);
  }

  selectTeraLayerSource(section: PageSection, sourceId: string) {
    selectTeraLayerSourceFromController(this, section, sourceId);
  }

  selectPreviewElement(element: Element, options: { revealCode?: boolean } = {}) {
    this.setHtmlCodeRevealTarget();
    if (!selectCanvasPreviewElement(this, element, options)) {
      this.setGlobalStatus("Ținta nu există în EditorNavigationSnapshot-ul Rust curent.", "error");
    }
  }

  selectHtmlTarget(target: EditorHtmlTarget, options: { revealCode?: boolean } = {}) {
    this.setHtmlCodeRevealTarget();
    const renderInstanceId = target.renderInstanceId
      ?? target.selector.match(/data-pana-render-instance-id=["']([^"']+)["']/)?.[1]
      ?? null;
    const sourceNodeId = target.sourceId
      ?? target.section?.sourceId
      ?? null;
    const candidates = this.editorNavigationSnapshot?.nodes.filter((node) =>
      node.kind === "htmlElement"
      && (
        renderInstanceId
          ? node.renderInstanceId === renderInstanceId
          : sourceNodeId
            ? node.sourceNodeId === sourceNodeId
            : false
      )
    ) ?? [];
    if (candidates.length !== 1) {
      this.setGlobalStatus(
        candidates.length > 1
          ? "Ținta HTML este ambiguă în EditorNavigationSnapshot-ul Rust curent."
          : "Ținta HTML nu există în EditorNavigationSnapshot-ul Rust curent.",
        "error",
      );
      return;
    }
    void this.applySelectionIntent({
      kind: "selectEditorNode",
      editorNodeId: candidates[0].id,
    }).then((selection) => {
      if (!selection) return;
      projectSelectionSnapshotOnCanvas(this, selection, options);
      if (options.revealCode) this.requestCodeSelectionReveal();
    });
  }

  selectionControllerHost(): SelectionControllerHost {
    return this;
  }

  updateAttributeValue(property: string, value: string) {
    updateAttributeValueFromController(this.htmlDraftControllerHost(), property, value);
    const session = this.captureActiveHtmlAttributeEditSession();
    if (!session) return;
    session.attributes = { ...this.attributeValues };
    this.projectLiveHtmlAttributeDraft(session);
  }

  updateTextContentValue(value: string, composing = false) {
    updateTextContentValueFromController(this.htmlDraftControllerHost(), value);
    const session = this.captureActiveHtmlTextEditSession(value);
    if (!session) return;
    session.text = value;
    this.activeHtmlTextEditValue = value;
    this.postPreviewMessage({
      type: "apply-live-text-draft",
      editSessionId: session.id,
      target: {
        selector: session.target.selector,
        sourceId: session.target.sourceId ?? null,
        sessionId: session.target.sessionId ?? null,
        expectedTag: session.target.tag,
      },
      text: value,
    });
    if (composing) return;
    this.enqueueHtmlTextDraftCommit(session);
    this.scheduleHtmlTextCanonicalProjection(session.id);
    this.scheduleHtmlTextHistoryBoundary(session.id);
  }

  private htmlDraftTargetIdentity(target: HtmlActionTarget) {
    return target.sourceId
      ?? target.sessionId
      ?? (target.sourceLocation
        ? `${target.sourceLocation.file}:${target.sourceLocation.line}:${target.sourceLocation.column ?? 0}`
        : target.selector);
  }

  private htmlAssetEditContextKey(target: HtmlActionTarget) {
    return [
      target.sourceId ?? "",
      target.sessionId ?? "",
      target.sourceLocation?.file ?? "",
      target.sourceLocation?.line ?? "",
      target.sourceLocation?.column ?? "",
      target.selector,
      target.tag,
    ].join("::");
  }

  private captureActiveHtmlAttributeEditSession(): ActiveHtmlAttributeEditSession | null {
    const selection = this.coordinatedElementSelection;
    const target = captureHtmlActionTarget(selection);
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    if (!selection || !target || !projectRoot || !runtimeSessionId) return null;
    const key = `${projectRoot}\u0000${runtimeSessionId}\u0000attributes\u0000${this.htmlDraftTargetIdentity(target)}`;
    const current = this.activeHtmlAttributeEditSession;
    if (
      current
      && current.key === key
      && current.projectSessionEpoch === this.projectSessionEpoch
    ) return current;

    if (current) this.cancelActiveHtmlAttributeEditSession();
    const id = `attr_${Date.now().toString(36)}_${(++this.htmlAttributeEditSessionSerial).toString(36)}`;
    const session: ActiveHtmlAttributeEditSession = {
      id,
      key,
      projectRoot,
      runtimeSessionId,
      projectSessionEpoch: this.projectSessionEpoch,
      target,
      attributes: { ...this.attributeValues },
      baselineAttributes: { ...this.attributeValues },
      baselineNames: Object.keys(target.attributes ?? {})
        .filter((name) => !name.toLowerCase().startsWith("data-pana-")),
      latestLiveEpoch: 0,
      latestLiveProjection: null,
      finishPromise: null,
    };
    this.activeHtmlAttributeEditSession = session;
    return session;
  }

  private projectLiveHtmlAttributeDraft(session: ActiveHtmlAttributeEditSession) {
    const draftEpoch = ++session.latestLiveEpoch;
    const projection = liveProjectableHtmlAttributeDraft(
      session.target.tag,
      session.attributes,
      session.baselineNames,
    );
    const settlement = this.previewRuntime.sendAndWait({
      type: "apply-live-attribute-draft",
      editSessionId: session.id,
      draftEpoch,
      target: {
        selector: session.target.selector,
        sourceId: session.target.sourceId ?? null,
        sessionId: session.target.sessionId ?? null,
        expectedTag: session.target.tag,
      },
      attributes: projection.attributes,
      baselineNames: projection.baselineNames,
    }).then((ack) => {
      if (!ack.ok) throw new Error(ack.error || t("workbench-attribute-live-rejected"));
      if (!isLatestHtmlAttributeDraftSettlement(
        this.activeHtmlAttributeEditSession?.id ?? null,
        this.activeHtmlAttributeEditSession?.latestLiveEpoch ?? -1,
        session.id,
        draftEpoch,
      )) return;
      this.attributeStatus = t("workbench-attribute-draft-confirmed");
    }).catch((error) => {
      if (isLatestHtmlAttributeDraftSettlement(
        this.activeHtmlAttributeEditSession?.id ?? null,
        this.activeHtmlAttributeEditSession?.latestLiveEpoch ?? -1,
        session.id,
        draftEpoch,
      )) {
        this.attributeStatus = t("workbench-attribute-live-failed", {
          message: error instanceof Error ? error.message : String(error),
        });
      }
      throw error;
    });
    session.latestLiveProjection = settlement;
    void settlement.catch(() => {});
  }

  private cancelActiveHtmlAttributeEditSession() {
    const session = this.activeHtmlAttributeEditSession;
    if (session) {
      const clear = this.previewRuntime.sendAndWait({
        type: "clear-live-attribute-draft",
        editSessionId: session.id,
        draftEpoch: session.latestLiveEpoch,
      });
      void clear.catch(() => {});
    }
    this.activeHtmlAttributeEditSession = null;
  }

  cancelHtmlAttributeDraft(expectedContextKey?: string) {
    const session = this.activeHtmlAttributeEditSession;
    if (!session) return;
    const sessionContextKey = this.htmlAssetEditContextKey(session.target);
    if (expectedContextKey && sessionContextKey !== expectedContextKey) return;
    const currentTarget = captureHtmlActionTarget(this.coordinatedElementSelection);
    if (currentTarget && this.htmlAssetEditContextKey(currentTarget) === sessionContextKey) {
      this.attributeValues = { ...session.baselineAttributes };
    }
    this.cancelActiveHtmlAttributeEditSession();
    this.setHtmlPending("attributes", false);
    this.attributeStatus = t("workbench-attribute-edit-cancelled");
  }

  private async finishActiveHtmlAttributeEditSession(
    attributeOverride?: EditableAttributes,
  ): Promise<EditorActionOutcome | null> {
    const session = this.activeHtmlAttributeEditSession;
    if (!session) return null;
    if (attributeOverride) session.attributes = { ...attributeOverride };
    if (
      session.projectRoot !== this.sessionProjectRoot
      || session.runtimeSessionId !== this.kernelProjectSessionId
      || session.projectSessionEpoch !== this.projectSessionEpoch
    ) {
      this.cancelActiveHtmlAttributeEditSession();
      return null;
    }
    // Direct inspector commits, Save flush and project transitions must join
    // the same canonical completion. The structural lane serializes commands,
    // but without this single-flight boundary it would still execute the same
    // attribute intent twice and turn the second, valid no-op into an error.
    if (session.finishPromise) return await session.finishPromise;
    const operation = this.finishCapturedHtmlAttributeEditSession(session);
    session.finishPromise = operation;
    try {
      return await operation;
    } finally {
      if (session.finishPromise === operation) session.finishPromise = null;
    }
  }

  private async finishCapturedHtmlAttributeEditSession(
    session: ActiveHtmlAttributeEditSession,
  ): Promise<EditorActionOutcome | null> {
    while (this.activeHtmlAttributeEditSession?.id === session.id) {
      const liveProjection = session.latestLiveProjection;
      if (liveProjection) {
        try {
          await liveProjection;
        } catch {
          // The speculative Canvas projection is allowed to fail closed. The
          // canonical ProjectWorkspace mutation below remains authoritative.
        }
      }
      if (this.activeHtmlAttributeEditSession?.id !== session.id) return null;

      const submittedLiveEpoch = session.latestLiveEpoch;
      const submittedAttributes = { ...session.attributes };
      const result = await applyAttributesToCapturedHtmlTarget(
        this.htmlActionsControllerHost(),
        session.target,
        submittedAttributes,
      );
      if (result.status !== "committed" && result.status !== "noop") return result;
      if (this.activeHtmlAttributeEditSession?.id !== session.id) return result;

      // A newer inspector value arrived while the canonical commit was in
      // flight. Keep the same owner and project only the latest draft next.
      if (session.latestLiveEpoch !== submittedLiveEpoch) continue;

      try {
        const ack = await this.previewRuntime.sendAndWait({
          type: "clear-live-attribute-draft",
          editSessionId: session.id,
          draftEpoch: submittedLiveEpoch,
        });
        if (!ack.ok) throw new Error(ack.error || t("workbench-canvas-draft-close-unconfirmed"));
      } catch (error) {
        this.attributeStatus = t("workbench-attribute-source-confirmed-canvas-failed", {
          message: error instanceof Error ? error.message : String(error),
        });
      }
      if (this.activeHtmlAttributeEditSession?.id !== session.id) return result;
      if (session.latestLiveEpoch !== submittedLiveEpoch) continue;

      this.activeHtmlAttributeEditSession = null;
      this.setHtmlPending("attributes", false);
      this.attributeStatus = result.status === "noop"
        ? t("workbench-attributes-no-changes")
        : t("workbench-attributes-confirmed");
      return result;
    }
    return null;
  }

  private captureActiveHtmlTextEditSession(value: string): ActiveHtmlTextEditSession | null {
    const selection = this.coordinatedElementSelection;
    const target = captureHtmlActionTarget(selection);
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    if (
      !selection
      || selection.observation.hasChildElements
      || !target
      || !projectRoot
      || !runtimeSessionId
    ) return null;
    const key = htmlTextSelectionKey(selection);
    const current = this.activeHtmlTextEditSession;
    if (
      current
      && current.key === key
      && current.projectRoot === projectRoot
      && current.runtimeSessionId === runtimeSessionId
      && current.projectSessionEpoch === this.projectSessionEpoch
    ) return current;

    this.clearHtmlTextEditTimers();
    const id = `text_${Date.now().toString(36)}_${(++this.htmlTextEditSessionSerial).toString(36)}`;
    const session: ActiveHtmlTextEditSession = {
      id,
      key,
      projectRoot,
      runtimeSessionId,
      projectSessionEpoch: this.projectSessionEpoch,
      target,
      text: value,
      projectedText: null,
    };
    this.activeHtmlTextEditSession = session;
    this.activeHtmlTextEditKey = key;
    this.activeHtmlTextEditValue = value;
    return session;
  }

  private enqueueHtmlTextDraftCommit(session: ActiveHtmlTextEditSession) {
    this.htmlTextDraftCommitQueue.enqueue({
      key: `${session.projectRoot}\u0000${session.runtimeSessionId}\u0000text\u0000${session.id}`,
      projectRoot: session.projectRoot,
      runtimeSessionId: session.runtimeSessionId,
      projectSessionEpoch: session.projectSessionEpoch,
      target: session.target,
      text: session.text,
      editSessionId: session.id,
    });
  }

  private clearHtmlTextEditTimers() {
    if (this.htmlTextCanonicalTimer !== null) clearTimeout(this.htmlTextCanonicalTimer);
    if (this.htmlTextHistoryTimer !== null) clearTimeout(this.htmlTextHistoryTimer);
    this.htmlTextCanonicalTimer = null;
    this.htmlTextHistoryTimer = null;
  }

  private scheduleHtmlTextCanonicalProjection(editSessionId: string) {
    if (this.htmlTextCanonicalTimer !== null) clearTimeout(this.htmlTextCanonicalTimer);
    this.htmlTextCanonicalTimer = setTimeout(() => {
      this.htmlTextCanonicalTimer = null;
      void this.projectActiveHtmlTextEditSession(editSessionId).catch((error) => {
        if (this.activeHtmlTextEditSession?.id !== editSessionId) return;
        this.setGlobalStatus(
          t("workbench-text-projection-failed", { message: errorMessage(error) }),
          "error",
        );
      });
    }, HTML_TEXT_CANONICAL_IDLE_MS);
  }

  private scheduleHtmlTextHistoryBoundary(editSessionId: string) {
    if (this.htmlTextHistoryTimer !== null) clearTimeout(this.htmlTextHistoryTimer);
    this.htmlTextHistoryTimer = setTimeout(() => {
      this.htmlTextHistoryTimer = null;
      void this.finishActiveHtmlTextEditSession(editSessionId).catch((error) => {
        if (this.activeHtmlTextEditSession?.id !== editSessionId) return;
        this.setGlobalStatus(t("workbench-text-edit-close-failed", {
          message: errorMessage(error),
        }), "error");
      });
    }, HTML_TEXT_HISTORY_IDLE_MS);
  }

  private projectActiveHtmlTextEditSession(editSessionId: string): Promise<void> {
    const task = this.htmlTextProjectionTail
      .catch(() => undefined)
      .then(async () => {
        const session = this.activeHtmlTextEditSession;
        if (!session || session.id !== editSessionId) return;
        await this.htmlTextDraftCommitQueue.flush({ throwOnFailure: true });
        const projectedText = session.text;
        if (
          this.activeHtmlTextEditSession?.id !== editSessionId
          || session.projectRoot !== this.sessionProjectRoot
          || session.runtimeSessionId !== this.kernelProjectSessionId
          || session.projectSessionEpoch !== this.projectSessionEpoch
        ) return;
        const workspaceRevision = this.projectWorkspaceSnapshot?.revision;
        if (workspaceRevision === undefined) {
          throw new Error(t("workbench-text-workspace-revision-missing"));
        }
        const derived = await this.reconcileWorkspaceDerivedState({
          expectedProjectRoot: session.projectRoot,
          expectedSessionId: session.runtimeSessionId,
          expectedWorkspaceRevision: workspaceRevision,
          topologyChanged: false,
          preferredRelativePath: session.target.sourceLocation?.file ?? null,
          refreshSourceGraph: true,
          refreshScss: false,
        });
        if (this.activeHtmlTextEditSession?.id !== editSessionId) return;
        const preview = await projectLatestProjectWorkspacePreview(this, {
          reason: "workspace-mutation",
          minimumWorkspaceRevision: workspaceRevision,
          expectedWorkspaceRevision: workspaceRevision,
          requestedPaths: session.target.sourceLocation?.file
            ? [session.target.sourceLocation.file]
            : undefined,
        });
        if (this.activeHtmlTextEditSession?.id === editSessionId) {
          session.projectedText = projectedText;
          if (derived.warnings.length > 0 || preview.status === "deferred") {
            this.setGlobalStatus(
              t("workbench-text-resync"),
              "unsaved",
            );
          }
        }
      });
    this.htmlTextProjectionTail = task.catch(() => undefined);
    return task;
  }

  private cancelActiveHtmlTextEditSession() {
    const session = this.activeHtmlTextEditSession;
    this.clearHtmlTextEditTimers();
    if (session) {
      this.postPreviewMessage({ type: "clear-live-text-draft", editSessionId: session.id });
    }
    this.activeHtmlTextEditSession = null;
    this.activeHtmlTextEditKey = null;
    this.activeHtmlTextEditValue = null;
    this.textEditOriginalKey = null;
    this.textEditOriginalText = null;
  }

  private async finishActiveHtmlTextEditSession(expectedEditSessionId?: string) {
    const session = this.activeHtmlTextEditSession;
    if (!session || (expectedEditSessionId && session.id !== expectedEditSessionId)) {
      await this.htmlTextDraftCommitQueue.flush({ throwOnFailure: true });
      return false;
    }
    this.clearHtmlTextEditTimers();
    await this.htmlTextDraftCommitQueue.flush({ throwOnFailure: true });
    if (this.activeHtmlTextEditSession?.id !== session.id) return false;
    await this.htmlTextProjectionTail.catch(() => undefined);
    if (this.activeHtmlTextEditSession?.id !== session.id) return false;
    if (session.projectedText !== session.text) {
      await this.projectActiveHtmlTextEditSession(session.id);
    }
    if (this.activeHtmlTextEditSession?.id !== session.id) return false;
    this.postPreviewMessage({ type: "clear-live-text-draft", editSessionId: session.id });
    this.activeHtmlTextEditSession = null;
    this.activeHtmlTextEditKey = null;
    this.activeHtmlTextEditValue = null;
    this.textEditOriginalKey = null;
    this.textEditOriginalText = null;
    this.setHtmlPending("text", false);
    this.textStatus = t("workbench-text-confirmed");
    return true;
  }

  htmlDraftControllerHost(): HtmlDraftControllerHost {
    return this;
  }

  // ── HTML mutations ────────────────────────────────────────────────────────

  async stageKernelPlannedTemplateDraft(
    tpl: { file: string; line: number },
    plannedSource: string,
    options: { pendingArea?: HtmlPendingArea; status?: string; isCurrent?: () => boolean } = {},
  ) {
    return await stageKernelPlannedTemplateDraftFromController(this.htmlMutationControllerHost(), tpl, plannedSource, options);
  }

  htmlMutationControllerHost(): HtmlMutationControllerHost {
    return this;
  }

  async insertNodeRelative(position: InsertPosition, opts: { tag: string; className: string; text: string }) {
    await insertNodeRelativeFromController(this.htmlActionsControllerHost(), position, opts);
  }

  startElementPaletteDrag(element: HtmlPaletteElement, event: PointerEvent) {
    startElementPaletteDragFromController(this.elementPaletteDragHost(), element, event);
  }

  startTeraPaletteDrag(item: TeraPaletteItem, event: PointerEvent) {
    startTeraPaletteDragFromController(this.teraPaletteDragHost(), item, event);
  }

  async insertPaletteElementAtTarget(request: PreviewInsertDropRequest) {
    await insertPaletteElementAtTargetFromController(this.htmlActionsControllerHost(), request);
  }

  async insertTeraPaletteItemAtTarget(request: TeraDropRequest) {
    return await insertTeraPaletteItemAtTargetFromController(this.teraActionsControllerHost(), request);
  }

  selectEditorNavigationNode(node: EditorNavigationNode) {
    selectEditorNavigationNodeFromController(this.editorNavigationControllerHost(), node);
  }

  hoverEditorNavigationNode(node: EditorNavigationNode | null) {
    hoverEditorNavigationNodeFromController(this.editorNavigationControllerHost(), node);
  }

  async enterEditorNavigationScope(scopeId: string) {
    return await enterEditorNavigationScopeFromController(
      this.editorNavigationControllerHost(),
      scopeId,
    );
  }

  exitEditorNavigationScope() {
    exitEditorNavigationScopeFromController(this.editorNavigationControllerHost());
  }

  async previewEditorNavigationMove(
    sourceNodeId: string,
    targetNodeId: string,
    position: ProjectMovePosition,
  ): Promise<EditorMovePlan> {
    return await previewEditorNavigationMoveFromController(
      this.editorNavigationControllerHost(),
      sourceNodeId,
      targetNodeId,
      position,
    );
  }

  async moveEditorNavigationNode(
    sourceNodeId: string,
    targetNodeId: string,
    position: ProjectMovePosition,
  ) {
    return await moveEditorNavigationNodeFromController(
      this.editorNavigationControllerHost(),
      sourceNodeId,
      targetNodeId,
      position,
    );
  }

  async deleteEditorNavigationNode(node: EditorNavigationNode) {
    const selector = editorNavigationNodeSelector(node) ?? "";
    if (node.kind === "htmlElement") {
      return await this.editorRuntime.dispatch({
        type: "delete-html",
        surface: "layers",
        target: {
          kind: "html",
          selector,
          tag: node.tag ?? "",
          label: node.label,
          sourceId: node.sourceNodeId,
        },
      });
    }
    if (node.kind !== "teraBoundary") return;
    const sourceNode = node.sourceNodeId
      ? this.sourceGraph?.nodes.find((candidate) =>
          candidate.id === node.sourceNodeId
        ) ?? null
      : null;
    return await this.editorRuntime.dispatch({
      type: "delete-tera",
      surface: "layers",
      target: {
        kind: "tera",
        sourceId: node.sourceNodeId ?? "",
        selector: selector || null,
        label: node.label,
        kindLabel: node.sourceKind ?? undefined,
        file: node.file,
        origin: node.origin === "project"
          ? "local"
          : node.origin === "theme"
            ? "theme"
            : "unknown",
        themeName: node.themeName,
        sourceNode,
      },
    });
  }

  async deleteHtmlElement(selector?: string | null) {
    const section = selector ? this.pageSections.find((item) => item.selector === selector) : null;
    const target = section
      ? htmlTargetFromPageSection(section)
      : this.coordinatedElementSelection
        ? htmlTargetFromCoordinatedSelection(this.coordinatedElementSelection)
        : null;
    if (!target) {
      return await this.editorRuntime.dispatch({
        type: "delete-html",
        surface: "runtime",
        target: {
          kind: "html",
          selector: selector ?? "",
          tag: "",
        },
      });
    }
    return await this.editorRuntime.dispatch({ type: "delete-html", surface: "runtime", target });
  }

  async duplicateHtmlElement(selector?: string | null) {
    const section = selector ? this.pageSections.find((item) => item.selector === selector) : null;
    const target = section
      ? htmlTargetFromPageSection(section)
      : this.coordinatedElementSelection
        ? htmlTargetFromCoordinatedSelection(this.coordinatedElementSelection)
        : null;
    if (!target) {
      return await this.editorRuntime.dispatch({
        type: "duplicate-html",
        surface: "runtime",
        target: {
          kind: "html",
          selector: selector ?? "",
          tag: "",
        },
      });
    }
    return await this.editorRuntime.dispatch({ type: "duplicate-html", surface: "runtime", target });
  }

  async deleteSelectedTeraNode(target: EditorTeraTarget | null = null) {
    const sourceNode = target
      ? target.sourceNode ?? null
      : this.selectedTemplateSourceNode;
    return await deleteSelectedTeraNodeFromController(
      this.teraActionsControllerHost(),
      sourceNode,
    );
  }

  async applyImageSourceToHtml(src?: string) {
    return await applyImageSourceToHtmlFromController(this.htmlActionsControllerHost(), src);
  }

  async applyZolaImageProcessingToHtml(intent: ProjectZolaImageIntent) {
    return await applyZolaImageProcessingToHtmlFromController(
      this.htmlActionsControllerHost(),
      intent,
    );
  }

  async applyNativeBlockOption(request: ApplyNativeBlockOptionRequest) {
    return await applyNativeBlockOptionToHtmlFromController(
      this.htmlActionsControllerHost(),
      request,
    );
  }

  async applyClassesToHtml() {
    return await applyClassesToHtmlFromController(this.htmlActionsControllerHost());
  }

  async generateClassForSelectedHtml() {
    return await generateClassForSelectedHtmlFromController(this.htmlActionsControllerHost());
  }

  async generateDataAnimForSelectedHtml() {
    return await generateDataAnimForSelectedHtmlFromController(this.htmlActionsControllerHost());
  }

  async openSourceLocation(source: string) {
    await openSourceLocationFromController(this.htmlActionsControllerHost(), source);
  }

  async changeElementTag(newTag: string) {
    return await changeHtmlElementTag(this.htmlEditControllerHost(), newTag);
  }

  async applyTagChange() {
    return await applyHtmlTagChange(this.htmlEditControllerHost());
  }

  removeAttribute(name: string) {
    removeAttributeFromController(this.htmlDraftControllerHost(), name);
    const session = this.captureActiveHtmlAttributeEditSession();
    if (!session) return;
    session.attributes = { ...this.attributeValues };
    this.projectLiveHtmlAttributeDraft(session);
  }

  async applyAttributesToHtml(attributes?: EditableAttributes) {
    const activeResult = await this.finishActiveHtmlAttributeEditSession(attributes);
    if (activeResult) return activeResult;
    if (!this.htmlPending.attributes) {
      return noopAction(t("workbench-attributes-already-confirmed"));
    }
    return await applyAttributesToHtmlFromController(this.htmlActionsControllerHost(), attributes);
  }

  async applyTextContentToHtml() {
    const committed = await this.finishActiveHtmlTextEditSession();
    if (!committed) {
      return noopAction(t("workbench-text-already-confirmed"));
    }
    return committedAction();
  }

  htmlActionsControllerHost(): HtmlActionsControllerHost {
    return this;
  }

  editorNavigationControllerHost(): EditorNavigationControllerHost {
    return this;
  }

  updatePageFrontmatterSource(relativePath: string, nextSource: string) {
    updatePageFrontmatterSourceFromController(this.pageSettingsControllerHost(), relativePath, nextSource);
  }

  async readPageSettingsDocument(relativePath: string): Promise<string> {
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    const cacheKey = scannedCacheKey({ relativePath });
    const cached = this.sourceCache[cacheKey];
    if (typeof cached === "string") return cached;
    const source = await readProjectFile(relativePath);
    if (
      this.sessionProjectRoot !== projectRoot
      || this.kernelProjectSessionId !== runtimeSessionId
    ) throw new Error(t("workbench-metadata-session-stale"));
    this.sourceCache = { ...this.sourceCache, [cacheKey]: source };
    return source;
  }

  pageSettingsControllerHost(): PageSettingsControllerHost {
    return this;
  }

  async resetHistoryAfterExternalReconcile() {
    this.cancelPendingHtmlMutations();
    this.overrideRules = {};
    this.variableOverrides = {};
    this.liveCssById = {};
    this.inspectorLiveCssEpoch = this.inspectorLiveCssEpoch >= Number.MAX_SAFE_INTEGER
      ? 1
      : this.inspectorLiveCssEpoch + 1;
    this.inspectorLiveCssIdentity = null;
    this.variableValues = {};
    this.htmlPending = createEmptyHtmlPending();
    this.resetInspectorPendingSources();
    this.inspectorPending = createEmptyInspectorPending();
    this.acceptedSelectionObservation = null;
    this.inspectorSelectionSummary = null;
    this.postPreviewMessage({ type: "clear-canvas-interaction-overlays" });
  }

  htmlEditControllerHost(): HtmlEditControllerHost {
    return this;
  }

  async saveSessionDrafts() {
    if (this.blockSaveForExternalProjectionConflict()) return false;
    if (this.blockSaveForKernelUndoRedoLease()) return false;
    return await saveSessionDraftsFromController(this.saveControllerHost());
  }

  async saveSourceFile() {
    if (this.blockSaveForAiLease()) return false;
    if (this.blockSaveForExternalProjectionConflict()) return false;
    if (this.blockSaveForKernelUndoRedoLease()) return false;
    return await saveSourceFileFromController(this.saveControllerHost());
  }

  async savePendingHtmlChanges() {
    if (this.blockSaveForAiLease()) {
      return blockedAction(
        t("workbench-html-save-ai-blocked"),
      );
    }
    if (this.blockSaveForExternalProjectionConflict()) {
      return blockedAction(
        t("workbench-html-save-external-blocked"),
      );
    }
    if (this.blockSaveForKernelUndoRedoLease()) {
      return blockedAction(
        t("workbench-html-save-history-blocked"),
      );
    }
    return await savePendingHtmlChangesFromController(this.saveControllerHost());
  }

  async saveActiveFile() {
    if (this.blockSaveForAiLease()) return false;
    if (this.blockSaveForExternalProjectionConflict()) return false;
    if (this.blockSaveForKernelUndoRedoLease()) return false;
    if (this.projectTransitionFrontendLeaseActive) {
      this.setGlobalStatus(
        t("workbench-save-transition-blocked"),
        "error",
      );
      return false;
    }
    if (this.saveOperationPromise) return await this.saveOperationPromise;
    const operation = (async () => {
      try {
        await suspendAndDrainExternalDiskMonitoringFromController(
          this.externalDiskControllerHost(),
        );
        if (this.blockSaveForExternalProjectionConflict()) return false;
        if (
          this.externalDiskState.checking
          || this.externalDiskState.reconciling
          || this.externalDiskState.changed
          || this.externalDiskState.blockedByDirtySession
        ) {
          this.setGlobalStatus(
            t("workbench-save-external-state-blocked"),
            "error",
          );
          return false;
        }
        return await saveActiveDocument(this.saveControllerHost());
      } catch (error) {
        this.setGlobalStatus(
          t("workbench-save-disk-boundary-failed", { message: errorMessage(error) }),
          "error",
        );
        return false;
      } finally {
        resumeExternalDiskMonitoringAfterSaveFromController(
          this.externalDiskControllerHost(),
        );
      }
    })();
    this.saveOperationPromise = operation;
    try {
      return await operation;
    } finally {
      if (this.saveOperationPromise === operation) this.saveOperationPromise = null;
    }
  }

  /**
   * Reserves the complete project-wide write boundary used by kernel history.
   * The reservation is raised before either drain so a new structural write
   * or monitor tick cannot enter behind the barrier and race the Undo/Redo
   * disk commit.
   */
  async beginKernelUndoRedoFrontendLease() {
    if (this.aiEditLeaseFrontendLockActive) {
      throw new Error(t("workbench-history-ai-blocked"));
    }
    if (this.projectTransitionFrontendLeaseActive) {
      throw new Error(
        t("workbench-history-transition-blocked"),
      );
    }
    if (this.kernelUndoRedoFrontendLeaseActive) {
      throw new Error(t("workbench-history-busy"));
    }

    this.kernelUndoRedoFrontendLeaseActive = true;
    contextMenu.close();
    this.quiesceExternalReconcileInteractions();
    try {
      await tick();
      if (this.saveOperationPromise) await this.saveOperationPromise;
      await this.flushInteractiveEditorDrafts("history");
      await drainPreviewStructuralLanes();
      await suspendAndDrainExternalDiskMonitoringFromController(
        this.externalDiskControllerHost(),
      );
      if (
        this.externalDiskState.checking
        || this.externalDiskState.reconciling
        || this.externalDiskState.changed
        || this.externalDiskState.blockedByDirtySession
        || this.externalDiskState.workspaceProjectionRecoveryRequired
      ) {
        throw new Error(
          t("workbench-history-disk-not-clean"),
        );
      }
    } catch (error) {
      this.endKernelUndoRedoFrontendLease();
      throw error;
    }
  }

  endKernelUndoRedoFrontendLease() {
    if (!this.kernelUndoRedoFrontendLeaseActive) return;
    this.kernelUndoRedoFrontendLeaseActive = false;
    resumeExternalDiskMonitoringAfterSaveFromController(
      this.externalDiskControllerHost(),
    );
  }

  async beginProjectTransitionFrontendLease() {
    if (
      this.aiEditLeaseFrontendLockActive
      && !this.aiReconciliationRecoveryReloadAuthorized
    ) {
      throw new Error(
        t("workbench-transition-ai-blocked"),
      );
    }
    if (this.kernelUndoRedoFrontendLeaseActive) {
      throw new Error(
        t("workbench-transition-history-blocked"),
      );
    }
    this.projectTransitionFrontendLeaseActive = true;
    this.cancelActiveHtmlAttributeEditSession();
    this.cancelActiveHtmlTextEditSession();
    this.htmlTextDraftCommitQueue.reset();
    invalidatePreviewRefreshLease(this.previewControllerHost());
    invalidatePreviewDomTreeProjection(this.previewControllerHost());
    this.sourceGraphLoadSerial += 1;
    contextMenu.close();
    this.quiesceExternalReconcileInteractions();
    try {
      await tick();
      if (this.saveOperationPromise) await this.saveOperationPromise;
      await suspendAndDrainExternalDiskMonitoringFromController(
        this.externalDiskControllerHost(),
      );
      await drainPreviewStructuralLanes();
    } catch (error) {
      this.endProjectTransitionFrontendLease();
      throw error;
    }
  }

  endProjectTransitionFrontendLease() {
    this.projectTransitionFrontendLeaseActive = false;
    resumeExternalDiskMonitoringAfterTransitionLeaseFromController(
      this.externalDiskControllerHost(),
    );
  }

  saveControllerHost(): SaveControllerHost {
    return this;
  }

  private blockSaveForExternalProjectionConflict() {
    if (!this.externalDiskState.workspaceProjectionRecoveryRequired) return false;
    this.setGlobalStatus(
      t("workbench-save-projection-recovery-blocked"),
      "error",
    );
    return true;
  }

  private blockSaveForAiLease() {
    if (!this.aiEditLeaseFrontendLockActive) return false;
    this.setGlobalStatus(
      t("workbench-save-ai-blocked"),
      "error",
    );
    return true;
  }

  private blockSaveForKernelUndoRedoLease() {
    if (!this.kernelUndoRedoFrontendLeaseActive) return false;
    this.setGlobalStatus(
      t("workbench-save-history-blocked"),
      "error",
    );
    return true;
  }

  // ── UI ────────────────────────────────────────────────────────────────────

  toggleUiTheme() {
    toggleUiThemeFromController(this.uiControllerHost());
    void this.persistApplicationTheme({
      mode: "fixed",
      value: this.uiTheme,
    });
  }

  setApplicationTheme(theme: ApplicationTheme) {
    this.setApplicationThemePreference({ mode: "fixed", value: theme });
  }

  setApplicationThemePreference(preference: ApplicationThemePreference) {
    if (preference.mode === "fixed") {
      setUiThemeFromController(this.uiControllerHost(), preference.value);
    }
    void this.persistApplicationTheme(preference);
  }

  openApplicationSettings() {
    this.applicationSurface = "settings";
  }

  openProjectWorkbench() {
    this.applicationSurface = "workbench";
  }

  async initApplicationSettings() {
    this.applicationSettingsLoading = true;
    try {
      const snapshot = await readApplicationSettings();
      this.applyApplicationSettingsSnapshot(snapshot);
    } catch (error) {
      this.escalateGlobalStatus({
        id: "application.settings.load",
        level: "warning",
        title: t("diagnostic-application-settings-load-failed"),
        message: errorMessage(error),
      });
    } finally {
      this.applicationSettingsLoading = false;
    }
  }

  private persistApplicationTheme(theme: ApplicationThemePreference) {
    return this.persistApplicationSettingsPatch(
      { theme },
      t("diagnostic-application-settings-save-failed"),
    );
  }

  persistApplicationSettingsPatch(
    patch: ApplicationSettingsPatch,
    failureTitle = t("diagnostic-application-settings-save-failed"),
  ) {
    const operation = this.applicationSettingsSaveTail.then(async () => {
      const current = this.applicationSettings ?? await readApplicationSettings();
      const snapshot = await saveApplicationSettings(current.revision, patch);
      this.applyApplicationSettingsSnapshot(snapshot);
      this.clearNotification("application.settings.load");
      this.clearNotification("application.settings.save");
    });
    this.applicationSettingsSaveTail = operation.then(
      () => undefined,
      (error) => {
        this.escalateGlobalStatus({
          id: "application.settings.save",
          level: "warning",
          title: failureTitle,
          message: errorMessage(error),
        });
      },
    );
    return this.applicationSettingsSaveTail;
  }

  refreshApplicationSettingsForSystemGeneration(generation: number) {
    if ((this.applicationSettings?.system.generation ?? 0) >= generation) return;
    const operation = this.applicationSettingsRefreshTail.then(async () => {
      if ((this.applicationSettings?.system.generation ?? 0) >= generation) return;
      const snapshot = await readApplicationSettings();
      this.applyApplicationSettingsSnapshot(snapshot);
    });
    this.applicationSettingsRefreshTail = operation.catch((error) => {
      this.escalateGlobalStatus({
        id: "application.settings.system-refresh",
        level: "warning",
        title: t("diagnostic-application-settings-system-refresh-failed"),
        message: errorMessage(error),
      });
    });
  }

  private applyApplicationSettingsSnapshot(snapshot: ApplicationSettingsSnapshot) {
    this.applicationSettings = snapshot;
    this.uiLocale = snapshot.effective.locale;
    this.uiDirection = snapshot.effective.direction;
    this.uiAccent = snapshot.effective.accent;
    l10n.setLocale(this.uiLocale);
    setUiThemeFromController(this.uiControllerHost(), snapshot.effective.theme);
    if (typeof document === "undefined") return;
    document.documentElement.lang = this.uiLocale;
    document.documentElement.dir = this.uiDirection;
    document.documentElement.dataset.panaLocale = this.uiLocale;
    document.documentElement.dataset.panaContrast =
      snapshot.system.contrast ?? "normal";
    document.documentElement.dataset.panaReducedMotion =
      snapshot.system.reducedMotion === true ? "true" : "false";
    document.documentElement.style.setProperty("--brand", this.uiAccent);
    document.documentElement.style.setProperty(
      "--brand-strong",
      `color-mix(in srgb, ${this.uiAccent} 70%, ${this.uiTheme === "dark" ? "white" : "black"})`,
    );
    document.documentElement.style.setProperty(
      "--brand-soft",
      `color-mix(in srgb, ${this.uiAccent} ${this.uiTheme === "dark" ? "19%" : "11%"}, transparent)`,
    );
    document.documentElement.style.setProperty(
      "--focus-ring",
      `color-mix(in srgb, ${this.uiAccent} 72%, ${this.uiTheme === "dark" ? "white" : "black"})`,
    );
    document.documentElement.style.setProperty(
      "--text-on-accent",
      contrastingTextColor(this.uiAccent),
    );
    applyApplicationBootProjection(document, snapshot.boot);
    if (typeof window !== "undefined") {
      try {
        storeApplicationBootProjection(window.localStorage, snapshot.boot);
      } catch {
        // The cache is optional and never participates in application state.
      }
    }
  }

  persistBlockPropertiesLayout(height: number, collapsed: boolean) {
    const normalizedHeight = Math.max(140, Math.min(520, Math.round(height)));
    const operation = this.applicationSettingsSaveTail.then(async () => {
      const current = this.applicationSettings ?? await readApplicationSettings();
      if (
        current.blockPropertiesHeight === normalizedHeight
        && current.blockPropertiesCollapsed === collapsed
      ) return;
      const snapshot = await saveApplicationSettings(current.revision, {
        blockPropertiesHeight: normalizedHeight,
        blockPropertiesCollapsed: collapsed,
      });
      this.applyApplicationSettingsSnapshot(snapshot);
      this.clearNotification("application.settings.save");
    });
    this.applicationSettingsSaveTail = operation.then(
      () => undefined,
      (error) => {
        this.escalateGlobalStatus({
          id: "application.settings.save",
          level: "warning",
          title: t("diagnostic-application-settings-layout-save-failed"),
          message: errorMessage(error),
        });
      },
    );
    return this.applicationSettingsSaveTail;
  }

  resetResize(kind: ResizeKind) {
    resetResizeFromController(this.uiControllerHost(), kind);
  }

  stopResizeDrag() {
    stopResizeDragFromController(this.uiControllerHost());
  }

  startResizeDrag(kind: ResizeKind, event: MouseEvent) {
    startResizeDragFromController(this.uiControllerHost(), kind, event);
  }

  uiControllerHost(): UiControllerHost {
    return this;
  }

  // ── Terminal tabs ─────────────────────────────────────────────────────────

  async openTerminalTab() {
    if (!(await this.setWorkbenchBottomPanel(true, "terminal"))) return;
    openTerminalTabFromController(this.terminalTabsHost());
  }

  async selectTerminalTab(tabId: string) {
    if (!(await this.setWorkbenchBottomPanel(true, "terminal"))) return;
    selectTerminalTabFromController(this.terminalTabsHost(), tabId);
  }

  closeTerminalTab(tabId: string) {
    closeTerminalTabFromController(this.terminalTabsHost(), tabId);
  }

  async runTerminalQuickTask(task: TerminalQuickTask) {
    if (!(await this.setWorkbenchBottomPanel(true, "terminal"))) return;
    await runTerminalQuickTaskFromController(this.terminalQuickTaskHost(), task);
  }

  async clearActiveTerminal() {
    await clearActiveTerminalFromController(this.terminalQuickTaskHost());
  }

  terminalTabsHost(): TerminalTabsHost {
    return this;
  }

  terminalQuickTaskHost(): TerminalQuickTaskHost {
    return this;
  }

  injectRawCss(id: string, css: string) {
    injectRawCssFromController(this.previewLiveControllerHost(), id, css);
  }

  restoreLiveCssLayersToPreview() {
    restoreLiveCssLayersToPreviewFromController(this.previewLiveControllerHost());
  }

  previewLiveControllerHost(): PreviewLiveControllerHost {
    return this;
  }

}
