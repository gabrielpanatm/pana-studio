import {
  createMoodBoardItemId,
  type MoodBoardItem,
  type MoodBoardVectorHandle,
  type MoodBoardVectorHandleMode,
  type MoodBoardVectorNode,
  type MoodBoardVectorTransform,
} from "$lib/mood-board/model";

export type MoodBoardPoint = {
  x: number;
  y: number;
};

export type MoodBoardBounds = {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
};

export const identityVectorTransform: MoodBoardVectorTransform = [1, 0, 0, 1, 0, 0];

function roundedMatrixValue(value: number) {
  return Math.round(value * 1000) / 1000;
}

export function multiplyVectorTransform(left: MoodBoardVectorTransform, right: MoodBoardVectorTransform): MoodBoardVectorTransform {
  const [a1, b1, c1, d1, e1, f1] = left;
  const [a2, b2, c2, d2, e2, f2] = right;
  return [
    roundedMatrixValue(a1 * a2 + c1 * b2),
    roundedMatrixValue(b1 * a2 + d1 * b2),
    roundedMatrixValue(a1 * c2 + c1 * d2),
    roundedMatrixValue(b1 * c2 + d1 * d2),
    roundedMatrixValue(a1 * e2 + c1 * f2 + e1),
    roundedMatrixValue(b1 * e2 + d1 * f2 + f1),
  ];
}

export function translateVectorTransform(
  transform: MoodBoardVectorTransform,
  dx: number,
  dy: number,
): MoodBoardVectorTransform {
  return multiplyVectorTransform([1, 0, 0, 1, dx, dy], transform);
}

export function scaleVectorTransform(
  transform: MoodBoardVectorTransform,
  scale: number,
  centerX: number,
  centerY: number,
): MoodBoardVectorTransform {
  return multiplyVectorTransform(
    [1, 0, 0, 1, centerX, centerY],
    multiplyVectorTransform(
      [scale, 0, 0, scale, 0, 0],
      multiplyVectorTransform([1, 0, 0, 1, -centerX, -centerY], transform),
    ),
  );
}

export function rotateVectorTransform(
  transform: MoodBoardVectorTransform,
  degrees: number,
  centerX: number,
  centerY: number,
): MoodBoardVectorTransform {
  const radians = degrees * Math.PI / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  return multiplyVectorTransform(
    [1, 0, 0, 1, centerX, centerY],
    multiplyVectorTransform(
      [cos, sin, -sin, cos, 0, 0],
      multiplyVectorTransform([1, 0, 0, 1, -centerX, -centerY], transform),
    ),
  );
}

export function invertVectorTransform(transform: MoodBoardVectorTransform): MoodBoardVectorTransform | null {
  const [a, b, c, d, e, f] = transform;
  const determinant = a * d - b * c;
  if (Math.abs(determinant) < 0.000001) return null;
  return [
    roundedMatrixValue(d / determinant),
    roundedMatrixValue(-b / determinant),
    roundedMatrixValue(-c / determinant),
    roundedMatrixValue(a / determinant),
    roundedMatrixValue((c * f - d * e) / determinant),
    roundedMatrixValue((b * e - a * f) / determinant),
  ];
}

export function applyVectorTransformPoint(point: MoodBoardPoint, transform: MoodBoardVectorTransform): MoodBoardPoint {
  const [a, b, c, d, e, f] = transform;
  return {
    x: roundedMatrixValue(a * point.x + c * point.y + e),
    y: roundedMatrixValue(b * point.x + d * point.y + f),
  };
}

export function vectorBoundsCenter(bounds: MoodBoardBounds): MoodBoardPoint {
  return {
    x: (bounds.minX + bounds.maxX) / 2,
    y: (bounds.minY + bounds.maxY) / 2,
  };
}

export function transformVectorBounds(bounds: MoodBoardBounds, transform: MoodBoardVectorTransform): MoodBoardBounds {
  const points = [
    applyVectorTransformPoint({ x: bounds.minX, y: bounds.minY }, transform),
    applyVectorTransformPoint({ x: bounds.maxX, y: bounds.minY }, transform),
    applyVectorTransformPoint({ x: bounds.maxX, y: bounds.maxY }, transform),
    applyVectorTransformPoint({ x: bounds.minX, y: bounds.maxY }, transform),
  ];
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  return {
    minX: Math.min(...xs),
    minY: Math.min(...ys),
    maxX: Math.max(...xs),
    maxY: Math.max(...ys),
  };
}

export function transformedPathPointBounds(nodes: MoodBoardVectorNode[], transform: MoodBoardVectorTransform): MoodBoardBounds {
  return transformVectorBounds(pathPointBounds(nodes), transform);
}

export function distanceBetweenPoints(first: MoodBoardPoint, second: MoodBoardPoint) {
  return Math.hypot(second.x - first.x, second.y - first.y);
}

export function angleBetweenPoints(first: MoodBoardPoint, second: MoodBoardPoint) {
  return Math.atan2(second.y - first.y, second.x - first.x);
}

export function buildStraightPath(nodes: MoodBoardVectorNode[], closed = false) {
  if (!nodes.length) return "";
  const [first, ...rest] = nodes;
  const d = [`M ${first.x} ${first.y}`, ...rest.map((node) => `L ${node.x} ${node.y}`)];
  if (closed && nodes.length >= 3) d.push("Z");
  return d.join(" ");
}

export function vectorControlPoint(node: MoodBoardVectorNode, handle: "in" | "out") {
  const offset = node[handle];
  return offset ? { x: node.x + offset.x, y: node.y + offset.y } : { x: node.x, y: node.y };
}

export function buildVectorSvgPath(vectorPath: Extract<MoodBoardItem, { type: "vectorPath" }>) {
  const nodes = vectorPath.nodes;
  if (!nodes.length) return "";
  const first = nodes[0];
  let previous = first;
  let path = `M ${first.x} ${first.y}`;

  for (const node of nodes.slice(1)) {
    const c1 = vectorControlPoint(previous, "out");
    const c2 = vectorControlPoint(node, "in");
    path += previous.out || node.in
      ? ` C ${c1.x} ${c1.y} ${c2.x} ${c2.y} ${node.x} ${node.y}`
      : ` L ${node.x} ${node.y}`;
    previous = node;
  }

  if (vectorPath.closed) {
    const c1 = vectorControlPoint(previous, "out");
    const c2 = vectorControlPoint(first, "in");
    if (previous.out || first.in) {
      path += ` C ${c1.x} ${c1.y} ${c2.x} ${c2.y} ${first.x} ${first.y}`;
    }
    path += " Z";
  }

  return path;
}

export function pathPointBounds(nodes: MoodBoardVectorNode[]): MoodBoardBounds {
  const points = nodes.flatMap((node) => {
    const result = [{ x: node.x, y: node.y }];
    if (node.in) result.push({ x: node.x + node.in.x, y: node.y + node.in.y });
    if (node.out) result.push({ x: node.x + node.out.x, y: node.y + node.out.y });
    return result;
  });
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  return {
    minX: Math.min(...xs),
    minY: Math.min(...ys),
    maxX: Math.max(...xs),
    maxY: Math.max(...ys),
  };
}

export function vectorBoundsFromCanvasNodes(nodes: MoodBoardVectorNode[]) {
  const xs = nodes.map((node) => node.x);
  const ys = nodes.map((node) => node.y);
  const minX = Math.min(...xs);
  const minY = Math.min(...ys);
  const maxX = Math.max(...xs);
  const maxY = Math.max(...ys);
  const padding = 16;
  const x = minX - padding;
  const y = minY - padding;
  const width = Math.max(80, maxX - minX + padding * 2);
  const height = Math.max(80, maxY - minY + padding * 2);
  return { x, y, width, height };
}

export function localizeVectorNodes(nodes: MoodBoardVectorNode[], x: number, y: number) {
  return nodes.map((node) => ({
    x: Math.round((node.x - x) * 10) / 10,
    y: Math.round((node.y - y) * 10) / 10,
    in: node.in ? { ...node.in } : null,
    out: node.out ? { ...node.out } : null,
    handleMode: node.handleMode ?? (node.in || node.out ? "independent" : "corner"),
  }));
}

export function createVectorPathItemFromNodes({
  nodes,
  closed,
  fill = "#d8eee8",
  stroke = "#1d7f6a",
  strokeWidth = 3,
}: {
  nodes: MoodBoardVectorNode[];
  closed: boolean;
  fill?: string;
  stroke?: string;
  strokeWidth?: number;
}): Extract<MoodBoardItem, { type: "vectorPath" }> | null {
  if (nodes.length < 2) return null;
  const bounds = vectorBoundsFromCanvasNodes(nodes);
  return {
    id: createMoodBoardItemId(),
    type: "vectorPath",
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    nodes: localizeVectorNodes(nodes, bounds.x, bounds.y),
    closed,
    fill,
    stroke,
    strokeWidth,
    viewBoxWidth: bounds.width,
    viewBoxHeight: bounds.height,
  };
}

function roundedPoint(value: number) {
  return Math.round(value * 10) / 10;
}

function roundedHandle(handle: MoodBoardVectorHandle): MoodBoardVectorHandle {
  return {
    x: roundedPoint(handle.x),
    y: roundedPoint(handle.y),
  };
}

function oppositeHandle(handle: MoodBoardVectorHandle): MoodBoardVectorHandle {
  return {
    x: roundedPoint(-handle.x),
    y: roundedPoint(-handle.y),
  };
}

function handleLength(handle: MoodBoardVectorHandle | null | undefined) {
  return handle ? Math.hypot(handle.x, handle.y) : 0;
}

function vectorNodeMode(node: MoodBoardVectorNode): MoodBoardVectorHandleMode {
  if (node.handleMode) return node.handleMode;
  return node.in || node.out ? "independent" : "corner";
}

function ensureNodeHandles(node: MoodBoardVectorNode, fallback: MoodBoardVectorNode) {
  const next = {
    ...node,
    in: node.in ? { ...node.in } : fallback.in ? { ...fallback.in } : null,
    out: node.out ? { ...node.out } : fallback.out ? { ...fallback.out } : null,
  };

  if (!next.in && next.out) next.in = oppositeHandle(next.out);
  if (!next.out && next.in) next.out = oppositeHandle(next.in);
  return next;
}

export function setVectorNodeHandleMode(
  node: MoodBoardVectorNode,
  mode: MoodBoardVectorHandleMode,
  fallback: MoodBoardVectorNode = node,
): MoodBoardVectorNode {
  if (mode === "corner") return { x: node.x, y: node.y, in: null, out: null, handleMode: "corner" };
  const withHandles = ensureNodeHandles(node, fallback);
  if (mode === "mirrored" && withHandles.out) withHandles.in = oppositeHandle(withHandles.out);
  if (mode === "locked") {
    if (withHandles.out && withHandles.in) {
      const length = handleLength(withHandles.in) || handleLength(withHandles.out);
      const outLength = handleLength(withHandles.out) || 1;
      withHandles.in = {
        x: roundedPoint(-withHandles.out.x / outLength * length),
        y: roundedPoint(-withHandles.out.y / outLength * length),
      };
    }
  }
  return { ...withHandles, handleMode: mode };
}

export function cornerVectorNodes(nodes: MoodBoardVectorNode[]) {
  return nodes.map((node) => ({
    x: node.x,
    y: node.y,
    in: null,
    out: null,
    handleMode: "corner" as const,
  }));
}

export function smoothVectorNodes(nodes: MoodBoardVectorNode[], closed: boolean) {
  if (nodes.length < 2) return cornerVectorNodes(nodes);
  return nodes.map((node, index) => {
    const previous = index > 0 ? nodes[index - 1] : closed ? nodes.at(-1) ?? null : null;
    const next = index < nodes.length - 1 ? nodes[index + 1] : closed ? nodes[0] ?? null : null;

    if (previous && next) {
      const dx = (next.x - previous.x) / 6;
      const dy = (next.y - previous.y) / 6;
      return {
        x: node.x,
        y: node.y,
        in: { x: roundedPoint(-dx), y: roundedPoint(-dy) },
        out: { x: roundedPoint(dx), y: roundedPoint(dy) },
        handleMode: "mirrored" as const,
      };
    }

    if (next) {
      return {
        x: node.x,
        y: node.y,
        in: null,
        out: {
          x: roundedPoint((next.x - node.x) / 3),
          y: roundedPoint((next.y - node.y) / 3),
        },
        handleMode: "independent" as const,
      };
    }

    if (previous) {
      return {
        x: node.x,
        y: node.y,
        in: {
          x: roundedPoint((previous.x - node.x) / 3),
          y: roundedPoint((previous.y - node.y) / 3),
        },
        out: null,
        handleMode: "independent" as const,
      };
    }

    return { x: node.x, y: node.y, in: null, out: null, handleMode: "corner" as const };
  });
}

export function insertMidpointVectorNode(nodes: MoodBoardVectorNode[], closed: boolean) {
  if (nodes.length < 2) return nodes;
  let bestIndex = 0;
  let bestDistance = -1;
  const segmentCount = closed ? nodes.length : nodes.length - 1;

  for (let index = 0; index < segmentCount; index += 1) {
    const first = nodes[index];
    const second = nodes[(index + 1) % nodes.length];
    if (!first || !second) continue;
    const distance = distanceBetweenPoints(first, second);
    if (distance > bestDistance) {
      bestDistance = distance;
      bestIndex = index;
    }
  }

  const first = nodes[bestIndex];
  const second = nodes[(bestIndex + 1) % nodes.length];
  if (!first || !second) return nodes;
  const midpoint: MoodBoardVectorNode = {
    x: roundedPoint((first.x + second.x) / 2),
    y: roundedPoint((first.y + second.y) / 2),
    in: null,
    out: null,
    handleMode: "corner",
  };
  const next = [...nodes];
  next.splice(bestIndex + 1, 0, midpoint);
  return next;
}

function cubicPoint(
  first: MoodBoardVectorNode,
  second: MoodBoardVectorNode,
  t: number,
) {
  const c1 = vectorControlPoint(first, "out");
  const c2 = vectorControlPoint(second, "in");
  if (!first.out && !second.in) {
    return {
      x: first.x + (second.x - first.x) * t,
      y: first.y + (second.y - first.y) * t,
    };
  }
  const mt = 1 - t;
  return {
    x: mt ** 3 * first.x + 3 * mt ** 2 * t * c1.x + 3 * mt * t ** 2 * c2.x + t ** 3 * second.x,
    y: mt ** 3 * first.y + 3 * mt ** 2 * t * c1.y + 3 * mt * t ** 2 * c2.y + t ** 3 * second.y,
  };
}

export function insertVectorNodeAtPoint(nodes: MoodBoardVectorNode[], closed: boolean, point: MoodBoardPoint) {
  if (nodes.length < 2) return nodes;
  let bestSegmentIndex = 0;
  let bestPoint = { x: nodes[0]?.x ?? 0, y: nodes[0]?.y ?? 0 };
  let bestDistance = Number.POSITIVE_INFINITY;
  const segmentCount = closed ? nodes.length : nodes.length - 1;

  for (let index = 0; index < segmentCount; index += 1) {
    const first = nodes[index];
    const second = nodes[(index + 1) % nodes.length];
    if (!first || !second) continue;

    for (let step = 1; step < 24; step += 1) {
      const candidate = cubicPoint(first, second, step / 24);
      const distance = distanceBetweenPoints(point, candidate);
      if (distance < bestDistance) {
        bestDistance = distance;
        bestSegmentIndex = index;
        bestPoint = candidate;
      }
    }
  }

  const next = [...nodes];
  next.splice(bestSegmentIndex + 1, 0, {
    x: roundedPoint(bestPoint.x),
    y: roundedPoint(bestPoint.y),
    in: null,
    out: null,
    handleMode: "corner",
  });
  return next;
}

export function removeLastVectorNode(nodes: MoodBoardVectorNode[], closed: boolean) {
  const minNodes = closed ? 3 : 2;
  if (nodes.length <= minNodes) return nodes;
  return nodes.slice(0, -1);
}

export function removeVectorNodesByIndexes(nodes: MoodBoardVectorNode[], indexes: Set<number>, closed: boolean) {
  const minNodes = closed ? 3 : 2;
  if (!indexes.size || nodes.length - indexes.size < minNodes) return nodes;
  return nodes.filter((_, index) => !indexes.has(index));
}

export function cornerSelectedVectorNodes(nodes: MoodBoardVectorNode[], indexes: Set<number>) {
  if (!indexes.size) return nodes;
  return nodes.map((node, index) => indexes.has(index)
    ? { x: node.x, y: node.y, in: null, out: null, handleMode: "corner" as const }
    : node);
}

export function smoothSelectedVectorNodes(nodes: MoodBoardVectorNode[], indexes: Set<number>, closed: boolean) {
  if (!indexes.size) return nodes;
  const smoothed = smoothVectorNodes(nodes, closed);
  return nodes.map((node, index) => indexes.has(index) ? smoothed[index] ?? node : node);
}

export function setSelectedVectorNodesHandleMode(
  nodes: MoodBoardVectorNode[],
  indexes: Set<number>,
  closed: boolean,
  mode: MoodBoardVectorHandleMode,
) {
  if (!indexes.size) return nodes;
  const smoothed = smoothVectorNodes(nodes, closed);
  return nodes.map((node, index) => indexes.has(index)
    ? setVectorNodeHandleMode(node, mode, smoothed[index] ?? node)
    : node);
}

export function moveVectorNodeHandle(
  nodes: MoodBoardVectorNode[],
  nodeIndex: number,
  handle: "node" | "in" | "out",
  dx: number,
  dy: number,
) {
  const nextNodes = nodes.map((node) => ({
    ...node,
    in: node.in ? { ...node.in } : null,
    out: node.out ? { ...node.out } : null,
  }));
  const node = nextNodes[nodeIndex];
  if (!node) return nextNodes;

  if (handle === "node") {
    node.x = roundedPoint(node.x + dx);
    node.y = roundedPoint(node.y + dy);
    return nextNodes;
  }

  const current = node[handle] ?? { x: 0, y: 0 };
  const nextHandle = roundedHandle({ x: current.x + dx, y: current.y + dy });
  const opposite = handle === "in" ? "out" : "in";
  node[handle] = nextHandle;

  const mode = vectorNodeMode(node);
  if (mode === "mirrored") {
    node[opposite] = oppositeHandle(nextHandle);
  } else if (mode === "locked") {
    const existingLength = handleLength(node[opposite]) || handleLength(nextHandle);
    const nextLength = handleLength(nextHandle) || 1;
    node[opposite] = {
      x: roundedPoint(-nextHandle.x / nextLength * existingLength),
      y: roundedPoint(-nextHandle.y / nextLength * existingLength),
    };
  } else if (mode === "corner") {
    node.handleMode = "independent";
  }

  return nextNodes;
}

export function shapeToVectorPath(shape: Extract<MoodBoardItem, { type: "shape" }>): Extract<MoodBoardItem, { type: "vectorPath" }> {
  const width = Math.max(1, shape.width);
  const height = Math.max(1, shape.height);
  const kappa = 0.5522847498;
  const nodes: MoodBoardVectorNode[] = shape.shape === "ellipse"
    ? [
      { x: width / 2, y: 0, in: { x: -width * kappa / 2, y: 0 }, out: { x: width * kappa / 2, y: 0 } },
      { x: width, y: height / 2, in: { x: 0, y: -height * kappa / 2 }, out: { x: 0, y: height * kappa / 2 } },
      { x: width / 2, y: height, in: { x: width * kappa / 2, y: 0 }, out: { x: -width * kappa / 2, y: 0 } },
      { x: 0, y: height / 2, in: { x: 0, y: height * kappa / 2 }, out: { x: 0, y: -height * kappa / 2 } },
    ].map((node) => ({ ...node, handleMode: "mirrored" as const }))
    : shape.shape === "diamond"
      ? [
        { x: width / 2, y: 0, in: null, out: null, handleMode: "corner" as const },
        { x: width, y: height / 2, in: null, out: null, handleMode: "corner" as const },
        { x: width / 2, y: height, in: null, out: null, handleMode: "corner" as const },
        { x: 0, y: height / 2, in: null, out: null, handleMode: "corner" as const },
      ]
      : [
        { x: 0, y: 0, in: null, out: null, handleMode: "corner" as const },
        { x: width, y: 0, in: null, out: null, handleMode: "corner" as const },
        { x: width, y: height, in: null, out: null, handleMode: "corner" as const },
        { x: 0, y: height, in: null, out: null, handleMode: "corner" as const },
      ];

  return {
    id: createMoodBoardItemId(),
    type: "vectorPath",
    x: shape.x,
    y: shape.y,
    width,
    height,
    nodes,
    closed: true,
    fill: shape.fill,
    stroke: shape.stroke,
    strokeWidth: shape.strokeWidth,
    viewBoxWidth: width,
    viewBoxHeight: height,
  };
}
