import { tick } from "svelte";
import { openUrl as openExternalUrl } from "@tauri-apps/plugin-opener";
import {
  resetFileBufferDraftSyncState,
  setFileBufferDraftSyncSession,
} from "$lib/session/file-buffer-draft-sync";
import {
  resetPageJsDraftSyncState,
  setPageJsDraftSyncSession,
} from "$lib/session/page-js-draft-sync";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import { createDiskState, diskStateFromProjectScan, markDiskMutation, type DiskState } from "$lib/session/disk-state";
import {
  closeProject,
  createCssRequestIdentity,
  createProjectPreviewRequestIdentity,
  createProjectContentPage,
  chooseProjectFolder,
  getScssVariables,
  inspectProjectOpenRecovery,
  openProject,
  readFileBufferStore,
  readProjectAppConfig,
  readProjectFile,
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
  zolaInit,
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
  PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID,
  PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
  projectTransitionActionForContinuation,
  type ProjectTransitionContinuation,
  type ProjectTransitionDecisionRequest,
} from "$lib/project/transition-decision";
import { resolveZolaIndexTemplateFile } from "$lib/project/zola-index";
import type {
  CenterView,
  HtmlPendingArea,
  InspectorPendingArea,
  ProjectFile,
  ProjectOpenRecoveryDecisionInput,
  ProjectScan,
  SaveState,
	  ScssVariable,
	  SelectionInfo,
	  SourceEditLocation,
	  SourceGraph,
    TemplateWorkbenchPlan,
} from "$lib/types";
import { errorMessage } from "$lib/util";
import {
  markProjectWorkspacePreviewPublished,
  resetProjectWorkspacePreviewCoordinator,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  requireCurrentKernelUndoRedoProjectionLease,
  type KernelUndoRedoProjectionLease,
} from "$lib/kernel/undo-redo-projection-lease";
import {
  previewStructuralCommandIdentity,
  previewStructuralSessionLeaseMatches,
  requireCurrentPreviewStructuralSession,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";

type OpenProjectRootOptions = {
  operatorDecisionId?: string | null;
  recoveryDecision?: ProjectOpenRecoveryDecisionInput | null;
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
};

const projectPreviewDependencies: ProjectPreviewDependencies = {
  start: startProjectPreview,
};

async function flushProjectDraftsBeforeTransition(host: ProjectControllerHost) {
  await flushWorkspaceMutationInputs("manual");
  await host.drainMoodBoardSaveBeforeTransition();
}

function createEmptyInspectorPending(): Record<InspectorPendingArea, boolean> {
  return { html: false, css: false, vars: false, js: false };
}

function createEmptyHtmlPending(): Record<HtmlPendingArea, boolean> {
  return { tag: false, attributes: false, text: false, image: false, classes: false, structure: false };
}

export type ProjectControllerHost = {
  selectedClass: string;
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
  templateWorkbenchActive: boolean;
  templateWorkbenchTarget: string | null;
  templateWorkbenchReturnPreviewPath: string | null;
  templateWorkbenchRequestSerial: number;
  selectedPreviewElement: Element | null;
  selectedElement: SelectionInfo | null;
  lastMeaningfulSelectedElement: SelectionInfo | null;
  lastSelectedImageElement: SelectionInfo | null;
  overrideRules: Record<string, unknown>;
  variableOverrides: Record<string, string>;
  htmlPending: Record<HtmlPendingArea, boolean>;
  inspectorPending: Record<InspectorPendingArea, boolean>;
  resetInspectorPendingSources: () => void;
  pendingTag: string | null;
  pendingTagOriginal: string | null;
  pendingTagSourceLocation: SourceEditLocation | null;
  tagStatus: string;
  saveState: SaveState;
  saveStatus: string;
  projectStatus: string;
  scannedProject: ProjectScan | null;
  projectOpenRecoveryDecisionRequest: ProjectOpenRecoveryDecisionRequest | null;
  projectTransitionDecisionRequest: ProjectTransitionDecisionRequest | null;
  sourceGraph: SourceGraph | null;
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
  beginPreviewStructuralWriteBoundary: () => Promise<void>;
  endPreviewStructuralWriteBoundary: () => void;
  historyPanelOpen: boolean;
  settingsPanelOpen: boolean;
  activeVersionPreview: unknown | null;
  reattachCurrentProjectSession?: () => Promise<boolean>;
  flushInteractiveEditorDrafts: () => Promise<void>;
  beginProjectTransitionFrontendLease?: () => Promise<void>;
  endProjectTransitionFrontendLease?: () => void;
  drainMoodBoardSaveBeforeTransition: () => Promise<void>;
  loadScannedProjectFile: (
    file: ProjectFile,
    options?: {
      strict?: boolean;
      skipDraftFlush?: boolean;
      deferPreviewRefresh?: boolean;
      activateTemplateWorkbench?: boolean;
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
      minimumWorkspaceRevision?: number;
      strict?: boolean;
    },
  ) => Promise<ProjectFile | null>;
  setSessionProjectRoot: (projectRoot?: string) => void;
  cancelPendingHtmlMutations: () => void;
  clearPreviewSelection: (options?: { clearTemplateGate?: boolean; clearHtmlMarker?: boolean }) => void;
  clearHtmlPending: () => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  requestPreviewRefresh: (reason: "project-rescan" | "discard") => Promise<boolean>;
  rescanCurrentProject: (
    preferredRelativePath?: string | null,
    options?: { strict?: boolean },
  ) => Promise<void>;
  refreshRenderedPreviewDocument: () => Promise<boolean>;
  prepareCanvasProjectionNavigation: (plan: CanvasProjectionPlan) => Promise<void>;
  reconcileTemplateWorkbenchPreviewDocument: (
    previewUrl: string,
    plan: CanvasProjectionPlan,
  ) => Promise<boolean>;
  previewUrlForScannedFile: (file: ProjectFile) => string;
  exitTemplateWorkbench: (options?: { deferPreviewRefresh?: boolean }) => Promise<void>;
  cancelPreviewSync: () => void;
  resetPageSections?: () => void;
  resetProjectLoopDefinitions?: () => void;
  loadProjectLoopDefinitions?: (projectRoot: string) => void;
  refreshSourceGraph?: (options?: { strict?: boolean }) => Promise<void>;
  resetControlledPreviewState?: () => void;
  scheduleZolaValidation?: (reason?: "project-open") => void;
  notify: (notification: {
    id: string;
    level: "info" | "warning" | "error";
    title: string;
    message: string;
    actionLabel?: string | null;
  }) => void;
  clearNotification: (id: string) => void;
  establishExternalDiskBaseline?: () => Promise<void>;
  startExternalDiskPolling?: () => void;
  resetExternalDiskState?: () => void;
  invalidateExternalReconcileForProjectTransition?: () => Promise<void>;
  resumeExternalMonitoringAfterFailedTransition?: () => void;
  markWorkspaceProjectionRecoveryRequired?: (message: string) => void;
};

export async function openProjectFolder(host: ProjectControllerHost) {
  console.info("[Pană Studio] openProjectFolder invoked");
  if (!host.scannedProject) {
    await host.reattachCurrentProjectSession?.();
  }
  host.projectStatus = "Alege un folder...";
  host.setGlobalStatus("Alege un folder de proiect.", "saving");
  await tick();
  try {
    console.info("[Pană Studio] requesting project folder from dialog");
    const selected = await chooseProjectFolder();
    console.info("[Pană Studio] project folder dialog returned", selected);
    if (!selected || Array.isArray(selected)) {
      host.projectStatus = "Alegerea folderului a fost anulată.";
      host.setGlobalStatus("Alegerea folderului a fost anulată.", "restored");
      return;
    }
    host.projectStatus = "Se scanează folderul ales și se pornește preview-ul local...";
    host.setGlobalStatus(`Se deschide proiectul: ${selected}`, "saving");
    await tick();
    await openProjectRoot(host, selected);
  } catch (error) {
    const message = `Deschiderea dosarului a eșuat: ${errorMessage(error)}`;
    host.projectStatus = message;
    host.setGlobalStatus(message, "error");
    host.notify({
      id: "project.open.error",
      level: "error",
      title: "Deschiderea proiectului a eșuat",
      message,
    });
  }
}

type FrontendProjectAttachmentMode = "open" | "reattach" | "reload";

export type ProjectPreviewStartOutcome =
  | { status: "canonical"; projectSessionId: string }
  | { status: "degraded"; projectSessionId: string; message: string }
  | { status: "stale"; projectSessionId: string };

export type ProjectReloadOutcome =
  | {
      status: "completed";
      projectSessionId: string;
      previewStatus: "canonical" | "degraded";
      message: string | null;
    }
  | { status: "cancelled"; projectSessionId: null; message: string }
  | { status: "failed"; projectSessionId: string | null; message: string };

type FrontendProjectAttachmentOptions = {
  preferredRelativePath?: string | null;
};

function requireProjectAttachmentAuthority(project: ProjectScan) {
  if (!project.kernelSessionId?.trim()) {
    throw new Error("ProjectScan nu conține identitatea runtime a ProjectSession.");
  }
  if (!project.acceptedDiskManifest || !project.acceptedDiskGeneration) {
    throw new Error("ProjectScan nu conține autoritatea AcceptedProjectDiskManifest.");
  }
}

async function projectPublishedSessionIntoFrontend(
  host: ProjectControllerHost,
  project: ProjectScan,
  mode: FrontendProjectAttachmentMode,
  options: FrontendProjectAttachmentOptions = {},
): Promise<ProjectPreviewRequestIdentity | null> {
  requireProjectAttachmentAuthority(project);
  host.projectOpenRecoveryDecisionRequest = null;
  host.projectTransitionDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
  host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
  resetProjectScopedState(host, { preserveExternalReconcileBarrier: true });
  host.scannedProject = project;
  host.kernelProjectSessionId = project.kernelSessionId ?? "";
  host.diskState = diskStateFromProjectScan(project, host.diskState);
  host.setSessionProjectRoot(project.root);
  setFileBufferDraftSyncSession(project.root, host.kernelProjectSessionId);
  setPageJsDraftSyncSession(project.root, host.kernelProjectSessionId);
  host.loadProjectLoopDefinitions?.(project.root);

  if (project.isEmpty) {
    host.sourceGraph = null;
    host.projectStatus = "Dosar gol. Poți inițializa un proiect web Pană Studio.";
    host.setGlobalStatus(
      mode === "reattach" ? "Sesiunea dosarului gol a fost reatașată." : "Dosar gol selectat.",
      "restored",
    );
    host.clearNotification("project.not-zola");
    host.resetExternalDiskState?.();
    return null;
  }

  if (!project.isZola) {
    host.sourceGraph = null;
    host.projectStatus = "Acest dosar nu este un proiect Pană Studio valid.";
    host.setGlobalStatus("Pană Studio așteaptă un proiect cu Zola în sursa/.", "error");
    host.notify({
      id: "project.not-zola",
      level: "warning",
      title: "Dosarul nu este proiect Pană Studio",
      message: "Alege root-ul proiectului complet, cu Zola în sursa/, sau un dosar gol pentru inițializare.",
    });
    host.resetExternalDiskState?.();
    return null;
  }

  const openPlan = planOpenedProject(project);
  await host.refreshSourceGraph?.({ strict: true });
  host.clearNotification("project.not-zola");

  // Recovery and editable buffers are restored by ProjectWorkspace before the
  // frontend receives ProjectScan. The browser only rebuilds its projection.
  const fileBuffers = await readFileBufferStore();
  if (
    !fileBuffers
    || fileBuffers.projectRoot !== project.root
    || fileBuffers.runtimeSessionId !== host.kernelProjectSessionId
  ) {
    throw new Error("Sesiunea proiectului nu corespunde sesiunii Rust publicate.");
  }
  const dirtyWorkspacePaths = fileBuffers.files
    .filter((file) => file.dirty)
    .map((file) => file.relativePath)
    .sort();
  const restoredDirtySession = dirtyWorkspacePaths.length > 0;

  host.projectStatus = openPlan.projectStatus;
  if (project.previewWarning) {
    host.setGlobalStatus(`Previzualizare indisponibilă: ${project.previewWarning}`, "error");
    host.notify({
      id: "project.preview.warning",
      level: "warning",
      title: "Previzualizare indisponibilă",
      message: project.previewWarning,
    });
  }
  if (openPlan.targetCssFile) host.targetCssFile = openPlan.targetCssFile;
  host.cachebustAssets = await readProjectAppConfig()
    .then((config) => config.cachebustAssets)
    .catch(() => false);
  const preferredFile = options.preferredRelativePath
    ? project.files.find((file) => file.relativePath === options.preferredRelativePath) ?? null
    : null;
  const fileToOpen = preferredFile
    ?? (await resolveZolaIndexTemplateFile(project, host.sourceCache, (_relativePath, cacheKey, source) => {
      host.sourceCache = { ...host.sourceCache, [cacheKey]: source };
    })) ?? openPlan.fileToOpen;

  if (fileToOpen) {
    await host.loadScannedProjectFile(fileToOpen, {
      strict: true,
      skipDraftFlush: true,
      // Source selection is established now; Workbench may only be requested
      // after the canonical Preview generation has been mounted and accepted.
      activateTemplateWorkbench: false,
      syncWorkbench: false,
    });
  }
  if (mode === "reattach" && !project.previewWarning) {
    const detail = dirtyWorkspacePaths.length > 0
      ? ` ${dirtyWorkspacePaths.length} fișier(e) nesalvat(e) au fost restaurate din ProjectWorkspace.`
      : "";
    host.setGlobalStatus(
      `Sesiunea Rust activă a fost reatașată.${detail}`,
      restoredDirtySession ? "unsaved" : "restored",
    );
    host.clearNotification("project.preview.warning");
  } else if (!restoredDirtySession && !project.previewWarning) {
    host.setGlobalStatus("Proiect încărcat din fișierele de pe disc.", "restored");
    host.clearNotification("project.preview.warning");
  }
  host.scssVariables = await getScssVariables(
    createCssRequestIdentity(project.root, host.kernelProjectSessionId),
  );
  host.resetExternalDiskState?.();
  await host.establishExternalDiskBaseline?.();
  host.startExternalDiskPolling?.();
  return {
    expectedProjectRoot: project.root,
    expectedSessionId: host.kernelProjectSessionId,
  };
}

export async function reattachCurrentProjectSession(host: ProjectControllerHost): Promise<boolean> {
  if (host.scannedProject) return true;
  await host.beginProjectTransitionFrontendLease?.();
  let previewIdentity: ProjectPreviewRequestIdentity | null = null;
  try {
    const project = await reattachProjectSession();
    if (!project) return false;
    previewIdentity = await projectPublishedSessionIntoFrontend(host, project, "reattach");
    host.clearNotification("project.reattach.error");
  } catch (error) {
    const message = `Reatașarea sesiunii Rust a eșuat: ${errorMessage(error)}`;
    host.projectStatus = message;
    host.setGlobalStatus(message, "error");
    host.notify({
      id: "project.reattach.error",
      level: "error",
      title: "Sesiunea activă nu a putut fi reproiectată",
      message,
    });
    throw error;
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }
  if (previewIdentity) await startPreviewAfterOpen(host, previewIdentity);
  await host.restoreWorkbenchState?.();
  return true;
}

export async function initZolaProject(host: ProjectControllerHost) {
  if (!host.scannedProject) return;
  host.projectStatus = "Se inițializează proiectul web Pană Studio...";
  host.setGlobalStatus("Se inițializează proiectul web Pană Studio...", "saving");
  try {
    await zolaInit(host.scannedProject.root);
    await openProjectRoot(host, host.scannedProject.root);
    host.setGlobalStatus("Proiect web Pană Studio inițializat.", "restored");
  } catch (error) {
    const message = `Init eșuat: ${errorMessage(error)}`;
    host.projectStatus = message;
    host.setGlobalStatus(message, "error");
    host.notify({
      id: "project.init.error",
      level: "error",
      title: "Inițializarea proiectului a eșuat",
      message,
    });
  }
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
    await flushProjectDraftsBeforeTransition(host);
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
  if (openAction === "open_project") {
    try {
      const assessment = await inspectProjectOpenRecovery(root);
      if (assessment.status === "decision_required") {
        const suppliedToken = options.recoveryDecision?.assessmentToken ?? null;
        if (!suppliedToken) {
          const request = createProjectOpenRecoveryDecisionRequest(
            root,
            assessment,
            options.operatorDecisionId ?? null,
          );
          host.projectOpenRecoveryDecisionRequest = request;
          host.projectStatus = "Deschiderea așteaptă decizia pentru sesiunea recuperabilă veche.";
          host.setGlobalStatus("Recovery incompatibil detectat pentru dosarul ales.", "idle");
          host.notify({
            id: PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
            level: "warning",
            title: "Sesiune recuperabilă incompatibilă",
            message: `${assessment.diagnostic ?? "Dosarul ales nu mai corespunde sesiunii recuperabile."} Recovery-ul rămâne intact până la o alegere explicită.`,
          });
          host.endProjectTransitionFrontendLease?.();
          return;
        }
        if (suppliedToken !== assessment.assessmentToken) {
          throw new Error(
            "Recovery-ul s-a schimbat după confirmare; verifică din nou alegerea.",
          );
        }
      } else if (options.recoveryDecision) {
        throw new Error(
          "Decizia de abandonare nu mai corespunde recuperării inspectate.",
        );
      }
    } catch (error) {
      host.endProjectTransitionFrontendLease?.();
      throw error;
    }
  } else if (options.recoveryDecision) {
    host.endProjectTransitionFrontendLease?.();
    throw new Error("Reîncărcarea proiectului nu acceptă o decizie de recuperare de la deschidere.");
  }
  try {
    await host.invalidateExternalReconcileForProjectTransition?.();
  } catch (error) {
    host.endProjectTransitionFrontendLease?.();
    throw error;
  }
  let rustSessionSwapped = false;
  let previewIdentity: ProjectPreviewRequestIdentity | null = null;
  try {
    const project = await openProject(
      root,
      options.operatorDecisionId ?? undefined,
      options.recoveryDecision ?? undefined,
    );
    rustSessionSwapped = true;
    console.info("[Pană Studio] openProject returned", project);
    previewIdentity = await projectPublishedSessionIntoFrontend(host, project, "open");
  } catch (error) {
    if (rustSessionSwapped) {
      host.markWorkspaceProjectionRecoveryRequired?.(
        "Nucleul a publicat proiectul, dar frontend-ul nu a terminat proiecția inițială.",
      );
    } else {
      host.resumeExternalMonitoringAfterFailedTransition?.();
    }
    throw error;
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }
  if (previewIdentity) await startPreviewAfterOpen(host, previewIdentity);
  await host.restoreWorkbenchState?.();
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

  if (policy.decision === "allow") return true;

  if (policy.decision === "confirm") {
    const request = createProjectTransitionDecisionRequest(
      targetRoot,
      currentProjectRoot,
      policy,
      continuation,
    );
    host.projectTransitionDecisionRequest = request;
    host.projectStatus = policy.message;
    host.setGlobalStatus(policy.title, "idle");
    host.notify({
      id: PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
      level: "warning",
      title: policy.title,
      message: `${policy.message} ${policy.recommendedAction}`,
    });
    return false;
  }

  host.projectTransitionDecisionRequest = null;
  const message = `${policy.title}: ${policy.message} ${policy.recommendedAction}`;
  host.projectStatus = message;
  host.setGlobalStatus(message, "error");
  host.notify({
    id: PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID,
    level: "error",
    title: policy.title,
    message,
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
    throw new Error("Decizia operator nu mai corespunde tranziției curente.");
  }
  host.projectStatus = "Se înregistrează decizia operator și se reia tranziția...";
  host.setGlobalStatus("Se înregistrează decizia operatorului pentru tranziția proiectului.", "saving");
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
    const message = `Tranziția proiectului nu poate continua: ${errorMessage(error)}`;
    host.projectStatus = message;
    host.setGlobalStatus(message, "error");
    host.notify({
      id: PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
      level: "error",
      title: "Tranziția proiectului a fost refuzată",
      message,
    });
    throw error;
  }
}

export function cancelProjectOpenRecoveryDecision(
  host: ProjectControllerHost,
  requestId: string,
) {
  if (host.projectOpenRecoveryDecisionRequest?.id !== requestId) return;
  host.projectOpenRecoveryDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.projectStatus = "Deschiderea dosarului a fost anulată; recuperarea veche a fost păstrată.";
  host.setGlobalStatus("Recovery păstrat. Dosarul nu a fost deschis.", "restored");
}

export async function continueProjectOpenWithRecoveryAbandonment(
  host: ProjectControllerHost,
  requestId: string,
) {
  const request = host.projectOpenRecoveryDecisionRequest;
  if (!request || request.id !== requestId) {
    throw new Error("Decizia de recuperare nu mai corespunde deschiderii curente.");
  }
  const decision = projectOpenRecoveryAbandonDecision(request);
  host.projectOpenRecoveryDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.projectStatus = "Se deschide dosarul actual fără drafturile sesiunii vechi...";
  host.setGlobalStatus("Se aplică decizia explicită de abandonare a recuperării.", "saving");
  try {
    await openProjectRoot(host, request.targetRoot, {
      operatorDecisionId: request.operatorDecisionId,
      recoveryDecision: decision,
    });
  } catch (error) {
    const message = `Dosarul nu a putut fi deschis după decizia de recovery: ${errorMessage(error)}`;
    host.projectStatus = message;
    host.setGlobalStatus(message, "error");
    host.notify({
      id: PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
      level: "error",
      title: "Decizia de recuperare nu a putut fi aplicată",
      message,
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
    && host.scannedProject?.isZola === true
    && host.scannedProject.root === identity.expectedProjectRoot
    && host.sessionProjectRoot === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId;
}

export async function startPreviewAfterOpen(
  host: ProjectControllerHost,
  identity: ProjectPreviewRequestIdentity,
  dependencies: ProjectPreviewDependencies = projectPreviewDependencies,
): Promise<ProjectPreviewStartOutcome> {
  const stale = (): ProjectPreviewStartOutcome => ({
    status: "stale",
    projectSessionId: identity.expectedSessionId,
  });
  if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
  let canvasConfirmation: Promise<void> | null = null;
  let startedPreviewUrl: string | null = null;
  try {
    const rawReceipt = await dependencies.start(identity);
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
    if (!rawReceipt) {
      throw new Error("Previzualizarea Zola nu a publicat nicio generație Canvas.");
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
    if (receipt.canvasProjection.phase === "prepared") {
      canvasConfirmation = host.prepareCanvasProjectionNavigation(receipt.canvasProjection);
    } else {
      host.pendingCanvasProjection = null;
      host.previewWorkspaceRevision = null;
      host.activeCanvasIdentity = { ...receipt.canvasProjection.identity };
    }
    const activeFile = currentProject.files.find((file) => file.relativePath === host.activeScannedPath)
      ?? currentProject.files.find((file) => file.role === "page")
      ?? null;
    if (activeFile && activeFile.role !== "template") {
      await host.loadScannedProjectFile(activeFile, { syncWorkbench: false });
    }
    if (canvasConfirmation) {
      const revision = receipt.canvasProjection.identity.previewRevision;
      const mountedUrl = host.previewSrc === "about:blank" ? null : new URL(host.previewSrc);
      if (mountedUrl?.searchParams.get("__pana_preview_revision") !== revision) {
        const fallbackPage = currentProject.files.find((file) => file.role === "page") ?? null;
        if (!fallbackPage) {
          throw new Error("Canvas-ul staged nu are nicio rută Zola care poate fi montată.");
        }
        host.previewSrc = host.previewUrlForScannedFile(fallbackPage);
        host.activePreviewPath = fallbackPage.relativePath;
        host.previewDocumentMarkup = null;
      }
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
    if (activeFile?.role === "template") {
      await host.loadScannedProjectFile(activeFile, {
        strict: true,
        skipDraftFlush: true,
        activateTemplateWorkbench: true,
        syncWorkbench: false,
      });
    }
    host.clearNotification("project.preview.warning");
    host.setGlobalStatus("Previzualizare Zola pornită.", "restored");
    host.scheduleZolaValidation?.("project-open");
    return {
      status: "canonical",
      projectSessionId: identity.expectedSessionId,
    };
  } catch (error) {
    host.resetControlledPreviewState?.();
    if (canvasConfirmation) await canvasConfirmation.catch(() => undefined);
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
    const currentProject = host.scannedProject;
    if (!currentProject) return stale();
    const message = errorMessage(error);
    host.scannedProject = {
      ...currentProject,
      // Rust poate avea în continuare serverul persistent activ chiar dacă
      // primul Canvas nu a confirmat styledReady. Păstrăm originea pentru ca
      // următorul refresh să poată reatașa bridge-ul prin navigare.
      previewBaseUrl: startedPreviewUrl ?? currentProject.previewBaseUrl,
      previewWarning: message,
    };
    host.notify({
      id: "project.preview.warning",
      level: "warning",
      title: "Previzualizare indisponibilă",
      message,
    });
    host.setGlobalStatus(`Previzualizare indisponibilă: ${message}`, "error");
    return {
      status: "degraded",
      projectSessionId: identity.expectedSessionId,
      message,
    };
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
  host.resetProjectLoopDefinitions?.();
  host.sourceGraph = null;
  host.sourceCache = {};
  host.templateWorkbenchPlan = null;
  host.templateWorkbenchPreferredPagePath = null;
  host.templateWorkbenchActive = false;
  host.templateWorkbenchTarget = null;
  host.templateWorkbenchReturnPreviewPath = null;
  host.templateWorkbenchRequestSerial += 1;
  host.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
  host.lastMeaningfulSelectedElement = null;
  host.lastSelectedImageElement = null;
  host.overrideRules = {};
  host.variableOverrides = {};
  host.htmlPending = createEmptyHtmlPending();
  host.resetInspectorPendingSources();
  host.inspectorPending = createEmptyInspectorPending();
  host.saveState = "idle";
  host.saveStatus = "Nicio modificare salvata in aceasta sesiune.";
  host.cachebustAssets = false;
  host.diskState = createDiskState();
  host.historyPanelOpen = false;
  host.settingsPanelOpen = false;
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
    await flushProjectDraftsBeforeTransition(host);
    await projectCurrentProjectRescan(host, preferredRelativePath, options, undefined, false);
  } finally {
    host.endProjectTransitionFrontendLease?.();
  }
}

/**
 * Projects a committed structural mutation while its per-session FIFO lane is
 * still held. This path deliberately does not acquire ProjectTransition and
 * does not flush drafts, because either action would drain/re-enter the same
 * structural lane and deadlock the operation that owns it.
 */
export async function rescanCurrentProjectWithinStructuralLane(
  host: ProjectControllerHost,
  lease: PreviewStructuralSessionLease,
  preferredRelativePath: string | null = host.activeScannedPath,
  options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
) {
  const requireCurrent = () => requireCurrentPreviewStructuralSession(host, lease);
  requireCurrent();
  await projectCurrentProjectRescan(host, preferredRelativePath, options, requireCurrent, true);
  requireCurrent();
}

/**
 * Reprojects a committed Undo/Redo transaction while the project-wide
 * Undo/Redo reservation remains active. This path must not acquire
 * ProjectTransition or flush drafts: the reservation already drained both
 * draft and structural lanes before the Rust transaction touched disk.
 */
export async function rescanCurrentProjectWithinKernelUndoRedoLease(
  host: ProjectControllerHost,
  lease: KernelUndoRedoProjectionLease,
  preferredRelativePath: string | null = host.activeScannedPath,
  options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
) {
  const requireCurrent = () => requireCurrentKernelUndoRedoProjectionLease(
    host,
    lease,
    "Reproiectarea structurală Undo/Redo",
  );
  requireCurrent();
  await projectCurrentProjectRescan(host, preferredRelativePath, options, requireCurrent, true);
  requireCurrent();
}

async function projectCurrentProjectRescan(
  host: ProjectControllerHost,
  preferredRelativePath: string | null,
  options: { strict?: boolean; deferPreviewRefresh?: boolean },
  requireProjectionCurrent: (() => void) | undefined,
  skipDraftFlush: boolean,
) {
  const requireCurrent = () => requireProjectionCurrent?.();
  requireCurrent();
  const currentProject = host.scannedProject;
  if (!currentProject) return;
  const scanned = await scanProject(currentProject.root);
  requireCurrent();
  const project = preservePreviewBaseUrl(scanned, currentProject);
  host.scannedProject = project;
  host.diskState = diskStateFromProjectScan(project, host.diskState);
  host.projectStatus = planOpenedProject(project).projectStatus;
  const nextFile = selectProjectFileAfterScan(project, preferredRelativePath);
  if (nextFile) {
    await host.loadScannedProjectFile(nextFile, {
      strict: options.strict,
      skipDraftFlush,
      deferPreviewRefresh: true,
    });
    requireCurrent();
  }
  if (host.refreshSourceGraph) {
    await host.refreshSourceGraph({ strict: options.strict });
    requireCurrent();
  } else if (options.strict) {
    throw new Error("Rescan-ul strict nu poate confirma proiecția Source Graph.");
  }
  const cssIdentity = createCssRequestIdentity(
    host.sessionProjectRoot,
    host.kernelProjectSessionId,
  );
  const nextScssVariables = options.strict
    ? await getScssVariables(cssIdentity)
    : await getScssVariables(cssIdentity).catch(() => host.scssVariables);
  requireCurrent();
  host.scssVariables = nextScssVariables;
  if (!options.deferPreviewRefresh) {
    host.refreshToken += 1;
    const previewRefreshed = await host.requestPreviewRefresh("project-rescan");
    requireCurrent();
    if (options.strict && !previewRefreshed) {
      throw new Error("Previzualizarea nu a confirmat proiecția rescanării stricte.");
    }
  }
  host.startExternalDiskPolling?.();
  host.setGlobalStatus("Structura proiectului a fost rescannată fără a reconstrui sesiunea Rust.", "restored");
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
    !host.scannedProject?.isZola
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
    && host.scannedProject.isZola
    && host.scannedProject.acceptedDiskGeneration === identity.expectedDiskGeneration
    && host.sessionProjectRoot === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId;
}

export async function openCurrentProjectInBrowser(
  host: ProjectControllerHost,
  dependencies: BrowserPreviewDependencies = browserPreviewDependencies,
  options: BrowserPreviewOpenOptions = {},
) {
  if (!host.scannedProject?.isZola) {
    host.setGlobalStatus("Deschide un proiect Zola valid înainte de browser preview.", "error");
    return;
  }

  const identity = captureBrowserPreviewRequestIdentity(host);
  if (!identity) {
    host.setGlobalStatus(
      "Browser preview cere un ProjectSession activ și coerent cu proiectul curent.",
      "error",
    );
    return;
  }

  host.setGlobalStatus("Se randă generația salvată cu motorul Zola pentru Source Browser...", "saving");
  try {
    const receipt = await dependencies.start(identity);
    if (!isBrowserPreviewRequestIdentityCurrent(host, identity)) return;
    if (!receipt) {
      host.setGlobalStatus("Browser preview indisponibil pentru proiectul curent.", "error");
      return;
    }
    if (
      receipt.projectRoot !== identity.expectedProjectRoot
      || receipt.runtimeSessionId !== identity.expectedSessionId
      || receipt.acceptedDiskGeneration !== identity.expectedDiskGeneration
    ) {
      throw new Error(
        "Rust a returnat un browser preview pentru altă instanță ProjectSession sau altă generație AcceptedDisk.",
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
    host.setGlobalStatus(`Source Browser deschis din generația salvată: ${targetUrl}`, "restored");
  } catch (error) {
    if (!isBrowserPreviewRequestIdentityCurrent(host, identity)) return;
    const message = `Browser preview indisponibil: ${errorMessage(error)}`;
    host.notify({
      id: "project.browser-preview.warning",
      level: "warning",
      title: "Browser preview indisponibil",
      message,
    });
    host.setGlobalStatus(message, "error");
  }
}

export function sourceBrowserUrlForRoute(baseUrl: string, requestedRoute?: string | null) {
  const route = requestedRoute?.trim() || "/";
  if (!route.startsWith("/") || route.startsWith("//")) {
    throw new Error("Ruta Source Browser trebuie să fie un path local absolut.");
  }
  if (route === "/") return baseUrl;

  const base = new URL(baseUrl);
  const target = new URL(route, `${base.origin}/`);
  if (target.origin !== base.origin || target.pathname.startsWith("/__pana_source/")) {
    throw new Error("Ruta Source Browser a încercat să părăsească pagina publică a proiectului.");
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
      await flushProjectDraftsBeforeTransition(host);
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

  host.projectStatus = "Se închide proiectul curent...";
  host.setGlobalStatus("Se închide proiectul curent.", "saving");
  let rustSessionClosed = false;
  try {
    await closeProject(options.operatorDecisionId ?? undefined);
    rustSessionClosed = true;
    resetProjectSessionState(host, true, "");
    resetProjectScopedState(host);
    host.scannedProject = null;
    host.source = "";
    host.activeScannedPath = null;
    host.previewSrc = "about:blank";
    host.activePreviewPath = "about:blank";
    host.previewWorkspaceRevision = null;
    host.previewDocumentMarkup = null;
    host.projectStatus = "Niciun proiect deschis.";
    host.projectOpenRecoveryDecisionRequest = null;
    host.projectTransitionDecisionRequest = null;
    host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
    host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
    host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
    host.clearNotification("project.preview.warning");
    host.clearNotification("project.not-zola");
    host.setGlobalStatus("Proiectul curent a fost închis.", "restored");
    return true;
  } catch (error) {
    if (!rustSessionClosed) host.resumeExternalMonitoringAfterFailedTransition?.();
    const message = `Închiderea proiectului a eșuat: ${errorMessage(error)}`;
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
      message: "Nu există un proiect care poate fi reîncărcat.",
    };
  }
  const projectRoot = host.scannedProject.root;
  await host.beginProjectTransitionFrontendLease?.();
  let transitionAllowed = false;
  try {
    await flushProjectDraftsBeforeTransition(host);
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
      message: "Tranziția proiectului nu a autorizat reîncărcarea.",
    };
  }

  const isDiscard = options.mode === "discard";
  host.projectStatus = isDiscard
    ? "Se aruncă sesiunea nesalvată și se reîncarcă proiectul de pe disc..."
    : "Se șterge sesiunea curentă și se reîncarcă proiectul de pe disc...";
  host.saveState = "saving";
  host.saveStatus = isDiscard
    ? "Se revine la starea fișierelor de pe disc..."
    : "Se reconstruiește sesiunea proiectului de pe disc...";
  let rustSessionSwapped = false;
  let publishedProjectSessionId: string | null = null;
  let previewIdentity: ProjectPreviewRequestIdentity | null = null;
  let attachmentFailure: string | null = null;
  try {
    const openedProject = await openProject(
      projectRoot,
      options.operatorDecisionId ?? undefined,
    );
    rustSessionSwapped = true;
    publishedProjectSessionId = openedProject.kernelSessionId ?? null;
    previewIdentity = await projectPublishedSessionIntoFrontend(
      host,
      openedProject,
      "reload",
      { preferredRelativePath },
    );
    if (isDiscard) {
      host.diskState = markDiskMutation(host.diskState, "discard", preferredRelativePath);
    }
    host.projectTransitionDecisionRequest = null;
    host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
    host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
  } catch (error) {
    attachmentFailure = errorMessage(error);
    if (rustSessionSwapped) {
      host.markWorkspaceProjectionRecoveryRequired?.(
        "Nucleul a publicat sesiunea reîncărcată, dar proiecția surselor nu a ajuns la o stare terminală.",
      );
    } else {
      host.resumeExternalMonitoringAfterFailedTransition?.();
    }
    const message = isDiscard
      ? `Revenirea la disk a eșuat: ${attachmentFailure}`
      : `Purge eșuat: ${attachmentFailure}`;
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
      message: "Sesiunea nu are o suprafață de previzualizare Zola.",
    };
  }

  const previewOutcome = await startPreviewAfterOpen(host, previewIdentity);
  if (previewOutcome.status === "stale") {
    const message = "Proiecția de previzualizare a fost înlocuită de altă sesiune de proiect.";
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
        ? "Sesiunea salvată automat a fost aruncată. Sursele și previzualizarea provin de pe disc."
        : "Curățare completă: sursele și previzualizarea au fost reconstruite de pe disc.",
      "restored",
    );
  }
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
  host.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
  host.lastMeaningfulSelectedElement = null;
  host.lastSelectedImageElement = null;
  host.previewDocumentMarkup = null;
  host.browserPreviewRoute = "/";
  host.refreshToken += 1;
}

export async function createContentPage(host: ProjectControllerHost) {
  if (!host.scannedProject) {
    host.projectStatus = "Crearea de pagini este disponibila doar pentru un proiect Zola real.";
    return;
  }
  const rawTitle = window.prompt("Titlul paginii noi:");
  if (rawTitle === null) return;
  return createContentPageFromInput(host, { title: rawTitle });
}

export async function createContentPageFromInput(
  host: ProjectControllerHost,
  input: { title: string; slug?: string | null; section?: string | null },
): Promise<string | null> {
  if (!host.scannedProject) {
    host.projectStatus = "Crearea de pagini este disponibila doar pentru un proiect Zola real.";
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
      const receipt = await createProjectContentPage({
        section: pagePlan.section,
        slug: pagePlan.slug,
        title: pagePlan.title,
      }, identity);
      requireCurrentPreviewStructuralSession(host, lease);
      const relativePath = receipt.relativePath;
      if (!relativePath) {
        throw new Error("Receipt-ul creării paginii nu conține path-ul rezultat.");
      }
      requireCurrentPreviewStructuralSession(host, lease);
      await rescanCurrentProjectWithinStructuralLane(host, lease, relativePath, { strict: true });
      requireCurrentPreviewStructuralSession(host, lease);
      host.saveState = "unsaved";
      host.saveStatus = `Pagina nouă este în sesiune: ${relativePath}. Ctrl+S persistă pe disc.`;
      return relativePath;
    } catch (error) {
      if (!previewStructuralSessionLeaseMatches(host, lease)) return null;
      host.projectStatus = `Nu am putut crea pagina: ${errorMessage(error)}`;
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
  } = {},
) {
  if (!host.scannedProject) return;
  const expectedRoot = host.scannedProject.root;
  const expectedSessionId = host.kernelProjectSessionId;
  const expectedSessionEpoch = host.projectSessionEpoch;
  if (!options.skipDraftFlush) await host.flushInteractiveEditorDrafts();
  if (!projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)) return;
  const loadPlan = planScannedProjectFileLoad(file);
  host.activeScannedPath = file.relativePath;
  host.source = "Se incarca fisierul...";
  host.centerView = loadPlan.centerView;

  if (loadPlan.isPreviewPage) {
    if (host.templateWorkbenchActive) {
      await host.exitTemplateWorkbench({ deferPreviewRefresh: options.deferPreviewRefresh });
    }
    host.templateWorkbenchPlan = null;
    host.templateWorkbenchPreferredPagePath = null;
    host.previewSrc = host.previewUrlForScannedFile(file);
    host.activePreviewPath = file.relativePath;
    host.browserPreviewRoute = file.previewPath ?? "/";
    host.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
    host.lastMeaningfulSelectedElement = null;
    host.lastSelectedImageElement = null;
    host.previewDocumentMarkup = null;
    host.cancelPreviewSync();
  }

  if (loadPlan.isTemplateFile) {
    if (options.activateTemplateWorkbench !== false) {
      clearTemplateFileSelection(host);
      await host.updateTemplateWorkbenchContext(
        host.scannedProject,
        file,
        host.templateWorkbenchPreferredPagePath,
        {
          deferPreviewRefresh: options.deferPreviewRefresh,
          strict: options.strict,
        },
      );
      clearTemplateFileSelection(host);
    }
  } else if (!loadPlan.isPreviewPage) {
    if (!loadPlan.isTemplateFile && host.templateWorkbenchActive) {
      await host.exitTemplateWorkbench({ deferPreviewRefresh: options.deferPreviewRefresh });
    }
    host.templateWorkbenchPlan = null;
    host.templateWorkbenchPreferredPagePath = null;
  }

  const cachedSource = host.sourceCache[loadPlan.cacheKey];
  if (typeof cachedSource === "string") {
    host.source = cachedSource;
    if (loadPlan.isPreviewPage && !options.deferPreviewRefresh) {
      await host.refreshRenderedPreviewDocument();
    }
    return;
  }

  try {
    const text = await readProjectFile(file.relativePath);
    if (
      host.activeScannedPath !== file.relativePath ||
      !projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)
    ) return;
    host.sourceCache = { ...host.sourceCache, [loadPlan.cacheKey]: text };
    host.source = text;
    if (loadPlan.isPreviewPage && !options.deferPreviewRefresh) {
      await host.refreshRenderedPreviewDocument();
    }
  } catch (error) {
    if (
      host.activeScannedPath !== file.relativePath ||
      !projectLoadLeaseMatches(host, expectedRoot, expectedSessionId, expectedSessionEpoch)
    ) return;
    if (options.strict) throw error;
    host.source = `Nu am putut incarca ${file.relativePath}: ${errorMessage(error)}`;
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

function clearTemplateFileSelection(host: ProjectControllerHost) {
  host.clearPreviewSelection({ clearTemplateGate: true, clearHtmlMarker: true });
  host.selectedPreviewElement = null;
  host.lastMeaningfulSelectedElement = null;
  host.lastSelectedImageElement = null;
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
    throw new Error("Context de template cere un template Zola explicit.");
  }
  if (project.root !== identity.expectedProjectRoot) {
    throw new Error("Context de template a refuzat un ProjectScan din altă sesiune.");
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

export async function updateTemplateWorkbenchContext(
  host: ProjectControllerHost,
  project: ProjectScan,
  templateFile: ProjectFile,
  preferredPagePath: string | null = null,
  options: {
    deferPreviewRefresh?: boolean;
    minimumWorkspaceRevision?: number;
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
        "Contextul de template nu a putut captura revizia sesiunii active a proiectului.",
      );
    }
    const minimumRevision = options.minimumWorkspaceRevision;
    if (
      minimumRevision !== undefined
      && (!Number.isSafeInteger(minimumRevision) || minimumRevision < 0)
    ) {
      throw new Error("Context de template a primit o revizie minimă invalidă.");
    }
    if (minimumRevision !== undefined && workspace.revision < minimumRevision) {
      throw new Error(
        `ProjectWorkspace este la revizia ${workspace.revision}, sub revizia minimă ${minimumRevision} cerută de Context de template.`,
      );
    }

    const request: TemplateWorkbenchPreviewRequest = {
      ...lease.identity,
      expectedWorkspaceRevision: workspace.revision,
      templatePath: lease.templatePath,
      preferredPagePath,
    };
    const receipt = await projectTemplateWorkbenchPreview(request);
    if (!templateWorkbenchUiLeaseMatches(host, lease)) return null;
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
        "Context de template a primit un receipt pentru altă revizie, sesiune sau sursă.",
      );
    }

    if (!host.templateWorkbenchActive) {
      host.templateWorkbenchReturnPreviewPath = host.activePreviewPath;
    }
    host.templateWorkbenchActive = true;
    host.templateWorkbenchTarget = lease.templatePath;
    host.templateWorkbenchPlan = receipt.plan;
    host.templateWorkbenchPreferredPagePath = receipt.plan.selectedContext?.pageFile ?? null;
    host.activePreviewPath = `Context de template: ${lease.templatePath}`;
    host.previewDocumentMarkup = null;
    if (receipt.canvasProjection.phase === "prepared") {
      const reconciled = await host.reconcileTemplateWorkbenchPreviewDocument(
        receipt.previewUrl,
        receipt.canvasProjection,
      );
      if (!reconciled) {
        throw new Error(
          "Context de template nu a confirmat candidatul Canvas al aceleiași revizii.",
        );
      }
    } else {
      host.previewSrc = receipt.previewUrl;
      if (!options.deferPreviewRefresh) await host.refreshRenderedPreviewDocument();
    }
    if (!templateWorkbenchUiLeaseMatches(host, lease)) return null;
    host.setGlobalStatus(
      `Context de template activ: ${receipt.plan.activeTemplate.name}.`,
      "restored",
    );
    const selectedPageFile = receipt.plan.selectedContext?.pageFile ?? null;
    return selectedPageFile
      ? project.files.find(
        (file) => file.role === "page" && file.relativePath === selectedPageFile,
      ) ?? null
      : null;
  } catch (error) {
    if (!templateWorkbenchUiLeaseMatches(host, lease)) return null;
    if (options.strict) throw error;
    host.setGlobalStatus(`Context de template indisponibil: ${errorMessage(error)}`, "error");
    return null;
  }
}

export async function exitTemplateWorkbench(
  host: ProjectControllerHost,
  options: { deferPreviewRefresh?: boolean } = {},
) {
  if (!host.templateWorkbenchActive) return;
  host.templateWorkbenchRequestSerial += 1;
  const returnPath = host.templateWorkbenchReturnPreviewPath;
  host.templateWorkbenchActive = false;
  host.templateWorkbenchTarget = null;
  host.templateWorkbenchReturnPreviewPath = null;
  host.templateWorkbenchPlan = null;
  host.templateWorkbenchPreferredPagePath = null;
  host.previewDocumentMarkup = null;
  const returnPage = returnPath
    ? host.scannedProject?.files.find((file) => file.relativePath === returnPath && file.role === "page")
    : null;
  const fallbackPage = returnPage
    ?? host.scannedProject?.files.find((file) => file.role === "page")
    ?? null;
  if (fallbackPage) {
    host.previewSrc = host.previewUrlForScannedFile(fallbackPage);
    host.activePreviewPath = fallbackPage.relativePath;
    if (!options.deferPreviewRefresh) await host.refreshRenderedPreviewDocument();
  } else {
    const previewBaseUrl = host.scannedProject?.previewBaseUrl ?? null;
    host.previewSrc = previewBaseUrl ?? "about:blank";
    host.activePreviewPath = previewBaseUrl ?? "about:blank";
    if (previewBaseUrl && !options.deferPreviewRefresh) {
      await host.refreshRenderedPreviewDocument();
    }
  }
  host.setGlobalStatus("Context de template închis. Previzualizarea site-ului este activă.", "idle");
}
