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
  type CanvasProjectionPlan,
} from "$lib/project/io";
import { projectLatestProjectWorkspacePreview } from "$lib/kernel/project-workspace-preview-coordinator";
import {
  invalidateFileBufferDraftSyncCursor,
} from "$lib/session/file-buffer-draft-sync";
import {
  flushWorkspaceMutationInputs,
  type WorkspaceDerivedReconciliationOutcome,
} from "$lib/session/workspace-mutation-coordinator";
import { markDiskMutation, type DiskState } from "$lib/session/disk-state";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
  ProjectScan,
  ProjectWorkspaceSaveReceipt,
  ProjectWorkspaceSnapshot,
  ScssVariable,
} from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { errorMessage, isRecoveryRequiredError } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

/**
 * Frontend projection needed to present a ProjectWorkspace Save.
 *
 * None of these fields decides what reaches disk. The exact Rust workspace
 * revision captured after every editor flush is the sole Save authority.
 */
export type SaveControllerHost = {
  sessionProjectRoot: string;
  kernelProjectSessionId: string;
  editorMutationEpoch: number;
  projectWorkspaceSnapshot: ProjectWorkspaceSnapshot | null;
  saveRequest: number;
  projectStatus: string;
  scannedProject: ProjectScan | null;
  diskState: DiskState;
  activeScannedPath: string | null;
  inspectorPending: Record<InspectorPendingArea, boolean>;
  htmlPending: Record<HtmlPendingArea, boolean>;
  pendingTag: string | null;
  scssVariables: ScssVariable[];
  refreshToken: number;
  jsRefreshToken: number;
  previewWorkspaceRevision: string | null;
  pendingCanvasProjection: CanvasProjectionPlan | null;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  setInspectorPending: (area: InspectorPendingArea, pending: boolean) => void;
  applyTagChange: () => Promise<EditorActionOutcome>;
  applyClassesToHtml: () => Promise<EditorActionOutcome>;
  applyAttributesToHtml: () => Promise<EditorActionOutcome>;
  applyImageSourceToHtml: (src?: string) => Promise<EditorActionOutcome>;
  applyTextContentToHtml: () => Promise<EditorActionOutcome>;
  refreshSourceGraph?: (options?: { strict?: boolean }) => Promise<void>;
  reconcileWorkspaceDerivedState: (options: {
    expectedProjectRoot: string;
    expectedSessionId: string;
    expectedWorkspaceRevision: number;
    topologyChanged: boolean;
    preferredRelativePath?: string | null;
    refreshSourceGraph?: boolean;
    refreshScss?: boolean;
  }) => Promise<WorkspaceDerivedReconciliationOutcome>;
  requestPreviewRefresh: (reason: "after-save") => Promise<boolean>;
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
  const identity = {
    expectedProjectRoot: host.sessionProjectRoot.trim(),
    expectedSessionId: host.kernelProjectSessionId.trim(),
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
  if (
    host.sessionProjectRoot !== identity.expectedProjectRoot
    || host.kernelProjectSessionId !== identity.expectedSessionId
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
    if (host.htmlPending[area]) {
      return blockedAction(
        t("save-controller-html-pending", { area, status: result.status }),
      );
    }
    committed ||= result.status === "committed";
    return null;
  };

  const tag = await apply("tag", Boolean(host.pendingTag || host.htmlPending.tag), () => host.applyTagChange());
  if (tag) return tag;
  const classes = await apply("classes", host.htmlPending.classes, () => host.applyClassesToHtml());
  if (classes) return classes;
  const attributes = await apply("attributes", host.htmlPending.attributes, () => host.applyAttributesToHtml());
  if (attributes) return attributes;
  const image = await apply("image", host.htmlPending.image, () => host.applyImageSourceToHtml());
  if (image) return image;
  const text = await apply("text", host.htmlPending.text, () => host.applyTextContentToHtml());
  if (text) return text;

  const remainingArea = (Object.keys(host.htmlPending) as HtmlPendingArea[])
    .find((area) => host.htmlPending[area]);
  if (remainingArea || host.pendingTag || host.inspectorPending.html) {
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
  if (
    host.sessionProjectRoot !== identity.expectedProjectRoot
    || host.kernelProjectSessionId !== identity.expectedSessionId
  ) {
    return warnings;
  }
  const noNewFrontendMutation = host.editorMutationEpoch === mutationEpoch;
  if (noNewFrontendMutation) {
    host.setInspectorPending("css", false);
    host.setInspectorPending("js", false);
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
        preferredRelativePath: host.activeScannedPath,
        refreshSourceGraph: true,
        refreshScss: true,
      });
      warnings.push(...derived.warnings);
    } catch (error) {
      warnings.push(
        t("save-controller-derived-resync", { message: errorMessage(error) }),
      );
    }
    host.refreshToken += 1;
    host.jsRefreshToken += 1;
    try {
      const preview = await projectLatestProjectWorkspacePreview(host, {
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
    host.diskState = markDiskMutation(host.diskState, "save", host.activeScannedPath);
  }
  return [...new Set(warnings)];
}

async function saveWorkspace(host: SaveControllerHost): Promise<boolean> {
  const identity = captureSaveSession(host);
  host.saveRequest += 1;
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
  const mutationEpoch = host.editorMutationEpoch;

  if (!before.dirty) {
    await settleFrontendProjection(host, identity, {
      status: "noop",
      revisionAfter: before.revision,
      writtenFiles: [],
      removedFiles: [],
    }, mutationEpoch, false);
    host.projectWorkspaceSnapshot = before;
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
  host.projectWorkspaceSnapshot = receipt.workspace;
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
    && host.sessionProjectRoot === identity.expectedProjectRoot
    && host.kernelProjectSessionId === identity.expectedSessionId
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
