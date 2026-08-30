import { sameCanvasProjectionIdentity as canvasProjectionIdentityMatches } from "$lib/contracts/canvas-identity";
import { t } from "$lib/i18n/runtime.svelte";
import {
  confirmTemplateWorkbenchReuse,
  createProjectPreviewRequestIdentity,
  projectPreviewRequestIdentityMatches,
  projectTemplateWorkbenchPreview,
  type ProjectPreviewRequestIdentity,
  type TemplateWorkbenchPreviewRequest,
  type TemplateWorkbenchPublicationStatus,
  type TemplateWorkbenchReuseRequest,
} from "$lib/preview/io";
import {
  readProjectLifecycle,
} from "$lib/project/io/lifecycle";
import {
  readProjectWorkspaceState,
} from "$lib/project/io/workspace";
import type {
  CanvasProjectionIdentity,
  CanvasProjectionPlan,
} from "$lib/contracts/canvas-projection";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type {
  ProjectFile,
  ProjectLifecycleSnapshot,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { TemplateWorkbenchPlan } from "$lib/project/template-workbench-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import { errorMessage } from "$lib/util";

export type ProjectTemplateWorkbenchHost = {
  activeCanvasIdentity: CanvasProjectionIdentity | null;
  activeCanvasUrl: string;
  activePreviewPath: string;
  activeScannedPath: string | null;
  browserPreviewRoute: string;
  kernelProjectSessionId: string;
  previewDocumentMarkup: string | null;
  previewSrc: string;
  projectLifecycle?: ProjectLifecycleSnapshot;
  projectSessionEpoch: number;
  projectWorkspaceMutationEpoch: number;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  scannedProject: ProjectScan | null;
  sessionProjectRoot: string;
  templateWorkbenchActive: boolean;
  templateWorkbenchPlan: TemplateWorkbenchPlan | null;
  templateWorkbenchPreferredPagePath: string | null;
  templateWorkbenchPreferredRoute: string | null;
  templateWorkbenchRequestSerial: number;
  templateWorkbenchReturnPreviewPath: string | null;
  templateWorkbenchTarget: string | null;
  editorSelection: {
    refreshNavigationSnapshot: (
      identity: CanvasProjectionIdentity,
      previewUrl: string,
      options?: { strict?: boolean },
    ) => Promise<unknown>;
  };
  templateWorkbenchCanvas: {
    reconcile: (previewUrl: string, plan: CanvasProjectionPlan) => Promise<boolean>;
    canReuse: (identity: CanvasProjectionIdentity, previewUrl: string) => boolean;
    getReuseToken: () => string | null;
    setReuseToken: (token: string | null) => void;
    setPublicationStatus: (status: TemplateWorkbenchPublicationStatus | null) => void;
  };
  refreshRenderedPreviewDocument: () => Promise<boolean>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

type TemplateWorkbenchUiLease = {
  identity: ProjectPreviewRequestIdentity;
  templatePath: string;
  projectSessionEpoch: number;
  projectWorkspaceMutationEpoch: number;
  activeScannedPath: string | null;
  bindToActiveDocument: boolean;
  requestSerial: number;
};

function captureTemplateWorkbenchUiLease(
  host: ProjectTemplateWorkbenchHost,
  project: ProjectScan,
  templateFile: ProjectFile,
  bindToActiveDocument: boolean,
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
    bindToActiveDocument,
    requestSerial: host.templateWorkbenchRequestSerial,
  };
}

function templateWorkbenchUiLeaseMatches(
  host: ProjectTemplateWorkbenchHost,
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
    && (
      lease.bindToActiveDocument
        ? host.activeScannedPath === lease.activeScannedPath
          && host.activeScannedPath === lease.templatePath
        : host.templateWorkbenchActive
          && host.templateWorkbenchTarget === lease.templatePath
    );
}

function normalizedTemplateContextPath(path: string | null | undefined) {
  return path?.trim().replaceAll("\\", "/").replace(/^\.\/+/, "") ?? "";
}

function sameTemplateWorkbenchContext(
  current: TemplateWorkbenchPlan | null,
  next: TemplateWorkbenchPlan,
) {
  return Boolean(
    current
    && current.activeTemplate.file === next.activeTemplate.file
    && current.activeTemplate.sourceId === next.activeTemplate.sourceId
    && current.renderMode === next.renderMode
    && normalizedTemplateContextPath(current.activeComponentName)
      === normalizedTemplateContextPath(next.activeComponentName)
    && normalizedTemplateContextPath(current.selectedContext?.pageFile)
      === normalizedTemplateContextPath(next.selectedContext?.pageFile)
    && normalizedTemplateContextPath(current.selectedRoute?.url)
      === normalizedTemplateContextPath(next.selectedRoute?.url)
  );
}

function templateWorkbenchPerformanceIsValid(
  performance: Awaited<ReturnType<typeof projectTemplateWorkbenchPreview>>["performance"],
) {
  return Boolean(
    performance
    && typeof performance.modelCacheHit === "boolean"
    && [
      performance.totalUs,
      performance.operationLockWaitUs,
      performance.projectModelUs,
      performance.planUs,
      performance.engineLockWaitUs,
      performance.publishUs,
      performance.renderUs,
      performance.graphUs,
      performance.prepareUs,
    ].every((value) => Number.isSafeInteger(value) && value >= 0)
  );
}

function templateWorkbenchReusePerformanceIsValid(
  performance: Awaited<ReturnType<typeof confirmTemplateWorkbenchReuse>>["performance"],
) {
  return Boolean(
    performance
    && [
      performance.totalUs,
      performance.operationLockWaitUs,
      performance.engineLockWaitUs,
    ].every((value) => Number.isSafeInteger(value) && value >= 0)
  );
}

type CompactTemplateWorkbenchReuseCandidate = {
  identity: CanvasProjectionIdentity;
  plan: TemplateWorkbenchPlan;
  reuseToken: string;
};

function compactTemplateWorkbenchReuseCandidate(
  host: ProjectTemplateWorkbenchHost,
  request: TemplateWorkbenchPreviewRequest,
): CompactTemplateWorkbenchReuseCandidate | null {
  const plan = host.templateWorkbenchPlan;
  const identity = host.activeCanvasIdentity;
  const reuseToken = host.templateWorkbenchCanvas.getReuseToken()?.trim() ?? "";
  const reusable = Boolean(
    host.templateWorkbenchActive
    && host.templateWorkbenchTarget === request.templatePath
    && host.activePreviewPath === request.templatePath
    && reuseToken
    && plan
    && plan.activeTemplate.file === request.templatePath
    && normalizedTemplateContextPath(plan.selectedContext?.pageFile)
      === normalizedTemplateContextPath(request.preferredPagePath)
    && normalizedTemplateContextPath(plan.selectedRoute?.url)
      === normalizedTemplateContextPath(request.preferredRoute)
    && normalizedTemplateContextPath(plan.activeComponentName)
      === normalizedTemplateContextPath(request.preferredComponentName)
    && normalizedTemplateContextPath(host.templateWorkbenchPreferredPagePath)
      === normalizedTemplateContextPath(request.preferredPagePath)
    && normalizedTemplateContextPath(host.templateWorkbenchPreferredRoute)
      === normalizedTemplateContextPath(request.preferredRoute)
    && identity
    && identity.projectRoot === request.expectedProjectRoot
    && identity.runtimeSessionId === request.expectedSessionId
    && identity.workspaceRevision === request.expectedWorkspaceRevision
    && identity.previewRevision.trim()
    && identity.transactionId.trim()
  );
  return reusable && plan && identity ? { identity, plan, reuseToken } : null;
}

function selectedTemplateWorkbenchPage(
  project: ProjectScan,
  plan: TemplateWorkbenchPlan,
) {
  const selectedPageFile = plan.selectedContext?.pageFile ?? null;
  return selectedPageFile
    ? project.files.find(
        (file) => file.role === "page" && file.relativePath === selectedPageFile,
      ) ?? null
    : null;
}

function publishTemplateWorkbenchContextStatus(
  host: ProjectTemplateWorkbenchHost,
  plan: TemplateWorkbenchPlan,
) {
  const contextStatus = plan.selectedContext
    ? t("project-controller-template-context-active-page", {
        name: plan.activeTemplate.name,
        title: plan.selectedContext.pageTitle,
        url: plan.selectedContext.pageUrl,
      })
    : plan.selectedRoute
      ? t("project-controller-template-context-active-route", {
          name: plan.activeTemplate.name,
          label: plan.selectedRoute.label,
          url: plan.selectedRoute.url,
        })
      : t("project-controller-template-context-active", {
          name: plan.activeTemplate.name,
        });
  host.setGlobalStatus(contextStatus, "restored");
}

export async function synchronizeActiveCanvasSurfaceRoute(
  host: ProjectTemplateWorkbenchHost,
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
  await host.editorSelection.refreshNavigationSnapshot(identity, previewUrl, { strict: true });
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
  host: ProjectTemplateWorkbenchHost,
  project: ProjectScan,
  templateFile: ProjectFile,
  preferredPagePath: string | null = null,
  options: {
    deferPreviewRefresh?: boolean;
    expectedWorkspaceRevision?: number;
    minimumWorkspaceRevision?: number;
    preferredRoute?: string | null;
    preferredComponentName?: string | null;
    strict?: boolean;
    bindToActiveDocument?: boolean;
  } = {},
) {
  const lease = captureTemplateWorkbenchUiLease(
    host,
    project,
    templateFile,
    options.bindToActiveDocument !== false,
  );
  try {
    const workspace = host.projectWorkspaceSnapshot ?? await readProjectWorkspaceState();
    if (!templateWorkbenchUiLeaseMatches(host, lease)) return null;
    if (
      !workspace
      || workspace.projectRoot !== lease.identity.expectedProjectRoot
      || workspace.runtimeSessionId !== lease.identity.expectedSessionId
    ) {
      throw new Error(t("project-controller-template-session-revision-missing"));
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
    if (expectedRevision !== undefined && workspace.revision !== expectedRevision) {
      return null;
    }

    const request: TemplateWorkbenchPreviewRequest = {
      ...lease.identity,
      expectedWorkspaceRevision: expectedRevision ?? workspace.revision,
      templatePath: lease.templatePath,
      preferredPagePath,
      preferredRoute: options.preferredRoute ?? null,
      preferredComponentName: options.preferredComponentName ?? null,
    };

    const reuseCandidate = compactTemplateWorkbenchReuseCandidate(host, request);
    if (reuseCandidate) {
      const canvasIdentity = reuseCandidate.identity;
      const reuseRequest: TemplateWorkbenchReuseRequest = {
        ...request,
        reuseToken: reuseCandidate.reuseToken,
        expectedPreviewRevision: canvasIdentity.previewRevision,
        expectedCanvasTransactionId: canvasIdentity.transactionId,
      };
      const confirmation = await confirmTemplateWorkbenchReuse(reuseRequest);
      if (
        !templateWorkbenchUiLeaseMatches(host, lease)
        || (
          expectedRevision !== undefined
          && host.projectWorkspaceSnapshot?.revision !== expectedRevision
        )
      ) return null;
      if (confirmation.status === "confirmed") {
        if (
          confirmation.workspaceRevision !== request.expectedWorkspaceRevision
          || confirmation.previewRevision !== canvasIdentity.previewRevision
          || confirmation.canvasTransactionId !== canvasIdentity.transactionId
          || confirmation.reuseToken !== reuseRequest.reuseToken
          || !confirmation.previewUrl?.trim()
          || !confirmation.route?.startsWith("/__pana_workbench/")
          || !templateWorkbenchReusePerformanceIsValid(confirmation.performance)
        ) {
          throw new Error(t("project-controller-template-receipt-mismatch"));
        }
        if (host.templateWorkbenchCanvas.canReuse(canvasIdentity, confirmation.previewUrl)) {
          host.templateWorkbenchCanvas.setPublicationStatus("reused");
          if (host.activeCanvasUrl !== confirmation.previewUrl) {
            host.activeCanvasUrl = confirmation.previewUrl;
          }
          publishTemplateWorkbenchContextStatus(host, reuseCandidate.plan);
          return selectedTemplateWorkbenchPage(project, reuseCandidate.plan);
        }
      } else if (
        confirmation.status !== "miss"
        || confirmation.workspaceRevision !== request.expectedWorkspaceRevision
        || confirmation.route !== null
        || confirmation.previewUrl !== null
        || confirmation.reuseToken !== null
        || confirmation.previewRevision !== null
        || confirmation.canvasTransactionId !== null
        || !templateWorkbenchReusePerformanceIsValid(confirmation.performance)
      ) {
        throw new Error(t("project-controller-template-receipt-mismatch"));
      }
    }

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
      || !receipt.reuseToken?.trim()
      || receipt.plan.activeTemplate.file !== lease.templatePath
      || normalizedTemplateContextPath(receipt.plan.activeComponentName)
        !== normalizedTemplateContextPath(request.preferredComponentName)
      || !["reused", "materialized"].includes(receipt.publicationStatus)
      || !templateWorkbenchPerformanceIsValid(receipt.performance)
    ) {
      throw new Error(t("project-controller-template-receipt-mismatch"));
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
      && normalizedTemplateContextPath(receipt.plan.selectedRoute?.url)
        !== normalizedTemplateContextPath(requestedRoute)
    ) {
      throw new Error(
        t("project-controller-template-route-unconfirmed", { route: requestedRoute }),
      );
    }

    const reusesExactCanonicalSurface = receipt.publicationStatus === "reused"
      && receipt.canvasProjection.phase === "canonicalVerified"
      && host.templateWorkbenchActive
      && host.templateWorkbenchTarget === lease.templatePath
      && host.activePreviewPath === lease.templatePath
      && host.projectWorkspaceSnapshot?.revision === receipt.workspaceRevision
      && sameTemplateWorkbenchContext(host.templateWorkbenchPlan, receipt.plan)
      && normalizedTemplateContextPath(host.templateWorkbenchPreferredPagePath)
        === normalizedTemplateContextPath(receipt.plan.selectedContext?.pageFile)
      && normalizedTemplateContextPath(host.templateWorkbenchPreferredRoute)
        === normalizedTemplateContextPath(receipt.plan.selectedRoute?.url)
      && host.templateWorkbenchCanvas.canReuse(
        receipt.canvasProjection.identity,
        receipt.previewUrl,
      );

    if (!host.templateWorkbenchActive) {
      host.templateWorkbenchReturnPreviewPath = host.activePreviewPath;
    }
    host.templateWorkbenchActive = true;
    host.templateWorkbenchTarget = lease.templatePath;
    host.templateWorkbenchPlan = receipt.plan;
    host.templateWorkbenchPreferredPagePath = receipt.plan.selectedContext?.pageFile ?? null;
    host.templateWorkbenchPreferredRoute = receipt.plan.selectedRoute?.url ?? null;
    host.templateWorkbenchCanvas.setReuseToken(receipt.reuseToken);
    host.templateWorkbenchCanvas.setPublicationStatus(receipt.publicationStatus);
    host.activePreviewPath = lease.templatePath;
    host.previewDocumentMarkup = null;
    if (receipt.canvasProjection.phase === "prepared") {
      const reconciled = await host.templateWorkbenchCanvas.reconcile(
        receipt.previewUrl,
        receipt.canvasProjection,
      );
      if (!reconciled) {
        throw new Error(t("project-controller-template-canvas-unconfirmed"));
      }
    } else if (!reusesExactCanonicalSurface) {
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
    publishTemplateWorkbenchContextStatus(host, receipt.plan);
    return selectedTemplateWorkbenchPage(project, receipt.plan);
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
  host: ProjectTemplateWorkbenchHost,
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
    ? host.scannedProject?.files.find(
        (file) => file.relativePath === returnPath && file.role === "page",
      )
    : null;
  const fallbackPage = returnPage
    ?? host.scannedProject?.files.find(
      (file) => file.role === "page" && file.previewPath === "/",
    )
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
  host.templateWorkbenchCanvas.setReuseToken(null);
  host.templateWorkbenchCanvas.setPublicationStatus(null);
  host.setGlobalStatus(t("project-controller-template-closed"), "idle");
}
