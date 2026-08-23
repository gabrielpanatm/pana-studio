import { sameCanvasProjectionIdentity as canvasProjectionIdentityMatches } from "$lib/contracts/canvas-identity";
import { t } from "$lib/i18n/runtime.svelte";
import type {
  FrontendProjectAttachment,
  ProjectPreviewStartOutcome,
} from "$lib/project/controller-contracts";
import {
  readProjectLifecycle,
  reportProjectCapabilityDegraded,
} from "$lib/project/io/lifecycle";
import {
  requireProjectPreviewStartReceipt,
  startProjectPreview,
  type ProjectPreviewRequestIdentity,
  type ProjectPreviewStartReceipt,
} from "$lib/preview/io";
import type {
  CanvasProjectionPlan,
} from "$lib/contracts/canvas-projection";
import { bindCanvasCandidateIdentityToPreviewUrl } from "$lib/project/preview-url";
import { markProjectWorkspacePreviewPublished } from "$lib/kernel/project-workspace-preview-coordinator";
import { isCanvasProjectionSurfaceUnavailableError } from "$lib/state/preview-controller";
import {
  type ProjectTemplateWorkbenchHost,
} from "$lib/state/project-template-workbench-controller";
import type { GlobalStatusEscalationRequest, GlobalStatusKind } from "$lib/status/global-status";
import type {
  ProjectFile,
  ProjectLifecycleSnapshot,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { ProjectBootstrapInitialSurface } from "$lib/project/lifecycle-contract";
import { errorMessage } from "$lib/util";

export type ProjectPreviewBootstrapHost = {
  scannedProject: ProjectScan | null;
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  activeScannedPath: string | null;
  projectLifecycle: ProjectLifecycleSnapshot;
  previewSrc: string;
  activePreviewPath: string;
  browserPreviewRoute: string;
  previewDocumentMarkup: string | null;
  activeCanvasIdentity: CanvasProjectionPlan["identity"] | null;
  activeCanvasUrl: string;
  editorSelection: ProjectTemplateWorkbenchHost["editorSelection"];
  templateWorkbenchPreferredPagePath: string | null;
  templateWorkbenchPreferredRoute: string | null;
  projectTransitionFrontendLeaseActive: boolean;
  projectTransitionFrontendLeaseGeneration: number;
  pendingCanvasProjection: CanvasProjectionPlan | null;
  previewWorkspaceRevision: string | null;
  scheduleZolaValidation?: (reason: "project-open") => void;
  prepareCanvasProjectionNavigation: (plan: CanvasProjectionPlan) => Promise<void>;
  hasMountedCanvasProjectionSurface?: () => boolean;
  deferWorkspacePreviewProjection?: () => void;
  markCanvasProjectionSurfaceCurrent?: () => void;
  resetControlledPreviewState?: () => void;
  refreshSourceGraph: (options?: { strict?: boolean }) => Promise<void>;
  mountBootstrapInitialTemplateSurface: (
    activeFile: ProjectFile | null,
    surface: ProjectBootstrapInitialSurface,
    receipt: ProjectPreviewStartReceipt,
  ) => boolean;
  synchronizeProjectCanvasSurfaceRoute: (
    previewSrc: string,
    expectedIdentity?: CanvasProjectionPlan["identity"],
  ) => Promise<void>;
  updateTemplateWorkbenchContext: (
    project: ProjectScan,
    templateFile: ProjectFile,
    preferredPagePath?: string | null,
    options?: {
      preferredRoute?: string | null;
      strict?: boolean;
    },
  ) => Promise<ProjectFile | null>;
  clearNotification: (id: string) => void;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

export type ProjectPreviewDependencies = {
  start: (identity: ProjectPreviewRequestIdentity) => Promise<ProjectPreviewStartReceipt | null>;
  readLifecycle?: () => Promise<ProjectLifecycleSnapshot>;
};

const projectPreviewDependencies: ProjectPreviewDependencies = {
  start: startProjectPreview,
  readLifecycle: readProjectLifecycle,
};

export function isProjectPreviewRequestIdentityCurrent(
  host: Pick<
    ProjectPreviewBootstrapHost,
    | "scannedProject"
    | "sessionProjectRoot"
    | "kernelProjectSessionId"
    | "projectTransitionFrontendLeaseActive"
    | "projectTransitionFrontendLeaseGeneration"
  >,
  identity: ProjectPreviewRequestIdentity & {
    expectedProjectTransitionGeneration?: number;
  },
) {
  return host.projectTransitionFrontendLeaseActive !== true
    && (
      identity.expectedProjectTransitionGeneration === undefined
      || host.projectTransitionFrontendLeaseGeneration
        === identity.expectedProjectTransitionGeneration
    )
    && host.scannedProject !== null
    && host.scannedProject.root === identity.expectedProjectRoot
    && host.sessionProjectRoot === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId;
}

export function mountBootstrapInitialSurface(
  host: ProjectTemplateWorkbenchHost,
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
    || !surface.reuseToken?.trim()
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
  host.templateWorkbenchCanvas.setReuseToken(surface.reuseToken);
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
  host: ProjectPreviewBootstrapHost,
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
    const lifecycle = await reportProjectCapabilityDegraded(
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      "preview",
      identity.previewWarning,
    ).catch(() => host.projectLifecycle);
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
    host.projectLifecycle = lifecycle;
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
      // Prepared is not current yet, even when an iframe happened to mount
      // before Preview bootstrap. Keep one recovery barrier armed until the
      // exact Canvas confirmation makes the surface canonical.
      host.deferWorkspacePreviewProjection?.();
      if (canvasSurfaceMounted) {
        canvasConfirmation = host.prepareCanvasProjectionNavigation(receipt.canvasProjection);
      }
    } else {
      host.pendingCanvasProjection = null;
      host.previewWorkspaceRevision = null;
      host.activeCanvasIdentity = { ...receipt.canvasProjection.identity };
      void host.editorSelection.refreshNavigationSnapshot(
        receipt.canvasProjection.identity,
        receipt.url,
      );
      if (!canvasSurfaceMounted) host.deferWorkspacePreviewProjection?.();
    }
    const activeFile = currentProject.files.find(
      (file) => file.relativePath === host.activeScannedPath,
    ) ?? currentProject.files.find((file) => file.role === "page") ?? null;
    const bootstrapSurfaceMounted = identity.initialSurface
      ? host.mountBootstrapInitialTemplateSurface(
          activeFile,
          identity.initialSurface,
          receipt,
        )
      : false;
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
      host.setGlobalStatus(t("project-controller-preview-running-canvas-paused"), "restored");
      return {
        status: "deferred",
        projectSessionId: identity.expectedSessionId,
      };
    }
    if (canvasConfirmation) {
      await canvasConfirmation;
      canvasConfirmation = null;
      if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
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
      host.setGlobalStatus(t("project-controller-preview-running-canvas-paused"), "restored");
      return {
        status: "deferred",
        projectSessionId: identity.expectedSessionId,
      };
    }
    host.markCanvasProjectionSurfaceCurrent?.();
    if (activeFile?.role === "template" && bootstrapSurfaceMounted) {
      await host.synchronizeProjectCanvasSurfaceRoute(
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
      await host.synchronizeProjectCanvasSurfaceRoute(
        host.previewSrc,
        host.activeCanvasIdentity,
      );
    }
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
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
      host.setGlobalStatus(t("project-controller-preview-running-canvas-paused"), "restored");
      return {
        status: "deferred",
        projectSessionId: identity.expectedSessionId,
      };
    }
    host.resetControlledPreviewState?.();
    const message = errorMessage(error);
    const lifecycle = await reportProjectCapabilityDegraded(
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      "preview",
      message,
    ).catch(() => host.projectLifecycle);
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return stale();
    host.projectLifecycle = lifecycle;
    host.scannedProject = {
      ...currentProject,
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

export async function refreshSourceGraphAfterCommit(
  host: ProjectPreviewBootstrapHost,
  identity: FrontendProjectAttachment,
) {
  try {
    await host.refreshSourceGraph({ strict: true });
  } catch (error) {
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return;
    const lifecycle = await reportProjectCapabilityDegraded(
      identity.expectedProjectRoot,
      identity.expectedSessionId,
      "source_graph",
      errorMessage(error),
    ).catch(() => host.projectLifecycle);
    if (!isProjectPreviewRequestIdentityCurrent(host, identity)) return;
    host.projectLifecycle = lifecycle;
  }
}
