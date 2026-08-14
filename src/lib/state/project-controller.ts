import { tick } from "svelte";
import { SOURCE_LOADING_SENTINEL } from "$lib/editor-runtime/source-state";
import { t } from "$lib/i18n/runtime.svelte";
import type { PreviewRefreshReason } from "$lib/preview/controlled";
import { openUrl as openExternalUrl } from "@tauri-apps/plugin-opener";
import {
  reanchorFileBufferDraftSyncCursor,
  resetFileBufferDraftSyncState,
  setFileBufferDraftSyncSession,
} from "$lib/session/file-buffer-draft-sync";
import {
  resetPageJsDraftSyncState,
  setPageJsDraftSyncSession,
} from "$lib/session/page-js-draft-sync";
import {
  flushWorkspaceMutationInputs,
  settleProjectWorkspaceMutation,
  type WorkspaceDerivedProjectionStatus,
  type WorkspaceDerivedReconciliationOutcome,
} from "$lib/session/workspace-mutation-coordinator";
import { createDiskState, diskStateFromProjectScan, markDiskMutation, type DiskState } from "$lib/session/disk-state";
import {
  closeProject,
  createCssRequestIdentity,
  createProjectPreviewRequestIdentity,
  createProjectContentPage,
  chooseProjectFolder,
  cancelProjectOpen,
  acknowledgeProjectFrontendHydrated,
  getScssVariables,
  inspectStartupFolder,
  inspectProjectOpen,
  openProject,
  planStartupCreation,
  applyStartupCreation,
  readStartupCreationCatalog,
  readProjectLifecycle,
  reportProjectCapabilityDegraded,
  readFileBufferText,
  readProjectWorkspaceState,
  readKernelProjectTransitionPolicy,
  recordProjectTransitionOperatorDecision,
  projectTemplateWorkbenchPreview,
  projectPreviewRequestIdentityMatches,
  requireProjectPreviewStartReceipt,
  reattachProjectSession,
  scanProject,
  startProjectBrowserPreview,
  startProjectPreview,
  type BrowserPreviewRequestIdentity,
  type BrowserPreviewStartReceipt,
  type CanvasProjectionIdentity,
  type CanvasProjectionPlan,
  type ProjectPreviewRequestIdentity,
  type ProjectPreviewStartReceipt,
  type TemplateWorkbenchPreviewRequest,
} from "$lib/project/io";
import {
  planContentPageCreation,
  planOpenedProject,
  planScannedProjectFileLoad,
  preservePreviewBaseUrl,
  selectProjectFileAfterScan,
} from "$lib/project/session";
import {
  createProjectOpenRecoveryDecisionRequest,
  projectOpenRecoveryAbandonDecision,
  PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
  type ProjectOpenRecoveryDecisionRequest,
} from "$lib/project/open-recovery";
import {
  createProjectTransitionDecisionRequest,
  localizedTransitionPolicyCopy,
  PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID,
  PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
  projectTransitionActionForContinuation,
  type ProjectTransitionContinuation,
  type ProjectTransitionDecisionRequest,
} from "$lib/project/transition-decision";
import type {
  ApplicationSurface,
  CenterView,
  HtmlPendingArea,
  InspectorPendingArea,
  ProjectFile,
  ProjectBootstrapInitialSurface,
  ProjectBootstrapSourceLocation,
  ProjectOpenRecoveryDecisionInput,
  ProjectOpenInspectionReceipt,
  ProjectOpenBootstrapReceipt,
  ProjectLifecycleSnapshot,
  ProjectScan,
  StartupCandidateSnapshot,
  StartupCreationCatalog,
    StartupCreationPlan,
  StartupFlowSnapshot,
  ScssVariable,
  SourceEditLocation,
  SourceGraph,
  ProjectWorkspaceSnapshot,
  WorkbenchSnapshot,
    InspectorSelectionSummarySnapshot,
    SelectionSnapshot,
    HoverSnapshot,
    TemplateWorkbenchPlan,
} from "$lib/types";
import type {
  GlobalStatusEscalationRequest,
  GlobalStatusKind,
} from "$lib/status/global-status";
import { errorMessage } from "$lib/util";
import {
  markProjectWorkspacePreviewPublished,
  projectLatestProjectWorkspacePreview,
  resetProjectWorkspacePreviewCoordinator,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  isCanvasProjectionSurfaceUnavailableError,
} from "$lib/state/preview-controller";
import {
  previewStructuralCommandIdentity,
  previewStructuralSessionLeaseMatches,
  requireCurrentPreviewStructuralSession,
  runInPreviewStructuralLane,
} from "$lib/kernel/preview-structural-lane";

type OpenProjectRootOptions = {
  operatorDecisionId?: string | null;
  recoveryDecision?: ProjectOpenRecoveryDecisionInput | null;
  startupCandidate?: StartupCandidateSnapshot | null;
  inspection?: ProjectOpenInspectionReceipt | null;
};

export type BrowserPreviewDependencies = {
  start: (identity: BrowserPreviewRequestIdentity) => Promise<BrowserPreviewStartReceipt | null>;
  openUrl: (url: string) => Promise<void>;
};

export type BrowserPreviewOpenOptions = {
  route?: string | null;
};

const browserPreviewDependencies: BrowserPreviewDependencies = {
  start: startProjectBrowserPreview,
  openUrl: openExternalUrl,
};

export type ProjectPreviewDependencies = {
  start: (identity: ProjectPreviewRequestIdentity) => Promise<ProjectPreviewStartReceipt | null>;
  readLifecycle?: () => Promise<ProjectLifecycleSnapshot>;
};

const projectPreviewDependencies: ProjectPreviewDependencies = {
  start: startProjectPreview,
  readLifecycle: readProjectLifecycle,
};

async function flushProjectDraftsBeforeTransition() {
  await flushWorkspaceMutationInputs("manual");
}

function createEmptyInspectorPending(): Record<InspectorPendingArea, boolean> {
  return { html: false, css: false, js: false };
}

function createEmptyHtmlPending(): Record<HtmlPendingArea, boolean> {
  return { tag: false, attributes: false, text: false, image: false, classes: false, structure: false };
}

export type ProjectControllerHost = {
  source: string;
  sourceCache: Record<string, string>;
  activeScannedPath: string | null;
  activePreviewPath: string;
  browserPreviewRoute: string;
  previewSrc: string;
  previewWorkspaceRevision: string | null;
  pendingCanvasProjection: CanvasProjectionPlan | null;
  activeCanvasIdentity: CanvasProjectionIdentity | null;
  activeCanvasUrl: string;
  previewDocumentMarkup: string | null;
  refreshToken: number;
  centerView: CenterView;
  templateWorkbenchPlan: TemplateWorkbenchPlan | null;
  templateWorkbenchPreferredPagePath: string | null;
  templateWorkbenchPreferredRoute: string | null;
  templateWorkbenchActive: boolean;
  templateWorkbenchTarget: string | null;
  templateWorkbenchReturnPreviewPath: string | null;
  templateWorkbenchRequestSerial: number;
  selectionSnapshot: SelectionSnapshot | null;
  inspectorSelectionSummary: InspectorSelectionSummarySnapshot | null;
  hoverSnapshot: HoverSnapshot | null;
  overrideRules: Record<string, unknown>;
  variableOverrides: Record<string, string>;
  htmlPending: Record<HtmlPendingArea, boolean>;
  inspectorPending: Record<InspectorPendingArea, boolean>;
  resetInspectorPendingSources: () => void;
  pendingTag: string | null;
  pendingTagOriginal: string | null;
  pendingTagSourceLocation: SourceEditLocation | null;
  tagStatus: string;
  projectStatus: string;
  scannedProject: ProjectScan | null;
  applicationSurface: ApplicationSurface;
  projectLifecycle: ProjectLifecycleSnapshot;
  startupFlow: StartupFlowSnapshot;
  startupCreationCatalog: StartupCreationCatalog | null;
  startupCreationPlan: StartupCreationPlan | null;
  startupSelectedOptionId: string | null;
  startupPending: boolean;
  startupError: string;
  projectOpenRecoveryDecisionRequest: ProjectOpenRecoveryDecisionRequest | null;
  projectTransitionDecisionRequest: ProjectTransitionDecisionRequest | null;
  sourceGraph: SourceGraph | null;
  sourceGraphProjectionStatus: WorkspaceDerivedProjectionStatus;
  sourceGraphWorkspaceRevision: number | null;
  diskState: DiskState;
  scssVariables: ScssVariable[];
  targetCssFile: string;
  cachebustAssets: boolean;
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  kernelUndoRedoFrontendLeaseActive?: boolean;
  projectTransitionFrontendLeaseActive?: boolean;
  aiReconciliationRecoveryReloadAuthorized?: boolean;
  projectSessionEpoch: number;
  projectWorkspaceMutationEpoch: number;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  workbenchSnapshot: WorkbenchSnapshot | null;
  fileExplorerSnapshot: import("$lib/types").FileExplorerSnapshot | null;
  fileExplorerLoading: boolean;
  fileExplorerError: string;
  hydrateWorkbenchBootstrap?: (snapshot: WorkbenchSnapshot) => void;
  revealBootstrapDiagnosticLocation?: (
    relativePath: string,
    location: ProjectBootstrapSourceLocation,
  ) => void;
  beginPreviewStructuralWriteBoundary: () => Promise<void>;
  endPreviewStructuralWriteBoundary: () => void;
  activeVersionPreview: unknown | null;
  reattachCurrentProjectSession?: () => Promise<boolean>;
  flushInteractiveEditorDrafts: () => Promise<void>;
  beginProjectTransitionFrontendLease?: () => Promise<void>;
  endProjectTransitionFrontendLease?: () => void;
  loadScannedProjectFile: (
    file: ProjectFile,
    options?: {
      strict?: boolean;
      skipDraftFlush?: boolean;
      deferPreviewRefresh?: boolean;
      activateTemplateWorkbench?: boolean;
      preferredTemplatePagePath?: string | null;
      preferredTemplateRoute?: string | null;
      syncWorkbench?: boolean;
    },
  ) => Promise<void>;
  restoreWorkbenchState?: () => Promise<unknown>;
  updateTemplateWorkbenchContext: (
    project: ProjectScan,
    templateFile: ProjectFile,
    preferredPagePath?: string | null,
    options?: {
      deferPreviewRefresh?: boolean;
      expectedWorkspaceRevision?: number;
      minimumWorkspaceRevision?: number;
      preferredRoute?: string | null;
      strict?: boolean;
    },
  ) => Promise<ProjectFile | null>;
  setSessionProjectRoot: (projectRoot?: string) => void;
  cancelPendingHtmlMutations: () => void;
  clearPreviewSelection: (options?: { clearCanvasOverlay?: boolean }) => void;
  clearHtmlPending: () => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  requestPreviewRefresh: (
    reason: PreviewRefreshReason,
  ) => Promise<boolean>;
  rescanCurrentProject: (
    preferredRelativePath?: string | null,
    options?: { strict?: boolean },
  ) => Promise<void>;
  reconcileWorkspaceDerivedState: (
    options: ReconcileWorkspaceDerivedStateOptions,
  ) => Promise<WorkspaceDerivedReconciliationOutcome>;
  refreshRenderedPreviewDocument: () => Promise<boolean>;
  prepareCanvasProjectionNavigation: (plan: CanvasProjectionPlan) => Promise<void>;
  hasMountedCanvasProjectionSurface?: () => boolean;
  deferWorkspacePreviewProjection?: () => void;
  markCanvasProjectionSurfaceCurrent?: () => void;
  reconcileTemplateWorkbenchPreviewDocument: (
    previewUrl: string,
    plan: CanvasProjectionPlan,
  ) => Promise<boolean>;
  previewUrlForScannedFile: (file: ProjectFile) => string;
  exitTemplateWorkbench: (options?: {
    deferPreviewRefresh?: boolean;
    returnPath?: string | null;
  }) => Promise<void>;
  cancelPreviewSync: () => void;
  resetPageSections?: () => void;
  refreshSourceGraph?: (options?: { strict?: boolean }) => Promise<void>;
  refreshEditorNavigationSnapshot?: (
    identity?: CanvasProjectionIdentity,
    previewUrl?: string,
    options?: { strict?: boolean },
  ) => Promise<void>;
  resetControlledPreviewState?: () => void;
  scheduleZolaValidation?: (reason?: "project-open") => void;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
  clearNotification: (id: string) => void;
  establishExternalDiskBaseline?: () => Promise<void>;
  startExternalDiskMonitoring?: () => void;
  resetExternalDiskState?: () => void;
  invalidateExternalReconcileForProjectTransition?: () => Promise<void>;
  resumeExternalMonitoringAfterFailedTransition?: () => void;
  markWorkspaceProjectionRecoveryRequired?: (message: string) => void;
};

export async function openProjectFolder(host: ProjectControllerHost) {
  console.info("[Pană Studio] openProjectFolder invoked");
  host.startupError = "";
  host.startupCreationPlan = null;
  host.startupCreationCatalog = null;
  host.startupSelectedOptionId = null;
  await tick();
  try {
    console.info("[Pană Studio] requesting project folder from dialog");
    const selected = await chooseProjectFolder();
    console.info("[Pană Studio] project folder dialog returned", selected);
    if (!selected || Array.isArray(selected)) {
      return;
    }
    host.startupPending = true;
    await tick();
    const startup = await inspectStartupFolder(selected);
    host.startupFlow = startup;
    const candidate = startup.candidate;
    if (!candidate) return;
    if (candidate.kind === "valid_project") {
      await openProjectRoot(host, candidate.root, { startupCandidate: candidate });
      return;
    }
    if (candidate.kind === "empty_directory") {
      host.startupCreationCatalog = await readStartupCreationCatalog(candidate.snapshotToken);
    }
  } catch (error) {
    const message = errorMessage(error);
    host.startupError = message;
    host.escalateGlobalStatus({
      id: "startup.folder.error",
      level: "error",
      title: "Dosarul nu a putut fi inspectat",
      message,
    });
  } finally {
    host.startupPending = false;
  }
}

export async function retryStartupProjectOpen(host: ProjectControllerHost) {
  const candidate = host.startupFlow.candidate;
  if (candidate?.kind !== "valid_project") {
    host.startupError = "Proiectul valid selectat nu mai este disponibil pentru redeschidere.";
    return;
  }
  host.startupPending = true;
  host.startupError = "";
  await tick();
  try {
    await openProjectRoot(host, candidate.root, { startupCandidate: candidate });
    host.clearNotification("startup.folder.error");
  } catch (error) {
    const message = errorMessage(error);
    host.startupError = message;
    host.escalateGlobalStatus({
      id: "startup.folder.error",
      level: "error",
      title: "Proiectul nu a putut fi deschis",
      message,
    });
  } finally {
    host.startupPending = false;
  }
}

export function selectStartupCreationOption(
  host: ProjectControllerHost,
  optionId: string,
) {
  if (!host.startupCreationCatalog?.options.some((option) => option.id === optionId)) return;
  host.startupSelectedOptionId = optionId;
  host.startupCreationPlan = null;
  host.startupError = "";
}

export async function planStartupProject(host: ProjectControllerHost) {
  const candidate = host.startupFlow.candidate;
  const optionId = host.startupSelectedOptionId;
  if (candidate?.kind !== "empty_directory" || !optionId) return;
  host.startupPending = true;
  host.startupError = "";
  try {
    host.startupCreationPlan = await planStartupCreation({
      expectedSnapshotToken: candidate.snapshotToken,
      optionId,
    });
  } catch (error) {
    host.startupError = errorMessage(error);
  } finally {
    host.startupPending = false;
  }
}

export function cancelStartupCreationPlan(host: ProjectControllerHost) {
  host.startupCreationPlan = null;
  host.startupError = "";
}

export async function applyStartupProject(host: ProjectControllerHost) {
  const plan = host.startupCreationPlan;
  if (!plan) return;
  host.startupPending = true;
  host.startupError = "";
  try {
    const receipt = await applyStartupCreation({
      expectedSnapshotToken: plan.expectedSnapshotToken,
      expectedPlanToken: plan.planToken,
    });
    host.startupFlow = receipt.startup;
    host.startupCreationPlan = null;
    host.startupCreationCatalog = null;
    host.startupSelectedOptionId = null;
    await openProjectRoot(host, receipt.projectRoot, {
      startupCandidate: receipt.startup.candidate,
    });
  } catch (error) {
    const message = errorMessage(error);
    host.startupError = message;
    host.escalateGlobalStatus({
      id: "startup.creation.error",
      level: "error",
      title: "Proiectul nu a putut fi creat",
      message,
    });
  } finally {
    host.startupPending = false;
  }
}

type FrontendProjectAttachmentMode = "open" | "reattach" | "reload";

type FrontendProjectAttachment = ProjectPreviewRequestIdentity & {
  initialSurface?: ProjectBootstrapInitialSurface | null;
  previewWarning?: string | null;
};

export type ProjectPreviewStartOutcome =
  | { status: "canonical"; projectSessionId: string }
  | { status: "deferred"; projectSessionId: string }
  | { status: "degraded"; projectSessionId: string; message: string }
  | { status: "stale"; projectSessionId: string };

export type ProjectReloadOutcome =
  | {
      status: "completed";
      projectSessionId: string;
      previewStatus: "canonical" | "deferred" | "degraded";
      message: string | null;
    }
  | { status: "cancelled"; projectSessionId: null; message: string }
  | { status: "failed"; projectSessionId: string | null; message: string };

type FrontendProjectAttachmentOptions = {
  bootstrap: ProjectOpenBootstrapReceipt;
};

function requireProjectAttachmentAuthority(project: ProjectScan) {
  if (!project.kernelSessionId?.trim()) {
    throw new Error(t("project-controller-scan-session-missing"));
  }
  if (!project.acceptedDiskManifest || !project.acceptedDiskGeneration) {
    throw new Error(t("project-controller-scan-manifest-missing"));
  }
}

async function projectPublishedSessionIntoFrontend(
  host: ProjectControllerHost,
  project: ProjectScan,
  mode: FrontendProjectAttachmentMode,
  options: FrontendProjectAttachmentOptions,
): Promise<FrontendProjectAttachment | null> {
  requireProjectAttachmentAuthority(project);
  host.projectOpenRecoveryDecisionRequest = null;
  host.projectTransitionDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
  host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
  resetProjectScopedState(host, { preserveExternalReconcileBarrier: true });
  if (mode !== "reattach") host.applicationSurface = "workbench";
  host.scannedProject = project;
  host.kernelProjectSessionId = project.kernelSessionId ?? "";
  host.diskState = diskStateFromProjectScan(project, host.diskState);
  host.setSessionProjectRoot(project.root);
  setFileBufferDraftSyncSession(project.root, host.kernelProjectSessionId);
  setPageJsDraftSyncSession(project.root, host.kernelProjectSessionId);

  const openPlan = planOpenedProject(project);
  // Recovery and editable buffers are restored by ProjectWorkspace before the
  // frontend receives ProjectScan. The browser only rebuilds its projection.
  const fileBuffers = options.bootstrap.fileBuffers;
  if (
    !fileBuffers
    || fileBuffers.projectRoot !== project.root
    || fileBuffers.runtimeSessionId !== host.kernelProjectSessionId
  ) {
    throw new Error(t("project-controller-session-mismatch"));
  }
  const dirtyWorkspacePaths = fileBuffers.files
    .filter((file) => file.dirty)
    .map((file) => file.relativePath)
    .sort();
  const restoredDirtySession = dirtyWorkspacePaths.length > 0;
  host.projectWorkspaceSnapshot = options.bootstrap.workspace;

  host.projectStatus = openPlan.projectStatus;
  if (project.previewWarning) {
    host.escalateGlobalStatus({
      id: "project.preview.warning",
      level: "warning",
      title: t("project-controller-preview-unavailable-title"),
      message: project.previewWarning,
      statusMessage: t("project-controller-preview-unavailable-detail", {
        message: project.previewWarning,
      }),
    });
  }
  const targetCssFile = options.bootstrap.targetCssFile ?? openPlan.targetCssFile;
  if (targetCssFile) host.targetCssFile = targetCssFile;
  host.cachebustAssets = options.bootstrap.projectSettings.cachebustAssets;
  host.workbenchSnapshot = options.bootstrap.workbench;
  host.hydrateWorkbenchBootstrap?.(options.bootstrap.workbench);
  const bootstrapFile = options.bootstrap.activeDocument
    ? project.files.find(
        (file) => file.relativePath === options.bootstrap.activeDocument?.relativePath,
      ) ?? null
    : null;
  if (bootstrapFile && options.bootstrap.activeDocument) {
    const loadPlan = planScannedProjectFileLoad(bootstrapFile);
    host.sourceCache = {
      ...host.sourceCache,
      [loadPlan.cacheKey]: options.bootstrap.activeDocument.source,
    };
  }
  // Rust has already selected exactly one document from the restored
  // Workbench. The frontend only projects that decision; it must not choose a
  // preferred or fallback document independently.
  const fileToOpen = bootstrapFile;

  if (fileToOpen) {
    await host.loadScannedProjectFile(fileToOpen, {
      strict: true,
      skipDraftFlush: true,
      // Source selection is established now; Workbench may only be requested
      // after the canonical Preview generation has been mounted and accepted.
      activateTemplateWorkbench: false,
      syncWorkbench: false,
    });
    if (options.bootstrap.activeDocument?.diagnosticLocation) {
      host.revealBootstrapDiagnosticLocation?.(
        fileToOpen.relativePath,
        options.bootstrap.activeDocument.diagnosticLocation,
      );
    }
  }
  if (mode === "reattach" && !project.previewWarning) {
    const detail = dirtyWorkspacePaths.length > 0
      ? ` ${t("project-controller-unsaved-restored", {
          count: dirtyWorkspacePaths.length,
        })}`
      : "";
    host.setGlobalStatus(
      `${t("project-controller-session-reattached")}${detail}`,
      restoredDirtySession ? "unsaved" : "restored",
    );
    host.clearNotification("project.preview.warning");
  } else if (!restoredDirtySession && !project.previewWarning) {
    host.setGlobalStatus(t("project-controller-loaded-from-disk"), "restored");
    host.clearNotification("project.preview.warning");
  }
  const scssIdentity = createCssRequestIdentity(project.root, host.kernelProjectSessionId);
  void getScssVariables(scssIdentity, host.projectWorkspaceSnapshot?.revision)
    .then((variables) => {
      if (
        host.sessionProjectRoot === project.root
        && host.kernelProjectSessionId === scssIdentity.expectedSessionId
      ) host.scssVariables = variables;
    })
    .catch(() => undefined);
  host.resetExternalDiskState?.();
  await host.establishExternalDiskBaseline?.();
  host.startExternalDiskMonitoring?.();
  host.projectLifecycle = await acknowledgeProjectFrontendHydrated(
    project.root,
    host.kernelProjectSessionId,
  );
  return {
    expectedProjectRoot: project.root,
    expectedSessionId: host.kernelProjectSessionId,
    initialSurface: options.bootstrap.initialSurface,
    previewWarning: project.previewWarning,
  };
}

export async function reattachCurrentProjectSession(host: ProjectControllerHost): Promise<boolean> {
  if (host.scannedProject) return true;
  await host.beginProjectTransitionFrontendLease?.();
  let previewIdentity: FrontendProjectAttachment | null = null;
  try {
    const bootstrap = await reattachProjectSession();
    if (!bootstrap) return false;
    host.projectLifecycle = bootstrap.lifecycle;
    previewIdentity = await projectPublishedSessionIntoFrontend(
      host,
      bootstrap.project,
      "reattach",
      { bootstrap },
    );
    host.clearNotification("project.reattach.error");
  } catch (error) {
    await degradeFrontendAttachment(host, errorMessage(error));
    const message = t("project-controller-reattach-failed", {
      message: errorMessage(error),
    });
    host.projectStatus = message;
    host.escalateGlobalStatus({
      id: "project.reattach.error",
      level: "error",
      title: t("project-controller-reattach-failed-title"),
      message,
      statusMessage: message,
    });
    throw error;
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }
  if (previewIdentity) {
    await startPreviewAfterOpen(host, previewIdentity);
    void refreshSourceGraphAfterCommit(host, previewIdentity);
  }
  return true;
}

async function openProjectRoot(
  host: ProjectControllerHost,
  root: string,
  options: OpenProjectRootOptions = {},
) {
  console.info("[Pană Studio] openProjectRoot started", root);
  await host.beginProjectTransitionFrontendLease?.();
  let transitionAllowed = false;
  try {
    await flushProjectDraftsBeforeTransition();
    transitionAllowed = await prepareProjectTransitionForTarget(
      host,
      root,
      { kind: "open_project" },
      options.operatorDecisionId ?? null,
    );
  } catch (error) {
    host.endProjectTransitionFrontendLease?.();
    throw error;
  }
  if (!transitionAllowed) {
    host.endProjectTransitionFrontendLease?.();
    return;
  }
  const openAction = projectTransitionActionForContinuation(
    root,
    host.scannedProject?.root,
    { kind: "open_project" },
  );
  let inspection = options.inspection ?? null;
  try {
    if (!inspection) {
      let candidate = options.startupCandidate ?? null;
      if (candidate?.kind !== "valid_project" || candidate.root !== root) {
        const startup = await inspectStartupFolder(root);
        host.startupFlow = startup;
        candidate = startup.candidate;
      }
      if (candidate?.kind !== "valid_project") {
        throw new Error(
          candidate?.diagnostics[0]?.message ?? "Dosarul nu este un proiect Zola valid.",
        );
      }
      inspection = await inspectProjectOpen(candidate.root, candidate.snapshotToken);
    }
    host.projectLifecycle = inspection.lifecycle;
    if (inspection.lifecycle.operationId !== inspection.operationId) {
      throw new Error("ProjectLifecycle a returnat o inspecție fără operationId autoritar.");
    }
  } catch (error) {
    host.endProjectTransitionFrontendLease?.();
    throw error;
  }
  if (openAction === "open_project") {
    try {
      const assessment = inspection.recovery;
      if (assessment.status === "decision_required") {
        const suppliedToken = options.recoveryDecision?.assessmentToken ?? null;
        if (!suppliedToken) {
          const request = createProjectOpenRecoveryDecisionRequest(
            root,
            assessment,
            options.operatorDecisionId ?? null,
            inspection.operationId,
            inspection.candidateToken,
            inspection,
          );
          host.projectOpenRecoveryDecisionRequest = request;
          host.projectStatus = t("project-controller-recovery-decision-pending");
          host.escalateGlobalStatus({
            id: PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
            level: "warning",
            title: t("project-controller-recovery-incompatible-title"),
            message: t("project-controller-recovery-incompatible-message"),
            statusMessage: t("project-controller-recovery-incompatible-status"),
          });
          host.endProjectTransitionFrontendLease?.();
          return;
        }
        if (suppliedToken !== assessment.assessmentToken) {
          throw new Error(
            t("project-controller-recovery-changed"),
          );
        }
      } else if (options.recoveryDecision) {
        throw new Error(
          t("project-controller-recovery-decision-stale"),
        );
      }
    } catch (error) {
      await cancelProjectOpen(inspection.operationId, "recovery_validation_failed").catch(() => {});
      host.endProjectTransitionFrontendLease?.();
      throw error;
    }
  } else if (options.recoveryDecision) {
    await cancelProjectOpen(inspection.operationId, "reload_recovery_decision_invalid").catch(() => {});
    host.endProjectTransitionFrontendLease?.();
    throw new Error(t("project-controller-reload-recovery-decision-invalid"));
  }
  try {
    await host.invalidateExternalReconcileForProjectTransition?.();
  } catch (error) {
    await cancelProjectOpen(inspection.operationId, "external_reconcile_failed").catch(() => {});
    host.endProjectTransitionFrontendLease?.();
    throw error;
  }
  let rustSessionSwapped = false;
  let previewIdentity: FrontendProjectAttachment | null = null;
  try {
    const bootstrap = await openProject(
      root,
      inspection.operationId,
      inspection.candidateToken,
      options.operatorDecisionId ?? undefined,
      options.recoveryDecision ?? undefined,
    );
    const project = bootstrap.project;
    host.projectLifecycle = bootstrap.lifecycle;
    rustSessionSwapped = true;
    console.info("[Pană Studio] openProject returned", project);
    previewIdentity = await projectPublishedSessionIntoFrontend(host, project, "open", {
      bootstrap,
    });
  } catch (error) {
    host.projectLifecycle = await readProjectLifecycle().catch(() => host.projectLifecycle);
    if (rustSessionSwapped) {
      await degradeFrontendAttachment(host, errorMessage(error));
      host.markWorkspaceProjectionRecoveryRequired?.(
        t("project-controller-initial-projection-incomplete"),
      );
    } else {
      host.resumeExternalMonitoringAfterFailedTransition?.();
    }
    throw error;
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }
  if (previewIdentity) {
    await startPreviewAfterOpen(host, previewIdentity);
    void refreshSourceGraphAfterCommit(host, previewIdentity);
  }
}

async function refreshSourceGraphAfterCommit(
  host: ProjectControllerHost,
  identity: ProjectPreviewRequestIdentity,
) {
  try {
    if (!host.refreshSourceGraph) {
      throw new Error(t("project-controller-source-graph-unavailable"));
    }
    await host.refreshSourceGraph({ strict: true });
  } catch (error) {
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return;
    host.projectLifecycle = await reportProjectCapabilityDegraded(
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      "source_graph",
      errorMessage(error),
    ).catch(() => host.projectLifecycle);
  }
}

async function degradeFrontendAttachment(
  host: ProjectControllerHost,
  diagnostic: string,
) {
  const active = host.projectLifecycle.activeSession;
  if (!active) return;
  host.projectLifecycle = await reportProjectCapabilityDegraded(
    active.projectRoot,
    active.runtimeSessionId,
    "frontend",
    diagnostic,
  ).catch(() => host.projectLifecycle);
}

async function prepareProjectTransitionForTarget(
  host: ProjectControllerHost,
  targetRoot: string,
  continuation: ProjectTransitionContinuation,
  operatorDecisionId: string | null,
) {
  if (!host.scannedProject && continuation.kind !== "close_project") return true;
  if (operatorDecisionId) return true;
  if (
    continuation.kind === "reload_project"
    && host.aiReconciliationRecoveryReloadAuthorized
  ) return true;

  const currentProjectRoot = host.scannedProject?.root ?? targetRoot;
  const action = projectTransitionActionForContinuation(targetRoot, currentProjectRoot, continuation);
  const policy = await readKernelProjectTransitionPolicy(action);
  const policyCopy = localizedTransitionPolicyCopy(policy);

  if (policy.decision === "allow") return true;

  if (policy.decision === "confirm") {
    const request = createProjectTransitionDecisionRequest(
      targetRoot,
      currentProjectRoot,
      policy,
      continuation,
    );
    host.projectTransitionDecisionRequest = request;
    host.projectStatus = policyCopy.message;
    host.escalateGlobalStatus({
      id: PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
      level: "warning",
      title: policyCopy.title,
      message: `${policyCopy.message} ${policyCopy.recommendedAction}`,
    });
    return false;
  }

  host.projectTransitionDecisionRequest = null;
  const message = `${policyCopy.title}: ${policyCopy.message} ${policyCopy.recommendedAction}`;
  host.projectStatus = message;
  host.escalateGlobalStatus({
    id: PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID,
    level: "error",
    title: policyCopy.title,
    message,
    statusMessage: message,
  });
  return false;
}

export async function continueProjectTransitionWithOperatorDecision(
  host: ProjectControllerHost,
  requestId: string,
  diagnostic: string,
) {
  const request = host.projectTransitionDecisionRequest;
  if (!request || request.id !== requestId) {
    throw new Error(t("project-transition-decision-expired"));
  }
  host.projectStatus = t("project-controller-recording-decision");
  host.setGlobalStatus(t("project-controller-recording-decision-status"), "saving");
  try {
    const receipt = await recordProjectTransitionOperatorDecision(
      request.targetRoot,
      diagnostic,
      request.action,
    );
    host.projectTransitionDecisionRequest = null;
    host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
    if (request.continuation.kind === "close_project") {
      await closeCurrentProject(host, {
        operatorDecisionId: receipt.decision.id,
        detachedProjectRoot: host.scannedProject ? null : request.targetRoot,
      });
    } else if (request.continuation.kind === "reload_project") {
      await reloadCurrentProjectFromDisk(host, request.continuation.preferredRelativePath, {
        mode: request.continuation.mode,
        operatorDecisionId: receipt.decision.id,
      });
    } else {
      await openProjectRoot(host, request.targetRoot, { operatorDecisionId: receipt.decision.id });
    }
  } catch (error) {
    const message = t("project-controller-transition-cannot-continue", {
      message: errorMessage(error),
    });
    host.projectStatus = message;
    host.escalateGlobalStatus({
      id: PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
      level: "error",
      title: t("project-controller-transition-refused-title"),
      message,
      statusMessage: message,
    });
    throw error;
  }
}

export async function cancelProjectOpenRecoveryDecision(
  host: ProjectControllerHost,
  requestId: string,
) {
  const request = host.projectOpenRecoveryDecisionRequest;
  if (request?.id !== requestId) return;
  if (request.operationId) {
    host.projectLifecycle = await cancelProjectOpen(
      request.operationId,
      "recovery_dialog_cancelled",
    );
  }
  host.projectOpenRecoveryDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.projectStatus = t("project-controller-open-cancelled-recovery-kept");
  host.setGlobalStatus(t("project-controller-recovery-kept"), "restored");
}

export async function continueProjectOpenWithRecoveryAbandonment(
  host: ProjectControllerHost,
  requestId: string,
) {
  const request = host.projectOpenRecoveryDecisionRequest;
  if (!request || request.id !== requestId) {
    throw new Error(t("project-controller-open-recovery-decision-stale"));
  }
  const decision = projectOpenRecoveryAbandonDecision(request);
  host.projectOpenRecoveryDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.projectStatus = t("project-controller-opening-without-recovery");
  host.setGlobalStatus(t("project-controller-abandoning-recovery"), "saving");
  try {
    await openProjectRoot(host, request.targetRoot, {
      operatorDecisionId: request.operatorDecisionId,
      recoveryDecision: decision,
      inspection: request.inspection,
    });
  } catch (error) {
    const message = t("project-controller-open-after-recovery-failed", {
      message: errorMessage(error),
    });
    host.projectStatus = message;
    host.escalateGlobalStatus({
      id: PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
      level: "error",
      title: t("project-controller-recovery-apply-failed-title"),
      message,
      statusMessage: message,
    });
    throw error;
  }
}

export function isProjectPreviewRequestIdentityCurrent(
  host: Pick<
    ProjectControllerHost,
    | "scannedProject"
    | "sessionProjectRoot"
    | "kernelProjectSessionId"
    | "projectTransitionFrontendLeaseActive"
  >,
  identity: ProjectPreviewRequestIdentity,
) {
  return host.projectTransitionFrontendLeaseActive !== true
    && host.scannedProject !== null
    && host.scannedProject.root === identity.expectedProjectRoot
    && host.sessionProjectRoot === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId;
}

export function bindCanvasCandidateIdentityToPreviewUrl(
  url: string,
  identity: CanvasProjectionIdentity,
) {
  const candidateUrl = new URL(url);
  candidateUrl.searchParams.set("__pana_preview_revision", identity.previewRevision);
  candidateUrl.searchParams.set("__pana_canvas_transaction", identity.transactionId);
  return candidateUrl.toString();
}

function mountBootstrapInitialSurface(
  host: ProjectControllerHost,
  activeFile: ProjectFile | null,
  surface: ProjectBootstrapInitialSurface | null | undefined,
  receipt: ProjectPreviewStartReceipt,
) {
  if (!surface) return false;
  let surfaceUrl: URL;
  try {
    surfaceUrl = new URL(surface.previewUrl);
  } catch {
    throw new Error(t("project-controller-template-receipt-mismatch"));
  }
  if (
    activeFile?.role !== "template"
    || activeFile.relativePath !== surface.documentPath
    || surface.plan.activeTemplate.file !== surface.documentPath
    || !surface.route.startsWith("/__pana_workbench/")
    || surfaceUrl.pathname !== surface.route
    || surfaceUrl.searchParams.get("__pana_preview_revision")
      !== receipt.canvasProjection.identity.previewRevision
    || surfaceUrl.searchParams.get("__pana_canvas_transaction")
      !== receipt.canvasProjection.identity.transactionId
    || surface.canvasProjection.phase !== receipt.canvasProjection.phase
    || !canvasProjectionIdentityMatches(
      surface.canvasProjection.identity,
      receipt.canvasProjection.identity,
    )
  ) {
    throw new Error(t("project-controller-template-receipt-mismatch"));
  }

  host.templateWorkbenchActive = true;
  host.templateWorkbenchTarget = surface.documentPath;
  host.templateWorkbenchPlan = surface.plan;
  host.templateWorkbenchPreferredPagePath = surface.plan.selectedContext?.pageFile ?? null;
  host.templateWorkbenchPreferredRoute = surface.plan.selectedRoute?.url ?? null;
  host.templateWorkbenchReturnPreviewPath = surface.plan.selectedContext?.pageFile ?? null;
  host.activePreviewPath = surface.documentPath;
  host.browserPreviewRoute = surface.plan.selectedContext?.pageUrl
    ?? surface.plan.selectedRoute?.url
    ?? "/";
  host.previewDocumentMarkup = null;
  host.previewSrc = surface.previewUrl;
  return true;
}

export async function startPreviewAfterOpen(
  host: ProjectControllerHost,
  identity: FrontendProjectAttachment,
  dependencies: ProjectPreviewDependencies = projectPreviewDependencies,
): Promise<ProjectPreviewStartOutcome> {
  const stale = (): ProjectPreviewStartOutcome => ({
    status: "stale",
    projectSessionId: identity.expectedSessionId,
  });
  if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
  if (identity.previewWarning) {
    host.resetControlledPreviewState?.();
    host.projectLifecycle = await reportProjectCapabilityDegraded(
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      "preview",
      identity.previewWarning,
    ).catch(() => host.projectLifecycle);
    return {
      status: "degraded",
      projectSessionId: identity.expectedSessionId,
      message: identity.previewWarning,
    };
  }
  let canvasConfirmation: Promise<void> | null = null;
  let startedPreviewUrl: string | null = null;
  try {
    const rawReceipt = await dependencies.start(identity);
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
    if (!rawReceipt) {
      throw new Error(t("project-controller-preview-generation-missing"));
    }
    const receipt = requireProjectPreviewStartReceipt(identity, rawReceipt);
    startedPreviewUrl = receipt.url;
    const currentProject = host.scannedProject;
    if (!currentProject) return stale();
    host.scannedProject = {
      ...currentProject,
      previewBaseUrl: receipt.url,
      previewWarning: null,
    };
    const canvasSurfaceMounted = host.hasMountedCanvasProjectionSurface?.() !== false;
    if (receipt.canvasProjection.phase === "prepared") {
      host.pendingCanvasProjection = receipt.canvasProjection;
      host.previewWorkspaceRevision = receipt.canvasProjection.identity.previewRevision;
      if (canvasSurfaceMounted) {
        canvasConfirmation = host.prepareCanvasProjectionNavigation(receipt.canvasProjection);
      } else {
        host.deferWorkspacePreviewProjection?.();
      }
    } else {
      host.pendingCanvasProjection = null;
      host.previewWorkspaceRevision = null;
      host.activeCanvasIdentity = { ...receipt.canvasProjection.identity };
      void host.refreshEditorNavigationSnapshot?.(
        receipt.canvasProjection.identity,
        receipt.url,
      );
      if (!canvasSurfaceMounted) host.deferWorkspacePreviewProjection?.();
    }
    const activeFile = currentProject.files.find((file) => file.relativePath === host.activeScannedPath)
      ?? currentProject.files.find((file) => file.role === "page")
      ?? null;
    const bootstrapSurfaceMounted = mountBootstrapInitialSurface(
      host,
      activeFile,
      identity.initialSurface,
      receipt,
    );
    if (!bootstrapSurfaceMounted && activeFile?.role === "page") {
      const activeUrl = new URL(activeFile.previewPath ?? "/", receipt.url).toString();
      host.previewSrc = receipt.canvasProjection.phase === "prepared"
        ? bindCanvasCandidateIdentityToPreviewUrl(
            activeUrl,
            receipt.canvasProjection.identity,
          )
        : activeUrl;
      host.activePreviewPath = activeFile.relativePath;
      host.browserPreviewRoute = activeFile.previewPath ?? "/";
      host.previewDocumentMarkup = null;
    }
    if (
      !bootstrapSurfaceMounted
      && (
        host.previewSrc === "about:blank"
        || receipt.canvasProjection.phase === "prepared"
      )
    ) {
      const revision = receipt.canvasProjection.identity.previewRevision;
      const mountedUrl = host.previewSrc === "about:blank" ? null : new URL(host.previewSrc);
      if (
        !mountedUrl
        || (
          receipt.canvasProjection.phase === "prepared"
          && mountedUrl.searchParams.get("__pana_preview_revision") !== revision
        )
      ) {
        const fallbackPage = currentProject.files.find((file) => file.role === "page") ?? null;
        if (!fallbackPage) {
          throw new Error(t("project-controller-canvas-route-missing"));
        }
        const fallbackUrl = new URL(fallbackPage.previewPath ?? "/", receipt.url).toString();
        host.previewSrc = receipt.canvasProjection.phase === "prepared"
          ? bindCanvasCandidateIdentityToPreviewUrl(
              fallbackUrl,
              receipt.canvasProjection.identity,
            )
          : fallbackUrl;
        host.activePreviewPath = fallbackPage.relativePath;
        host.previewDocumentMarkup = null;
      }
    }
    if (!canvasConfirmation && receipt.canvasProjection.phase === "prepared") {
      host.pendingCanvasProjection = null;
      host.previewWorkspaceRevision = null;
      if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
      host.clearNotification("project.preview.warning");
      host.setGlobalStatus(
        t("project-controller-preview-running-canvas-paused"),
        "restored",
      );
      host.projectLifecycle = await reportProjectCapabilityDegraded(
        identity.expectedProjectRoot,
        identity.expectedSessionId,
        "canvas",
        "Suprafața Canvas nu este montată; proiecția a fost amânată.",
      ).catch(() => host.projectLifecycle);
      return {
        status: "deferred",
        projectSessionId: identity.expectedSessionId,
      };
    }
    if (canvasConfirmation) {
      await canvasConfirmation;
      canvasConfirmation = null;
      host.previewWorkspaceRevision = null;
    }
    if (host.activeCanvasIdentity) host.activeCanvasUrl = host.previewSrc;
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
    markProjectWorkspacePreviewPublished(
      receipt.projectRoot,
      receipt.runtimeSessionId,
      receipt.workspaceRevision,
      receipt.canvasProjection,
    );
    if (!canvasSurfaceMounted) {
      host.clearNotification("project.preview.warning");
      host.setGlobalStatus(
        t("project-controller-preview-running-canvas-paused"),
        "restored",
      );
      return {
        status: "deferred",
        projectSessionId: identity.expectedSessionId,
      };
    }
    host.markCanvasProjectionSurfaceCurrent?.();
    if (activeFile?.role === "template" && bootstrapSurfaceMounted) {
      await synchronizeActiveCanvasSurfaceRoute(
        host,
        host.previewSrc,
        receipt.canvasProjection.identity,
      );
    } else if (activeFile?.role === "template") {
      await host.updateTemplateWorkbenchContext(
        currentProject,
        activeFile,
        host.templateWorkbenchPreferredPagePath,
        {
          preferredRoute: host.templateWorkbenchPreferredRoute,
          strict: true,
        },
      );
    } else if (host.activeCanvasIdentity) {
      await synchronizeActiveCanvasSurfaceRoute(
        host,
        host.previewSrc,
        host.activeCanvasIdentity,
      );
    }
    host.clearNotification("project.preview.warning");
    host.setGlobalStatus(t("project-controller-preview-started"), "restored");
    host.scheduleZolaValidation?.("project-open");
    return {
      status: "canonical",
      projectSessionId: identity.expectedSessionId,
    };
  } catch (error) {
    if (canvasConfirmation) await canvasConfirmation.catch(() => undefined);
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
    const currentProject = host.scannedProject;
    if (!currentProject) return stale();
    if (isCanvasProjectionSurfaceUnavailableError(error)) {
      host.pendingCanvasProjection = null;
      host.previewWorkspaceRevision = null;
      host.deferWorkspacePreviewProjection?.();
      host.scannedProject = {
        ...currentProject,
        previewBaseUrl: startedPreviewUrl ?? currentProject.previewBaseUrl,
        previewWarning: null,
      };
      host.clearNotification("project.preview.warning");
      host.setGlobalStatus(
        t("project-controller-preview-running-canvas-paused"),
        "restored",
      );
      host.projectLifecycle = await reportProjectCapabilityDegraded(
        identity.expectedProjectRoot,
        identity.expectedSessionId,
        "canvas",
        errorMessage(error),
      ).catch(() => host.projectLifecycle);
      return {
        status: "deferred",
        projectSessionId: identity.expectedSessionId,
      };
    }
    host.resetControlledPreviewState?.();
    const message = errorMessage(error);
    host.projectLifecycle = await reportProjectCapabilityDegraded(
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      "preview",
      message,
    ).catch(() => host.projectLifecycle);
    host.scannedProject = {
      ...currentProject,
      // Rust poate avea în continuare serverul persistent activ chiar dacă
      // primul Canvas nu a confirmat styledReady. Păstrăm originea pentru ca
      // următorul refresh să poată reatașa bridge-ul prin navigare.
      previewBaseUrl: startedPreviewUrl ?? currentProject.previewBaseUrl,
      previewWarning: message,
    };
    host.escalateGlobalStatus({
      id: "project.preview.warning",
      level: "warning",
      title: t("project-controller-preview-unavailable-title"),
      message,
      statusMessage: t("project-controller-preview-unavailable-detail", { message }),
    });
    return {
      status: "degraded",
      projectSessionId: identity.expectedSessionId,
      message,
    };
  } finally {
    const lifecycle = await dependencies.readLifecycle?.().catch(() => null);
    if (
      lifecycle?.activeSession?.projectRoot === identity.expectedProjectRoot
      && lifecycle.activeSession.runtimeSessionId === identity.expectedSessionId
    ) {
      host.projectLifecycle = lifecycle;
    }
  }
}

export function resetProjectScopedState(
  host: ProjectControllerHost,
  options: { preserveExternalReconcileBarrier?: boolean } = {},
) {
  resetProjectWorkspacePreviewCoordinator();
  resetFileBufferDraftSyncState();
  resetPageJsDraftSyncState();
  if (!options.preserveExternalReconcileBarrier) host.resetExternalDiskState?.();
  host.resetControlledPreviewState?.();
  host.resetPageSections?.();
  host.sourceGraph = null;
  host.sourceGraphProjectionStatus = "deferred";
  host.sourceGraphWorkspaceRevision = null;
  host.sourceCache = {};
  host.source = "";
  host.activeScannedPath = null;
  host.previewSrc = "about:blank";
  host.activePreviewPath = "about:blank";
  host.browserPreviewRoute = "/";
  host.previewDocumentMarkup = null;
  host.previewWorkspaceRevision = null;
  host.projectWorkspaceSnapshot = null;
  host.workbenchSnapshot = null;
  host.fileExplorerSnapshot = null;
  host.fileExplorerLoading = false;
  host.fileExplorerError = "";
  host.scssVariables = [];
  host.targetCssFile = "styles.css";
  host.templateWorkbenchPlan = null;
  host.templateWorkbenchPreferredPagePath = null;
  host.templateWorkbenchPreferredRoute = null;
  host.templateWorkbenchActive = false;
  host.templateWorkbenchTarget = null;
  host.templateWorkbenchReturnPreviewPath = null;
  host.templateWorkbenchRequestSerial += 1;
  host.clearPreviewSelection({ clearCanvasOverlay: true });
  host.selectionSnapshot = null;
  host.inspectorSelectionSummary = null;
  host.hoverSnapshot = null;
  host.overrideRules = {};
  host.variableOverrides = {};
  host.htmlPending = createEmptyHtmlPending();
  host.resetInspectorPendingSources();
  host.inspectorPending = createEmptyInspectorPending();
  host.setGlobalStatus(t("project-controller-no-session-save"), "idle");
  host.cachebustAssets = false;
  host.diskState = createDiskState();
  host.activeVersionPreview = null;
  host.setSessionProjectRoot();
  host.kernelProjectSessionId = "";
}

export async function rescanCurrentProject(
  host: ProjectControllerHost,
  preferredRelativePath: string | null = host.activeScannedPath,
  options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
) {
  if (!host.scannedProject) return;
  await host.beginProjectTransitionFrontendLease?.();
  try {
    await flushProjectDraftsBeforeTransition();
    await projectCurrentProjectRescan(host, preferredRelativePath, options);
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }
}

export type ReconcileWorkspaceDerivedStateOptions = {
  expectedProjectRoot: string;
  expectedSessionId: string;
  expectedWorkspaceRevision: number;
  topologyChanged: boolean;
  preferredRelativePath?: string | null;
  refreshSourceGraph?: boolean;
  refreshScss?: boolean;
};

function derivedProjectionOutcome(
  workspaceRevision: number,
): WorkspaceDerivedReconciliationOutcome {
  return {
    workspaceRevision,
    topology: "current",
    sourceGraph: "current",
    scss: "current",
    warnings: [],
  };
}

function derivedProjectionSessionStatus(
  host: ProjectControllerHost,
  options: Pick<
    ReconcileWorkspaceDerivedStateOptions,
    "expectedProjectRoot" | "expectedSessionId" | "expectedWorkspaceRevision"
  >,
): "current" | "superseded" {
  if (
    host.sessionProjectRoot !== options.expectedProjectRoot
    || host.kernelProjectSessionId !== options.expectedSessionId
  ) {
    return "superseded";
  }
  const revision = host.projectWorkspaceSnapshot?.revision;
  if (revision === undefined || revision === options.expectedWorkspaceRevision) return "current";
  if (revision > options.expectedWorkspaceRevision) return "superseded";
  throw new Error(
    t("project-controller-derived-revision-mismatch", {
      expected: options.expectedWorkspaceRevision,
      actual: revision,
    }),
  );
}

function supersedeDerivedProjection(
  outcome: WorkspaceDerivedReconciliationOutcome,
) {
  outcome.topology = "superseded";
  outcome.sourceGraph = "superseded";
  outcome.scss = "superseded";
  return outcome;
}

type WorkspaceDerivedReconciliationRequest = {
  host: ProjectControllerHost;
  options: ReconcileWorkspaceDerivedStateOptions;
  resolve: (outcome: WorkspaceDerivedReconciliationOutcome) => void;
  reject: (error: unknown) => void;
};

type WorkspaceDerivedReconciliationLane = {
  activeOptions: ReconcileWorkspaceDerivedStateOptions | null;
  pending: WorkspaceDerivedReconciliationRequest | null;
  running: boolean;
};

const workspaceDerivedReconciliationLanes = new Map<
  string,
  WorkspaceDerivedReconciliationLane
>();

function workspaceDerivedReconciliationKey(
  options: ReconcileWorkspaceDerivedStateOptions,
) {
  return `${options.expectedProjectRoot}\u0000${options.expectedSessionId}`;
}

function supersededDerivedProjectionFor(
  options: ReconcileWorkspaceDerivedStateOptions,
) {
  return supersedeDerivedProjection(
    derivedProjectionOutcome(options.expectedWorkspaceRevision),
  );
}

function mergeWorkspaceDerivedReconciliationOptions(
  previous: ReconcileWorkspaceDerivedStateOptions | null,
  next: ReconcileWorkspaceDerivedStateOptions,
): ReconcileWorkspaceDerivedStateOptions {
  if (!previous) return next;
  return {
    ...next,
    topologyChanged: previous.topologyChanged || next.topologyChanged,
    refreshSourceGraph: (previous.refreshSourceGraph ?? true)
      || (next.refreshSourceGraph ?? true),
    refreshScss: (previous.refreshScss ?? true) || (next.refreshScss ?? true),
  };
}

async function drainWorkspaceDerivedReconciliationLane(
  key: string,
  lane: WorkspaceDerivedReconciliationLane,
) {
  if (lane.running) return;
  lane.running = true;
  try {
    while (lane.pending) {
      const request = lane.pending;
      lane.pending = null;
      lane.activeOptions = request.options;
      try {
        request.resolve(
          await runWorkspaceDerivedStateReconciliation(
            request.host,
            request.options,
          ),
        );
      } catch (error) {
        request.reject(error);
      } finally {
        lane.activeOptions = null;
      }
    }
  } finally {
    lane.running = false;
    if (!lane.pending && workspaceDerivedReconciliationLanes.get(key) === lane) {
      workspaceDerivedReconciliationLanes.delete(key);
    } else if (lane.pending) {
      void drainWorkspaceDerivedReconciliationLane(key, lane);
    }
  }
}

/**
 * Rebuilds replaceable frontend projections for one already-committed Rust
 * revision. It deliberately performs no draft flush, Project Transition or
 * Preview publication.
 */
export async function reconcileWorkspaceDerivedState(
  host: ProjectControllerHost,
  options: ReconcileWorkspaceDerivedStateOptions,
): Promise<WorkspaceDerivedReconciliationOutcome> {
  const key = workspaceDerivedReconciliationKey(options);
  let lane = workspaceDerivedReconciliationLanes.get(key);
  if (!lane) {
    lane = { activeOptions: null, pending: null, running: false };
    workspaceDerivedReconciliationLanes.set(key, lane);
  }

  if (
    (lane.activeOptions?.expectedWorkspaceRevision ?? -1)
      > options.expectedWorkspaceRevision
    || (lane.pending?.options.expectedWorkspaceRevision ?? -1)
      > options.expectedWorkspaceRevision
  ) {
    return supersededDerivedProjectionFor(options);
  }

  const mergedOptions = mergeWorkspaceDerivedReconciliationOptions(
    lane.pending?.options ?? lane.activeOptions,
    options,
  );
  if (lane.pending) {
    lane.pending.resolve(supersededDerivedProjectionFor(lane.pending.options));
  }

  return new Promise<WorkspaceDerivedReconciliationOutcome>((resolve, reject) => {
    lane!.pending = {
      host,
      options: mergedOptions,
      resolve,
      reject,
    };
    void drainWorkspaceDerivedReconciliationLane(key, lane!);
  });
}

async function runWorkspaceDerivedStateReconciliation(
  host: ProjectControllerHost,
  options: ReconcileWorkspaceDerivedStateOptions,
): Promise<WorkspaceDerivedReconciliationOutcome> {
  const outcome = derivedProjectionOutcome(options.expectedWorkspaceRevision);
  const current = () => (
    derivedProjectionSessionStatus(host, options) === "current"
  );
  if (!current()) return supersedeDerivedProjection(outcome);

  if (options.topologyChanged) {
    const currentProject = host.scannedProject;
    if (!currentProject) {
      outcome.topology = "deferred";
    } else {
      try {
        const scanned = await scanProject(options.expectedProjectRoot);
        if (!current()) return supersedeDerivedProjection(outcome);
        if (
          scanned.root !== options.expectedProjectRoot
          || scanned.kernelSessionId !== options.expectedSessionId
          || typeof scanned.workspaceRevision !== "number"
          || !Number.isSafeInteger(scanned.workspaceRevision)
        ) {
          throw new Error(
            t("project-controller-scan-revision-identity-missing"),
          );
        }
        if (scanned.workspaceRevision !== options.expectedWorkspaceRevision) {
          if ((scanned.workspaceRevision ?? -1) > options.expectedWorkspaceRevision) {
            return supersedeDerivedProjection(outcome);
          }
          throw new Error(
            t("project-controller-scan-revision-mismatch", {
              actual: scanned.workspaceRevision,
              expected: options.expectedWorkspaceRevision,
            }),
          );
        }
        const project = preservePreviewBaseUrl(scanned, currentProject);
        host.scannedProject = project;
        host.diskState = diskStateFromProjectScan(project, host.diskState);
        host.projectStatus = planOpenedProject(project).projectStatus;
        const nextFile = selectProjectFileAfterScan(
          project,
          options.preferredRelativePath ?? host.activeScannedPath,
        );
        if (nextFile) {
          await host.loadScannedProjectFile(nextFile, {
            strict: true,
            skipDraftFlush: true,
            deferPreviewRefresh: true,
          });
          if (!current()) return supersedeDerivedProjection(outcome);
        }
      } catch (error) {
        outcome.topology = current() ? "degraded" : "superseded";
        outcome.warnings.push(
          t("project-controller-navigation-resync", { message: errorMessage(error) }),
        );
      }
    }
  }

  if (options.refreshSourceGraph ?? true) {
    try {
      if (host.refreshSourceGraph) {
        await host.refreshSourceGraph({ strict: true });
      } else {
        throw new Error(t("project-controller-source-graph-unavailable"));
      }
      if (!current()) return supersedeDerivedProjection(outcome);
    } catch (error) {
      outcome.sourceGraph = current() ? "degraded" : "superseded";
      outcome.warnings.push(
        t("project-controller-source-graph-preserved", { message: errorMessage(error) }),
      );
    }
  }

  if (options.refreshScss ?? true) {
    try {
      const variables = await getScssVariables(
        createCssRequestIdentity(
          options.expectedProjectRoot,
          options.expectedSessionId,
        ),
        options.expectedWorkspaceRevision,
      );
      if (!current()) return supersedeDerivedProjection(outcome);
      host.scssVariables = variables;
    } catch (error) {
      outcome.scss = current() ? "degraded" : "superseded";
      outcome.warnings.push(
        t("project-controller-scss-preserved", { message: errorMessage(error) }),
      );
    }
  }

  host.startExternalDiskMonitoring?.();
  return outcome;
}

export type CommittedHistoryProjectionContext = Readonly<{
  projectRoot: string;
  runtimeSessionId: string;
  projectSessionEpoch: number;
  workspaceRevision: number;
}>;

/**
 * Reprojects topology for an already committed Rust History revision. The
 * immutable identity is checked around every await; Rust revision checks
 * serialize writes, so no frontend write reservation is required.
 */
export async function rescanCurrentProjectForCommittedHistory(
  host: ProjectControllerHost,
  context: CommittedHistoryProjectionContext,
  preferredRelativePath: string | null = host.activeScannedPath,
  options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
) {
  const requireCurrent = () => {
    if (
      host.sessionProjectRoot !== context.projectRoot
      || host.kernelProjectSessionId !== context.runtimeSessionId
      || host.projectSessionEpoch !== context.projectSessionEpoch
      || (
        host.projectWorkspaceSnapshot
        && host.projectWorkspaceSnapshot.revision > context.workspaceRevision
      )
    ) {
      throw new Error(t("project-controller-history-reprojection"));
    }
  };
  requireCurrent();
  await projectCurrentProjectRescan(host, preferredRelativePath, options, requireCurrent);
  requireCurrent();
}

async function projectCurrentProjectRescan(
  host: ProjectControllerHost,
  preferredRelativePath: string | null,
  options: { strict?: boolean; deferPreviewRefresh?: boolean },
  requireProjectionCurrent: (() => void) | undefined = undefined,
) {
  const requireCurrent = () => requireProjectionCurrent?.();
  requireCurrent();
  const currentProject = host.scannedProject;
  if (!currentProject) return;
  const expectedWorkspaceRevision = host.projectWorkspaceSnapshot?.revision ?? 0;
  const reconciliation = await reconcileWorkspaceDerivedState(host, {
    expectedProjectRoot: host.sessionProjectRoot,
    expectedSessionId: host.kernelProjectSessionId,
    expectedWorkspaceRevision,
    topologyChanged: true,
    preferredRelativePath,
    refreshSourceGraph: true,
    refreshScss: true,
  });
  requireCurrent();
  if (
    options.strict
    && (
      reconciliation.topology === "degraded"
      || reconciliation.sourceGraph === "degraded"
      || reconciliation.scss === "degraded"
      || reconciliation.topology === "deferred"
      || reconciliation.sourceGraph === "deferred"
      || reconciliation.scss === "deferred"
      || reconciliation.topology === "superseded"
      || reconciliation.sourceGraph === "superseded"
      || reconciliation.scss === "superseded"
    )
  ) {
    throw new Error(
      reconciliation.warnings.join(" ")
        || t("project-controller-strict-rescan-unconfirmed"),
    );
  }
  if (!options.deferPreviewRefresh) {
    host.refreshToken += 1;
    const previewRefreshed = await host.requestPreviewRefresh("project-rescan");
    requireCurrent();
    if (options.strict && !previewRefreshed) {
      throw new Error(t("project-controller-strict-rescan-preview-unconfirmed"));
    }
  }
  host.startExternalDiskMonitoring?.();
  host.setGlobalStatus(t("project-controller-structure-rescanned"), "restored");
}

export async function discardSessionAndReloadFromDisk(
  host: ProjectControllerHost,
  preferredRelativePath: string | null = host.activeScannedPath,
) {
  return await reloadCurrentProjectFromDisk(host, preferredRelativePath, { mode: "discard" });
}

export function captureBrowserPreviewRequestIdentity(
  host: Pick<
    ProjectControllerHost,
    "scannedProject" | "sessionProjectRoot" | "kernelProjectSessionId"
  >,
): BrowserPreviewRequestIdentity | null {
  const projectRoot = host.scannedProject?.root.trim() ?? "";
  const runtimeSessionId = host.kernelProjectSessionId.trim();
  const expectedDiskGeneration = host.scannedProject?.acceptedDiskGeneration;
  if (
    !host.scannedProject
    || !projectRoot
    || !runtimeSessionId
    || host.sessionProjectRoot.trim() !== projectRoot
    || !Number.isSafeInteger(expectedDiskGeneration)
    || (expectedDiskGeneration ?? 0) < 1
  ) {
    return null;
  }
  return {
    expectedProjectRoot: projectRoot,
    expectedSessionId: runtimeSessionId,
    expectedDiskGeneration: expectedDiskGeneration as number,
  };
}

export function isBrowserPreviewRequestIdentityCurrent(
  host: Pick<
    ProjectControllerHost,
    "scannedProject" | "sessionProjectRoot" | "kernelProjectSessionId"
  >,
  identity: BrowserPreviewRequestIdentity,
) {
  return host.scannedProject?.root === identity.expectedProjectRoot
    && host.scannedProject !== null
    && host.scannedProject.acceptedDiskGeneration === identity.expectedDiskGeneration
    && host.sessionProjectRoot === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId;
}

export async function openCurrentProjectInBrowser(
  host: ProjectControllerHost,
  dependencies: BrowserPreviewDependencies = browserPreviewDependencies,
  options: BrowserPreviewOpenOptions = {},
) {
  if (!host.scannedProject) {
    host.setGlobalStatus(t("project-controller-browser-project-required"), "error");
    return;
  }

  const identity = captureBrowserPreviewRequestIdentity(host);
  if (!identity) {
    host.setGlobalStatus(
      t("project-controller-browser-session-required"),
      "error",
    );
    return;
  }

  host.setGlobalStatus(t("project-controller-browser-rendering"), "saving");
  try {
    const receipt = await dependencies.start(identity);
    if (!isBrowserPreviewRequestIdentityCurrent(host, identity)) return;
    if (!receipt) {
      host.setGlobalStatus(t("project-controller-browser-unavailable"), "error");
      return;
    }
    if (
      receipt.projectRoot !== identity.expectedProjectRoot
      || receipt.runtimeSessionId !== identity.expectedSessionId
      || receipt.acceptedDiskGeneration !== identity.expectedDiskGeneration
    ) {
      throw new Error(
        t("project-controller-browser-receipt-mismatch"),
      );
    }
    // No await is allowed between this final UI CAS and dispatching the opener
    // IPC. A project transition that resumes the old promise therefore cannot
    // open its obsolete URL or overwrite the next session's status.
    if (!isBrowserPreviewRequestIdentityCurrent(host, identity)) return;
    const targetUrl = sourceBrowserUrlForRoute(receipt.url, options.route);
    await dependencies.openUrl(targetUrl);
    if (!isBrowserPreviewRequestIdentityCurrent(host, identity)) return;
    host.clearNotification("project.browser-preview.warning");
    host.setGlobalStatus(t("project-controller-browser-opened", { url: targetUrl }), "restored");
  } catch (error) {
    if (!isBrowserPreviewRequestIdentityCurrent(host, identity)) return;
    const message = t("project-controller-browser-failed", { message: errorMessage(error) });
    host.escalateGlobalStatus({
      id: "project.browser-preview.warning",
      level: "warning",
      title: t("project-controller-browser-unavailable-title"),
      message,
      statusMessage: message,
    });
  }
}

export function sourceBrowserUrlForRoute(baseUrl: string, requestedRoute?: string | null) {
  const route = requestedRoute?.trim() || "/";
  if (!route.startsWith("/") || route.startsWith("//")) {
    throw new Error(t("project-controller-browser-path-absolute"));
  }
  if (route === "/") return baseUrl;

  const base = new URL(baseUrl);
  const target = new URL(route, `${base.origin}/`);
  if (target.origin !== base.origin || target.pathname.startsWith("/__pana_source/")) {
    throw new Error(t("project-controller-browser-path-escaped"));
  }
  return target.toString();
}

export async function closeCurrentProject(
  host: ProjectControllerHost,
  options: {
    operatorDecisionId?: string | null;
    detachedProjectRoot?: string | null;
  } = {},
) {
  const detachedProjectRoot = host.scannedProject ? null : options.detachedProjectRoot?.trim() || null;
  const projectRoot = host.scannedProject?.root ?? detachedProjectRoot;
  if (!projectRoot) return false;
  await host.beginProjectTransitionFrontendLease?.();
  let transitionAllowed = false;
  try {
    if (host.scannedProject) {
      await flushProjectDraftsBeforeTransition();
    }
    transitionAllowed = await prepareProjectTransitionForTarget(
      host,
      projectRoot,
      { kind: "close_project" },
      options.operatorDecisionId ?? null,
    );
    if (transitionAllowed) {
      await host.invalidateExternalReconcileForProjectTransition?.();
    }
  } catch (error) {
    host.endProjectTransitionFrontendLease?.();
    throw error;
  }
  if (!transitionAllowed) {
    host.endProjectTransitionFrontendLease?.();
    return false;
  }

  host.projectStatus = t("project-controller-closing");
  host.setGlobalStatus(t("project-controller-closing"), "saving");
  let rustSessionClosed = false;
  try {
    await closeProject(options.operatorDecisionId ?? undefined);
    rustSessionClosed = true;
    host.projectLifecycle = await readProjectLifecycle();
    resetProjectSessionState(host, true, "");
    resetProjectScopedState(host);
    host.scannedProject = null;
    host.source = "";
    host.activeScannedPath = null;
    host.previewSrc = "about:blank";
    host.activePreviewPath = "about:blank";
    host.previewWorkspaceRevision = null;
    host.previewDocumentMarkup = null;
    host.projectStatus = t("project-controller-no-project");
    host.projectOpenRecoveryDecisionRequest = null;
    host.projectTransitionDecisionRequest = null;
    host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
    host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
    host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
    host.clearNotification("project.preview.warning");
    host.clearNotification("project.not-zola");
    host.setGlobalStatus(t("project-controller-closed"), "restored");
    return true;
  } catch (error) {
    if (!rustSessionClosed) host.resumeExternalMonitoringAfterFailedTransition?.();
    const message = t("project-controller-close-failed", {
      message: errorMessage(error),
    });
    host.projectStatus = message;
    host.setGlobalStatus(message, "error");
    if (detachedProjectRoot) throw error;
    return false;
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }
}

async function reloadCurrentProjectFromDisk(
  host: ProjectControllerHost,
  preferredRelativePath: string | null,
  options: {
    mode: "purge" | "discard";
    operatorDecisionId?: string | null;
  },
): Promise<ProjectReloadOutcome> {
  if (!host.scannedProject) {
    return {
      status: "cancelled",
      projectSessionId: null,
      message: t("project-controller-reload-no-project"),
    };
  }
  const projectRoot = host.scannedProject.root;
  await host.beginProjectTransitionFrontendLease?.();
  let transitionAllowed = false;
  try {
    await flushProjectDraftsBeforeTransition();
    transitionAllowed = await prepareProjectTransitionForTarget(
      host,
      projectRoot,
      {
        kind: "reload_project",
        mode: options.mode,
        preferredRelativePath,
      },
      options.operatorDecisionId ?? null,
    );
    if (transitionAllowed) {
      await host.invalidateExternalReconcileForProjectTransition?.();
    }
  } catch (error) {
    host.endProjectTransitionFrontendLease?.();
    throw error;
  }
  if (!transitionAllowed) {
    host.endProjectTransitionFrontendLease?.();
    return {
      status: "cancelled",
      projectSessionId: null,
      message: t("project-controller-reload-not-authorized"),
    };
  }

  const isDiscard = options.mode === "discard";
  host.projectStatus = isDiscard
    ? t("project-controller-reload-discarding")
    : t("project-controller-reload-purging");
  host.setGlobalStatus(
    isDiscard
      ? t("project-controller-reload-restoring-disk")
      : t("project-controller-reload-rebuilding"),
    "saving",
  );
  let rustSessionSwapped = false;
  let publishedProjectSessionId: string | null = null;
  let previewIdentity: FrontendProjectAttachment | null = null;
  let attachmentFailure: string | null = null;
  try {
    const startup = await inspectStartupFolder(projectRoot);
    host.startupFlow = startup;
    const candidate = startup.candidate;
    if (candidate?.kind !== "valid_project") {
      throw new Error(
        candidate?.diagnostics[0]?.message ?? "Proiectul nu mai este un proiect Zola valid.",
      );
    }
    const inspection = await inspectProjectOpen(projectRoot, candidate.snapshotToken);
    const bootstrap = await openProject(
      projectRoot,
      inspection.operationId,
      inspection.candidateToken,
      options.operatorDecisionId ?? undefined,
    );
    const openedProject = bootstrap.project;
    host.projectLifecycle = bootstrap.lifecycle;
    rustSessionSwapped = true;
    publishedProjectSessionId = openedProject.kernelSessionId ?? null;
    previewIdentity = await projectPublishedSessionIntoFrontend(
      host,
      openedProject,
      "reload",
      { bootstrap },
    );
    if (isDiscard) {
      host.diskState = markDiskMutation(host.diskState, "discard", preferredRelativePath);
    }
    host.projectTransitionDecisionRequest = null;
    host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
    host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
  } catch (error) {
    attachmentFailure = errorMessage(error);
    host.projectLifecycle = await readProjectLifecycle().catch(() => host.projectLifecycle);
    if (rustSessionSwapped) {
      await degradeFrontendAttachment(host, attachmentFailure);
      host.markWorkspaceProjectionRecoveryRequired?.(
        t("project-controller-reload-projection-incomplete"),
      );
    } else {
      host.resumeExternalMonitoringAfterFailedTransition?.();
    }
    const message = isDiscard
      ? t("project-controller-reload-discard-failed", { message: attachmentFailure })
      : t("project-controller-reload-purge-failed", { message: attachmentFailure });
    host.projectStatus = message;
    host.setGlobalStatus(message, "error");
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }

  if (attachmentFailure) {
    return {
      status: "failed",
      projectSessionId: publishedProjectSessionId,
      message: attachmentFailure,
    };
  }
  if (!previewIdentity) {
    return {
      status: "completed",
      projectSessionId: publishedProjectSessionId ?? host.kernelProjectSessionId,
      previewStatus: "degraded",
      message: t("project-controller-reload-preview-missing"),
    };
  }

  const previewOutcome = await startPreviewAfterOpen(host, previewIdentity);
  if (previewOutcome.status === "stale") {
    const message = t("project-controller-reload-preview-superseded");
    host.setGlobalStatus(message, "error");
    return {
      status: "failed",
      projectSessionId: previewOutcome.projectSessionId,
      message,
    };
  }

  if (previewOutcome.status === "canonical") {
    host.setGlobalStatus(
      isDiscard
        ? t("project-controller-reload-discard-complete")
        : t("project-controller-reload-purge-complete"),
      "restored",
    );
  }
  void refreshSourceGraphAfterCommit(host, previewIdentity);
  return {
    status: "completed",
    projectSessionId: previewOutcome.projectSessionId,
    previewStatus: previewOutcome.status,
    message: previewOutcome.status === "degraded" ? previewOutcome.message : null,
  };
}

function resetProjectSessionState(host: ProjectControllerHost, shouldResetHistory: boolean, projectRoot = host.scannedProject?.root ?? "") {
  host.sourceCache = {};
  host.overrideRules = {};
  host.variableOverrides = {};
  if (shouldResetHistory) host.cancelPendingHtmlMutations();
  host.clearHtmlPending();
  host.resetInspectorPendingSources();
  host.inspectorPending = createEmptyInspectorPending();
  host.setSessionProjectRoot(projectRoot);
  if (shouldResetHistory) {
  }
  host.clearPreviewSelection({ clearCanvasOverlay: true });
  host.previewDocumentMarkup = null;
  host.browserPreviewRoute = "/";
  host.refreshToken += 1;
}

export async function createContentPageFromInput(
  host: ProjectControllerHost,
  input: { title: string; slug?: string | null; section?: string | null },
): Promise<string | null> {
  if (!host.scannedProject) {
    host.projectStatus = t("project-controller-page-project-required");
    return null;
  }
  const pagePlan = planContentPageCreation(input.title, host.activeScannedPath, {
    slug: input.slug,
    section: input.section,
  });
  if (!pagePlan.ok) {
    host.projectStatus = pagePlan.status;
    return null;
  }
  return await runInPreviewStructuralLane(host, async (lease): Promise<string | null> => {
    host.projectStatus = pagePlan.creatingStatus;
    try {
      const identity = previewStructuralCommandIdentity(lease);
      let receipt: Awaited<ReturnType<typeof createProjectContentPage>>;
      try {
        receipt = await createProjectContentPage({
          section: pagePlan.section,
          slug: pagePlan.slug,
          title: pagePlan.title,
        }, identity);
      } catch (error) {
        if (!previewStructuralSessionLeaseMatches(host, lease)) return null;
        host.projectStatus = t("project-controller-page-create-failed", {
          message: errorMessage(error),
        });
        return null;
      }
      requireCurrentPreviewStructuralSession(host, lease);
      const relativePath = receipt.relativePath;
      if (!relativePath) {
        throw new Error(t("project-controller-page-path-missing"));
      }

      const settlement = await settleProjectWorkspaceMutation(host, receipt, {
        preferredRelativePath: relativePath,
        warningLabel: t("project-controller-page-create-operation"),
      });
      requireCurrentPreviewStructuralSession(host, lease);
      host.setGlobalStatus(
        t("project-controller-page-created-save", { path: relativePath }),
        "unsaved",
      );
      host.projectStatus = settlement.warnings.length > 0
        ? t("project-controller-page-created-resync")
        : t("project-controller-page-created", { path: relativePath });
      return relativePath;
    } catch (error) {
      if (!previewStructuralSessionLeaseMatches(host, lease)) return null;
      host.projectStatus = t("project-controller-page-create-failed", {
        message: errorMessage(error),
      });
      return null;
    }
  });
}

export async function loadScannedProjectFile(
  host: ProjectControllerHost,
  file: ProjectFile,
  options: {
    strict?: boolean;
    skipDraftFlush?: boolean;
    deferPreviewRefresh?: boolean;
    activateTemplateWorkbench?: boolean;
    preferredTemplatePagePath?: string | null;
    preferredTemplateRoute?: string | null;
  } = {},
) {
  if (!host.scannedProject) return;
  const expectedRoot = host.scannedProject.root;
  const expectedSessionId = host.kernelProjectSessionId;
  const expectedSessionEpoch = host.projectSessionEpoch;
  if (!options.skipDraftFlush) await host.flushInteractiveEditorDrafts();
  if (!projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)) return;
  const loadPlan = planScannedProjectFileLoad(file);
  const reusesActiveTemplateContext = loadPlan.isTemplateFile
    && host.templateWorkbenchActive
    && normalizedTemplateContextPath(host.templateWorkbenchTarget)
      === normalizedTemplateContextPath(file.relativePath);
  host.activeScannedPath = file.relativePath;
  host.source = SOURCE_LOADING_SENTINEL;
  host.centerView = loadPlan.centerView;

  // Source loading is independent from the Preview surface. A failed Canvas
  // navigation must never strand CodeMirror on the internal loading sentinel.
  const cachedSource = host.sourceCache[loadPlan.cacheKey];
  if (typeof cachedSource === "string") {
    host.source = cachedSource;
  } else {
    try {
      const snapshot = await readFileBufferText(file.relativePath, {
        expectedProjectRoot: expectedRoot,
        expectedSessionId,
      });
      const text = snapshot.text;
      if (
        host.activeScannedPath !== file.relativePath
        || !projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)
      ) return;
      reanchorFileBufferDraftSyncCursor(file.relativePath, {
        revision: snapshot.revision,
        hash: snapshot.hash,
      });
      host.sourceCache = { ...host.sourceCache, [loadPlan.cacheKey]: text };
      host.source = text;
    } catch (error) {
      if (
        host.activeScannedPath !== file.relativePath
        || !projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)
      ) return;
      if (options.strict) throw error;
      host.source = t("project-controller-file-load-failed", {
        path: file.relativePath,
        message: errorMessage(error),
      });
    }
  }

  if (loadPlan.isPreviewPage) {
    if (host.templateWorkbenchActive) {
      await host.exitTemplateWorkbench({
        deferPreviewRefresh: true,
        returnPath: file.relativePath,
      });
    }
    host.templateWorkbenchPlan = null;
    host.templateWorkbenchPreferredPagePath = null;
    host.templateWorkbenchPreferredRoute = null;
    const previewUrl = host.previewUrlForScannedFile(file);
    host.previewSrc = host.pendingCanvasProjection
      ? bindCanvasCandidateIdentityToPreviewUrl(
          previewUrl,
          host.pendingCanvasProjection.identity,
        )
      : previewUrl;
    host.activePreviewPath = file.relativePath;
    host.browserPreviewRoute = file.previewPath ?? "/";
    host.previewDocumentMarkup = null;
    host.cancelPreviewSync();
  }

  if (loadPlan.isTemplateFile) {
    if (options.activateTemplateWorkbench !== false) {
      await host.updateTemplateWorkbenchContext(
        host.scannedProject,
        file,
        options.preferredTemplatePagePath !== undefined
          ? options.preferredTemplatePagePath
          : reusesActiveTemplateContext
            ? host.templateWorkbenchPreferredPagePath
            : null,
        {
          deferPreviewRefresh: options.deferPreviewRefresh,
          preferredRoute: options.preferredTemplateRoute !== undefined
            ? options.preferredTemplateRoute
            : reusesActiveTemplateContext
              ? host.templateWorkbenchPreferredRoute
              : null,
          strict: options.strict,
        },
      );
    }
  } else if (!loadPlan.isTemplateFile) {
    // Code/style/assets are source documents, not Preview destinations. Keep
    // the mounted page or Template Workbench surface intact while editing them.
  }

  if (loadPlan.isPreviewPage && !options.deferPreviewRefresh) {
    await host.refreshRenderedPreviewDocument();
  }
}

function projectLoadLeaseMatches(
  host: ProjectControllerHost,
  expectedRoot: string,
  expectedSessionId: string,
  expectedSessionEpoch: number,
) {
  return host.scannedProject?.root === expectedRoot
    && host.kernelProjectSessionId === expectedSessionId
    && host.projectSessionEpoch === expectedSessionEpoch;
}

type TemplateWorkbenchUiLease = {
  identity: ProjectPreviewRequestIdentity;
  templatePath: string;
  projectSessionEpoch: number;
  projectWorkspaceMutationEpoch: number;
  activeScannedPath: string | null;
  requestSerial: number;
};

function captureTemplateWorkbenchUiLease(
  host: ProjectControllerHost,
  project: ProjectScan,
  templateFile: ProjectFile,
): TemplateWorkbenchUiLease {
  const identity = createProjectPreviewRequestIdentity(
    host.sessionProjectRoot,
    host.kernelProjectSessionId,
  );
  const templatePath = templateFile.relativePath.trim();
  if (!templatePath) {
    throw new Error(t("project-controller-template-required"));
  }
  if (project.root !== identity.expectedProjectRoot) {
    throw new Error(t("project-controller-template-scan-session-mismatch"));
  }
  host.templateWorkbenchRequestSerial += 1;
  return {
    identity,
    templatePath,
    projectSessionEpoch: host.projectSessionEpoch,
    projectWorkspaceMutationEpoch: host.projectWorkspaceMutationEpoch,
    activeScannedPath: host.activeScannedPath,
    requestSerial: host.templateWorkbenchRequestSerial,
  };
}

function templateWorkbenchUiLeaseMatches(
  host: ProjectControllerHost,
  lease: TemplateWorkbenchUiLease,
): boolean {
  return host.scannedProject?.root === lease.identity.expectedProjectRoot
    && projectPreviewRequestIdentityMatches(
      lease.identity,
      host.sessionProjectRoot,
      host.kernelProjectSessionId,
    )
    && host.projectSessionEpoch === lease.projectSessionEpoch
    && host.projectWorkspaceMutationEpoch === lease.projectWorkspaceMutationEpoch
    && host.templateWorkbenchRequestSerial === lease.requestSerial
    && host.activeScannedPath === lease.activeScannedPath
    && host.activeScannedPath === lease.templatePath;
}

function normalizedTemplateContextPath(path: string | null | undefined) {
  return path?.trim().replaceAll("\\", "/").replace(/^\.\/+/, "") ?? "";
}

function canvasProjectionIdentityMatches(
  left: CanvasProjectionIdentity,
  right: CanvasProjectionIdentity,
) {
  return left.projectRoot === right.projectRoot
    && left.runtimeSessionId === right.runtimeSessionId
    && left.workspaceRevision === right.workspaceRevision
    && left.transactionId === right.transactionId
    && left.previewRevision === right.previewRevision;
}

async function synchronizeActiveCanvasSurfaceRoute(
  host: ProjectControllerHost,
  previewUrl: string,
  expectedIdentity?: CanvasProjectionIdentity,
) {
  const identity = host.activeCanvasIdentity;
  if (!identity) return;
  if (
    expectedIdentity
    && host.projectWorkspaceSnapshot
    && host.projectWorkspaceSnapshot.revision !== expectedIdentity.workspaceRevision
  ) return;
  if (expectedIdentity && !canvasProjectionIdentityMatches(identity, expectedIdentity)) {
    throw new Error(t("project-controller-template-receipt-mismatch"));
  }
  host.activeCanvasUrl = previewUrl;
  await host.refreshEditorNavigationSnapshot?.(identity, previewUrl, { strict: true });
  const readiness = host.projectLifecycle?.activeSession?.readiness.state ?? null;
  if (!readiness || readiness === "ready" || readiness === "degraded") return;
  const lifecycle = await readProjectLifecycle();
  if (
    lifecycle.activeSession?.projectRoot === identity.projectRoot
    && lifecycle.activeSession.runtimeSessionId === identity.runtimeSessionId
  ) {
    host.projectLifecycle = lifecycle;
  }
}

export async function updateTemplateWorkbenchContext(
  host: ProjectControllerHost,
  project: ProjectScan,
  templateFile: ProjectFile,
  preferredPagePath: string | null = null,
  options: {
    deferPreviewRefresh?: boolean;
    expectedWorkspaceRevision?: number;
    minimumWorkspaceRevision?: number;
    preferredRoute?: string | null;
    strict?: boolean;
  } = {},
) {
  // Identitatea este capturată înaintea primului await; un rezultat depășit nu
  // poate schimba ținta vizuală a unei sesiuni sau revizii mai noi.
  const lease = captureTemplateWorkbenchUiLease(host, project, templateFile);
  try {
    const workspace = await readProjectWorkspaceState();
    if (!templateWorkbenchUiLeaseMatches(host, lease)) return null;
    if (
      !workspace
      || workspace.projectRoot !== lease.identity.expectedProjectRoot
      || workspace.runtimeSessionId !== lease.identity.expectedSessionId
    ) {
      throw new Error(
        t("project-controller-template-session-revision-missing"),
      );
    }
    const minimumRevision = options.minimumWorkspaceRevision;
    const expectedRevision = options.expectedWorkspaceRevision;
    if (
      minimumRevision !== undefined
      && (!Number.isSafeInteger(minimumRevision) || minimumRevision < 0)
    ) {
      throw new Error(t("project-controller-template-min-revision-invalid"));
    }
    if (minimumRevision !== undefined && workspace.revision < minimumRevision) {
      throw new Error(
        t("project-controller-template-revision-too-old", {
          actual: workspace.revision,
          minimum: minimumRevision,
        }),
      );
    }
    if (
      expectedRevision !== undefined
      && (!Number.isSafeInteger(expectedRevision) || expectedRevision < 0)
    ) {
      throw new Error(t("project-controller-template-min-revision-invalid"));
    }
    // The canonical Preview candidate belongs to one exact workspace
    // revision. If another inspector mutation already advanced Rust, this
    // request is superseded; asking Template Workbench for the newer revision
    // here races ahead of its not-yet-materialized Preview generation.
    if (expectedRevision !== undefined && workspace.revision !== expectedRevision) {
      return null;
    }

    const request: TemplateWorkbenchPreviewRequest = {
      ...lease.identity,
      expectedWorkspaceRevision: expectedRevision ?? workspace.revision,
      templatePath: lease.templatePath,
      preferredPagePath,
      preferredRoute: options.preferredRoute ?? null,
    };
    const receipt = await projectTemplateWorkbenchPreview(request);
    if (
      !templateWorkbenchUiLeaseMatches(host, lease)
      || (
        expectedRevision !== undefined
        && host.projectWorkspaceSnapshot?.revision !== expectedRevision
      )
    ) return null;
    if (
      receipt.workspaceRevision !== request.expectedWorkspaceRevision
      || receipt.canvasProjection.identity.projectRoot !== request.expectedProjectRoot
      || receipt.canvasProjection.identity.runtimeSessionId !== request.expectedSessionId
      || receipt.canvasProjection.identity.workspaceRevision !== request.expectedWorkspaceRevision
      || receipt.canvasProjection.identity.previewRevision !== receipt.previewRevision
      || !["prepared", "canonicalVerified"].includes(receipt.canvasProjection.phase)
      || !receipt.previewUrl?.trim()
      || !receipt.route?.startsWith("/__pana_workbench/")
      || receipt.plan.activeTemplate.file !== lease.templatePath
    ) {
      throw new Error(
        t("project-controller-template-receipt-mismatch"),
      );
    }
    const requestedContextPath = normalizedTemplateContextPath(preferredPagePath);
    if (
      requestedContextPath
      && normalizedTemplateContextPath(receipt.plan.selectedContext?.pageFile) !== requestedContextPath
    ) {
      throw new Error(
        t("project-controller-template-page-unconfirmed", { path: requestedContextPath }),
      );
    }
    const requestedRoute = options.preferredRoute?.trim() ?? "";
    if (
      requestedRoute
      && normalizedTemplateContextPath(receipt.plan.selectedRoute?.url) !== normalizedTemplateContextPath(requestedRoute)
    ) {
      throw new Error(
        t("project-controller-template-route-unconfirmed", { route: requestedRoute }),
      );
    }

    if (!host.templateWorkbenchActive) {
      host.templateWorkbenchReturnPreviewPath = host.activePreviewPath;
    }
    host.templateWorkbenchActive = true;
    host.templateWorkbenchTarget = lease.templatePath;
    host.templateWorkbenchPlan = receipt.plan;
    host.templateWorkbenchPreferredPagePath = receipt.plan.selectedContext?.pageFile ?? null;
    host.templateWorkbenchPreferredRoute = receipt.plan.selectedRoute?.url ?? null;
    host.activePreviewPath = lease.templatePath;
    host.previewDocumentMarkup = null;
    if (receipt.canvasProjection.phase === "prepared") {
      const reconciled = await host.reconcileTemplateWorkbenchPreviewDocument(
        receipt.previewUrl,
        receipt.canvasProjection,
      );
      if (!reconciled) {
        throw new Error(
          t("project-controller-template-canvas-unconfirmed"),
        );
      }
    } else {
      host.previewSrc = receipt.previewUrl;
      if (!options.deferPreviewRefresh) await host.refreshRenderedPreviewDocument();
    }
    if (
      !templateWorkbenchUiLeaseMatches(host, lease)
      || (
        expectedRevision !== undefined
        && host.projectWorkspaceSnapshot?.revision !== expectedRevision
      )
    ) return null;
    await synchronizeActiveCanvasSurfaceRoute(
      host,
      receipt.previewUrl,
      receipt.canvasProjection.identity,
    );
    if (
      !templateWorkbenchUiLeaseMatches(host, lease)
      || (
        expectedRevision !== undefined
        && host.projectWorkspaceSnapshot?.revision !== expectedRevision
      )
    ) return null;
    const contextStatus = receipt.plan.selectedContext
      ? t("project-controller-template-context-active-page", {
        name: receipt.plan.activeTemplate.name,
        title: receipt.plan.selectedContext.pageTitle,
        url: receipt.plan.selectedContext.pageUrl,
      })
      : receipt.plan.selectedRoute
        ? t("project-controller-template-context-active-route", {
          name: receipt.plan.activeTemplate.name,
          label: receipt.plan.selectedRoute.label,
          url: receipt.plan.selectedRoute.url,
        })
        : t("project-controller-template-context-active", {
          name: receipt.plan.activeTemplate.name,
        });
    host.setGlobalStatus(contextStatus, "restored");
    const selectedPageFile = receipt.plan.selectedContext?.pageFile ?? null;
    return selectedPageFile
      ? project.files.find(
        (file) => file.role === "page" && file.relativePath === selectedPageFile,
      ) ?? null
      : null;
  } catch (error) {
    if (!templateWorkbenchUiLeaseMatches(host, lease)) return null;
    if (options.strict) throw error;
    host.setGlobalStatus(t("project-controller-template-context-unavailable", {
      message: errorMessage(error),
    }), "error");
    return null;
  }
}

export async function exitTemplateWorkbench(
  host: ProjectControllerHost,
  options: {
    deferPreviewRefresh?: boolean;
    returnPath?: string | null;
  } = {},
) {
  if (!host.templateWorkbenchActive) return;
  host.templateWorkbenchRequestSerial += 1;
  const returnPath = options.returnPath !== undefined
    ? options.returnPath
    : host.templateWorkbenchReturnPreviewPath;
  const workbenchPlan = host.templateWorkbenchPlan;
  const returnPage = returnPath
    ? host.scannedProject?.files.find((file) => file.relativePath === returnPath && file.role === "page")
    : null;
  const fallbackPage = returnPage
    ?? host.scannedProject?.files.find((file) => file.role === "page" && file.previewPath === "/")
    ?? host.scannedProject?.files.find((file) => file.role === "page")
    ?? null;
  const canonicalContextRoute = returnPath
    ? workbenchPlan?.consumers?.find((consumer) => consumer.pageFile === returnPath)?.pageUrl
      ?? (workbenchPlan?.selectedContext?.pageFile === returnPath
        ? workbenchPlan.selectedContext.pageUrl
        : null)
    : null;
  const previousPreview = {
    src: host.previewSrc,
    activePath: host.activePreviewPath,
    browserRoute: host.browserPreviewRoute,
    markup: host.previewDocumentMarkup,
    canvasUrl: host.activeCanvasUrl,
  };
  if (fallbackPage) {
    const previewBaseUrl = host.scannedProject?.previewBaseUrl ?? host.previewSrc;
    const route = canonicalContextRoute ?? fallbackPage.previewPath ?? "/";
    host.previewSrc = new URL(route, previewBaseUrl).toString();
    host.activePreviewPath = fallbackPage.relativePath;
    host.browserPreviewRoute = route;
  } else {
    const previewBaseUrl = host.scannedProject?.previewBaseUrl ?? null;
    host.previewSrc = previewBaseUrl ?? "about:blank";
    host.activePreviewPath = previewBaseUrl ?? "about:blank";
    host.browserPreviewRoute = "/";
  }
  host.previewDocumentMarkup = null;
  try {
    if (host.previewSrc !== "about:blank" && !options.deferPreviewRefresh) {
      await host.refreshRenderedPreviewDocument();
    }
    if (
      host.previewSrc
      && host.previewSrc !== "about:blank"
      && !options.deferPreviewRefresh
    ) {
      await synchronizeActiveCanvasSurfaceRoute(host, host.previewSrc);
    }
  } catch (error) {
    host.previewSrc = previousPreview.src;
    host.activePreviewPath = previousPreview.activePath;
    host.browserPreviewRoute = previousPreview.browserRoute;
    host.previewDocumentMarkup = previousPreview.markup;
    host.activeCanvasUrl = previousPreview.canvasUrl;
    throw error;
  }
  host.templateWorkbenchActive = false;
  host.templateWorkbenchTarget = null;
  host.templateWorkbenchReturnPreviewPath = null;
  host.templateWorkbenchPlan = null;
  host.templateWorkbenchPreferredPagePath = null;
  host.templateWorkbenchPreferredRoute = null;
  host.setGlobalStatus(t("project-controller-template-closed"), "idle");
}
