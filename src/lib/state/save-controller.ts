import {
  blockedAction,
  committedAction,
  editorActionSucceeded,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  getScssVariables,
  readProjectWorkspaceState,
  saveProjectWorkspace,
  scanProject,
  type CanvasProjectionPlan,
} from "$lib/project/io";
import { projectLatestProjectWorkspacePreview } from "$lib/kernel/project-workspace-preview-coordinator";
import { planOpenedProject, preservePreviewBaseUrl } from "$lib/project/session";
import {
  invalidateFileBufferDraftSyncCursor,
} from "$lib/session/file-buffer-draft-sync";
import { flushWorkspaceMutationInputs } from "$lib/session/workspace-mutation-coordinator";
import { markDiskMutation, type DiskState } from "$lib/session/disk-state";
import type {
  HtmlPendingArea,
  InspectorPendingArea,
  ProjectScan,
  ProjectWorkspaceSaveReceipt,
  ProjectWorkspaceSnapshot,
  SaveState,
  ScssVariable,
} from "$lib/types";
import { errorMessage } from "$lib/util";

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
  saveState: SaveState;
  saveStatus: string;
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
  setGlobalStatus: (text: string, kind: SaveState) => void;
  setInspectorPending: (area: InspectorPendingArea, pending: boolean) => void;
  applyTagChange: () => Promise<EditorActionOutcome>;
  applyClassesToHtml: () => Promise<EditorActionOutcome>;
  applyAttributesToHtml: () => Promise<EditorActionOutcome>;
  applyImageSourceToHtml: (src?: string) => Promise<EditorActionOutcome>;
  applyTextContentToHtml: () => Promise<EditorActionOutcome>;
  refreshSourceGraph?: (options?: { strict?: boolean }) => Promise<void>;
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
    throw new Error("Salvarea cere o sesiune activă a proiectului, legată de rădăcină și de identitatea Rust.");
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
    throw new Error(`${operation} a fost invalidat de schimbarea ProjectSession.`);
  }
}

function requireWorkspaceSnapshot(
  snapshot: ProjectWorkspaceSnapshot | null,
  identity: SaveSessionIdentity,
): ProjectWorkspaceSnapshot {
  if (!snapshot) throw new Error("Sesiunea proiectului nu este inițializată.");
  if (
    snapshot.projectRoot !== identity.expectedProjectRoot
    || snapshot.runtimeSessionId !== identity.expectedSessionId
  ) {
    throw new Error("Instantaneul sesiunii proiectului aparține altei sesiuni.");
  }
  if (!Number.isSafeInteger(snapshot.revision) || snapshot.revision < 0) {
    throw new Error("Sesiunea proiectului a returnat o revizie invalidă.");
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
    throw new Error("Confirmarea salvării aparține altei sesiuni.");
  }
  if (
    receipt.revisionBefore !== before.revision
    || receipt.revisionAfter !== receipt.workspace.revision
    || receipt.diskGenerationBefore !== before.diskGeneration
    || receipt.diskGenerationAfter !== receipt.workspace.diskGeneration
  ) {
    throw new Error("Confirmarea salvării nu respectă revizia și generația rezervate.");
  }
  if (receipt.workspace.dirty) {
    throw new Error("Salvarea s-a încheiat cu modificări nesalvate; starea de referință nu a fost acceptată.");
  }
  if (
    receipt.acceptedManifest.root !== identity.expectedProjectRoot
    || receipt.acceptedManifest.truncated
    || !Number.isSafeInteger(receipt.diskGenerationAfter)
    || receipt.diskGenerationAfter < 1
  ) {
    throw new Error("Confirmarea salvării nu conține un manifest complet și valid al discului acceptat.");
  }
  if (receipt.status === "noop" && before.dirty) {
    throw new Error("Sesiunea proiectului nu a raportat nicio operație pentru o revizie modificată.");
  }
}

async function flushAllEditorDrafts(host: SaveControllerHost, identity: SaveSessionIdentity) {
  await flushWorkspaceMutationInputs("save", {
    checkpoint: (phase) => {
      const label = phase === "editors"
        ? "Save draft flush"
        : phase === "page-js"
          ? "Save Page JS flush"
          : "Save FileBuffer flush";
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
        `Salvarea a fost oprită: editarea HTML „${area}” a rămas în așteptare după ${result.status}.`,
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
      `Salvarea a fost oprită: există încă o editare HTML în așteptare${remainingArea ? ` (${remainingArea})` : ""}.`,
    );
  }
  return committed ? committedAction() : noopAction("Nu există editări HTML în așteptare.");
}

async function settleFrontendProjection(
  host: SaveControllerHost,
  identity: SaveSessionIdentity,
  receipt: SaveSettlementReceipt,
  mutationEpoch: number,
) {
  requireCurrentSaveSession(host, identity, "Save settlement");
  const noNewFrontendMutation = host.editorMutationEpoch === mutationEpoch;
  if (noNewFrontendMutation) {
    host.setInspectorPending("css", false);
    host.setInspectorPending("vars", false);
    host.setInspectorPending("js", false);
  }
  for (const path of [...receipt.writtenFiles, ...receipt.removedFiles]) {
    invalidateFileBufferDraftSyncCursor(path);
  }

  if (receipt.status === "saved") {
    const previousProject = host.scannedProject;
    if (previousProject) {
      host.scannedProject = preservePreviewBaseUrl(
        await scanProject(identity.expectedProjectRoot),
        previousProject,
      );
      requireCurrentSaveSession(host, identity, "Save project rescan");
      host.projectStatus = planOpenedProject(host.scannedProject).projectStatus;
    }
    await host.refreshSourceGraph?.({ strict: true });
    requireCurrentSaveSession(host, identity, "Save Source Graph refresh");
    host.scssVariables = await getScssVariables(identity).catch(() => host.scssVariables);
    host.refreshToken += 1;
    host.jsRefreshToken += 1;
    await projectLatestProjectWorkspacePreview(host, {
      reason: "after-save",
      minimumWorkspaceRevision: receipt.revisionAfter,
      requestedPaths: [...new Set([...receipt.writtenFiles, ...receipt.removedFiles])].sort(),
      force: true,
    });
    requireCurrentSaveSession(host, identity, "Save ProjectWorkspace Preview projection");
    host.scheduleZolaValidation?.("save");
    host.markPreviewSavedToDisk?.("Sesiunea proiectului a fost salvată atomic pe disc.");
    host.diskState = markDiskMutation(host.diskState, "save", host.activeScannedPath);
  }
}

async function saveWorkspace(host: SaveControllerHost): Promise<boolean> {
  const identity = captureSaveSession(host);
  host.saveRequest += 1;
  host.saveState = "saving";
  host.saveStatus = "Se sincronizează editorii în sesiunea proiectului...";

  await flushAllEditorDrafts(host, identity);
  const html = await savePendingHtmlChanges(host);
  requireCurrentSaveSession(host, identity, "Save HTML staging");
  if (!editorActionSucceeded(html)) {
    throw new Error(html.reason ?? `Salvarea HTML a fost oprită (${html.status}).`);
  }
  // HTML staging may update a code-editor draft as part of the same action.
  await flushAllEditorDrafts(host, identity);

  const before = requireWorkspaceSnapshot(await readProjectWorkspaceState(), identity);
  requireCurrentSaveSession(host, identity, "Save workspace snapshot");
  const mutationEpoch = host.editorMutationEpoch;

  if (!before.dirty) {
    await settleFrontendProjection(host, identity, {
      status: "noop",
      revisionAfter: before.revision,
      writtenFiles: [],
      removedFiles: [],
    }, mutationEpoch);
    host.projectWorkspaceSnapshot = before;
    host.setGlobalStatus("Nicio modificare de salvat.", "saved");
    return false;
  }

  host.saveStatus = `Se salvează atomic revizia ${before.revision} a sesiunii proiectului...`;
  const receipt = await saveProjectWorkspace({
    expectedProjectRoot: identity.expectedProjectRoot,
    expectedSessionId: identity.expectedSessionId,
    expectedRevision: before.revision,
  });
  requireCurrentSaveSession(host, identity, "Save receipt");
  requireSaveReceipt(receipt, before, identity);
  host.acceptProjectWorkspaceSaveBaseline(
    receipt.acceptedManifest,
    receipt.diskGenerationAfter,
  );
  host.projectWorkspaceSnapshot = receipt.workspace;
  await settleFrontendProjection(host, identity, receipt, mutationEpoch);
  host.setGlobalStatus(
    `Salvat atomic: ${receipt.writtenFiles.length} fișier(e) scrise, ${receipt.removedFiles.length} șterse.`,
    "saved",
  );
  return receipt.status === "saved";
}

export async function saveSessionDrafts(host: SaveControllerHost): Promise<boolean> {
  try {
    return await saveWorkspace(host);
  } catch (error) {
    host.setGlobalStatus(`Salvarea sesiunii proiectului a eșuat: ${errorMessage(error)}`, "error");
    return false;
  }
}

export async function saveSourceFile(host: SaveControllerHost): Promise<boolean> {
  return await saveSessionDrafts(host);
}

export async function saveActiveFile(host: SaveControllerHost): Promise<boolean> {
  return await saveSessionDrafts(host);
}
