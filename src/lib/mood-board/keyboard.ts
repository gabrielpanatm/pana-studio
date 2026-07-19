import type { MoodBoardTool } from "$lib/mood-board/model";

export type MoodBoardKeyboardIntent =
  | { kind: "none" }
  | { kind: "pasteClipboardFallback" }
  | { kind: "finishPenPath"; closed: boolean }
  | { kind: "clearPenDraft" }
  | { kind: "exitVectorEdit" }
  | { kind: "deleteSelection" }
  | { kind: "duplicateSelection" }
  | { kind: "groupSelection" }
  | { kind: "ungroupSelection" }
  | { kind: "bringSelectionToFront" }
  | { kind: "sendSelectionToBack" }
  | { kind: "nudgeSelection"; dx: number; dy: number };

export function isMoodBoardEditableTarget(target: EventTarget | null) {
  return target instanceof HTMLElement
    && Boolean(target.closest("input, textarea, select, button, [contenteditable='true']"));
}

export function moodBoardKeyboardIntent(
  event: KeyboardEvent,
  state: {
    tool: MoodBoardTool;
    hasSelection: boolean;
    hasVectorEditTarget: boolean;
    penNodeCount: number;
  },
): MoodBoardKeyboardIntent {
  if (isMoodBoardEditableTarget(event.target)) return { kind: "none" };

  const modifier = event.ctrlKey || event.metaKey;
  const key = event.key.toLowerCase();

  if (modifier && key === "v") return { kind: "pasteClipboardFallback" };

  if (state.tool === "pen") {
    if (event.key === "Escape") {
      return state.penNodeCount >= 2
        ? { kind: "finishPenPath", closed: false }
        : { kind: "clearPenDraft" };
    }

    if (event.key === "Enter") {
      return { kind: "finishPenPath", closed: state.penNodeCount >= 3 };
    }
  }

  if (!state.hasSelection) return { kind: "none" };

  if (event.key === "Escape" && state.hasVectorEditTarget) return { kind: "exitVectorEdit" };
  if (event.key === "Delete" || event.key === "Backspace") return { kind: "deleteSelection" };
  if (modifier && key === "d") return { kind: "duplicateSelection" };

  if (modifier && key === "g") {
    return event.shiftKey ? { kind: "ungroupSelection" } : { kind: "groupSelection" };
  }

  if (modifier && event.key === "]") return { kind: "bringSelectionToFront" };
  if (modifier && event.key === "[") return { kind: "sendSelectionToBack" };

  const step = event.shiftKey ? 10 : 1;
  if (event.key === "ArrowLeft") return { kind: "nudgeSelection", dx: -step, dy: 0 };
  if (event.key === "ArrowRight") return { kind: "nudgeSelection", dx: step, dy: 0 };
  if (event.key === "ArrowUp") return { kind: "nudgeSelection", dx: 0, dy: -step };
  if (event.key === "ArrowDown") return { kind: "nudgeSelection", dx: 0, dy: step };

  return { kind: "none" };
}
