import type { CodeEditorContextMenuRequest, CodeEditorController } from "$lib/editor/controller";
import { tick } from "svelte";
import { contextMenu } from "$lib/context-menu/store.svelte";
import {
  htmlElementContextMenuItems,
  teraContextMenuItems,
} from "$lib/editor-runtime/context-menu";
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
  htmlTargetFromPageSection,
  htmlTargetFromSelection,
  teraTargetFromGate,
  type EditorLayerContextMenuRequest,
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
import { stopVersionPreview } from "$lib/project/io";
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
import { dispatchExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
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
import type { StatusControllerHost } from "$lib/state/status-controller";
import type { InsertPosition } from "$lib/html/mutations";
import {
  isLatestHtmlAttributeDraftSettlement,
  liveProjectableHtmlAttributeDraft,
} from "$lib/html/live-attribute-draft";
import type { TeraDropRequest, TeraMoveRequest, TeraPaletteItem } from "$lib/tera/model";
import { canRequestTemplateEditGateKind, templateEditGateSelectionStatus } from "$lib/tera/template-edit-gate";
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
import type { LayerMoveRequest } from "$lib/project/layers-drag";
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
  ApplicationSettingsSnapshot,
  ApplicationSurface,
  ApplicationTheme,
  CenterView,
  DesignClassInventorySnapshot,
  ExternalDiskState,
  PageSection,
  ProjectDiskManifest,
  ProjectAuditSnapshot,
  ProjectFile,
  ProjectScan,
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
  PreviewSelectionState,
  SaveState,
  ScssVariable,
	  SelectionInfo,
	  SourceEditLocation,
	  SourceGraph,
  TemplateWorkbenchPlan,
  SourceGraphNode,
} from "$lib/types";
import {
  WorkbenchProjectionController,
  type WorkbenchProjectionHost,
} from "$lib/workbench/controller";
import {
  createInspectorPendingSourceRegistry,
  type InspectorPendingSource,
} from "$lib/state/inspector-pending";
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
  invalidatePreviewDomTreeProjection,
  invalidatePreviewRefreshLease,
  postPreviewMessage as postPreviewMessageFromController,
  prepareCanvasProjectionNavigation as prepareCanvasProjectionNavigationFromController,
  previewReloadUrl as previewReloadUrlFromController,
  reconcileTemplateWorkbenchPreviewDocument as reconcileTemplateWorkbenchPreviewDocumentFromController,
  refreshRenderedPreviewDocument as refreshRenderedPreviewDocumentFromController,
  reloadPreview as reloadPreviewFromController,
  sendPreviewOperation as sendPreviewOperationFromController,
  type CanvasProjectionConfirmation,
  type PreviewControllerHost,
  type PreviewRefreshLease,
} from "$lib/state/preview-controller";
import {
  reconcileSelectionWithSourceDocument as reconcileSelectionWithSourceDocumentFromController,
  selectDomNode as selectDomNodeFromController,
  selectPreviewElement as selectPreviewElementFromController,
  setActiveCssSelector as setActiveCssSelectorFromController,
  type SelectionControllerHost,
} from "$lib/state/selection-controller";
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
  discardSessionAndReloadFromDisk as discardSessionAndReloadFromDiskFromController,
  initZolaProject as initZolaProjectFromController,
  loadScannedProjectFile as loadScannedProjectFileFromController,
  openCurrentProjectInBrowser as openCurrentProjectInBrowserFromController,
  openProjectFolder as openProjectFolderFromController,
  reattachCurrentProjectSession as reattachCurrentProjectSessionFromController,
  rescanCurrentProject as rescanCurrentProjectFromController,
  rescanCurrentProjectWithinKernelUndoRedoLease as rescanCurrentProjectWithinKernelUndoRedoLeaseFromController,
  rescanCurrentProjectWithinStructuralLane as rescanCurrentProjectWithinStructuralLaneFromController,
  resetProjectScopedState as resetProjectScopedStateFromController,
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
  moveLayerElement as moveLayerElementFromController,
  type LayersDragControllerHost,
} from "$lib/state/layers-drag-controller";
import {
  moveProjectFile as moveProjectFileFromController,
  type FilesDragControllerHost,
} from "$lib/state/files-drag-controller";
import {
  deleteProjectFile as deleteProjectFileFromController,
  renameProjectFile as renameProjectFileFromController,
  type FilesControllerHost,
  type ProjectEntryDeleteRequest,
  type ProjectEntryRenameRequest,
} from "$lib/state/files-controller";
import {
  type PreviewDragControllerHost,
} from "$lib/state/preview-drag-controller";
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
  moveTeraNodeAtTarget as moveTeraNodeAtTargetFromController,
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
import type { FileMoveRequest } from "$lib/project/files-drag";
import { scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import {
  cssRuleContextFromSource,
  type CssViewport,
} from "$lib/css/source-sync";
import { errorMessage } from "$lib/util";
import {
  createEmptyHtmlPending,
  createEmptyInspectorPending,
  initialUiTheme,
  type PreviewTemplateGate,
} from "$lib/state/app-helpers";
import { registerAppEffects } from "$lib/state/app-effects.svelte";
import {
  allowTemplateHtmlEditFromBridge as allowTemplateHtmlEditFromBridgeFromController,
  applySelectionState as applySelectionStateFromAppSelectionController,
  clearPreviewHtmlSelectionMarker as clearPreviewHtmlSelectionMarkerFromController,
  clearPreviewSelection as clearPreviewSelectionFromController,
  clearPreviewTeraSelection as clearPreviewTeraSelectionFromController,
  clearTemplateGateInPreview as clearTemplateGateInPreviewFromController,
  editSelectedTeraLayer as editSelectedTeraLayerFromController,
  hoverLayerSection as hoverLayerSectionFromController,
  hoverPreviewSelection as hoverPreviewSelectionFromController,
  openSelectedTeraSource as openSelectedTeraSourceFromController,
  previewDropGateStatus as previewDropGateStatusFromController,
  rememberSelectedElement as rememberSelectedElementFromController,
  renderPreviewSelectionToBridge as renderPreviewSelectionToBridgeFromController,
  requestTemplateHtmlEditPermission as requestTemplateHtmlEditPermissionFromController,
  selectLayerSection as selectLayerSectionFromController,
  selectPreviewTemplateElement as selectPreviewTemplateElementFromController,
  selectTemplateGateFromBridge as selectTemplateGateFromBridgeFromController,
  selectTeraLayerSource as selectTeraLayerSourceFromController,
  setPreviewTeraSelection as setPreviewTeraSelectionFromController,
  syncPreviewTeraGateState as syncPreviewTeraGateStateFromController,
  syncTemplateHtmlEditLock as syncTemplateHtmlEditLockFromController,
  templateGateContext as templateGateContextFromController,
  templateGateForPageSection as templateGateForPageSectionFromController,
  templateGateForPreviewClick as templateGateForPreviewClickFromController,
  templateGateForSelection as templateGateForSelectionFromController,
  templateGateForTeraSource as templateGateForTeraSourceFromController,
  templateGateSourceIdForSelection as templateGateSourceIdForSelectionFromController,
  hoverTeraLayerSource as hoverTeraLayerSourceFromController,
} from "$lib/state/app-selection-controller";
import {
  applyStagedOverrideStylesToPreview as applyStagedOverrideStylesToPreviewFromController,
  attachPreviewInspector as attachPreviewInspectorFromController,
  derivePreviewSelectionState as derivePreviewSelectionStateFromController,
  handlePreviewMessage as handlePreviewMessageFromController,
  previewUrlForScannedFile as previewUrlForScannedFileFromController,
  refreshSourceGraph as refreshSourceGraphFromController,
  resolveSourceEditLocationForSourceId as resolveSourceEditLocationForSourceIdFromController,
  resolveSourceEditTargetForSourceId as resolveSourceEditTargetForSourceIdFromController,
  syncHtmlCodeToPreview as syncHtmlCodeToPreviewFromController,
} from "$lib/state/app-preview-runtime-controller";
import {
  afterSave as afterSaveFromController,
  clearHtmlPending as clearHtmlPendingFromController,
  clearNotification as clearNotificationFromController,
  createProjectFile as createProjectFileFromController,
  dismissNotification as dismissNotificationFromController,
  handleNotificationAction as handleNotificationActionFromController,
  notify as notifyFromController,
  refreshCurrentSession as refreshCurrentSessionFromController,
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
  deriveSelectedSessionSourceLocation,
  deriveSelectedSourceEditTarget,
  deriveSelectedTemplateSourceNode,
  deriveSessionHasPending,
  deriveSourceLanguage,
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

const SELECTED_CLASS = "pana-studio-selected-element";
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

export class AppState {
  // Expose constants for template access
  readonly selectedClass = SELECTED_CLASS;

  // ── DOM refs (set by component via $effect) ──
  previewFrame = $state<HTMLIFrameElement | undefined>(undefined);
  codeEditorHost = $state<HTMLDivElement | undefined>(undefined);
  terminalHost = $state<HTMLDivElement | undefined>(undefined);

  // ── Editor / source state ──
  source = $state("");
  sourceCache = $state<Record<string, string>>({});
  /** Local UI edits only; used to reject stale asynchronous UI settlements. */
  editorMutationEpoch = $state(0);
  /** Durable Rust authority notifications; drives read-only workspace mirrors. */
  projectWorkspaceMutationEpoch = $state(0);
  selectionEpoch = $state(0);
  selectedPreviewElement: Element | null = null;
  selectedElement = $state<SelectionInfo | null>(null);
  lastMeaningfulSelectedElement = $state<SelectionInfo | null>(null);
  lastSelectedImageElement = $state<SelectionInfo | null>(null);
  selectedTemplateSourceId = $state<string | null>(null);
  selectedTemplatePreviewSelector = $state<string | null>(null);
  templateHtmlEditSourceId = $state<string | null>(null);

  // ── Element editor values ──
  attributeValues = $state<EditableAttributes>({});
  attributeStatus = $state("Atributele HTML pot fi editate direct in pagina activa.");
  textContentValue = $state("");
  activeHtmlTextEditKey = $state<string | null>(null);
  activeHtmlTextEditValue = $state<string | null>(null);
  textEditOriginalKey = $state<string | null>(null);
  textEditOriginalText = $state<string | null>(null);
  textStatus = $state("Textul poate fi editat pentru elemente simple, fara copii HTML.");
  classEditorValue = $state("");
  classStatus = $state("Clasele elementului pot fi editate direct in HTML.");
  imageSourceValue = $state("");
  imageStatus = $state("Sursa imaginii poate fi editata direct in HTML.");
  pendingTag = $state<string | null>(null);
  pendingTagOriginal = $state<string | null>(null);
	  pendingTagSourceLocation = $state<SourceEditLocation | null>(null);
  tagStatus = $state("");
  structureStatus = $state("Operatiile structurale se aplica pe pagina HTML activa.");
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
          throw new Error(result.reason ?? `Confirmarea draftului de text a fost ${result.status}.`);
        }
      },
      onError: (error, task) => {
        if (
          task.projectRoot !== this.sessionProjectRoot
          || task.runtimeSessionId !== this.kernelProjectSessionId
          || task.projectSessionEpoch !== this.projectSessionEpoch
        ) return;
        this.setGlobalStatus(
          `Draft-ul de text nu a putut fi confirmat de kernel: ${error instanceof Error ? error.message : String(error)}`,
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
  activeCssSelector = $state("");
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

  // ── Save / project state ──
  saveState = $state<SaveState>("idle");
  saveStatus = $state("Nicio modificare salvata in aceasta sesiune.");
  projectWorkspaceSnapshot = $state<ProjectWorkspaceSnapshot | null>(null);
  projectAuditSnapshot = $state<ProjectAuditSnapshot | null>(null);
  projectAuditLoading = $state(false);
  projectAuditError = $state("");
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
  jsRefreshToken = $state(0);
  scannedProject = $state<ProjectScan | null>(null);
  projectOpenRecoveryDecisionRequest = $state<ProjectOpenRecoveryDecisionRequest | null>(null);
  projectTransitionDecisionRequest = $state<ProjectTransitionDecisionRequest | null>(null);
  sourceGraph = $state<SourceGraph | null>(null);
  sourceGraphLoadSerial = 0;
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
  activeCanvasUrl = $state("about:blank");
  interactivePreviewEnabled = $state(false);
  interactivePreviewDomNodes = $state<InteractivePreviewDomNode[]>([]);
  canvasProjectionConfirmation: CanvasProjectionConfirmation | null = null;
  activeScannedPath = $state<string | null>(null);
  activePreviewPath = $state("about:blank");
  browserPreviewRoute = $state("/");
  previewDocumentMarkup = $state<string | null>(null);
  pageSections = $state<PageSection[]>([]);
  pendingSelectionSelector = $state<string | null>(null);
  latestPreviewMessageRevision = $state(0);
  controlledPreview = $state<ControlledPreviewState>(createControlledPreviewState());
  zolaValidationTimer: number | null = null;
  zolaValidationSerial = 0;
  templateWorkbenchPlan = $state<TemplateWorkbenchPlan | null>(null);
  templateWorkbenchPreferredPagePath = $state<string | null>(null);
  templateWorkbenchActive = $state(false);
  templateWorkbenchTarget = $state<string | null>(null);
  templateWorkbenchReturnPreviewPath = $state<string | null>(null);
  templateWorkbenchRequestSerial = 0;

  // ── UI state ──
  centerView = $state<CenterView>("preview");
  codeRevealTarget = $state<CodeRevealTarget>({ kind: "html" });
  codeSelectedCssTarget = $state<{ selector: string; file: string; revision: number } | null>(null);
  codeSelectedCssTargetRevision = 0;
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
  statusDismissTimer: number | null = null;
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
  previewSelection = $derived<PreviewSelectionState>(this.derivePreviewSelectionState());
  selectedSessionSourceLocation = $derived(deriveSelectedSessionSourceLocation(this));
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
      ? "Operațiile pe surse sunt blocate cât timp AI deține sau reconciliază autoritatea de editare."
      : this.externalDiskState.workspaceProjectionRecoveryRequired
      ? "Operațiile pe disc sunt blocate până la reîncărcarea explicită a proiecției externe."
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

  async restoreWorkbenchState() {
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    const snapshot = await this.workbenchController.refresh();
    if (
      !snapshot
      || !projectRoot
      || !runtimeSessionId
      || snapshot.projectRoot !== projectRoot
      || snapshot.runtimeSessionId !== runtimeSessionId
      || this.scannedProject?.root !== projectRoot
    ) return snapshot;

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
      this.notify({
        id: "workbench.restore.missing-document",
        level: "warning",
        title: "Documentul restaurat nu mai există",
        message: document.relativePath,
      });
    } else if (file) {
      await this.loadScannedProjectFile(file, {
        strict: true,
        skipDraftFlush: true,
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
      this.notify({
        id: "workbench.canvas-viewport",
        level: "warning",
        title: "Canvas-ul responsive nu a putut fi actualizat",
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
        throw new Error("Deschide un document înainte de a activa split view.");
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
      this.notify({
        id: "workbench.split",
        level: "warning",
        title: "Split view nu a putut fi actualizat",
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
      this.notify({
        id: "workbench.split-ratio",
        level: "warning",
        title: "Dividerul split nu a putut fi salvat",
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
      this.notify({
        id: "workbench.bottom-panel",
        level: "warning",
        title: "Panoul inferior nu a putut fi actualizat",
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

  markSelectionMutation() {
    this.selectionEpoch += 1;
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
      throw new Error("O altă mutație structurală deține deja bariera monitorului de disc.");
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
          "Monitorul discului nu a ajuns la o graniță curată înaintea mutației structurale.",
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

  setGlobalStatus(text: string, kind: SaveState) {
    setGlobalStatusFromAppSessionController(this, text, kind);
  }

  notify(notification: Omit<AppNotification, "createdAt">) {
    notifyFromController(this, notification);
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
        `Acțiunea „${notification.actionLabel ?? actionId}” a eșuat: ${errorMessage(error)}`,
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
          throw new Error("Auditul Rust a răspuns pentru altă identitate a sesiunii proiectului.");
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
        ) throw new Error("Inventarul de clase aparține altei revizii a sesiunii proiectului.");
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
    this.setGlobalStatus("Tranziția proiectului a fost anulată de operator.", "idle");
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
    this.terminalController.destroyAll();
    this.terminalTabs = initialTerminalTabs();
    this.activeTerminalTabId = "terminal-shell-1";
    this.terminalTabSerial = 1;
    this.terminalPaneOpen = false;
  }

  async initZolaProject(themeId: string) {
    await initZolaProjectFromController(this.projectControllerHost(), themeId);
  }

  resetProjectScopedState() {
    resetProjectScopedStateFromController(this.projectControllerHost());
    this.workbenchController.reset();
    this.workbenchHydratedRuntimeSessionId = "";
  }

  async rescanCurrentProject(
    preferredRelativePath: string | null = this.activeScannedPath,
    options: { strict?: boolean } = {},
  ) {
    await rescanCurrentProjectFromController(
      this.projectControllerHost(),
      preferredRelativePath,
      options,
    );
  }

  async rescanCurrentProjectWithinStructuralLane(
    lease: import("$lib/kernel/preview-structural-lane").PreviewStructuralSessionLease,
    preferredRelativePath: string | null = this.activeScannedPath,
    options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
  ) {
    await rescanCurrentProjectWithinStructuralLaneFromController(
      this.projectControllerHost(),
      lease,
      preferredRelativePath,
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
      } catch (error) {
        this.notify({
          id: "workbench.document-sync",
          level: "warning",
          title: "Workbench nesincronizat",
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
        "Context de template activ nu mai are un template selectat în ProjectSession.",
      );
    }
    await this.updateTemplateWorkbenchContext(
      project,
      templateFile,
      this.templateWorkbenchPreferredPagePath,
      { minimumWorkspaceRevision, strict: true },
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
        `CanvasPatch ${patch.patchId} a fost retras după eșecul proiecției canonice.`,
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
      this.projectStatus || "Previzualizarea nu a confirmat generația curentă a sesiunii proiectului.",
    );
  }

  canProjectWorkspacePreview() {
    return Boolean(
      this.previewFrame?.contentWindow
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
    this.pendingCanvasProjection = null;
    this.activeCanvasIdentity = null;
    this.activeCanvasUrl = "about:blank";
    this.interactivePreviewEnabled = false;
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
        throw new Error("Kernelul a confirmat alt eveniment Canvas Runtime.");
      }
    } catch (error) {
      if (this.activeCanvasIdentity?.transactionId !== identity.transactionId) return;
      this.setGlobalStatus(
        `Observability Canvas Runtime a eșuat: ${errorMessage(error)}`,
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

  async refreshSourceGraph(options: { strict?: boolean } = {}) {
    await refreshSourceGraphFromController(this, options);
  }

  resolveSourceEditTargetForSourceId(sourceId: string | null | undefined) {
    return resolveSourceEditTargetForSourceIdFromController(this, sourceId);
  }

  resolveSourceEditLocationForSourceId(sourceId: string | null | undefined) {
    return resolveSourceEditLocationForSourceIdFromController(this, sourceId);
  }

  derivePreviewSelectionState(): PreviewSelectionState {
    return derivePreviewSelectionStateFromController(this);
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
      throw new Error("Proiecția CSS live a primit un receipt din altă ProjectSession.");
    }
    if (
      authority.schemaVersion !== 2
      || authority.documents.map((projection) => projection.relativePath).join("\u0000")
        !== authority.touchedFiles.join("\u0000")
      || (authority.status === "noop" && authority.documents.length !== 0)
    ) {
      throw new Error("Proiecția CSS a primit documente care nu aparțin receipt-ului canonic.");
    }

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

    const mutation = authority.workspaceMutation;
    const transactionId = mutation?.transactionId?.trim() ?? "";
    if (
      authority.status !== "staged"
      || !mutation?.changed
      || mutation.revisionBefore !== authority.revisionBefore
      || mutation.revisionAfter !== authority.revisionAfter
      || !transactionId
    ) {
      throw new Error("Proiecția CSS live nu are o tranzacție exactă a sesiunii proiectului.");
    }

    let boundIdentity: InspectorLiveCssIdentity | null = null;
    await projectLatestProjectWorkspacePreview(this, {
      reason: "workspace-mutation",
      minimumWorkspaceRevision: authority.revisionAfter,
      requestedPaths: authority.touchedFiles,
      expectedWorkspaceRevision: authority.revisionAfter,
      expectedWorkspaceTransactionId: transactionId,
      onCanvasPlanPrepared: (plan) => {
        if (plan.workspaceTransactionId !== transactionId) {
          throw new Error("Planul Canvas CSS nu aparține tranzacției confirmate a sesiunii proiectului.");
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
    this.scssVariables = await getScssVariables(identity).catch(() => (
      this.scssVariables.map((entry) => (
        entry.file === variable.file && entry.name === variable.name
          ? { ...entry, value: nextValue }
          : entry
      ))
    ));
    this.setGlobalStatus(`Tokenul $${variable.name} a fost actualizat în ProjectWorkspace.`, "unsaved");
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
    this.scssVariables = await getScssVariables(identity);
    this.setGlobalStatus(`Tokenul $${name.replace(/^\$/, "")} a fost creat în ProjectWorkspace.`, "unsaved");
    return true;
  }

  async createDesignSystemClass(name: string, relativePath: string): Promise<boolean> {
    const outcome = await runInPreviewStructuralLane(this, async (lease) => {
      const receipt = await createDesignClassCommand(name, relativePath, {
        expectedProjectRoot: lease.projectRoot,
        expectedSessionId: lease.sessionId,
      });
      requireCurrentPreviewStructuralSession(this, lease);
      this.projectWorkspaceSnapshot = receipt.workspace;
      await this.rescanCurrentProjectWithinStructuralLane(
        lease,
        relativePath,
        { strict: true },
      );
      requireCurrentPreviewStructuralSession(this, lease);
      await this.refreshDesignClassInventory(true);
      requireCurrentPreviewStructuralSession(this, lease);
      this.setGlobalStatus(
        `Clasa .${name.replace(/^\./, "")} a fost creată în ${relativePath}.`,
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
      if (
        receipt.workspace.projectRoot !== lease.projectRoot
        || receipt.workspace.runtimeSessionId !== lease.sessionId
        || receipt.workspace.workspace.revision !== receipt.workspace.mutation.revisionAfter
      ) throw new Error("Redenumirea clasei a primit o confirmare inconsistentă a sesiunii proiectului.");

      this.projectWorkspaceSnapshot = receipt.workspace.workspace;
      await this.rescanCurrentProjectWithinStructuralLane(
        lease,
        this.activeScannedPath,
        { strict: true },
      );
      requireCurrentPreviewStructuralSession(this, lease);
      await this.refreshDesignClassInventory(true);
      requireCurrentPreviewStructuralSession(this, lease);
      this.setGlobalStatus(
        `Clasa .${receipt.oldName} a devenit .${receipt.newName} în ${receipt.changedFiles.length} fișiere (${receipt.replacementCount} referințe).`,
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

  openPreviewContextMenu(data: Record<string, unknown>) {
    const selection = (data.selection ?? null) as SelectionInfo | null;
    if (!selection) return;

    const frameRect = this.previewFrame?.getBoundingClientRect();
    const clientX = typeof data.clientX === "number" ? data.clientX : 0;
    const clientY = typeof data.clientY === "number" ? data.clientY : 0;
    let x = clientX;
    let y = clientY;
    if (frameRect) {
      const viewportWidth = typeof data.viewportWidth === "number" && data.viewportWidth > 0
        ? data.viewportWidth
        : frameRect.width || 1;
      const viewportHeight = typeof data.viewportHeight === "number" && data.viewportHeight > 0
        ? data.viewportHeight
        : frameRect.height || 1;
      x = frameRect.left + clientX * (frameRect.width / viewportWidth);
      y = frameRect.top + clientY * (frameRect.height / viewportHeight);
    }

    const templateGate = this.templateGateForSelection(selection);
    if (templateGate && templateGate.sourceId !== this.templateHtmlEditSourceId) {
      this.setPreviewTeraSelection(templateGate, {
        status: templateEditGateSelectionStatus(templateGate.canSelectHtml, "element"),
      });
      const sourceNode = this.sourceGraph?.nodes.find((node) => node.id === templateGate.sourceId) ?? null;
      const title = sourceNode
        ? `${sourceNode.kind}: ${sourceNode.label}`
        : "Gate Tera";
      contextMenu.open({
        source: "preview",
        x,
        y,
        title,
        subtitle: sourceNode?.file ?? templateGate.selector,
        items: teraContextMenuItems(
          this.editorRuntime,
          teraTargetFromGate(templateGate, {
            label: sourceNode?.label ?? "Gate Tera",
            kindLabel: sourceNode?.kind ?? "Tera",
            file: sourceNode?.file ?? null,
            sourceNode,
          }),
          "preview",
        ),
      });
      return;
    }

    this.applySelectionState(selection);
    this.syncTemplateHtmlEditLock(selection);

    const target = htmlTargetFromSelection(selection);
    contextMenu.open({
      source: "preview",
      x,
      y,
      title: selection.selector || `<${selection.tag}>`,
      subtitle: selection.text,
      items: htmlElementContextMenuItems(this.editorRuntime, target, "preview"),
    });
  }

  openLayerContextMenu(request: EditorLayerContextMenuRequest) {
    if (request.kind === "html") {
      const gate = this.templateGateForPageSection(request.section);
      if (gate && gate.sourceId !== this.templateHtmlEditSourceId) {
        this.setPreviewTeraSelection(gate, {
          status: templateEditGateSelectionStatus(gate.canSelectHtml, "zone"),
        });
        const sourceNode = this.sourceGraph?.nodes.find((node) => node.id === gate.sourceId) ?? null;
        contextMenu.open({
          source: "layers",
          x: request.x,
          y: request.y,
          title: sourceNode ? `${sourceNode.kind}: ${sourceNode.label}` : "Gate Tera",
          subtitle: sourceNode?.file ?? gate.selector,
          items: teraContextMenuItems(
            this.editorRuntime,
            teraTargetFromGate(gate, {
              label: sourceNode?.label ?? "Gate Tera",
              kindLabel: sourceNode?.kind ?? "Tera",
              file: sourceNode?.file ?? null,
              sourceNode,
            }),
            "layers",
          ),
        });
        return;
      }

      this.selectDomNode(request.section.selector);
      const target = htmlTargetFromPageSection(request.section, request.label);
      contextMenu.open({
        source: "layers",
        x: request.x,
        y: request.y,
        title: `<${request.section.tag}> ${request.label ?? request.section.label}`,
        subtitle: request.section.selector,
        items: htmlElementContextMenuItems(this.editorRuntime, target, "layers", {
          selectLabel: "Selecteaza stratul",
        }),
      });
      return;
    }

    this.selectTeraLayerSource(request.section, request.sourceId);
    const sourceNode = this.sourceGraph?.nodes.find((node) => node.id === request.sourceId) ?? null;
    contextMenu.open({
      source: "layers",
      x: request.x,
      y: request.y,
      title: `${request.kindLabel ?? "Tera"}: ${request.label ?? "Sursă Tera"}`,
      subtitle: request.file ?? request.sourceId,
      items: teraContextMenuItems(
        this.editorRuntime,
        {
          kind: "tera",
          sourceId: request.sourceId,
          selector: request.selector,
          label: request.label,
          kindLabel: request.kindLabel,
          file: request.file ?? null,
          origin: request.origin ?? null,
          themeName: request.themeName ?? null,
          canSelectHtml: sourceNode ? canRequestTemplateEditGateKind(sourceNode.kind) : undefined,
          section: request.section,
          sourceNode,
        },
        "layers",
      ),
    });
  }

  previewDragControllerHost(): PreviewDragControllerHost {
    return this;
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
        this.setGlobalStatus(`Schimbarea workspace-ului a fost blocată: ${errorMessage(error)}`, "error");
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
        this.notify({
          id: "workbench.activity-sync",
          level: "warning",
          title: "Activitatea nu a putut fi schimbată",
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
        this.notify({
          id: "workbench.surface-sync",
          level: "warning",
          title: "Modul documentului nu a fost persistat",
          message: errorMessage(error),
        });
      }
    }
    if (enteringPreview && this.scannedProject?.isZola) {
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
              `Ultima revizie ProjectWorkspace nu a putut fi proiectată în Preview: ${errorMessage(error)}`,
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
      throw new Error("Previzualizarea versiunii aparține unei sesiuni de proiect depășite.");
    }
    await this.flushInteractiveEditorDrafts("template-switch");
    invalidatePreviewRefreshLease(this);
    this.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
    this.interactivePreviewEnabled = false;
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
    this.activeCssSelector = target.selector;
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

  selectCssSelectorFromCode(target: { selector: string; file: string }) {
    if (!target.selector || !target.file) return;
    this.setCssCodeRevealTarget(target);
    this.codeSelectedCssTargetRevision += 1;
    this.codeSelectedCssTarget = {
      selector: target.selector,
      file: target.file,
      revision: this.codeSelectedCssTargetRevision,
    };
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
    if (!this.scannedProject || !this.selectedElement?.sourceLocation?.file) return;
    const sourceFile = this.selectedElement.sourceLocation.file;
    const targetPath = zolaRelativePath(sourceFile);
    const file = this.scannedProject.files.find(
      (item) => item.relativePath === sourceFile || zolaRelativePath(item.relativePath) === targetPath,
    );
    if (!file || this.activeScannedPath === file.relativePath) return;

    const selectedElement = this.selectedElement;
    const lastMeaningfulSelectedElement = this.lastMeaningfulSelectedElement;
    const lastSelectedImageElement = this.lastSelectedImageElement;
    await this.loadScannedProjectFile(file);
    this.selectedElement = selectedElement;
    this.lastMeaningfulSelectedElement = lastMeaningfulSelectedElement;
    this.lastSelectedImageElement = lastSelectedImageElement;
  }

  async createCodeEditor() {
    await createSourceEditorFromController(this.sourceEditorControllerHost());
  }

  handleCodeCursorSelection(position: number, sourceText: string) {
    handleCodeCursorSelectionFromController(this.sourceEditorControllerHost(), position, sourceText);
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
      title: this.currentSourcePath || "Cod sursa",
      subtitle: `Linia ${request.line}, coloana ${request.column}`,
      items: [
        {
          id: "save-source",
          label: "Salveaza",
          shortcut: "Ctrl+S",
          disabled: !this.saveHasPending,
          action: async () => {
            await this.saveActiveFile();
          },
        },
        {
          id: "select-html-at-cursor",
          label: "Selecteaza nodul HTML de la cursor",
          disabled: this.sourceLanguage !== "html",
          separatorBefore: true,
          action: () => this.handleCodeCursorSelection(request.position, request.docText),
        },
        {
          id: "reveal-current-selection",
          label: "Arata selectia curenta in cod",
          disabled: !this.selectedElement,
          action: () => this.syncCodeSelectionHighlight(true),
        },
        {
          id: "copy-code-selection",
          label: "Copiaza selectia",
          disabled: !request.hasSelection,
          separatorBefore: true,
          action: async () => {
            if (!request.selectedText) return;
            await navigator.clipboard?.writeText(request.selectedText);
            this.setGlobalStatus("Selectia din cod a fost copiata.", "idle");
          },
        },
      ],
    });
  }

  sourceEditorControllerHost(): SourceEditorControllerHost {
    return this;
  }

  // ── Selection ─────────────────────────────────────────────────────────────

  clearPreviewHtmlSelectionMarker() {
    clearPreviewHtmlSelectionMarkerFromController(this);
  }

  renderPreviewSelectionToBridge(selection: PreviewSelectionState = this.previewSelection) {
    renderPreviewSelectionToBridgeFromController(this, selection);
  }

  syncPreviewTeraGateState() {
    syncPreviewTeraGateStateFromController(this);
  }

  clearPreviewTeraSelection() {
    clearPreviewTeraSelectionFromController(this);
  }

  clearPreviewSelection(options: { clearTemplateGate?: boolean; clearHtmlMarker?: boolean } = {}) {
    clearPreviewSelectionFromController(this, options);
  }

  setPreviewTeraSelection(
    gate: PreviewTemplateGate,
    options: { status?: string; showGate?: boolean; clearHtmlMarker?: boolean } = {},
  ) {
    setPreviewTeraSelectionFromController(this, gate, options);
  }

  applySelectionState(
    selection: SelectionInfo,
    resolvedStyles?: EditableStyles,
    markUserMutation = true,
  ) {
    if (markUserMutation) this.markSelectionMutation();
    applySelectionStateFromAppSelectionController(this, selection, resolvedStyles);
  }

  rememberSelectedElement(selection = this.selectedElement) {
    rememberSelectedElementFromController(this, selection);
  }

  templateGateContext() {
    return templateGateContextFromController(this);
  }

  templateGateForSelection(selection: SelectionInfo): PreviewTemplateGate | null {
    return templateGateForSelectionFromController(this, selection);
  }

  templateGateForPageSection(section: PageSection): PreviewTemplateGate | null {
    return templateGateForPageSectionFromController(this, section);
  }

  templateGateForPreviewClick(element: Element): (PreviewTemplateGate & { element: Element }) | null {
    return templateGateForPreviewClickFromController(this, element);
  }

  templateGateForTeraSource(sourceId: string | null | undefined, selector: string | null | undefined): PreviewTemplateGate | null {
    return templateGateForTeraSourceFromController(this, sourceId, selector);
  }

  templateGateSourceIdForSelection(selection: SelectionInfo) {
    return templateGateSourceIdForSelectionFromController(this, selection);
  }

  syncTemplateHtmlEditLock(selection: SelectionInfo | null) {
    syncTemplateHtmlEditLockFromController(this, selection);
  }

  previewDropGateStatus(target: { targetSourceId?: string | null; targetTemplateSourceId?: string | null }) {
    return previewDropGateStatusFromController(this, target);
  }

  selectTemplateGateFromBridge(data: Record<string, unknown>) {
    selectTemplateGateFromBridgeFromController(this, data);
  }

  async allowTemplateHtmlEditFromBridge(data: Record<string, unknown>) {
    await allowTemplateHtmlEditFromBridgeFromController(this, data);
  }

  async allowTemplateHtmlEdit(sourceId: string | null, selector: string | null) {
    await requestTemplateHtmlEditPermissionFromController(this, sourceId, selector);
  }

  async editSelectedTeraLayer() {
    await editSelectedTeraLayerFromController(this);
  }

  async openSelectedTeraSource() {
    await openSelectedTeraSourceFromController(this);
  }

  selectLayerSection(section: PageSection) {
    selectLayerSectionFromController(this, section);
  }

  selectTeraLayerSource(section: PageSection, sourceId: string) {
    selectTeraLayerSourceFromController(this, section, sourceId);
  }

  hoverLayerSection(section: PageSection | null) {
    hoverLayerSectionFromController(this, section);
  }

  hoverTeraLayerSource(section: PageSection, sourceId: string) {
    hoverTeraLayerSourceFromController(this, section, sourceId);
  }

  hoverPreviewSelection(selection: SelectionInfo | null) {
    hoverPreviewSelectionFromController(this, selection);
  }

  clearTemplateGateInPreview() {
    clearTemplateGateInPreviewFromController(this);
  }

  selectPreviewTemplateElement(
    element: Element,
    gate: PreviewTemplateGate,
  ) {
    selectPreviewTemplateElementFromController(this, element, gate);
  }

  setActiveCssSelector(selector: string) {
    setActiveCssSelectorFromController(this.selectionControllerHost(), selector);
  }

  selectPreviewElement(element: Element, options: { revealCode?: boolean } = {}) {
    this.setHtmlCodeRevealTarget();
    selectPreviewElementFromController(this.selectionControllerHost(), element, options);
    this.syncTemplateHtmlEditLock(this.selectedElement);
    this.rememberSelectedElement();
  }

  selectDomNode(selector: string, options: { revealCode?: boolean } = {}) {
    this.setHtmlCodeRevealTarget();
    selectDomNodeFromController(this.selectionControllerHost(), selector, options);
    this.syncTemplateHtmlEditLock(this.selectedElement);
    this.rememberSelectedElement();
  }

  reconcileSelectionWithSourceDocument(document: Document, preferredSelector: string | null = null) {
    reconcileSelectionWithSourceDocumentFromController(this.selectionControllerHost(), document, preferredSelector);
    this.rememberSelectedElement();
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
    const selection = this.selectedElement;
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
      if (!ack.ok) throw new Error(ack.error || "Previzualizarea a refuzat ciorna live de atribute.");
      if (!isLatestHtmlAttributeDraftSettlement(
        this.activeHtmlAttributeEditSession?.id ?? null,
        this.activeHtmlAttributeEditSession?.latestLiveEpoch ?? -1,
        session.id,
        draftEpoch,
      )) return;
      this.attributeStatus = "Ciorna atributelor este confirmată de Canvas; starea canonică rămâne în sesiunea proiectului.";
    }).catch((error) => {
      if (isLatestHtmlAttributeDraftSettlement(
        this.activeHtmlAttributeEditSession?.id ?? null,
        this.activeHtmlAttributeEditSession?.latestLiveEpoch ?? -1,
        session.id,
        draftEpoch,
      )) {
        this.attributeStatus = `Proiecția live a atributelor a eșuat: ${error instanceof Error ? error.message : String(error)}`;
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
    const currentTarget = captureHtmlActionTarget(this.selectedElement);
    if (currentTarget && this.htmlAssetEditContextKey(currentTarget) === sessionContextKey) {
      this.attributeValues = { ...session.baselineAttributes };
    }
    this.cancelActiveHtmlAttributeEditSession();
    this.setHtmlPending("attributes", false);
    this.attributeStatus = "Editarea atributelor a fost anulată; sursa canonică nu a fost modificată.";
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
        if (!ack.ok) throw new Error(ack.error || "Canvas nu a confirmat închiderea draftului.");
      } catch (error) {
        this.attributeStatus = `Sursa a fost confirmată, dar Canvas nu a confirmat închiderea draftului: ${error instanceof Error ? error.message : String(error)}`;
      }
      if (this.activeHtmlAttributeEditSession?.id !== session.id) return result;
      if (session.latestLiveEpoch !== submittedLiveEpoch) continue;

      this.activeHtmlAttributeEditSession = null;
      this.setHtmlPending("attributes", false);
      this.attributeStatus = result.status === "noop"
        ? "Atributele nu au modificări de aplicat."
        : "Atribute confirmate în sesiunea proiectului și proiectate canonic.";
      return result;
    }
    return null;
  }

  private captureActiveHtmlTextEditSession(value: string): ActiveHtmlTextEditSession | null {
    const selection = this.selectedElement;
    const target = captureHtmlActionTarget(selection);
    const projectRoot = this.sessionProjectRoot;
    const runtimeSessionId = this.kernelProjectSessionId;
    if (
      !selection
      || selection.hasChildElements
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
          `Textul este recuperabil în sesiune, dar proiecția canonică a eșuat: ${errorMessage(error)}`,
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
        this.setGlobalStatus(`Închiderea editării textului a eșuat: ${errorMessage(error)}`, "error");
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
        await this.refreshSourceGraph({ strict: true });
        if (this.activeHtmlTextEditSession?.id !== editSessionId) return;
        await projectLatestProjectWorkspacePreview(this, {
          reason: "workspace-mutation",
          requestedPaths: session.target.sourceLocation?.file
            ? [session.target.sourceLocation.file]
            : undefined,
        });
        if (this.activeHtmlTextEditSession?.id === editSessionId) {
          session.projectedText = projectedText;
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
    this.textStatus = "Text confirmat în sesiunea proiectului și proiectat canonic.";
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

  async moveTeraNodeAtTarget(request: TeraMoveRequest) {
    return await moveTeraNodeAtTargetFromController(this.teraActionsControllerHost(), request);
  }

  async moveLayerElement(request: LayerMoveRequest) {
    return await moveLayerElementFromController(this.layersDragControllerHost(), request);
  }

  async deleteHtmlElement(selector?: string | null) {
    const section = selector ? this.pageSections.find((item) => item.selector === selector) : null;
    const target = section
      ? htmlTargetFromPageSection(section)
      : this.selectedElement
        ? htmlTargetFromSelection(this.selectedElement)
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
      : this.selectedElement
        ? htmlTargetFromSelection(this.selectedElement)
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

  async moveProjectFile(request: FileMoveRequest) {
    await moveProjectFileFromController(this.filesDragControllerHost(), request);
  }

  async deleteProjectFile(request: ProjectEntryDeleteRequest) {
    await deleteProjectFileFromController(this.filesControllerHost(), request);
  }

  async renameProjectFile(request: ProjectEntryRenameRequest) {
    return renameProjectFileFromController(this.filesControllerHost(), request);
  }

  async applyImageSourceToHtml(src?: string) {
    if ((!this.selectedElement || this.selectedElement.tag !== "img") && this.lastSelectedImageElement) {
      this.selectedElement = this.lastSelectedImageElement;
    }
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
      return noopAction("Atributele sunt deja confirmate de sesiunea proiectului.");
    }
    return await applyAttributesToHtmlFromController(this.htmlActionsControllerHost(), attributes);
  }

  async applyTextContentToHtml() {
    const committed = await this.finishActiveHtmlTextEditSession();
    if (!committed) {
      return noopAction("Textul este deja confirmat de sesiunea proiectului.");
    }
    return committedAction();
  }

  htmlActionsControllerHost(): HtmlActionsControllerHost {
    return this;
  }

  layersDragControllerHost(): LayersDragControllerHost {
    return this;
  }

  filesDragControllerHost(): FilesDragControllerHost {
    return this;
  }

  filesControllerHost(): FilesControllerHost {
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
    ) throw new Error("Documentul metadata aparține unei sesiuni care nu mai este activă.");
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
    this.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
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
        "Salvarea HTML este blocată cât timp AI deține sau reconciliază autoritatea de editare.",
      );
    }
    if (this.blockSaveForExternalProjectionConflict()) {
      return blockedAction(
        "Salvarea HTML este blocată până când conflictul proiecției externe este reconciliat.",
      );
    }
    if (this.blockSaveForKernelUndoRedoLease()) {
      return blockedAction(
        "Salvarea HTML este blocată cât timp anularea sau refacerea rezervă sesiunea curentă.",
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
        "Salvarea este temporar blocată: tranziția proiectului a rezervat sesiunea curentă.",
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
            "Salvarea este blocată: monitorul extern a detectat o stare pe disc care trebuie reconciliată înainte de scriere.",
            "error",
          );
          return false;
        }
        return await saveActiveDocument(this.saveControllerHost());
      } catch (error) {
        this.setGlobalStatus(
          `Save nu a putut obține bariera monitorului disk: ${errorMessage(error)}`,
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
      throw new Error("Undo/Redo este blocat cât timp AI deține autoritatea de editare.");
    }
    if (this.projectTransitionFrontendLeaseActive) {
      throw new Error(
        "Anularea sau refacerea nu poate porni cât timp tranziția proiectului rezervă sesiunea.",
      );
    }
    if (this.kernelUndoRedoFrontendLeaseActive) {
      throw new Error("O altă operație Undo/Redo rezervă deja sesiunea.");
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
          "Monitorul discului nu a ajuns la o graniță curată înainte de anulare sau refacere.",
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
        "Tranziția proiectului este blocată cât timp AI deține sau reconciliază autoritatea de editare.",
      );
    }
    if (this.kernelUndoRedoFrontendLeaseActive) {
      throw new Error(
        "Tranziția proiectului este blocată cât timp anularea sau refacerea finalizează proiecția curentă.",
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
      "Salvarea este blocată: proiecția UI s-a schimbat în timpul reconcilierii externe. Folosește «Reîncarcă de pe disc» înainte de orice scriere.",
      "error",
    );
    return true;
  }

  private blockSaveForAiLease() {
    if (!this.aiEditLeaseFrontendLockActive) return false;
    this.setGlobalStatus(
      "Salvarea este blocată: AI deține sau reconciliază autoritatea de editare a surselor.",
      "error",
    );
    return true;
  }

  private blockSaveForKernelUndoRedoLease() {
    if (!this.kernelUndoRedoFrontendLeaseActive) return false;
    this.setGlobalStatus(
      "Salvarea este temporar blocată: anularea sau refacerea rezervă sesiunea curentă.",
      "error",
    );
    return true;
  }

  // ── UI ────────────────────────────────────────────────────────────────────

  toggleUiTheme() {
    toggleUiThemeFromController(this.uiControllerHost());
    void this.persistApplicationTheme(this.uiTheme);
  }

  setApplicationTheme(theme: ApplicationTheme) {
    if (theme === this.uiTheme) return;
    setUiThemeFromController(this.uiControllerHost(), theme);
    void this.persistApplicationTheme(theme);
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
      this.applicationSettings = snapshot;
      if (snapshot.initialized) {
        setUiThemeFromController(this.uiControllerHost(), snapshot.theme);
      } else {
        await this.persistApplicationTheme(this.uiTheme);
      }
    } catch (error) {
      this.notify({
        id: "application.settings.load",
        level: "warning",
        title: "Setările aplicației nu au fost încărcate",
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      this.applicationSettingsLoading = false;
    }
  }

  private persistApplicationTheme(theme: ApplicationTheme) {
    const operation = this.applicationSettingsSaveTail.then(async () => {
      const current = this.applicationSettings ?? await readApplicationSettings();
      const snapshot = await saveApplicationSettings(
        current.revision,
        theme,
        current.blockPropertiesHeight,
        current.blockPropertiesCollapsed,
      );
      this.applicationSettings = snapshot;
      this.clearNotification("application.settings.load");
      this.clearNotification("application.settings.save");
    });
    this.applicationSettingsSaveTail = operation.then(
      () => undefined,
      (error) => {
        this.notify({
          id: "application.settings.save",
          level: "warning",
          title: "Tema nu a fost salvată în configurația aplicației",
          message: error instanceof Error ? error.message : String(error),
        });
      },
    );
    return this.applicationSettingsSaveTail;
  }

  persistBlockPropertiesLayout(height: number, collapsed: boolean) {
    const normalizedHeight = Math.max(140, Math.min(520, Math.round(height)));
    const operation = this.applicationSettingsSaveTail.then(async () => {
      const current = this.applicationSettings ?? await readApplicationSettings();
      if (
        current.blockPropertiesHeight === normalizedHeight
        && current.blockPropertiesCollapsed === collapsed
      ) return;
      const snapshot = await saveApplicationSettings(
        current.revision,
        current.theme,
        normalizedHeight,
        collapsed,
      );
      this.applicationSettings = snapshot;
      this.clearNotification("application.settings.save");
    });
    this.applicationSettingsSaveTail = operation.then(
      () => undefined,
      (error) => {
        this.notify({
          id: "application.settings.save",
          level: "warning",
          title: "Layout-ul Inspectorului nu a fost salvat",
          message: error instanceof Error ? error.message : String(error),
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

  // ── File creation ─────────────────────────────────────────────────────────

  async createProjectFile(relativePath: string, content: string) {
    await createProjectFileFromController(this, relativePath, content);
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

  // ── After-save callback ───────────────────────────────────────────────────

  async afterSave() {
    await afterSaveFromController(this);
  }
}
