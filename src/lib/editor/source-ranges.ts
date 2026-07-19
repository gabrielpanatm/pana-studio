import type { SourceRange } from "$lib/types";

export type CodeSelectionRange = {
  from: number;
  to: number;
};

export type CodeSelectionRanges = CodeSelectionRange | CodeSelectionRange[];

export type CssSelectorCursorTarget = {
  selector: string;
};

export function codeSelectionRangeForSourceRange(source: string, range: SourceRange): CodeSelectionRange {
  const from = sourceOffsetForLineColumn(source, range.line, range.column);
  const to = sourceOffsetForLineColumn(source, range.endLine, range.endColumn);
  return { from, to: Math.max(from, to) };
}

export function sourceOffsetForLineColumn(source: string, line: number, column: number) {
  const targetLine = Math.max(1, line);
  const targetColumn = Math.max(1, column);
  let lineStart = 0;
  let currentLine = 1;

  while (currentLine < targetLine) {
    const nextBreak = source.indexOf("\n", lineStart);
    if (nextBreak === -1) return source.length;
    lineStart = nextBreak + 1;
    currentLine += 1;
  }

  const lineEnd = source.indexOf("\n", lineStart);
  const maxOffset = lineEnd === -1 ? source.length : lineEnd;
  return Math.min(lineStart + targetColumn - 1, maxOffset);
}

export function codeSelectionRangeForCssSelector(source: string, selector: string): CodeSelectionRange[] | null {
  const target = normalizeSelector(selector);
  if (!target) return null;

  for (let braceIndex = source.indexOf("{"); braceIndex !== -1; braceIndex = source.indexOf("{", braceIndex + 1)) {
    const selectorStart = selectorStartBeforeBrace(source, braceIndex);
    const rawSelector = source.slice(selectorStart, braceIndex);
    if (isAtRuleSelector(rawSelector)) continue;
    const selectorParts = rawSelector
      .split(",")
      .map(normalizeSelector)
      .filter(Boolean);

    if (!selectorParts.includes(target)) continue;

    const trimmedStart = selectorStart + leadingWhitespaceLength(rawSelector);
    const ruleEnd = matchingClosingBrace(source, braceIndex);
    const ranges: CodeSelectionRange[] = [
      { from: trimmedStart, to: Math.max(trimmedStart + 1, braceIndex + 1) },
    ];
    if (ruleEnd !== -1) {
      ranges.push({ from: ruleEnd, to: ruleEnd + 1 });
    }
    return ranges;
  }

  return null;
}

export function cssSelectorAtPosition(source: string, position: number): CssSelectorCursorTarget | null {
  const cursor = Math.max(0, Math.min(position, source.length));
  const openBrace = source.indexOf("{", cursor);
  if (openBrace === -1) return null;

  const selectorStart = selectorStartBeforeBrace(source, openBrace);
  if (cursor < selectorStart || cursor > openBrace) return null;

  const rawSelector = source.slice(selectorStart, openBrace);
  if (isAtRuleSelector(rawSelector)) return null;

  const relativeCursor = cursor - selectorStart;
  const part = selectorPartAtOffset(rawSelector, relativeCursor);
  const selector = normalizeSelector(part);
  return selector.startsWith(".") ? { selector } : null;
}

function selectorStartBeforeBrace(source: string, braceIndex: number) {
  for (let index = braceIndex - 1; index >= 0; index -= 1) {
    const char = source[index];
    if (char === "}" || char === "{" || char === ";") return index + 1;
  }
  return 0;
}

function matchingClosingBrace(source: string, openBraceIndex: number) {
  let depth = 0;
  let quote: string | null = null;
  let inBlockComment = false;
  let inLineComment = false;

  for (let index = openBraceIndex; index < source.length; index += 1) {
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
      if (char === "\\") {
        index += 1;
      } else if (char === quote) {
        quote = null;
      }
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
      depth -= 1;
      if (depth === 0) return index;
    }
  }

  return -1;
}

function normalizeSelector(selector: string) {
  return selector
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function selectorPartAtOffset(selectorList: string, offset: number) {
  let start = 0;

  for (let index = 0; index <= selectorList.length; index += 1) {
    if (index < selectorList.length && selectorList[index] !== ",") continue;
    const end = index;
    if (offset >= start && offset <= end) {
      return selectorList.slice(start, end);
    }
    start = index + 1;
  }

  return selectorList;
}

function isAtRuleSelector(selector: string) {
  return normalizeSelector(selector).startsWith("@");
}

function leadingWhitespaceLength(value: string) {
  return value.match(/^\s*/)?.[0].length ?? 0;
}
