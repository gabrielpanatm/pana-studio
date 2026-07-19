import {
  createMoodBoardItemId,
  type MoodBoardItem,
  type MoodBoardVectorGroupElement,
  type MoodBoardVectorGroupPathElement,
  type MoodBoardVectorNode,
  type MoodBoardVectorTransform,
} from "$lib/mood-board/model";
import { buildVectorSvgPath, pathPointBounds } from "$lib/mood-board/vector";

export function escapeSvgAttribute(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("\"", "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function escapeSvgText(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

export function vectorPathToSvg(vectorPath: Extract<MoodBoardItem, { type: "vectorPath" }>) {
  const d = escapeSvgAttribute(buildVectorSvgPath(vectorPath));
  const fill = escapeSvgAttribute(vectorPath.fill || "none");
  const stroke = escapeSvgAttribute(vectorPath.stroke || "none");
  const width = Math.max(1, Math.round(vectorPath.viewBoxWidth * 100) / 100);
  const height = Math.max(1, Math.round(vectorPath.viewBoxHeight * 100) / 100);
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}">`,
    `  <path d="${d}" fill="${fill}" stroke="${stroke}" stroke-width="${vectorPath.strokeWidth}" vector-effect="non-scaling-stroke"/>`,
    "</svg>",
    "",
  ].join("\n");
}

function svgTransformAttribute(transform: MoodBoardVectorGroupElement["transform"]) {
  if (
    transform[0] === 1
    && transform[1] === 0
    && transform[2] === 0
    && transform[3] === 1
    && transform[4] === 0
    && transform[5] === 0
  ) return "";
  return ` transform="matrix(${transform.join(" ")})"`;
}

export function vectorGroupToSvg(group: Extract<MoodBoardItem, { type: "vectorGroup" }>) {
  const width = Math.max(1, Math.round(group.viewBoxWidth * 100) / 100);
  const height = Math.max(1, Math.round(group.viewBoxHeight * 100) / 100);
  const children = group.elements.map((element) => {
    if (element.type === "text") {
      return `  <text x="${element.x}" y="${element.y}" fill="${escapeSvgAttribute(element.fill || "none")}" font-family="${escapeSvgAttribute(element.fontFamily)}" font-size="${element.fontSize}" font-weight="${element.fontWeight}" opacity="${element.opacity}"${svgTransformAttribute(element.transform)}>${escapeSvgText(element.text)}</text>`;
    }
    return `  <path d="${escapeSvgAttribute(element.d)}" fill="${escapeSvgAttribute(element.fill || "none")}" stroke="${escapeSvgAttribute(element.stroke || "none")}" stroke-width="${element.strokeWidth}" opacity="${element.opacity}"${svgTransformAttribute(element.transform)}/>`;
  });
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${group.viewBoxX} ${group.viewBoxY} ${width} ${height}" width="${width}" height="${height}">`,
    ...children,
    "</svg>",
    "",
  ].join("\n");
}

export function parseSvgLength(value: string | null, fallback: number) {
  if (!value) return fallback;
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function parseSvgNumber(value: string | null, fallback = 0) {
  if (!value) return fallback;
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function parseSvgStyle(style: string | null) {
  const values = new Map<string, string>();
  if (!style) return values;
  for (const entry of style.split(";")) {
    const [rawKey, ...rawValue] = entry.split(":");
    const key = rawKey?.trim().toLowerCase();
    const value = rawValue.join(":").trim();
    if (key && value) values.set(key, value);
  }
  return values;
}

type SvgStyleRules = Map<string, Map<string, string>>;

function parseSvgStyleRules(document: Document): SvgStyleRules {
  const rules: SvgStyleRules = new Map();
  const styleText = Array.from(document.querySelectorAll("style"))
    .map((style) => style.textContent ?? "")
    .join("\n");
  const ruleMatcher = /([^{}]+)\{([^{}]+)\}/g;
  for (const match of styleText.matchAll(ruleMatcher)) {
    const selectors = (match[1] ?? "").split(",").map((selector) => selector.trim()).filter(Boolean);
    const declarations = parseSvgStyle(match[2] ?? "");
    if (!declarations.size) continue;
    for (const selector of selectors) {
      if (!selector.startsWith(".") && !selector.startsWith("#") && !/^[a-zA-Z][\w-]*$/.test(selector)) continue;
      const existing = rules.get(selector) ?? new Map<string, string>();
      for (const [key, value] of declarations) existing.set(key, value);
      rules.set(selector, existing);
    }
  }
  return rules;
}

function styleRuleValue(element: Element, name: string, rules: SvgStyleRules) {
  const tagRule = rules.get(element.tagName.toLowerCase())?.get(name);
  const idRule = element.id ? rules.get(`#${element.id}`)?.get(name) : "";
  let classRule = "";
  for (const className of Array.from(element.classList)) {
    classRule = rules.get(`.${className}`)?.get(name) ?? classRule;
  }
  return idRule || classRule || tagRule || "";
}

function inheritedSvgAttribute(element: Element, name: string, fallback = "", rules: SvgStyleRules = new Map()) {
  let current: Element | null = element;
  while (current && current.tagName.toLowerCase() !== "svg") {
    const styleValue = parseSvgStyle(current.getAttribute("style")).get(name);
    const directValue = current.getAttribute(name);
    if (styleValue) return styleValue.trim();
    const ruleValue = styleRuleValue(current, name, rules);
    if (ruleValue) return ruleValue.trim();
    if (directValue) return directValue.trim();
    current = current.parentElement;
  }
  return fallback;
}

function svgAttribute(element: Element, name: string, rules: SvgStyleRules = new Map()) {
  const styleValue = parseSvgStyle(element.getAttribute("style")).get(name);
  return styleValue?.trim() || styleRuleValue(element, name, rules) || element.getAttribute(name)?.trim() || "";
}

function svgNumberAttribute(element: Element, name: string, fallback: number, rules: SvgStyleRules) {
  const value = Number.parseFloat(inheritedSvgAttribute(element, name, String(fallback), rules));
  return Number.isFinite(value) ? value : fallback;
}

function isSvgElementHidden(element: Element, rules: SvgStyleRules) {
  let current: Element | null = element;
  while (current && current.tagName.toLowerCase() !== "svg") {
    const display = svgAttribute(current, "display", rules);
    const visibility = svgAttribute(current, "visibility", rules);
    if (display === "none" || visibility === "hidden" || visibility === "collapse") return true;
    current = current.parentElement;
  }
  return false;
}

function inheritedSvgOpacity(element: Element, rules: SvgStyleRules) {
  let opacity = 1;
  let current: Element | null = element;
  while (current && current.tagName.toLowerCase() !== "svg") {
    const value = svgAttribute(current, "opacity", rules);
    const parsed = Number.parseFloat(value);
    if (Number.isFinite(parsed)) opacity *= Math.min(1, Math.max(0, parsed));
    current = current.parentElement;
  }
  return opacity;
}

function normalizedSvgPaint(value: string, fallback: string) {
  const paint = value.trim();
  if (!paint || paint === "currentColor") return fallback;
  if (paint.startsWith("url(")) return fallback;
  return paint;
}

function detectUnsupportedSvgFeatures(document: Document) {
  const unsupported = new Set<string>();
  const featureSelectors: Array<[string, string]> = [
    ["linearGradient, radialGradient", "gradient"],
    ["pattern", "pattern"],
    ["mask", "mask"],
    ["clipPath", "clipPath"],
    ["filter", "filter"],
    ["use, symbol", "use/symbol"],
    ["image", "embedded image"],
    ["foreignObject", "foreignObject"],
  ];
  for (const [selector, label] of featureSelectors) {
    if (document.querySelector(selector)) unsupported.add(label);
  }
  const paintServerElement = Array.from(document.querySelectorAll("*")).find((element) => (
    (element.getAttribute("fill") ?? "").includes("url(")
    || (element.getAttribute("stroke") ?? "").includes("url(")
    || (element.getAttribute("style") ?? "").includes("url(")
  ));
  if (paintServerElement) unsupported.add("paint server");
  return [...unsupported];
}

function svgNumber(element: Element, name: string, fallback = 0) {
  return parseSvgLength(element.getAttribute(name), fallback);
}

function svgCoordinate(element: Element, name: string, fallback = 0) {
  return parseSvgNumber(element.getAttribute(name), fallback);
}

type SvgMatrix = [number, number, number, number, number, number];

const identityMatrix: SvgMatrix = [1, 0, 0, 1, 0, 0];

function multiplyMatrix(left: SvgMatrix, right: SvgMatrix): SvgMatrix {
  const [a1, b1, c1, d1, e1, f1] = left;
  const [a2, b2, c2, d2, e2, f2] = right;
  return [
    a1 * a2 + c1 * b2,
    b1 * a2 + d1 * b2,
    a1 * c2 + c1 * d2,
    b1 * c2 + d1 * d2,
    a1 * e2 + c1 * f2 + e1,
    b1 * e2 + d1 * f2 + f1,
  ];
}

function applyMatrix(point: Point, matrix: SvgMatrix): Point {
  const [a, b, c, d, e, f] = matrix;
  return {
    x: a * point.x + c * point.y + e,
    y: b * point.x + d * point.y + f,
  };
}

function parseTransformNumbers(value: string) {
  return value
    .trim()
    .split(/[\s,]+/)
    .map(Number)
    .filter(Number.isFinite);
}

function transformFunctionMatrix(name: string, values: number[]): SvgMatrix {
  if (name === "matrix" && values.length >= 6) {
    return [values[0], values[1], values[2], values[3], values[4], values[5]];
  }

  if (name === "translate") {
    return [1, 0, 0, 1, values[0] ?? 0, values[1] ?? 0];
  }

  if (name === "scale") {
    const sx = values[0] ?? 1;
    const sy = values[1] ?? sx;
    return [sx, 0, 0, sy, 0, 0];
  }

  if (name === "rotate") {
    const angle = ((values[0] ?? 0) * Math.PI) / 180;
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    const rotation: SvgMatrix = [cos, sin, -sin, cos, 0, 0];
    if (values.length < 3) return rotation;
    const [cx, cy] = [values[1], values[2]];
    return multiplyMatrix(
      multiplyMatrix([1, 0, 0, 1, cx, cy], rotation),
      [1, 0, 0, 1, -cx, -cy],
    );
  }

  if (name === "skewX") {
    return [1, 0, Math.tan(((values[0] ?? 0) * Math.PI) / 180), 1, 0, 0];
  }

  if (name === "skewY") {
    return [1, Math.tan(((values[0] ?? 0) * Math.PI) / 180), 0, 1, 0, 0];
  }

  return identityMatrix;
}

function parseTransformAttribute(transform: string | null) {
  if (!transform) return identityMatrix;
  const matcher = /([a-zA-Z]+)\(([^)]*)\)/g;
  let matrix = identityMatrix;
  for (const match of transform.matchAll(matcher)) {
    const name = match[1];
    const values = parseTransformNumbers(match[2] ?? "");
    matrix = multiplyMatrix(matrix, transformFunctionMatrix(name, values));
  }
  return matrix;
}

function cumulativeSvgTransform(element: Element): SvgMatrix {
  const transforms: SvgMatrix[] = [];
  let current: Element | null = element;
  while (current && current.tagName.toLowerCase() !== "svg") {
    transforms.unshift(parseTransformAttribute(current.getAttribute("transform")));
    current = current.parentElement;
  }
  return transforms.reduce((matrix, transform) => multiplyMatrix(matrix, transform), identityMatrix);
}

function transformVectorNodes(nodes: MoodBoardVectorNode[], matrix: SvgMatrix) {
  return nodes.map((node) => {
    const point = applyMatrix(node, matrix);
    const inPoint = node.in ? applyMatrix({ x: node.x + node.in.x, y: node.y + node.in.y }, matrix) : null;
    const outPoint = node.out ? applyMatrix({ x: node.x + node.out.x, y: node.y + node.out.y }, matrix) : null;
    return {
      x: point.x,
      y: point.y,
      in: inPoint ? { x: inPoint.x - point.x, y: inPoint.y - point.y } : null,
      out: outPoint ? { x: outPoint.x - point.x, y: outPoint.y - point.y } : null,
      handleMode: node.handleMode ?? (node.in || node.out ? "independent" : "corner"),
    };
  });
}

export function vectorNodesToPathData(nodes: MoodBoardVectorNode[], closed: boolean) {
  return buildVectorSvgPath({
    id: "normalized",
    type: "vectorPath",
    x: 0,
    y: 0,
    width: 1,
    height: 1,
    nodes,
    closed,
    fill: "none",
    stroke: "none",
    strokeWidth: 0,
    viewBoxWidth: 1,
    viewBoxHeight: 1,
  });
}

export function editableSvgElementNodes(element: MoodBoardVectorGroupElement) {
  if (element.type !== "path") return null;
  return parseEditablePathData(element.d);
}

export function svgElementWithVectorNodes(
  element: MoodBoardVectorGroupPathElement,
  nodes: MoodBoardVectorNode[],
  closed: boolean,
) {
  return {
    ...element,
    d: vectorNodesToPathData(nodes, closed),
  };
}

function normalizeSvgPathData(d: string, transform: SvgMatrix) {
  const parsed = parseEditablePathData(d);
  if (!parsed) return d;
  const transformedNodes = transformVectorNodes(parsed.nodes, transform);
  return vectorNodesToPathData(transformedNodes, parsed.closed);
}

export function svgElementPathData(element: Element) {
  const tag = element.tagName.toLowerCase();
  if (tag === "path") return element.getAttribute("d")?.trim() ?? "";
  if (tag === "rect") {
    const x = svgNumber(element, "x");
    const y = svgNumber(element, "y");
    const width = svgNumber(element, "width");
    const height = svgNumber(element, "height");
    if (!width || !height) return "";
    return `M ${x} ${y} H ${x + width} V ${y + height} H ${x} Z`;
  }
  if (tag === "circle") {
    const cx = svgNumber(element, "cx");
    const cy = svgNumber(element, "cy");
    const r = svgNumber(element, "r");
    if (!r) return "";
    return `M ${cx - r} ${cy} A ${r} ${r} 0 1 0 ${cx + r} ${cy} A ${r} ${r} 0 1 0 ${cx - r} ${cy} Z`;
  }
  if (tag === "ellipse") {
    const cx = svgNumber(element, "cx");
    const cy = svgNumber(element, "cy");
    const rx = svgNumber(element, "rx");
    const ry = svgNumber(element, "ry");
    if (!rx || !ry) return "";
    return `M ${cx - rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx + rx} ${cy} A ${rx} ${ry} 0 1 0 ${cx - rx} ${cy} Z`;
  }
  if (tag === "line") {
    return `M ${svgNumber(element, "x1")} ${svgNumber(element, "y1")} L ${svgNumber(element, "x2")} ${svgNumber(element, "y2")}`;
  }
  if (tag === "polygon" || tag === "polyline") {
    const points = element.getAttribute("points")?.trim() ?? "";
    if (!points) return "";
    const values = points.split(/[\s,]+/).map(Number).filter(Number.isFinite);
    const pairs: string[] = [];
    for (let index = 0; index < values.length - 1; index += 2) pairs.push(`${values[index]} ${values[index + 1]}`);
    if (!pairs.length) return "";
    return `M ${pairs[0]} ${pairs.slice(1).map((pair) => `L ${pair}`).join(" ")}${tag === "polygon" ? " Z" : ""}`;
  }
  return "";
}

function svgGroupPath(element: Element) {
  const groups: string[] = [];
  let current = element.parentElement;
  while (current && current.tagName.toLowerCase() !== "svg") {
    if (current.tagName.toLowerCase() === "g") {
      const id = current.getAttribute("id")?.trim();
      const label = current.getAttribute("inkscape:label")?.trim()
        || current.getAttribute("aria-label")?.trim()
        || current.getAttribute("class")?.trim();
      groups.unshift(id || label || "g");
    }
    current = current.parentElement;
  }
  return groups;
}

function svgTextElementToMoodElements(
  element: Element,
  index: number,
  rules: SvgStyleRules,
): MoodBoardVectorGroupElement[] {
  const tspans = Array.from(element.children).filter((child) => child.tagName.toLowerCase() === "tspan");
  const groupPath = svgGroupPath(element);

  if (!tspans.length) {
    const text = element.textContent?.trim() ?? "";
    if (!text) return [];
    const opacity = inheritedSvgOpacity(element, rules);
    return [{
      id: element.getAttribute("id") || `svg_text_${index}`,
      type: "text",
      groupPath,
      text,
      x: svgCoordinate(element, "x"),
      y: svgCoordinate(element, "y"),
      fontSize: Math.min(512, Math.max(1, parseSvgLength(inheritedSvgAttribute(element, "font-size", "16", rules), 16))),
      fontFamily: inheritedSvgAttribute(element, "font-family", "sans-serif", rules).replace(/^["']|["']$/g, ""),
      fontWeight: Math.min(1000, Math.max(100, Number.parseInt(inheritedSvgAttribute(element, "font-weight", "400", rules), 10) || 400)),
      fill: normalizedSvgPaint(inheritedSvgAttribute(element, "fill", "#1d7f6a", rules), "#1d7f6a"),
      opacity: Math.min(1, Math.max(0, opacity * svgNumberAttribute(element, "fill-opacity", 1, rules))),
      transform: cumulativeSvgTransform(element),
    }];
  }

  let cursorX = svgCoordinate(element, "x");
  let cursorY = svgCoordinate(element, "y");
  return tspans.flatMap((tspan, tspanIndex): MoodBoardVectorGroupElement[] => {
    const text = tspan.textContent?.trim() ?? "";
    if (!text) return [];
    cursorX = tspan.hasAttribute("x")
      ? svgCoordinate(tspan, "x", cursorX)
      : cursorX + svgCoordinate(tspan, "dx", 0);
    cursorY = tspan.hasAttribute("y")
      ? svgCoordinate(tspan, "y", cursorY)
      : cursorY + svgCoordinate(tspan, "dy", 0);
    const opacity = inheritedSvgOpacity(tspan, rules);
    return [{
      id: tspan.getAttribute("id") || `${element.getAttribute("id") || `svg_text_${index}`}_tspan_${tspanIndex}`,
      type: "text",
      groupPath,
      text,
      x: cursorX,
      y: cursorY,
      fontSize: Math.min(512, Math.max(1, parseSvgLength(inheritedSvgAttribute(tspan, "font-size", inheritedSvgAttribute(element, "font-size", "16", rules), rules), 16))),
      fontFamily: inheritedSvgAttribute(tspan, "font-family", inheritedSvgAttribute(element, "font-family", "sans-serif", rules), rules).replace(/^["']|["']$/g, ""),
      fontWeight: Math.min(1000, Math.max(100, Number.parseInt(inheritedSvgAttribute(tspan, "font-weight", inheritedSvgAttribute(element, "font-weight", "400", rules), rules), 10) || 400)),
      fill: normalizedSvgPaint(inheritedSvgAttribute(tspan, "fill", inheritedSvgAttribute(element, "fill", "#1d7f6a", rules), rules), "#1d7f6a"),
      opacity: Math.min(1, Math.max(0, opacity * svgNumberAttribute(tspan, "fill-opacity", 1, rules))),
      transform: cumulativeSvgTransform(tspan),
    }];
  });
}

export function parseEditableSvg(sourcePath: string, source: string, point: { x: number; y: number }): MoodBoardItem | null {
  const document = new DOMParser().parseFromString(source, "image/svg+xml");
  const parserError = document.querySelector("parsererror");
  const svg = document.querySelector("svg");
  if (parserError || !svg) return null;
  const styleRules = parseSvgStyleRules(document);
  const unsupportedFeatures = detectUnsupportedSvgFeatures(document);

  const viewBox = svg.getAttribute("viewBox")?.trim().split(/[\s,]+/).map(Number) ?? [];
  const viewBoxX = Number.isFinite(viewBox[0]) ? viewBox[0] : 0;
  const viewBoxY = Number.isFinite(viewBox[1]) ? viewBox[1] : 0;
  const viewBoxWidth = Number.isFinite(viewBox[2]) && viewBox[2] > 0
    ? viewBox[2]
    : parseSvgLength(svg.getAttribute("width"), 320);
  const viewBoxHeight = Number.isFinite(viewBox[3]) && viewBox[3] > 0
    ? viewBox[3]
    : parseSvgLength(svg.getAttribute("height"), 240);
  const elements = Array.from(svg.querySelectorAll("path, rect, circle, ellipse, line, polygon, polyline, text"))
    .flatMap((element, index): MoodBoardVectorGroupElement[] => {
      if (isSvgElementHidden(element, styleRules)) return [];
      const tag = element.tagName.toLowerCase();
      if (tag === "text") {
        return svgTextElementToMoodElements(element, index, styleRules);
      }

      const rawPathData = svgElementPathData(element);
      if (!rawPathData) return [];
      const d = normalizeSvgPathData(rawPathData, cumulativeSvgTransform(element));
      const fill = normalizedSvgPaint(inheritedSvgAttribute(element, "fill", "#1d7f6a", styleRules), "#1d7f6a");
      const stroke = normalizedSvgPaint(inheritedSvgAttribute(element, "stroke", "transparent", styleRules), "transparent");
      const paintOpacity = Math.min(
        fill !== "none" && fill !== "transparent" ? svgNumberAttribute(element, "fill-opacity", 1, styleRules) : 1,
        stroke !== "none" && stroke !== "transparent" ? svgNumberAttribute(element, "stroke-opacity", 1, styleRules) : 1,
      );
      return [{
        id: element.getAttribute("id") || `svg_path_${index}`,
        type: "path" as const,
        sourceTag: tag,
        groupPath: svgGroupPath(element),
        d,
        fill,
        stroke,
        strokeWidth: Math.min(64, Math.max(0, parseSvgLength(inheritedSvgAttribute(element, "stroke-width", "1", styleRules), 1))),
        opacity: Math.min(1, Math.max(0, inheritedSvgOpacity(element, styleRules) * paintOpacity)),
        transform: [1, 0, 0, 1, 0, 0] satisfies MoodBoardVectorTransform,
      }];
    })
    .filter((element): element is MoodBoardVectorGroupElement => Boolean(element));

  if (!elements.length) return null;

  const maxWidth = 360;
  const ratio = viewBoxWidth / Math.max(1, viewBoxHeight);
  const width = ratio >= 1 ? maxWidth : Math.max(120, Math.round(maxWidth * ratio));
  const height = ratio >= 1 ? Math.max(120, Math.round(maxWidth / ratio)) : maxWidth;

  return {
    id: createMoodBoardItemId(),
    type: "vectorGroup",
    x: point.x - width / 2,
    y: point.y - height / 2,
    width,
    height,
    title: sourcePath.split("/").filter(Boolean).at(-1)?.replace(/\.svg$/i, "") ?? "SVG",
    sourcePath,
    unsupportedFeatures,
    viewBoxX,
    viewBoxY,
    viewBoxWidth,
    viewBoxHeight,
    elements,
  };
}

function isPathCommand(token: string | undefined) {
  return Boolean(token && /^[a-zA-Z]$/.test(token));
}

function tokenizeSvgPath(d: string) {
  return d.match(/[a-zA-Z]|[-+]?(?:\d*\.\d+|\d+\.?)(?:e[-+]?\d+)?/gi) ?? [];
}

type Point = { x: number; y: number };

function angleBetween(left: Point, right: Point) {
  return Math.atan2(left.x * right.y - left.y * right.x, left.x * right.x + left.y * right.y);
}

function svgArcToCubicSegments({
  start,
  end,
  rx,
  ry,
  rotation,
  largeArc,
  sweep,
}: {
  start: Point;
  end: Point;
  rx: number;
  ry: number;
  rotation: number;
  largeArc: boolean;
  sweep: boolean;
}) {
  const absRx = Math.abs(rx);
  const absRy = Math.abs(ry);
  if (!absRx || !absRy || (start.x === end.x && start.y === end.y)) return [];

  const phi = (rotation * Math.PI) / 180;
  const cosPhi = Math.cos(phi);
  const sinPhi = Math.sin(phi);
  const dx = (start.x - end.x) / 2;
  const dy = (start.y - end.y) / 2;
  const x1p = cosPhi * dx + sinPhi * dy;
  const y1p = -sinPhi * dx + cosPhi * dy;
  let correctedRx = absRx;
  let correctedRy = absRy;

  const radiusScale = (x1p ** 2) / (correctedRx ** 2) + (y1p ** 2) / (correctedRy ** 2);
  if (radiusScale > 1) {
    const scale = Math.sqrt(radiusScale);
    correctedRx *= scale;
    correctedRy *= scale;
  }

  const rx2 = correctedRx ** 2;
  const ry2 = correctedRy ** 2;
  const x1p2 = x1p ** 2;
  const y1p2 = y1p ** 2;
  const denominator = rx2 * y1p2 + ry2 * x1p2;
  if (!denominator) return [];

  const sign = largeArc === sweep ? -1 : 1;
  const centerScale = sign * Math.sqrt(Math.max(0, ((rx2 * ry2) - (rx2 * y1p2) - (ry2 * x1p2)) / denominator));
  const cxp = centerScale * ((correctedRx * y1p) / correctedRy);
  const cyp = centerScale * (-(correctedRy * x1p) / correctedRx);
  const cx = cosPhi * cxp - sinPhi * cyp + (start.x + end.x) / 2;
  const cy = sinPhi * cxp + cosPhi * cyp + (start.y + end.y) / 2;
  const v1 = { x: (x1p - cxp) / correctedRx, y: (y1p - cyp) / correctedRy };
  const v2 = { x: (-x1p - cxp) / correctedRx, y: (-y1p - cyp) / correctedRy };
  const theta1 = angleBetween({ x: 1, y: 0 }, v1);
  let deltaTheta = angleBetween(v1, v2);

  if (!sweep && deltaTheta > 0) deltaTheta -= Math.PI * 2;
  if (sweep && deltaTheta < 0) deltaTheta += Math.PI * 2;

  const segmentCount = Math.max(1, Math.ceil(Math.abs(deltaTheta) / (Math.PI / 2)));
  const segmentTheta = deltaTheta / segmentCount;
  const transformPoint = (point: Point) => ({
    x: cx + correctedRx * (cosPhi * point.x - sinPhi * point.y),
    y: cy + correctedRy * (sinPhi * point.x + cosPhi * point.y),
  });
  const segments: Array<{ c1: Point; c2: Point; end: Point }> = [];

  for (let segment = 0; segment < segmentCount; segment += 1) {
    const t1 = theta1 + segment * segmentTheta;
    const t2 = t1 + segmentTheta;
    const alpha = (4 / 3) * Math.tan((t2 - t1) / 4);
    const p1 = { x: Math.cos(t1), y: Math.sin(t1) };
    const p2 = { x: Math.cos(t2), y: Math.sin(t2) };
    segments.push({
      c1: transformPoint({ x: p1.x - p1.y * alpha, y: p1.y + p1.x * alpha }),
      c2: transformPoint({ x: p2.x + p2.y * alpha, y: p2.y - p2.x * alpha }),
      end: transformPoint(p2),
    });
  }

  if (segments.at(-1)) segments[segments.length - 1].end = end;
  return segments;
}

export function parseEditablePathData(d: string) {
  const tokens = tokenizeSvgPath(d);
  let index = 0;
  let command = "";
  let current = { x: 0, y: 0 };
  let start = { x: 0, y: 0 };
  let closed = false;
  const nodes: MoodBoardVectorNode[] = [];

  const readNumber = () => {
    const token = tokens[index];
    if (token === undefined || isPathCommand(token)) return null;
    index += 1;
    const value = Number(token);
    return Number.isFinite(value) ? value : null;
  };

  const readPoint = (relative: boolean) => {
    const x = readNumber();
    const y = readNumber();
    if (x === null || y === null) return null;
    return relative ? { x: current.x + x, y: current.y + y } : { x, y };
  };

  const pushLine = (point: { x: number; y: number }) => {
    nodes.push({ x: point.x, y: point.y, in: null, out: null, handleMode: "corner" });
    current = point;
  };

  while (index < tokens.length) {
    if (isPathCommand(tokens[index])) {
      command = tokens[index] ?? "";
      index += 1;
    }
    if (!command) return null;

    const upper = command.toUpperCase();
    const relative = command === command.toLowerCase();

    if (upper === "M") {
      const point = readPoint(relative);
      if (!point) return null;
      if (nodes.length > 0) return null;
      nodes.push({ x: point.x, y: point.y, in: null, out: null, handleMode: "corner" });
      current = point;
      start = point;
      command = relative ? "l" : "L";
    } else if (upper === "L") {
      const point = readPoint(relative);
      if (!point) return null;
      pushLine(point);
    } else if (upper === "H") {
      const x = readNumber();
      if (x === null) return null;
      pushLine({ x: relative ? current.x + x : x, y: current.y });
    } else if (upper === "V") {
      const y = readNumber();
      if (y === null) return null;
      pushLine({ x: current.x, y: relative ? current.y + y : y });
    } else if (upper === "C") {
      const c1 = readPoint(relative);
      const c2 = readPoint(relative);
      const point = readPoint(relative);
      const previous = nodes.at(-1);
      if (!previous || !c1 || !c2 || !point) return null;
      previous.out = { x: c1.x - previous.x, y: c1.y - previous.y };
      previous.handleMode ??= "independent";
      nodes.push({
        x: point.x,
        y: point.y,
        in: { x: c2.x - point.x, y: c2.y - point.y },
        out: null,
        handleMode: "independent",
      });
      current = point;
    } else if (upper === "S") {
      const previous = nodes.at(-1);
      const c2 = readPoint(relative);
      const point = readPoint(relative);
      if (!previous || !c2 || !point) return null;
      const c1 = previous.in
        ? { x: previous.x - previous.in.x, y: previous.y - previous.in.y }
        : { x: previous.x, y: previous.y };
      previous.out = { x: c1.x - previous.x, y: c1.y - previous.y };
      previous.handleMode = previous.in ? "mirrored" : "independent";
      nodes.push({
        x: point.x,
        y: point.y,
        in: { x: c2.x - point.x, y: c2.y - point.y },
        out: null,
        handleMode: "independent",
      });
      current = point;
    } else if (upper === "Q") {
      const control = readPoint(relative);
      const point = readPoint(relative);
      const previous = nodes.at(-1);
      if (!previous || !control || !point) return null;
      const c1 = {
        x: previous.x + (2 / 3) * (control.x - previous.x),
        y: previous.y + (2 / 3) * (control.y - previous.y),
      };
      const c2 = {
        x: point.x + (2 / 3) * (control.x - point.x),
        y: point.y + (2 / 3) * (control.y - point.y),
      };
      previous.out = { x: c1.x - previous.x, y: c1.y - previous.y };
      previous.handleMode ??= "independent";
      nodes.push({
        x: point.x,
        y: point.y,
        in: { x: c2.x - point.x, y: c2.y - point.y },
        out: null,
        handleMode: "independent",
      });
      current = point;
    } else if (upper === "A") {
      const rx = readNumber();
      const ry = readNumber();
      const rotation = readNumber();
      const largeArcFlag = readNumber();
      const sweepFlag = readNumber();
      const point = readPoint(relative);
      const previous = nodes.at(-1);
      if (
        !previous
        || rx === null
        || ry === null
        || rotation === null
        || largeArcFlag === null
        || sweepFlag === null
        || !point
      ) return null;

      const segments = svgArcToCubicSegments({
        start: current,
        end: point,
        rx,
        ry,
        rotation,
        largeArc: Boolean(largeArcFlag),
        sweep: Boolean(sweepFlag),
      });

      if (!segments.length) {
        pushLine(point);
      } else {
        for (const segment of segments) {
          const lastNode = nodes.at(-1);
          if (!lastNode) return null;
          lastNode.out = { x: segment.c1.x - lastNode.x, y: segment.c1.y - lastNode.y };
          lastNode.handleMode ??= "independent";
          nodes.push({
            x: segment.end.x,
            y: segment.end.y,
            in: { x: segment.c2.x - segment.end.x, y: segment.c2.y - segment.end.y },
            out: null,
            handleMode: "independent",
          });
        }
        current = point;
      }
    } else if (upper === "Z") {
      closed = true;
      current = start;
      command = "";
    } else {
      return null;
    }
  }

  return nodes.length >= 2 ? { nodes, closed } : null;
}

export function canExtractSvgSubPath(element: MoodBoardVectorGroupElement) {
  if (element.type !== "path") return false;
  return Boolean(parseEditablePathData(element.d));
}

export function svgSubPathToVectorPath(
  group: Extract<MoodBoardItem, { type: "vectorGroup" }>,
  element: MoodBoardVectorGroupElement,
): Extract<MoodBoardItem, { type: "vectorPath" }> | null {
  if (element.type !== "path") return null;
  const parsed = parseEditablePathData(element.d);
  if (!parsed) return null;

  const bounds = pathPointBounds(parsed.nodes);
  const scaleX = group.width / Math.max(1, group.viewBoxWidth);
  const scaleY = group.height / Math.max(1, group.viewBoxHeight);
  const width = Math.max(24, (bounds.maxX - bounds.minX) * scaleX);
  const height = Math.max(24, (bounds.maxY - bounds.minY) * scaleY);

  return {
    id: createMoodBoardItemId(),
    type: "vectorPath",
    x: group.x + (bounds.minX - group.viewBoxX) * scaleX,
    y: group.y + (bounds.minY - group.viewBoxY) * scaleY,
    width,
    height,
    nodes: parsed.nodes.map((node) => ({
      x: Math.round((node.x - bounds.minX) * scaleX * 10) / 10,
      y: Math.round((node.y - bounds.minY) * scaleY * 10) / 10,
      in: node.in ? {
        x: Math.round(node.in.x * scaleX * 10) / 10,
        y: Math.round(node.in.y * scaleY * 10) / 10,
      } : null,
      out: node.out ? {
        x: Math.round(node.out.x * scaleX * 10) / 10,
        y: Math.round(node.out.y * scaleY * 10) / 10,
      } : null,
      handleMode: node.handleMode ?? (node.in || node.out ? "independent" : "corner"),
    })),
    closed: parsed.closed,
    fill: element.fill,
    stroke: element.stroke,
    strokeWidth: element.strokeWidth,
    viewBoxWidth: width,
    viewBoxHeight: height,
  };
}
