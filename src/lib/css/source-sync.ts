import type { CssProperty, CssRuleContext } from "$lib/types";

export type CssViewport = CssRuleContext["viewport"];

type CssBlock = {
  selector: string;
  selectorStart: number;
  open: number;
  close: number;
  contentStart: number;
  contentEnd: number;
};

const viewportBreakpoints: Record<Exclude<CssViewport, "desktop">, { token: string; fallbacks: string[] }> = {
  tablet: { token: "$bp-tableta", fallbacks: ["1024px", "64rem"] },
  mobile: { token: "$bp-mobil", fallbacks: ["768px", "48rem"] },
};

export function cssRuleContextFromSource(
  source: string,
  file: string,
  selector: string,
  viewport: CssViewport,
): CssRuleContext {
  const baseBlock = findRuleBlock(source, selector);
  const baseRules = baseBlock ? parseDeclarations(source.slice(baseBlock.contentStart, baseBlock.contentEnd)) : [];
  const viewportBlock = viewport === "desktop" ? baseBlock : findViewportRuleBlock(source, selector, viewport);
  const viewportRules = viewportBlock
    ? parseDeclarations(source.slice(viewportBlock.contentStart, viewportBlock.contentEnd))
    : viewport === "desktop"
      ? baseRules
      : [];

  return {
    file,
    selector,
    viewport,
    resolvedBreakpoint: viewport === "desktop" ? null : viewportBreakpoints[viewport].token,
    baseRules,
    viewportRules,
    hasBaseRule: Boolean(baseBlock),
    hasViewportRule: viewport === "desktop" ? Boolean(baseBlock) : Boolean(viewportBlock),
  };
}

function findRuleBlock(source: string, selector: string, start = 0, end = source.length) {
  const target = normalizeSelector(selector);
  return collectImmediateBlocks(source, start, end).find((block) => {
    if (normalizeSelector(block.selector).startsWith("@")) return false;
    return block.selector.split(",").map(normalizeSelector).includes(target);
  }) ?? null;
}

function findViewportRuleBlock(source: string, selector: string, viewport: Exclude<CssViewport, "desktop">) {
  const mediaBlock = findViewportMediaBlock(source, viewport);
  return mediaBlock ? findRuleBlock(source, selector, mediaBlock.contentStart, mediaBlock.contentEnd) : null;
}

function findViewportMediaBlock(source: string, viewport: Exclude<CssViewport, "desktop">) {
  const meta = viewportBreakpoints[viewport];
  return collectImmediateBlocks(source).find((block) => {
    const selector = normalizeSelector(block.selector);
    if (!selector.startsWith("@media")) return false;
    return selector.includes(meta.token) || meta.fallbacks.some((fallback) => selector.includes(fallback));
  }) ?? null;
}

function parseDeclarations(content: string): CssProperty[] {
  const declarations: CssProperty[] = [];
  for (const segment of topLevelDeclarationSegments(content)) {
    const match = segment.match(/^\s*([\w-]+)\s*:\s*([\s\S]*?)\s*;\s*$/);
    if (match) {
      declarations.push({ property: match[1], value: match[2].trim() });
    }
  }
  return declarations;
}

function topLevelDeclarationSegments(content: string) {
  const segments: string[] = [];
  let depth = 0;
  let start = 0;
  let quote: string | null = null;
  let inBlockComment = false;
  let inLineComment = false;

  for (let index = 0; index < content.length; index += 1) {
    const char = content[index];
    const next = content[index + 1];

    if (inBlockComment) {
      if (char === "*" && next === "/") {
        inBlockComment = false;
        index += 1;
      }
      continue;
    }
    if (inLineComment) {
      if (char === "\n") inLineComment = false;
      continue;
    }
    if (quote) {
      if (char === "\\") index += 1;
      else if (char === quote) quote = null;
      continue;
    }
    if (char === "/" && next === "*") {
      inBlockComment = true;
      index += 1;
      continue;
    }
    if (char === "/" && next === "/") {
      inLineComment = true;
      index += 1;
      continue;
    }
    if (char === "\"" || char === "'") {
      quote = char;
      continue;
    }
    if (char === "{") {
      depth += 1;
      continue;
    }
    if (char === "}") {
      depth = Math.max(0, depth - 1);
      continue;
    }
    if (char === ";" && depth === 0) {
      segments.push(content.slice(start, index + 1));
      start = index + 1;
    }
  }

  return segments;
}

function collectImmediateBlocks(source: string, start = 0, end = source.length): CssBlock[] {
  const blocks: CssBlock[] = [];
  let depth = 0;
  let active: Omit<CssBlock, "close" | "contentEnd"> | null = null;
  let quote: string | null = null;
  let inBlockComment = false;
  let inLineComment = false;

  for (let index = start; index < end; index += 1) {
    const char = source[index];
    const next = source[index + 1];

    if (inBlockComment) {
      if (char === "*" && next === "/") {
        inBlockComment = false;
        index += 1;
      }
      continue;
    }
    if (inLineComment) {
      if (char === "\n") inLineComment = false;
      continue;
    }
    if (quote) {
      if (char === "\\") index += 1;
      else if (char === quote) quote = null;
      continue;
    }
    if (char === "/" && next === "*") {
      inBlockComment = true;
      index += 1;
      continue;
    }
    if (char === "/" && next === "/") {
      inLineComment = true;
      index += 1;
      continue;
    }
    if (char === "\"" || char === "'") {
      quote = char;
      continue;
    }
    if (char === "{") {
      if (depth === 0) {
        const selectorStart = selectorStartBeforeBrace(source, index, start);
        active = {
          selector: source.slice(selectorStart, index).trim(),
          selectorStart,
          open: index,
          contentStart: index + 1,
        };
      }
      depth += 1;
      continue;
    }
    if (char === "}" && depth > 0) {
      depth -= 1;
      if (depth === 0 && active) {
        blocks.push({ ...active, close: index, contentEnd: index });
        active = null;
      }
    }
  }

  return blocks;
}

function selectorStartBeforeBrace(source: string, braceIndex: number, minIndex: number) {
  for (let index = braceIndex - 1; index >= minIndex; index -= 1) {
    const char = source[index];
    if (char === "}" || char === "{" || char === ";") return index + 1;
  }
  return minIndex;
}

function normalizeSelector(selector: string) {
  return selector
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
