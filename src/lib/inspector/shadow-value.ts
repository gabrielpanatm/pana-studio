export type BoxShadowLayer = {
  x: string;
  y: string;
  blur: string;
  spread: string;
  color: string;
  inset: boolean;
};

export type TextShadowLayer = {
  x: string;
  y: string;
  blur: string;
  color: string;
};

export function splitShadowList(value: string): string[] | null {
  const parts: string[] = [];
  let current = "";
  let quote: string | null = null;
  let escaped = false;
  const stack: string[] = [];
  for (const character of value) {
    if (quote) {
      current += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "\"" || character === "'") {
      quote = character;
      current += character;
      continue;
    }
    if (character === "(") stack.push(character);
    else if (character === ")") {
      if (stack.pop() !== "(") return null;
    } else if (character === "," && stack.length === 0) {
      if (!current.trim()) return null;
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += character;
  }
  if (quote || stack.length > 0) return null;
  if (current.trim()) parts.push(current.trim());
  return parts;
}

function splitTopLevelWhitespace(value: string): string[] | null {
  const tokens: string[] = [];
  let current = "";
  let quote: string | null = null;
  let escaped = false;
  let depth = 0;
  for (const character of value) {
    if (quote) {
      current += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === "\"" || character === "'") {
      quote = character;
      current += character;
      continue;
    }
    if (character === "(") depth += 1;
    else if (character === ")") {
      depth -= 1;
      if (depth < 0) return null;
    }
    if (/\s/.test(character) && depth === 0) {
      if (current) tokens.push(current);
      current = "";
    } else {
      current += character;
    }
  }
  if (quote || depth !== 0) return null;
  if (current) tokens.push(current);
  return tokens;
}

function isColorToken(token: string) {
  return /^(?:#[\da-f]{3,8}|(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color|color-mix)\(|var\(|currentcolor$|transparent$|[a-z]+$)/i.test(token);
}

function isLengthToken(token: string) {
  return token === "0"
    || /^-?(?:\d+|\d*\.\d+)(?:[a-z%]+)$/i.test(token)
    || /^(?:calc|min|max|clamp|var)\(/i.test(token)
    || /^\$[a-z_][\w-]*$/i.test(token);
}

function parseLayerTokens(value: string, kind: "box" | "text") {
  const tokens = splitTopLevelWhitespace(value);
  if (!tokens) return null;
  let inset = false;
  const remaining = tokens.filter((token) => {
    if (kind === "box" && token.toLowerCase() === "inset" && !inset) {
      inset = true;
      return false;
    }
    return true;
  });
  const colorIndices = remaining
    .map((token, index) => isColorToken(token) ? index : -1)
    .filter((index) => index >= 0);
  if (colorIndices.length > 1) return null;
  const colorIndex = colorIndices[0] ?? -1;
  const color = colorIndex >= 0 ? remaining[colorIndex] : "currentColor";
  const lengths = remaining.filter((_, index) => index !== colorIndex);
  const minimum = 2;
  const maximum = kind === "box" ? 4 : 3;
  if (lengths.length < minimum || lengths.length > maximum || !lengths.every(isLengthToken)) {
    return null;
  }
  return { lengths, color, inset };
}

export function parseBoxShadowList(value: string): BoxShadowLayer[] | null {
  const normalized = value.trim();
  if (!normalized || normalized.toLowerCase() === "none") return [];
  const layers = splitShadowList(normalized);
  if (!layers) return null;
  const result: BoxShadowLayer[] = [];
  for (const layer of layers) {
    const parsed = parseLayerTokens(layer, "box");
    if (!parsed) return null;
    result.push({
      x: parsed.lengths[0],
      y: parsed.lengths[1],
      blur: parsed.lengths[2] ?? "0",
      spread: parsed.lengths[3] ?? "0",
      color: parsed.color,
      inset: parsed.inset,
    });
  }
  return result;
}

export function parseTextShadowList(value: string): TextShadowLayer[] | null {
  const normalized = value.trim();
  if (!normalized || normalized.toLowerCase() === "none") return [];
  const layers = splitShadowList(normalized);
  if (!layers) return null;
  const result: TextShadowLayer[] = [];
  for (const layer of layers) {
    const parsed = parseLayerTokens(layer, "text");
    if (!parsed) return null;
    result.push({
      x: parsed.lengths[0],
      y: parsed.lengths[1],
      blur: parsed.lengths[2] ?? "0",
      color: parsed.color,
    });
  }
  return result;
}

export function serializeBoxShadowList(layers: BoxShadowLayer[]) {
  return layers
    .map((layer) => `${layer.inset ? "inset " : ""}${layer.x} ${layer.y} ${layer.blur} ${layer.spread} ${layer.color}`)
    .join(", ");
}

export function serializeTextShadowList(layers: TextShadowLayer[]) {
  return layers
    .map((layer) => `${layer.x} ${layer.y} ${layer.blur} ${layer.color}`)
    .join(", ");
}
