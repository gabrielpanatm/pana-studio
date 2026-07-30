import {
  canElementAcceptChildren,
  htmlVoidTags,
  normalizeClassTokens,
  type InsertPosition,
} from "$lib/html/mutations";
import { parseHtmlSourceNodes } from "$lib/html/parser";
import { generateUniqueHtmlIdentity } from "$lib/html/generated-identity";
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
  projectCommittedPreviewStructuralMutation,
  previewStructuralBlockingDiagnostic,
  requireCommittedPreviewStructuralPatch,
  type PreviewStructuralCanonicalProjectionHost,
  type PreviewStructuralExecutionReceipt,
} from "$lib/kernel/preview-projection-control";
import {
  capturePreviewStructuralSessionLease,
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
  executePreviewHtmlTextIntent,
  readProjectFile,
  readProjectWorkspaceState,
} from "$lib/project/io";
import { committedDraftCanSettle } from "$lib/session/committed-draft-settlement";
import { settleProjectWorkspaceMutation } from "$lib/session/workspace-mutation-coordinator";
import {
  formatSourceEditLocation,
  parseSourceEditLocation,
  sourceLocationForEditTarget,
} from "$lib/source-graph/location";
import type { PreviewInsertDropRequest } from "$lib/state/preview-insert-controller";
import type {
  EditableAttributes,
  HtmlPendingArea,
  NativeBlockOptionIntent,
  ProjectHtmlAttributePatch,
  ProjectHtmlAttributeMutation,
  ProjectZolaImageIntent,
  ProjectHtmlTextPatch,
  ProjectFile,
  ProjectDiskManifest,
  ProjectScan,
  CoordinatedElementSelection,
  SourceEditLocation,
  SourceEditTarget,
  ZolaImagePresentation,
} from "$lib/types";
import type { GlobalStatusKind } from "$lib/status/global-status";
import { errorMessage } from "$lib/util";
import { t } from "$lib/i18n/runtime.svelte";

export type HtmlActionsControllerHost = PreviewStructuralCanonicalProjectionHost & {
  coordinatedElementSelection: CoordinatedElementSelection | null;
  pageSections: { selector: string; tag: string; sourceId?: string | null; sourceLocation?: SourceEditLocation | null; sessionId?: string | null }[];
  structureStatus: string;
  canEditHtmlStructure: boolean;
  canAddChildToSelectedElement: boolean;
  imageStatus: string;
  isActivePreviewHtmlSource: boolean;
  activeScannedPath: string | null;
  source: string;
  htmlSourceMutationBlockedReason: string;
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
  currentHtmlRelativePath: string;
  stageKernelPlannedTemplateDraft: (
    tpl: SourceEditLocation,
    plannedSource: string,
    options?: { pendingArea?: HtmlPendingArea; status?: string; isCurrent?: () => boolean },
  ) => Promise<string | null>;
  resolveSourceEditTargetForSourceId: (sourceId: string | null | undefined) => SourceEditTarget | null;
  getPreviewDocument: () => Document | undefined;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  loadScannedProjectFile: (file: ProjectFile) => Promise<void>;
};

export type HtmlActionTarget = {
  selector: string;
  cssSelector?: string | null;
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
      selector: observation.domPath,
      cssSelector: observation.cssSelector,
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
    const section = target.section ?? null;
    return freezeHtmlActionTarget({
      selector: target.selector,
      cssSelector: observation?.cssSelector ?? null,
      tag: target.tag,
      selectionRevision: target.selectionRevision ?? null,
      renderInstanceId: target.renderInstanceId ?? null,
      sourceId: target.sourceId ?? section?.sourceId ?? null,
      templateSourceId:
        target.templateSourceId
        ?? section?.templateSourceId
        ?? null,
      sourceLocation: target.sourceLocation ?? section?.sourceLocation ?? null,
      sessionId: target.sessionId ?? section?.sessionId ?? null,
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
  if (target.sourceId && current.sourceNodeId) {
    return target.sourceId === current.sourceNodeId
      && target.selector === current.observation.domPath;
  }
  return target.selector === current.observation.domPath;
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

function missingKernelLocationMessage(action: string) {
  return t("html-actions-location-missing", { action });
}

function projectSourceLocation(tpl: SourceEditLocation) {
  return {
    file: tpl.file,
    line: tpl.line,
    column: tpl.column ?? 0,
  };
}

function isIdentityScanFile(file: ProjectFile) {
  return ["HTML", "MD", "CSS", "SCSS", "JS"].includes(file.kind);
}

async function collectIdentitySourceTexts(host: HtmlActionsControllerHost) {
  const texts: string[] = [];
  const seen = new Set<string>();

  for (const [cacheKey, value] of Object.entries(host.sourceCache)) {
    if (typeof value !== "string") continue;
    texts.push(value);
    if (cacheKey.startsWith("scanned:")) seen.add(cacheKey.slice("scanned:".length));
  }

  const files = host.scannedProject?.files.filter(isIdentityScanFile) ?? [];
  const reads = files
    .filter((file) => !seen.has(file.relativePath))
    .map(async (file) => {
      const cached = host.sourceCache[scannedCacheKey(file)];
      if (typeof cached === "string") return cached;
      return await readProjectFile(file.relativePath);
    });

  const settled = await Promise.allSettled(reads);
  for (const result of settled) {
    if (result.status === "fulfilled") texts.push(result.value);
  }

  return texts;
}

function sourceLocationAtOffset(file: string, sourceText: string, offset: number): SourceEditLocation {
  const before = sourceText.slice(0, Math.max(0, offset));
  const lines = before.split("\n");
  const linePrefix = lines[lines.length - 1] ?? "";
  return {
    file,
    line: lines.length,
    column: Array.from(linePrefix).length + 1,
  };
}

function selectorVariants(selector: string) {
  const trimmed = selector.trim();
  const variants = new Set<string>();
  if (!trimmed) return variants;
  variants.add(trimmed);

  const htmlPrefix = "html:nth-of-type(1) > ";
  if (trimmed.startsWith(htmlPrefix)) {
    variants.add(trimmed.slice(htmlPrefix.length));
  } else {
    variants.add(`${htmlPrefix}${trimmed}`);
  }

  return variants;
}

function currentActiveHtmlSource(host: HtmlActionsControllerHost) {
  const cacheKey = scannedCacheKey({ relativePath: host.currentHtmlRelativePath });
  if (host.activeScannedPath === host.currentHtmlRelativePath) {
    return host.source || host.sourceCache[cacheKey] || "";
  }
  return host.sourceCache[cacheKey] || "";
}

function activeHtmlSourceLocationForTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
): SourceEditLocation | null {
  if (!host.isActivePreviewHtmlSource || !host.currentHtmlRelativePath) return null;

  const sourceText = currentActiveHtmlSource(host);
  if (!sourceText) return null;

  const variants = selectorVariants(target.selector);
  const nodes = parseHtmlSourceNodes(sourceText, htmlVoidTags);
  const selectorMatches = nodes.filter((node) =>
    node.tag === target.tag && variants.has(node.selector),
  );
  const selected =
    selectorMatches.length === 1
      ? selectorMatches[0]
      : target.cssSelector
        ? uniqueSourceNode(nodes.filter((node) =>
            node.tag === target.tag && node.cssSelector === target.cssSelector,
          ))
        : null;

  return selected
    ? sourceLocationAtOffset(host.currentHtmlRelativePath, sourceText, selected.openStart)
    : null;
}

function uniqueSourceNode<T>(items: T[]) {
  return items.length === 1 ? items[0] : null;
}

function sourceLocationForSourceReference(
  host: HtmlActionsControllerHost,
  sourceId: string | null | undefined,
  fallbackSourceLocation?: SourceEditLocation | null,
): SourceEditLocation | null {
  const target = host.resolveSourceEditTargetForSourceId(sourceId);
  if (target) return sourceLocationForEditTarget(target);
  return fallbackSourceLocation ?? null;
}

function sourceLocationForSessionReference(
  host: HtmlActionsControllerHost,
  sessionId: string | null | undefined,
  capturedTarget?: HtmlActionTarget | null,
): SourceEditLocation | null {
  if (!sessionId) return null;
  if (capturedTarget?.sessionId === sessionId && capturedTarget.sourceLocation) {
    return capturedTarget.sourceLocation;
  }
  return host.pageSections.find((section) => section.sessionId === sessionId)?.sourceLocation ?? null;
}

function sourceLocationForInsertTarget(
  host: HtmlActionsControllerHost,
  request: PreviewInsertDropRequest,
  targetSourceId: string | null,
  capturedTarget?: HtmlActionTarget | null,
): SourceEditLocation | null {
  if (request.targetKind !== "empty-tera-slot") {
    const sessionLocation = sourceLocationForSessionReference(
      host,
      request.targetSessionId,
      capturedTarget,
    );
    if (sessionLocation) return sessionLocation;
  }
  return sourceLocationForSourceReference(host, targetSourceId, request.targetSourceLocation);
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
): Promise<EditorActionOutcome> {
  const result = await runInPreviewStructuralLane(host, async (lease) => {
    const location = sourceLocationForSourceReference(
      host,
      target.sourceId,
      target.sourceLocation,
    ) ?? activeHtmlSourceLocationForTarget(host, target);
    if (!location) {
      throw new Error(missingKernelLocationMessage(t("html-actions-attributes-noun")));
    }

    const receipt = await executePreviewHtmlAttributesIntent({
      intent: {
        messageType: "preview-html-attributes",
        selector: target.selector,
        sourceId: target.sourceId,
        sourceTag: target.tag,
      },
      attributeIntent: {
        targetSourceId: target.sourceId ?? null,
        targetLocation: projectSourceLocation(location),
        targetTag: target.tag,
        targetSelector: target.selector,
        attributes: attributeMutationsFromRecord(attributes),
        ...(zolaImage ? { zolaImage } : {}),
        ...(nativeBlockOption ? { nativeBlockOption } : {}),
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
  rootSelector: string;
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
    selector: request.rootSelector,
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
    const location = groupedEditSession && target.sourceLocation
      ? target.sourceLocation
      : sourceLocationForSourceReference(
        host,
        target.sourceId,
        target.sourceLocation,
      ) ?? activeHtmlSourceLocationForTarget(host, target);
    if (!location) throw new Error(missingKernelLocationMessage(t("html-actions-text-noun")));

    const receipt = await executePreviewHtmlTextIntent({
      intent: {
        messageType: "preview-html-text",
        selector: target.selector,
        sourceId: target.sourceId,
        sourceTag: target.tag,
      },
      textIntent: {
        targetSourceId: target.sourceId ?? null,
        // A grouped edit is a long-lived Rust-owned logical identity. Once
        // the first text mutation changes Source Graph byte ranges, combining
        // its aliased Source ID with a newly resolved physical location can
        // describe two different nodes. Keep the ID as the sole semantic
        // anchor; direct HTML targets without Source IDs still use location.
        targetLocation: groupedEditSession && target.sourceId
          ? null
          : projectSourceLocation(location),
        targetTag: target.tag,
        targetSelector: target.selector,
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
  const capturedTarget = captureHtmlActionTarget(editorTarget ?? host.coordinatedElementSelection);
  try {
    const result = await runInPreviewStructuralLane(host, async (lease) => {
      const target = capturedTarget;
      if (!target) {
        host.structureStatus = t("html-actions-delete-select");
        host.setGlobalStatus(host.structureStatus, "error");
        return blockedAction(host.structureStatus);
      }

      const targetSelector = target.selector;
      const tpl = sourceLocationForSourceReference(host, target.sourceId, target.sourceLocation);
      const kernelTargetLocation = tpl ?? activeHtmlSourceLocationForTarget(host, target);

      if (!kernelTargetLocation) {
        const message = host.isActivePreviewHtmlSource
          ? missingKernelLocationMessage(t("html-actions-delete-noun"))
          : host.htmlSourceMutationBlockedReason || t("html-actions-source-not-editable");
        host.structureStatus = message;
        host.setGlobalStatus(message, "error");
        return blockedAction(message);
      }

      const receipt = await executePreviewHtmlDeleteIntent({
        intent: {
          messageType: "preview-delete-selected",
          selector: targetSelector,
          sourceId: target.sourceId ?? null,
          sourceTag: target.tag,
        },
        deleteIntent: {
          targetSourceId: target.sourceId ?? null,
          targetLocation: projectSourceLocation(kernelTargetLocation),
          targetTag: target.tag,
          targetSelector,
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

      const targetSelector = target.selector;
      const tpl = sourceLocationForSourceReference(host, target.sourceId, target.sourceLocation);
      const kernelSourceLocation = tpl ?? activeHtmlSourceLocationForTarget(host, target);
      if (!kernelSourceLocation) {
        const message = host.isActivePreviewHtmlSource
          ? missingKernelLocationMessage(t("html-actions-duplicate-noun"))
          : host.htmlSourceMutationBlockedReason || t("html-actions-source-not-editable");
        host.structureStatus = message;
        host.setGlobalStatus(message, "error");
        return blockedAction(message);
      }

      const receipt = await executePreviewHtmlDuplicateIntent({
        intent: {
          messageType: "preview-duplicate-selected",
          selector: targetSelector,
          sourceId: target.sourceId ?? null,
          sourceTag: target.tag,
        },
        duplicateIntent: {
          sourceSourceId: target.sourceId ?? null,
          sourceLocation: projectSourceLocation(kernelSourceLocation),
          sourceTag: target.tag,
          sourceSelector: targetSelector,
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

export async function insertPaletteElementAtTarget(
  host: HtmlActionsControllerHost,
  request: PreviewInsertDropRequest,
) {
  const capturedRequest = Object.freeze({
    ...request,
    targetSourceLocation: request.targetSourceLocation
      ? Object.freeze({ ...request.targetSourceLocation })
      : null,
    element: Object.freeze({ ...request.element }),
  });
  const capturedTarget = captureHtmlActionTarget(host.coordinatedElementSelection);
  try {
    await runInPreviewStructuralLane(host, (lease) =>
      insertPaletteElementAtTargetInLane(host, capturedRequest, capturedTarget, lease));
  } catch (error) {
    host.structureStatus = t("html-actions-insert-failed", {
      message: errorMessage(error),
    });
    host.setGlobalStatus(t("html-actions-insert-error", {
      message: errorMessage(error),
    }), "error");
  }
}

async function insertPaletteElementAtTargetInLane(
  host: HtmlActionsControllerHost,
  request: PreviewInsertDropRequest,
  capturedTarget: HtmlActionTarget | null,
  lease: PreviewStructuralSessionLease,
) {
  const targetSourceId = request.targetSourceId ||
    (request.targetKind === "empty-tera-slot" ? request.targetTemplateSourceId : null);
  const targetTpl = sourceLocationForInsertTarget(
    host,
    request,
    targetSourceId,
    capturedTarget,
  );
  const targetLocation = targetTpl ?? activeHtmlSourceLocationForTarget(host, {
    selector: request.targetSelector,
    tag: request.targetTag,
    sourceId: targetSourceId,
    sourceLocation: request.targetSourceLocation,
    sessionId: request.targetSessionId,
  });
  if (!host.canEditHtmlStructure && !targetLocation) {
    host.structureStatus = t("html-actions-switch-preview");
    host.setGlobalStatus(host.structureStatus, "error");
    return;
  }
  if (request.position === "inside" && !canElementAcceptChildren(request.targetTag, htmlVoidTags)) {
    host.structureStatus = t("html-actions-target-no-children");
    host.setGlobalStatus(host.structureStatus, "error");
    return;
  }

  if (!targetLocation) {
    host.structureStatus = t("html-actions-target-metadata-unstable");
    host.setGlobalStatus(host.structureStatus, "error");
    return;
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
        targetSelector: request.targetSelector,
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
        targetLocation: projectSourceLocation(targetLocation),
        targetTag: request.targetTag,
        targetSelector: request.targetSelector,
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
  } catch (error) {
    host.structureStatus = t("html-actions-insert-failed", {
      message: errorMessage(error),
    });
    host.setGlobalStatus(t("html-actions-insert-error", {
      message: errorMessage(error),
    }), "error");
  }
}

function generatedPanaClass(className: string) {
  return /^ps-[a-z0-9-]+-[a-z0-9]{6,}$/i.test(className.trim());
}

function existingGeneratedClass(
  classEditorValue: string,
  target: HtmlActionTarget,
) {
  return normalizeClassTokens(classEditorValue || target.classes?.join(" ") || "")
    .find(generatedPanaClass) ?? null;
}

function validClassToken(value: string) {
  return /^[A-Za-z_-][A-Za-z0-9_-]*$/.test(value);
}

export async function generateClassForSelectedHtml(
  host: HtmlActionsControllerHost,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target) {
    host.classStatus = t("html-actions-class-select");
    return blockedAction(host.classStatus);
  }
  let sessionLease: PreviewStructuralSessionLease;
  try {
    sessionLease = capturePreviewStructuralSessionLease(host);
  } catch (error) {
    if (isPreviewStructuralCancellation(error)) return cancelledAction(errorMessage(error));
    throw error;
  }

  const classEditorValue = host.classEditorValue;
  const attributeValues = Object.freeze({ ...host.attributeValues });
  const currentClasses = normalizeClassTokens(classEditorValue || target.classes?.join(" ") || "");
  const existing = currentClasses.find(generatedPanaClass);
  if (existing) {
    host.classStatus = t("html-actions-class-already-generated", { name: existing });
    return noopAction(host.classStatus);
  }

  const currentDataAnim = attributeValues["data-anim"]?.trim() ?? "";
  const reusableDataAnim = generatedPanaClass(currentDataAnim) && validClassToken(currentDataAnim) ? currentDataAnim : null;
  const identity = reusableDataAnim
    ? { className: reusableDataAnim }
    : generateUniqueHtmlIdentity(target.tag, await collectIdentitySourceTexts(host));
  if (!previewStructuralSessionLeaseMatches(host, sessionLease)) {
    return cancelledAction(t("html-actions-class-session-cancelled"));
  }
  return await applyClassesToTarget(
    host,
    target,
    [...currentClasses, identity.className],
    { markPending: false },
  );
}

export async function generateDataAnimForSelectedHtml(
  host: HtmlActionsControllerHost,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.coordinatedElementSelection);
  if (!target) {
    host.attributeStatus = t("html-actions-data-anim-select");
    return blockedAction(host.attributeStatus);
  }
  let sessionLease: PreviewStructuralSessionLease;
  try {
    sessionLease = capturePreviewStructuralSessionLease(host);
  } catch (error) {
    if (isPreviewStructuralCancellation(error)) return cancelledAction(errorMessage(error));
    throw error;
  }

  const attributeValues = Object.freeze({ ...host.attributeValues });
  const classEditorValue = host.classEditorValue;
  const currentDataAnim = attributeValues["data-anim"]?.trim() ?? "";
  if (currentDataAnim) {
    host.attributeStatus = t("html-actions-data-anim-exists", {
      value: currentDataAnim,
    });
    return noopAction(host.attributeStatus);
  }

  const reusableClass = existingGeneratedClass(classEditorValue, target);
  const identity = reusableClass
    ? { dataAnim: reusableClass }
    : generateUniqueHtmlIdentity(target.tag, await collectIdentitySourceTexts(host));
  if (!previewStructuralSessionLeaseMatches(host, sessionLease)) {
    return cancelledAction(t("html-actions-data-anim-session-cancelled"));
  }
  return await applyAttributesToTarget(
    host,
    target,
    {
      ...attributeValues,
      "data-anim": identity.dataAnim,
    },
    { markPending: false },
  );
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

  const tpl = sourceLocationForSourceReference(host, target.sourceId, target.sourceLocation);
  const targetSelector = target.selector;
  const kernelTargetLocation = tpl ?? activeHtmlSourceLocationForTarget(host, target);
  if (kernelTargetLocation) {
    try {
      const insertPosition = position === "child" ? "inside" : position;
      const receipt = await executePreviewHtmlInsertDropIntent({
        intent: {
          messageType: "preview-insert-drop",
          targetSelector,
          targetSourceId: target.sourceId ?? null,
          targetTemplateSourceId: target.templateSourceId,
          targetSessionId: target.sessionId,
          targetTag: target.tag,
          targetKind: "html",
          position: insertPosition,
          elementTag: opts.tag,
        },
        insertIntent: {
          targetSourceId: target.sourceId ?? null,
          targetLocation: projectSourceLocation(kernelTargetLocation),
          targetTag: target.tag,
          targetSelector,
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
    return;
  }

  const message = missingKernelLocationMessage(t("html-actions-insert-noun"));
  host.structureStatus = message;
  host.setGlobalStatus(message, "error");
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
  const selectionSourceKey =
    target.sourceId ??
    (target.sourceLocation ? formatSourceEditLocation(target.sourceLocation) : "");
  const selectionKey = `${selectionSourceKey}::${target.selector}`;
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
