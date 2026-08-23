import { openUrl as openExternalUrl } from "@tauri-apps/plugin-opener";
import { t } from "$lib/i18n/runtime.svelte";
import {
  startProjectBrowserPreview,
  type BrowserPreviewRequestIdentity,
  type BrowserPreviewStartReceipt,
} from "$lib/preview/io";
import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type {
  GlobalStatusEscalationRequest,
  GlobalStatusKind,
} from "$lib/status/global-status";
import { errorMessage } from "$lib/util";

export type ProjectBrowserPreviewHost = {
  scannedProject: ProjectScan | null;
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectTransitionFrontendLeaseActive: boolean;
  projectTransitionFrontendLeaseGeneration: number;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
  clearNotification: (id: string) => void;
};

export type BrowserPreviewDependencies = {
  start: (identity: BrowserPreviewRequestIdentity) => Promise<BrowserPreviewStartReceipt | null>;
  openUrl: (url: string) => Promise<void>;
};

export type BrowserPreviewOpenOptions = {
  route?: string | null;
};

type FrontendBrowserPreviewRequestIdentity = BrowserPreviewRequestIdentity & {
  expectedProjectTransitionGeneration: number;
};

const browserPreviewDependencies: BrowserPreviewDependencies = {
  start: startProjectBrowserPreview,
  openUrl: openExternalUrl,
};

export type ProjectBrowserPreviewServiceDependencies = {
  project: ProjectSessionState;
  transition: ProjectTransitionLeaseState;
  status: GlobalStatusState;
  route: () => string;
};

/** Opens the current Rust ProjectSession in an isolated source-browser server. */
export class ProjectBrowserPreviewService {
  private readonly host: ProjectBrowserPreviewHost;
  private readonly dependencies: ProjectBrowserPreviewServiceDependencies;

  constructor(dependencies: ProjectBrowserPreviewServiceDependencies) {
    this.dependencies = dependencies;
    const { project, transition, status } = dependencies;
    this.host = {
      get scannedProject() { return project.project; },
      get sessionProjectRoot() { return project.root; },
      get kernelProjectSessionId() { return project.runtimeSessionId; },
      get projectTransitionFrontendLeaseActive() { return transition.isActive; },
      get projectTransitionFrontendLeaseGeneration() { return transition.generation; },
      setGlobalStatus: (text, kind) => status.set(text, kind),
      escalateGlobalStatus: (notification) => status.escalate(notification),
      clearNotification: (id) => status.clear(id),
    };
  }

  async open(route: string | null = null) {
    await openCurrentProjectInBrowser(this.host, undefined, {
      route: route?.trim() || this.dependencies.route(),
    });
  }
}

export function captureBrowserPreviewRequestIdentity(
  host: Pick<
    ProjectBrowserPreviewHost,
    | "scannedProject"
    | "sessionProjectRoot"
    | "kernelProjectSessionId"
    | "projectTransitionFrontendLeaseActive"
    | "projectTransitionFrontendLeaseGeneration"
  >,
): FrontendBrowserPreviewRequestIdentity | null {
  const projectRoot = host.scannedProject?.root.trim() ?? "";
  const runtimeSessionId = host.kernelProjectSessionId.trim();
  const expectedDiskGeneration = host.scannedProject?.acceptedDiskGeneration;
  if (
    !host.scannedProject
    || !projectRoot
    || !runtimeSessionId
    || host.sessionProjectRoot.trim() !== projectRoot
    || host.projectTransitionFrontendLeaseActive
    || !Number.isSafeInteger(expectedDiskGeneration)
    || (expectedDiskGeneration ?? 0) < 1
  ) {
    return null;
  }
  return {
    expectedProjectRoot: projectRoot,
    expectedSessionId: runtimeSessionId,
    expectedDiskGeneration: expectedDiskGeneration as number,
    expectedProjectTransitionGeneration:
      host.projectTransitionFrontendLeaseGeneration ?? 0,
  };
}

export function isBrowserPreviewRequestIdentityCurrent(
  host: Pick<
    ProjectBrowserPreviewHost,
    | "scannedProject"
    | "sessionProjectRoot"
    | "kernelProjectSessionId"
    | "projectTransitionFrontendLeaseActive"
    | "projectTransitionFrontendLeaseGeneration"
  >,
  identity: FrontendBrowserPreviewRequestIdentity,
) {
  return !host.projectTransitionFrontendLeaseActive
    && (host.projectTransitionFrontendLeaseGeneration ?? 0)
      === identity.expectedProjectTransitionGeneration
    && host.scannedProject?.root === identity.expectedProjectRoot
    && host.scannedProject !== null
    && host.scannedProject.acceptedDiskGeneration === identity.expectedDiskGeneration
    && host.sessionProjectRoot === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId;
}

export async function openCurrentProjectInBrowser(
  host: ProjectBrowserPreviewHost,
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
    const receipt = await dependencies.start({
      expectedProjectRoot: identity.expectedProjectRoot,
      expectedSessionId: identity.expectedSessionId,
      expectedDiskGeneration: identity.expectedDiskGeneration,
    });
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
      throw new Error(t("project-controller-browser-receipt-mismatch"));
    }
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
