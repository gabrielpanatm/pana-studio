import {
  isTeraConstructKind,
  type TeraDropRequest,
  type TeraPaletteFamily,
  type TeraPaletteItem,
} from "$lib/tera/model";
import type { DropPosition } from "$lib/ui/drag";
import type { SaveState } from "$lib/types";
import {
  blockedAction,
  type EditorActionOutcome,
} from "$lib/editor-runtime/action-outcome";

export type PreviewTeraInsertControllerHost = {
  insertTeraPaletteItemAtTarget: (request: TeraDropRequest) => Promise<EditorActionOutcome>;
  setGlobalStatus: (text: string, kind: SaveState) => void;
  previewDropGateStatus?: (target: {
    targetSourceId?: string | null;
    targetTemplateSourceId?: string | null;
  }) => { allowed: boolean; message?: string };
};

const dropPositions = new Set<DropPosition>(["before", "after", "inside"]);
const families = new Set<TeraPaletteFamily>(["composition", "logic", "data", "reuse", "safe"]);

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
    sourceNodeId: typeof data.sourceNodeId === "string" ? data.sourceNodeId : undefined,
  };
}

export async function handlePreviewTeraInsertDrop(
  host: PreviewTeraInsertControllerHost,
  payload: unknown,
): Promise<EditorActionOutcome> {
  const data = payload as Record<string, unknown>;
  const targetSelector = stringValue(data.targetSelector);
  const targetSessionId = stringValue(data.targetSessionId) || null;
  const targetSourceId = stringValue(data.targetSourceId) || null;
  const targetTemplateSourceId = stringValue(data.targetTemplateSourceId) || null;
  const targetTag = stringValue(data.targetTag).toLowerCase();
  const position = dropPositionValue(data.position);
  const item = teraPaletteItemValue(data.item);

  if (!targetSelector || !targetTag || !position || !item) {
    host.setGlobalStatus("Drop invalid pentru Tera.", "error");
    return blockedAction("Drop invalid pentru Tera.");
  }

  const gate = host.previewDropGateStatus?.({ targetSourceId, targetTemplateSourceId });
  if (gate && !gate.allowed) {
    host.setGlobalStatus(gate.message || "Drop blocat de gate-ul Tera.", "error");
    return blockedAction(gate.message || "Drop blocat de gate-ul Tera.");
  }

  return await host.insertTeraPaletteItemAtTarget({
    targetSelector,
    targetSessionId,
    targetSourceId,
    targetTemplateSourceId,
    targetTag,
    position,
    item,
  });
}
