import { t } from "$lib/i18n/runtime.svelte";
import type {
  FrontendProjectAttachment,
  FrontendProjectAttachmentMode,
  OpenProjectRootOptions,
  ProjectPreviewStartOutcome,
  ProjectReloadOutcome,
} from "$lib/project/controller-contracts";
import {
  cancelProjectOpen,
  closeProject,
  inspectProjectOpen,
  openProject,
  readProjectLifecycle,
  reattachProjectSession,
  reportProjectCapabilityDegraded,
} from "$lib/project/io/lifecycle";
import {
  inspectStartupFolder,
} from "$lib/project/io/startup";
import {
  readKernelProjectTransitionPolicy,
  recordProjectTransitionOperatorDecision,
} from "$lib/kernel/recovery-io";
import {
  createProjectOpenRecoveryDecisionRequest,
  projectOpenRecoveryAbandonDecision,
  PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
  type ProjectOpenRecoveryDecisionRequest,
} from "$lib/project/open-recovery";
import {
  createProjectTransitionDecisionRequest,
  localizedTransitionPolicyCopy,
  PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID,
  PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
  projectTransitionActionForContinuation,
  type ProjectTransitionContinuation,
  type ProjectTransitionDecisionRequest,
} from "$lib/project/transition-decision";
import { markDiskMutation, type DiskState } from "$lib/session/disk-state";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import type { ProjectSessionResetOptions } from "$lib/state/project-session-reset";
import type {
  ProjectTransitionFrontendLease,
  ProjectTransitionFrontendLeaseOwner,
  ProjectTransitionFrontendLeaseRequest,
} from "$lib/state/project-transition-frontend-lease";
import type { GlobalStatusEscalationRequest, GlobalStatusKind } from "$lib/status/global-status";
import type {
  ProjectLifecycleSnapshot,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { ProjectOpenBootstrapReceipt } from "$lib/project/lifecycle-contract";
import type { StartupFlowSnapshot } from "$lib/project/lifecycle-contract";
import { errorMessage } from "$lib/util";

type ProjectTransitionStateHost = {
  scannedProject: ProjectScan | null;
  projectLifecycle: ProjectLifecycleSnapshot;
  startupFlow: StartupFlowSnapshot;
  projectOpenRecoveryDecisionRequest: ProjectOpenRecoveryDecisionRequest | null;
  projectTransitionDecisionRequest: ProjectTransitionDecisionRequest | null;
  projectStatus: string;
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  activeScannedPath: string | null;
  diskState: DiskState;
  aiReconciliationRecoveryReloadAuthorized?: boolean;
  runWithProjectTransitionFrontendLease: <T>(
    request: ProjectTransitionFrontendLeaseRequest,
    operation: (lease: ProjectTransitionFrontendLease) => Promise<T>,
  ) => Promise<T>;
  requireProjectTransitionFrontendLease: (lease: ProjectTransitionFrontendLease) => void;
  invalidateExternalReconcileForProjectTransition: () => Promise<void>;
  markWorkspaceProjectionRecoveryRequired: (message: string) => void;
  clearNotification: (id: string) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  escalateGlobalStatus: (notification: GlobalStatusEscalationRequest) => void;
};

type ProjectTransitionCapabilities = {
  attachPublishedProjectSession: (
    project: ProjectScan,
    mode: FrontendProjectAttachmentMode,
    bootstrap: ProjectOpenBootstrapReceipt,
    lease: ProjectTransitionFrontendLease,
  ) => Promise<FrontendProjectAttachment | null>;
  startAttachedProjectPreview: (
    attachment: FrontendProjectAttachment,
  ) => Promise<ProjectPreviewStartOutcome>;
  refreshAttachedProjectSourceGraph: (
    attachment: FrontendProjectAttachment,
  ) => Promise<void>;
  resetProjectSessionProjection: (options?: ProjectSessionResetOptions) => void;
};

type ProjectTransitionHost = ProjectTransitionStateHost & ProjectTransitionCapabilities;
type ProjectCloseTransitionHost = ProjectTransitionStateHost
  & Pick<ProjectTransitionCapabilities, "resetProjectSessionProjection">;

async function flushProjectDraftsBeforeTransition() {
  await flushWorkspaceMutationInputs("manual");
}

export async function reattachCurrentProjectSession(
  host: ProjectTransitionHost,
): Promise<boolean> {
  if (host.scannedProject) return true;
  try {
    const result = await host.runWithProjectTransitionFrontendLease(
      { kind: "reattach", owner: "project-transition-controller" },
      async (lease) => {
        try {
          const bootstrap = await reattachProjectSession();
          host.requireProjectTransitionFrontendLease(lease);
          if (!bootstrap) return { attached: false, previewIdentity: null };
          host.projectLifecycle = bootstrap.lifecycle;
          const previewIdentity = await host.attachPublishedProjectSession(
            bootstrap.project,
            "reattach",
            bootstrap,
            lease,
          );
          host.requireProjectTransitionFrontendLease(lease);
          host.clearNotification("project.reattach.error");
          return { attached: true, previewIdentity };
        } catch (error) {
          await degradeFrontendAttachment(host, errorMessage(error), lease);
          host.requireProjectTransitionFrontendLease(lease);
          throw error;
        }
      },
    );
    if (result.previewIdentity) {
      await host.startAttachedProjectPreview(result.previewIdentity);
      void host.refreshAttachedProjectSourceGraph(result.previewIdentity);
    }
    return result.attached;
  } catch (error) {
    const message = t("project-controller-reattach-failed", {
      message: errorMessage(error),
    });
    host.projectStatus = message;
    host.escalateGlobalStatus({
      id: "project.reattach.error",
      level: "error",
      title: t("project-controller-reattach-failed-title"),
      message,
      statusMessage: message,
    });
    throw error;
  }
}

export async function openProjectRoot(
  host: ProjectTransitionHost,
  root: string,
  options: OpenProjectRootOptions = {},
) {
  const previewIdentity = await host.runWithProjectTransitionFrontendLease(
    { kind: "open", owner: "project-transition-controller" },
    async (lease) => {
      await flushProjectDraftsBeforeTransition();
      host.requireProjectTransitionFrontendLease(lease);
      const transitionAllowed = await prepareProjectTransitionForTarget(
        host,
        root,
        { kind: "open_project" },
        options.operatorDecisionId ?? null,
        () => host.requireProjectTransitionFrontendLease(lease),
      );
      host.requireProjectTransitionFrontendLease(lease);
      if (!transitionAllowed) return null;

      const openAction = projectTransitionActionForContinuation(
        root,
        host.scannedProject?.root,
        { kind: "open_project" },
      );
      let inspection = options.inspection ?? null;
      if (!inspection) {
        let candidate = options.startupCandidate ?? null;
        if (candidate?.kind !== "valid_project" || candidate.root !== root) {
          const startup = await inspectStartupFolder(root);
          host.requireProjectTransitionFrontendLease(lease);
          host.startupFlow = startup;
          candidate = startup.candidate;
        }
        if (candidate?.kind !== "valid_project") {
          throw new Error(
            candidate?.diagnostics[0]?.message ?? "Dosarul nu este un proiect Zola valid.",
          );
        }
        inspection = await inspectProjectOpen(candidate.root, candidate.snapshotToken);
        host.requireProjectTransitionFrontendLease(lease);
      }
      host.projectLifecycle = inspection.lifecycle;
      if (inspection.lifecycle.operationId !== inspection.operationId) {
        throw new Error("ProjectLifecycle a returnat o inspecție fără operationId autoritar.");
      }

      if (openAction === "open_project") {
        try {
          const assessment = inspection.recovery;
          if (assessment.status === "decision_required") {
            const suppliedToken = options.recoveryDecision?.assessmentToken ?? null;
            if (!suppliedToken) {
              const request = createProjectOpenRecoveryDecisionRequest(
                root,
                assessment,
                options.operatorDecisionId ?? null,
                inspection.operationId,
                inspection.candidateToken,
                inspection,
              );
              host.projectOpenRecoveryDecisionRequest = request;
              host.projectStatus = t("project-controller-recovery-decision-pending");
              host.escalateGlobalStatus({
                id: PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
                level: "warning",
                title: t("project-controller-recovery-incompatible-title"),
                message: t("project-controller-recovery-incompatible-message"),
                statusMessage: t("project-controller-recovery-incompatible-status"),
              });
              return null;
            }
            if (suppliedToken !== assessment.assessmentToken) {
              throw new Error(t("project-controller-recovery-changed"));
            }
          } else if (options.recoveryDecision) {
            throw new Error(t("project-controller-recovery-decision-stale"));
          }
        } catch (error) {
          await cancelProjectOpen(
            inspection.operationId,
            "recovery_validation_failed",
          ).catch(() => {});
          host.requireProjectTransitionFrontendLease(lease);
          throw error;
        }
      } else if (options.recoveryDecision) {
        await cancelProjectOpen(
          inspection.operationId,
          "reload_recovery_decision_invalid",
        ).catch(() => {});
        host.requireProjectTransitionFrontendLease(lease);
        throw new Error(t("project-controller-reload-recovery-decision-invalid"));
      }

      try {
        await host.invalidateExternalReconcileForProjectTransition();
        host.requireProjectTransitionFrontendLease(lease);
      } catch (error) {
        await cancelProjectOpen(
          inspection.operationId,
          "external_reconcile_failed",
        ).catch(() => {});
        host.requireProjectTransitionFrontendLease(lease);
        throw error;
      }

      let rustSessionSwapped = false;
      try {
        const bootstrap = await openProject(
          root,
          inspection.operationId,
          inspection.candidateToken,
          options.operatorDecisionId ?? undefined,
          options.recoveryDecision ?? undefined,
        );
        host.requireProjectTransitionFrontendLease(lease);
        const project = bootstrap.project;
        host.projectLifecycle = bootstrap.lifecycle;
        rustSessionSwapped = true;
        const attachment = await host.attachPublishedProjectSession(
          project,
          "open",
          bootstrap,
          lease,
        );
        host.requireProjectTransitionFrontendLease(lease);
        return attachment;
      } catch (error) {
        const lifecycle = await readProjectLifecycle().catch(() => host.projectLifecycle);
        host.requireProjectTransitionFrontendLease(lease);
        host.projectLifecycle = lifecycle;
        if (rustSessionSwapped) {
          await degradeFrontendAttachment(host, errorMessage(error), lease);
          host.requireProjectTransitionFrontendLease(lease);
          host.markWorkspaceProjectionRecoveryRequired(
            t("project-controller-initial-projection-incomplete"),
          );
        }
        throw error;
      }
    },
  );
  if (previewIdentity) {
    await host.startAttachedProjectPreview(previewIdentity);
    void host.refreshAttachedProjectSourceGraph(previewIdentity);
  }
}

async function degradeFrontendAttachment(
  host: ProjectTransitionStateHost,
  diagnostic: string,
  lease: ProjectTransitionFrontendLease,
) {
  const active = host.projectLifecycle.activeSession;
  if (!active) return;
  const degradedLifecycle = await reportProjectCapabilityDegraded(
    active.projectRoot,
    active.runtimeSessionId,
    "frontend",
    diagnostic,
  ).catch(() => host.projectLifecycle);
  host.requireProjectTransitionFrontendLease(lease);
  host.projectLifecycle = degradedLifecycle;
}

async function prepareProjectTransitionForTarget(
  host: ProjectTransitionStateHost,
  targetRoot: string,
  continuation: ProjectTransitionContinuation,
  operatorDecisionId: string | null,
  requireCurrent: () => void,
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
  requireCurrent();
  if (policy.decision === "allow") return true;

  const policyCopy = localizedTransitionPolicyCopy(policy);

  if (policy.decision === "confirm") {
    const request = createProjectTransitionDecisionRequest(
      targetRoot,
      currentProjectRoot,
      policy,
      continuation,
    );
    host.projectTransitionDecisionRequest = request;
    host.projectStatus = policyCopy.message;
    host.escalateGlobalStatus({
      id: PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
      level: "warning",
      title: policyCopy.title,
      message: `${policyCopy.message} ${policyCopy.recommendedAction}`,
    });
    return false;
  }

  host.projectTransitionDecisionRequest = null;
  const message = `${policyCopy.title}: ${policyCopy.message} ${policyCopy.recommendedAction}`;
  host.projectStatus = message;
  host.escalateGlobalStatus({
    id: PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID,
    level: "error",
    title: policyCopy.title,
    message,
    statusMessage: message,
  });
  return false;
}

export async function continueProjectTransitionWithOperatorDecision(
  host: ProjectTransitionHost,
  requestId: string,
  diagnostic: string,
) {
  const request = host.projectTransitionDecisionRequest;
  if (!request || request.id !== requestId) {
    throw new Error(t("project-transition-decision-expired"));
  }
  host.projectStatus = t("project-controller-recording-decision");
  host.setGlobalStatus(t("project-controller-recording-decision-status"), "saving");
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
    const message = t("project-controller-transition-cannot-continue", {
      message: errorMessage(error),
    });
    host.projectStatus = message;
    host.escalateGlobalStatus({
      id: PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID,
      level: "error",
      title: t("project-controller-transition-refused-title"),
      message,
      statusMessage: message,
    });
    throw error;
  }
}

export async function cancelProjectOpenRecoveryDecision(
  host: ProjectTransitionStateHost,
  requestId: string,
) {
  const request = host.projectOpenRecoveryDecisionRequest;
  if (request?.id !== requestId) return;
  if (request.operationId) {
    host.projectLifecycle = await cancelProjectOpen(
      request.operationId,
      "recovery_dialog_cancelled",
    );
  }
  host.projectOpenRecoveryDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.projectStatus = t("project-controller-open-cancelled-recovery-kept");
  host.setGlobalStatus(t("project-controller-recovery-kept"), "restored");
}

export async function continueProjectOpenWithRecoveryAbandonment(
  host: ProjectTransitionHost,
  requestId: string,
) {
  const request = host.projectOpenRecoveryDecisionRequest;
  if (!request || request.id !== requestId) {
    throw new Error(t("project-controller-open-recovery-decision-stale"));
  }
  const decision = projectOpenRecoveryAbandonDecision(request);
  host.projectOpenRecoveryDecisionRequest = null;
  host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
  host.projectStatus = t("project-controller-opening-without-recovery");
  host.setGlobalStatus(t("project-controller-abandoning-recovery"), "saving");
  try {
    await openProjectRoot(host, request.targetRoot, {
      operatorDecisionId: request.operatorDecisionId,
      recoveryDecision: decision,
      inspection: request.inspection,
    });
  } catch (error) {
    const message = t("project-controller-open-after-recovery-failed", {
      message: errorMessage(error),
    });
    host.projectStatus = message;
    host.escalateGlobalStatus({
      id: PROJECT_OPEN_RECOVERY_NOTIFICATION_ID,
      level: "error",
      title: t("project-controller-recovery-apply-failed-title"),
      message,
      statusMessage: message,
    });
    throw error;
  }
}

export async function closeCurrentProject(
  host: ProjectCloseTransitionHost,
  options: {
    operatorDecisionId?: string | null;
    detachedProjectRoot?: string | null;
    leaseOwner?: ProjectTransitionFrontendLeaseOwner;
  } = {},
) {
  const detachedProjectRoot = host.scannedProject ? null : options.detachedProjectRoot?.trim() || null;
  const projectRoot = host.scannedProject?.root ?? detachedProjectRoot;
  if (!projectRoot) return false;
  return await host.runWithProjectTransitionFrontendLease(
    {
      kind: "close",
      owner: options.leaseOwner ?? "project-transition-controller",
    },
    async (lease) => {
      if (host.scannedProject) {
        await flushProjectDraftsBeforeTransition();
        host.requireProjectTransitionFrontendLease(lease);
      }
      const transitionAllowed = await prepareProjectTransitionForTarget(
        host,
        projectRoot,
        { kind: "close_project" },
        options.operatorDecisionId ?? null,
        () => host.requireProjectTransitionFrontendLease(lease),
      );
      host.requireProjectTransitionFrontendLease(lease);
      if (!transitionAllowed) return false;
      await host.invalidateExternalReconcileForProjectTransition();
      host.requireProjectTransitionFrontendLease(lease);

      host.projectStatus = t("project-controller-closing");
      host.setGlobalStatus(t("project-controller-closing"), "saving");
      try {
        await closeProject(options.operatorDecisionId ?? undefined);
        host.requireProjectTransitionFrontendLease(lease);
        const lifecycle = await readProjectLifecycle();
        host.requireProjectTransitionFrontendLease(lease);
        host.projectLifecycle = lifecycle;
        host.resetProjectSessionProjection({ invalidateHistory: true });
        host.scannedProject = null;
        host.projectStatus = t("project-controller-no-project");
        host.projectOpenRecoveryDecisionRequest = null;
        host.projectTransitionDecisionRequest = null;
        host.clearNotification(PROJECT_OPEN_RECOVERY_NOTIFICATION_ID);
        host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
        host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
        host.clearNotification("project.preview.warning");
        host.clearNotification("project.not-zola");
        host.setGlobalStatus(t("project-controller-closed"), "restored");
        return true;
      } catch (error) {
        host.requireProjectTransitionFrontendLease(lease);
        const message = t("project-controller-close-failed", {
          message: errorMessage(error),
        });
        host.projectStatus = message;
        host.setGlobalStatus(message, "error");
        if (detachedProjectRoot) throw error;
        return false;
      }
    },
  );
}

async function reloadCurrentProjectFromDisk(
  host: ProjectTransitionHost,
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
      message: t("project-controller-reload-no-project"),
    };
  }
  const projectRoot = host.scannedProject.root;
  const isDiscard = options.mode === "discard";
  const transition = await host.runWithProjectTransitionFrontendLease(
    { kind: "reload", owner: "project-transition-controller" },
    async (lease) => {
      await flushProjectDraftsBeforeTransition();
      host.requireProjectTransitionFrontendLease(lease);
      const transitionAllowed = await prepareProjectTransitionForTarget(
        host,
        projectRoot,
        {
          kind: "reload_project",
          mode: options.mode,
          preferredRelativePath,
        },
        options.operatorDecisionId ?? null,
        () => host.requireProjectTransitionFrontendLease(lease),
      );
      host.requireProjectTransitionFrontendLease(lease);
      if (!transitionAllowed) {
        return {
          authorized: false as const,
          attachmentFailure: null,
          previewIdentity: null,
          publishedProjectSessionId: null,
        };
      }
      await host.invalidateExternalReconcileForProjectTransition();
      host.requireProjectTransitionFrontendLease(lease);

      host.projectStatus = isDiscard
        ? t("project-controller-reload-discarding")
        : t("project-controller-reload-purging");
      host.setGlobalStatus(
        isDiscard
          ? t("project-controller-reload-restoring-disk")
          : t("project-controller-reload-rebuilding"),
        "saving",
      );
      let rustSessionSwapped = false;
      let publishedProjectSessionId: string | null = null;
      let previewIdentity: FrontendProjectAttachment | null = null;
      let attachmentFailure: string | null = null;
      try {
        const startup = await inspectStartupFolder(projectRoot);
        host.requireProjectTransitionFrontendLease(lease);
        host.startupFlow = startup;
        const candidate = startup.candidate;
        if (candidate?.kind !== "valid_project") {
          throw new Error(
            candidate?.diagnostics[0]?.message ?? "Proiectul nu mai este un proiect Zola valid.",
          );
        }
        const inspection = await inspectProjectOpen(projectRoot, candidate.snapshotToken);
        host.requireProjectTransitionFrontendLease(lease);
        const bootstrap = await openProject(
          projectRoot,
          inspection.operationId,
          inspection.candidateToken,
          options.operatorDecisionId ?? undefined,
        );
        host.requireProjectTransitionFrontendLease(lease);
        const openedProject = bootstrap.project;
        host.projectLifecycle = bootstrap.lifecycle;
        rustSessionSwapped = true;
        publishedProjectSessionId = openedProject.kernelSessionId ?? null;
        previewIdentity = await host.attachPublishedProjectSession(
          openedProject,
          "reload",
          bootstrap,
          lease,
        );
        host.requireProjectTransitionFrontendLease(lease);
        if (isDiscard) {
          host.diskState = markDiskMutation(host.diskState, "discard", preferredRelativePath);
        }
        host.projectTransitionDecisionRequest = null;
        host.clearNotification(PROJECT_TRANSITION_CONFIRM_NOTIFICATION_ID);
        host.clearNotification(PROJECT_TRANSITION_BLOCKED_NOTIFICATION_ID);
      } catch (error) {
        attachmentFailure = errorMessage(error);
        const lifecycle = await readProjectLifecycle().catch(() => host.projectLifecycle);
        host.requireProjectTransitionFrontendLease(lease);
        host.projectLifecycle = lifecycle;
        if (rustSessionSwapped) {
          await degradeFrontendAttachment(host, attachmentFailure, lease);
          host.requireProjectTransitionFrontendLease(lease);
          host.markWorkspaceProjectionRecoveryRequired(
            t("project-controller-reload-projection-incomplete"),
          );
        }
        const message = isDiscard
          ? t("project-controller-reload-discard-failed", { message: attachmentFailure })
          : t("project-controller-reload-purge-failed", { message: attachmentFailure });
        host.projectStatus = message;
        host.setGlobalStatus(message, "error");
      }
      return {
        authorized: true as const,
        attachmentFailure,
        previewIdentity,
        publishedProjectSessionId,
      };
    },
  );

  if (!transition.authorized) {
    return {
      status: "cancelled",
      projectSessionId: null,
      message: t("project-controller-reload-not-authorized"),
    };
  }
  const {
    attachmentFailure,
    previewIdentity,
    publishedProjectSessionId,
  } = transition;

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
      message: t("project-controller-reload-preview-missing"),
    };
  }

  const previewOutcome = await host.startAttachedProjectPreview(previewIdentity);
  if (previewOutcome.status === "stale") {
    const message = t("project-controller-reload-preview-superseded");
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
        ? t("project-controller-reload-discard-complete")
        : t("project-controller-reload-purge-complete"),
      "restored",
    );
  }
  void host.refreshAttachedProjectSourceGraph(previewIdentity);
  return {
    status: "completed",
    projectSessionId: previewOutcome.projectSessionId,
    previewStatus: previewOutcome.status,
    message: previewOutcome.status === "degraded" ? previewOutcome.message : null,
  };
}

export async function discardSessionAndReloadFromDisk(
  host: ProjectTransitionHost,
  preferredRelativePath: string | null = host.activeScannedPath,
) {
  return await reloadCurrentProjectFromDisk(host, preferredRelativePath, { mode: "discard" });
}
