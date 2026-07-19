import type {
  MoodBoardImageMask,
  MoodBoardItem,
  MoodBoardVectorHandleMode,
  MoodBoardVectorNode,
  MoodBoardVectorTransform,
} from "$lib/mood-board/model";
import {
  applyVectorTransformPoint,
  buildVectorSvgPath,
  transformedPathPointBounds,
  vectorControlPoint,
  type MoodBoardPoint,
} from "$lib/mood-board/vector";

export function cloneMoodBoardItem<T extends MoodBoardItem>(item: T): T {
  return JSON.parse(JSON.stringify(item)) as T;
}

export function moodBoardItemStyle(item: MoodBoardItem) {
  const base = `width:${item.width}px;height:${item.height}px;transform:translate3d(${item.x}px,${item.y}px,0);`;
  if (item.type === "frame") return `${base}--frame-tone:${item.tone};--frame-bg:${item.background};`;
  if (item.type === "shape") {
    return `${base}--shape-fill:${item.fill};--shape-stroke:${item.stroke};--shape-stroke-width:${item.strokeWidth}px;`;
  }
  if (item.type === "vectorPath") {
    return `${base}--vector-fill:${item.fill};--vector-stroke:${item.stroke};--vector-stroke-width:${item.strokeWidth}px;`;
  }
  return base;
}

export function moodBoardImageMaskPath(mask: MoodBoardImageMask) {
  return buildVectorSvgPath({
    id: "mask",
    type: "vectorPath",
    x: 0,
    y: 0,
    width: mask.viewBoxWidth,
    height: mask.viewBoxHeight,
    nodes: mask.nodes,
    closed: mask.closed,
    fill: "#000000",
    stroke: "#000000",
    strokeWidth: 0,
    viewBoxWidth: mask.viewBoxWidth,
    viewBoxHeight: mask.viewBoxHeight,
  });
}

export function moodBoardVectorNodeHandleMode(node: MoodBoardVectorNode): MoodBoardVectorHandleMode {
  return node.handleMode ?? (node.in || node.out ? "independent" : "corner");
}

export function moodBoardVectorPathDisplayPoint(
  item: Extract<MoodBoardItem, { type: "vectorPath" }>,
  point: MoodBoardPoint,
) {
  return {
    x: point.x / Math.max(1, item.viewBoxWidth) * item.width,
    y: point.y / Math.max(1, item.viewBoxHeight) * item.height,
  };
}

export function moodBoardVectorPathDisplayHandle(
  item: Extract<MoodBoardItem, { type: "vectorPath" }>,
  node: MoodBoardVectorNode,
  handle: "in" | "out",
) {
  return moodBoardVectorPathDisplayPoint(item, vectorControlPoint(node, handle));
}

export function moodBoardVectorNodeStyle(item: Extract<MoodBoardItem, { type: "vectorPath" }>, node: MoodBoardVectorNode) {
  const point = moodBoardVectorPathDisplayPoint(item, node);
  return `left:${point.x}px;top:${point.y}px;`;
}

export function moodBoardVectorHandleStyle(
  item: Extract<MoodBoardItem, { type: "vectorPath" }>,
  node: MoodBoardVectorNode,
  handle: "in" | "out",
) {
  const point = moodBoardVectorPathDisplayHandle(item, node, handle);
  return `left:${point.x}px;top:${point.y}px;`;
}

export function moodBoardVectorHandleLineStyle(first: MoodBoardPoint, second: MoodBoardPoint) {
  const dx = second.x - first.x;
  const dy = second.y - first.y;
  const length = Math.hypot(dx, dy);
  const angle = Math.atan2(dy, dx) * 180 / Math.PI;
  return `left:${first.x}px;top:${first.y}px;width:${length}px;transform:rotate(${angle}deg);`;
}

export function moodBoardVectorHandleLineStyleFromNode(
  item: Extract<MoodBoardItem, { type: "vectorPath" }>,
  node: MoodBoardVectorNode,
  handle: "in" | "out",
) {
  return moodBoardVectorHandleLineStyle(
    moodBoardVectorPathDisplayPoint(item, node),
    moodBoardVectorPathDisplayHandle(item, node, handle),
  );
}

export function moodBoardVectorGroupDisplayPoint(
  item: Extract<MoodBoardItem, { type: "vectorGroup" }>,
  point: MoodBoardPoint,
) {
  return {
    x: (point.x - item.viewBoxX) / Math.max(1, item.viewBoxWidth) * item.width,
    y: (point.y - item.viewBoxY) / Math.max(1, item.viewBoxHeight) * item.height,
  };
}

export function moodBoardVectorGroupElementDisplayPoint(
  item: Extract<MoodBoardItem, { type: "vectorGroup" }>,
  point: MoodBoardPoint,
  transform: MoodBoardVectorTransform,
) {
  return moodBoardVectorGroupDisplayPoint(item, applyVectorTransformPoint(point, transform));
}

export function moodBoardVectorGroupHandleOffset(item: Extract<MoodBoardItem, { type: "vectorGroup" }>) {
  return Math.max(item.viewBoxWidth / Math.max(1, item.width), item.viewBoxHeight / Math.max(1, item.height)) * 28;
}

export function moodBoardSvgTextEditWidth(text: string, fontSize: number) {
  return Math.max(fontSize * 3, text.length * fontSize * 0.64);
}

export function moodBoardSelectedElementTransformedBounds(nodes: MoodBoardVectorNode[], transform: MoodBoardVectorTransform) {
  return transformedPathPointBounds(nodes, transform);
}
