import type { MoodBoardItem } from "$lib/mood-board/model";

export type MoodBoardAlignMode = "left" | "center" | "right" | "top" | "middle" | "bottom";
export type MoodBoardDistributeMode = "horizontal" | "vertical";

function selectionBounds(items: MoodBoardItem[]) {
  return {
    minX: Math.min(...items.map((item) => item.x)),
    minY: Math.min(...items.map((item) => item.y)),
    maxX: Math.max(...items.map((item) => item.x + item.width)),
    maxY: Math.max(...items.map((item) => item.y + item.height)),
  };
}

export function alignMoodBoardItems(items: MoodBoardItem[], selectedIds: Set<string>, mode: MoodBoardAlignMode) {
  const selected = items.filter((item) => selectedIds.has(item.id));
  if (selected.length < 2) return items;
  const bounds = selectionBounds(selected);

  return items.map((item) => {
    if (!selectedIds.has(item.id)) return item;
    if (mode === "left") return { ...item, x: bounds.minX };
    if (mode === "center") return { ...item, x: bounds.minX + (bounds.maxX - bounds.minX - item.width) / 2 };
    if (mode === "right") return { ...item, x: bounds.maxX - item.width };
    if (mode === "top") return { ...item, y: bounds.minY };
    if (mode === "middle") return { ...item, y: bounds.minY + (bounds.maxY - bounds.minY - item.height) / 2 };
    return { ...item, y: bounds.maxY - item.height };
  });
}

export function distributeMoodBoardItems(items: MoodBoardItem[], selectedIds: Set<string>, mode: MoodBoardDistributeMode) {
  const selected = items.filter((item) => selectedIds.has(item.id));
  if (selected.length < 3) return items;
  const ordered = [...selected].sort((left, right) => (
    mode === "horizontal" ? left.x - right.x : left.y - right.y
  ));
  const first = ordered[0];
  const last = ordered.at(-1);
  if (!first || !last) return items;

  if (mode === "horizontal") {
    const totalWidth = ordered.reduce((sum, item) => sum + item.width, 0);
    const available = (last.x + last.width) - first.x - totalWidth;
    const gap = available / (ordered.length - 1);
    let cursor = first.x;
    const positions = new Map<string, number>();
    for (const item of ordered) {
      positions.set(item.id, cursor);
      cursor += item.width + gap;
    }
    return items.map((item) => positions.has(item.id) ? { ...item, x: positions.get(item.id) ?? item.x } : item);
  }

  const totalHeight = ordered.reduce((sum, item) => sum + item.height, 0);
  const available = (last.y + last.height) - first.y - totalHeight;
  const gap = available / (ordered.length - 1);
  let cursor = first.y;
  const positions = new Map<string, number>();
  for (const item of ordered) {
    positions.set(item.id, cursor);
    cursor += item.height + gap;
  }
  return items.map((item) => positions.has(item.id) ? { ...item, y: positions.get(item.id) ?? item.y } : item);
}
