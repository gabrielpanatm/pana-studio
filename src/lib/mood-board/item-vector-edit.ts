import type {
  MoodBoardVectorGroupItem,
  MoodBoardVectorNode,
  MoodBoardVectorPathItem,
  MoodBoardVectorTransform,
} from "$lib/mood-board/model";
import { svgElementWithVectorNodes } from "$lib/mood-board/svg";
import {
  selectedSvgGroupPath,
  svgGroupBounds,
  svgGroupElements,
  svgGroupElementBounds,
  svgGroupTransformMap,
  type SvgGroupPath,
  type SvgGroupTransformMap,
} from "$lib/mood-board/svg-groups";
import {
  applyVectorTransformPoint,
  cornerSelectedVectorNodes,
  invertVectorTransform,
  removeVectorNodesByIndexes,
  setSelectedVectorNodesHandleMode,
  smoothSelectedVectorNodes,
  type MoodBoardBounds,
  type MoodBoardPoint,
} from "$lib/mood-board/vector";
import { moodBoardVectorNodeHandleMode } from "$lib/mood-board/item-view";

export type MoodBoardVectorNodeDragState = {
  kind: "vectorPath" | "vectorGroup";
  elementId?: string;
  closed?: boolean;
  nodeIndex: number;
  handle: "node" | "in" | "out";
  startX: number;
  startY: number;
  startNodes: MoodBoardVectorNode[];
  currentNodes: MoodBoardVectorNode[];
  elementTransform?: MoodBoardVectorTransform;
};

export type MoodBoardVectorGroupDragData = {
  groupPath: SvgGroupPath | null;
  startGroupTransforms?: SvgGroupTransformMap;
};

export type MoodBoardVectorNodeUpdater = (
  nodes: MoodBoardVectorNode[],
  selected: Set<number>,
  closed: boolean,
) => MoodBoardVectorNode[];

export type MoodBoardVectorNodeKeyboardAction =
  | { type: "update"; updater: MoodBoardVectorNodeUpdater }
  | { type: "clearSelection" };

export function cloneMoodBoardVectorNodes(nodes: MoodBoardVectorNode[]) {
  return nodes.map((node) => ({
    x: node.x,
    y: node.y,
    in: node.in ? { ...node.in } : null,
    out: node.out ? { ...node.out } : null,
    handleMode: moodBoardVectorNodeHandleMode(node),
  }));
}

export function moodBoardVectorPathPointFromClient(
  item: MoodBoardVectorPathItem,
  clientX: number,
  clientY: number,
  rect: DOMRect | null | undefined,
): MoodBoardPoint {
  if (!rect) return { x: 0, y: 0 };
  return {
    x: ((clientX - rect.left) / Math.max(1, rect.width)) * item.viewBoxWidth,
    y: ((clientY - rect.top) / Math.max(1, rect.height)) * item.viewBoxHeight,
  };
}

export function moodBoardVectorGroupPointFromClient(
  item: MoodBoardVectorGroupItem,
  clientX: number,
  clientY: number,
  rect: DOMRect | null | undefined,
): MoodBoardPoint {
  if (!rect) return { x: 0, y: 0 };
  return {
    x: item.viewBoxX + ((clientX - rect.left) / Math.max(1, rect.width)) * item.viewBoxWidth,
    y: item.viewBoxY + ((clientY - rect.top) / Math.max(1, rect.height)) * item.viewBoxHeight,
  };
}

export function moodBoardVectorGroupElementPointFromClient(
  item: MoodBoardVectorGroupItem,
  clientX: number,
  clientY: number,
  rect: DOMRect | null | undefined,
  transform: MoodBoardVectorTransform,
): MoodBoardPoint {
  const point = moodBoardVectorGroupPointFromClient(item, clientX, clientY, rect);
  const inverse = invertVectorTransform(transform);
  return inverse ? applyVectorTransformPoint(point, inverse) : point;
}

export function moodBoardVectorGroupActiveElementBounds(
  item: MoodBoardVectorGroupItem,
  elementId: string,
): MoodBoardBounds | null {
  const element = item.elements.find((entry) => entry.id === elementId);
  return element ? svgGroupElementBounds(element) : null;
}

export function moodBoardVectorGroupActiveGroupBounds(
  item: MoodBoardVectorGroupItem,
  elementId: string,
): MoodBoardBounds | null {
  const element = item.elements.find((entry) => entry.id === elementId);
  const groupPath = selectedSvgGroupPath(element);
  return svgGroupElements(item.elements, groupPath).length > 1
    ? svgGroupBounds(item.elements, groupPath)
    : null;
}

export function moodBoardVectorGroupActiveTransformBounds(
  item: MoodBoardVectorGroupItem,
  elementId: string,
): MoodBoardBounds | null {
  return moodBoardVectorGroupActiveGroupBounds(item, elementId)
    ?? moodBoardVectorGroupActiveElementBounds(item, elementId);
}

export function moodBoardVectorGroupDragData(
  item: MoodBoardVectorGroupItem,
  elementId: string,
): MoodBoardVectorGroupDragData {
  const element = item.elements.find((entry) => entry.id === elementId);
  const groupPath = selectedSvgGroupPath(element);
  const groupElements = svgGroupElements(item.elements, groupPath);
  return groupPath && groupElements.length > 1
    ? { groupPath, startGroupTransforms: svgGroupTransformMap(item.elements, groupPath) }
    : { groupPath: null, startGroupTransforms: undefined };
}

export function moodBoardVectorNodeKeyboardAction(event: KeyboardEvent): MoodBoardVectorNodeKeyboardAction | null {
  const key = event.key.toLowerCase();
  if (key === "delete" || key === "backspace") {
    return { type: "update", updater: removeVectorNodesByIndexes };
  }

  if (event.metaKey || event.ctrlKey || event.altKey) return null;

  if (key === "c") {
    return { type: "update", updater: cornerSelectedVectorNodes };
  }
  if (key === "s") {
    return { type: "update", updater: smoothSelectedVectorNodes };
  }
  if (key === "i") {
    return {
      type: "update",
      updater: (nodes, indexes, closed) => setSelectedVectorNodesHandleMode(nodes, indexes, closed, "independent"),
    };
  }
  if (key === "m") {
    return {
      type: "update",
      updater: (nodes, indexes, closed) => setSelectedVectorNodesHandleMode(nodes, indexes, closed, "mirrored"),
    };
  }
  if (key === "l") {
    return {
      type: "update",
      updater: (nodes, indexes, closed) => setSelectedVectorNodesHandleMode(nodes, indexes, closed, "locked"),
    };
  }
  if (key === "escape") {
    return { type: "clearSelection" };
  }

  return null;
}

export function moodBoardVectorPathWithNodes(
  item: MoodBoardVectorPathItem,
  nodes: MoodBoardVectorNode[],
): MoodBoardVectorPathItem {
  return { ...item, nodes };
}

export function moodBoardVectorGroupWithElementNodes(
  item: MoodBoardVectorGroupItem,
  elementId: string,
  nodes: MoodBoardVectorNode[],
  closed: boolean,
): MoodBoardVectorGroupItem {
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === elementId && element.type === "path"
        ? svgElementWithVectorNodes(element, nodes, closed)
        : element
    )),
  };
}
