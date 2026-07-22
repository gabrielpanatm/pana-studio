export type AppShortcutIntent =
  | "none"
  | "preventNativeZoom"
  | "commandCenter"
  | "toggleTerminal"
  | "togglePrimarySidebar"
  | "showProblems"
  | "toggleEditorSplit"
  | "save"
  | "undo"
  | "redo";
export type DeleteShortcutIntent = "none" | "deleteSelectedHtml";

type DeleteShortcutState = {
  activeWorkbenchActivity: string;
  centerView: string;
  selectedElement: unknown;
  settingsPanelOpen: boolean;
};

export function isAppTextEditingTarget(target: EventTarget | null) {
  return typeof HTMLElement !== "undefined"
    && target instanceof HTMLElement
    && Boolean(target.closest("input, textarea, select, [contenteditable='true'], .cm-editor"));
}

export function isManagedWorkspaceEditorTarget(target: EventTarget | null) {
  return typeof HTMLElement !== "undefined"
    && target instanceof HTMLElement
    && Boolean(target.closest(".cm-editor, .tiptap-markdown-editor"));
}

export function appShortcutIntent(event: KeyboardEvent): AppShortcutIntent {
  const hasModifier = event.ctrlKey || event.metaKey;
  if (!hasModifier || event.altKey) return "none";
  const key = event.key.toLowerCase();

  if (key === "k") return "commandCenter";
  if (key === "`" || key === "~") return "toggleTerminal";
  if (key === "b" && !isAppTextEditingTarget(event.target)) return "togglePrimarySidebar";
  if (key === "m" && event.shiftKey) return "showProblems";
  if (key === "\\") return "toggleEditorSplit";

  if (key === "+" || key === "=" || key === "-" || key === "_" || key === "0") {
    return "preventNativeZoom";
  }

  if (key === "s") return "save";

  if (key === "z") {
    if (isManagedWorkspaceEditorTarget(event.target)) return event.shiftKey ? "redo" : "undo";
    if (isAppTextEditingTarget(event.target)) return "none";
    return event.shiftKey ? "redo" : "undo";
  }

  if (key === "y" && isManagedWorkspaceEditorTarget(event.target)) return "redo";

  return "none";
}

export function deleteShortcutIntent(event: KeyboardEvent, state: DeleteShortcutState): DeleteShortcutIntent {
  if (event.key !== "Delete" && event.key !== "Backspace") return "none";
  if (event.ctrlKey || event.metaKey || event.altKey) return "none";
  if (state.activeWorkbenchActivity !== "editor") return "none";
  if (state.centerView === "site" || state.centerView === "kernel") return "none";
  if (isAppTextEditingTarget(event.target)) return "none";
  if (!state.selectedElement || state.settingsPanelOpen) return "none";
  return "deleteSelectedHtml";
}
