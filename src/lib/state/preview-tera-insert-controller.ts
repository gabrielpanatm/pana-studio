import {
  isTeraConstructKind,
  type TeraDropRequest,
  type TeraPaletteFamily,
  type TeraPaletteItem,
} from "$lib/tera/model";
import type { DropPosition } from "$lib/ui/drag";
import type { GlobalStatusKind } from "$lib/status/global-status";
import {
  blockedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";
import { t } from "$lib/i18n/runtime.svelte";

export type PreviewTeraInsertControllerHost = {
  insertTeraPaletteItemAtTarget: (request: TeraDropRequest) => Promise<EditorActionOutcome>;
  setGlobalStatus: (text: string, kind: GlobalStatusKind) => void;
  previewDropTargetStatus?: (target: {
    targetRenderInstanceId?: string | null;
    targetBoundarySourceId?: string | null;
    targetBoundaryInstanceId?: string | null;
  }) => { allowed: boolean; message?: string };
};

const dropPositions = new Set<DropPosition>(["before", "after", "inside"]);
const families = new Set<TeraPaletteFamily>(["composition", "logic", "data", "reuse", "safe"]);
const inFlightDrops = new Map<string, Promise<EditorActionOutcome>>();

function stringValue(value: unknown) {
  return typeof value === "string" ? value.trim() : "";
}

function dropPositionValue(value: unknown): DropPosition | null {
  return typeof value === "string" && dropPositions.has(value as DropPosition)
    ? value as DropPosition
    : null;
}

function teraPaletteItemValue(value: unknown): TeraPaletteItem | null {
  const data = value as Record<string, unknown> | null;
  if (!data || typeof data !== "object") return null;
  const kind = stringValue(data.kind);
  if (!isTeraConstructKind(kind)) return null;
  const familyValue = stringValue(data.family);
  const family = families.has(familyValue as TeraPaletteFamily)
    ? familyValue as TeraPaletteFamily
    : "composition";
  const label = stringValue(data.label) || kind;
  const id = stringValue(data.id) || `${kind}:${label.toLowerCase().replace(/\s+/g, "-")}`;

  return {
    id,
    kind,
    family,
    label,
    description: stringValue(data.description),
    snippet: typeof data.snippet === "string" ? data.snippet : "",
    target: typeof data.target === "string" ? data.target : undefined,
    name: typeof data.name === "string" ? data.name : undefined,
    expression: typeof data.expression === "string" ? data.expression : undefined,
    dynamicBinding: typeof data.dynamicBinding === "object" && data.dynamicBinding !== null
      ? data.dynamicBinding as TeraPaletteItem["dynamicBinding"]
      : undefined,
    dynamicWidget: typeof data.dynamicWidget === "object" && data.dynamicWidget !== null
      ? data.dynamicWidget as TeraPaletteItem["dynamicWidget"]
      : undefined,
    sourceNodeId: typeof data.sourceNodeId === "string" ? data.sourceNodeId : undefined,
  };
}

export async function handlePreviewTeraInsertDrop(
  host: PreviewTeraInsertControllerHost,
  payload: unknown,
): Promise<EditorActionOutcome> {
  const data = payload as Record<string, unknown>;
  const targetRenderInstanceId = stringValue(data.targetRenderInstanceId) || null;
  const targetSelector = stringValue(data.targetSelector);
  const targetSessionId = stringValue(data.targetSessionId) || null;
  const targetSourceId = stringValue(data.targetSourceId) || null;
  const targetTemplateSourceId = stringValue(data.targetTemplateSourceId) || null;
  const targetBoundaryInstanceId = stringValue(data.targetBoundaryInstanceId) || null;
  const documentRootTarget =
    data.targetKind === "empty-tera-slot" || data.targetKind === "active-document-root";
  const targetTag = stringValue(data.targetTag).toLowerCase();
  const position = dropPositionValue(data.position);
  const item = teraPaletteItemValue(data.item);

  if (!targetSelector || !targetTag || !position || !item) {
    const reason = t("preview-drop-tera-invalid");
    host.setGlobalStatus(reason, "error");
    return blockedAction(reason);
  }

  const targetStatus = host.previewDropTargetStatus?.({
    targetRenderInstanceId,
    targetBoundarySourceId:
      documentRootTarget ? targetTemplateSourceId : null,
    targetBoundaryInstanceId:
      documentRootTarget ? targetBoundaryInstanceId : null,
  });
  if (targetStatus && !targetStatus.allowed) {
    const reason = targetStatus.message || t("preview-drop-navigation-target-blocked");
    host.setGlobalStatus(reason, "error");
    return blockedAction(reason);
  }

  const request: TeraDropRequest = {
    targetSelector,
    targetSessionId,
    targetSourceId,
    targetTemplateSourceId,
    targetTag,
    position,
    item,
  };
  const fingerprint = JSON.stringify(request);
  const existing = inFlightDrops.get(fingerprint);
  if (existing) return await existing;

  const operation = Promise.resolve().then(() => host.insertTeraPaletteItemAtTarget(request));
  inFlightDrops.set(fingerprint, operation);
  try {
    return await operation;
  } finally {
    if (inFlightDrops.get(fingerprint) === operation) {
      inFlightDrops.delete(fingerprint);
    }
  }
}
