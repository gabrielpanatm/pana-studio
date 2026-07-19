import type { MoodBoardPoint } from "$lib/mood-board/factory";
import type { MoodBoardVectorNode } from "$lib/mood-board/model";
import {
  buildStraightPath,
  createVectorPathItemFromNodes,
  localizeVectorNodes,
  vectorBoundsFromCanvasNodes,
} from "$lib/mood-board/vector";

export function createMoodBoardVectorPathFromPen(nodes: MoodBoardVectorNode[], closed: boolean) {
  return createVectorPathItemFromNodes({
    nodes,
    closed,
    fill: closed ? "#d8eee8" : "transparent",
  });
}

export function moodBoardPenDraftPreviewBounds(nodes: MoodBoardVectorNode[]) {
  if (!nodes.length) return null;

  const bounds = vectorBoundsFromCanvasNodes(nodes);
  const localizedNodes = localizeVectorNodes(nodes, bounds.x, bounds.y);

  return {
    ...bounds,
    nodes: localizedNodes,
    path: buildStraightPath(localizedNodes, false),
  };
}

export function isMoodBoardPenCloseHit(nodes: MoodBoardVectorNode[], point: MoodBoardPoint, zoom: number) {
  const first = nodes[0];
  const closeDistance = 12 / Math.max(0.2, zoom || 1);
  return Boolean(
    first
    && nodes.length >= 3
    && Math.hypot(point.x - first.x, point.y - first.y) <= closeDistance,
  );
}

export function appendMoodBoardPenNode(nodes: MoodBoardVectorNode[], point: MoodBoardPoint) {
  return [...nodes, { x: point.x, y: point.y, in: null, out: null }];
}
