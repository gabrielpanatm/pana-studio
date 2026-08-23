import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  exitTemplateWorkbench,
  synchronizeActiveCanvasSurfaceRoute,
  updateTemplateWorkbenchContext,
  type ProjectTemplateWorkbenchHost,
} from "$lib/state/project-template-workbench-controller";
import { mountBootstrapInitialSurface } from "$lib/state/project-preview-bootstrap-controller";
import type {
  CanvasProjectionIdentity,
} from "$lib/contracts/canvas-projection";
import type {
  ProjectPreviewStartReceipt,
} from "$lib/preview/io";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { ProjectBootstrapInitialSurface } from "$lib/project/lifecycle-contract";
import { t } from "$lib/i18n/runtime.svelte";

export type TemplateWorkbenchServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  preview: PreviewWorkspaceState;
  selection: SelectionWorkspaceState;
  status: GlobalStatusState;
}>;

/** Owns the canonical Template Workbench context and its Canvas route. */
export class TemplateWorkbenchService {
  private readonly dependencies: TemplateWorkbenchServiceDependencies;

  constructor(dependencies: TemplateWorkbenchServiceDependencies) {
    this.dependencies = dependencies;
  }

  host(): ProjectTemplateWorkbenchHost {
    const d = this.dependencies;
    return {
      get activeCanvasIdentity() { return d.preview.activeIdentity; },
      set activeCanvasIdentity(identity) { d.preview.activeIdentity = identity; },
      get activeCanvasUrl() { return d.preview.activeUrl; },
      set activeCanvasUrl(url) { d.preview.activeUrl = url; },
      get activePreviewPath() { return d.documents.activePreviewPath; },
      set activePreviewPath(path) { d.documents.activePreviewPath = path; },
      get activeScannedPath() { return d.documents.activeScannedPath; },
      set activeScannedPath(path) { d.documents.activeScannedPath = path; },
      get browserPreviewRoute() { return d.documents.browserPreviewRoute; },
      set browserPreviewRoute(route) { d.documents.browserPreviewRoute = route; },
      get kernelProjectSessionId() { return d.project.runtimeSessionId; },
      get previewDocumentMarkup() { return d.preview.documentMarkup; },
      set previewDocumentMarkup(markup) { d.preview.documentMarkup = markup; },
      get previewSrc() { return d.preview.src; },
      set previewSrc(src) { d.preview.src = src; },
      get projectLifecycle() { return d.project.lifecycle; },
      set projectLifecycle(lifecycle) { d.project.lifecycle = lifecycle; },
      get projectSessionEpoch() { return d.project.epoch; },
      get projectWorkspaceMutationEpoch() { return d.project.workspaceMutationEpoch; },
      get projectWorkspaceSnapshot() { return d.project.workspace; },
      get scannedProject() { return d.project.project; },
      get sessionProjectRoot() { return d.project.root; },
      get templateWorkbenchActive() { return d.documents.templateActive; },
      set templateWorkbenchActive(active) { d.documents.templateActive = active; },
      get templateWorkbenchPlan() { return d.documents.templatePlan; },
      set templateWorkbenchPlan(plan) { d.documents.templatePlan = plan; },
      get templateWorkbenchPreferredPagePath() { return d.documents.templatePreferredPagePath; },
      set templateWorkbenchPreferredPagePath(path) { d.documents.templatePreferredPagePath = path; },
      get templateWorkbenchPreferredRoute() { return d.documents.templatePreferredRoute; },
      set templateWorkbenchPreferredRoute(route) { d.documents.templatePreferredRoute = route; },
      get templateWorkbenchRequestSerial() { return d.documents.templateRequestSerial; },
      set templateWorkbenchRequestSerial(serial) { d.documents.templateRequestSerial = serial; },
      get templateWorkbenchReturnPreviewPath() { return d.documents.templateReturnPreviewPath; },
      set templateWorkbenchReturnPreviewPath(path) { d.documents.templateReturnPreviewPath = path; },
      get templateWorkbenchTarget() { return d.documents.templateTarget; },
      set templateWorkbenchTarget(target) { d.documents.templateTarget = target; },
      editorSelection: d.selection.session,
      templateWorkbenchCanvas: {
        reconcile: (url, plan) => d.preview.reconcileWorkbenchDocument(url, plan),
        canReuse: (identity, url) => d.preview.canReuseCanonicalWorkbenchSurface(identity, url),
        getReuseToken: () => d.documents.templateReuseToken,
        setReuseToken: (token) => { d.documents.templateReuseToken = token; },
        setPublicationStatus: (status) => { d.documents.templatePublicationStatus = status; },
      },
      refreshRenderedPreviewDocument: () => d.preview.refreshDocument(),
      setGlobalStatus: (text, kind) => d.status.set(text, kind),
    };
  }

  update(
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
    return updateTemplateWorkbenchContext(
      this.host(),
      project,
      templateFile,
      preferredPagePath,
      options,
    );
  }

  synchronizeRoute(previewSrc: string, expectedIdentity?: CanvasProjectionIdentity) {
    return synchronizeActiveCanvasSurfaceRoute(this.host(), previewSrc, expectedIdentity);
  }

  mountBootstrap(
    activeFile: ProjectFile | null,
    surface: ProjectBootstrapInitialSurface,
    receipt: ProjectPreviewStartReceipt,
  ) {
    return mountBootstrapInitialSurface(this.host(), activeFile, surface, receipt);
  }

  exit(options: { deferPreviewRefresh?: boolean; returnPath?: string | null } = {}) {
    return exitTemplateWorkbench(this.host(), options);
  }

  async reproject(minimumWorkspaceRevision: number) {
    const { project, documents, preview } = this.dependencies;
    if (!documents.templateActive) return false;
    if (project.workspace?.revision !== minimumWorkspaceRevision) return false;
    const target = documents.templateTarget;
    const templateFile = project.project && target
      ? project.project.files.find(
        (file) => file.relativePath === target && file.role === "template",
      ) ?? null
      : null;
    if (!project.project || !templateFile || documents.activeScannedPath !== target) {
      throw new Error(t("workbench-template-context-missing"));
    }
    await this.update(
      project.project,
      templateFile,
      documents.templatePreferredPagePath,
      {
        expectedWorkspaceRevision: minimumWorkspaceRevision,
        minimumWorkspaceRevision,
        preferredRoute: documents.templatePreferredRoute,
        strict: true,
      },
    );
    return documents.templateActive
      && preview.activeIdentity?.projectRoot === project.root
      && preview.activeIdentity?.runtimeSessionId === project.runtimeSessionId
      && preview.activeIdentity?.workspaceRevision === minimumWorkspaceRevision;
  }
}
