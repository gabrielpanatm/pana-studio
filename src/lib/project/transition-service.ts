import type { ApplicationShellState } from "$lib/application/shell-state.svelte";
import type { ProjectSessionState } from "$lib/project/session-state.svelte";
import type { ProjectStartupState } from "$lib/project/startup-state.svelte";
import type { ProjectDocumentWorkspaceState } from "$lib/project/document-workspace.svelte";
import type { ProjectTransitionLeaseState } from "$lib/project/transition-lease-state.svelte";
import type { ProjectAttachmentService } from "$lib/project/attachment-service";
import type { ProjectPreviewBootstrapService } from "$lib/project/preview-bootstrap-service";
import type { ProjectResetService } from "$lib/project/reset-service";
import type { ExternalDiskState } from "$lib/session/external-disk-state.svelte";
import type { AcceptedDiskState } from "$lib/session/accepted-disk-state.svelte";
import type { AiCoordinationState } from "$lib/ai/coordination-state.svelte";
import type { GlobalStatusState } from "$lib/status/state.svelte";
import type { TerminalWorkspaceState } from "$lib/terminal/workspace.svelte";
import type { ProjectAuditWorkspaceState } from "$lib/audit/workspace-state.svelte";
import {
  cancelProjectOpenRecoveryDecision,
  closeCurrentProject,
  continueProjectOpenWithRecoveryAbandonment,
  continueProjectTransitionWithOperatorDecision,
  discardSessionAndReloadFromDisk,
  openProjectRoot,
  reattachCurrentProjectSession,
} from "$lib/state/project-transition-controller";
import {
  cancelPendingNativeWindowClose,
  closeNativeWindowIfProjectClosed,
  type NativeWindowCloseControllerHost,
} from "$lib/state/native-window-close-controller";
import {
  PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
} from "$lib/project/transition-decision";
import type { OpenProjectRootOptions } from "$lib/project/controller-contracts";
import type { ProjectTransitionFrontendLeaseOwner } from "$lib/state/project-transition-frontend-lease";
import type {
  ProjectTransitionFrontendLease,
  ProjectTransitionFrontendLeaseRequest,
} from "$lib/state/project-transition-frontend-lease";
import { tick } from "svelte";
import { t } from "$lib/i18n/runtime.svelte";

export type ProjectTransitionServiceDependencies = Readonly<{
  project: ProjectSessionState;
  startup: ProjectStartupState;
  documents: ProjectDocumentWorkspaceState;
  lease: ProjectTransitionLeaseState;
  attachment: ProjectAttachmentService;
  preview: ProjectPreviewBootstrapService;
  reset: ProjectResetService;
  externalDisk: ExternalDiskState;
  acceptedDisk: AcceptedDiskState;
  ai: AiCoordinationState;
  status: GlobalStatusState;
  shell: ApplicationShellState;
  terminal: TerminalWorkspaceState;
  audit: ProjectAuditWorkspaceState;
}>;

/** Serializes project open/reattach/reload/close under the single frontend lease. */
export class ProjectTransitionService {
  private readonly dependencies: ProjectTransitionServiceDependencies;

  constructor(dependencies: ProjectTransitionServiceDependencies) {
    this.dependencies = dependencies;
  }

  private host() {
    const d = this.dependencies;
    return {
      get scannedProject() { return d.project.project; },
      set scannedProject(project) { d.project.project = project; },
      get projectLifecycle() { return d.project.lifecycle; },
      set projectLifecycle(lifecycle) { d.project.lifecycle = lifecycle; },
      get startupFlow() { return d.startup.flow; },
      set startupFlow(flow) { d.startup.flow = flow; },
      get projectOpenRecoveryDecisionRequest() { return d.startup.openRecoveryDecision; },
      set projectOpenRecoveryDecisionRequest(request) { d.startup.openRecoveryDecision = request; },
      get projectTransitionDecisionRequest() { return d.startup.transitionDecision; },
      set projectTransitionDecisionRequest(request) { d.startup.transitionDecision = request; },
      get projectStatus() { return d.project.status; },
      set projectStatus(status) { d.project.status = status; },
      get sessionProjectRoot() { return d.project.root; },
      set sessionProjectRoot(root) { d.project.root = root; },
      get kernelProjectSessionId() { return d.project.runtimeSessionId; },
      set kernelProjectSessionId(sessionId) { d.project.runtimeSessionId = sessionId; },
      get activeScannedPath() { return d.documents.activeScannedPath; },
      set activeScannedPath(path) { d.documents.activeScannedPath = path; },
      get diskState() { return d.acceptedDisk.snapshot; },
      set diskState(snapshot) { d.acceptedDisk.snapshot = snapshot; },
      get aiReconciliationRecoveryReloadAuthorized() { return d.ai.recoveryReloadAuthorized; },
      set aiReconciliationRecoveryReloadAuthorized(value) { d.ai.recoveryReloadAuthorized = value; },
      runWithProjectTransitionFrontendLease: <T>(
        request: ProjectTransitionFrontendLeaseRequest,
        operation: (lease: ProjectTransitionFrontendLease) => Promise<T>,
      ) => d.lease.run(request, operation),
      requireProjectTransitionFrontendLease: (lease: ProjectTransitionFrontendLease) => d.lease.require(lease),
      invalidateExternalReconcileForProjectTransition: async () => {
        d.externalDisk.invalidateForProjectTransition();
        await tick();
      },
      markWorkspaceProjectionRecoveryRequired: (message: string) => (
        d.externalDisk.markProjectionRecoveryRequired(message)
      ),
      clearNotification: (id: string) => d.status.clear(id),
      setGlobalStatus: (text: string, kind: Parameters<GlobalStatusState["set"]>[1]) => d.status.set(text, kind),
      escalateGlobalStatus: (notification: Parameters<GlobalStatusState["escalate"]>[0]) => d.status.escalate(notification),
      attachPublishedProjectSession: (
        project: Parameters<ProjectAttachmentService["attach"]>[0],
        mode: Parameters<ProjectAttachmentService["attach"]>[1],
        bootstrap: Parameters<ProjectAttachmentService["attach"]>[2],
        lease: Parameters<ProjectAttachmentService["attach"]>[3],
      ) => d.attachment.attach(project, mode, bootstrap, lease),
      startAttachedProjectPreview: (attachment: Parameters<ProjectPreviewBootstrapService["start"]>[0]) => (
        d.preview.start(attachment)
      ),
      refreshAttachedProjectSourceGraph: (attachment: Parameters<ProjectPreviewBootstrapService["refreshSourceGraph"]>[0]) => (
        d.preview.refreshSourceGraph(attachment)
      ),
      resetProjectSessionProjection: (options?: Parameters<ProjectResetService["reset"]>[0]) => (
        d.reset.reset(options)
      ),
    };
  }

  nativeWindowHost(): NativeWindowCloseControllerHost {
    const d = this.dependencies;
    return {
      get nativeWindowClosePending() { return d.shell.nativeWindowClosePending; },
      set nativeWindowClosePending(pending) { d.shell.nativeWindowClosePending = pending; },
      get nativeWindowCloseInProgress() { return d.shell.nativeWindowCloseInProgress; },
      set nativeWindowCloseInProgress(inProgress) { d.shell.nativeWindowCloseInProgress = inProgress; },
      get projectTransitionFrontendLeaseActive() { return d.lease.isActive; },
      get projectTransitionFrontendLease() { return d.lease.active; },
      get scannedProject() { return d.project.project; },
      get projectTransitionDecisionRequest() { return d.startup.transitionDecision; },
      closeCurrentProject: (root, owner) => this.close(root, owner),
      waitForProjectTransitionFrontendLeaseIdle: () => d.lease.waitForIdle(),
      setGlobalStatus: (text, kind) => d.status.set(text, kind),
    };
  }

  open(root: string, options: OpenProjectRootOptions = {}) {
    return openProjectRoot(this.host(), root, options);
  }

  async reattach() {
    const project = this.dependencies.project;
    if (project.project) return true;
    if (project.reattachPromise) return await project.reattachPromise;
    const operation = reattachCurrentProjectSession(this.host());
    project.reattachPromise = operation;
    try {
      return await operation;
    } finally {
      if (project.reattachPromise === operation) project.reattachPromise = null;
    }
  }

  cancelOpenRecovery(requestId: string) {
    return cancelProjectOpenRecoveryDecision(this.host(), requestId);
  }

  confirmOpenRecoveryAbandonment(requestId: string) {
    return continueProjectOpenWithRecoveryAbandonment(this.host(), requestId);
  }

  cancelOperatorDecision(requestId: string) {
    const d = this.dependencies;
    if (d.startup.transitionDecision?.id !== requestId) return;
    d.startup.transitionDecision = null;
    cancelPendingNativeWindowClose(this.nativeWindowHost());
    d.status.clear(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
    d.status.set(t("workbench-transition-cancelled"), "idle");
  }

  async confirmOperatorDecision(requestId: string, diagnostic: string) {
    await continueProjectTransitionWithOperatorDecision(this.host(), requestId, diagnostic);
    if (!this.dependencies.project.project) {
      this.clearClosedRuntimeState();
      await closeNativeWindowIfProjectClosed(this.nativeWindowHost());
    }
  }

  async close(
    detachedProjectRoot: string | null = null,
    leaseOwner: ProjectTransitionFrontendLeaseOwner = "project-transition-controller",
  ) {
    const closed = await closeCurrentProject(this.host(), { detachedProjectRoot, leaseOwner });
    if (closed && !this.dependencies.project.project) {
      this.clearClosedRuntimeState();
      await this.dependencies.startup.refreshFlow();
      await closeNativeWindowIfProjectClosed(this.nativeWindowHost());
    }
    return closed;
  }

  discardAndReload(preferredPath: string | null = this.dependencies.documents.activeScannedPath) {
    return discardSessionAndReloadFromDisk(this.host(), preferredPath);
  }

  clearClosedRuntimeState() {
    const d = this.dependencies;
    d.shell.surface = "workbench";
    d.terminal.reset();
    d.audit.reset({ resetView: true });
    d.startup.clearCreation();
  }
}
