import {
  blockedAction,
  committedAction,
  editorActionSucceeded,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  readProjectWorkspaceState,
  saveProjectWorkspace,
} from "$lib/project/io/workspace";
import type {
  ProjectWorkspacePreviewProjectionOptions,
  ProjectWorkspacePreviewProjectionOutcome,
} from "$lib/kernel/project-workspace-preview-coordinator";
import {
  invalidateFileBufferDraftSyncCursor,
} from "$lib/session/file-buffer-draft-sync";
import {
  flushWorkspaceMutationInputs,
  type WorkspaceDerivedReconciliationOutcome,
} from "$lib/session/workspace-mutation-coordinator";
import type { DiskState } from "$lib/session/disk-state";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
} from "$lib/canvas/contracts";
import type {
  ProjectWorkspaceSaveReceipt,
  ProjectWorkspaceSnapshot,
} from "$lib/project/workspace-contract";
import type { GlobalStatusKind } from "$lib/status/global-status";
import type { HtmlDraftSessionController } from "$lib/state/html-draft-session.svelte";
import { errorMessage, isRecoveryRequiredError } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

/**
 * Frontend projection needed to present a ProjectWorkspace Save.
 *
 * None of these fields decides what reaches disk. The exact Rust workspace
 * revision captured after every editor flush is the sole Save authority.
 */
export type SaveControllerHost = {
  context: () => Readonly<{
    projectRoot: string;
    runtimeSessionId: string;
    editorMutationEpoch: number;
    workspace: ProjectWorkspaceSnapshot | null;
    diskState: DiskState;
    activeScannedPath: string | null;
  }>;
  incrementSaveRequest: () => void;
  acceptWorkspace: (workspace: ProjectWorkspaceSnapshot) => void;
  markDiskSaved: (activeScannedPath: string | null) => void;
  bumpRefreshTokens: () => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  html: {
    inspectorPending: Record<InspectorPendingArea, boolean>;
    pending: Record<HtmlPendingArea, boolean>;
    pendingTag: string | null;
    setInspectorPending: (area: InspectorPendingArea, pending: boolean) => void;
    applyTagChange: () => Promise<EditorActionOutcome>;
    applyClasses: () => Promise<EditorActionOutcome>;
    draft: Pick<HtmlDraftSessionController, "applyAttributes" | "applyText">;
    applyImageSource: (src?: string) => Promise<EditorActionOutcome>;
  };
  reconcileWorkspaceDerivedState: (options: {
    expectedProjectRoot: string;
    expectedSessionId: string;
    expectedWorkspaceRevision: number;
    topologyChanged: boolean;
    preferredRelativePath?: string | null;
    refreshSourceGraph?: boolean;
    refreshScss?: boolean;
  }) => Promise<WorkspaceDerivedReconciliationOutcome>;
  projectLatestPreview: (
    options: ProjectWorkspacePreviewProjectionOptions<"after-save">,
  ) => Promise<ProjectWorkspacePreviewProjectionOutcome>;
  markPreviewSavedToDisk?: (message?: string) => void;
  scheduleZolaValidation?: (reason?: "save") => void;
  acceptProjectWorkspaceSaveBaseline: (
    acceptedManifest: ProjectWorkspaceSaveReceipt["acceptedManifest"],
    diskGeneration: number,
  ) => void;
};

type SaveSessionIdentity = {
  expectedProjectRoot: string;
  expectedSessionId: string;
};

type SaveSettlementReceipt = Pick<
  ProjectWorkspaceSaveReceipt,
  "status" | "writtenFiles" | "removedFiles" | "revisionAfter"
>;

function captureSaveSession(host: SaveControllerHost): SaveSessionIdentity {
  const context = host.context();
  const identity = {
    expectedProjectRoot: context.projectRoot.trim(),
    expectedSessionId: context.runtimeSessionId.trim(),
  };
  if (!identity.expectedProjectRoot || !identity.expectedSessionId) {
    throw new Error(t("save-controller-session-required"));
  }
  return identity;
}

function requireCurrentSaveSession(
  host: SaveControllerHost,
  identity: SaveSessionIdentity,
  operation: string,
) {
  const context = host.context();
  if (
    context.projectRoot !== identity.expectedProjectRoot
    || context.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("save-controller-session-changed", { operation }));
  }
}

function requireWorkspaceSnapshot(
  snapshot: ProjectWorkspaceSnapshot | null,
  identity: SaveSessionIdentity,
): ProjectWorkspaceSnapshot {
  if (!snapshot) throw new Error(t("save-controller-session-uninitialized"));
  if (
    snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("save-controller-snapshot-session-mismatch"));
  }
  if (!Number.isSafeInteger(snapshot.revision) || snapshot.revision < 0) {
    throw new Error(t("save-controller-revision-invalid"));
  }
  return snapshot;
}

function requireSaveReceipt(
  receipt: ProjectWorkspaceSaveReceipt,
  before: ProjectWorkspaceSnapshot,
  identity: SaveSessionIdentity,
) {
  if (
    receipt.projectRoot !== identity.expectedProjectRoot
    || receipt.runtimeSessionId !== identity.expectedSessionId
    || receipt.workspace.projectRoot !== identity.expectedProjectRoot
    || receipt.workspace.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error(t("save-controller-receipt-session-mismatch"));
  }
  if (
    receipt.revisionBefore !== before.revision
    || receipt.revisionAfter !== receipt.workspace.revision
    || receipt.diskGenerationBefore !== before.diskGeneration
    || receipt.diskGenerationAfter !== receipt.workspace.diskGeneration
  ) {
    throw new Error(t("save-controller-receipt-reservation-mismatch"));
  }
  if (receipt.workspace.dirty) {
    throw new Error(t("save-controller-receipt-still-dirty"));
  }
  if (
    receipt.acceptedManifest.root !== identity.expectedProjectRoot
    || receipt.acceptedManifest.truncated
    || !Number.isSafeInteger(receipt.diskGenerationAfter)
    || receipt.diskGenerationAfter < 1
  ) {
    throw new Error(t("save-controller-manifest-invalid"));
  }
  if (receipt.status === "noop" && before.dirty) {
    throw new Error(t("save-controller-operations-missing"));
  }
}

async function flushAllEditorDrafts(host: SaveControllerHost, identity: SaveSessionIdentity) {
  await flushWorkspaceMutationInputs("save", {
    checkpoint: (phase) => {
      const label = phase === "editors"
        ? t("save-controller-operation-draft-flush")
        : phase === "page-js"
          ? t("save-controller-operation-page-js-flush")
          : t("save-controller-operation-file-buffer-flush");
      requireCurrentSaveSession(host, identity, label);
    },
  });
}

export async function savePendingHtmlChanges(
  host: SaveControllerHost,
): Promise<EditorActionOutcome> {
  let committed = false;
  const apply = async (
    area: HtmlPendingArea,
    pending: boolean,
    action: () => Promise<EditorActionOutcome>,
  ): Promise<EditorActionOutcome | null> => {
    if (!pending) return null;
    const result = await action();
    if (!editorActionSucceeded(result)) return result;
    if (host.html.pending[area]) {
      return blockedAction(
        t("save-controller-html-pending", { area, status: result.status }),
      );
    }
    committed ||= result.status === "committed";
    return null;
  };

  const tag = await apply("tag", Boolean(host.html.pendingTag || host.html.pending.tag), () => host.html.applyTagChange());
  if (tag) return tag;
  const classes = await apply("classes", host.html.pending.classes, () => host.html.applyClasses());
  if (classes) return classes;
  const attributes = await apply("attributes", host.html.pending.attributes, () => host.html.draft.applyAttributes());
  if (attributes) return attributes;
  const image = await apply("image", host.html.pending.image, () => host.html.applyImageSource());
  if (image) return image;
  const text = await apply("text", host.html.pending.text, () => host.html.draft.applyText());
  if (text) return text;

  const remainingArea = (Object.keys(host.html.pending) as HtmlPendingArea[])
    .find((area) => host.html.pending[area]);
  if (remainingArea || host.html.pendingTag || host.html.inspectorPending.html) {
    return blockedAction(
      t("save-controller-html-still-pending", {
        area: remainingArea ? ` (${remainingArea})` : "",
      }),
    );
  }
  return committed
    ? committedAction()
    : noopAction(t("save-controller-no-html-pending"));
}

async function settleFrontendProjection(
  host: SaveControllerHost,
  identity: SaveSessionIdentity,
  receipt: SaveSettlementReceipt,
  mutationEpoch: number,
  topologyChanged: boolean,
): Promise<string[]> {
  const warnings: string[] = [];
  const context = host.context();
  if (
    context.projectRoot !== identity.expectedProjectRoot
    || context.runtimeSessionId !== identity.expectedSessionId
  ) {
    return warnings;
  }
  const noNewFrontendMutation = context.editorMutationEpoch === mutationEpoch;
  if (noNewFrontendMutation) {
    host.html.setInspectorPending("css", false);
    host.html.setInspectorPending("js", false);
  }
  for (const path of [...receipt.writtenFiles, ...receipt.removedFiles]) {
    invalidateFileBufferDraftSyncCursor(path);
  }

  if (receipt.status === "saved") {
    try {
      const derived = await host.reconcileWorkspaceDerivedState({
        expectedProjectRoot: identity.expectedProjectRoot,
        expectedSessionId: identity.expectedSessionId,
        expectedWorkspaceRevision: receipt.revisionAfter,
        topologyChanged,
        preferredRelativePath: context.activeScannedPath,
        refreshSourceGraph: true,
        refreshScss: true,
      });
      warnings.push(...derived.warnings);
    } catch (error) {
      warnings.push(
        t("save-controller-derived-resync", { message: errorMessage(error) }),
      );
    }
    host.bumpRefreshTokens();
    try {
      const preview = await host.projectLatestPreview({
        reason: "after-save",
        minimumWorkspaceRevision: receipt.revisionAfter,
        requestedPaths: [...new Set([...receipt.writtenFiles, ...receipt.removedFiles])].sort(),
        force: true,
      });
      if (preview.status === "deferred") {
        warnings.push(t("save-controller-preview-deferred"));
      }
    } catch (error) {
      warnings.push(t("save-controller-preview-resync", { message: errorMessage(error) }));
    }
    host.scheduleZolaValidation?.("save");
    host.markPreviewSavedToDisk?.(t("save-controller-preview-saved"));
    host.markDiskSaved(context.activeScannedPath);
  }
  return [...new Set(warnings)];
}

async function saveWorkspace(host: SaveControllerHost): Promise<boolean> {
  const identity = captureSaveSession(host);
  host.incrementSaveRequest();
  host.setGlobalStatus(t("save-controller-syncing-editors"), "saving");

  await flushAllEditorDrafts(host, identity);
  const html = await savePendingHtmlChanges(host);
  requireCurrentSaveSession(host, identity, t("save-controller-operation-html-staging"));
  if (!editorActionSucceeded(html)) {
    throw new Error(html.reason ?? t("save-controller-html-stopped", { status: html.status }));
  }
  // HTML staging may update a code-editor draft as part of the same action.
  await flushAllEditorDrafts(host, identity);

  const before = requireWorkspaceSnapshot(await readProjectWorkspaceState(), identity);
  requireCurrentSaveSession(host, identity, t("save-controller-operation-workspace-snapshot"));
  const mutationEpoch = host.context().editorMutationEpoch;

  if (!before.dirty) {
    await settleFrontendProjection(host, identity, {
      status: "noop",
      revisionAfter: before.revision,
      writtenFiles: [],
      removedFiles: [],
    }, mutationEpoch, false);
    host.acceptWorkspace(before);
    host.setGlobalStatus(t("save-controller-no-changes"), "saved");
    return false;
  }

  host.setGlobalStatus(
    t("save-controller-saving-revision", { revision: before.revision }),
    "saving",
  );
  const receipt = await saveProjectWorkspace({
    expectedProjectRoot: identity.expectedProjectRoot,
    expectedSessionId: identity.expectedSessionId,
    expectedRevision: before.revision,
  });
  requireCurrentSaveSession(host, identity, t("save-controller-operation-receipt"));
  requireSaveReceipt(receipt, before, identity);
  host.acceptProjectWorkspaceSaveBaseline(
    receipt.acceptedManifest,
    receipt.diskGenerationAfter,
  );
  host.acceptWorkspace(receipt.workspace);
  const successMessage = t("save-controller-saved", {
    written: receipt.writtenFiles.length,
    removed: receipt.removedFiles.length,
  });
  host.setGlobalStatus(
    successMessage,
    "saved",
  );
  const warnings = await settleFrontendProjection(
    host,
    identity,
    receipt,
    mutationEpoch,
    before.createdDocumentCount > 0 || before.deletedDocumentCount > 0,
  );
  if (
    warnings.length > 0
    && host.context().projectRoot === identity.expectedProjectRoot
    && host.context().runtimeSessionId === identity.expectedSessionId
  ) {
    host.setGlobalStatus(
      t("save-controller-saved-resync", {
        success: successMessage,
        warnings: warnings.join(" "),
      }),
      "saved",
    );
  }
  return receipt.status === "saved";
}

export async function saveSessionDrafts(host: SaveControllerHost): Promise<boolean> {
  try {
    return await saveWorkspace(host);
  } catch (error) {
    host.setGlobalStatus(
      isRecoveryRequiredError(error)
        ? t("save-controller-recovery-required", { message: errorMessage(error) })
        : t("save-controller-rejected", { message: errorMessage(error) }),
      "error",
    );
    return false;
  }
}

export async function saveSourceFile(host: SaveControllerHost): Promise<boolean> {
  return await saveSessionDrafts(host);
}

export async function saveActiveFile(host: SaveControllerHost): Promise<boolean> {
  return await saveSessionDrafts(host);
}
