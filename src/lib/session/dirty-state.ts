import type { HtmlPendingArea, InspectorPendingArea } from "$lib/types";

export type SessionDirtyArea = "workspace" | "html" | "css" | "vars" | "js";

export type GlobalDirtyState = {
  dirty: boolean;
  areas: SessionDirtyArea[];
  canSave: boolean;
  requiresImmediateDiskWrite: boolean;
  blocksImmediateDiskOperations: boolean;
  immediateDiskOperationBlockedReason: string;
};

export type DirtyStateInput = {
  workspaceDirty?: boolean;
  htmlPending: Record<HtmlPendingArea, boolean>;
  inspectorPending: Record<InspectorPendingArea, boolean>;
};

export function deriveGlobalDirtyState(input: DirtyStateInput): GlobalDirtyState {
  const areas: SessionDirtyArea[] = [];
  if (input.workspaceDirty) areas.push("workspace");
  const htmlDirty = Object.values(input.htmlPending).some(Boolean) || input.inspectorPending.html;
  if (htmlDirty) areas.push("html");
  if (input.inspectorPending.css) areas.push("css");
  if (input.inspectorPending.vars) areas.push("vars");
  if (input.inspectorPending.js) areas.push("js");

  const uniqueAreas = [...new Set(areas)];
  return {
    dirty: uniqueAreas.length > 0,
    areas: uniqueAreas,
    canSave: uniqueAreas.length > 0,
    requiresImmediateDiskWrite: false,
    blocksImmediateDiskOperations: uniqueAreas.length > 0,
    immediateDiskOperationBlockedReason: uniqueAreas.length > 0
      ? "Salvează sau renunță la modificările curente înainte de operații directe pe disk."
      : "",
  };
}
