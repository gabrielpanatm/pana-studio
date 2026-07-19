import type {
  MoodBoardItem,
  MoodBoardVectorNode,
  MoodBoardVectorNodeEditState,
} from "$lib/mood-board/model";
import { editableSvgElementNodes } from "$lib/mood-board/svg";
import { moodBoardVectorNodeHandleMode } from "$lib/mood-board/item-view";

export function moodBoardVectorPathNodeScope(itemId: string) {
  return `${itemId}:vectorPath`;
}

export function moodBoardVectorGroupNodeScope(itemId: string, elementId: string) {
  return `${itemId}:vectorGroup:${elementId}`;
}

export function moodBoardVectorNodeScopeFromEditState(itemId: string, state: MoodBoardVectorNodeEditState) {
  if (state.itemId !== itemId) return null;
  return state.svgElementId
    ? moodBoardVectorGroupNodeScope(itemId, state.svgElementId)
    : moodBoardVectorPathNodeScope(itemId);
}

export function moodBoardVectorNodesForScope(item: MoodBoardItem, scope: string) {
  if (item.type === "vectorPath" && scope === moodBoardVectorPathNodeScope(item.id)) {
    return {
      svgElementId: null,
      nodes: item.nodes,
    };
  }

  if (item.type !== "vectorGroup") return null;

  const prefix = `${item.id}:vectorGroup:`;
  if (!scope.startsWith(prefix)) return null;
  const elementId = scope.slice(prefix.length);
  const element = item.elements.find((entry) => entry.id === elementId);
  const parsed = element?.type === "path" ? editableSvgElementNodes(element) : null;
  if (!parsed) return null;

  return {
    svgElementId: elementId,
    nodes: parsed.nodes,
  };
}

export function moodBoardVectorNodeSelectionState(
  item: MoodBoardItem,
  scope: string,
  selectedIndexes: number[],
): MoodBoardVectorNodeEditState | null {
  const data = moodBoardVectorNodesForScope(item, scope);
  const indexes = [...new Set(selectedIndexes)]
    .filter((index) => index >= 0 && index < (data?.nodes.length ?? 0))
    .sort((left, right) => left - right);

  if (!data || !indexes.length) return null;

  return {
    itemId: item.id,
    svgElementId: data.svgElementId,
    indexes,
    modes: indexes.map((index) => moodBoardVectorNodeHandleMode(data.nodes[index])),
    nodeCount: data.nodes.length,
  };
}

export function nextMoodBoardVectorNodeSelection(
  currentScope: string,
  currentIndexes: number[],
  scope: string,
  nodeIndex: number,
  additive: boolean,
) {
  if (currentScope !== scope) {
    return {
      scope,
      indexes: [nodeIndex],
    };
  }

  if (!additive) {
    return {
      scope,
      indexes: [nodeIndex],
    };
  }

  return {
    scope,
    indexes: currentIndexes.includes(nodeIndex)
      ? currentIndexes.filter((index) => index !== nodeIndex)
      : [...currentIndexes, nodeIndex],
  };
}

export function moodBoardSelectedVectorNodeSet(options: {
  itemId: string;
  scope: string;
  currentScope: string;
  currentIndexes: number[];
  editState: MoodBoardVectorNodeEditState | null;
}) {
  if (options.currentScope === options.scope && options.currentIndexes.length) {
    return new Set(options.currentIndexes);
  }

  if (
    options.editState
    && moodBoardVectorNodeScopeFromEditState(options.itemId, options.editState) === options.scope
  ) {
    return new Set(options.editState.indexes);
  }

  return new Set<number>();
}

export function moodBoardAdjacentVectorNodeIndex(index: number, direction: -1 | 1, count: number, closed: boolean) {
  const nextIndex = index + direction;
  if (nextIndex >= 0 && nextIndex < count) return nextIndex;
  if (!closed || count < 2) return null;
  return direction < 0 ? count - 1 : 0;
}

export function moodBoardVectorHandleVisible(
  selectedNodes: Set<number>,
  nodes: MoodBoardVectorNode[],
  index: number,
  handle: "in" | "out",
  closed: boolean,
) {
  if (!selectedNodes.size) return false;
  if (selectedNodes.has(index)) return true;

  const neighborIndex = handle === "in"
    ? moodBoardAdjacentVectorNodeIndex(index, -1, nodes.length, closed)
    : moodBoardAdjacentVectorNodeIndex(index, 1, nodes.length, closed);

  return neighborIndex !== null && selectedNodes.has(neighborIndex);
}

export function closestMoodBoardVectorNodeIndex(nodes: MoodBoardVectorNode[], point: { x: number; y: number }) {
  let bestIndex = 0;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (const [index, node] of nodes.entries()) {
    const distance = Math.hypot(node.x - point.x, node.y - point.y);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestIndex = index;
    }
  }
  return bestIndex;
}
