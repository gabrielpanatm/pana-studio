import type { MoodBoard, MoodBoardItem } from "$lib/mood-board/model";

export type MoodBoardViewportRect = Pick<DOMRect, "left" | "top" | "width" | "height">;

export type MoodBoardItemBounds = {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
};

export function screenPointToCanvas(
  board: MoodBoard,
  rect: MoodBoardViewportRect | null | undefined,
  clientX: number,
  clientY: number,
) {
  const zoom = board.viewport.zoom || 1;
  return {
    x: (clientX - (rect?.left ?? 0) - board.viewport.x) / zoom,
    y: (clientY - (rect?.top ?? 0) - board.viewport.y) / zoom,
  };
}

export function clampMoodBoardZoom(value: number) {
  return Math.min(3, Math.max(0.2, value));
}

export function boardWithZoomAtPoint(
  sourceBoard: MoodBoard,
  rect: MoodBoardViewportRect | null | undefined,
  nextZoomValue: number,
  clientX: number,
  clientY: number,
): MoodBoard {
  const nextZoom = clampMoodBoardZoom(nextZoomValue);
  const canvasPoint = screenPointToCanvas(sourceBoard, rect, clientX, clientY);
  return {
    ...sourceBoard,
    viewport: {
      zoom: nextZoom,
      x: (clientX - (rect?.left ?? 0)) - canvasPoint.x * nextZoom,
      y: (clientY - (rect?.top ?? 0)) - canvasPoint.y * nextZoom,
    },
  };
}

export function moodBoardItemBounds(items: MoodBoardItem[]): MoodBoardItemBounds | null {
  if (items.length === 0) return null;

  return items.reduce((bounds, item) => ({
    minX: Math.min(bounds.minX, item.x),
    minY: Math.min(bounds.minY, item.y),
    maxX: Math.max(bounds.maxX, item.x + item.width),
    maxY: Math.max(bounds.maxY, item.y + item.height),
  }), {
    minX: items[0].x,
    minY: items[0].y,
    maxX: items[0].x + items[0].width,
    maxY: items[0].y + items[0].height,
  });
}

export function moodBoardViewportForBounds(
  rect: MoodBoardViewportRect | null | undefined,
  bounds: MoodBoardItemBounds | null,
  padding = 80,
): MoodBoard["viewport"] {
  if (!rect || !bounds) return { x: 0, y: 0, zoom: 1 };

  const contentWidth = Math.max(1, bounds.maxX - bounds.minX);
  const contentHeight = Math.max(1, bounds.maxY - bounds.minY);
  const availableWidth = Math.max(1, rect.width - padding * 2);
  const availableHeight = Math.max(1, rect.height - padding * 2);
  const zoom = Math.min(2, Math.max(0.2, Math.min(availableWidth / contentWidth, availableHeight / contentHeight)));

  return {
    zoom,
    x: (rect.width - contentWidth * zoom) / 2 - bounds.minX * zoom,
    y: (rect.height - contentHeight * zoom) / 2 - bounds.minY * zoom,
  };
}
