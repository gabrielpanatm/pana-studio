import type { MoodBoard, MoodBoardItem } from "$lib/mood-board/model";
import { rootAttachFrameForItems } from "$lib/mood-board/tree";

export function attachRootItemToFrameIfNeeded(sourceBoard: MoodBoard, itemId: string) {
  const item = sourceBoard.items.find((entry) => entry.id === itemId);
  if (!item || item.type === "frame") return sourceBoard;

  const centerX = item.x + item.width / 2;
  const centerY = item.y + item.height / 2;
  const frame = sourceBoard.items.find((entry) => (
    entry.type === "frame"
    && centerX >= entry.x
    && centerX <= entry.x + entry.width
    && centerY >= entry.y
    && centerY <= entry.y + entry.height
  ));
  if (!frame || frame.type !== "frame") return sourceBoard;

  return {
    ...sourceBoard,
    items: sourceBoard.items
      .filter((entry) => entry.id !== itemId)
      .map((entry) => {
        if (entry.id !== frame.id || entry.type !== "frame") return entry;
        return {
          ...entry,
          children: [
            ...entry.children,
            {
              ...item,
              x: item.x - entry.x,
              y: item.y - entry.y,
            },
          ],
        };
      }),
  };
}

export function attachRootItemsToFrameIfNeeded(sourceBoard: MoodBoard, itemIds: string[]) {
  const frame = rootAttachFrameForItems(sourceBoard, itemIds);
  if (!frame || frame.type !== "frame") return sourceBoard;

  const selectedSet = new Set(itemIds);
  const itemsToAttach = sourceBoard.items.filter((entry) => selectedSet.has(entry.id) && entry.type !== "frame");
  if (itemsToAttach.length === 0) return sourceBoard;

  return {
    ...sourceBoard,
    items: sourceBoard.items
      .filter((entry) => !selectedSet.has(entry.id))
      .map((entry) => {
        if (entry.id !== frame.id || entry.type !== "frame") return entry;
        return {
          ...entry,
          children: [
            ...entry.children,
            ...itemsToAttach.map((item) => ({
              ...item,
              x: item.x - entry.x,
              y: item.y - entry.y,
            })),
          ],
        };
      }),
  };
}

export function detachChildFromFrameIfNeeded(sourceBoard: MoodBoard, itemId: string, force = false) {
  let detached: MoodBoardItem | null = null;
  const items = sourceBoard.items.map((entry) => {
    if (entry.type !== "frame") return entry;
    const child = entry.children.find((candidate) => candidate.id === itemId);
    if (!child) return entry;

    const centerX = child.x + child.width / 2;
    const centerY = child.y + child.height / 2;
    const inside = centerX >= 0 && centerX <= entry.width && centerY >= 0 && centerY <= entry.height;
    if (inside && !force) return entry;

    detached = {
      ...child,
      x: entry.x + child.x,
      y: entry.y + child.y,
    };
    return {
      ...entry,
      children: entry.children.filter((candidate) => candidate.id !== itemId),
    };
  });

  return detached ? { ...sourceBoard, items: [...items, detached] } : sourceBoard;
}
