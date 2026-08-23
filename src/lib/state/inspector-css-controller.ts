import type { ScssVariable } from "$lib/css/contracts";
import {
  createCssRequestIdentity,
  createDesignClass,
  createScssVariable,
  getScssVariables,
  renameDesignClass,
  setScssVariable,
} from "$lib/css/io";
import type { CssMutationAuthorityReceipt } from "$lib/css/mutation-contract";
import { t } from "$lib/i18n/runtime.svelte";
import {
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import { scannedCacheKey } from "$lib/project/files";
import {
  readProjectWorkspaceState,
} from "$lib/project/io/workspace";
import {
  flushFileBufferDraftSync,
  rebaseFileBufferDraftSyncProjection,
} from "$lib/session/file-buffer-draft-sync";
import {
  type WorkspaceMutationAuthorityReceipt,
  type WorkspaceMutationSettlement,
  type WorkspaceMutationSettlementOptions,
} from "$lib/session/workspace-mutation-coordinator";
import {
  bindInspectorLiveCssTransaction,
  captureInspectorLiveCssIdentity,
  clearInspectorLiveProperties,
  type InspectorLiveCssIdentity,
  type PreviewLiveControllerHost,
} from "$lib/state/preview-live-controller";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { ProjectWorkspaceSnapshot } from "$lib/project/workspace-contract";
import { errorMessage } from "$lib/util";

export type InspectorCssControllerHost = {
  context: () => Readonly<{
    projectRoot: string;
    runtimeSessionId: string;
    workspace: ProjectWorkspaceSnapshot | null;
    activeScannedPath: string | null;
  }>;
  acceptWorkspace: (workspace: ProjectWorkspaceSnapshot) => void;
  source: { source: string; sourceCache: Record<string, string> };
  scssVariables: () => ScssVariable[];
  acceptScssVariables: (variables: ScssVariable[]) => void;
  previewLive: PreviewLiveControllerHost;
  runStructural: <T>(
    operation: (lease: PreviewStructuralSessionLease) => Promise<T>,
  ) => Promise<T | null>;
  requireStructural: (lease: PreviewStructuralSessionLease) => void;
  settleMutation: (
    receipt: WorkspaceMutationAuthorityReceipt,
    options?: WorkspaceMutationSettlementOptions,
  ) => Promise<WorkspaceMutationSettlement>;
  notifyCssSourceChanged: () => void;
  refreshDesignClassInventory: (force?: boolean) => Promise<unknown>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
};

function currentCssSession(
  host: InspectorCssControllerHost,
  projectRoot: string,
  runtimeSessionId: string,
) {
  const context = host.context();
  return context.projectRoot === projectRoot
    && context.runtimeSessionId === runtimeSessionId;
}

export async function projectCommittedInspectorCssMutation(
  host: InspectorCssControllerHost,
  authority: CssMutationAuthorityReceipt,
  liveEpoch: number | null,
) {
  const { projectRoot, runtimeSessionId: sessionId } = host.context();
  if (authority.projectRoot !== projectRoot || authority.sessionId !== sessionId) {
    throw new Error(t("workbench-css-live-session-mismatch"));
  }
  if (
    authority.schemaVersion !== 2
    || authority.documents.map((projection) => projection.relativePath).join("\u0000")
      !== authority.touchedFiles.join("\u0000")
    || (authority.status === "noop" && authority.documents.length !== 0)
  ) {
    throw new Error(t("workbench-css-documents-mismatch"));
  }

  const mutation = authority.workspaceMutation;
  const transactionId = mutation?.transactionId?.trim() ?? "";
  if (
    authority.status === "staged"
    && (
      !mutation?.changed
      || mutation.revisionBefore !== authority.revisionBefore
      || mutation.revisionAfter !== authority.revisionAfter
      || !transactionId
    )
  ) {
    throw new Error(t("workbench-css-transaction-missing"));
  }

  let localProjectionWarning = "";
  try {
    await flushFileBufferDraftSync({ throwOnFailure: true });
    for (const projection of authority.documents) {
      rebaseFileBufferDraftSyncProjection(projection.relativePath, projection.snapshot);
      const cacheKey = scannedCacheKey({ relativePath: projection.relativePath });
      if (projection.snapshot) {
        host.source.sourceCache = {
          ...host.source.sourceCache,
          [cacheKey]: projection.snapshot.text,
        };
        if (host.context().activeScannedPath === projection.relativePath) {
          host.source.source = projection.snapshot.text;
        }
      } else {
        const nextCache = { ...host.source.sourceCache };
        delete nextCache[cacheKey];
        host.source.sourceCache = nextCache;
        if (host.context().activeScannedPath === projection.relativePath) {
          host.source.source = "";
        }
      }
    }
    if (
      authority.status === "noop"
      || authority.documents.some((projection) => /\.(?:css|scss)$/i.test(projection.relativePath))
    ) {
      host.notifyCssSourceChanged();
    }
  } catch (error) {
    localProjectionWarning = errorMessage(error);
  }

  const draftIdentity = liveEpoch === null
    ? null
    : captureInspectorLiveCssIdentity(host.previewLive, liveEpoch);

  if (authority.status === "noop") {
    if (draftIdentity) clearInspectorLiveProperties(host.previewLive, draftIdentity);
    return;
  }
  if (!mutation) {
    throw new Error(t("workbench-css-staged-mutation-missing"));
  }

  let workspace: ProjectWorkspaceSnapshot;
  try {
    const currentWorkspace = await readProjectWorkspaceState();
    if (
      !currentWorkspace
      || currentWorkspace.projectRoot !== projectRoot
      || currentWorkspace.runtimeSessionId !== sessionId
      || currentWorkspace.revision !== authority.revisionAfter
    ) {
      throw new Error(t("workbench-css-revision-unconfirmed"));
    }
    workspace = currentWorkspace;
    host.acceptWorkspace(currentWorkspace);
  } catch (error) {
    if (currentCssSession(host, projectRoot, sessionId)) {
      host.setGlobalStatus(
        t("workbench-css-resync", { message: errorMessage(error) }),
        "unsaved",
      );
    }
    if (draftIdentity) clearInspectorLiveProperties(host.previewLive, draftIdentity);
    return;
  }
  if (localProjectionWarning && currentCssSession(host, projectRoot, sessionId)) {
    host.setGlobalStatus(
      t("workbench-css-editor-resync", { message: localProjectionWarning }),
      "unsaved",
    );
  }
  void settleCommittedInspectorCssProjection(
    host,
    projectRoot,
    sessionId,
    transactionId,
    mutation,
    workspace,
    draftIdentity,
  );
}

export async function settleCommittedInspectorCssProjection(
  host: InspectorCssControllerHost,
  projectRoot: string,
  sessionId: string,
  transactionId: string,
  mutation: NonNullable<CssMutationAuthorityReceipt["workspaceMutation"]>,
  workspace: ProjectWorkspaceSnapshot,
  draftIdentity: InspectorLiveCssIdentity | null,
) {
  let boundIdentity: InspectorLiveCssIdentity | null = null;
  const topologyChanged = (mutation.entry?.topologyPaths.length ?? 0) > 0;
  try {
    await host.settleMutation({
      projectRoot,
      runtimeSessionId: sessionId,
      mutation,
      workspace,
    }, {
      warningLabel: "Modificarea CSS",
      refreshSourceGraph: topologyChanged,
      refreshScss: topologyChanged,
      onCanvasPlanPrepared: (plan) => {
        if (plan.workspaceTransactionId !== transactionId) {
          throw new Error(t("workbench-css-canvas-plan-mismatch"));
        }
        if (!draftIdentity) return;
        boundIdentity = bindInspectorLiveCssTransaction(host.previewLive, draftIdentity, {
          workspaceRevision: plan.identity.workspaceRevision,
          workspaceTransactionId: transactionId,
          canvasTransactionId: plan.identity.transactionId,
          previewRevision: plan.identity.previewRevision,
        });
      },
    });
  } catch (error) {
    if (currentCssSession(host, projectRoot, sessionId)) {
      host.setGlobalStatus(
        t("workbench-css-resync", { message: errorMessage(error) }),
        "unsaved",
      );
    }
  }
  if (!currentCssSession(host, projectRoot, sessionId)) return;
  const exactIdentity = boundIdentity ?? draftIdentity;
  if (exactIdentity) clearInspectorLiveProperties(host.previewLive, exactIdentity);
}

export async function updateDesignSystemVariable(
  host: InspectorCssControllerHost,
  variable: ScssVariable,
  value: string,
): Promise<boolean> {
  const nextValue = value.trim();
  if (!nextValue || nextValue === variable.value) return false;
  const { projectRoot, runtimeSessionId } = host.context();
  const identity = createCssRequestIdentity(projectRoot, runtimeSessionId);
  const receipt = await setScssVariable(variable.file, variable.name, nextValue, identity);
  if (!currentCssSession(host, projectRoot, runtimeSessionId)) return false;
  await projectCommittedInspectorCssMutation(host, receipt.authority, null);
  if (!currentCssSession(host, projectRoot, runtimeSessionId)) return false;
  const currentVariables = host.scssVariables();
  host.acceptScssVariables(await getScssVariables(
    identity,
    host.context().workspace?.revision,
  ).catch(() => (
    currentVariables.map((entry) => (
      entry.file === variable.file && entry.name === variable.name
        ? { ...entry, value: nextValue }
        : entry
    ))
  )));
  host.setGlobalStatus(t("workbench-token-updated", { name: variable.name }), "unsaved");
  return true;
}

export async function createDesignSystemVariable(
  host: InspectorCssControllerHost,
  relativePath: string,
  name: string,
  value: string,
): Promise<boolean> {
  const { projectRoot, runtimeSessionId } = host.context();
  const identity = createCssRequestIdentity(projectRoot, runtimeSessionId);
  const receipt = await createScssVariable(relativePath, name, value, identity);
  if (!currentCssSession(host, projectRoot, runtimeSessionId)) return false;
  await projectCommittedInspectorCssMutation(host, receipt.authority, null);
  if (!currentCssSession(host, projectRoot, runtimeSessionId)) return false;
  let scssProjectionCurrent = true;
  const currentVariables = host.scssVariables();
  host.acceptScssVariables(await getScssVariables(
    identity,
    host.context().workspace?.revision,
  ).catch(() => {
    scssProjectionCurrent = false;
    return currentVariables;
  }));
  host.setGlobalStatus(
    scssProjectionCurrent
      ? t("workbench-token-created", { name: name.replace(/^\$/, "") })
      : t("workbench-token-created-resync", { name: name.replace(/^\$/, "") }),
    "unsaved",
  );
  return true;
}

export async function createDesignSystemClass(
  host: InspectorCssControllerHost,
  name: string,
  relativePath: string,
): Promise<boolean> {
  const outcome = await host.runStructural(async (lease) => {
    const receipt = await createDesignClass(name, relativePath, {
      expectedProjectRoot: lease.projectRoot,
      expectedSessionId: lease.sessionId,
    });
    host.requireStructural(lease);
    const settlement = await host.settleMutation(receipt, {
      preferredRelativePath: relativePath,
      warningLabel: t("workbench-class-create-operation"),
    });
    host.requireStructural(lease);
    try {
      await host.refreshDesignClassInventory(true);
    } catch (error) {
      settlement.warnings.push(
        t("workbench-class-inventory-resync", { message: errorMessage(error) }),
      );
    }
    host.requireStructural(lease);
    host.setGlobalStatus(
      settlement.warnings.length > 0
        ? t("workbench-class-created-resync", {
            name: name.replace(/^\./, ""),
            path: relativePath,
          })
        : t("workbench-class-created", {
            name: name.replace(/^\./, ""),
            path: relativePath,
          }),
      "unsaved",
    );
    return true;
  });
  return outcome ?? false;
}

export async function renameDesignSystemClass(
  host: InspectorCssControllerHost,
  oldName: string,
  newName: string,
): Promise<boolean> {
  const outcome = await host.runStructural(async (lease) => {
    const receipt = await renameDesignClass(oldName, newName, {
      expectedProjectRoot: lease.projectRoot,
      expectedSessionId: lease.sessionId,
    });
    host.requireStructural(lease);
    const settlement = await host.settleMutation(receipt.workspace, {
      preferredRelativePath: host.context().activeScannedPath,
      warningLabel: t("workbench-class-rename-operation"),
    });
    host.requireStructural(lease);
    try {
      await host.refreshDesignClassInventory(true);
    } catch (error) {
      settlement.warnings.push(
        t("workbench-class-inventory-resync", { message: errorMessage(error) }),
      );
    }
    host.requireStructural(lease);
    host.setGlobalStatus(
      settlement.warnings.length > 0
        ? t("workbench-class-renamed-resync", {
            oldName: receipt.oldName,
            newName: receipt.newName,
          })
        : t("workbench-class-renamed", {
            oldName: receipt.oldName,
            newName: receipt.newName,
            files: receipt.changedFiles.length,
            references: receipt.replacementCount,
          }),
      "unsaved",
    );
    return true;
  });
  return outcome ?? false;
}
