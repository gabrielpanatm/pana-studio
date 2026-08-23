import type { TemplateWorkbenchPlan } from "$lib/project/template-workbench-contract";
import type {
  CanvasProjectionPlan,
  PreviewPhaseReceipt,
  PreviewRuntimeEventInput,
  PreviewRuntimeEventReceipt,
} from "$lib/contracts/canvas-projection";
import { invoke } from "@tauri-apps/api/core";
import { t } from "$lib/i18n/runtime.svelte";

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

export type TemplateWorkbenchReuseRequest = TemplateWorkbenchPreviewRequest & {
  reuseToken: string;
  expectedPreviewRevision: string;
  expectedCanvasTransactionId: string;
};

export type TemplateWorkbenchReusePerformance = {
  totalUs: number;
  operationLockWaitUs: number;
  engineLockWaitUs: number;
};

export type TemplateWorkbenchReuseReceipt = {
  status: "confirmed" | "miss";
  route: string | null;
  previewUrl: string | null;
  reuseToken: string | null;
  workspaceRevision: number;
  previewRevision: string | null;
  canvasTransactionId: string | null;
  performance: TemplateWorkbenchReusePerformance;
};

export type TemplateWorkbenchPublicationStatus = "reused" | "materialized";

export type TemplateWorkbenchPerformance = {
  totalUs: number;
  operationLockWaitUs: number;
  projectModelUs: number;
  planUs: number;
  engineLockWaitUs: number;
  publishUs: number;
  renderUs: number;
  graphUs: number;
  prepareUs: number;
  modelCacheHit: boolean;
};

export type TemplateWorkbenchPreviewReceipt = {
  plan: TemplateWorkbenchPlan;
  route: string;
  previewUrl: string;
  reuseToken: string;
  workspaceRevision: number;
  previewRevision: string;
  canvasProjection: CanvasProjectionPlan;
  publicationStatus: TemplateWorkbenchPublicationStatus;
  performance: TemplateWorkbenchPerformance;
};

type ProjectPreviewMutationReceipt = {
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

export function confirmTemplateWorkbenchReuse(
  input: TemplateWorkbenchReuseRequest,
): Promise<TemplateWorkbenchReuseReceipt> {
  return invoke<TemplateWorkbenchReuseReceipt>("confirm_template_workbench_reuse", {
    input,
  });
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
