import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { SourceWorkspaceState } from "$lib/editor/source-workspace.svelte";
import type { ProjectAnalysisState } from "$lib/project/analysis-state.svelte";
import type { CssAuthoringState } from "$lib/css/authoring-state.svelte";
import type { PublishWorkspaceState } from "$lib/deploy/publish-state.svelte";
import type { WorkbenchWorkspaceState } from "$lib/workbench/workspace-state.svelte";
import type { ProjectResetService } from "$lib/project/reset-service";
import type { ProjectDocumentService } from "$lib/project/document-service";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { AcceptedDiskState } from "$lib/session/accepted-disk-state.svelte";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { ProjectStartupState } from "$lib/project/startup-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import {
  publishProjectSessionIntoFrontend,
  type ProjectAttachmentHost,
} from "$lib/state/project-attachment-controller";
import type {
  FrontendProjectAttachmentMode,
} from "$lib/project/controller-contracts";
import type { ProjectScan } from "$lib/project/lifecycle-contract";
import type { ProjectOpenBootstrapReceipt } from "$lib/project/lifecycle-contract";
import type { ProjectTransitionFrontendLease } from "$lib/state/project-transition-frontend-lease";

export type ProjectAttachmentServiceDependencies = Readonly<{
  shell: ApplicationShellState;
  project: ProjectSessionState;
  source: SourceWorkspaceState;
  analysis: ProjectAnalysisState;
  css: CssAuthoringState;
  publish: PublishWorkspaceState;
  workbench: WorkbenchWorkspaceState;
  reset: ProjectResetService;
  documents: ProjectDocumentService;
  externalDisk: ExternalDiskState;
  acceptedDisk: AcceptedDiskState;
  transition: ProjectTransitionLeaseState;
  startup: ProjectStartupState;
  status: GlobalStatusState;
  setProjectRoot: (root?: string) => void;
}>;

/** Publishes one Rust bootstrap receipt into domain-owned frontend state. */
export class ProjectAttachmentService {
  private readonly dependencies: ProjectAttachmentServiceDependencies;

  constructor(dependencies: ProjectAttachmentServiceDependencies) {
    this.dependencies = dependencies;
  }

  host(): ProjectAttachmentHost {
    const d = this.dependencies;
    return {
      get applicationSurface() { return d.shell.surface; },
      set applicationSurface(surface) { d.shell.surface = surface; },
      get scannedProject() { return d.project.project; },
      set scannedProject(project) { d.project.project = project; },
      get projectLifecycle() { return d.project.lifecycle; },
      set projectLifecycle(lifecycle) { d.project.lifecycle = lifecycle; },
      get projectOpenRecoveryDecisionRequest() { return d.startup.openRecoveryDecision; },
      set projectOpenRecoveryDecisionRequest(request) { d.startup.openRecoveryDecision = request; },
      get projectTransitionDecisionRequest() { return d.startup.transitionDecision; },
      set projectTransitionDecisionRequest(request) { d.startup.transitionDecision = request; },
      get projectStatus() { return d.project.status; },
      set projectStatus(status) { d.project.status = status; },
      get projectWorkspaceSnapshot() { return d.project.workspace; },
      set projectWorkspaceSnapshot(snapshot) { d.project.workspace = snapshot; },
      get workbenchSnapshot() { return d.workbench.snapshot; },
      set workbenchSnapshot(snapshot) {
        if (snapshot) d.workbench.acceptSnapshot(snapshot);
        else d.workbench.reset();
      },
      get sourceCache() { return d.source.sourceCache; },
      set sourceCache(cache) { d.source.sourceCache = cache; },
      get scssVariables() { return d.analysis.scssVariables; },
      set scssVariables(variables) { d.analysis.scssVariables = variables; },
      get targetCssFile() { return d.css.targetFile; },
      set targetCssFile(file) { d.css.targetFile = file; },
      publishWorkspace: d.publish,
      get sessionProjectRoot() { return d.project.root; },
      get kernelProjectSessionId() { return d.project.runtimeSessionId; },
      set kernelProjectSessionId(sessionId) { d.project.runtimeSessionId = sessionId; },
      get diskState() { return d.acceptedDisk.snapshot; },
      set diskState(snapshot) { d.acceptedDisk.snapshot = snapshot; },
      get projectTransitionFrontendLeaseGeneration() { return d.transition.generation; },
      setSessionProjectRoot: d.setProjectRoot,
      resetProjectSessionProjection: (options) => d.reset.reset(options),
      requireProjectTransitionFrontendLease: (lease) => d.transition.require(lease),
      loadScannedProjectFile: (file, options) => d.documents.load(file, options),
      hydrateWorkbenchBootstrap: (snapshot) => d.workbench.hydrateBootstrap(snapshot),
      revealBootstrapDiagnosticLocation: (path, location) => (
        d.source.revealBootstrapDiagnostic(path, location)
      ),
      resetExternalDiskState: () => d.externalDisk.reset(),
      establishExternalDiskBaseline: () => d.externalDisk.establishBaseline(),
      startExternalDiskMonitoring: () => d.externalDisk.start(),
      clearNotification: (id) => d.status.clear(id),
      setGlobalStatus: (text, kind) => d.status.set(text, kind),
      escalateGlobalStatus: (notification) => d.status.escalate(notification),
    };
  }

  attach(
    project: ProjectScan,
    mode: FrontendProjectAttachmentMode,
    bootstrap: ProjectOpenBootstrapReceipt,
    lease: ProjectTransitionFrontendLease,
  ) {
    return publishProjectSessionIntoFrontend(this.host(), project, mode, bootstrap, lease);
  }
}
