import {
  defaultImageAdjustments,
} from "$lib/mood-board/image-adjustments";
import {
  defaultImageFraming,
} from "$lib/mood-board/image-framing";
import type {
  MoodBoardColorItem,
  MoodBoardFrameItem,
  MoodBoardImageAdjustments,
  MoodBoardImageFraming,
  MoodBoardImageItem,
  MoodBoardShapeItem,
  MoodBoardTextItem,
  MoodBoardVectorGroupElement,
  MoodBoardVectorGroupItem,
  MoodBoardVectorPathItem,
  MoodBoardVectorTransform,
} from "$lib/mood-board/model";
import {
  moodBoardImageAdjustmentValue,
  moodBoardImageFramingValue,
  moodBoardImageRadiusValue,
  moodBoardShapeStrokeWidthValue,
  moodBoardSvgOpacityValue,
  moodBoardSvgStrokeWidthValue,
  moodBoardSvgTextFontSizeValue,
  moodBoardTextFontSizeValue,
  moodBoardVectorStrokeWidthValue,
} from "$lib/mood-board/control-values";
import { editableSvgElementNodes, svgElementWithVectorNodes } from "$lib/mood-board/svg";
import {
  deleteSvgGroupElements,
  duplicateSvgGroupElements,
  moveSvgGroupElements,
  type SvgGroupPath,
} from "$lib/mood-board/svg-groups";
import {
  applyVectorTransformPoint,
  cornerVectorNodes,
  identityVectorTransform,
  insertMidpointVectorNode,
  removeLastVectorNode,
  rotateVectorTransform,
  scaleVectorTransform,
  smoothVectorNodes,
  transformedPathPointBounds,
  vectorBoundsCenter,
} from "$lib/mood-board/vector";

type SvgPathData = NonNullable<ReturnType<typeof editableSvgElementNodes>>;

export function moodBoardImageWithFit(item: MoodBoardImageItem, fit: MoodBoardImageItem["fit"]): MoodBoardImageItem {
  return { ...item, fit };
}

export function moodBoardImageWithRadius(item: MoodBoardImageItem, value: string): MoodBoardImageItem {
  return { ...item, radius: moodBoardImageRadiusValue(value) };
}

export function moodBoardImageWithToggledShadow(item: MoodBoardImageItem): MoodBoardImageItem {
  return { ...item, shadow: !item.shadow };
}

export function moodBoardImageWithAdjustment(
  item: MoodBoardImageItem,
  current: MoodBoardImageAdjustments,
  field: keyof MoodBoardImageAdjustments,
  value: string,
): MoodBoardImageItem {
  return {
    ...item,
    adjustments: {
      ...current,
      [field]: moodBoardImageAdjustmentValue(field, value, current),
    },
  };
}

export function moodBoardImageWithDefaultAdjustments(item: MoodBoardImageItem): MoodBoardImageItem {
  return { ...item, adjustments: { ...defaultImageAdjustments } };
}

export function moodBoardImageWithFraming(
  item: MoodBoardImageItem,
  current: MoodBoardImageFraming,
  field: keyof MoodBoardImageFraming,
  value: string,
): MoodBoardImageItem {
  return {
    ...item,
    framing: {
      ...current,
      [field]: moodBoardImageFramingValue(field, value, current),
    },
  };
}

export function moodBoardImageWithDefaultFraming(item: MoodBoardImageItem): MoodBoardImageItem {
  return { ...item, framing: { ...defaultImageFraming } };
}

export function moodBoardImageWithPath(item: MoodBoardImageItem, path: string): MoodBoardImageItem {
  return { ...item, path: path.trim().replaceAll("\\", "/") };
}

export function moodBoardTextWithColor(item: MoodBoardTextItem, color: string): MoodBoardTextItem {
  return { ...item, color };
}

export function moodBoardTextWithFontSize(item: MoodBoardTextItem, value: string): MoodBoardTextItem {
  return { ...item, fontSize: moodBoardTextFontSizeValue(value) };
}

export function moodBoardTextWithToggledWeight(item: MoodBoardTextItem): MoodBoardTextItem {
  return { ...item, fontWeight: item.fontWeight >= 700 ? 400 : 800 };
}

export function moodBoardTextWithAlign(item: MoodBoardTextItem, textAlign: MoodBoardTextItem["textAlign"]): MoodBoardTextItem {
  return { ...item, textAlign };
}

export function moodBoardShapeWithKind(item: MoodBoardShapeItem, value: string): MoodBoardShapeItem | null {
  if (value !== "rectangle" && value !== "ellipse" && value !== "diamond") return null;
  return { ...item, shape: value };
}

export function moodBoardShapeWithFill(item: MoodBoardShapeItem, fill: string): MoodBoardShapeItem {
  return { ...item, fill };
}

export function moodBoardShapeWithStroke(item: MoodBoardShapeItem, stroke: string): MoodBoardShapeItem {
  return { ...item, stroke };
}

export function moodBoardShapeWithStrokeWidth(item: MoodBoardShapeItem, value: string): MoodBoardShapeItem {
  return { ...item, strokeWidth: moodBoardShapeStrokeWidthValue(value) };
}

export function moodBoardVectorWithFill(item: MoodBoardVectorPathItem, fill: string): MoodBoardVectorPathItem {
  return { ...item, fill };
}

export function moodBoardVectorWithStroke(item: MoodBoardVectorPathItem, stroke: string): MoodBoardVectorPathItem {
  return { ...item, stroke };
}

export function moodBoardVectorWithStrokeWidth(item: MoodBoardVectorPathItem, value: string): MoodBoardVectorPathItem {
  return { ...item, strokeWidth: moodBoardVectorStrokeWidthValue(value) };
}

export function moodBoardVectorWithToggledClosed(item: MoodBoardVectorPathItem): MoodBoardVectorPathItem {
  return { ...item, closed: !item.closed };
}

export function moodBoardVectorWithCornerNodes(item: MoodBoardVectorPathItem): MoodBoardVectorPathItem {
  return { ...item, nodes: cornerVectorNodes(item.nodes) };
}

export function moodBoardVectorWithSmoothNodes(item: MoodBoardVectorPathItem): MoodBoardVectorPathItem {
  return { ...item, nodes: smoothVectorNodes(item.nodes, item.closed) };
}

export function moodBoardVectorWithInsertedNode(item: MoodBoardVectorPathItem): MoodBoardVectorPathItem {
  return { ...item, nodes: insertMidpointVectorNode(item.nodes, item.closed) };
}

export function moodBoardVectorWithRemovedNode(item: MoodBoardVectorPathItem): MoodBoardVectorPathItem {
  return { ...item, nodes: removeLastVectorNode(item.nodes, item.closed) };
}

export function moodBoardFrameWithTone(item: MoodBoardFrameItem, tone: string): MoodBoardFrameItem {
  return { ...item, tone };
}

export function moodBoardFrameWithBackground(item: MoodBoardFrameItem, background: string): MoodBoardFrameItem {
  return { ...item, background };
}

export function moodBoardColorWithValue(item: MoodBoardColorItem, color: string): MoodBoardColorItem {
  return { ...item, color };
}

function mapSelectedSvgElements(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string | null,
  updater: (element: MoodBoardVectorGroupElement) => MoodBoardVectorGroupElement,
): MoodBoardVectorGroupItem {
  return {
    ...item,
    elements: item.elements.map((element) => (
      !selectedElementId || element.id === selectedElementId ? updater(element) : element
    )),
  };
}

export function moodBoardSvgGroupWithFill(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string | null,
  fill: string,
): MoodBoardVectorGroupItem {
  return mapSelectedSvgElements(item, selectedElementId, (element) => ({ ...element, fill }));
}

export function moodBoardSvgGroupWithStroke(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string | null,
  stroke: string,
): MoodBoardVectorGroupItem {
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.type === "path" && (!selectedElementId || element.id === selectedElementId)
        ? { ...element, stroke }
        : element
    )),
  };
}

export function moodBoardSvgGroupWithStrokeWidth(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string | null,
  value: string,
): MoodBoardVectorGroupItem {
  const strokeWidth = moodBoardSvgStrokeWidthValue(value);
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.type === "path" && (!selectedElementId || element.id === selectedElementId)
        ? { ...element, strokeWidth }
        : element
    )),
  };
}

export function moodBoardSvgGroupWithOpacity(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string | null,
  value: string,
): MoodBoardVectorGroupItem {
  const opacity = moodBoardSvgOpacityValue(value);
  return mapSelectedSvgElements(item, selectedElementId, (element) => ({ ...element, opacity }));
}

export function moodBoardSvgGroupWithText(
  item: MoodBoardVectorGroupItem,
  textElementId: string,
  text: string,
): MoodBoardVectorGroupItem {
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === textElementId && element.type === "text" ? { ...element, text } : element
    )),
  };
}

export function moodBoardSvgGroupWithTextFontSize(
  item: MoodBoardVectorGroupItem,
  textElementId: string,
  value: string,
): MoodBoardVectorGroupItem {
  const fontSize = moodBoardSvgTextFontSizeValue(value);
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === textElementId && element.type === "text" ? { ...element, fontSize } : element
    )),
  };
}

export function moodBoardSvgGroupWithToggledTextWeight(
  item: MoodBoardVectorGroupItem,
  textElementId: string,
): MoodBoardVectorGroupItem {
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === textElementId && element.type === "text"
        ? { ...element, fontWeight: element.fontWeight >= 700 ? 400 : 800 }
        : element
    )),
  };
}

export function moodBoardSvgGroupWithMovedElement(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
  direction: "front" | "back",
): MoodBoardVectorGroupItem | null {
  const index = item.elements.findIndex((element) => element.id === selectedElementId);
  if (index < 0) return null;
  const nextIndex = direction === "front"
    ? Math.min(item.elements.length - 1, index + 1)
    : Math.max(0, index - 1);
  if (nextIndex === index) return null;

  const elements = [...item.elements];
  const [element] = elements.splice(index, 1);
  elements.splice(nextIndex, 0, element);
  return { ...item, elements };
}

export function moodBoardSvgGroupWithDuplicatedElement(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem | null {
  const index = item.elements.findIndex((element) => element.id === selectedElementId);
  const element = item.elements[index];
  if (!element) return null;

  const elements = [...item.elements];
  elements.splice(index + 1, 0, {
    ...element,
    id: `${element.id}-copy-${Date.now().toString(36)}`,
  });
  return { ...item, elements };
}

export function moodBoardSvgGroupWithoutElement(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem | null {
  if (item.elements.length <= 1) return null;
  const elements = item.elements.filter((element) => element.id !== selectedElementId);
  if (elements.length === item.elements.length) return null;
  return { ...item, elements };
}

export function moodBoardSvgGroupWithDuplicatedGroup(
  item: MoodBoardVectorGroupItem,
  groupPath: SvgGroupPath,
): MoodBoardVectorGroupItem {
  return { ...item, elements: duplicateSvgGroupElements(item.elements, groupPath) };
}

export function moodBoardSvgGroupWithoutGroup(
  item: MoodBoardVectorGroupItem,
  groupPath: SvgGroupPath,
): MoodBoardVectorGroupItem | null {
  const elements = deleteSvgGroupElements(item.elements, groupPath);
  if (elements === item.elements) return null;
  return { ...item, elements };
}

export function moodBoardSvgGroupWithMovedGroup(
  item: MoodBoardVectorGroupItem,
  groupPath: SvgGroupPath,
  direction: "front" | "back",
): MoodBoardVectorGroupItem {
  return { ...item, elements: moveSvgGroupElements(item.elements, groupPath, direction) };
}

function updateSvgPathData(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
  updater: (parsed: SvgPathData) => SvgPathData,
): MoodBoardVectorGroupItem | null {
  const selectedElement = item.elements.find((element) => element.id === selectedElementId);
  if (!selectedElement || selectedElement.type !== "path") return null;
  const pathData = editableSvgElementNodes(selectedElement);
  if (!pathData) return null;
  const parsed = updater(pathData);
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === selectedElement.id && element.type === "path"
        ? svgElementWithVectorNodes(element, parsed.nodes, parsed.closed)
        : element
    )),
  };
}

export function moodBoardSvgGroupWithToggledSelectedPathClosed(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem | null {
  return updateSvgPathData(item, selectedElementId, (parsed) => ({ ...parsed, closed: !parsed.closed }));
}

export function moodBoardSvgGroupWithCornerSelectedPath(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem | null {
  return updateSvgPathData(item, selectedElementId, (parsed) => ({ ...parsed, nodes: cornerVectorNodes(parsed.nodes) }));
}

export function moodBoardSvgGroupWithSmoothSelectedPath(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem | null {
  return updateSvgPathData(item, selectedElementId, (parsed) => ({ ...parsed, nodes: smoothVectorNodes(parsed.nodes, parsed.closed) }));
}

export function moodBoardSvgGroupWithInsertedSelectedPathNode(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem | null {
  return updateSvgPathData(item, selectedElementId, (parsed) => ({
    ...parsed,
    nodes: insertMidpointVectorNode(parsed.nodes, parsed.closed),
  }));
}

export function moodBoardSvgGroupWithRemovedSelectedPathNode(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem | null {
  return updateSvgPathData(item, selectedElementId, (parsed) => ({
    ...parsed,
    nodes: removeLastVectorNode(parsed.nodes, parsed.closed),
  }));
}

function svgElementTransformCenter(element: MoodBoardVectorGroupElement) {
  if (element.type === "path") {
    const pathData = editableSvgElementNodes(element);
    if (!pathData) return null;
    return vectorBoundsCenter(transformedPathPointBounds(pathData.nodes, element.transform));
  }

  const approxWidth = Math.max(element.fontSize, element.text.length * element.fontSize * 0.55);
  return applyVectorTransformPoint({
    x: element.x + approxWidth / 2,
    y: element.y - element.fontSize / 2,
  }, element.transform);
}

function updateSelectedSvgElementTransform(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
  updater: (transform: MoodBoardVectorTransform, center: { x: number; y: number }) => MoodBoardVectorTransform,
): MoodBoardVectorGroupItem | null {
  const selectedElement = item.elements.find((element) => element.id === selectedElementId);
  if (!selectedElement) return null;
  const center = svgElementTransformCenter(selectedElement);
  if (!center) return null;
  const transform = updater(selectedElement.transform, center);
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === selectedElement.id ? { ...element, transform } : element
    )),
  };
}

export function moodBoardSvgGroupWithScaledElement(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
  scale: number,
): MoodBoardVectorGroupItem | null {
  return updateSelectedSvgElementTransform(
    item,
    selectedElementId,
    (transform, center) => scaleVectorTransform(transform, scale, center.x, center.y),
  );
}

export function moodBoardSvgGroupWithRotatedElement(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
  degrees: number,
): MoodBoardVectorGroupItem | null {
  return updateSelectedSvgElementTransform(
    item,
    selectedElementId,
    (transform, center) => rotateVectorTransform(transform, degrees, center.x, center.y),
  );
}

export function moodBoardSvgGroupWithResetElementTransform(
  item: MoodBoardVectorGroupItem,
  selectedElementId: string,
): MoodBoardVectorGroupItem {
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === selectedElementId ? { ...element, transform: identityVectorTransform } : element
    )),
  };
}
