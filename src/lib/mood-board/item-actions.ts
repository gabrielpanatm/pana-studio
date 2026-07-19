import {
  alignMoodBoardItems,
  distributeMoodBoardItems,
  type MoodBoardAlignMode,
  type MoodBoardDistributeMode,
} from "$lib/mood-board/layout";
import {
  cloneMoodBoard,
  createMoodBoardItemId,
  findMoodBoardItem,
  mapMoodBoardItems,
  type MoodBoard,
  type MoodBoardItem,
} from "$lib/mood-board/model";
import {
  containingListInfo,
  duplicateItemTree,
  mutateContainingList,
  selectedItemsInSameList,
} from "$lib/mood-board/tree";

export type MoodBoardItemActionResult = {
  board?: MoodBoard;
  selectedItemIds?: string[];
  selectionMode?: "selectOnly" | "setSelection" | "assign";
  selectedSvgElementId?: string | null;
  status?: {
    text: string;
    kind: "saved" | "error";
  };
};

export function bringItemToFront(board: MoodBoard, itemId: string): MoodBoardItemActionResult | null {
  const next = cloneMoodBoard(board);
  const changed = mutateContainingList(next.items, itemId, (items, index) => {
    if (index === items.length - 1) return;
    const [item] = items.splice(index, 1);
    items.push(item);
  });
  if (!changed) return null;
  return { board: next, selectedItemIds: [itemId], selectionMode: "selectOnly" };
}

export function sendItemToBack(board: MoodBoard, itemId: string): MoodBoardItemActionResult | null {
  const next = cloneMoodBoard(board);
  const changed = mutateContainingList(next.items, itemId, (items, index) => {
    if (index <= 0) return;
    const [item] = items.splice(index, 1);
    items.unshift(item);
  });
  if (!changed) return null;
  return { board: next, selectedItemIds: [itemId], selectionMode: "selectOnly" };
}

export function bringItemsToFront(board: MoodBoard, itemIds: string[]): MoodBoardItemActionResult | null {
  if (itemIds.length === 0) return null;
  const next = cloneMoodBoard(board);
  for (const itemId of itemIds) {
    mutateContainingList(next.items, itemId, (items, index) => {
      if (index === items.length - 1) return;
      const [item] = items.splice(index, 1);
      items.push(item);
    });
  }
  return { board: next, selectedItemIds: itemIds, selectionMode: "setSelection" };
}

export function sendItemsToBack(board: MoodBoard, itemIds: string[]): MoodBoardItemActionResult | null {
  if (itemIds.length === 0) return null;
  const next = cloneMoodBoard(board);
  for (const itemId of [...itemIds].reverse()) {
    mutateContainingList(next.items, itemId, (items, index) => {
      if (index <= 0) return;
      const [item] = items.splice(index, 1);
      items.unshift(item);
    });
  }
  return { board: next, selectedItemIds: itemIds, selectionMode: "setSelection" };
}

export function duplicateItem(board: MoodBoard, itemId: string): MoodBoardItemActionResult | null {
  const source = findMoodBoardItem(board.items, itemId);
  if (!source) return null;
  const duplicate = duplicateItemTree(source);
  const next = cloneMoodBoard(board);
  const changed = mutateContainingList(next.items, itemId, (items, index) => {
    items.splice(index + 1, 0, duplicate);
  });
  if (!changed) return null;
  return { board: next, selectedItemIds: [duplicate.id], selectionMode: "selectOnly" };
}

export function duplicateItems(board: MoodBoard, itemIds: string[]): MoodBoardItemActionResult | null {
  if (itemIds.length === 0) return null;
  if (itemIds.length === 1) return duplicateItem(board, itemIds[0]);

  const next = cloneMoodBoard(board);
  const duplicates: MoodBoardItem[] = [];
  for (const itemId of itemIds) {
    const source = findMoodBoardItem(board.items, itemId);
    if (!source) continue;
    const duplicate = duplicateItemTree(source);
    const changed = mutateContainingList(next.items, itemId, (items, index) => {
      items.splice(index + 1, 0, duplicate);
    });
    if (changed) duplicates.push(duplicate);
  }
  if (duplicates.length === 0) return null;
  return {
    board: next,
    selectedItemIds: duplicates.map((item) => item.id),
    selectionMode: "assign",
    selectedSvgElementId: null,
  };
}

export function alignItems(
  board: MoodBoard,
  itemIds: string[],
  mode: MoodBoardAlignMode,
): MoodBoardItemActionResult | null {
  if (itemIds.length < 2) return null;
  const next = cloneMoodBoard(board);
  const targetList = selectedItemsInSameList(next, itemIds);
  if (!targetList) {
    return { status: { text: "Aliniază doar elemente din același container.", kind: "error" } };
  }
  const selectedSet = new Set(itemIds);
  targetList.splice(0, targetList.length, ...alignMoodBoardItems(targetList, selectedSet, mode));
  return { board: next, selectedItemIds: itemIds, selectionMode: "setSelection" };
}

export function distributeItems(
  board: MoodBoard,
  itemIds: string[],
  mode: MoodBoardDistributeMode,
): MoodBoardItemActionResult | null {
  if (itemIds.length < 3) return null;
  const next = cloneMoodBoard(board);
  const targetList = selectedItemsInSameList(next, itemIds);
  if (!targetList) {
    return { status: { text: "Distribuie doar elemente din același container.", kind: "error" } };
  }
  const selectedSet = new Set(itemIds);
  targetList.splice(0, targetList.length, ...distributeMoodBoardItems(targetList, selectedSet, mode));
  return { board: next, selectedItemIds: itemIds, selectionMode: "setSelection" };
}

export function groupItems(board: MoodBoard, itemIds: string[]): MoodBoardItemActionResult | null {
  if (itemIds.length < 2) return null;
  const next = cloneMoodBoard(board);
  const infos = itemIds
    .map((itemId) => containingListInfo(next.items, itemId))
    .filter((info): info is { items: MoodBoardItem[]; index: number } => Boolean(info));
  if (infos.length !== itemIds.length) return null;
  const targetList = infos[0]?.items;
  if (!targetList || infos.some((info) => info.items !== targetList)) {
    return { status: { text: "Grupează doar elemente din același container.", kind: "error" } };
  }

  const selectedSet = new Set(itemIds);
  const selectedInLayerOrder = targetList.filter((item) => selectedSet.has(item.id));
  if (selectedInLayerOrder.length < 2) return null;
  const minX = Math.min(...selectedInLayerOrder.map((item) => item.x));
  const minY = Math.min(...selectedInLayerOrder.map((item) => item.y));
  const maxX = Math.max(...selectedInLayerOrder.map((item) => item.x + item.width));
  const maxY = Math.max(...selectedInLayerOrder.map((item) => item.y + item.height));
  const group = {
    id: createMoodBoardItemId(),
    type: "group" as const,
    x: minX,
    y: minY,
    width: Math.max(40, maxX - minX),
    height: Math.max(40, maxY - minY),
    title: "Grup",
    children: selectedInLayerOrder.map((item) => ({
      ...item,
      x: item.x - minX,
      y: item.y - minY,
    })),
  };
  const firstIndex = Math.min(...infos.map((info) => info.index));
  const remaining = targetList.filter((item) => !selectedSet.has(item.id));
  const insertionIndex = Math.min(firstIndex, remaining.length);
  remaining.splice(insertionIndex, 0, group);
  targetList.splice(0, targetList.length, ...remaining);

  return {
    board: next,
    selectedItemIds: [group.id],
    selectionMode: "assign",
    selectedSvgElementId: null,
    status: { text: `${selectedInLayerOrder.length} elemente grupate.`, kind: "saved" },
  };
}

export function ungroupGroup(board: MoodBoard, groupId: string | null): MoodBoardItemActionResult | null {
  const group = groupId ? findMoodBoardItem(board.items, groupId) : null;
  if (!group || group.type !== "group") return null;
  const next = cloneMoodBoard(board);
  const info = containingListInfo(next.items, group.id);
  if (!info) return null;
  const sourceGroup = info.items[info.index];
  if (!sourceGroup || sourceGroup.type !== "group") return null;
  const children = sourceGroup.children.map((child) => ({
    ...child,
    x: sourceGroup.x + child.x,
    y: sourceGroup.y + child.y,
  }));
  info.items.splice(info.index, 1, ...children);

  return {
    board: next,
    selectedItemIds: children.map((item) => item.id),
    selectionMode: "assign",
    selectedSvgElementId: null,
    status: { text: `${children.length} elemente scoase din grup.`, kind: "saved" },
  };
}

export function nudgeItem(board: MoodBoard, itemId: string, dx: number, dy: number): MoodBoard | null {
  const next = cloneMoodBoard(board);
  let moved = false;
  next.items = mapMoodBoardItems(next.items, (item) => {
    if (item.id !== itemId) return item;
    moved = true;
    return { ...item, x: item.x + dx, y: item.y + dy };
  });
  return moved ? next : null;
}

export function nudgeItems(board: MoodBoard, itemIds: string[], dx: number, dy: number): MoodBoard | null {
  if (itemIds.length === 0) return null;
  if (itemIds.length === 1) return nudgeItem(board, itemIds[0], dx, dy);

  const selectedSet = new Set(itemIds);
  const next = cloneMoodBoard(board);
  next.items = mapMoodBoardItems(next.items, (item) => selectedSet.has(item.id)
    ? { ...item, x: item.x + dx, y: item.y + dy }
    : item);
  return next;
}
