import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { PreviewWorkspaceState } from "$lib/preview/workspace-state.svelte";
import type { SelectionWorkspaceState } from "$lib/editor/selection-workspace.svelte";
import type { TemplateWorkbenchService } from "$lib/project/template-workbench-service";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { ControlledPreviewWorkspaceState } from "$lib/preview/controlled-state.svelte";
import type { PreviewRuntimeService } from "$lib/preview/runtime-service";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  refreshSourceGraphAfterCommit,
  startPreviewAfterOpen,
  type ProjectPreviewBootstrapHost,
} from "$lib/state/project-preview-bootstrap-controller";
import type { FrontendProjectAttachment } from "$lib/project/controller-contracts";

export type ProjectPreviewBootstrapServiceDependencies = Readonly<{
  project: ProjectSessionState;
  documents: ProjectDocumentWorkspaceState;
  preview: PreviewWorkspaceState;
  selection: SelectionWorkspaceState;
  template: TemplateWorkbenchService;
  transition: ProjectTransitionLeaseState;
  controlled: ControlledPreviewWorkspaceState;
  runtime: PreviewRuntimeService;
  status: GlobalStatusState;
}>;

/** Starts and validates the first Preview projection after a project attach. */
export class ProjectPreviewBootstrapService {
  private readonly dependencies: ProjectPreviewBootstrapServiceDependencies;

  constructor(dependencies: ProjectPreviewBootstrapServiceDependencies) {
    this.dependencies = dependencies;
  }

  host(): ProjectPreviewBootstrapHost {
    const d = this.dependencies;
    return {
      get scannedProject() { return d.project.project; },
      set scannedProject(project) { d.project.project = project; },
      get sessionProjectRoot() { return d.project.root; },
      get kernelProjectSessionId() { return d.project.runtimeSessionId; },
      get activeScannedPath() { return d.documents.activeScannedPath; },
      set activeScannedPath(path) { d.documents.activeScannedPath = path; },
      get projectLifecycle() { return d.project.lifecycle; },
      set projectLifecycle(lifecycle) { d.project.lifecycle = lifecycle; },
      get previewSrc() { return d.preview.src; },
      set previewSrc(src) { d.preview.src = src; },
      get activePreviewPath() { return d.documents.activePreviewPath; },
      set activePreviewPath(path) { d.documents.activePreviewPath = path; },
      get browserPreviewRoute() { return d.documents.browserPreviewRoute; },
      set browserPreviewRoute(route) { d.documents.browserPreviewRoute = route; },
      get previewDocumentMarkup() { return d.preview.documentMarkup; },
      set previewDocumentMarkup(markup) { d.preview.documentMarkup = markup; },
      get activeCanvasIdentity() { return d.preview.activeIdentity; },
      set activeCanvasIdentity(identity) { d.preview.activeIdentity = identity; },
      get activeCanvasUrl() { return d.preview.activeUrl; },
      set activeCanvasUrl(url) { d.preview.activeUrl = url; },
      editorSelection: d.selection.session,
      get templateWorkbenchPreferredPagePath() { return d.documents.templatePreferredPagePath; },
      set templateWorkbenchPreferredPagePath(path) { d.documents.templatePreferredPagePath = path; },
      get templateWorkbenchPreferredRoute() { return d.documents.templatePreferredRoute; },
      set templateWorkbenchPreferredRoute(route) { d.documents.templatePreferredRoute = route; },
      get projectTransitionFrontendLeaseActive() { return d.transition.isActive; },
      get projectTransitionFrontendLeaseGeneration() { return d.transition.generation; },
      get pendingCanvasProjection() { return d.preview.pendingProjection; },
      set pendingCanvasProjection(plan) { d.preview.setPendingProjection(plan); },
      get previewWorkspaceRevision() { return d.preview.workspaceRevision; },
      set previewWorkspaceRevision(revision) { d.preview.workspaceRevision = revision; },
      scheduleZolaValidation: (reason) => d.controlled.scheduleValidation(reason),
      prepareCanvasProjectionNavigation: (plan) => d.preview.prepareNavigation(plan),
      hasMountedCanvasProjectionSurface: () => d.preview.hasMountedSurface(),
      deferWorkspacePreviewProjection: () => d.preview.deferSurfaceProjection(),
      markCanvasProjectionSurfaceCurrent: () => d.preview.markSurfaceCurrent(),
      resetControlledPreviewState: () => d.preview.resetControlled(),
      refreshSourceGraph: async (options) => { await d.runtime.refreshSourceGraph(options); },
      mountBootstrapInitialTemplateSurface: (file, surface, receipt) => (
        d.template.mountBootstrap(file, surface, receipt)
      ),
      synchronizeProjectCanvasSurfaceRoute: (url, identity) => (
        d.template.synchronizeRoute(url, identity)
      ),
      updateTemplateWorkbenchContext: (project, file, path, options) => (
        d.template.update(project, file, path, options)
      ),
      clearNotification: (id) => d.status.clear(id),
      escalateGlobalStatus: (notification) => d.status.escalate(notification),
      setGlobalStatus: (text, kind) => d.status.set(text, kind),
    };
  }

  start(attachment: FrontendProjectAttachment) {
    return startPreviewAfterOpen(this.host(), attachment);
  }

  refreshSourceGraph(attachment: FrontendProjectAttachment) {
    return refreshSourceGraphAfterCommit(this.host(), attachment);
  }
}
