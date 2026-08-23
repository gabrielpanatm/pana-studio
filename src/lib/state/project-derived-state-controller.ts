import { createCssRequestIdentity, getScssVariables } from "$lib/css/io";
import type { ScssVariable } from "$lib/css/contracts";
import {
  type CommittedHistoryProjectionContext,
  type ReconcileWorkspaceDerivedStateOptions,
} from "$lib/project/controller-contracts";
import {
  scanProject,
} from "$lib/project/io/startup";
import {
  planOpenedProject,
  preservePreviewBaseUrl,
  selectProjectFileAfterScan,
} from "$lib/project/session";
import { diskStateFromProjectScan, type DiskState } from "$lib/session/disk-state";
import {
  flushWorkspaceMutationInputs,
  type WorkspaceDerivedReconciliationOutcome,
} from "$lib/session/workspace-mutation-coordinator";
import type {
  ProjectTransitionFrontendLease,
  ProjectTransitionFrontendLeaseRequest,
} from "$lib/state/project-transition-frontend-lease";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type {
  ProjectFile,
  ProjectScan,
} from "$lib/project/lifecycle-contract";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

export type ProjectDerivedStateHost = {
  activeScannedPath: string | null;
  diskState: DiskState;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
  projectStatus: string;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  refreshToken: number;
  scannedProject: ProjectScan | null;
  scssVariables: ScssVariable[];
  sessionProjectRoot: string;
  loadScannedProjectFile: (
    file: ProjectFile,
    options?: {
      strict?: boolean;
      skipDraftFlush?: boolean;
      deferPreviewRefresh?: boolean;
    },
  ) => Promise<void>;
  refreshSourceGraph: (options?: { strict?: boolean }) => Promise<void>;
  requestPreviewRefresh: (reason: "project-rescan") => Promise<boolean>;
  requireProjectTransitionFrontendLease: (lease: ProjectTransitionFrontendLease) => void;
  runWithProjectTransitionFrontendLease: <T>(
    request: ProjectTransitionFrontendLeaseRequest,
    operation: (lease: ProjectTransitionFrontendLease) => Promise<T>,
  ) => Promise<T>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  startExternalDiskMonitoring: () => void;
};

export async function rescanCurrentProject(
  host: ProjectDerivedStateHost,
  preferredRelativePath: string | null = host.activeScannedPath,
  options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
) {
  if (!host.scannedProject) return;
  await host.runWithProjectTransitionFrontendLease(
    { kind: "rescan", owner: "project-transition-controller" },
    async (lease) => {
      const requireCurrent = () => host.requireProjectTransitionFrontendLease(lease);
      await flushWorkspaceMutationInputs("manual");
      requireCurrent();
      await projectCurrentProjectRescan(
        host,
        preferredRelativePath,
        options,
        requireCurrent,
      );
      requireCurrent();
    },
  );
}

function derivedProjectionOutcome(
  workspaceRevision: number,
): WorkspaceDerivedReconciliationOutcome {
  return {
    workspaceRevision,
    topology: "current",
    sourceGraph: "current",
    scss: "current",
    warnings: [],
  };
}

function derivedProjectionSessionStatus(
  host: ProjectDerivedStateHost,
  options: Pick<
    ReconcileWorkspaceDerivedStateOptions,
    "expectedProjectRoot" | "expectedSessionId" | "expectedWorkspaceRevision"
  >,
): "current" | "superseded" {
  if (
    host.sessionProjectRoot !== options.expectedProjectRoot
    || host.kernelProjectSessionId !== options.expectedSessionId
  ) {
    return "superseded";
  }
  const revision = host.projectWorkspaceSnapshot?.revision;
  if (revision === undefined || revision === options.expectedWorkspaceRevision) return "current";
  if (revision > options.expectedWorkspaceRevision) return "superseded";
  throw new Error(
    t("project-controller-derived-revision-mismatch", {
      expected: options.expectedWorkspaceRevision,
      actual: revision,
    }),
  );
}

function supersedeDerivedProjection(
  outcome: WorkspaceDerivedReconciliationOutcome,
) {
  outcome.topology = "superseded";
  outcome.sourceGraph = "superseded";
  outcome.scss = "superseded";
  return outcome;
}

type WorkspaceDerivedReconciliationRequest = {
  host: ProjectDerivedStateHost;
  options: ReconcileWorkspaceDerivedStateOptions;
  resolve: (outcome: WorkspaceDerivedReconciliationOutcome) => void;
  reject: (error: unknown) => void;
};

type WorkspaceDerivedReconciliationLane = {
  activeOptions: ReconcileWorkspaceDerivedStateOptions | null;
  pending: WorkspaceDerivedReconciliationRequest | null;
  running: boolean;
};

const workspaceDerivedReconciliationLanes = new Map<
  string,
  WorkspaceDerivedReconciliationLane
>();

function workspaceDerivedReconciliationKey(
  options: ReconcileWorkspaceDerivedStateOptions,
) {
  return `${options.expectedProjectRoot}\u0000${options.expectedSessionId}`;
}

function supersededDerivedProjectionFor(
  options: ReconcileWorkspaceDerivedStateOptions,
) {
  return supersedeDerivedProjection(
    derivedProjectionOutcome(options.expectedWorkspaceRevision),
  );
}

function mergeWorkspaceDerivedReconciliationOptions(
  previous: ReconcileWorkspaceDerivedStateOptions | null,
  next: ReconcileWorkspaceDerivedStateOptions,
): ReconcileWorkspaceDerivedStateOptions {
  if (!previous) return next;
  return {
    ...next,
    topologyChanged: previous.topologyChanged || next.topologyChanged,
    refreshSourceGraph: (previous.refreshSourceGraph ?? true)
      || (next.refreshSourceGraph ?? true),
    refreshScss: (previous.refreshScss ?? true) || (next.refreshScss ?? true),
  };
}

async function drainWorkspaceDerivedReconciliationLane(
  key: string,
  lane: WorkspaceDerivedReconciliationLane,
) {
  if (lane.running) return;
  lane.running = true;
  try {
    while (lane.pending) {
      const request = lane.pending;
      lane.pending = null;
      lane.activeOptions = request.options;
      try {
        request.resolve(
          await runWorkspaceDerivedStateReconciliation(
            request.host,
            request.options,
          ),
        );
      } catch (error) {
        request.reject(error);
      } finally {
        lane.activeOptions = null;
      }
    }
  } finally {
    lane.running = false;
    if (!lane.pending && workspaceDerivedReconciliationLanes.get(key) === lane) {
      workspaceDerivedReconciliationLanes.delete(key);
    } else if (lane.pending) {
      void drainWorkspaceDerivedReconciliationLane(key, lane);
    }
  }
}

export async function reconcileWorkspaceDerivedState(
  host: ProjectDerivedStateHost,
  options: ReconcileWorkspaceDerivedStateOptions,
): Promise<WorkspaceDerivedReconciliationOutcome> {
  const key = workspaceDerivedReconciliationKey(options);
  let lane = workspaceDerivedReconciliationLanes.get(key);
  if (!lane) {
    lane = { activeOptions: null, pending: null, running: false };
    workspaceDerivedReconciliationLanes.set(key, lane);
  }

  if (
    (lane.activeOptions?.expectedWorkspaceRevision ?? -1)
      > options.expectedWorkspaceRevision
    || (lane.pending?.options.expectedWorkspaceRevision ?? -1)
      > options.expectedWorkspaceRevision
  ) {
    return supersededDerivedProjectionFor(options);
  }

  const mergedOptions = mergeWorkspaceDerivedReconciliationOptions(
    lane.pending?.options ?? lane.activeOptions,
    options,
  );
  if (lane.pending) {
    lane.pending.resolve(supersededDerivedProjectionFor(lane.pending.options));
  }

  return new Promise<WorkspaceDerivedReconciliationOutcome>((resolve, reject) => {
    lane!.pending = {
      host,
      options: mergedOptions,
      resolve,
      reject,
    };
    void drainWorkspaceDerivedReconciliationLane(key, lane!);
  });
}

async function runWorkspaceDerivedStateReconciliation(
  host: ProjectDerivedStateHost,
  options: ReconcileWorkspaceDerivedStateOptions,
): Promise<WorkspaceDerivedReconciliationOutcome> {
  const outcome = derivedProjectionOutcome(options.expectedWorkspaceRevision);
  const current = () => (
    derivedProjectionSessionStatus(host, options) === "current"
  );
  if (!current()) return supersedeDerivedProjection(outcome);

  if (options.topologyChanged) {
    const currentProject = host.scannedProject;
    if (!currentProject) {
      outcome.topology = "deferred";
    } else {
      try {
        const scanned = await scanProject(options.expectedProjectRoot);
        if (!current()) return supersedeDerivedProjection(outcome);
        if (
          scanned.root !== options.expectedProjectRoot
          || scanned.kernelSessionId !== options.expectedSessionId
          || typeof scanned.workspaceRevision !== "number"
          || !Number.isSafeInteger(scanned.workspaceRevision)
        ) {
          throw new Error(t("project-controller-scan-revision-identity-missing"));
        }
        if (scanned.workspaceRevision !== options.expectedWorkspaceRevision) {
          if ((scanned.workspaceRevision ?? -1) > options.expectedWorkspaceRevision) {
            return supersedeDerivedProjection(outcome);
          }
          throw new Error(
            t("project-controller-scan-revision-mismatch", {
              actual: scanned.workspaceRevision,
              expected: options.expectedWorkspaceRevision,
            }),
          );
        }
        const project = preservePreviewBaseUrl(scanned, currentProject);
        host.scannedProject = project;
        host.diskState = diskStateFromProjectScan(project, host.diskState);
        host.projectStatus = planOpenedProject(project).projectStatus;
        const nextFile = selectProjectFileAfterScan(
          project,
          options.preferredRelativePath ?? host.activeScannedPath,
        );
        if (nextFile) {
          await host.loadScannedProjectFile(nextFile, {
            strict: true,
            skipDraftFlush: true,
            deferPreviewRefresh: true,
          });
          if (!current()) return supersedeDerivedProjection(outcome);
        }
      } catch (error) {
        outcome.topology = current() ? "degraded" : "superseded";
        outcome.warnings.push(
          t("project-controller-navigation-resync", { message: errorMessage(error) }),
        );
      }
    }
  }

  if (options.refreshSourceGraph ?? true) {
    try {
      await host.refreshSourceGraph({ strict: true });
      if (!current()) return supersedeDerivedProjection(outcome);
    } catch (error) {
      outcome.sourceGraph = current() ? "degraded" : "superseded";
      outcome.warnings.push(
        t("project-controller-source-graph-preserved", { message: errorMessage(error) }),
      );
    }
  }

  if (options.refreshScss ?? true) {
    try {
      const variables = await getScssVariables(
        createCssRequestIdentity(
          options.expectedProjectRoot,
          options.expectedSessionId,
        ),
        options.expectedWorkspaceRevision,
      );
      if (!current()) return supersedeDerivedProjection(outcome);
      host.scssVariables = variables;
    } catch (error) {
      outcome.scss = current() ? "degraded" : "superseded";
      outcome.warnings.push(
        t("project-controller-scss-preserved", { message: errorMessage(error) }),
      );
    }
  }

  host.startExternalDiskMonitoring();
  return outcome;
}

export async function rescanCurrentProjectForCommittedHistory(
  host: ProjectDerivedStateHost,
  context: CommittedHistoryProjectionContext,
  preferredRelativePath: string | null = host.activeScannedPath,
  options: { strict?: boolean; deferPreviewRefresh?: boolean } = {},
) {
  const requireCurrent = () => {
    if (
      host.sessionProjectRoot !== context.projectRoot
      || host.kernelProjectSessionId !== context.runtimeSessionId
      || host.projectSessionEpoch !== context.projectSessionEpoch
      || (
        host.projectWorkspaceSnapshot
        && host.projectWorkspaceSnapshot.revision > context.workspaceRevision
      )
    ) {
      throw new Error(t("project-controller-history-reprojection"));
    }
  };
  requireCurrent();
  await projectCurrentProjectRescan(host, preferredRelativePath, options, requireCurrent);
  requireCurrent();
}

async function projectCurrentProjectRescan(
  host: ProjectDerivedStateHost,
  preferredRelativePath: string | null,
  options: { strict?: boolean; deferPreviewRefresh?: boolean },
  requireProjectionCurrent: (() => void) | undefined = undefined,
) {
  const requireCurrent = () => requireProjectionCurrent?.();
  requireCurrent();
  const currentProject = host.scannedProject;
  if (!currentProject) return;
  const expectedWorkspaceRevision = host.projectWorkspaceSnapshot?.revision ?? 0;
  const reconciliation = await reconcileWorkspaceDerivedState(host, {
    expectedProjectRoot: host.sessionProjectRoot,
    expectedSessionId: host.kernelProjectSessionId,
    expectedWorkspaceRevision,
    topologyChanged: true,
    preferredRelativePath,
    refreshSourceGraph: true,
    refreshScss: true,
  });
  requireCurrent();
  if (
    options.strict
    && (
      reconciliation.topology === "degraded"
      || reconciliation.sourceGraph === "degraded"
      || reconciliation.scss === "degraded"
      || reconciliation.topology === "deferred"
      || reconciliation.sourceGraph === "deferred"
      || reconciliation.scss === "deferred"
      || reconciliation.topology === "superseded"
      || reconciliation.sourceGraph === "superseded"
      || reconciliation.scss === "superseded"
    )
  ) {
    throw new Error(
      reconciliation.warnings.join(" ")
        || t("project-controller-strict-rescan-unconfirmed"),
    );
  }
  if (!options.deferPreviewRefresh) {
    host.refreshToken += 1;
    const previewRefreshed = await host.requestPreviewRefresh("project-rescan");
    requireCurrent();
    if (options.strict && !previewRefreshed) {
      throw new Error(t("project-controller-strict-rescan-preview-unconfirmed"));
    }
  }
  host.startExternalDiskMonitoring();
  host.setGlobalStatus(t("project-controller-structure-rescanned"), "restored");
}
