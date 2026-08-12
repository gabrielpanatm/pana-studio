import {
  canElementAcceptChildren,
  htmlVoidTags,
  normalizeClassTokens,
  type InsertPosition,
} from "$lib/html/mutations";
import type { EditorHtmlTarget } from "$lib/editor-runtime/commands";
import {
  blockedAction,
  cancelledAction,
  committedAction,
  editorActionSucceeded,
  failedAction,
  noopAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import {
  projectCommittedPreviewSelectionBatchMutation,
  projectCommittedPreviewStructuralMutation,
  previewStructuralBlockingDiagnostic,
  requireCommittedPreviewStructuralPatch,
  type PreviewStructuralCanonicalProjectionHost,
  type PreviewStructuralExecutionReceipt,
} from "$lib/kernel/preview-projection-control";
import {
  isPreviewStructuralCancellation,
  previewStructuralSessionLeaseMatches,
  previewStructuralCommandIdentity,
  runInPreviewStructuralLane,
  type PreviewStructuralSessionLease,
} from "$lib/kernel/preview-structural-lane";
import { htmlPaletteInsertOptions } from "$lib/project/html-palette";
import {
  createZolaImageIntent,
  resolveZolaImageSource,
  zolaImageSourceFailureMessage,
} from "$lib/html/zola-image";
import { reconcilePageAssetContracts } from "$lib/page-assets/contract";
import { isZolaTemplatePath, scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import {
  executePreviewHtmlAttributesIntent,
  executePreviewHtmlDeleteIntent,
  executePreviewHtmlDuplicateIntent,
  executePreviewHtmlInsertDropIntent,
  executePreviewSelectionBatchIntent,
  executePreviewHtmlTextIntent,
  readProjectWorkspaceState,
} from "$lib/project/io";
import { committedDraftCanSettle } from "$lib/session/committed-draft-settlement";
import { settleProjectWorkspaceMutation } from "$lib/session/workspace-mutation-coordinator";
import {
  parseSourceEditLocation,
} from "$lib/source-graph/location";
import type { PreviewInsertDropRequest } from "$lib/state/preview-insert-controller";
import type {
  EditableAttributes,
  HtmlPendingArea,
  NativeBlockOptionIntent,
  NativeIconMutationIntent,
  NativeBlockSlotMutationRequest,
  ProjectHtmlAttributePatch,
  ProjectHtmlAttributeMutation,
  ProjectMovePosition,
  PreviewSelectionBatchAction,
  ProjectGeneratedIdentityIntent,
  ProjectGeneratedIdentityKind,
  ProjectZolaImageIntent,
  ProjectHtmlTextPatch,
  ProjectFile,
  ProjectDiskManifest,
  ProjectScan,
  CoordinatedElementSelection,
  SourceEditLocation,
  ZolaImagePresentation,
} from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export type HtmlActionsControllerHost = PreviewStructuralCanonicalProjectionHost & {
  coordinatedElementSelection: CoordinatedElementSelection | null;
  structureStatus: string;
  canEditHtmlStructure: boolean;
  canAddChildToSelectedElement: boolean;
  imageStatus: string;
  activeScannedPath: string | null;
  source: string;
  imageSourceValue: string;
  classStatus: string;
  classEditorValue: string;
  attributeStatus: string;
  attributeValues: EditableAttributes;
  textStatus: string;
  textContentValue: string;
  textEditOriginalKey: string | null;
  textEditOriginalText: string | null;
  scannedProject: ProjectScan | null;
  sourceCache: Record<string, string>;
  stageKernelPlannedTemplateDraft: (
    tpl: SourceEditLocation,
    plannedSource: string,
    options?: { pendingArea?: HtmlPendingArea; status?: string; isCurrent?: () => boolean },
  ) => Promise<string | null>;
  getPreviewDocument: () => Document | undefined;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  loadScannedProjectFile: (file: ProjectFile) => Promise<void>;
};

export type HtmlActionTarget = {
  tag: string;
  selectionRevision?: number | null;
  renderInstanceId?: string | null;
  sourceId?: string | null;
  templateSourceId?: string | null;
  sourceLocation?: SourceEditLocation | null;
  sessionId?: string | null;
  hasChildElements?: boolean;
  rawText?: string;
  attributes?: Readonly<Record<string, string>>;
  classes?: readonly string[];
  zolaImage?: ZolaImagePresentation | null;
};

function freezeHtmlActionTarget(target: HtmlActionTarget): HtmlActionTarget {
  return Object.freeze({
    ...target,
    sourceLocation: target.sourceLocation
      ? Object.freeze({ ...target.sourceLocation })
      : null,
    attributes: Object.freeze({ ...(target.attributes ?? {}) }),
    classes: Object.freeze([...(target.classes ?? [])]),
  });
}

/** Captures selection/source identity before an operation can wait in the structural lane. */
export function captureHtmlActionTarget(
  target: CoordinatedElementSelection | EditorHtmlTarget | null | undefined,
): HtmlActionTarget | null {
  if (!target) return null;
  if ("snapshot" in target) {
    const observation = target.observation;
    return freezeHtmlActionTarget({
      tag: observation.tag,
      selectionRevision: target.snapshot.selectionRevision,
      renderInstanceId: target.renderInstanceId,
      sourceId: target.sourceNodeId,
      templateSourceId: null,
      sourceLocation: target.sourceLocation,
      sessionId: target.snapshot.runtimeSessionId,
      hasChildElements: observation.hasChildElements,
      rawText: observation.rawText,
      attributes: observation.attributes,
      zolaImage: observation.zolaImage,
      classes: observation.classes,
    });
  }
  if ("kind" in target) {
    const observation = target.observation ?? null;
    return freezeHtmlActionTarget({
      tag: target.tag,
      selectionRevision: target.selectionRevision ?? null,
      renderInstanceId: target.renderInstanceId ?? null,
      sourceId: target.sourceId ?? null,
      templateSourceId: target.templateSourceId ?? null,
      sourceLocation: target.sourceLocation ?? null,
      sessionId: target.sessionId ?? null,
      hasChildElements: observation?.hasChildElements,
      rawText: observation?.rawText,
      attributes: observation?.attributes,
      zolaImage: observation?.zolaImage ?? null,
      classes: observation?.classes,
    });
  }
  return null;
}

function currentSelectionMatchesTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
) {
  const current = host.coordinatedElementSelection;
  if (!current) return false;
  if (
    target.selectionRevision
    && target.selectionRevision !== current.snapshot.selectionRevision
  ) return false;
  if (target.renderInstanceId && target.renderInstanceId !== current.renderInstanceId) return false;
  if (target.sessionId && target.sessionId !== current.snapshot.runtimeSessionId) return false;
  if (!target.sourceId || !current.sourceNodeId) return false;
  return target.sourceId === current.sourceNodeId;
}

function normalizedAttributeDraft(attributes: Readonly<EditableAttributes>) {
  return Object.fromEntries(
    Object.entries(attributes)
      .filter(([name]) => !name.toLowerCase().startsWith("data-pana-"))
      .map(([name, value]) => [name, value] as const)
      .sort(([left], [right]) => left.localeCompare(right)),
  );
}

function attributeDraftMatches(
  current: Readonly<EditableAttributes>,
  submitted: Readonly<EditableAttributes>,
) {
  return JSON.stringify(normalizedAttributeDraft(current))
    === JSON.stringify(normalizedAttributeDraft(submitted));
}

function attributeDraftToken(attributes: Readonly<EditableAttributes>) {
  return JSON.stringify(normalizedAttributeDraft(attributes));
}

function blockedReceiptOutcome(
  receipt: PreviewStructuralExecutionReceipt,
  fallback: string,
): EditorActionOutcome | null {
  if (receipt.status === "committed") return null;
  const diagnostic = previewStructuralBlockingDiagnostic(receipt);
  const reason = (diagnostic ? errorMessage(diagnostic.diagnostic) : "")
    || errorMessage(receipt.messageDiagnostic)
    || fallback;
  return blockedAction(reason);
}

function actionErrorOutcome(error: unknown): EditorActionOutcome {
  const reason = errorMessage(error);
  return isPreviewStructuralCancellation(error)
    ? cancelledAction(reason)
    : failedAction(reason);
}

function insertPositionLabel(position: PreviewInsertDropRequest["position"]) {
  if (position === "before") return t("html-actions-position-before");
  if (position === "after") return t("html-actions-position-after");
  return t("html-actions-position-inside");
}

function missingKernelIdentityMessage(action: string) {
  return t("html-actions-identity-missing", { action });
}

function hasMultiElementSelection(host: HtmlActionsControllerHost) {
  return (host.selectionSnapshot?.members.length ?? 0) > 1;
}

async function executeSelectionBatch(
  host: HtmlActionsControllerHost,
  action: PreviewSelectionBatchAction,
): Promise<EditorActionOutcome> {
  const result = await runInPreviewStructuralLane(host, async (lease) => {
    if (!lease.selection || lease.selection.members.length < 2) {
      return blockedAction("Operația batch cere cel puțin două elemente selectate.");
    }
    const receipt = await executePreviewSelectionBatchIntent({
      schemaVersion: 1,
      action,
    }, previewStructuralCommandIdentity(lease, true));
    if (receipt.status !== "committed") {
      return blockedAction(receipt.diagnostics[0] || "Operația batch a fost blocată de kernel.");
    }
    await projectCommittedPreviewSelectionBatchMutation(host, lease, receipt);
    return committedAction();
  });
  return result ?? cancelledAction("Operația batch a fost anulată odată cu sesiunea structurală.");
}

export async function moveSelectedHtmlElements(
  host: HtmlActionsControllerHost,
  targetSourceId: string,
  targetTag: string | null,
  position: ProjectMovePosition,
): Promise<EditorActionOutcome> {
  if (position === "inside") {
    return blockedAction("Mutarea multiplă v1 acceptă numai pozițiile înainte/după între frați.");
  }
  try {
    const result = await executeSelectionBatch(host, {
      kind: "move",
      targetSourceId,
      targetTag,
      position,
    });
    host.structureStatus = editorActionSucceeded(result)
      ? "Elementele selectate au fost mutate."
      : (result.reason ?? "Mutarea batch a fost blocată.");
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.structureStatus = result.reason ?? result.status;
    host.setGlobalStatus(host.structureStatus, "error");
    return result;
  }
}

export function attributeMutationsFromRecord(attributes: Record<string, string | null>): ProjectHtmlAttributeMutation[] {
  return Object.entries(attributes).map(([name, value]) => value === null
    ? { kind: "removeAttribute", name }
    : { kind: "setAttribute", name, value });
}

export function htmlAttributeRecordForKernel(
  attributes: Readonly<EditableAttributes>,
  targetAttributes: Readonly<Record<string, string>> = {},
  zolaImageManaged = false,
): Record<string, string | null> {
  const next: Record<string, string | null> = Object.fromEntries(
    Object.entries(attributes)
      .filter(([name]) => !name.toLowerCase().startsWith("data-pana-"))
      .map(([name, value]) => [name, value]),
  );
  if (zolaImageManaged) {
    delete next.src;
    delete next.width;
    delete next.height;
  }
  for (const name of Object.keys(targetAttributes)) {
    if (
      !(name in attributes)
      && !name.toLowerCase().startsWith("data-pana-")
      && !["class", "style"].includes(name)
      && !(zolaImageManaged && ["src", "width", "height"].includes(name.toLowerCase()))
    ) {
      next[name] = null;
    }
  }
  return next;
}

const BATCH_COMMON_HTML_ATTRIBUTES = new Set([
  "title",
  "lang",
  "dir",
  "tabindex",
  "hidden",
  "inert",
  "contenteditable",
  "draggable",
  "spellcheck",
  "translate",
  "role",
]);

function isBatchCommonHtmlAttribute(name: string) {
  const normalized = name.trim().toLowerCase();
  return BATCH_COMMON_HTML_ATTRIBUTES.has(normalized)
    || normalized.startsWith("aria-")
    || (normalized.startsWith("data-") && !normalized.startsWith("data-pana-"));
}

export function batchCommonAttributeMutations(
  attributes: Readonly<EditableAttributes>,
  primaryAttributes: Readonly<Record<string, string>>,
): ProjectHtmlAttributeMutation[] {
  const next = htmlAttributeRecordForKernel(attributes, primaryAttributes, false);
  const names = new Set([...Object.keys(next), ...Object.keys(primaryAttributes)]);
  const mutations: ProjectHtmlAttributeMutation[] = [];
  for (const name of names) {
    const normalized = name.trim().toLowerCase();
    if (!isBatchCommonHtmlAttribute(normalized)) continue;
    const nextValue = next[name] ?? next[normalized] ?? null;
    const previousValue = primaryAttributes[name] ?? primaryAttributes[normalized] ?? null;
    if (nextValue === previousValue) continue;
    mutations.push(nextValue === null
      ? { kind: "removeAttribute", name: normalized }
      : { kind: "setAttribute", name: normalized, value: nextValue });
  }
  return mutations;
}

function cacheCommittedHtmlPatch(
  host: HtmlActionsControllerHost,
  patch: { file: string; contents: string },
) {
  host.sourceCache = {
    ...host.sourceCache,
    [scannedCacheKey({ relativePath: patch.file })]: patch.contents,
  };
  if (host.activeScannedPath === patch.file) {
    host.source = patch.contents;
  }
}

async function executeSelectedHtmlAttributes(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
  attributes: Record<string, string | null>,
  project: (patch: ProjectHtmlAttributePatch, target: HtmlActionTarget) => Promise<void> | void,
  zolaImage: ProjectZolaImageIntent | null = null,
  nativeBlockOption: NativeBlockOptionIntent | null = null,
  nativeIcon: NativeIconMutationIntent | null = null,
  generatedIdentity: ProjectGeneratedIdentityIntent | null = null,
): Promise<EditorActionOutcome> {
  const result = await runInPreviewStructuralLane(host, async (lease) => {
    if (!target.sourceId) {
      throw new Error(missingKernelIdentityMessage(t("html-actions-attributes-noun")));
    }

    const receipt = await executePreviewHtmlAttributesIntent({
      intent: {
        messageType: "preview-html-attributes",
        sourceId: target.sourceId,
        sourceTag: target.tag,
      },
      attributeIntent: {
        targetSourceId: target.sourceId,
        targetTag: target.tag,
        attributes: attributeMutationsFromRecord(attributes),
        ...(zolaImage ? { zolaImage } : {}),
        ...(nativeBlockOption ? { nativeBlockOption } : {}),
        ...(nativeIcon ? { nativeIcon } : {}),
        ...(generatedIdentity ? { generatedIdentity } : {}),
      },
    }, previewStructuralCommandIdentity(lease, true));

    const blocked = blockedReceiptOutcome(
      receipt,
      t("html-actions-attributes-engine-blocked"),
    );
    if (blocked) return blocked;

    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("html-actions-attributes-engine-blocked"),
    );
    if (
      receipt.workspaceMutation
      && !receipt.workspaceMutation.changed
      && receipt.workspaceMutation.revisionAfter === receipt.workspaceMutation.revisionBefore
    ) {
      await project(patch, target);
      return noopAction(t("html-actions-attributes-already-match"));
    }
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      cacheCommittedHtmlPatch(host, patch);
      await project(patch, target);
    });
    return committedAction();
  });
  return result ?? cancelledAction(t("html-actions-attributes-session-cancelled"));
}

export type ApplyNativeBlockOptionRequest = {
  providerId: string;
  optionId: string;
  value: NativeBlockOptionIntent["value"];
  rootTag: string;
  rootSourceId: string | null;
  rootLocation: SourceEditLocation | null;
  rootSessionId: string | null;
};

export async function applyNativeBlockOptionToHtml(
  host: HtmlActionsControllerHost,
  request: ApplyNativeBlockOptionRequest,
): Promise<EditorActionOutcome> {
  const target = freezeHtmlActionTarget({
    tag: request.rootTag,
    sourceId: request.rootSourceId,
    sourceLocation: request.rootLocation,
    sessionId: request.rootSessionId,
  });
  try {
    const result = await executeSelectedHtmlAttributes(
      host,
      target,
      {},
      () => {},
      null,
      {
        providerId: request.providerId,
        optionId: request.optionId,
        value: request.value,
      },
    );
    if (result.status === "committed") {
      host.setGlobalStatus(t("html-actions-block-property-confirmed"), "unsaved");
    }
    return result;
  } catch (error) {
    const outcome = actionErrorOutcome(error);
    host.setGlobalStatus(
      t("html-actions-block-property-failed", {
        message: outcome.reason ?? outcome.status,
      }),
      "error",
    );
    return outcome;
  }
}

export type ApplyNativeIconRequest = {
  intent: NativeIconMutationIntent;
  rootTag: string;
  rootSourceId: string | null;
  rootLocation: SourceEditLocation | null;
  rootSessionId: string | null;
};

export async function applyNativeIconToHtml(
  host: HtmlActionsControllerHost,
  request: ApplyNativeIconRequest,
): Promise<EditorActionOutcome> {
  const target = freezeHtmlActionTarget({
    tag: request.rootTag,
    sourceId: request.rootSourceId,
    sourceLocation: request.rootLocation,
    sessionId: request.rootSessionId,
  });
  try {
    const result = await executeSelectedHtmlAttributes(
      host,
      target,
      {},
      async () => {},
      null,
      null,
      request.intent,
    );
    if (result.status === "committed") {
      host.setGlobalStatus(t("html-actions-icon-property-confirmed"), "unsaved");
    }
    return result;
  } catch (error) {
    const outcome = actionErrorOutcome(error);
    host.setGlobalStatus(
      t("html-actions-icon-property-failed", {
        message: outcome.reason ?? outcome.status,
      }),
      "error",
    );
    return outcome;
  }
}

export async function applyZolaImageProcessingToHtml(
  host: HtmlActionsControllerHost,
  intent: ProjectZolaImageIntent,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target || target.tag !== "img") {
    host.imageStatus = t("html-actions-zola-image-select");
    return blockedAction(host.imageStatus);
  }

  host.setHtmlPending("image", true);
  host.imageStatus = intent.enabled
    ? t("html-actions-zola-image-configuring")
    : t("html-actions-zola-image-removing");
  try {
    const result = await executeSelectedHtmlAttributes(
      host,
      target,
      {},
      (_patch, capturedTarget) => {
        if (!currentSelectionMatchesTarget(host, capturedTarget)) return;
        host.imageStatus = intent.enabled
          ? t("html-actions-zola-image-managed")
          : t("html-actions-zola-image-removed");
      },
      intent,
    );
    host.setHtmlPending("image", false);
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.imageStatus = t("html-actions-zola-image-failed", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-zola-image-error", {
      message: result.reason ?? result.status,
    }), "error");
    host.setHtmlPending("image", false);
    return result;
  }
}

async function executeSelectedHtmlText(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
  text: string,
  project: (patch: ProjectHtmlTextPatch, target: HtmlActionTarget) => Promise<void> | void,
  options: {
    deferCanonicalProjection?: boolean;
    editSessionId?: string | null;
  } = {},
): Promise<EditorActionOutcome> {
  const result = await runInPreviewStructuralLane(host, async (lease) => {
    const groupedEditSession = Boolean(options.editSessionId);
    if (!target.sourceId) {
      throw new Error(missingKernelIdentityMessage(t("html-actions-text-noun")));
    }

    const receipt = await executePreviewHtmlTextIntent({
      intent: {
        messageType: "preview-html-text",
        sourceId: target.sourceId,
        sourceTag: target.tag,
      },
      textIntent: {
        targetSourceId: target.sourceId,
        targetTag: target.tag,
        text,
      },
      deferCanonicalProjection: options.deferCanonicalProjection === true,
      editSessionId: options.editSessionId ?? null,
    }, previewStructuralCommandIdentity(lease, !groupedEditSession));

    const blocked = blockedReceiptOutcome(
      receipt,
      t("html-actions-text-engine-blocked"),
    );
    if (blocked) return blocked;

    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("html-actions-text-engine-blocked"),
    );
    if (
      !options.deferCanonicalProjection
      && receipt.workspaceMutation
      && !receipt.workspaceMutation.changed
      && receipt.workspaceMutation.revisionAfter === receipt.workspaceMutation.revisionBefore
    ) {
      await project(patch, target);
      return noopAction(t("html-actions-text-already-matches"));
    }
    if (options.deferCanonicalProjection) {
      if (!previewStructuralSessionLeaseMatches(host, lease)) {
        return cancelledAction(t("html-actions-text-draft-session-closed"));
      }
      const mutation = receipt.workspaceMutation;
      if (
        mutation
        && !mutation.changed
        && mutation.revisionAfter === mutation.revisionBefore
      ) {
        await project(patch, target);
        return noopAction(t("html-actions-text-draft-already-matches"));
      }
      if (
        !mutation?.changed
        || mutation.revisionAfter <= mutation.revisionBefore
        || !mutation.transactionId?.trim()
      ) {
        throw new Error(
          t("html-actions-text-draft-transition-invalid"),
        );
      }
      try {
        const snapshot = await readProjectWorkspaceState();
        if (
          !snapshot
          || snapshot.projectRoot !== lease.projectRoot
          || snapshot.runtimeSessionId !== lease.sessionId
          || snapshot.revision !== mutation.revisionAfter
        ) {
          throw new Error(
            t("html-actions-text-draft-revision-unconfirmed"),
          );
        }
        await settleProjectWorkspaceMutation(host, {
          projectRoot: lease.projectRoot,
          runtimeSessionId: lease.sessionId,
          mutation,
          workspace: snapshot,
        }, {
          preferredRelativePath: patch.file,
          refreshSourceGraph: false,
          refreshScss: false,
          projectPreview: false,
          warningLabel: t("html-actions-text-draft-operation"),
        });
      } catch (error) {
        host.setGlobalStatus(
          t("html-actions-text-resync", { message: errorMessage(error) }),
          "unsaved",
        );
      }
      cacheCommittedHtmlPatch(host, patch);
      await project(patch, target);
      return committedAction();
    }
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      cacheCommittedHtmlPatch(host, patch);
      await project(patch, target);
    });
    return committedAction();
  });
  return result ?? cancelledAction(t("html-actions-text-session-cancelled"));
}

export async function deleteSelectedHtmlElement(
  host: HtmlActionsControllerHost,
  editorTarget: EditorHtmlTarget | null = null,
): Promise<EditorActionOutcome> {
  if (hasMultiElementSelection(host)) {
    try {
      const result = await executeSelectionBatch(host, { kind: "delete" });
      host.structureStatus = editorActionSucceeded(result)
        ? "Elementele selectate au fost șterse."
        : (result.reason ?? "Ștergerea batch a fost blocată.");
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.structureStatus = result.reason ?? result.status;
      host.setGlobalStatus(host.structureStatus, "error");
      return result;
    }
  }
  const capturedTarget = captureHtmlActionTarget(editorTarget ?? host.coordinatedElementSelection);
  try {
    const result = await runInPreviewStructuralLane(host, async (lease) => {
      const target = capturedTarget;
      if (!target) {
        host.structureStatus = t("html-actions-delete-select");
        host.setGlobalStatus(host.structureStatus, "error");
        return blockedAction(host.structureStatus);
      }

      if (!target.sourceId) {
        const message = missingKernelIdentityMessage(t("html-actions-delete-noun"));
        host.structureStatus = message;
        host.setGlobalStatus(message, "error");
        return blockedAction(message);
      }

      const receipt = await executePreviewHtmlDeleteIntent({
        intent: {
          messageType: "preview-delete-selected",
          sourceId: target.sourceId ?? null,
          sourceTag: target.tag,
        },
        deleteIntent: {
          targetSourceId: target.sourceId,
          targetRenderInstanceId: target.renderInstanceId ?? null,
          targetTag: target.tag,
        },
      }, previewStructuralCommandIdentity(lease, true));

      const blocked = blockedReceiptOutcome(
        receipt,
        t("html-actions-delete-engine-blocked"),
      );
      if (blocked) return blocked;

      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        t("html-actions-delete-engine-blocked"),
      );
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        host.structureStatus = t("html-actions-deleted", { tag: target.tag });
      });
      return committedAction();
    });
    return result ?? cancelledAction(t("html-actions-delete-session-cancelled"));
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.structureStatus = t("html-actions-delete-failed", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-delete-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function duplicateSelectedHtmlElement(
  host: HtmlActionsControllerHost,
  editorTarget: EditorHtmlTarget | null = null,
): Promise<EditorActionOutcome> {
  if (hasMultiElementSelection(host)) {
    try {
      const result = await executeSelectionBatch(host, { kind: "duplicate" });
      host.structureStatus = editorActionSucceeded(result)
        ? "Elementele selectate au fost duplicate."
        : (result.reason ?? "Duplicarea batch a fost blocată.");
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.structureStatus = result.reason ?? result.status;
      host.setGlobalStatus(host.structureStatus, "error");
      return result;
    }
  }
  const capturedTarget = captureHtmlActionTarget(editorTarget ?? host.coordinatedElementSelection);
  try {
    const result = await runInPreviewStructuralLane(host, async (lease) => {
      const target = capturedTarget;
      if (!target) {
        host.structureStatus = t("html-actions-duplicate-select");
        host.setGlobalStatus(host.structureStatus, "error");
        return blockedAction(host.structureStatus);
      }
      if (target.tag === "body" || target.tag === "html") {
        host.structureStatus = t("html-actions-root-cannot-duplicate");
        host.setGlobalStatus(host.structureStatus, "error");
        return blockedAction(host.structureStatus);
      }

      if (!target.sourceId) {
        const message = missingKernelIdentityMessage(t("html-actions-duplicate-noun"));
        host.structureStatus = message;
        host.setGlobalStatus(message, "error");
        return blockedAction(message);
      }

      const receipt = await executePreviewHtmlDuplicateIntent({
        intent: {
          messageType: "preview-duplicate-selected",
          sourceId: target.sourceId ?? null,
          sourceTag: target.tag,
        },
        duplicateIntent: {
          sourceSourceId: target.sourceId,
          sourceTag: target.tag,
        },
      }, previewStructuralCommandIdentity(lease, true));

      const blocked = blockedReceiptOutcome(
        receipt,
        t("html-actions-duplicate-engine-blocked"),
      );
      if (blocked) return blocked;

      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        t("html-actions-duplicate-engine-blocked"),
      );
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
        cacheCommittedHtmlPatch(host, patch);
        host.structureStatus = t("html-actions-duplicated", { tag: patch.tag });
      });
      return committedAction();
    });
    return result ?? cancelledAction(t("html-actions-duplicate-session-cancelled"));
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.structureStatus = t("html-actions-duplicate-failed", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-duplicate-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function mutateNativeBlockSlotStructure(
  host: HtmlActionsControllerHost,
  request: NativeBlockSlotMutationRequest,
): Promise<EditorActionOutcome> {
  if (request.operation === "move") {
    return blockedAction("Mutarea slotului trebuie executată atomic prin Editor Move.");
  }
  try {
    const result = await runInPreviewStructuralLane(host, async (lease) => {
      const context = Object.freeze({ ...request.context });
      let receipt: PreviewStructuralExecutionReceipt;
      if (request.operation === "insert") {
        const targetSourceId = request.slot.containerSourceNodeId;
        if (!targetSourceId) {
          return blockedAction("Containerul slotului nu mai are ancoră Source Graph stabilă.");
        }
        receipt = await executePreviewHtmlInsertDropIntent({
          intent: {
            messageType: "preview-insert-drop",
            targetSourceId,
            targetTag: "div",
            targetKind: "html",
            position: "inside",
            elementTag: "div",
          },
          insertIntent: {
            targetSourceId,
            targetTag: "div",
            targetKind: "html",
            position: "inside",
            element: {
              kind: "nativeBlockSlotItem",
              blockId: context.providerId,
              tag: "div",
              label: request.slot.itemKind,
            },
            nativeBlockSlot: context,
          },
        }, previewStructuralCommandIdentity(lease, true));
      } else if (request.operation === "duplicate") {
        const item = request.item;
        if (!item) return blockedAction("Slide-ul de duplicat nu mai există.");
        receipt = await executePreviewHtmlDuplicateIntent({
          intent: {
            messageType: "preview-duplicate-selected",
            sourceId: item.sourceNodeId,
            sourceTag: item.tag,
          },
          duplicateIntent: {
            sourceSourceId: item.sourceNodeId,
            sourceTag: item.tag,
            nativeBlockSlot: context,
          },
        }, previewStructuralCommandIdentity(lease, true));
      } else {
        const item = request.item;
        if (!item) return blockedAction("Slide-ul de șters nu mai există.");
        receipt = await executePreviewHtmlDeleteIntent({
          intent: {
            messageType: "preview-delete-selected",
            sourceId: item.sourceNodeId,
            sourceTag: item.tag,
          },
          deleteIntent: {
            targetSourceId: item.sourceNodeId,
            targetTag: item.tag,
            nativeBlockSlot: context,
          },
        }, previewStructuralCommandIdentity(lease, true));
      }
      const blocked = blockedReceiptOutcome(receipt, "Rust a refuzat mutația slotului.");
      if (blocked) return blocked;
      if (receipt.status !== "committed" || !receipt.patch) {
        throw new Error("Rust nu a emis patch-ul structural al slotului.");
      }
      const patch = receipt.patch;
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        host.structureStatus = "Structura Slider a fost salvată.";
      });
      return committedAction();
    });
    return result ?? cancelledAction("Sesiunea structurală s-a schimbat.");
  } catch (error) {
    const outcome = actionErrorOutcome(error);
    host.setGlobalStatus(outcome.reason ?? "Mutația Slider a eșuat.", "error");
    return outcome;
  }
}

export async function insertPaletteElementAtTarget(
  host: HtmlActionsControllerHost,
  request: PreviewInsertDropRequest,
): Promise<EditorActionOutcome> {
  const capturedRequest = Object.freeze({
    ...request,
    element: Object.freeze({ ...request.element }),
  });
  try {
    const result = await runInPreviewStructuralLane(host, (lease) =>
      insertPaletteElementAtTargetInLane(host, capturedRequest, lease));
    return result ?? cancelledAction();
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.structureStatus = t("html-actions-insert-failed", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-insert-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

async function insertPaletteElementAtTargetInLane(
  host: HtmlActionsControllerHost,
  request: PreviewInsertDropRequest,
  lease: PreviewStructuralSessionLease,
): Promise<EditorActionOutcome> {
  const targetSourceId = request.targetSourceId ||
    (request.targetKind === "empty-tera-slot" || request.targetKind === "active-document-root"
      ? request.targetTemplateSourceId
      : null);
  if (!host.canEditHtmlStructure) {
    host.structureStatus = t("html-actions-switch-preview");
    host.setGlobalStatus(host.structureStatus, "error");
    return blockedAction(host.structureStatus);
  }
  if (request.position === "inside" && !canElementAcceptChildren(request.targetTag, htmlVoidTags)) {
    host.structureStatus = t("html-actions-target-no-children");
    host.setGlobalStatus(host.structureStatus, "error");
    return blockedAction(host.structureStatus);
  }

  if (!targetSourceId) {
    host.structureStatus = t("html-actions-target-metadata-unstable");
    host.setGlobalStatus(host.structureStatus, "error");
    return blockedAction(host.structureStatus);
  }

  const blockId = request.element.kind === "block" ? request.element.blockId ?? null : null;
  const options = blockId
    ? {
        tag: request.element.tag,
        className: request.element.className,
        text: request.element.text,
        html: request.element.html,
      }
    : htmlPaletteInsertOptions(request.element);
  const label = insertPositionLabel(request.position);

  try {
    const receipt = await executePreviewHtmlInsertDropIntent({
      intent: {
        messageType: "preview-insert-drop",
        targetSourceId,
        targetTemplateSourceId: request.targetTemplateSourceId,
        targetSessionId: request.targetSessionId,
        targetTag: request.targetTag,
        targetKind: request.targetKind ?? "html",
        position: request.position,
        elementTag: request.element.tag,
      },
      insertIntent: {
        targetSourceId,
        targetTag: request.targetTag,
        targetKind: request.targetKind ?? "html",
        position: request.position,
        element: {
          kind: request.element.kind ?? "html",
          blockId,
          tag: options.tag,
          className: options.className,
          text: options.text,
          label: request.element.label,
        },
      },
    }, previewStructuralCommandIdentity(lease));

    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("html-actions-insert-engine-blocked"),
    );
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      cacheCommittedHtmlPatch(host, patch);
      host.structureStatus = t("html-actions-inserted-saved", {
        tag: patch.tag,
        position: label,
      });
    });
    return committedAction();
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.structureStatus = t("html-actions-insert-failed", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-insert-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

function generatedPanaClass(className: string) {
  return /^ps-[a-z0-9-]+-[a-z0-9]{6,}$/i.test(className.trim());
}

export async function generateClassForSelectedHtml(
  host: HtmlActionsControllerHost,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target) {
    host.classStatus = t("html-actions-class-select");
    return blockedAction(host.classStatus);
  }
  if (hasMultiElementSelection(host)) {
    host.setHtmlPending("classes", true);
    host.classStatus = t("inspector-applying");
    try {
      const result = await runInPreviewStructuralLane(host, async (lease) => {
        if (!lease.selection || lease.selection.members.length < 2) {
          return blockedAction("Generarea clasei comune cere cel puțin două elemente selectate.");
        }
        const receipt = await executePreviewSelectionBatchIntent({
          schemaVersion: 1,
          action: { kind: "generateSharedClass" },
        }, previewStructuralCommandIdentity(lease, true));
        if (receipt.status !== "committed" || !receipt.generatedClass) {
          return blockedAction(
            receipt.diagnostics[0] || "Kernelul nu a confirmat clasa comună generată.",
          );
        }
        await projectCommittedPreviewSelectionBatchMutation(host, lease, receipt);
        const classes = normalizeClassTokens(host.classEditorValue);
        if (!classes.includes(receipt.generatedClass)) {
          host.classEditorValue = [...classes, receipt.generatedClass].join(" ");
        }
        return committedAction();
      });
      const outcome = result ?? cancelledAction("Generarea clasei comune a fost anulată.");
      host.classStatus = editorActionSucceeded(outcome)
        ? t("html-actions-classes-applied")
        : (outcome.reason ?? t("html-actions-classes-kernel-refused"));
      return outcome;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.classStatus = result.reason ?? result.status;
      host.setGlobalStatus(host.classStatus, "error");
      return result;
    } finally {
      host.setHtmlPending("classes", false);
    }
  }
  return await generateIdentityForTarget(host, target, "class");
}

export async function generateDataAnimForSelectedHtml(
  host: HtmlActionsControllerHost,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target) {
    host.attributeStatus = t("html-actions-data-anim-select");
    return blockedAction(host.attributeStatus);
  }
  return await generateIdentityForTarget(host, target, "dataAnim");
}

async function generateIdentityForTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
  kind: ProjectGeneratedIdentityKind,
): Promise<EditorActionOutcome> {
  const pendingArea: HtmlPendingArea = kind === "class" ? "classes" : "attributes";
  const baselineClasses = normalizeClassTokens(host.classEditorValue).join(" ");
  const baselineAttributeValues = Object.freeze({ ...host.attributeValues });
  const baselineAttributes = attributeDraftToken(baselineAttributeValues);
  host.setHtmlPending(pendingArea, true);
  if (kind === "class") host.classStatus = t("inspector-applying");
  else host.attributeStatus = t("inspector-applying");

  try {
    const result = await executeSelectedHtmlAttributes(
      host,
      target,
      {},
      (patch, capturedTarget) => {
        const projection = patch.generatedIdentity;
        if (!projection || projection.kind !== kind) {
          throw new Error(t("html-actions-generated-identity-receipt-invalid"));
        }
        if (!currentSelectionMatchesTarget(host, capturedTarget)) return;

        if (kind === "class") {
          const submittedClasses = normalizeClassTokens(projection.classes.join(" ")).join(" ");
          const currentClasses = normalizeClassTokens(host.classEditorValue).join(" ");
          if (committedDraftCanSettle(currentClasses, submittedClasses, baselineClasses)) {
            host.classEditorValue = submittedClasses;
          }
          host.classStatus = projection.alreadyPresent
            ? t("html-actions-class-already-generated", { name: projection.value })
            : t("html-actions-classes-applied");
          return;
        }

        const nextAttributes = { ...baselineAttributeValues };
        if (projection.dataAnim) nextAttributes["data-anim"] = projection.dataAnim;
        else delete nextAttributes["data-anim"];
        const submittedAttributes = attributeDraftToken(nextAttributes);
        if (committedDraftCanSettle(
          attributeDraftToken(host.attributeValues),
          submittedAttributes,
          baselineAttributes,
        )) {
          host.attributeValues = nextAttributes;
        }
        host.attributeStatus = projection.alreadyPresent
          ? t("html-actions-data-anim-exists", { value: projection.value })
          : t("html-actions-attributes-applied");
      },
      null,
      null,
      null,
      { kind },
    );
    host.setHtmlPending(pendingArea, false);
    if (!editorActionSucceeded(result)) {
      const reason = result.reason ?? t("html-actions-attributes-kernel-refused");
      if (kind === "class") host.classStatus = reason;
      else host.attributeStatus = reason;
      host.setGlobalStatus(reason, "error");
    }
    return result;
  } catch (error) {
    host.setHtmlPending(pendingArea, false);
    const result = actionErrorOutcome(error);
    const message = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    if (kind === "class") host.classStatus = message;
    else host.attributeStatus = message;
    host.setGlobalStatus(message, "error");
    return result;
  }
}

export async function insertNodeRelative(
  host: HtmlActionsControllerHost,
  position: InsertPosition,
  opts: { tag: string; className: string; text: string },
) {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  const capturedOptions = Object.freeze({ ...opts });
  const canEditHtmlStructure = host.canEditHtmlStructure;
  const canAddChild = host.canAddChildToSelectedElement;
  try {
    await runInPreviewStructuralLane(host, (lease) =>
      insertNodeRelativeInLane(
        host,
        target,
        position,
        capturedOptions,
        canEditHtmlStructure,
        canAddChild,
        lease,
      ));
  } catch (error) {
    host.structureStatus = t("html-actions-generic-error", { message: errorMessage(error) });
    host.setGlobalStatus(t("html-actions-insert-error", {
      message: errorMessage(error),
    }), "error");
  }
}

async function insertNodeRelativeInLane(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget | null,
  position: InsertPosition,
  opts: { tag: string; className: string; text: string },
  canEditHtmlStructure: boolean,
  canAddChild: boolean,
  lease: PreviewStructuralSessionLease,
) {
  if (!target) {
    host.structureStatus = t("html-actions-insert-select");
    return;
  }
  if (!canEditHtmlStructure) {
    host.structureStatus = t("html-actions-switch-preview-or-code");
    return;
  }
  if (position === "child" && !canAddChild) {
    host.structureStatus = t("html-actions-selected-no-children");
    return;
  }

  if (!target.sourceId) {
    const message = missingKernelIdentityMessage(t("html-actions-insert-noun"));
    host.structureStatus = message;
    host.setGlobalStatus(message, "error");
    return;
  }

  try {
    const insertPosition = position === "child" ? "inside" : position;
    const receipt = await executePreviewHtmlInsertDropIntent({
      intent: {
        messageType: "preview-insert-drop",
        targetSourceId: target.sourceId,
        targetTemplateSourceId: target.templateSourceId,
        targetSessionId: target.sessionId,
        targetTag: target.tag,
        targetKind: "html",
        position: insertPosition,
        elementTag: opts.tag,
      },
      insertIntent: {
        targetSourceId: target.sourceId,
        targetTag: target.tag,
        targetKind: "html",
        position: insertPosition,
        element: {
          kind: "html",
          blockId: null,
          tag: opts.tag,
          className: opts.className,
          text: opts.text,
          label: t("html-actions-element-label", { tag: opts.tag }),
        },
      },
    }, previewStructuralCommandIdentity(lease, true));

    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      t("html-actions-insert-engine-blocked"),
    );
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, () => {
      cacheCommittedHtmlPatch(host, patch);
      const label = position === "before"
        ? t("html-actions-position-before")
        : position === "after"
          ? t("html-actions-position-after")
          : t("html-actions-position-child");
      host.structureStatus = t("html-actions-inserted", {
        tag: patch.tag,
        position: label,
      });
    });
  } catch (error) {
    host.structureStatus = t("html-actions-generic-error", {
      message: errorMessage(error),
    });
    host.setGlobalStatus(t("html-actions-insert-error", {
      message: errorMessage(error),
    }), "error");
  }
}

export async function applyImageSourceToHtml(
  host: HtmlActionsControllerHost,
  sourceOverride?: string,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target || target.tag !== "img") {
    host.imageStatus = t("html-actions-image-select");
    return blockedAction(host.imageStatus);
  }

  const src = (sourceOverride ?? host.imageSourceValue).trim();
  host.imageSourceValue = src;
  const zolaImage = host.coordinatedElementSelection?.observation.zolaImage ?? null;
  if (zolaImage) {
    const source = resolveZolaImageSource(src, host.scannedProject?.files ?? []);
    if (!source.eligible) {
      host.imageStatus = zolaImageSourceFailureMessage(source.code);
      return blockedAction(host.imageStatus);
    }
    return await applyZolaImageProcessingToHtml(host, createZolaImageIntent({
      enabled: true,
      source,
      width: zolaImage.width,
      height: zolaImage.height,
      operation: zolaImage.operation,
      format: zolaImage.format,
      quality: zolaImage.quality,
    }));
  }
  if ((target.attributes?.src ?? "").trim() === src) {
    host.setHtmlPending("image", false);
    host.imageStatus = t("html-actions-image-no-changes");
    return noopAction(host.imageStatus);
  }
  host.setHtmlPending("image", true);
  try {
    const result = await executeSelectedHtmlAttributes(host, target, { src: src || null }, (patch, capturedTarget) => {
      if (currentSelectionMatchesTarget(host, capturedTarget)) {
        host.imageSourceValue = src;
        host.imageStatus = t("html-actions-image-applied");
      }
    });
    if (
      editorActionSucceeded(result)
      && currentSelectionMatchesTarget(host, target)
      && host.imageSourceValue.trim() === src
    ) {
      host.setHtmlPending("image", false);
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.imageStatus = t("html-actions-image-failed", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-image-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function applyClassesToHtml(host: HtmlActionsControllerHost): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target) {
    host.classStatus = t("html-actions-classes-select");
    return blockedAction(host.classStatus);
  }

  const normalizedClasses = normalizeClassTokens(host.classEditorValue);
  if (hasMultiElementSelection(host)) {
    const primaryClasses = normalizeClassTokens(target.classes?.join(" ") ?? "");
    const nextClasses = new Set(normalizedClasses);
    const previousClasses = new Set(primaryClasses);
    const add = normalizedClasses.filter((className) => !previousClasses.has(className));
    const remove = primaryClasses.filter((className) => !nextClasses.has(className));
    if (add.length === 0 && remove.length === 0) {
      host.setHtmlPending("classes", false);
      host.classStatus = t("html-actions-classes-no-changes");
      return noopAction(host.classStatus);
    }
    host.setHtmlPending("classes", true);
    try {
      const result = await executeSelectionBatch(host, {
        kind: "mutateClasses",
        add,
        remove,
      });
      host.classStatus = editorActionSucceeded(result)
        ? t("html-actions-classes-applied")
        : (result.reason ?? t("html-actions-classes-kernel-refused"));
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.classStatus = result.reason ?? result.status;
      host.setGlobalStatus(host.classStatus, "error");
      return result;
    } finally {
      host.setHtmlPending("classes", false);
    }
  }
  return await applyClassesToTarget(host, target, normalizedClasses);
}

async function applyClassesToTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
  classes: readonly string[],
  options: { markPending?: boolean } = {},
): Promise<EditorActionOutcome> {
  const normalizedClasses = normalizeClassTokens(classes.join(" "));
  const submittedClasses = normalizedClasses.join(" ");
  const baselineClasses = normalizeClassTokens(host.classEditorValue).join(" ");
  const targetClassTokens = normalizeClassTokens(target.classes?.join(" ") ?? "");
  const targetClasses = targetClassTokens.join(" ");
  const submittedClassSet = new Set(normalizedClasses);
  const removedGeneratedClass = targetClassTokens.some(
    (className) => generatedPanaClass(className) && !submittedClassSet.has(className),
  );
  if (submittedClasses === targetClasses) {
    host.setHtmlPending("classes", false);
    host.classStatus = t("html-actions-classes-no-changes");
    return noopAction(host.classStatus);
  }
  if (options.markPending !== false) {
    host.setHtmlPending("classes", true);
  }
  let submittedDraftProjected = false;
  try {
    const result = await executeSelectedHtmlAttributes(
      host,
      target,
      { class: submittedClasses || null },
      async (patch, capturedTarget) => {
        const currentClasses = normalizeClassTokens(host.classEditorValue).join(" ");
        if (
          currentSelectionMatchesTarget(host, capturedTarget)
          && committedDraftCanSettle(currentClasses, submittedClasses, baselineClasses)
        ) {
          host.classEditorValue = submittedClasses;
          host.classStatus = t("html-actions-classes-applied");
          submittedDraftProjected = true;
        }
        if (removedGeneratedClass && isZolaTemplatePath(patch.file)) {
          await reconcilePageAssetContracts(host, patch.targetLocation);
        }
      },
    );
    if (
      editorActionSucceeded(result)
      && normalizeClassTokens(host.classEditorValue).join(" ") === submittedClasses
      && (submittedDraftProjected || currentSelectionMatchesTarget(host, target))
    ) {
      host.setHtmlPending("classes", false);
    }
    if (!editorActionSucceeded(result)) {
      const reason = result.reason ?? t("html-actions-classes-kernel-refused");
      host.classStatus = t("html-actions-classes-not-applied", { reason });
      host.setGlobalStatus(host.classStatus, "error");
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.classStatus = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-classes-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function openSourceLocation(host: HtmlActionsControllerHost, source: string) {
  const relativePath = parseSourceEditLocation(source)?.file ?? source;
  if (!host.scannedProject) return;
  const file = host.scannedProject.files.find(
    (item) => item.relativePath === relativePath || zolaRelativePath(item.relativePath) === relativePath,
  );
  if (file) {
    await host.loadScannedProjectFile(file);
  }
}

export async function applyAttributesToHtml(
  host: HtmlActionsControllerHost,
  attributeOverride: EditableAttributes | null = null,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target) {
    host.attributeStatus = t("html-actions-attributes-select");
    return blockedAction(host.attributeStatus);
  }

  const attributeValues = Object.freeze({
    ...(attributeOverride ?? host.attributeValues),
  });
  if (hasMultiElementSelection(host)) {
    const mutations = batchCommonAttributeMutations(attributeValues, target.attributes ?? {});
    if (mutations.length === 0) {
      host.setHtmlPending("attributes", false);
      host.attributeStatus = t("inspector-no-canonical-difference");
      return noopAction(host.attributeStatus);
    }
    host.setHtmlPending("attributes", true);
    try {
      const result = await executeSelectionBatch(host, {
        kind: "setAttributes",
        attributes: mutations,
      });
      host.attributeStatus = editorActionSucceeded(result)
        ? t("html-actions-attributes-applied")
        : (result.reason ?? t("html-actions-attributes-kernel-refused"));
      return result;
    } catch (error) {
      const result = actionErrorOutcome(error);
      host.attributeStatus = result.reason ?? result.status;
      host.setGlobalStatus(host.attributeStatus, "error");
      return result;
    } finally {
      host.setHtmlPending("attributes", false);
    }
  }
  return await applyAttributesToTarget(host, target, attributeValues);
}

async function applyAttributesToTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
  capturedAttributeValues: Readonly<EditableAttributes>,
  options: { markPending?: boolean } = {},
): Promise<EditorActionOutcome> {
  const attributeValues: EditableAttributes = { ...capturedAttributeValues };
  const baselineAttributeDraft = attributeDraftToken(host.attributeValues);
  const submittedAttributeDraft = attributeDraftToken(attributeValues);
  const nextKernelAttributes = htmlAttributeRecordForKernel(
    attributeValues,
    target.attributes,
    Boolean(target.zolaImage),
  );
  const targetDataAnim = target.attributes?.["data-anim"]?.trim() ?? "";
  const submittedDataAnim = nextKernelAttributes["data-anim"]?.trim() ?? "";
  const removedOrReplacedDataAnim = targetDataAnim.length > 0 && targetDataAnim !== submittedDataAnim;

  if (options.markPending !== false) {
    host.setHtmlPending("attributes", true);
  }
  let submittedDraftProjected = false;
  try {
    const result = await executeSelectedHtmlAttributes(host, target, nextKernelAttributes, async (patch, capturedTarget) => {
      if (
        currentSelectionMatchesTarget(host, capturedTarget)
        && committedDraftCanSettle(
          attributeDraftToken(host.attributeValues),
          submittedAttributeDraft,
          baselineAttributeDraft,
        )
      ) {
        host.attributeValues = { ...attributeValues };
        host.attributeStatus = t("html-actions-attributes-applied");
        submittedDraftProjected = true;
      }
      if (removedOrReplacedDataAnim && isZolaTemplatePath(patch.file)) {
        await reconcilePageAssetContracts(host, patch.targetLocation);
      }
    });
    if (
      editorActionSucceeded(result)
      && attributeDraftMatches(host.attributeValues, attributeValues)
      && (submittedDraftProjected || currentSelectionMatchesTarget(host, target))
    ) {
      host.setHtmlPending("attributes", false);
    }
    if (!editorActionSucceeded(result)) {
      const reason = result.reason ?? t("html-actions-attributes-kernel-refused");
      host.attributeStatus = t("html-actions-attributes-not-applied", { reason });
      host.setGlobalStatus(host.attributeStatus, "error");
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.attributeStatus = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-attributes-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}

export async function applyAttributesToCapturedHtmlTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
  attributeValues: Readonly<EditableAttributes>,
): Promise<EditorActionOutcome> {
  return await applyAttributesToTarget(host, target, attributeValues);
}

export async function applyTextContentToHtml(host: HtmlActionsControllerHost): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target) {
    host.textStatus = t("html-actions-text-select");
    return blockedAction(host.textStatus);
  }
  if (target.hasChildElements) {
    host.textStatus = t("html-actions-text-simple-only");
    return blockedAction(host.textStatus);
  }
  return await applyTextContentToCapturedHtmlTarget(
    host,
    target,
    host.textContentValue,
  );
}

export async function applyTextContentToCapturedHtmlTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
  committedText: string,
  options: {
    deferCanonicalProjection?: boolean;
    editSessionId?: string | null;
  } = {},
): Promise<EditorActionOutcome> {
  if (target.hasChildElements) {
    host.textStatus = t("html-actions-text-simple-only");
    return blockedAction(host.textStatus);
  }
  if (!target.sourceId) {
    host.textStatus = missingKernelIdentityMessage(t("html-actions-text-noun"));
    return blockedAction(host.textStatus);
  }
  const selectionKey = [
    target.sessionId ?? "",
    target.selectionRevision ?? "",
    target.sourceId,
    target.renderInstanceId ?? "",
  ].join("::");
  const previousText =
    host.textEditOriginalKey === selectionKey
      ? host.textEditOriginalText ?? ""
      : target.rawText ?? "";
  if (!options.deferCanonicalProjection && committedText === previousText) {
    host.textEditOriginalKey = null;
    host.textEditOriginalText = null;
    host.setHtmlPending("text", false);
    host.textStatus = t("html-actions-text-no-changes");
    return noopAction(host.textStatus);
  }

  host.setHtmlPending("text", true);
  try {
    const result = await executeSelectedHtmlText(host, target, committedText, (_patch, capturedTarget) => {
      if (
        currentSelectionMatchesTarget(host, capturedTarget)
        && host.textContentValue === committedText
      ) {
        host.textStatus = options.deferCanonicalProjection
          ? t("html-actions-text-confirmed-recoverable")
          : t("html-actions-text-applied");
        if (!options.deferCanonicalProjection) {
          host.textEditOriginalKey = null;
          host.textEditOriginalText = null;
        }
      }
      // The committed text is owned by ProjectWorkspace history. The canonical
      // Preview projection performs the frontend history handoff.
    }, options);
    if (
      editorActionSucceeded(result)
      && currentSelectionMatchesTarget(host, target)
      && host.textContentValue === committedText
    ) {
      host.setHtmlPending("text", false);
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.textStatus = t("html-actions-generic-error", {
      message: result.reason ?? result.status,
    });
    host.setGlobalStatus(t("html-actions-text-error", {
      message: result.reason ?? result.status,
    }), "error");
    return result;
  }
}
