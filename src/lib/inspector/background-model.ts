import type { CssProperty } from "$lib/types";

export const CSS_BACKGROUND_SCHEMA_VERSION = 1;

export const BACKGROUND_LONGHAND_PROPERTIES = [
  "background-color",
  "background-image",
  "background-position",
  "background-size",
  "background-repeat",
  "background-attachment",
  "background-origin",
  "background-clip",
  "background-blend-mode",
] as const;

export type CssBackgroundLayerKind = "image" | "gradient" | "opaque";
export type CssGradientKind = "linear" | "radial" | "conic";

export type CssGradientStop = {
  kind: "stop";
  id: string;
  color: string;
  positions: string[];
  raw: string;
};

export type CssGradientHint = {
  kind: "hint";
  id: string;
  position: string;
  raw: string;
};

export type CssGradientOpaqueItem = {
  kind: "opaque";
  id: string;
  raw: string;
};

export type CssGradientItem = CssGradientStop | CssGradientHint | CssGradientOpaqueItem;

export type CssGradient = {
  kind: CssGradientKind;
  repeating: boolean;
  prelude: string;
  items: CssGradientItem[];
  raw: string;
  structurallyEditable: boolean;
};

export type CssBackgroundLayer = {
  id: string;
  kind: CssBackgroundLayerKind;
  source: string;
  position: string;
  size: string;
  repeat: string;
  attachment: string;
  origin: string;
  clip: string;
  blendMode: string;
  gradient: CssGradient | null;
  structurallyEditable: boolean;
};

export type CssBackground = {
  schemaVersion: number;
  color: string | null;
  layers: CssBackgroundLayer[];
  shorthand: string | null;
  opaqueProperties: Record<string, string>;
  structurallyEditable: boolean;
};

const BACKGROUND_LIST_DEFAULTS = {
  position: "0% 0%",
  size: "auto",
  repeat: "repeat",
  attachment: "scroll",
  origin: "padding-box",
  clip: "border-box",
  blendMode: "normal",
} as const;

let nextBackgroundId = 1;

function uniqueId(prefix: string) {
  const id = nextBackgroundId;
  nextBackgroundId += 1;
  return `${prefix}-${id}`;
}

export function splitTopLevelCssList(
  value: string,
  separator: "comma" | "space" = "comma",
  emptyItemFallback: string | null = null,
): string[] | null {
  const result: string[] = [];
  let start = 0;
  let quote: string | null = null;
  let escaped = false;
  let parenDepth = 0;
  let bracketDepth = 0;
  let interpolationDepth = 0;
  let lineComment = false;
  let blockComment = false;

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (lineComment) {
      if (character === "\n") lineComment = false;
      continue;
    }
    if (blockComment) {
      if (character === "*" && value[index + 1] === "/") {
        blockComment = false;
        index += 1;
      }
      continue;
    }
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "/" && value[index + 1] === "/") {
      lineComment = true;
      index += 1;
      continue;
    }
    if (character === "/" && value[index + 1] === "*") {
      blockComment = true;
      index += 1;
      continue;
    }
    if (character === "\"" || character === "'") {
      quote = character;
      continue;
    }
    if (character === "#" && value[index + 1] === "{") {
      interpolationDepth += 1;
      index += 1;
      continue;
    }
    if (character === "(") parenDepth += 1;
    else if (character === ")") {
      if (parenDepth === 0) return null;
      parenDepth -= 1;
    } else if (character === "[") bracketDepth += 1;
    else if (character === "]") {
      if (bracketDepth === 0) return null;
      bracketDepth -= 1;
    } else if (character === "}" && interpolationDepth > 0) interpolationDepth -= 1;

    const topLevel = parenDepth === 0 && bracketDepth === 0 && interpolationDepth === 0;
    const separates = separator === "comma"
      ? character === "," && topLevel
      : /\s/.test(character) && topLevel;
    if (!separates) continue;
    const part = value.slice(start, index).trim();
    if (part) result.push(part);
    else if (separator === "comma") {
      if (emptyItemFallback === null) return null;
      result.push(emptyItemFallback);
    }
    start = index + 1;
  }
  if (quote || blockComment || parenDepth || bracketDepth || interpolationDepth) return null;
  const tail = value.slice(start).trim();
  if (tail) result.push(tail);
  else if (separator === "comma" && emptyItemFallback !== null && result.length) {
    result.push(emptyItemFallback);
  }
  return result.length ? result : null;
}

function functionName(value: string): string | null {
  const trimmed = value.trim();
  const open = trimmed.indexOf("(");
  if (open <= 0 || !trimmed.endsWith(")")) return null;
  const name = trimmed.slice(0, open).trim();
  return /^[a-z][a-z0-9-]*$/i.test(name) ? name.toLowerCase() : null;
}

function numericUnit(value: string, unit: string) {
  if (!value.endsWith(unit)) return false;
  const number = value.slice(0, -unit.length);
  return number !== "" && Number.isFinite(Number(number));
}

function dynamicPosition(value: string) {
  return value.startsWith("$") || ["var(", "calc(", "min(", "max(", "clamp(", "env("].some((prefix) => value.startsWith(prefix));
}

export function isGradientPosition(value: string) {
  return [
    "%", "px", "em", "rem", "ch", "ex", "vw", "vh", "vmin", "vmax", "cm", "mm",
    "q", "in", "pc", "pt", "deg", "grad", "rad", "turn",
  ].some((unit) => numericUnit(value, unit)) || value === "0" || dynamicPosition(value);
}

function isAngle(value: string) {
  return ["deg", "grad", "rad", "turn"].some((unit) => numericUnit(value, unit)) || dynamicPosition(value);
}

function isGradientPrelude(kind: CssGradientKind, value: string) {
  const normalized = value.trim().toLowerCase();
  if (kind === "linear") return normalized.startsWith("to ") || isAngle(normalized);
  if (kind === "conic") return normalized.startsWith("from ") || normalized.startsWith("at ");
  return normalized.includes(" at ")
    || normalized.startsWith("at ")
    || ["circle", "ellipse", "closest-side", "closest-corner", "farthest-side", "farthest-corner"]
      .some((prefix) => normalized.startsWith(prefix));
}

function parseGradientItem(value: string, index: number): CssGradientItem {
  const raw = value.trim();
  const tokens = splitTopLevelCssList(value, "space");
  if (!tokens) return { kind: "opaque", id: uniqueId(`gradient-opaque-${index}`), raw };
  if (tokens.length === 1 && isGradientPosition(tokens[0]) && !dynamicPosition(tokens[0])) {
    return { kind: "hint", id: uniqueId(`gradient-hint-${index}`), position: tokens[0], raw };
  }
  let positionStart = tokens.length;
  while (
    positionStart > 0
    && tokens.length - positionStart < 2
    && isGradientPosition(tokens[positionStart - 1])
  ) positionStart -= 1;
  const color = tokens.slice(0, positionStart).join(" ");
  if (!color) return { kind: "opaque", id: uniqueId(`gradient-opaque-${index}`), raw };
  return {
    kind: "stop",
    id: uniqueId(`gradient-stop-${index}`),
    color,
    positions: tokens.slice(positionStart),
    raw,
  };
}

export function parseCssGradient(value: string): CssGradient | null {
  const name = functionName(value);
  if (!name) return null;
  const definition = ({
    "linear-gradient": ["linear", false],
    "repeating-linear-gradient": ["linear", true],
    "radial-gradient": ["radial", false],
    "repeating-radial-gradient": ["radial", true],
    "conic-gradient": ["conic", false],
    "repeating-conic-gradient": ["conic", true],
  } as const)[name];
  if (!definition) return null;
  const [kind, repeating] = definition;
  const open = value.indexOf("(");
  const parts = splitTopLevelCssList(value.slice(open + 1, -1));
  if (!parts) return null;
  const hasPrelude = isGradientPrelude(kind, parts[0] ?? "");
  const prelude = hasPrelude ? parts[0].trim() : "";
  const items = parts.slice(hasPrelude ? 1 : 0).map(parseGradientItem);
  const stopCount = items.filter((item) => item.kind === "stop").length;
  return {
    kind,
    repeating,
    prelude,
    items,
    raw: value.trim(),
    structurallyEditable: stopCount >= 2 && items.every((item) => item.kind !== "opaque"),
  };
}

function meaningful(value: string | undefined) {
  const trimmed = value?.trim() ?? "";
  return trimmed ? trimmed : null;
}

function repeated(values: string[] | null, index: number, fallback: string) {
  return values?.length ? values[index % values.length] : fallback;
}

function isOpaqueListExpression(value: string | undefined) {
  const trimmed = value?.trim() ?? "";
  return trimmed.startsWith("$") || trimmed.startsWith("var(") || trimmed.startsWith("#{");
}

export function backgroundFromProperties(properties: CssProperty[] | Readonly<Record<string, string>>): CssBackground {
  const declarations = Array.isArray(properties)
    ? Object.fromEntries(properties.map((property) => [property.property.toLowerCase(), property.value]))
    : properties;
  const imageValue = declarations["background-image"]?.trim() ?? "";
  const sources = !imageValue || imageValue.toLowerCase() === "none"
    ? []
    : splitTopLevelCssList(imageValue) ?? [imageValue];
  const lists = {
    position: splitOptionalList(declarations["background-position"], BACKGROUND_LIST_DEFAULTS.position),
    size: splitOptionalList(declarations["background-size"], BACKGROUND_LIST_DEFAULTS.size),
    repeat: splitOptionalList(declarations["background-repeat"], BACKGROUND_LIST_DEFAULTS.repeat),
    attachment: splitOptionalList(declarations["background-attachment"], BACKGROUND_LIST_DEFAULTS.attachment),
    origin: splitOptionalList(declarations["background-origin"], BACKGROUND_LIST_DEFAULTS.origin),
    clip: splitOptionalList(declarations["background-clip"], BACKGROUND_LIST_DEFAULTS.clip),
    blendMode: splitOptionalList(declarations["background-blend-mode"], BACKGROUND_LIST_DEFAULTS.blendMode),
  };
  const propertyByList = {
    position: "background-position",
    size: "background-size",
    repeat: "background-repeat",
    attachment: "background-attachment",
    origin: "background-origin",
    clip: "background-clip",
    blendMode: "background-blend-mode",
  } as const;
  const opaqueProperties = Object.fromEntries(Object.entries(lists)
    .filter(([, list]) => list === null)
    .map(([field]) => {
      const property = propertyByList[field as keyof typeof propertyByList];
      return [property, declarations[property] ?? ""];
    }));
  const listsEditable = Object.keys(opaqueProperties).length === 0;
  const layers = sources.map((source, index): CssBackgroundLayer => {
    const gradient = parseCssGradient(source);
    const name = functionName(source);
    const kind: CssBackgroundLayerKind = gradient
      ? "gradient"
      : name && ["url", "image", "image-set", "cross-fade", "element"].includes(name)
        ? "image"
        : "opaque";
    const structurallyEditable = listsEditable
      && (kind === "image" || (kind === "gradient" && Boolean(gradient?.structurallyEditable)));
    return {
      id: uniqueId(`background-layer-${index}`),
      kind,
      source,
      position: repeated(lists.position, index, BACKGROUND_LIST_DEFAULTS.position),
      size: repeated(lists.size, index, BACKGROUND_LIST_DEFAULTS.size),
      repeat: repeated(lists.repeat, index, BACKGROUND_LIST_DEFAULTS.repeat),
      attachment: repeated(lists.attachment, index, BACKGROUND_LIST_DEFAULTS.attachment),
      origin: repeated(lists.origin, index, BACKGROUND_LIST_DEFAULTS.origin),
      clip: repeated(lists.clip, index, BACKGROUND_LIST_DEFAULTS.clip),
      blendMode: repeated(lists.blendMode, index, BACKGROUND_LIST_DEFAULTS.blendMode),
      gradient,
      structurallyEditable,
    };
  });
  const shorthand = meaningful(declarations.background);
  return {
    schemaVersion: CSS_BACKGROUND_SCHEMA_VERSION,
    color: meaningful(declarations["background-color"]),
    layers,
    shorthand,
    opaqueProperties,
    structurallyEditable: !shorthand && listsEditable && layers.every((layer) => layer.structurallyEditable),
  };
}

function splitOptionalList(value: string | undefined, emptyItemFallback: string): string[] | null {
  if (!value?.trim()) return [];
  if (isOpaqueListExpression(value)) return null;
  // A focused editor field is allowed to become empty between keystrokes. An
  // older implementation serialized that transient state as `, value`, then
  // classified its own output as opaque and disabled the field. Empty slots
  // are recoverable as the CSS initial value for that longhand; genuinely
  // dynamic or unbalanced expressions still remain opaque and untouched.
  return splitTopLevelCssList(value, "comma", emptyItemFallback);
}

export function serializeCssGradient(gradient: CssGradient): string {
  const name = `${gradient.repeating ? "repeating-" : ""}${gradient.kind}-gradient`;
  const items = gradient.items.map((item) => {
    if (item.kind === "stop") return [item.color, ...item.positions].filter(Boolean).join(" ");
    if (item.kind === "hint") return item.position;
    return item.raw;
  });
  const body = [gradient.prelude.trim(), ...items].filter(Boolean).join(", ");
  return `${name}(${body})`;
}

export function serializeBackgroundLonghands(background: CssBackground): Record<string, string> {
  const properties: Record<string, string> = {
    "background-color": background.color ?? "",
  };
  if (!background.layers.length) {
    properties["background-image"] = "none";
    for (const property of BACKGROUND_LONGHAND_PROPERTIES.slice(2)) properties[property] = "";
    return { ...properties, ...background.opaqueProperties };
  }
  const join = (
    read: (layer: CssBackgroundLayer) => string,
    emptyItemFallback: string,
  ) => background.layers
    .map((layer) => read(layer).trim() || emptyItemFallback)
    .join(", ");
  properties["background-image"] = join((layer) => layer.source, "none");
  properties["background-position"] = join((layer) => layer.position, BACKGROUND_LIST_DEFAULTS.position);
  properties["background-size"] = join((layer) => layer.size, BACKGROUND_LIST_DEFAULTS.size);
  properties["background-repeat"] = join((layer) => layer.repeat, BACKGROUND_LIST_DEFAULTS.repeat);
  properties["background-attachment"] = join((layer) => layer.attachment, BACKGROUND_LIST_DEFAULTS.attachment);
  properties["background-origin"] = join((layer) => layer.origin, BACKGROUND_LIST_DEFAULTS.origin);
  properties["background-clip"] = join((layer) => layer.clip, BACKGROUND_LIST_DEFAULTS.clip);
  properties["background-blend-mode"] = join((layer) => layer.blendMode, BACKGROUND_LIST_DEFAULTS.blendMode);
  return { ...properties, ...background.opaqueProperties };
}

export function createDefaultGradient(kind: CssGradientKind = "linear"): CssGradient {
  const prelude = kind === "linear" ? "180deg" : kind === "radial" ? "circle at center" : "from 0deg at center";
  const items: CssGradientItem[] = [
    { kind: "stop", id: uniqueId("gradient-stop"), color: "#ffffff", positions: ["0%"], raw: "#ffffff 0%" },
    { kind: "stop", id: uniqueId("gradient-stop"), color: "#000000", positions: ["100%"], raw: "#000000 100%" },
  ];
  const gradient: CssGradient = { kind, repeating: false, prelude, items, raw: "", structurallyEditable: true };
  gradient.raw = serializeCssGradient(gradient);
  return gradient;
}

export function createBackgroundLayer(kind: "image" | "gradient"): CssBackgroundLayer {
  const gradient = kind === "gradient" ? createDefaultGradient() : null;
  return {
    id: uniqueId("background-layer"),
    kind,
    source: gradient ? serializeCssGradient(gradient) : 'url("")',
    position: "center",
    size: kind === "image" ? "cover" : "auto",
    repeat: "no-repeat",
    attachment: "scroll",
    origin: "padding-box",
    clip: "border-box",
    blendMode: "normal",
    gradient,
    structurallyEditable: true,
  };
}

export function cloneBackgroundLayer(layer: CssBackgroundLayer): CssBackgroundLayer {
  return {
    ...layer,
    id: uniqueId("background-layer"),
    gradient: layer.gradient ? {
      ...layer.gradient,
      items: layer.gradient.items.map((item) => ({ ...item, id: uniqueId(`gradient-${item.kind}`) })),
    } : null,
  };
}

export function gradientStopVisualPosition(stop: CssGradientStop, index: number, count: number) {
  const first = stop.positions[0] ?? "";
  const match = first.match(/^(-?[\d.]+)%$/);
  if (match) return Math.max(0, Math.min(100, Number(match[1])));
  return count <= 1 ? 0 : Math.round((index / (count - 1)) * 100);
}
