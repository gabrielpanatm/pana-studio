import type {
  MoodBoardVectorGroupElement,
  MoodBoardVectorTransform,
} from "$lib/mood-board/model";
import {
  applySvgGroupTransformMap,
  transformedSvgGroupMap,
  type SvgGroupPath,
  type SvgGroupTransformMap,
} from "$lib/mood-board/svg-groups";
import {
  angleBetweenPoints,
  distanceBetweenPoints,
  rotateVectorTransform,
  scaleVectorTransform,
  translateVectorTransform,
  type MoodBoardPoint,
} from "$lib/mood-board/vector";

export type MoodBoardVectorGroupElementDrag = {
  mode: "move" | "scale" | "rotate";
  elementId: string;
  groupPath?: SvgGroupPath | null;
  startX: number;
  startY: number;
  startTransform: MoodBoardVectorTransform;
  startGroupTransforms?: SvgGroupTransformMap;
  currentTransform: MoodBoardVectorTransform;
  currentGroupTransforms?: SvgGroupTransformMap;
  center?: MoodBoardPoint;
  startDistance?: number;
  startAngle?: number;
};

export function updateMoodBoardVectorGroupElementDrag(
  drag: MoodBoardVectorGroupElementDrag,
  point: MoodBoardPoint,
) {
  let transform: MoodBoardVectorTransform;
  let groupTransforms: SvgGroupTransformMap | undefined;

  if (drag.mode === "scale" && drag.center && drag.startDistance) {
    const center = drag.center;
    const scale = Math.min(8, Math.max(0.12, distanceBetweenPoints(center, point) / drag.startDistance));
    if (drag.startGroupTransforms) {
      groupTransforms = transformedSvgGroupMap({
        mode: "scale",
        startTransforms: drag.startGroupTransforms,
        scale,
        centerX: center.x,
        centerY: center.y,
      });
      transform = groupTransforms[drag.elementId] ?? drag.startTransform;
    } else {
      transform = scaleVectorTransform(drag.startTransform, scale, center.x, center.y);
    }
  } else if (drag.mode === "rotate" && drag.center && drag.startAngle !== undefined) {
    const center = drag.center;
    const delta = angleBetweenPoints(center, point) - drag.startAngle;
    const degrees = delta * 180 / Math.PI;
    if (drag.startGroupTransforms) {
      groupTransforms = transformedSvgGroupMap({
        mode: "rotate",
        startTransforms: drag.startGroupTransforms,
        degrees,
        centerX: center.x,
        centerY: center.y,
      });
      transform = groupTransforms[drag.elementId] ?? drag.startTransform;
    } else {
      transform = rotateVectorTransform(drag.startTransform, degrees, center.x, center.y);
    }
  } else {
    const dx = point.x - drag.startX;
    const dy = point.y - drag.startY;
    if (drag.startGroupTransforms) {
      groupTransforms = transformedSvgGroupMap({
        mode: "move",
        startTransforms: drag.startGroupTransforms,
        dx,
        dy,
      });
      transform = groupTransforms[drag.elementId] ?? drag.startTransform;
    } else {
      transform = translateVectorTransform(drag.startTransform, dx, dy);
    }
  }

  return {
    ...drag,
    currentTransform: transform,
    currentGroupTransforms: groupTransforms,
  };
}

export function applyMoodBoardVectorGroupElementDrag(
  elements: MoodBoardVectorGroupElement[],
  drag: MoodBoardVectorGroupElementDrag,
) {
  return drag.currentGroupTransforms
    ? applySvgGroupTransformMap(elements, drag.currentGroupTransforms)
    : elements.map((element) => (
      element.id === drag.elementId ? { ...element, transform: drag.currentTransform } : element
    ));
}
