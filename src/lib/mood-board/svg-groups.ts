import type { MoodBoardVectorGroupElement } from "$lib/mood-board/model";
import { editableSvgElementNodes } from "$lib/mood-board/svg";
import {
  pathPointBounds,
  rotateVectorTransform,
  scaleVectorTransform,
  transformVectorBounds,
  translateVectorTransform,
  type MoodBoardBounds,
} from "$lib/mood-board/vector";

export type SvgGroupPath = string[];
export type SvgGroupTransformMode = "move" | "scale" | "rotate";
export type SvgGroupTransformMap = Record<string, MoodBoardVectorGroupElement["transform"]>;

export function svgGroupPathKey(path: SvgGroupPath | undefined) {
  return path?.length ? path.join("\u001f") : "";
}

export function sameSvgGroupPath(left: SvgGroupPath | undefined, right: SvgGroupPath | undefined) {
  return Boolean(left?.length && right?.length && svgGroupPathKey(left) === svgGroupPathKey(right));
}

export function selectedSvgGroupPath(element: MoodBoardVectorGroupElement | null | undefined) {
  return element?.groupPath?.length ? element.groupPath : null;
}

export function svgGroupElements(elements: MoodBoardVectorGroupElement[], path: SvgGroupPath | null) {
  if (!path?.length) return [];
  return elements.filter((element) => sameSvgGroupPath(element.groupPath, path));
}

export function duplicateSvgGroupElements(elements: MoodBoardVectorGroupElement[], path: SvgGroupPath) {
  const selected = svgGroupElements(elements, path);
  if (!selected.length) return elements;
  const selectedIds = new Set(selected.map((element) => element.id));
  const insertAfter = Math.max(...elements.map((element, index) => selectedIds.has(element.id) ? index : -1));
  const suffix = Date.now().toString(36);
  const copies = selected.map((element, index) => ({
    ...element,
    id: `${element.id}-group-copy-${suffix}-${index}`,
  }));
  const next = [...elements];
  next.splice(insertAfter + 1, 0, ...copies);
  return next;
}

export function deleteSvgGroupElements(elements: MoodBoardVectorGroupElement[], path: SvgGroupPath) {
  const next = elements.filter((element) => !sameSvgGroupPath(element.groupPath, path));
  return next.length ? next : elements;
}

export function moveSvgGroupElements(elements: MoodBoardVectorGroupElement[], path: SvgGroupPath, direction: "front" | "back") {
  const group = svgGroupElements(elements, path);
  if (!group.length) return elements;
  const groupIds = new Set(group.map((element) => element.id));
  const rest = elements.filter((element) => !groupIds.has(element.id));
  return direction === "front" ? [...rest, ...group] : [...group, ...rest];
}

function mergeBounds(bounds: MoodBoardBounds[]) {
  if (!bounds.length) return null;
  return {
    minX: Math.min(...bounds.map((entry) => entry.minX)),
    minY: Math.min(...bounds.map((entry) => entry.minY)),
    maxX: Math.max(...bounds.map((entry) => entry.maxX)),
    maxY: Math.max(...bounds.map((entry) => entry.maxY)),
  };
}

export function svgGroupElementBounds(element: MoodBoardVectorGroupElement): MoodBoardBounds | null {
  if (element.type === "path") {
    const parsed = editableSvgElementNodes(element);
    return parsed ? transformVectorBounds(pathPointBounds(parsed.nodes), element.transform) : null;
  }

  const width = Math.max(element.fontSize * 3, element.text.length * element.fontSize * 0.64);
  const bounds = {
    minX: element.x,
    minY: element.y - element.fontSize,
    maxX: element.x + width,
    maxY: element.y + element.fontSize * 0.25,
  };
  return transformVectorBounds(bounds, element.transform);
}

export function svgGroupBounds(elements: MoodBoardVectorGroupElement[], path: SvgGroupPath | null) {
  return mergeBounds(svgGroupElements(elements, path)
    .map((element) => svgGroupElementBounds(element))
    .filter((bounds): bounds is MoodBoardBounds => Boolean(bounds)));
}

export function svgGroupTransformMap(elements: MoodBoardVectorGroupElement[], path: SvgGroupPath | null): SvgGroupTransformMap {
  const map: SvgGroupTransformMap = {};
  for (const element of svgGroupElements(elements, path)) {
    map[element.id] = [...element.transform] as MoodBoardVectorGroupElement["transform"];
  }
  return map;
}

export function transformedSvgGroupMap({
  mode,
  startTransforms,
  dx = 0,
  dy = 0,
  scale = 1,
  degrees = 0,
  centerX = 0,
  centerY = 0,
}: {
  mode: SvgGroupTransformMode;
  startTransforms: SvgGroupTransformMap;
  dx?: number;
  dy?: number;
  scale?: number;
  degrees?: number;
  centerX?: number;
  centerY?: number;
}): SvgGroupTransformMap {
  const next: SvgGroupTransformMap = {};
  for (const [id, transform] of Object.entries(startTransforms)) {
    next[id] = mode === "scale"
      ? scaleVectorTransform(transform, scale, centerX, centerY)
      : mode === "rotate"
        ? rotateVectorTransform(transform, degrees, centerX, centerY)
        : translateVectorTransform(transform, dx, dy);
  }
  return next;
}

export function applySvgGroupTransformMap(
  elements: MoodBoardVectorGroupElement[],
  transforms: SvgGroupTransformMap,
) {
  return elements.map((element) => transforms[element.id] ? { ...element, transform: transforms[element.id] } : element);
}
