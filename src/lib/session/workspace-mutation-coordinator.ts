import {
  flushRegisteredEditDrafts,
  hasPendingRegisteredEditDrafts,
  type EditFlushReason,
} from "$lib/session/edit-flush-registry";
import {
  flushFileBufferDraftSync,
  hasPendingFileBufferDraftSync,
  hashFileBufferText,
  rebaseFileBufferDraftSyncProjection,
} from "$lib/session/file-buffer-draft-sync";
import {
  flushPageJsDraftSync,
  hasPendingPageJsDraftSync,
} from "$lib/session/page-js-draft-sync";
import {
  projectLatestProjectWorkspacePreview,
  type ProjectWorkspacePreviewHost,
  type ProjectWorkspacePreviewProjectionOutcome,
} from "$lib/kernel/project-workspace-preview-coordinator";
import type {
  CanvasProjectionPlan,
} from "$lib/contracts/canvas-projection";
import type { PreviewRefreshReason } from "$lib/preview/controlled";
import type {
  ProjectWorkspaceMutationReceipt,
  ProjectWorkspaceSnapshot,
  WorkspaceDocumentProjection,
} from "$lib/project/workspace-contract";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { PROJECT_WORKSPACE_SCHEMA_VERSION } from "$lib/project/workspace-contract";
import { scannedCacheKey } from "$lib/project/files";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export type WorkspaceMutationFlushPhase = "editors" | "page-js" | "file-buffer";

export type WorkspaceMutationAuthorityStatus =
  | "rejected"
  | "noop"
  | "committed"
  | "recovery_required";

export type WorkspaceDerivedProjectionStatus =
  | "current"
  | "deferred"
  | "degraded"
  | "superseded";

export type WorkspaceDerivedReconciliationOutcome = {
  workspaceRevision: number;
  topology: WorkspaceDerivedProjectionStatus;
  sourceGraph: WorkspaceDerivedProjectionStatus;
  scss: WorkspaceDerivedProjectionStatus;
  warnings: string[];
};

export type WorkspaceMutationAuthorityReceipt = {
  projectRoot: string;
  runtimeSessionId: string;
  mutation: ProjectWorkspaceMutationReceipt;
  workspace: ProjectWorkspaceSnapshot;
};

export type WorkspaceMutationSettlement = {
  authority: Extract<WorkspaceMutationAuthorityStatus, "noop" | "committed">;
  workspaceRevision: number;
  transactionId: string | null;
  projections: WorkspaceDerivedReconciliationOutcome & {
    preview: WorkspaceDerivedProjectionStatus;
    previewOutcome: ProjectWorkspacePreviewProjectionOutcome | null;
  };
  warnings: string[];
};

export type WorkspaceMutationSettlementHost = ProjectWorkspacePreviewHost & {
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  activeScannedPath: string | null;
  source: string;
  sourceCache: Record<string, string>;
  setGlobalStatus?: (text: string, kind: GlobalStatusKind) => void;
  reconcileWorkspaceDerivedState: (options: {
    expectedProjectRoot: string;
    expectedSessionId: string;
    expectedWorkspaceRevision: number;
    topologyChanged: boolean;
    preferredRelativePath?: string | null;
    refreshSourceGraph?: boolean;
    refreshScss?: boolean;
  }) => Promise<WorkspaceDerivedReconciliationOutcome>;
};

export type WorkspaceMutationSettlementOptions = {
  preferredRelativePath?: string | null;
  refreshSourceGraph?: boolean;
  refreshScss?: boolean;
  projectPreview?: boolean;
  forcePreview?: boolean;
  previewReason?: PreviewRefreshReason;
  warningLabel?: string;
  onCanvasPlanPrepared?: (plan: CanvasProjectionPlan) => void;
};

export function workspaceMutationAuthorityReceipt(
  mutation: ProjectWorkspaceMutationReceipt,
  workspace: ProjectWorkspaceSnapshot,
): WorkspaceMutationAuthorityReceipt {
  return {
    projectRoot: workspace.projectRoot,
    runtimeSessionId: workspace.runtimeSessionId,
    mutation,
    workspace,
  };
}

function currentSettlementSession(
  host: Pick<
    WorkspaceMutationSettlementHost,
    "sessionProjectRoot" | "kernelProjectSessionId"
  >,
  receipt: WorkspaceMutationAuthorityReceipt,
) {
  return host.sessionProjectRoot === receipt.projectRoot
    && host.kernelProjectSessionId === receipt.runtimeSessionId;
}

/**
 * Validates the Rust authority boundary before any frontend projection is
 * allowed to observe it. A malformed receipt is a protocol failure; a later
 * projection failure is not.
 */
export function requireWorkspaceMutationAuthorityReceipt(
  receipt: WorkspaceMutationAuthorityReceipt,
): WorkspaceMutationAuthorityReceipt {
  const { mutation, workspace } = receipt;
  const normalizedTouchedFiles = [...new Set(mutation.touchedFiles)].sort();
  const documents = Array.isArray(mutation.documents) ? mutation.documents : [];
  const documentPaths = documents.map((projection) => projection.relativePath);
  if (
    !receipt.projectRoot.trim()
    || !receipt.runtimeSessionId.trim()
    || mutation.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION
    || workspace.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION
    || workspace.projectRoot !== receipt.projectRoot
    || workspace.runtimeSessionId !== receipt.runtimeSessionId
    || mutation.revisionAfter !== workspace.revision
    || mutation.dirty !== workspace.dirty
    || !Number.isSafeInteger(mutation.revisionBefore)
    || !Number.isSafeInteger(mutation.revisionAfter)
    || mutation.revisionBefore < 0
    || mutation.revisionAfter < 0
    || normalizedTouchedFiles.length !== mutation.touchedFiles.length
    || normalizedTouchedFiles.some((path, index) => path !== mutation.touchedFiles[index])
    || !Array.isArray(mutation.documents)
    || documentPaths.length !== mutation.touchedFiles.length
    || documentPaths.some((path, index) => path !== mutation.touchedFiles[index])
    || documents.some((projection) => !validDocumentProjection(projection))
  ) {
    throw new Error(
      t("workspace-mutation-receipt-identity-invalid"),
    );
  }
  if (mutation.changed) {
    if (
      mutation.revisionAfter <= mutation.revisionBefore
      || !mutation.transactionId?.trim()
      || !mutation.entry
      || mutation.entry.transactionId !== mutation.transactionId
      || mutation.entry.documentPaths.length !== mutation.touchedFiles.length
      || mutation.entry.documentPaths.some(
        (path, index) => path !== mutation.touchedFiles[index],
      )
      || mutation.entry.topologyPaths.some(
        (path) => !mutation.touchedFiles.includes(path),
      )
    ) {
      throw new Error(
        t("workspace-mutation-transaction-missing"),
      );
    }
  } else if (
    mutation.revisionAfter !== mutation.revisionBefore
    || mutation.transactionId !== null
    || mutation.entry !== null
    || mutation.touchedFiles.length !== 0
    || mutation.documents.length !== 0
  ) {
    throw new Error(
      t("workspace-mutation-noop-invalid"),
    );
  }
  return receipt;
}

function validDocumentProjection(projection: WorkspaceDocumentProjection) {
  const snapshot = projection.snapshot;
  if (!projection.relativePath.trim()) return false;
  if (!snapshot) return true;
  return snapshot.relativePath === projection.relativePath
    && snapshot.hash === hashFileBufferText(snapshot.text)
    && snapshot.bytes === new TextEncoder().encode(snapshot.text).byteLength
    && Number.isSafeInteger(snapshot.revision)
    && snapshot.revision >= 0;
}

function projectMutationDocuments(
  host: WorkspaceMutationSettlementHost,
  documents: WorkspaceDocumentProjection[],
) {
  for (const projection of documents) {
    rebaseFileBufferDraftSyncProjection(
      projection.relativePath,
      projection.snapshot,
    );
  }

  const nextCache = { ...host.sourceCache };
  let nextActiveSource: string | null = null;
  for (const projection of documents) {
    const cacheKey = scannedCacheKey({ relativePath: projection.relativePath });
    if (projection.snapshot) {
      nextCache[cacheKey] = projection.snapshot.text;
      if (host.activeScannedPath === projection.relativePath) {
        nextActiveSource = projection.snapshot.text;
      }
    } else {
      delete nextCache[cacheKey];
      if (host.activeScannedPath === projection.relativePath) {
        nextActiveSource = "";
      }
    }
  }
  host.sourceCache = nextCache;
  if (nextActiveSource !== null) host.source = nextActiveSource;
}

function previewProjectionStatus(
  outcome: ProjectWorkspacePreviewProjectionOutcome,
): WorkspaceDerivedProjectionStatus {
  if (outcome.status === "deferred") return "deferred";
  if (outcome.status === "superseded") return "superseded";
  return "current";
}

function emptyDerivedProjection(
  workspaceRevision: number,
  status: WorkspaceDerivedProjectionStatus,
): WorkspaceDerivedReconciliationOutcome {
  return {
    workspaceRevision,
    topology: status,
    sourceGraph: status,
    scss: status,
    warnings: [],
  };
}

/**
 * Publishes one already-authoritative Rust mutation and reconciles its derived
 * frontend views. Once this function accepts `committed`, no projection error
 * can change the authority result or replay the mutation.
 */
export async function settleProjectWorkspaceMutation(
  host: WorkspaceMutationSettlementHost,
  authorityReceipt: WorkspaceMutationAuthorityReceipt,
  options: WorkspaceMutationSettlementOptions = {},
): Promise<WorkspaceMutationSettlement> {
  const receipt = requireWorkspaceMutationAuthorityReceipt(authorityReceipt);
  const authority = receipt.mutation.changed ? "committed" : "noop";
  const workspaceRevision = receipt.workspace.revision;
  const warnings: string[] = [];
  const warningLabel = options.warningLabel?.trim() || t("workspace-mutation-operation");

  if (!currentSettlementSession(host, receipt)) {
    return {
      authority,
      workspaceRevision,
      transactionId: receipt.mutation.transactionId,
      projections: {
        ...emptyDerivedProjection(workspaceRevision, "superseded"),
        preview: "superseded",
        previewOutcome: {
          status: "superseded",
          workspaceRevision,
        },
      },
      warnings,
    };
  }

  const alreadyPublishedRevision = host.projectWorkspaceSnapshot?.revision ?? -1;
  if (alreadyPublishedRevision > workspaceRevision) {
    return {
      authority,
      workspaceRevision,
      transactionId: receipt.mutation.transactionId,
      projections: {
        ...emptyDerivedProjection(workspaceRevision, "superseded"),
        preview: "superseded",
        previewOutcome: {
          status: "superseded",
          workspaceRevision: alreadyPublishedRevision,
        },
      },
      warnings,
    };
  }

  // The Rust snapshot is visible immediately. Everything below is replaceable
  // derived state and may legitimately lag behind this authority revision.
  host.projectWorkspaceSnapshot = receipt.workspace;
  try {
    projectMutationDocuments(host, receipt.mutation.documents);
  } catch (error) {
    warnings.push(
      t("workspace-mutation-editor-projection-failed", {
        operation: warningLabel,
        message: errorMessage(error),
      }),
    );
  }

  if (!receipt.mutation.changed) {
    return {
      authority,
      workspaceRevision,
      transactionId: null,
      projections: {
        ...emptyDerivedProjection(workspaceRevision, "current"),
        preview: "current",
        previewOutcome: { status: "already_current", workspaceRevision },
      },
      warnings,
    };
  }

  const shouldRefreshSourceGraph = options.refreshSourceGraph ?? true;

  // Topology/SCSS inventories can consume the immutable Rust revision while
  // Preview builds the canonical ProjectModel. SourceGraph is intentionally
  // excluded from this phase: it consumes that ProjectModel and is reconciled
  // only after the Preview attempt has settled. When Canvas is unavailable,
  // Rust's SourceGraph projection lazily materializes the same exact revision.
  const derivedTask = (async () => {
    try {
      const derived = await host.reconcileWorkspaceDerivedState({
        expectedProjectRoot: receipt.projectRoot,
        expectedSessionId: receipt.runtimeSessionId,
        expectedWorkspaceRevision: workspaceRevision,
        topologyChanged: (receipt.mutation.entry?.topologyPaths.length ?? 0) > 0,
        preferredRelativePath: options.preferredRelativePath,
        refreshSourceGraph: false,
        refreshScss: options.refreshScss ?? true,
      });
      return { derived, warning: null as string | null };
    } catch (error) {
      const warning = t("workspace-mutation-derived-reconcile-failed", {
        operation: warningLabel,
        message: errorMessage(error),
      });
      return {
        derived: {
          ...emptyDerivedProjection(workspaceRevision, "degraded"),
          warnings: [warning],
        },
        warning,
      };
    }
  })();

  const previewTask = (async () => {
    let preview: WorkspaceDerivedProjectionStatus = options.projectPreview === false
      ? "deferred"
      : "current";
    let previewOutcome: ProjectWorkspacePreviewProjectionOutcome | null = null;
    let warning: string | null = null;
    const publishedWorkspaceRevision =
      host.projectWorkspaceSnapshot?.revision ?? workspaceRevision;
    if (publishedWorkspaceRevision > workspaceRevision) {
      preview = "superseded";
      previewOutcome = {
        status: "superseded",
        workspaceRevision: publishedWorkspaceRevision,
      };
    } else if (options.projectPreview !== false && currentSettlementSession(host, receipt)) {
      try {
        previewOutcome = await projectLatestProjectWorkspacePreview(host, {
          reason: options.previewReason ?? "workspace-mutation",
          minimumWorkspaceRevision: workspaceRevision,
          expectedWorkspaceRevision: workspaceRevision,
          expectedWorkspaceTransactionId: receipt.mutation.transactionId ?? undefined,
          requestedPaths: receipt.mutation.touchedFiles,
          force: options.forcePreview,
          onCanvasPlanPrepared: options.onCanvasPlanPrepared,
        });
        preview = previewProjectionStatus(previewOutcome);
      } catch (error) {
        preview = currentSettlementSession(host, receipt) ? "degraded" : "superseded";
        warning = t("workspace-mutation-preview-resync", {
          operation: warningLabel,
          message: errorMessage(error),
        });
      }
    } else if (!currentSettlementSession(host, receipt)) {
      preview = "superseded";
      previewOutcome = { status: "superseded", workspaceRevision };
    }
    return { preview, previewOutcome, warning };
  })();

  const [{ derived }, previewResult] = await Promise.all([
    derivedTask,
    previewTask,
  ]);

  if (!shouldRefreshSourceGraph) {
    derived.sourceGraph = "deferred";
  } else if (!currentSettlementSession(host, receipt)) {
    derived.sourceGraph = "superseded";
  } else {
    try {
      const sourceGraphProjection = await host.reconcileWorkspaceDerivedState({
        expectedProjectRoot: receipt.projectRoot,
        expectedSessionId: receipt.runtimeSessionId,
        expectedWorkspaceRevision: workspaceRevision,
        topologyChanged: false,
        preferredRelativePath: options.preferredRelativePath,
        refreshSourceGraph: true,
        refreshScss: false,
      });
      derived.sourceGraph = sourceGraphProjection.sourceGraph;
      derived.warnings.push(...sourceGraphProjection.warnings);
    } catch (error) {
      derived.sourceGraph = currentSettlementSession(host, receipt)
        ? "degraded"
        : "superseded";
      derived.warnings.push(
        t("workspace-mutation-derived-reconcile-failed", {
          operation: warningLabel,
          message: errorMessage(error),
        }),
      );
    }
  }

  warnings.push(...derived.warnings);
  if (previewResult.warning) warnings.push(previewResult.warning);

  return {
    authority,
    workspaceRevision,
    transactionId: receipt.mutation.transactionId,
    projections: {
      ...derived,
      preview: previewResult.preview,
      previewOutcome: previewResult.previewOutcome,
    },
    warnings: [...new Set(warnings)],
  };
}

/**
 * Establishes the single frontend mutation boundary used before Save,
 * history, project transitions and external-disk reconciliation.
 */
export async function flushWorkspaceMutationInputs(
  reason: EditFlushReason,
  options: {
    checkpoint?: (phase: WorkspaceMutationFlushPhase) => void;
  } = {},
) {
  if (hasPendingRegisteredEditDrafts()) {
    await flushRegisteredEditDrafts(reason);
  }
  options.checkpoint?.("editors");
  if (hasPendingPageJsDraftSync()) {
    await flushPageJsDraftSync({ throwOnFailure: true });
  }
  options.checkpoint?.("page-js");
  if (hasPendingFileBufferDraftSync()) {
    await flushFileBufferDraftSync({ throwOnFailure: true });
  }
  options.checkpoint?.("file-buffer");
}
