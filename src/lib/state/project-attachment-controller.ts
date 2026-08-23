import { createCssRequestIdentity, getScssVariables } from "$lib/css/io";
import type { ScssVariable } from "$lib/css/contracts";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  FrontendProjectAttachment,
  FrontendProjectAttachmentMode,
} from "$lib/project/controller-contracts";
import {
  acknowledgeProjectFrontendHydrated,
} from "$lib/project/io/lifecycle";
import { planOpenedProject, planScannedProjectFileLoad } from "$lib/project/session";
import {
  PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
  type ProjectOpenRecoveryDecisionRequest,
} from "$lib/project/open-recovery";
import {
  PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID,
  PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
  type ProjectTransitionDecisionRequest,
} from "$lib/project/transition-decision";
import {
  setFileBufferDraftSyncSession,
} from "$lib/session/file-buffer-draft-sync";
import { setPageJsDraftSyncSession } from "$lib/session/page-js-draft-sync";
import { diskStateFromProjectScan, type DiskState } from "$lib/session/disk-state";
import {
  type ProjectSessionResetOptions,
} from "$lib/state/project-session-reset";
import type { ProjectTransitionFrontendLease } from "$lib/state/project-transition-frontend-lease";
import type { GlobalStatusEscalationRequest, GlobalStatusKind } from "$lib/status/global-status";
import type { ApplicationSurface } from "$lib/application/contracts";
import type {
  ProjectFile,
  ProjectLifecycleSnapshot,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type {
  ProjectBootstrapSourceLocation,
  ProjectOpenBootstrapReceipt,
} from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import type { WorkbenchSnapshot } from "$lib/workbench/contracts";
import type { PublishWorkspaceState } from "$lib/deploy/publish-state.svelte";

export type ProjectAttachmentHost = {
  applicationSurface: ApplicationSurface;
  scannedProject: ProjectScan | null;
  projectLifecycle: ProjectLifecycleSnapshot;
  projectOpenRecoveryDecisionRequest: ProjectOpenRecoveryDecisionRequest | null;
  projectTransitionDecisionRequest: ProjectTransitionDecisionRequest | null;
  projectStatus: string;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  workbenchSnapshot: WorkbenchSnapshot | null;
  sourceCache: Record<string, string>;
  scssVariables: ScssVariable[];
  targetCssFile: string;
  publishWorkspace: Pick<PublishWorkspaceState, "cachebustAssets">;
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  diskState: DiskState;
  projectTransitionFrontendLeaseGeneration: number;
  setSessionProjectRoot: (projectRoot?: string) => void;
  resetProjectSessionProjection: (options?: ProjectSessionResetOptions) => void;
  requireProjectTransitionFrontendLease: (lease: ProjectTransitionFrontendLease) => void;
  loadScannedProjectFile: (
    file: ProjectFile,
    options?: {
      strict?: boolean;
      skipDraftFlush?: boolean;
      activateTemplateWorkbench?: boolean;
      syncWorkbench?: boolean;
    },
  ) => Promise<void>;
  hydrateWorkbenchBootstrap: (snapshot: WorkbenchSnapshot) => void;
  revealBootstrapDiagnosticLocation?: (
    relativePath: string,
    location: ProjectBootstrapSourceLocation,
  ) => void;
  resetExternalDiskState: () => void;
  establishExternalDiskBaseline: () => Promise<void>;
  startExternalDiskMonitoring: () => void;
  clearNotification: (id: string) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
};

function requireProjectAttachmentAuthority(project: ProjectScan) {
  if (!project.kernelSessionId?.trim()) {
    throw new Error(t("project-controller-scan-session-missing"));
  }
  if (!project.acceptedDiskManifest || !project.acceptedDiskGeneration) {
    throw new Error(t("project-controller-scan-manifest-missing"));
  }
}

export async function publishProjectSessionIntoFrontend(
  host: ProjectAttachmentHost,
  project: ProjectScan,
  mode: FrontendProjectAttachmentMode,
  bootstrap: ProjectOpenBootstrapReceipt,
  lease: ProjectTransitionFrontendLease,
): Promise<FrontendProjectAttachment | null> {
  host.requireProjectTransitionFrontendLease(lease);
  requireProjectAttachmentAuthority(project);
  host.projectOpenRecoveryDecisionRequest = null;
  host.projectTransitionDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
  host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
  host.resetProjectSessionProjection({ preserveExternalReconcileBarrier: true });
  if (mode !== "reattach") host.applicationSurface = "workbench";
  host.scannedProject = project;
  host.kernelProjectSessionId = project.kernelSessionId ?? "";
  host.diskState = diskStateFromProjectScan(project, host.diskState);
  host.setSessionProjectRoot(project.root);
  setFileBufferDraftSyncSession(project.root, host.kernelProjectSessionId);
  setPageJsDraftSyncSession(project.root, host.kernelProjectSessionId);

  const openPlan = planOpenedProject(project);
  const fileBuffers = bootstrap.workspace.documents;
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
  host.projectWorkspaceSnapshot = bootstrap.workspace;

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
  const targetCssFile = bootstrap.targetCssFile ?? openPlan.targetCssFile;
  if (targetCssFile) host.targetCssFile = targetCssFile;
  host.publishWorkspace.cachebustAssets = bootstrap.projectSettings.cachebustAssets;
  host.workbenchSnapshot = bootstrap.workbench;
  host.hydrateWorkbenchBootstrap(bootstrap.workbench);
  const bootstrapFile = bootstrap.activeDocument
    ? project.files.find(
        (file) => file.relativePath === bootstrap.activeDocument?.relativePath,
      ) ?? null
    : null;
  if (bootstrapFile && bootstrap.activeDocument) {
    const loadPlan = planScannedProjectFileLoad(bootstrapFile);
    host.sourceCache = {
      ...host.sourceCache,
      [loadPlan.cacheKey]: bootstrap.activeDocument.source,
    };
  }

  if (bootstrapFile) {
    await host.loadScannedProjectFile(bootstrapFile, {
      strict: true,
      skipDraftFlush: true,
      activateTemplateWorkbench: false,
      syncWorkbench: false,
    });
    host.requireProjectTransitionFrontendLease(lease);
    if (bootstrap.activeDocument?.diagnosticLocation) {
      host.revealBootstrapDiagnosticLocation?.(
        bootstrapFile.relativePath,
        bootstrap.activeDocument.diagnosticLocation,
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
  const expectedProjectTransitionGeneration = lease.generation;
  void getScssVariables(scssIdentity, host.projectWorkspaceSnapshot?.revision)
    .then((variables) => {
      if (
        host.sessionProjectRoot === project.root
        && host.kernelProjectSessionId === scssIdentity.expectedSessionId
        && host.projectTransitionFrontendLeaseGeneration
          === expectedProjectTransitionGeneration
      ) host.scssVariables = variables;
    })
    .catch(() => undefined);
  host.resetExternalDiskState();
  await host.establishExternalDiskBaseline();
  host.requireProjectTransitionFrontendLease(lease);
  host.startExternalDiskMonitoring();
  const hydratedLifecycle = await acknowledgeProjectFrontendHydrated(
    project.root,
    host.kernelProjectSessionId,
  );
  host.requireProjectTransitionFrontendLease(lease);
  host.projectLifecycle = hydratedLifecycle;
  return {
    expectedProjectRoot: project.root,
    expectedSessionId: host.kernelProjectSessionId,
    expectedProjectTransitionGeneration,
    initialSurface: bootstrap.initialSurface,
    previewWarning: project.previewWarning,
  };
}
