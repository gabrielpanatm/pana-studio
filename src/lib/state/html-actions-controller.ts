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
  reconcilePageComponentContracts,
} from "$lib/page-components/contract";
import { reconcilePageAssetContracts } from "$lib/page-assets/contract";
import { isZolaTemplatePath, scannedCacheKey, zolaRelativePath } from "$lib/project/files";
import {
  executePreviewHtmlAttributesIntent,
  executePreviewHtmlDeleteIntent,
  executePreviewHtmlDuplicateIntent,
  executePreviewHtmlInsertDropIntent,
  executePreviewHtmlTextIntent,
  readProjectFile,
} from "$lib/project/io";
import { committedDraftCanSettle } from "$lib/session/committed-draft-settlement";
import { createDomPathSelector } from "$lib/preview/selection";
import {
  formatSourceEditLocation,
  parseSourceEditLocation,
  sourceLocationForEditTarget,
} from "$lib/source-graph/location";
import type { PreviewInsertDropRequest } from "$lib/state/preview-insert-controller";
import type {
  EditableAttributes,
  HtmlPendingArea,
  ProjectHtmlAttributePatch,
  ProjectHtmlAttributeMutation,
  ProjectHtmlTextPatch,
  ProjectFile,
  ProjectDiskManifest,
  ProjectScan,
  SaveState,
  SelectionInfo,
  SourceEditLocation,
  SourceEditTarget,
} from "$lib/types";
import { errorMessage } from "$lib/util";

export type HtmlActionsControllerHost = PreviewStructuralCanonicalProjectionHost & {
  selectedElement: SelectionInfo | null;
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
  pendingSelectionSelector: string | null;
  postPreviewMessage: (payload: Record<string, unknown>) => void;
  setHtmlPending: (area: HtmlPendingArea, pending: boolean) => void;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  loadScannedProjectFile: (file: ProjectFile) => Promise<void>;
};

export type HtmlActionTarget = {
  selector: string;
  cssSelector?: string | null;
  tag: string;
  sourceId?: string | null;
  templateSourceId?: string | null;
  sourceLocation?: SourceEditLocation | null;
  sessionId?: string | null;
  hasChildElements?: boolean;
  rawText?: string;
  attributes?: Readonly<Record<string, string>>;
  classes?: readonly string[];
  parentSelector?: string | null;
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
  target: SelectionInfo | EditorHtmlTarget | null | undefined,
): HtmlActionTarget | null {
  if (!target) return null;
  if ("kind" in target) {
    const selection = target.selection ?? null;
    const section = target.section ?? null;
    return freezeHtmlActionTarget({
      selector: target.selector,
      cssSelector: selection?.cssSelector ?? null,
      tag: target.tag,
      sourceId: target.sourceId ?? selection?.sourceId ?? section?.sourceId ?? null,
      templateSourceId:
        target.templateSourceId
        ?? selection?.templateSourceId
        ?? section?.templateSourceId
        ?? null,
      sourceLocation: selection?.sourceLocation ?? section?.sourceLocation ?? null,
      sessionId: target.sessionId ?? selection?.sessionId ?? section?.sessionId ?? null,
      hasChildElements: selection?.hasChildElements,
      rawText: selection?.rawText,
      attributes: selection?.attributes,
      classes: selection?.classes,
      parentSelector: selection?.parentNode?.selector ?? null,
    });
  }
  return freezeHtmlActionTarget({
    selector: target.domPath,
    cssSelector: target.cssSelector,
    tag: target.tag,
    sourceId: target.sourceId,
    templateSourceId: target.templateSourceId,
    sourceLocation: target.sourceLocation,
    sessionId: target.sessionId,
    hasChildElements: target.hasChildElements,
    rawText: target.rawText,
    attributes: target.attributes,
    classes: target.classes,
    parentSelector: target.parentNode?.selector ?? null,
  });
}

function currentSelectionMatchesTarget(
  host: HtmlActionsControllerHost,
  target: HtmlActionTarget,
) {
  const current = host.selectedElement;
  if (!current) return false;
  if (target.sessionId && current.sessionId) return target.sessionId === current.sessionId;
  if (target.sourceId && current.sourceId) {
    return target.sourceId === current.sourceId && target.selector === current.domPath;
  }
  return target.selector === current.domPath;
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
  const reason = previewStructuralBlockingDiagnostic(receipt)?.message
    || receipt.message
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
  if (position === "before") return "înainte";
  if (position === "after") return "după";
  return "în interior";
}

function missingKernelLocationMessage(action: string) {
  return `Nu pot executa ${action}: ținta nu are locație sursă unică pentru kernel.`;
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

function parentSelectorFor(host: HtmlActionsControllerHost, target: HtmlActionTarget) {
  const selector = target.selector;
  const document = host.getPreviewDocument();
  if (document) {
    try {
      const element = document.querySelector(selector);
      if (element?.parentElement && element.parentElement !== document.body && element.parentElement !== document.documentElement) {
        return createDomPathSelector(element.parentElement);
      }
    } catch {
      // Fall through to selection metadata.
    }
  }
  return target.parentSelector ?? null;
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
): Promise<EditorActionOutcome> {
  const result = await runInPreviewStructuralLane(host, async (lease) => {
    const location = sourceLocationForSourceReference(
      host,
      target.sourceId,
      target.sourceLocation,
    ) ?? activeHtmlSourceLocationForTarget(host, target);
    if (!location) throw new Error(missingKernelLocationMessage("schimbarea atributelor"));

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
      },
    }, previewStructuralCommandIdentity(lease));

    const blocked = blockedReceiptOutcome(
      receipt,
      "HTML Attribute Engine-ul a blocat atributele.",
    );
    if (blocked) return blocked;

    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      "HTML Attribute Engine-ul a blocat atributele.",
    );
    if (
      receipt.workspaceMutation
      && !receipt.workspaceMutation.changed
      && receipt.workspaceMutation.revisionAfter === receipt.workspaceMutation.revisionBefore
    ) {
      await project(patch, target);
      return noopAction("Atributele coincid deja cu sesiunea proiectului.");
    }
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      cacheCommittedHtmlPatch(host, patch);
      await project(patch, target);
    });
    return committedAction();
  });
  return result ?? cancelledAction("Aplicarea atributelor a fost anulată odată cu sesiunea structurală.");
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
    const location = sourceLocationForSourceReference(
      host,
      target.sourceId,
      target.sourceLocation,
    ) ?? activeHtmlSourceLocationForTarget(host, target);
    if (!location) throw new Error(missingKernelLocationMessage("editarea textului"));

    const receipt = await executePreviewHtmlTextIntent({
      intent: {
        messageType: "preview-html-text",
        selector: target.selector,
        sourceId: target.sourceId,
        sourceTag: target.tag,
      },
      textIntent: {
        targetSourceId: target.sourceId ?? null,
        targetLocation: projectSourceLocation(location),
        targetTag: target.tag,
        targetSelector: target.selector,
        text,
      },
      deferCanonicalProjection: options.deferCanonicalProjection === true,
      editSessionId: options.editSessionId ?? null,
    }, previewStructuralCommandIdentity(lease));

    const blocked = blockedReceiptOutcome(
      receipt,
      "HTML Text Engine-ul a blocat textul.",
    );
    if (blocked) return blocked;

    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      "HTML Text Engine-ul a blocat textul.",
    );
    if (
      !options.deferCanonicalProjection
      && receipt.workspaceMutation
      && !receipt.workspaceMutation.changed
      && receipt.workspaceMutation.revisionAfter === receipt.workspaceMutation.revisionBefore
    ) {
      await project(patch, target);
      return noopAction("Textul coincide deja cu sesiunea proiectului.");
    }
    if (options.deferCanonicalProjection) {
      if (!previewStructuralSessionLeaseMatches(host, lease)) {
        return cancelledAction("Ciorna de text aparține unei sesiuni de previzualizare închise.");
      }
      const mutation = receipt.workspaceMutation;
      if (
        mutation
        && !mutation.changed
        && mutation.revisionAfter === mutation.revisionBefore
      ) {
        await project(patch, target);
        return noopAction("Ciorna de text coincide deja cu sesiunea proiectului.");
      }
      if (
        !mutation?.changed
        || mutation.revisionAfter <= mutation.revisionBefore
        || !mutation.transactionId?.trim()
      ) {
        throw new Error(
          "Confirmarea ciornei de text nu conține o tranziție validă a sesiunii proiectului.",
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
  return result ?? cancelledAction("Aplicarea textului a fost anulată odată cu sesiunea structurală.");
}

export async function deleteSelectedHtmlElement(
  host: HtmlActionsControllerHost,
  editorTarget: EditorHtmlTarget | null = null,
): Promise<EditorActionOutcome> {
  const capturedTarget = captureHtmlActionTarget(editorTarget ?? host.selectedElement);
  try {
    const result = await runInPreviewStructuralLane(host, async (lease) => {
      const target = capturedTarget;
      if (!target) {
        host.structureStatus = "Selectează un element înainte de ștergere.";
        host.setGlobalStatus(host.structureStatus, "error");
        return blockedAction(host.structureStatus);
      }

      const targetSelector = target.selector;
      const parentSelector = parentSelectorFor(host, target);
      const tpl = sourceLocationForSourceReference(host, target.sourceId, target.sourceLocation);
      const kernelTargetLocation = tpl ?? activeHtmlSourceLocationForTarget(host, target);

      if (!kernelTargetLocation) {
        const message = host.isActivePreviewHtmlSource
          ? missingKernelLocationMessage("ștergerea")
          : host.htmlSourceMutationBlockedReason || "Elementul nu are sursă HTML editabilă.";
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
      }, previewStructuralCommandIdentity(lease));

      const blocked = blockedReceiptOutcome(
        receipt,
        "HTML Delete Engine-ul a blocat ștergerea.",
      );
      if (blocked) return blocked;

      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        "HTML Delete Engine-ul a blocat ștergerea.",
      );
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        host.pendingSelectionSelector = parentSelector;
        host.structureStatus = `Element <${target.tag}> șters și salvat prin kernel.`;
      });
      return committedAction();
    });
    return result ?? cancelledAction("Ștergerea a fost anulată odată cu sesiunea structurală.");
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.structureStatus = `Nu am putut șterge elementul: ${result.reason ?? result.status}`;
    host.setGlobalStatus(`Eroare ștergere: ${result.reason ?? result.status}`, "error");
    return result;
  }
}

export async function duplicateSelectedHtmlElement(
  host: HtmlActionsControllerHost,
  editorTarget: EditorHtmlTarget | null = null,
): Promise<EditorActionOutcome> {
  const capturedTarget = captureHtmlActionTarget(editorTarget ?? host.selectedElement);
  try {
    const result = await runInPreviewStructuralLane(host, async (lease) => {
      const target = capturedTarget;
      if (!target) {
        host.structureStatus = "Selectează un element înainte de duplicare.";
        host.setGlobalStatus(host.structureStatus, "error");
        return blockedAction(host.structureStatus);
      }
      if (target.tag === "body" || target.tag === "html") {
        host.structureStatus = "Elementul rădăcină nu poate fi duplicat.";
        host.setGlobalStatus(host.structureStatus, "error");
        return blockedAction(host.structureStatus);
      }

      const targetSelector = target.selector;
      const tpl = sourceLocationForSourceReference(host, target.sourceId, target.sourceLocation);
      const kernelSourceLocation = tpl ?? activeHtmlSourceLocationForTarget(host, target);
      if (!kernelSourceLocation) {
        const message = host.isActivePreviewHtmlSource
          ? missingKernelLocationMessage("duplicarea")
          : host.htmlSourceMutationBlockedReason || "Elementul nu are sursă HTML editabilă.";
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
      }, previewStructuralCommandIdentity(lease));

      const blocked = blockedReceiptOutcome(
        receipt,
        "HTML Duplicate Engine-ul a blocat duplicarea.",
      );
      if (blocked) return blocked;

      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        "HTML Duplicate Engine-ul a blocat duplicarea.",
      );
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
        cacheCommittedHtmlPatch(host, patch);
        if (patch.componentIds.length > 0) {
          await reconcilePageComponentContracts(host, patch.insertedLocation);
        }
        host.structureStatus = `Element <${patch.tag}> duplicat prin kernel.`;
      });
      return committedAction();
    });
    return result ?? cancelledAction("Duplicarea a fost anulată odată cu sesiunea structurală.");
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.structureStatus = `Nu am putut duplica elementul: ${result.reason ?? result.status}`;
    host.setGlobalStatus(`Eroare duplicare: ${result.reason ?? result.status}`, "error");
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
  const capturedTarget = captureHtmlActionTarget(host.selectedElement);
  try {
    await runInPreviewStructuralLane(host, (lease) =>
      insertPaletteElementAtTargetInLane(host, capturedRequest, capturedTarget, lease));
  } catch (error) {
    host.structureStatus = `Nu am putut adăuga elementul: ${errorMessage(error)}`;
    host.setGlobalStatus(`Eroare inserare: ${errorMessage(error)}`, "error");
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
    host.structureStatus = "Comută pe Previzualizare ca să adaugi elemente vizual.";
    host.setGlobalStatus(host.structureStatus, "error");
    return;
  }
  if (request.position === "inside" && !canElementAcceptChildren(request.targetTag, htmlVoidTags)) {
    host.structureStatus = "Destinația nu poate primi copii.";
    host.setGlobalStatus(host.structureStatus, "error");
    return;
  }

  if (!targetLocation) {
    host.structureStatus = "Ținta nu are metadate template stabile. Inserarea este blocată pentru această zonă.";
    host.setGlobalStatus(host.structureStatus, "error");
    return;
  }

  const componentId = request.element.kind === "component" ? request.element.componentId ?? null : null;
  const options = componentId
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
          componentId,
          tag: options.tag,
          className: options.className,
          text: options.text,
          label: request.element.label,
        },
      },
    }, previewStructuralCommandIdentity(lease));

    const patch = requireCommittedPreviewStructuralPatch(
      receipt,
      "HTML Insert Engine-ul a blocat inserarea.",
    );
    await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, async () => {
      cacheCommittedHtmlPatch(host, patch);
      if (patch.componentId) {
        await reconcilePageComponentContracts(host, patch.insertedLocation, { ensureComponentId: patch.componentId });
      }
      host.structureStatus = `Element <${patch.tag}> adăugat ${label} și salvat prin kernel.`;
    });
  } catch (error) {
    host.structureStatus = `Nu am putut adăuga elementul: ${errorMessage(error)}`;
    host.setGlobalStatus(`Eroare inserare: ${errorMessage(error)}`, "error");
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
  const target = captureHtmlActionTarget(host.selectedElement);
  if (!target) {
    host.classStatus = "Selecteaza un element inainte sa generezi o clasa.";
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
    host.classStatus = `Elementul are deja clasa generata ${existing}.`;
    return noopAction(host.classStatus);
  }

  const currentDataAnim = attributeValues["data-anim"]?.trim() ?? "";
  const reusableDataAnim = generatedPanaClass(currentDataAnim) && validClassToken(currentDataAnim) ? currentDataAnim : null;
  const identity = reusableDataAnim
    ? { className: reusableDataAnim }
    : generateUniqueHtmlIdentity(target.tag, await collectIdentitySourceTexts(host));
  if (!previewStructuralSessionLeaseMatches(host, sessionLease)) {
    return cancelledAction("Generarea clasei a fost anulată deoarece ProjectSession s-a schimbat.");
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
  const target = captureHtmlActionTarget(host.selectedElement);
  if (!target) {
    host.attributeStatus = "Selecteaza un element inainte sa generezi data-anim.";
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
    host.attributeStatus = `Elementul are deja data-anim="${currentDataAnim}".`;
    return noopAction(host.attributeStatus);
  }

  const reusableClass = existingGeneratedClass(classEditorValue, target);
  const identity = reusableClass
    ? { dataAnim: reusableClass }
    : generateUniqueHtmlIdentity(target.tag, await collectIdentitySourceTexts(host));
  if (!previewStructuralSessionLeaseMatches(host, sessionLease)) {
    return cancelledAction("Generarea data-anim a fost anulată deoarece ProjectSession s-a schimbat.");
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
  const target = captureHtmlActionTarget(host.selectedElement);
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
    host.structureStatus = `Eroare: ${errorMessage(error)}`;
    host.setGlobalStatus(`Eroare inserare: ${errorMessage(error)}`, "error");
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
    host.structureStatus = "Selecteaza un element inainte sa adaugi un nod nou.";
    return;
  }
  if (!canEditHtmlStructure) {
    host.structureStatus = "Comută pe Previzualizare sau pe codul HTML al paginii active.";
    return;
  }
  if (position === "child" && !canAddChild) {
    host.structureStatus = "Elementul selectat nu poate primi copii.";
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
            componentId: null,
            tag: opts.tag,
            className: opts.className,
            text: opts.text,
            label: `Element <${opts.tag}>`,
          },
        },
      }, previewStructuralCommandIdentity(lease));

      const patch = requireCommittedPreviewStructuralPatch(
        receipt,
        "HTML Insert Engine-ul a blocat inserarea.",
      );
      await projectCommittedPreviewStructuralMutation(host, lease, receipt, patch, () => {
        cacheCommittedHtmlPatch(host, patch);
        const label = position === "before" ? "înainte" : position === "after" ? "după" : "ca fiu";
        host.structureStatus = `Element <${patch.tag}> adăugat ${label} prin kernel.`;
      });
    } catch (error) {
      host.structureStatus = `Eroare: ${errorMessage(error)}`;
      host.setGlobalStatus(`Eroare inserare: ${errorMessage(error)}`, "error");
    }
    return;
  }

  const message = missingKernelLocationMessage("inserarea");
  host.structureStatus = message;
  host.setGlobalStatus(message, "error");
}

export async function applyImageSourceToHtml(
  host: HtmlActionsControllerHost,
  sourceOverride?: string,
): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.selectedElement);
  if (!target || target.tag !== "img") {
    host.imageStatus = "Selecteaza o imagine inainte sa schimbi src.";
    return blockedAction(host.imageStatus);
  }

  const src = (sourceOverride ?? host.imageSourceValue).trim();
  host.imageSourceValue = src;
  if ((target.attributes?.src ?? "").trim() === src) {
    host.setHtmlPending("image", false);
    host.imageStatus = "Sursa imaginii nu are modificări de aplicat.";
    return noopAction(host.imageStatus);
  }
  host.setHtmlPending("image", true);
  try {
    const result = await executeSelectedHtmlAttributes(host, target, { src: src || null }, (patch, capturedTarget) => {
      if (currentSelectionMatchesTarget(host, capturedTarget)) {
        host.imageSourceValue = src;
        host.imageStatus = "Sursa imaginii a fost aplicată prin kernel.";
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
    host.imageStatus = `Nu am putut aplica sursa imaginii: ${result.reason ?? result.status}`;
    host.setGlobalStatus(`Eroare imagine: ${result.reason ?? result.status}`, "error");
    return result;
  }
}

export async function applyClassesToHtml(host: HtmlActionsControllerHost): Promise<EditorActionOutcome> {
  const target = captureHtmlActionTarget(host.selectedElement);
  if (!target) {
    host.classStatus = "Selecteaza un element inainte sa editezi clasele.";
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
    host.classStatus = "Clasele nu au modificări de aplicat.";
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
          host.classStatus = "Clase aplicate prin kernel.";
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
      const reason = result.reason ?? "Kernelul a refuzat schimbarea claselor.";
      host.classStatus = `Clasele nu au fost aplicate: ${reason}`;
      host.setGlobalStatus(host.classStatus, "error");
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.classStatus = `Eroare: ${result.reason ?? result.status}`;
    host.setGlobalStatus(`Eroare clase: ${result.reason ?? result.status}`, "error");
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
  const target = captureHtmlActionTarget(host.selectedElement);
  if (!target) {
    host.attributeStatus = "Selecteaza un element inainte sa editezi atributele.";
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
  const nextKernelAttributes: Record<string, string | null> = Object.fromEntries(
    Object.entries(attributeValues)
      .filter(([name]) => !name.toLowerCase().startsWith("data-pana-"))
      .map(([name, value]) => [name, value]),
  );
  for (const name of Object.keys(target.attributes ?? {})) {
    if (
      !(name in attributeValues)
      && !name.toLowerCase().startsWith("data-pana-")
      && !["class", "style"].includes(name)
    ) {
      nextKernelAttributes[name] = null;
    }
  }
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
        host.attributeStatus = "Atribute aplicate prin kernel.";
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
      const reason = result.reason ?? "Kernelul a refuzat schimbarea atributelor.";
      host.attributeStatus = `Atributele nu au fost aplicate: ${reason}`;
      host.setGlobalStatus(host.attributeStatus, "error");
    }
    return result;
  } catch (error) {
    const result = actionErrorOutcome(error);
    host.attributeStatus = `Eroare: ${result.reason ?? result.status}`;
    host.setGlobalStatus(`Eroare atribute: ${result.reason ?? result.status}`, "error");
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
  const target = captureHtmlActionTarget(host.selectedElement);
  if (!target) {
    host.textStatus = "Selecteaza un element inainte sa editezi textul.";
    return blockedAction(host.textStatus);
  }
  if (target.hasChildElements) {
    host.textStatus = "Editarea textului e disponibila doar pentru elemente fara copii HTML.";
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
    host.textStatus = "Editarea textului e disponibilă doar pentru elemente fără copii HTML.";
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
    host.textStatus = "Textul nu are modificari de aplicat.";
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
          ? "Text confirmat și recuperabil în sesiunea proiectului."
          : "Text aplicat prin kernel.";
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
    host.textStatus = `Eroare: ${result.reason ?? result.status}`;
    host.setGlobalStatus(`Eroare text: ${result.reason ?? result.status}`, "error");
    return result;
  }
}
