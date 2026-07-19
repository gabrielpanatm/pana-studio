export const DEFAULT_BACKGROUND_GRADIENT = "linear-gradient(180deg, #ffffff 0%, #000000 100%)";

export type GradientType = "linear" | "radial" | "conic";

export type GradientStop = {
  id: number;
  color: string;
  opacity: number;
  position: number;
};

export type GradientState = {
  type: GradientType;
  angle: number;
  stops: GradientStop[];
};

let nextStopId = 0;

export function createGradientStop(color: string, opacity: number, position: number): GradientStop {
  return { id: nextStopId++, color, opacity, position };
}

export function createDefaultGradientState(): GradientState {
  return {
    type: "linear",
    angle: 180,
    stops: [createGradientStop("#ffffff", 100, 0), createGradientStop("#000000", 100, 100)],
  };
}

function splitTopLevelCommas(value: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = "";

  for (const char of value) {
    if (char === "(") depth += 1;
    else if (char === ")") depth -= 1;
    else if (char === "," && depth === 0) {
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += char;
  }

  if (current.trim()) parts.push(current.trim());
  return parts;
}

function parseGradientColor(value: string): { hex: string; opacity: number } | null {
  const trimmed = value.trim();
  const rgba = trimmed.match(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+))?\s*\)/);

  if (rgba) {
    const r = parseInt(rgba[1], 10);
    const g = parseInt(rgba[2], 10);
    const b = parseInt(rgba[3], 10);
    const a = rgba[4] !== undefined ? parseFloat(rgba[4]) : 1;
    return {
      hex: `#${[r, g, b].map((channel) => channel.toString(16).padStart(2, "0")).join("")}`,
      opacity: Math.round(a * 100),
    };
  }

  if (trimmed.startsWith("#")) {
    const hex = trimmed.length === 4
      ? `#${trimmed[1]}${trimmed[1]}${trimmed[2]}${trimmed[2]}${trimmed[3]}${trimmed[3]}`
      : trimmed.slice(0, 7);
    return { hex, opacity: 100 };
  }

  return null;
}

function parseStopToken(token: string): { colorValue: string; position: number } | null {
  const match = token.trim().match(/^([\s\S]+?)\s+([\d.]+)%\s*$/);
  if (!match) return null;
  return { colorValue: match[1].trim(), position: parseFloat(match[2]) };
}

function parseGradientStops(tokens: string[]): GradientStop[] {
  return tokens
    .map((token) => {
      const parsed = parseStopToken(token);
      if (!parsed) return null;
      const color = parseGradientColor(parsed.colorValue);
      if (!color) return null;
      const { hex, opacity } = color;
      return createGradientStop(hex, opacity, parsed.position);
    })
    .filter((stop): stop is GradientStop => Boolean(stop));
}

export function tryParseBackgroundGradient(css: string): GradientState | null {
  if (!css || css === "none") return null;

  const linearMatch = css.match(/^linear-gradient\(([\s\S]+)\)$/i);
  if (linearMatch) {
    const parts = splitTopLevelCommas(linearMatch[1]);
    if (!parts.length) return null;

    let angle = 180;
    let start = 0;
    const first = parts[0].trim();
    const angleMatch = first.match(/^(-?[\d.]+)deg$/i);
    const toMatch = first.match(/^to\s+(top|bottom|left|right)/i);

    if (angleMatch) {
      angle = parseFloat(angleMatch[1]);
      start = 1;
    } else if (toMatch) {
      angle = ({ top: 0, right: 90, bottom: 180, left: 270 } as Record<string, number>)[toMatch[1].toLowerCase()] ?? 180;
      start = 1;
    }

    const stops = parseGradientStops(parts.slice(start));
    if (stops.length >= 2) return { type: "linear", angle, stops };
  }

  const radialMatch = css.match(/^radial-gradient\(([\s\S]+)\)$/i);
  if (radialMatch) {
    const parts = splitTopLevelCommas(radialMatch[1]);
    if (!parts.length) return null;
    const first = parts[0].trim();
    const start = /^(circle|ellipse|closest|farthest)/.test(first) ? 1 : 0;
    const stops = parseGradientStops(parts.slice(start));
    if (stops.length >= 2) return { type: "radial", angle: 0, stops };
  }

  const conicMatch = css.match(/^conic-gradient\(([\s\S]+)\)$/i);
  if (conicMatch) {
    const parts = splitTopLevelCommas(conicMatch[1]);
    if (!parts.length) return null;

    let angle = 0;
    let start = 0;
    const fromMatch = parts[0].trim().match(/^from\s+(-?[\d.]+)deg$/i);

    if (fromMatch) {
      angle = parseFloat(fromMatch[1]);
      start = 1;
    }

    const stops = parseGradientStops(parts.slice(start));
    if (stops.length >= 2) return { type: "conic", angle, stops };
  }

  return null;
}

export function parseBackgroundGradient(css: string): GradientState {
  return tryParseBackgroundGradient(css) ?? createDefaultGradientState();
}

export function isBackgroundGradientStructurallyEditable(css: string): boolean {
  return tryParseBackgroundGradient(css) !== null;
}

export function colorWithAlpha(hex: string, opacity: number): string {
  if (opacity >= 100) return hex;
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return `rgba(${r}, ${g}, ${b}, ${(opacity / 100).toFixed(2)})`;
}

export function serializeBackgroundGradient(gradient: GradientState): string {
  const sorted = [...gradient.stops].sort((a, b) => a.position - b.position);
  const stops = sorted.map((stop) => `${colorWithAlpha(stop.color, stop.opacity)} ${stop.position}%`).join(", ");

  if (gradient.type === "linear") return `linear-gradient(${gradient.angle}deg, ${stops})`;
  if (gradient.type === "radial") return `radial-gradient(circle, ${stops})`;
  return `conic-gradient(from ${gradient.angle}deg, ${stops})`;
}

export function gradientPreviewBackground(stops: GradientStop[]): string {
  const sorted = [...stops].sort((a, b) => a.position - b.position);
  const serializedStops = sorted.map((stop) => `${colorWithAlpha(stop.color, stop.opacity)} ${stop.position}%`).join(", ");
  return `linear-gradient(to right, ${serializedStops})`;
}

export function gradientPositionFromClientX(clientX: number, rect: { left: number; width: number }): number {
  if (rect.width <= 0) return 0;
  return Math.round(Math.max(0, Math.min(100, ((clientX - rect.left) / rect.width) * 100)));
}

export function gradientStopAppearanceAtPosition(stops: GradientStop[], position: number): Pick<GradientStop, "color" | "opacity"> {
  const sorted = [...stops].sort((a, b) => a.position - b.position);
  const first = sorted[0];
  if (!first) return { color: "#000000", opacity: 100 };

  let color = first.color;
  let opacity = first.opacity;

  for (let index = 0; index < sorted.length - 1; index += 1) {
    const current = sorted[index];
    const next = sorted[index + 1];
    if (position < current.position || position > next.position) continue;

    const distance = next.position - current.position;
    const ratio = distance === 0 ? 0 : (position - current.position) / distance;
    color = ratio >= 0.5 ? next.color : current.color;
    opacity = ratio >= 0.5 ? next.opacity : current.opacity;
    break;
  }

  return { color, opacity };
}
