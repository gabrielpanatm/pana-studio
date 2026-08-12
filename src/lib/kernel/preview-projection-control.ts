import { readProjectWorkspaceState } from "$lib/project/io";
import {
  settleProjectWorkspaceMutation,
  type WorkspaceMutationSettlement,
  type WorkspaceMutationSettlementHost,
} from "$lib/session/workspace-mutation-coordinator";
import {
  type PreviewStructuralSessionHost,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import type {
  PreviewHtmlAttributesExecutionReceipt,
  PreviewHtmlDeleteExecutionReceipt,
  PreviewHtmlDuplicateExecutionReceipt,
  PreviewHtmlInsertDropExecutionReceipt,
  PreviewHtmlTagExecutionReceipt,
  PreviewHtmlTextExecutionReceipt,
  PreviewProjectionDiagnostic,
  PreviewSelectionBatchExecutionReceipt,
  PreviewTeraDeleteExecutionReceipt,
  PreviewTeraInsertDropExecutionReceipt,
  EditorMoveExecutionReceipt,
  CanvasPatch,
  LocalizedDiagnostic,
  ProjectWorkspaceMutationReceipt,
} from "$lib/types";
import { PROJECT_WORKSPACE_SCHEMA_VERSION } from "$lib/types";
import { t } from "$lib/i18n/runtime.svelte";
import { errorMessage } from "$lib/util";

export type PreviewStructuralExecutionReceipt =
  | PreviewHtmlInsertDropExecutionReceipt
  | PreviewHtmlAttributesExecutionReceipt
  | PreviewHtmlTextExecutionReceipt
  | PreviewHtmlTagExecutionReceipt
  | PreviewHtmlDuplicateExecutionReceipt
  | PreviewHtmlDeleteExecutionReceipt
  | PreviewTeraInsertDropExecutionReceipt
  | PreviewTeraDeleteExecutionReceipt;

export type PreviewStructuralPatch = NonNullable<PreviewStructuralExecutionReceipt["patch"]>;

export type CommittedMutationProjectionHost =
  WorkspaceMutationSettlementHost & {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  projectSessionEpoch: number;
  applyCanvasPatchToPreview: (patch: CanvasPatch) => Promise<unknown>;
  rollbackCanvasPatchInPreview: (patch: CanvasPatch) => Promise<unknown>;
};

export type PreviewStructuralCanonicalProjectionHost =
  PreviewStructuralSessionHost & CommittedMutationProjectionHost;

export type CommittedMutationProjectionContext = Readonly<{
  projectRoot: string;
  sessionId: string;
  projectSessionEpoch: number;
  expectedWorkspaceRevision: number | null;
}>;

export function previewStructuralBlockingDiagnostic(
  receipt: Pick<PreviewStructuralExecutionReceipt, "diagnostics">,
): PreviewProjectionDiagnostic | null {
  return receipt.diagnostics.find((diagnostic) => diagnostic.blocking) ?? null;
}

export function requireCommittedPreviewStructuralPatch<TPatch extends PreviewStructuralPatch>(
  receipt: {
    status: "committed" | "blocked";
    messageDiagnostic: LocalizedDiagnostic;
    patch: TPatch | null;
    diagnostics: PreviewProjectionDiagnostic[];
  },
  fallbackMessage: string,
): TPatch {
  if (receipt.status === "committed" && receipt.patch) return receipt.patch;
  const blocking = receipt.diagnostics.find((diagnostic) => diagnostic.blocking);
  throw new Error(
    (blocking ? errorMessage(blocking.diagnostic) : "")
      || errorMessage(receipt.messageDiagnostic)
      || fallbackMessage,
  );
}

/**
 * Projects one already-committed Rust workspace mutation into the immutable
 * Preview generation carrying exactly the mutation's `revisionAfter`.
 * ProjectWorkspace remains authoritative if the derived renderer is
 * temporarily unavailable; no disk acknowledgement or recovery promotion is
 * involved in this path.
 */
export async function projectCommittedPreviewStructuralMutation(
  host: PreviewStructuralCanonicalProjectionHost,
  context: PreviewStructuralSessionLease,
  receipt: Pick<
    PreviewStructuralExecutionReceipt,
    "intent" | "touchedFiles" | "workspaceMutation" | "canvasPatch"
  >,
  patch: { file?: string } | null,
  projectLocalState: () => Promise<void> | void,
): Promise<WorkspaceMutationSettlement> {
  return projectCommittedStructuralMutation(
    host,
    {
      projectRoot: context.projectRoot,
      sessionId: context.sessionId,
      projectSessionEpoch: context.projectSessionEpoch,
      expectedWorkspaceRevision:
        receipt.workspaceMutation?.revisionAfter ?? null,
    },
    receipt.intent,
    receipt,
    patch,
    projectLocalState,
    structuralRefreshReason(receipt),
    "await",
  );
}

export async function projectCommittedPreviewSelectionBatchMutation(
  host: PreviewStructuralCanonicalProjectionHost,
  context: PreviewStructuralSessionLease,
  receipt: PreviewSelectionBatchExecutionReceipt,
  projectLocalState: () => Promise<void> | void = () => {},
): Promise<WorkspaceMutationSettlement> {
  if (receipt.status !== "committed" || !receipt.workspaceMutation) {
    throw new Error(receipt.diagnostics[0] || "Operația batch a fost blocată de kernel.");
  }
  return projectCommittedStructuralMutation(
    host,
    {
      projectRoot: context.projectRoot,
      sessionId: context.sessionId,
      projectSessionEpoch: context.projectSessionEpoch,
      expectedWorkspaceRevision: receipt.workspaceMutation.revisionAfter,
    },
    {
      projectRoot: context.projectRoot,
      runtimeSessionId: context.sessionId,
    },
    {
      touchedFiles: receipt.workspaceMutation.touchedFiles,
      workspaceMutation: receipt.workspaceMutation,
      canvasPatch: receipt.canvasPatch,
    },
    null,
    projectLocalState,
    "html-structural",
    "await",
  );
}

export async function projectCommittedEditorMoveMutation(
  host: CommittedMutationProjectionHost,
  context: CommittedMutationProjectionContext,
  receipt: Pick<
    EditorMoveExecutionReceipt,
    | "operation"
    | "projectRoot"
    | "runtimeSessionId"
    | "touchedFiles"
    | "workspaceMutation"
    | "canvasPatch"
  >,
  projectLocalState: () => Promise<void> | void = () => {},
): Promise<WorkspaceMutationSettlement> {
  return projectCommittedStructuralMutation(
    host,
    context,
    receipt,
    receipt,
    null,
    projectLocalState,
    receipt.canvasPatch ? "html-structural" : "tera-structural",
    "background",
  );
}

async function projectCommittedStructuralMutation(
  host: CommittedMutationProjectionHost,
  context: CommittedMutationProjectionContext,
  receiptIdentity: { projectRoot: string | null; runtimeSessionId: string | null },
  receipt: {
    touchedFiles: string[];
    workspaceMutation: ProjectWorkspaceMutationReceipt | null;
    canvasPatch: CanvasPatch | null;
  },
  patch: { file?: string } | null,
  projectLocalState: () => Promise<void> | void,
  previewReason: "html-structural" | "tera-structural",
  projectionMode: "await" | "background",
): Promise<WorkspaceMutationSettlement> {
  requireCurrentCommittedMutationContext(host, context);
  requireCommittedMutationReceiptIdentity(receiptIdentity, context);
  const mutation = requireWorkspaceMutation(receipt.workspaceMutation);
  if (
    context.expectedWorkspaceRevision !== null
    && mutation.revisionAfter !== context.expectedWorkspaceRevision
  ) {
    throw new Error(t("structural-projection-transition-invalid"));
  }
  let snapshot: NonNullable<Awaited<ReturnType<typeof readProjectWorkspaceState>>>;
  try {
    const candidate = await readProjectWorkspaceState();
    requireCurrentCommittedMutationContext(host, context);
    if (
      !candidate
      || candidate.projectRoot !== context.projectRoot
      || candidate.runtimeSessionId !== context.sessionId
    ) {
      throw new Error(
        t("structural-projection-workspace-snapshot-missing"),
      );
    }
    if (candidate.revision > mutation.revisionAfter) {
      host.projectWorkspaceSnapshot = candidate;
      return supersededCommittedStructuralSettlement(
        mutation,
        candidate.revision,
      );
    }
    if (candidate.revision !== mutation.revisionAfter) {
      throw new Error(
        t("structural-projection-exact-snapshot-missing"),
      );
    }
    snapshot = candidate;
  } catch (error) {
    requireCurrentCommittedMutationContext(host, context);
    const warnings = [
      t("structural-projection-workspace-resync", {
        message: error instanceof Error ? error.message : String(error),
      }),
    ];
    try {
      await projectLocalState();
    } catch (localError) {
      warnings.push(
        t("structural-projection-local-resync", {
          message: localError instanceof Error ? localError.message : String(localError),
        }),
      );
    }
    requireCurrentCommittedMutationContext(host, context);
    const settlement = degradedCommittedStructuralSettlement(mutation, warnings);
    publishStructuralProjectionWarning(host, settlement);
    return settlement;
  }
  host.projectWorkspaceSnapshot = snapshot;

  const localWarnings: string[] = [];
  try {
    await projectLocalState();
  } catch (error) {
    localWarnings.push(
      t("structural-projection-local-resync", {
        message: error instanceof Error ? error.message : String(error),
      }),
    );
  }
  requireCurrentCommittedMutationContext(host, context);

  let canvasPatchApplied = false;
  if (receipt.canvasPatch) {
    try {
      canvasPatchApplied = await applyCommittedCanvasPatch(
        host,
        context,
        receipt.canvasPatch,
        mutation,
        snapshot,
      );
    } catch (error) {
      localWarnings.push(
        t("structural-projection-canvas-patch-skipped", {
          message: error instanceof Error ? error.message : String(error),
        }),
      );
    }
  }
  requireCurrentCommittedMutationContext(host, context);

  const settleCanonicalProjection = async () => {
    const settlement = await settleProjectWorkspaceMutation(host, {
      projectRoot: context.projectRoot,
      runtimeSessionId: context.sessionId,
      mutation,
      workspace: snapshot,
    }, {
      preferredRelativePath: patch?.file ?? receipt.touchedFiles[0] ?? null,
      refreshSourceGraph: true,
      refreshScss: false,
      previewReason,
      warningLabel: t("structural-projection-operation"),
    });
    settlement.warnings.push(...localWarnings);

    if (
      settlement.projections.preview === "degraded"
      && canvasPatchApplied
      && receipt.canvasPatch
    ) {
      try {
        await host.rollbackCanvasPatchInPreview(receipt.canvasPatch);
      } catch (rollbackError) {
        settlement.warnings.push(
          t("structural-projection-canvas-rollback-refused", {
            message: rollbackError instanceof Error ? rollbackError.message : String(rollbackError),
          }),
        );
      }
    }
    publishStructuralProjectionWarning(host, settlement);
    return settlement;
  };
  if (projectionMode === "background") {
    void settleCanonicalProjection().catch((error) => {
      if (
        host.sessionProjectRoot === context.projectRoot
        && host.kernelProjectSessionId === context.sessionId
        && host.projectSessionEpoch === context.projectSessionEpoch
      ) {
        host.setGlobalStatus?.(
          t("structural-projection-workspace-resync", {
            message: error instanceof Error ? error.message : String(error),
          }),
          "unsaved",
        );
      }
    });
    return deferredCommittedStructuralSettlement(mutation, localWarnings);
  }
  return await settleCanonicalProjection();
}

function deferredCommittedStructuralSettlement(
  mutation: ProjectWorkspaceMutationReceipt,
  warnings: string[],
): WorkspaceMutationSettlement {
  return {
    authority: "committed",
    workspaceRevision: mutation.revisionAfter,
    transactionId: mutation.transactionId,
    projections: {
      workspaceRevision: mutation.revisionAfter,
      topology: "deferred",
      sourceGraph: "deferred",
      scss: "deferred",
      preview: "deferred",
      previewOutcome: null,
      warnings: [...new Set(warnings)],
    },
    warnings: [...new Set(warnings)],
  };
}

function degradedCommittedStructuralSettlement(
  mutation: ProjectWorkspaceMutationReceipt,
  warnings: string[],
): WorkspaceMutationSettlement {
  return {
    authority: "committed",
    workspaceRevision: mutation.revisionAfter,
    transactionId: mutation.transactionId,
    projections: {
      workspaceRevision: mutation.revisionAfter,
      topology: "degraded",
      sourceGraph: "degraded",
      scss: "degraded",
      preview: "degraded",
      previewOutcome: null,
      warnings,
    },
    warnings: [...new Set(warnings)],
  };
}

function supersededCommittedStructuralSettlement(
  mutation: ProjectWorkspaceMutationReceipt,
  publishedWorkspaceRevision: number,
): WorkspaceMutationSettlement {
  return {
    authority: "committed",
    workspaceRevision: mutation.revisionAfter,
    transactionId: mutation.transactionId,
    projections: {
      workspaceRevision: mutation.revisionAfter,
      topology: "superseded",
      sourceGraph: "superseded",
      scss: "superseded",
      preview: "superseded",
      previewOutcome: {
        status: "superseded",
        workspaceRevision: publishedWorkspaceRevision,
      },
      warnings: [],
    },
    warnings: [],
  };
}

function publishStructuralProjectionWarning(
  host: CommittedMutationProjectionHost,
  settlement: WorkspaceMutationSettlement,
) {
  if (settlement.warnings.length === 0) return;
  host.setGlobalStatus?.(
    t("structural-projection-committed-resync"),
    "unsaved",
  );
}

async function applyCommittedCanvasPatch(
  host: CommittedMutationProjectionHost,
  context: CommittedMutationProjectionContext,
  patch: CanvasPatch,
  mutation: ProjectWorkspaceMutationReceipt,
  snapshot: NonNullable<Awaited<ReturnType<typeof readProjectWorkspaceState>>>,
) {
  const transactionId = mutation.transactionId?.trim() ?? "";
  if (
    patch.schemaVersion !== 1
    || patch.projectRoot !== context.projectRoot
    || patch.runtimeSessionId !== context.sessionId
    || patch.baseWorkspaceRevision !== mutation.revisionBefore
    || patch.workspaceRevision !== mutation.revisionAfter
    || patch.workspaceTransactionId !== transactionId
  ) {
    throw new Error(t("structural-projection-canvas-patch-mismatch"));
  }
  if (
    snapshot.projectRoot !== context.projectRoot
    || snapshot.runtimeSessionId !== context.sessionId
    || snapshot.revision !== patch.workspaceRevision
    || snapshot.lastProjectionTransactionId !== patch.workspaceTransactionId
  ) {
    // A second kernel mutation (for example a component contract) has already
    // superseded this one. Applying an incomplete fast patch would create a
    // false Canvas, so canonical Zola projection is used directly.
    return false;
  }
  try {
    await host.applyCanvasPatchToPreview(patch);
    return true;
  } catch {
    // Repeated render instances or a changed mounted anchor are legitimate
    // reasons to skip acceleration. The authoritative mutation remains in
    // ProjectWorkspace and will be projected canonically below.
    return false;
  }
}

function requireCurrentCommittedMutationContext(
  host: CommittedMutationProjectionHost,
  context: CommittedMutationProjectionContext,
) {
  if (
    host.sessionProjectRoot !== context.projectRoot
    || host.kernelProjectSessionId !== context.sessionId
    || host.projectSessionEpoch !== context.projectSessionEpoch
  ) {
    throw new Error(t("structural-lane-session-changed"));
  }
}

function requireCommittedMutationReceiptIdentity(
  receipt: { projectRoot: string | null; runtimeSessionId: string | null },
  context: CommittedMutationProjectionContext,
) {
  if (
    receipt.projectRoot !== context.projectRoot
    || receipt.runtimeSessionId !== context.sessionId
  ) {
    throw new Error(t("structural-lane-receipt-session-mismatch"));
  }
}

function requireWorkspaceMutation(
  mutation: ProjectWorkspaceMutationReceipt | null,
): ProjectWorkspaceMutationReceipt {
  const transactionId = mutation?.transactionId?.trim() ?? "";
  if (
    !mutation?.changed
    || mutation.schemaVersion !== PROJECT_WORKSPACE_SCHEMA_VERSION
    || mutation.revisionAfter <= mutation.revisionBefore
    || !transactionId
    || mutation.entry?.transactionId !== transactionId
  ) {
    throw new Error(
      t("structural-projection-transition-invalid"),
    );
  }
  return mutation;
}

function structuralRefreshReason(
  receipt: Pick<PreviewStructuralExecutionReceipt, "intent">,
): "html-structural" | "tera-structural" {
  return receipt.intent.kind?.startsWith("tera_")
    ? "tera-structural"
    : "html-structural";
}
