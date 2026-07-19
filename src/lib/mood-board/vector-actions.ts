import {
  cloneMoodBoard,
  findMoodBoardItem,
  mapMoodBoardItems,
  type MoodBoard,
  type MoodBoardItem,
  type MoodBoardVectorNode,
} from "$lib/mood-board/model";
import { svgSubPathToVectorPath } from "$lib/mood-board/svg";
import { shapeToVectorPath } from "$lib/mood-board/vector";
import { mutateContainingList } from "$lib/mood-board/tree";

type MoodBoardImageItem = Extract<MoodBoardItem, { type: "image" }>;
type MoodBoardVectorPathItem = Extract<MoodBoardItem, { type: "vectorPath" }>;

export type MoodBoardVectorActionStatus = {
  text: string;
  kind: "saved" | "error";
};

export type MoodBoardVectorActionResult = {
  board?: MoodBoard;
  selectedItemIds?: string[];
  selectedSvgElementId?: string | null;
  status?: MoodBoardVectorActionStatus;
};

export function selectedImageAndVectorPath(board: MoodBoard, selectedItemIds: string[]) {
  const selected = selectedItemIds
    .map((id) => findMoodBoardItem(board.items, id))
    .filter((item): item is MoodBoardItem => Boolean(item));
  const image = selected.find((item) => item.type === "image");
  const vectorPath = selected.find((item) => item.type === "vectorPath");
  return image?.type === "image" && vectorPath?.type === "vectorPath" ? { image, vectorPath } : null;
}

export function canApplyVectorMask(board: MoodBoard, selectedItemIds: string[], itemId: string) {
  const pair = selectedImageAndVectorPath(board, selectedItemIds);
  return Boolean(pair && pair.image.id === itemId);
}

export function applyVectorMaskToImage(
  board: MoodBoard,
  selectedItemIds: string[],
  itemId: string,
): MoodBoardVectorActionResult | null {
  const pair = selectedImageAndVectorPath(board, selectedItemIds);
  if (!pair || pair.image.id !== itemId) return null;

  const next = cloneMoodBoard(board);
  const maskMode = itemsOverlap(pair.image, pair.vectorPath) ? "canvas-position" : "fit-to-image";
  next.items = mapMoodBoardItems(next.items, (item) => {
    if (item.id !== pair.image.id || item.type !== "image") return item;
    return {
      ...item,
      radius: 0,
      mask: {
        nodes: pair.vectorPath.nodes.map((node) => vectorNodeToImageMaskNode(node, pair.vectorPath, pair.image, maskMode)),
        closed: pair.vectorPath.closed,
        viewBoxWidth: pair.image.width,
        viewBoxHeight: pair.image.height,
      },
    };
  });

  return { board: next, selectedItemIds: [pair.image.id], selectedSvgElementId: null };
}

export function clearVectorMaskFromImage(board: MoodBoard, itemId: string): MoodBoard | null {
  const image = findMoodBoardItem(board.items, itemId);
  if (!image || image.type !== "image" || !image.mask) return null;
  const next = cloneMoodBoard(board);
  next.items = mapMoodBoardItems(next.items, (item) => item.id === itemId && item.type === "image"
    ? { ...item, mask: null }
    : item);
  return next;
}

export function extractSvgSubPath(
  board: MoodBoard,
  itemId: string,
  elementId: string,
): MoodBoardVectorActionResult | null {
  const group = findMoodBoardItem(board.items, itemId);
  if (!group || group.type !== "vectorGroup") return null;
  const element = group.elements.find((entry) => entry.id === elementId);
  if (!element) return null;
  const vectorPath = svgSubPathToVectorPath(group, element);
  if (!vectorPath) {
    return {
      status: {
        text: "Sub-path-ul SVG folosește comenzi care nu sunt încă editabile ca Bezier.",
        kind: "error",
      },
    };
  }

  const next = cloneMoodBoard(board);
  const changed = mutateContainingList(next.items, itemId, (items, index) => {
    items.splice(index + 1, 0, vectorPath);
  });
  if (!changed) return null;

  return {
    board: next,
    selectedItemIds: [vectorPath.id],
    selectedSvgElementId: null,
    status: { text: "Sub-path SVG extras ca path editabil.", kind: "saved" },
  };
}

export function extractAllSvgSubPaths(board: MoodBoard, itemId: string): MoodBoardVectorActionResult | null {
  const group = findMoodBoardItem(board.items, itemId);
  if (!group || group.type !== "vectorGroup") return null;
  const vectorPaths = group.elements
    .map((element) => svgSubPathToVectorPath(group, element))
    .filter((item): item is MoodBoardVectorPathItem => Boolean(item));

  if (!vectorPaths.length) {
    return {
      status: {
        text: "SVG-ul nu are sub-path-uri extractibile ca Bezier.",
        kind: "error",
      },
    };
  }

  const next = cloneMoodBoard(board);
  const changed = mutateContainingList(next.items, itemId, (items, index) => {
    items.splice(index + 1, 0, ...vectorPaths);
  });
  if (!changed) return null;

  return {
    board: next,
    selectedItemIds: vectorPaths.map((item) => item.id),
    selectedSvgElementId: null,
    status: { text: `${vectorPaths.length} sub-path-uri SVG extrase ca path-uri editabile.`, kind: "saved" },
  };
}

export function ungroupVectorGroup(board: MoodBoard, itemId: string): MoodBoardVectorActionResult | null {
  const group = findMoodBoardItem(board.items, itemId);
  if (!group || group.type !== "vectorGroup") return null;
  const vectorPaths = group.elements
    .map((element) => svgSubPathToVectorPath(group, element))
    .filter((item): item is MoodBoardVectorPathItem => Boolean(item));

  if (!vectorPaths.length) {
    return {
      status: {
        text: "Grupul vectorial nu are path-uri compatibile pentru degrupare.",
        kind: "error",
      },
    };
  }

  const next = cloneMoodBoard(board);
  const changed = mutateContainingList(next.items, itemId, (items, index) => {
    items.splice(index, 1, ...vectorPaths);
  });
  if (!changed) return null;

  return {
    board: next,
    selectedItemIds: vectorPaths.map((item) => item.id),
    selectedSvgElementId: null,
    status: { text: `${vectorPaths.length} path-uri create din grupul vectorial.`, kind: "saved" },
  };
}

export function convertShapeToPath(board: MoodBoard, itemId: string): MoodBoardVectorActionResult | null {
  const shape = findMoodBoardItem(board.items, itemId);
  if (!shape || shape.type !== "shape") return null;
  const vectorPath = shapeToVectorPath(shape);
  const next = cloneMoodBoard(board);
  const changed = mutateContainingList(next.items, itemId, (items, index) => {
    items.splice(index, 1, vectorPath);
  });
  if (!changed) return null;

  return {
    board: next,
    selectedItemIds: [vectorPath.id],
    selectedSvgElementId: null,
    status: { text: "Forma a fost convertită în path editabil.", kind: "saved" },
  };
}

function vectorNodeToImageMaskNode(
  node: MoodBoardVectorNode,
  vectorPath: MoodBoardVectorPathItem,
  image: MoodBoardImageItem,
  mode: "canvas-position" | "fit-to-image",
): MoodBoardVectorNode {
  const scaleX = mode === "fit-to-image"
    ? image.width / Math.max(1, vectorPath.viewBoxWidth)
    : vectorPath.width / Math.max(1, vectorPath.viewBoxWidth);
  const scaleY = mode === "fit-to-image"
    ? image.height / Math.max(1, vectorPath.viewBoxHeight)
    : vectorPath.height / Math.max(1, vectorPath.viewBoxHeight);
  const originX = mode === "fit-to-image" ? 0 : vectorPath.x - image.x;
  const originY = mode === "fit-to-image" ? 0 : vectorPath.y - image.y;
  return {
    x: Math.round((originX + node.x * scaleX) * 10) / 10,
    y: Math.round((originY + node.y * scaleY) * 10) / 10,
    in: node.in ? {
      x: Math.round(node.in.x * scaleX * 10) / 10,
      y: Math.round(node.in.y * scaleY * 10) / 10,
    } : null,
    out: node.out ? {
      x: Math.round(node.out.x * scaleX * 10) / 10,
      y: Math.round(node.out.y * scaleY * 10) / 10,
    } : null,
    handleMode: node.handleMode,
  };
}

function itemsOverlap(left: MoodBoardItem, right: MoodBoardItem) {
  return left.x < right.x + right.width
    && left.x + left.width > right.x
    && left.y < right.y + right.height
    && left.y + left.height > right.y;
}
