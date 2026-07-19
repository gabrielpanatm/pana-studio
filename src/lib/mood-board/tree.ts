import {
  cloneMoodBoard,
  createMoodBoardItemId,
  type MoodBoard,
  type MoodBoardItem,
} from "$lib/mood-board/model";
import type { MoodBoardSnapGuide } from "$lib/mood-board/snap";

export function mutateContainingList(
  items: MoodBoardItem[],
  itemId: string,
  mutator: (items: MoodBoardItem[], index: number) => void,
): boolean {
  const index = items.findIndex((item) => item.id === itemId);
  if (index >= 0) {
    mutator(items, index);
    return true;
  }

  for (const item of items) {
    if ((item.type === "frame" || item.type === "group") && mutateContainingList(item.children, itemId, mutator)) {
      return true;
    }
  }

  return false;
}

export function duplicateItemTree(source: MoodBoardItem, offsetRoot = true): MoodBoardItem {
  const duplicate = cloneMoodBoard({
    version: 2,
    updatedAt: Date.now(),
    viewport: { x: 0, y: 0, zoom: 1 },
    items: [source],
  }).items[0];
  const assignIds = (item: MoodBoardItem, isRoot: boolean): MoodBoardItem => {
    const next = {
      ...item,
      id: createMoodBoardItemId(),
      x: isRoot && offsetRoot ? item.x + 24 : item.x,
      y: isRoot && offsetRoot ? item.y + 24 : item.y,
    };
    if (next.type !== "frame" && next.type !== "group") return next;
    return {
      ...next,
      children: next.children.map((child) => assignIds(child, false)),
    };
  };
  return assignIds(duplicate, true);
}

export function containingFrameForItem(items: MoodBoardItem[], itemId: string): MoodBoardItem | null {
  for (const item of items) {
    if (item.type === "frame") {
      if (item.children.some((child) => child.id === itemId)) return item;
      const nested = containingFrameForItem(item.children, itemId);
      if (nested) return nested;
    } else if (item.type === "group") {
      const nested = containingFrameForItem(item.children, itemId);
      if (nested) return nested;
    }
  }
  return null;
}

export function parentFrameTitle(board: MoodBoard, itemId: string) {
  const frame = containingFrameForItem(board.items, itemId);
  return frame?.type === "frame" ? frame.title : "";
}

export function rootAttachFrameForItem(sourceBoard: MoodBoard, itemId: string) {
  const item = sourceBoard.items.find((entry) => entry.id === itemId);
  if (!item || item.type === "frame") return null;
  const centerX = item.x + item.width / 2;
  const centerY = item.y + item.height / 2;
  return sourceBoard.items.find((entry) => (
    entry.type === "frame"
    && centerX >= entry.x
    && centerX <= entry.x + entry.width
    && centerY >= entry.y
    && centerY <= entry.y + entry.height
  )) ?? null;
}

export function rootAttachFrameForItems(sourceBoard: MoodBoard, itemIds: string[]) {
  if (itemIds.length === 0) return null;
  const rootItems = itemIds.map((itemId) => sourceBoard.items.find((entry) => entry.id === itemId));
  if (rootItems.some((item) => !item || item.type === "frame")) return null;

  let targetFrameId: string | null = null;
  for (const item of rootItems) {
    if (!item || item.type === "frame") return null;
    const frame = rootAttachFrameForItem(sourceBoard, item.id);
    if (!frame || frame.type !== "frame") return null;
    if (targetFrameId && targetFrameId !== frame.id) return null;
    targetFrameId = frame.id;
  }

  return sourceBoard.items.find((entry) => entry.id === targetFrameId && entry.type === "frame") ?? null;
}

export function snapContextItems(sourceBoard: MoodBoard, draggedIds: Set<string>) {
  const ids = [...draggedIds];
  if (ids.length !== 1) return sourceBoard.items;
  const frame = containingFrameForItem(sourceBoard.items, ids[0]);
  if (!frame || frame.type !== "frame") return sourceBoard.items;
  return [
    {
      ...frame,
      id: `${frame.id}__bounds`,
      x: 0,
      y: 0,
      children: [],
    },
    ...frame.children,
  ];
}

export function guidesInRootCoordinates(
  sourceBoard: MoodBoard,
  itemId: string,
  guides: MoodBoardSnapGuide[],
) {
  const frame = containingFrameForItem(sourceBoard.items, itemId);
  if (!frame || frame.type !== "frame") return guides;
  return guides.map((guide) => (
    guide.orientation === "vertical"
      ? {
        ...guide,
        position: guide.position + frame.x,
        start: guide.start + frame.y,
        end: guide.end + frame.y,
      }
      : {
        ...guide,
        position: guide.position + frame.y,
        start: guide.start + frame.x,
        end: guide.end + frame.x,
      }
  ));
}

export function containingListInfo(
  items: MoodBoardItem[],
  itemId: string,
): { items: MoodBoardItem[]; index: number } | null {
  const index = items.findIndex((item) => item.id === itemId);
  if (index >= 0) return { items, index };

  for (const item of items) {
    if (item.type !== "frame" && item.type !== "group") continue;
    const nested = containingListInfo(item.children, itemId);
    if (nested) return nested;
  }

  return null;
}

export function selectedItemsInSameList(sourceBoard: MoodBoard, ids: string[]) {
  const infos = ids
    .map((itemId) => containingListInfo(sourceBoard.items, itemId))
    .filter((info): info is { items: MoodBoardItem[]; index: number } => Boolean(info));
  if (infos.length !== ids.length) return null;
  const targetList = infos[0]?.items;
  if (!targetList || infos.some((info) => info.items !== targetList)) return null;
  return targetList;
}
