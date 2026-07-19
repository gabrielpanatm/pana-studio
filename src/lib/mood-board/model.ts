export type MoodBoardItemType = "note" | "text" | "color" | "reference" | "image" | "frame" | "group" | "shape" | "vectorPath" | "vectorGroup";
export type MoodBoardTool = "select" | "pan" | "note" | "text" | "color" | "reference" | "image" | "frame" | "shape" | "pen";
export type MoodBoardShapeKind = "rectangle" | "ellipse" | "diamond";
export type MoodBoardResizeHandle = "nw" | "ne" | "sw" | "se";
export type MoodBoardSaveState = "idle" | "saving" | "saved" | "error";

export type MoodBoardVectorHandle = {
  x: number;
  y: number;
};

export type MoodBoardVectorHandleMode = "corner" | "independent" | "mirrored" | "locked";

export type MoodBoardVectorNode = {
  x: number;
  y: number;
  in?: MoodBoardVectorHandle | null;
  out?: MoodBoardVectorHandle | null;
  handleMode?: MoodBoardVectorHandleMode;
};

export type MoodBoardVectorNodeEditState = {
  itemId: string;
  svgElementId: string | null;
  indexes: number[];
  modes: MoodBoardVectorHandleMode[];
  nodeCount: number;
};

export type MoodBoardImageMask = {
  nodes: MoodBoardVectorNode[];
  closed: boolean;
  viewBoxWidth: number;
  viewBoxHeight: number;
};

export type MoodBoardImageAdjustments = {
  opacity: number;
  brightness: number;
  contrast: number;
  saturation: number;
  grayscale: number;
  blur: number;
};

export type MoodBoardImageFraming = {
  positionX: number;
  positionY: number;
  scale: number;
};

export type MoodBoardVectorTransform = [number, number, number, number, number, number];

export type MoodBoardVectorGroupPathElement = {
  id: string;
  type: "path";
  sourceTag?: string;
  groupPath?: string[];
  d: string;
  fill: string;
  stroke: string;
  strokeWidth: number;
  opacity: number;
  transform: MoodBoardVectorTransform;
};

export type MoodBoardVectorGroupTextElement = {
  id: string;
  type: "text";
  groupPath?: string[];
  text: string;
  x: number;
  y: number;
  fontSize: number;
  fontFamily: string;
  fontWeight: number;
  fill: string;
  opacity: number;
  transform: MoodBoardVectorTransform;
};

export type MoodBoardVectorGroupElement = MoodBoardVectorGroupPathElement | MoodBoardVectorGroupTextElement;

export type MoodBoardViewport = {
  x: number;
  y: number;
  zoom: number;
};

type MoodBoardItemBase = {
  id: string;
  type: MoodBoardItemType;
  x: number;
  y: number;
  width: number;
  height: number;
};

export type MoodBoardNoteItem = MoodBoardItemBase & {
  type: "note";
  text: string;
};

export type MoodBoardTextItem = MoodBoardItemBase & {
  type: "text";
  text: string;
  color: string;
  fontSize: number;
  fontWeight: number;
  textAlign: "left" | "center" | "right";
};

export type MoodBoardColorItem = MoodBoardItemBase & {
  type: "color";
  color: string;
  label: string;
};

export type MoodBoardReferenceItem = MoodBoardItemBase & {
  type: "reference";
  title: string;
  url: string;
  note: string;
};

export type MoodBoardImageItem = MoodBoardItemBase & {
  type: "image";
  title: string;
  path: string;
  fit: "cover" | "contain";
  radius: number;
  shadow: boolean;
  adjustments: MoodBoardImageAdjustments;
  framing: MoodBoardImageFraming;
  mask?: MoodBoardImageMask | null;
};

export type MoodBoardFrameItem = MoodBoardItemBase & {
  type: "frame";
  title: string;
  tone: string;
  preset: "custom" | "desktop" | "tablet" | "mobile" | "hero" | "section";
  background: string;
  children: MoodBoardItem[];
};

export type MoodBoardGroupItem = MoodBoardItemBase & {
  type: "group";
  title: string;
  children: MoodBoardItem[];
};

export type MoodBoardShapeItem = MoodBoardItemBase & {
  type: "shape";
  shape: MoodBoardShapeKind;
  fill: string;
  stroke: string;
  strokeWidth: number;
};

export type MoodBoardVectorPathItem = MoodBoardItemBase & {
  type: "vectorPath";
  nodes: MoodBoardVectorNode[];
  closed: boolean;
  fill: string;
  stroke: string;
  strokeWidth: number;
  viewBoxWidth: number;
  viewBoxHeight: number;
};

export type MoodBoardVectorGroupItem = MoodBoardItemBase & {
  type: "vectorGroup";
  title: string;
  sourcePath?: string;
  unsupportedFeatures?: string[];
  viewBoxX: number;
  viewBoxY: number;
  viewBoxWidth: number;
  viewBoxHeight: number;
  elements: MoodBoardVectorGroupElement[];
};

export type MoodBoardItem =
  | MoodBoardNoteItem
  | MoodBoardTextItem
  | MoodBoardColorItem
  | MoodBoardReferenceItem
  | MoodBoardImageItem
  | MoodBoardFrameItem
  | MoodBoardGroupItem
  | MoodBoardShapeItem
  | MoodBoardVectorPathItem
  | MoodBoardVectorGroupItem;

export type MoodBoard = {
  version: 2;
  updatedAt: number;
  viewport: MoodBoardViewport;
  items: MoodBoardItem[];
};

export function createEmptyMoodBoard(): MoodBoard {
  return {
    version: 2,
    updatedAt: Date.now(),
    viewport: { x: 0, y: 0, zoom: 1 },
    items: [],
  };
}

export function createMoodBoardItemId() {
  return `mood_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

const zolaRootDirs = new Set(["content", "templates", "sass", "static", "themes"]);
const zolaRootFiles = new Set(["zola.toml", "config.toml"]);

function normalizeProjectRelativePath(path: string) {
  const normalized = path.trim().replaceAll("\\", "/").replace(/^\/+/, "");
  if (!normalized || normalized.startsWith("sursa/")) return normalized;
  const firstSegment = normalized.split("/")[0] ?? "";
  return zolaRootDirs.has(firstSegment) || zolaRootFiles.has(normalized)
    ? `sursa/${normalized}`
    : normalized;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function sanitizeVectorHandle(value: Partial<MoodBoardVectorHandle> | null | undefined): MoodBoardVectorHandle | null {
  if (!value || typeof value !== "object") return null;
  return {
    x: isFiniteNumber(value.x) ? value.x : 0,
    y: isFiniteNumber(value.y) ? value.y : 0,
  };
}

function sanitizeVectorHandleMode(
  value: unknown,
  inHandle: MoodBoardVectorHandle | null,
  outHandle: MoodBoardVectorHandle | null,
): MoodBoardVectorHandleMode {
  if (
    value === "corner"
    || value === "independent"
    || value === "mirrored"
    || value === "locked"
  ) {
    return value;
  }

  if (!inHandle && !outHandle) return "corner";
  if (inHandle && outHandle) {
    const mirrored = Math.abs(inHandle.x + outHandle.x) < 0.2
      && Math.abs(inHandle.y + outHandle.y) < 0.2;
    return mirrored ? "mirrored" : "independent";
  }

  return "independent";
}

function sanitizeVectorNode(value: Partial<MoodBoardVectorNode>): MoodBoardVectorNode | null {
  if (!value || typeof value !== "object") return null;
  if (!isFiniteNumber(value.x) || !isFiniteNumber(value.y)) return null;
  const inHandle = sanitizeVectorHandle(value.in);
  const outHandle = sanitizeVectorHandle(value.out);
  return {
    x: value.x,
    y: value.y,
    in: inHandle,
    out: outHandle,
    handleMode: sanitizeVectorHandleMode(value.handleMode, inHandle, outHandle),
  };
}

function defaultVectorPathNodes(): MoodBoardVectorNode[] {
  return [
    { x: 24, y: 96, out: { x: 36, y: -88 }, handleMode: "independent" },
    { x: 118, y: 34, in: { x: -42, y: -4 }, out: { x: 54, y: 4 }, handleMode: "independent" },
    { x: 216, y: 92, in: { x: -32, y: -88 }, out: { x: 30, y: 88 }, handleMode: "independent" },
    { x: 116, y: 142, in: { x: 58, y: 10 }, out: { x: -58, y: -10 }, handleMode: "mirrored" },
  ];
}

function sanitizeVectorTransform(value: unknown): MoodBoardVectorTransform {
  if (!Array.isArray(value) || value.length < 6) return [1, 0, 0, 1, 0, 0];
  const values = value.slice(0, 6).map((entry) => Number(entry));
  if (values.some((entry) => !Number.isFinite(entry))) return [1, 0, 0, 1, 0, 0];
  return values as MoodBoardVectorTransform;
}

function sanitizeVectorGroupElement(value: Partial<MoodBoardVectorGroupElement>, index: number): MoodBoardVectorGroupElement | null {
  if (!value || typeof value !== "object") return null;
  if (value.type === "text") {
    return {
      id: typeof value.id === "string" && value.id ? value.id : `svg_text_${index}`,
      type: "text",
      text: typeof value.text === "string" ? value.text : "",
      groupPath: Array.isArray(value.groupPath)
        ? value.groupPath.filter((entry): entry is string => typeof entry === "string" && Boolean(entry.trim()))
        : [],
      x: isFiniteNumber(value.x) ? value.x : 0,
      y: isFiniteNumber(value.y) ? value.y : 0,
      fontSize: isFiniteNumber(value.fontSize) ? Math.min(512, Math.max(1, value.fontSize)) : 16,
      fontFamily: typeof value.fontFamily === "string" && value.fontFamily ? value.fontFamily : "sans-serif",
      fontWeight: isFiniteNumber(value.fontWeight) ? Math.min(1000, Math.max(100, value.fontWeight)) : 400,
      fill: typeof value.fill === "string" && value.fill ? value.fill : "#1d7f6a",
      opacity: isFiniteNumber(value.opacity) ? Math.min(1, Math.max(0, value.opacity)) : 1,
      transform: sanitizeVectorTransform(value.transform),
    };
  }
  if (value.type !== "path" || typeof value.d !== "string" || !value.d.trim()) return null;
  return {
    id: typeof value.id === "string" && value.id ? value.id : `svg_path_${index}`,
    type: "path",
    sourceTag: typeof value.sourceTag === "string" && value.sourceTag ? value.sourceTag : undefined,
    groupPath: Array.isArray(value.groupPath)
      ? value.groupPath.filter((entry): entry is string => typeof entry === "string" && Boolean(entry.trim()))
      : [],
    d: value.d,
    fill: typeof value.fill === "string" && value.fill ? value.fill : "transparent",
    stroke: typeof value.stroke === "string" && value.stroke ? value.stroke : "#1d7f6a",
    strokeWidth: isFiniteNumber(value.strokeWidth) ? Math.min(64, Math.max(0, value.strokeWidth)) : 1,
    opacity: isFiniteNumber(value.opacity) ? Math.min(1, Math.max(0, value.opacity)) : 1,
    transform: sanitizeVectorTransform(value.transform),
  };
}

function sanitizeViewport(value: Partial<MoodBoardViewport> | undefined): MoodBoardViewport {
  return {
    x: isFiniteNumber(value?.x) ? value.x : 0,
    y: isFiniteNumber(value?.y) ? value.y : 0,
    zoom: isFiniteNumber(value?.zoom) ? Math.min(3, Math.max(0.2, value.zoom)) : 1,
  };
}

function sanitizeImageAdjustments(value: Partial<MoodBoardImageAdjustments> | undefined): MoodBoardImageAdjustments {
  return {
    opacity: isFiniteNumber(value?.opacity) ? Math.min(1, Math.max(0, value.opacity)) : 1,
    brightness: isFiniteNumber(value?.brightness) ? Math.min(200, Math.max(0, value.brightness)) : 100,
    contrast: isFiniteNumber(value?.contrast) ? Math.min(200, Math.max(0, value.contrast)) : 100,
    saturation: isFiniteNumber(value?.saturation) ? Math.min(250, Math.max(0, value.saturation)) : 100,
    grayscale: isFiniteNumber(value?.grayscale) ? Math.min(100, Math.max(0, value.grayscale)) : 0,
    blur: isFiniteNumber(value?.blur) ? Math.min(24, Math.max(0, value.blur)) : 0,
  };
}

function sanitizeImageFraming(value: Partial<MoodBoardImageFraming> | undefined): MoodBoardImageFraming {
  return {
    positionX: isFiniteNumber(value?.positionX) ? Math.min(100, Math.max(0, value.positionX)) : 50,
    positionY: isFiniteNumber(value?.positionY) ? Math.min(100, Math.max(0, value.positionY)) : 50,
    scale: isFiniteNumber(value?.scale) ? Math.min(300, Math.max(100, value.scale)) : 100,
  };
}

function sanitizeItem(value: Partial<MoodBoardItem>): MoodBoardItem | null {
  if (!value || typeof value !== "object") return null;
  const base = {
    id: typeof value.id === "string" && value.id ? value.id : createMoodBoardItemId(),
    x: isFiniteNumber(value.x) ? value.x : 0,
    y: isFiniteNumber(value.y) ? value.y : 0,
    width: isFiniteNumber(value.width) ? Math.max(80, value.width) : 220,
    height: isFiniteNumber(value.height) ? Math.max(80, value.height) : 140,
  };

  if (value.type === "color") {
    return {
      ...base,
      type: "color",
      color: typeof value.color === "string" && value.color ? value.color : "#1d7f6a",
      label: typeof value.label === "string" ? value.label : "Culoare",
    };
  }

  if (value.type === "reference") {
    return {
      ...base,
      type: "reference",
      title: typeof value.title === "string" ? value.title : "Referință",
      url: typeof value.url === "string" ? value.url : "",
      note: typeof value.note === "string" ? value.note : "",
    };
  }

  if (value.type === "image") {
    return {
      ...base,
      type: "image",
      title: typeof value.title === "string" ? value.title : "Imagine",
      path: typeof value.path === "string" ? normalizeProjectRelativePath(value.path) : "",
      fit: value.fit === "contain" ? "contain" : "cover",
      radius: isFiniteNumber(value.radius) ? Math.min(48, Math.max(0, value.radius)) : 8,
      shadow: typeof value.shadow === "boolean" ? value.shadow : true,
      adjustments: sanitizeImageAdjustments(value.adjustments),
      framing: sanitizeImageFraming(value.framing),
      mask: value.mask && typeof value.mask === "object"
        ? {
          nodes: Array.isArray(value.mask.nodes)
            ? value.mask.nodes
              .map((node) => sanitizeVectorNode(node as Partial<MoodBoardVectorNode>))
              .filter((node): node is MoodBoardVectorNode => Boolean(node))
            : [],
          closed: typeof value.mask.closed === "boolean" ? value.mask.closed : true,
          viewBoxWidth: isFiniteNumber(value.mask.viewBoxWidth) ? Math.max(1, value.mask.viewBoxWidth) : base.width,
          viewBoxHeight: isFiniteNumber(value.mask.viewBoxHeight) ? Math.max(1, value.mask.viewBoxHeight) : base.height,
        }
        : null,
    };
  }

  if (value.type === "frame") {
    return {
      ...base,
      type: "frame",
      width: isFiniteNumber(value.width) ? Math.max(260, value.width) : 520,
      height: isFiniteNumber(value.height) ? Math.max(180, value.height) : 320,
      title: typeof value.title === "string" ? value.title : "Frame",
      tone: typeof value.tone === "string" && value.tone ? value.tone : "#1d7f6a",
      preset: value.preset === "desktop" || value.preset === "tablet" || value.preset === "mobile" || value.preset === "hero" || value.preset === "section"
        ? value.preset
        : "custom",
      background: typeof value.background === "string" && value.background ? value.background : "#ffffff",
      children: Array.isArray(value.children)
        ? value.children
          .map((child) => sanitizeItem(child as Partial<MoodBoardItem>))
          .filter((child): child is MoodBoardItem => Boolean(child))
        : [],
    };
  }

  if (value.type === "group") {
    return {
      ...base,
      type: "group",
      width: isFiniteNumber(value.width) ? Math.max(40, value.width) : 240,
      height: isFiniteNumber(value.height) ? Math.max(40, value.height) : 160,
      title: typeof value.title === "string" && value.title ? value.title : "Grup",
      children: Array.isArray(value.children)
        ? value.children
          .map((child) => sanitizeItem(child as Partial<MoodBoardItem>))
          .filter((child): child is MoodBoardItem => Boolean(child))
        : [],
    };
  }

  if (value.type === "shape") {
    const shape = value.shape === "ellipse" || value.shape === "diamond" || value.shape === "rectangle"
      ? value.shape
      : "rectangle";
    return {
      ...base,
      type: "shape",
      width: isFiniteNumber(value.width) ? Math.max(60, value.width) : 160,
      height: isFiniteNumber(value.height) ? Math.max(60, value.height) : 120,
      shape,
      fill: typeof value.fill === "string" && value.fill ? value.fill : "#ffffff",
      stroke: typeof value.stroke === "string" && value.stroke ? value.stroke : "#1d7f6a",
      strokeWidth: isFiniteNumber(value.strokeWidth) ? Math.min(24, Math.max(0, value.strokeWidth)) : 2,
    };
  }

  if (value.type === "vectorPath") {
    const nodes = Array.isArray(value.nodes)
      ? value.nodes
        .map((node) => sanitizeVectorNode(node as Partial<MoodBoardVectorNode>))
        .filter((node): node is MoodBoardVectorNode => Boolean(node))
      : [];
    return {
      ...base,
      type: "vectorPath",
      width: isFiniteNumber(value.width) ? Math.max(80, value.width) : 240,
      height: isFiniteNumber(value.height) ? Math.max(80, value.height) : 160,
      nodes: nodes.length >= 2 ? nodes : defaultVectorPathNodes(),
      closed: typeof value.closed === "boolean" ? value.closed : true,
      fill: typeof value.fill === "string" && value.fill ? value.fill : "#d8eee8",
      stroke: typeof value.stroke === "string" && value.stroke ? value.stroke : "#1d7f6a",
      strokeWidth: isFiniteNumber(value.strokeWidth) ? Math.min(32, Math.max(0, value.strokeWidth)) : 3,
      viewBoxWidth: isFiniteNumber(value.viewBoxWidth) ? Math.max(1, value.viewBoxWidth) : 240,
      viewBoxHeight: isFiniteNumber(value.viewBoxHeight) ? Math.max(1, value.viewBoxHeight) : 160,
    };
  }

  const rawVectorGroup = value as Partial<MoodBoardVectorGroupItem> & {
    type?: unknown;
    elements?: unknown;
  };
  if (rawVectorGroup.type === "vectorGroup" || rawVectorGroup.type === "svgGroup") {
    const elements = Array.isArray(rawVectorGroup.elements)
      ? rawVectorGroup.elements
        .map((element, index) => sanitizeVectorGroupElement(element as Partial<MoodBoardVectorGroupElement>, index))
        .filter((element): element is MoodBoardVectorGroupElement => Boolean(element))
      : [];
    return {
      ...base,
      type: "vectorGroup",
      width: isFiniteNumber(value.width) ? Math.max(80, value.width) : 320,
      height: isFiniteNumber(value.height) ? Math.max(80, value.height) : 240,
      title: typeof rawVectorGroup.title === "string" && rawVectorGroup.title ? rawVectorGroup.title : "SVG",
      sourcePath: typeof rawVectorGroup.sourcePath === "string" && rawVectorGroup.sourcePath ? normalizeProjectRelativePath(rawVectorGroup.sourcePath) : undefined,
      unsupportedFeatures: Array.isArray(rawVectorGroup.unsupportedFeatures)
        ? rawVectorGroup.unsupportedFeatures
          .filter((feature): feature is string => typeof feature === "string" && Boolean(feature.trim()))
          .map((feature) => feature.trim())
        : [],
      viewBoxX: isFiniteNumber(rawVectorGroup.viewBoxX) ? rawVectorGroup.viewBoxX : 0,
      viewBoxY: isFiniteNumber(rawVectorGroup.viewBoxY) ? rawVectorGroup.viewBoxY : 0,
      viewBoxWidth: isFiniteNumber(rawVectorGroup.viewBoxWidth) ? Math.max(1, rawVectorGroup.viewBoxWidth) : 320,
      viewBoxHeight: isFiniteNumber(rawVectorGroup.viewBoxHeight) ? Math.max(1, rawVectorGroup.viewBoxHeight) : 240,
      elements: elements.length ? elements : [{
        id: "svg_path_0",
        type: "path",
        d: "M 20 20 H 300 V 220 H 20 Z",
        fill: "#d8eee8",
        stroke: "#1d7f6a",
        strokeWidth: 1,
        opacity: 1,
        transform: [1, 0, 0, 1, 0, 0],
      }],
    };
  }

  if (value.type === "text") {
    return {
      ...base,
      type: "text",
      width: isFiniteNumber(value.width) ? Math.max(80, value.width) : 280,
      height: isFiniteNumber(value.height) ? Math.max(44, value.height) : 96,
      text: typeof value.text === "string" ? value.text : "Text",
      color: typeof value.color === "string" && value.color ? value.color : "#1a2825",
      fontSize: isFiniteNumber(value.fontSize) ? Math.min(120, Math.max(10, value.fontSize)) : 32,
      fontWeight: isFiniteNumber(value.fontWeight) ? Math.min(900, Math.max(300, value.fontWeight)) : 800,
      textAlign: value.textAlign === "center" || value.textAlign === "right" ? value.textAlign : "left",
    };
  }

  if (value.type === "note") {
    return {
      ...base,
      type: "note",
      text: typeof value.text === "string" ? value.text : "",
    };
  }

  return null;
}

const MAX_PERSISTED_MOOD_BOARD_ITEMS = 5_000;
const MAX_PERSISTED_MOOD_BOARD_NESTING = 32;

type PersistedMoodBoardValidation = {
  itemCount: number;
  itemIds: Set<string>;
};

function persistedObject(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Document Mood Board invalid: ${path} trebuie să fie obiect.`);
  }
  return value as Record<string, unknown>;
}

function persistedString(
  source: Record<string, unknown>,
  field: string,
  path: string,
  nonEmpty = false,
) {
  const value = source[field];
  if (typeof value !== "string" || (nonEmpty && !value.trim())) {
    throw new Error(`Document Mood Board invalid: ${path}.${field} trebuie să fie text${nonEmpty ? " nevid" : ""}.`);
  }
  return value;
}

function persistedNumber(source: Record<string, unknown>, field: string, path: string) {
  const value = source[field];
  if (!isFiniteNumber(value)) {
    throw new Error(`Document Mood Board invalid: ${path}.${field} trebuie să fie număr finit.`);
  }
  return value;
}

function persistedBoolean(source: Record<string, unknown>, field: string, path: string) {
  const value = source[field];
  if (typeof value !== "boolean") {
    throw new Error(`Document Mood Board invalid: ${path}.${field} trebuie să fie boolean.`);
  }
  return value;
}

function persistedEnum(
  source: Record<string, unknown>,
  field: string,
  path: string,
  values: readonly string[],
) {
  const value = persistedString(source, field, path);
  if (!values.includes(value)) {
    throw new Error(`Document Mood Board invalid: ${path}.${field} are valoarea necunoscută ${JSON.stringify(value)}.`);
  }
  return value;
}

function validatePersistedVectorHandle(value: unknown, path: string) {
  if (value === undefined || value === null) return;
  const handle = persistedObject(value, path);
  persistedNumber(handle, "x", path);
  persistedNumber(handle, "y", path);
}

function validatePersistedVectorNode(value: unknown, path: string) {
  const node = persistedObject(value, path);
  persistedNumber(node, "x", path);
  persistedNumber(node, "y", path);
  validatePersistedVectorHandle(node.in, `${path}.in`);
  validatePersistedVectorHandle(node.out, `${path}.out`);
  if (node.handleMode !== undefined) {
    persistedEnum(node, "handleMode", path, ["corner", "independent", "mirrored", "locked"]);
  }
}

function validatePersistedVectorTransform(value: unknown, path: string) {
  if (!Array.isArray(value) || value.length !== 6 || value.some((entry) => !isFiniteNumber(entry))) {
    throw new Error(`Document Mood Board invalid: ${path} trebuie să conțină exact șase numere finite.`);
  }
}

function validateOptionalPersistedStringArray(value: unknown, path: string) {
  if (value === undefined) return;
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== "string" || !entry.trim())) {
    throw new Error(`Document Mood Board invalid: ${path} trebuie să fie o listă de texte nevide.`);
  }
}

function validatePersistedVectorGroupElement(value: unknown, path: string) {
  const element = persistedObject(value, path);
  persistedString(element, "id", path, true);
  validateOptionalPersistedStringArray(element.groupPath, `${path}.groupPath`);
  persistedNumber(element, "opacity", path);
  validatePersistedVectorTransform(element.transform, `${path}.transform`);
  if (element.type === "path") {
    persistedString(element, "d", path, true);
    persistedString(element, "fill", path, true);
    persistedString(element, "stroke", path, true);
    persistedNumber(element, "strokeWidth", path);
    if (element.sourceTag !== undefined && typeof element.sourceTag !== "string") {
      throw new Error(`Document Mood Board invalid: ${path}.sourceTag trebuie să fie text.`);
    }
    return;
  }
  if (element.type === "text") {
    persistedString(element, "text", path);
    persistedNumber(element, "x", path);
    persistedNumber(element, "y", path);
    persistedNumber(element, "fontSize", path);
    persistedString(element, "fontFamily", path, true);
    persistedNumber(element, "fontWeight", path);
    persistedString(element, "fill", path, true);
    return;
  }
  throw new Error(`Document Mood Board invalid: ${path}.type nu este path sau text.`);
}

function validatePersistedMoodBoardItem(
  value: unknown,
  path: string,
  depth: number,
  validation: PersistedMoodBoardValidation,
) {
  if (depth > MAX_PERSISTED_MOOD_BOARD_NESTING) {
    throw new Error(`Document Mood Board invalid: ${path} depășește adâncimea maximă.`);
  }
  validation.itemCount += 1;
  if (validation.itemCount > MAX_PERSISTED_MOOD_BOARD_ITEMS) {
    throw new Error(`Document Mood Board invalid: depășește limita de ${MAX_PERSISTED_MOOD_BOARD_ITEMS} iteme.`);
  }

  const item = persistedObject(value, path);
  const id = persistedString(item, "id", path, true);
  if (validation.itemIds.has(id)) {
    throw new Error(`Document Mood Board invalid: id-ul ${JSON.stringify(id)} este duplicat.`);
  }
  validation.itemIds.add(id);
  const type = persistedString(item, "type", path, true);
  persistedNumber(item, "x", path);
  persistedNumber(item, "y", path);
  const width = persistedNumber(item, "width", path);
  const height = persistedNumber(item, "height", path);
  if (width <= 0 || height <= 0) {
    throw new Error(`Document Mood Board invalid: ${path} are dimensiuni nepozitive.`);
  }

  if (type === "note") {
    persistedString(item, "text", path);
    return;
  }
  if (type === "text") {
    persistedString(item, "text", path);
    persistedString(item, "color", path, true);
    persistedNumber(item, "fontSize", path);
    persistedNumber(item, "fontWeight", path);
    persistedEnum(item, "textAlign", path, ["left", "center", "right"]);
    return;
  }
  if (type === "color") {
    persistedString(item, "color", path, true);
    persistedString(item, "label", path);
    return;
  }
  if (type === "reference") {
    persistedString(item, "title", path);
    persistedString(item, "url", path);
    persistedString(item, "note", path);
    return;
  }
  if (type === "image") {
    persistedString(item, "title", path);
    persistedString(item, "path", path, true);
    persistedEnum(item, "fit", path, ["cover", "contain"]);
    persistedNumber(item, "radius", path);
    persistedBoolean(item, "shadow", path);
    const adjustments = persistedObject(item.adjustments, `${path}.adjustments`);
    for (const field of ["opacity", "brightness", "contrast", "saturation", "grayscale", "blur"]) {
      persistedNumber(adjustments, field, `${path}.adjustments`);
    }
    const framing = persistedObject(item.framing, `${path}.framing`);
    for (const field of ["positionX", "positionY", "scale"]) {
      persistedNumber(framing, field, `${path}.framing`);
    }
    if (item.mask !== undefined && item.mask !== null) {
      const mask = persistedObject(item.mask, `${path}.mask`);
      if (!Array.isArray(mask.nodes)) {
        throw new Error(`Document Mood Board invalid: ${path}.mask.nodes trebuie să fie listă.`);
      }
      mask.nodes.forEach((node, index) => validatePersistedVectorNode(node, `${path}.mask.nodes[${index}]`));
      persistedBoolean(mask, "closed", `${path}.mask`);
      persistedNumber(mask, "viewBoxWidth", `${path}.mask`);
      persistedNumber(mask, "viewBoxHeight", `${path}.mask`);
    }
    return;
  }
  if (type === "frame" || type === "group") {
    persistedString(item, "title", path);
    if (type === "frame") {
      persistedString(item, "tone", path, true);
      persistedEnum(item, "preset", path, ["custom", "desktop", "tablet", "mobile", "hero", "section"]);
      persistedString(item, "background", path, true);
    }
    if (!Array.isArray(item.children)) {
      throw new Error(`Document Mood Board invalid: ${path}.children trebuie să fie listă.`);
    }
    item.children.forEach((child, index) => {
      validatePersistedMoodBoardItem(child, `${path}.children[${index}]`, depth + 1, validation);
    });
    return;
  }
  if (type === "shape") {
    persistedEnum(item, "shape", path, ["rectangle", "ellipse", "diamond"]);
    persistedString(item, "fill", path, true);
    persistedString(item, "stroke", path, true);
    persistedNumber(item, "strokeWidth", path);
    return;
  }
  if (type === "vectorPath") {
    if (!Array.isArray(item.nodes) || item.nodes.length < 2) {
      throw new Error(`Document Mood Board invalid: ${path}.nodes cere minimum două noduri.`);
    }
    item.nodes.forEach((node, index) => validatePersistedVectorNode(node, `${path}.nodes[${index}]`));
    persistedBoolean(item, "closed", path);
    persistedString(item, "fill", path, true);
    persistedString(item, "stroke", path, true);
    persistedNumber(item, "strokeWidth", path);
    persistedNumber(item, "viewBoxWidth", path);
    persistedNumber(item, "viewBoxHeight", path);
    return;
  }
  if (type === "vectorGroup" || type === "svgGroup") {
    persistedString(item, "title", path, true);
    if (item.sourcePath !== undefined && typeof item.sourcePath !== "string") {
      throw new Error(`Document Mood Board invalid: ${path}.sourcePath trebuie să fie text.`);
    }
    validateOptionalPersistedStringArray(item.unsupportedFeatures, `${path}.unsupportedFeatures`);
    persistedNumber(item, "viewBoxX", path);
    persistedNumber(item, "viewBoxY", path);
    persistedNumber(item, "viewBoxWidth", path);
    persistedNumber(item, "viewBoxHeight", path);
    if (!Array.isArray(item.elements) || item.elements.length === 0) {
      throw new Error(`Document Mood Board invalid: ${path}.elements trebuie să fie listă nevidă.`);
    }
    const elementIds = new Set<string>();
    item.elements.forEach((element, index) => {
      const elementPath = `${path}.elements[${index}]`;
      validatePersistedVectorGroupElement(element, elementPath);
      const elementId = (element as Record<string, unknown>).id as string;
      if (elementIds.has(elementId)) {
        throw new Error(`Document Mood Board invalid: ${elementPath}.id este duplicat.`);
      }
      elementIds.add(elementId);
    });
    return;
  }
  throw new Error(`Document Mood Board invalid: ${path}.type ${JSON.stringify(type)} nu este cunoscut.`);
}

/**
 * Strict trust boundary for persisted/IPC Mood Board documents.
 *
 * `sanitizeMoodBoard` remains intentionally tolerant for explicit imports and
 * in-memory compatibility work. Disk reads must never turn malformed data into
 * an empty-but-editable document, so they pass through this validator first.
 */
export function parsePersistedMoodBoard(value: unknown): MoodBoard {
  const document = persistedObject(value, "document");
  if (document.version !== 2) {
    throw new Error("Document Mood Board invalid: este acceptată numai versiunea 2.");
  }
  const updatedAt = persistedNumber(document, "updatedAt", "document");
  if (updatedAt < 0) {
    throw new Error("Document Mood Board invalid: document.updatedAt nu poate fi negativ.");
  }
  const viewport = persistedObject(document.viewport, "document.viewport");
  persistedNumber(viewport, "x", "document.viewport");
  persistedNumber(viewport, "y", "document.viewport");
  const zoom = persistedNumber(viewport, "zoom", "document.viewport");
  if (zoom < 0.2 || zoom > 3) {
    throw new Error("Document Mood Board invalid: document.viewport.zoom este în afara intervalului 0.2–3.");
  }
  if (!Array.isArray(document.items)) {
    throw new Error("Document Mood Board invalid: document.items trebuie să fie listă.");
  }
  const validation: PersistedMoodBoardValidation = { itemCount: 0, itemIds: new Set() };
  document.items.forEach((item, index) => {
    validatePersistedMoodBoardItem(item, `document.items[${index}]`, 0, validation);
  });
  return sanitizeMoodBoard(document);
}

export function sanitizeMoodBoard(value: unknown): MoodBoard {
  if (!value || typeof value !== "object") return createEmptyMoodBoard();
  const source = value as Partial<MoodBoard>;
  return {
    version: 2,
    updatedAt: isFiniteNumber(source.updatedAt) ? source.updatedAt : Date.now(),
    viewport: sanitizeViewport(source.viewport),
    items: Array.isArray(source.items)
      ? source.items.map((item) => sanitizeItem(item as Partial<MoodBoardItem>)).filter((item): item is MoodBoardItem => Boolean(item))
      : [],
  };
}

export function flattenMoodBoardItems(items: MoodBoardItem[]): MoodBoardItem[] {
  return items.flatMap((item) => (
    item.type === "frame" || item.type === "group" ? [item, ...flattenMoodBoardItems(item.children)] : [item]
  ));
}

export function mapMoodBoardItems(items: MoodBoardItem[], mapper: (item: MoodBoardItem) => MoodBoardItem): MoodBoardItem[] {
  return items.map((item) => {
    const mapped = mapper(item);
    if (mapped.type !== "frame" && mapped.type !== "group") return mapped;
    return {
      ...mapped,
      children: mapMoodBoardItems(mapped.children, mapper),
    };
  });
}

export function findMoodBoardItem(items: MoodBoardItem[], itemId: string): MoodBoardItem | null {
  for (const item of items) {
    if (item.id === itemId) return item;
    if (item.type === "frame" || item.type === "group") {
      const child = findMoodBoardItem(item.children, itemId);
      if (child) return child;
    }
  }
  return null;
}

export function removeMoodBoardItems(items: MoodBoardItem[], itemIds: Set<string>): MoodBoardItem[] {
  return items
    .filter((item) => !itemIds.has(item.id))
    .map((item) => {
      if (item.type !== "frame" && item.type !== "group") return item;
      return {
        ...item,
        children: removeMoodBoardItems(item.children, itemIds),
      };
    });
}

export function cloneMoodBoard(board: MoodBoard): MoodBoard {
  return JSON.parse(JSON.stringify(board)) as MoodBoard;
}

export function sameMoodBoard(left: MoodBoard, right: MoodBoard) {
  return JSON.stringify(left) === JSON.stringify(right);
}
