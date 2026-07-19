import type { SourceGraphNode, SourceRange } from "$lib/types";
import type { TeraDropResolution } from "$lib/tera/model";

type AllowedTeraDropResolution = Extract<TeraDropResolution, { allowed: true }>;
const templateLevelTeraKinds = new Set(["extends", "block", "import", "macro"]);
const deletableTeraKinds = new Set([
  "include",
  "for",
  "if",
  "set",
  "with",
  "teraVariable",
  "teraComment",
]);

export type TeraMutationCapability = {
  canRun: boolean;
  label: string;
  reason: string;
};

export function deleteTeraNodeCapability(node: SourceGraphNode | null): TeraMutationCapability {
  if (!node) {
    return {
      canRun: false,
      label: "Șterge nod Tera",
      reason: "Selectează un nod Tera pentru a activa mutația.",
    };
  }

  if (templateLevelTeraKinds.has(node.kind)) {
    return {
      canRun: false,
      label: "Șterge Tera",
      reason: "Directivele Tera de nivel template se editează din cod sau printr-o acțiune dedicată, nu prin delete vizual.",
    };
  }

  if (node.kind === "tera") {
    return {
      canRun: false,
      label: "Șterge Tera",
      reason: "Sintaxa Tera nespecializată se editează din cod sau printr-o acțiune dedicată, nu prin delete vizual.",
    };
  }

  if (node.kind === "raw") {
    return {
      canRun: false,
      label: "Șterge Tera",
      reason: "Blocurile raw Tera sunt scope-uri code-only și se editează din cod sau printr-o acțiune dedicată, nu prin delete vizual.",
    };
  }

  if (!deletableTeraKinds.has(node.kind)) {
    return {
      canRun: false,
      label: "Șterge nod Tera",
      reason: "Nodul selectat nu este o construcție Tera editabilă.",
    };
  }

  if (!node.range) {
    return {
      canRun: false,
      label: "Șterge Tera",
      reason: "Nodul Tera nu are încă o ancoră de sursă suficientă pentru editare sigură.",
    };
  }

  return {
    canRun: true,
    label: node.kind === "include" ? "Șterge include" : "Șterge Tera",
    reason: "Elimină construcția Tera din template-ul care o deține.",
  };
}

export function deleteTeraNodeFromSource(source: string, node: SourceGraphNode): string {
  const capability = deleteTeraNodeCapability(node);
  if (!capability.canRun || !node.range) {
    throw new Error(capability.reason);
  }

  return removeSourceRangePreservingLines(source, node.range);
}

export function insertTeraDropIntoSource(source: string, resolution: AllowedTeraDropResolution): string {
  if (!resolution.anchor.range) {
    throw new Error("Nu pot insera Tera fără o ancoră de sursă stabilă.");
  }
  return insertTeraSnippetAtRange(source, resolution.anchor.range, resolution.position, resolution.snippet);
}

export function replaceTeraNodeSource(source: string, node: SourceGraphNode, nextSnippet: string): string {
  if (!node.range || !deletableTeraKinds.has(node.kind)) {
    throw new Error("Nodul Tera nu poate fi editat source-preserving.");
  }
  const start = sourceIndexFromLocation(source, node.range.line, node.range.column);
  const end = sourceIndexFromLocation(source, node.range.endLine, node.range.endColumn);
  if (start === null || end === null || end <= start) {
    throw new Error("Nu pot localiza exact nodul Tera în sursă.");
  }
  return source.slice(0, start) + nextSnippet + source.slice(end);
}

export function teraNodeSourceText(source: string, node: SourceGraphNode): string {
  if (!node.range || !deletableTeraKinds.has(node.kind)) {
    throw new Error("Nodul Tera nu poate fi extras source-preserving.");
  }
  const block = sourceBlockForRange(source, node.range);
  if (!block) {
    throw new Error("Nu pot localiza exact nodul Tera în sursă.");
  }
  return block.text.trimEnd();
}

export function removeTeraNodeFromSource(source: string, node: SourceGraphNode): string {
  const capability = deleteTeraNodeCapability(node);
  if (!capability.canRun || !node.range) {
    throw new Error(capability.reason);
  }
  return removeSourceRangePreservingLines(source, node.range);
}

export function insertTeraSourceAtAnchor(
  source: string,
  anchor: SourceGraphNode,
  position: AllowedTeraDropResolution["position"],
  snippet: string,
): string {
  if (!anchor.range) {
    throw new Error("Ancora Tera nu are range de sursă stabil.");
  }
  return insertTeraSnippetAtRange(source, anchor.range, position, snippet);
}

function insertTeraSnippetAtRange(
  source: string,
  range: SourceRange,
  position: AllowedTeraDropResolution["position"],
  snippet: string,
) {
  const start = sourceIndexFromLocation(source, range.line, range.column);
  const end = sourceIndexFromLocation(source, range.endLine, range.endColumn);

  if (start === null || end === null || end < start) {
    throw new Error("Nu pot localiza exact ancora Tera în sursă.");
  }

  const anchorStartLineStart = lineStartIndex(source, start);
  const anchorStartLineBreak = lineBreakIndex(source, start);
  const anchorEndLineBreak = lineBreakIndex(source, end);
  const insertIndent = indentationForLine(source, anchorStartLineStart);
  const nestedIndent = `${insertIndent}  `;
  const block = formatInsertedTeraSnippet(snippet, position === "inside" ? nestedIndent : insertIndent);

  if (position === "before") {
    return insertSourceBlock(source, anchorStartLineStart, block);
  }

  const insertLineBreak = position === "inside" ? anchorStartLineBreak : anchorEndLineBreak;
  const insertIndex = insertLineBreak === -1 ? source.length : insertLineBreak + 1;
  return insertSourceBlock(source, insertIndex, block);
}

function lineStartIndex(source: string, index: number) {
  return source.lastIndexOf("\n", Math.max(0, index - 1)) + 1;
}

function lineBreakIndex(source: string, index: number) {
  return source.indexOf("\n", index);
}

function indentationForLine(source: string, lineStart: number) {
  const lineEnd = source.indexOf("\n", lineStart);
  const line = source.slice(lineStart, lineEnd === -1 ? source.length : lineEnd);
  return line.match(/^\s*/)?.[0] ?? "";
}

function formatInsertedTeraSnippet(snippet: string, indent: string) {
  const lines = stripCommonIndent(snippet.replace(/\s+$/g, "")).split("\n");
  const body = lines.map((line) => line.trim().length === 0 ? "" : `${indent}${line}`).join("\n");
  return `${body}\n`;
}

function stripCommonIndent(snippet: string) {
  const lines = snippet.split("\n");
  const contentLines = lines.filter((line) => line.trim().length > 0);
  const commonIndentLength = Math.min(
    ...contentLines.map((line) => line.match(/^[ \t]*/)?.[0].length ?? 0),
  );
  if (!Number.isFinite(commonIndentLength) || commonIndentLength <= 0) return snippet;
  return lines
    .map((line) => line.trim().length === 0 ? "" : line.slice(commonIndentLength))
    .join("\n");
}

function insertSourceBlock(source: string, index: number, block: string) {
  const needsLeadingBreak = index > 0 && source[index - 1] !== "\n";
  const insertion = needsLeadingBreak ? `\n${block}` : block;
  return source.slice(0, index) + insertion + source.slice(index);
}

function removeSourceRangePreservingLines(source: string, range: SourceRange) {
  const block = sourceBlockForRange(source, range);
  if (!block) {
    throw new Error("Nu pot localiza exact nodul Tera în sursă.");
  }
  return source.slice(0, block.start) + source.slice(block.end);
}

function sourceBlockForRange(source: string, range: SourceRange) {
  const start = sourceIndexFromLocation(source, range.line, range.column);
  const end = sourceIndexFromLocation(source, range.endLine, range.endColumn);

  if (start === null || end === null || end <= start) {
    return null;
  }

  const lineStart = source.lastIndexOf("\n", Math.max(0, start - 1)) + 1;
  const nextLineBreak = source.indexOf("\n", end);
  const lineEnd = nextLineBreak === -1 ? source.length : nextLineBreak;
  const lineEndWithBreak = nextLineBreak === -1 ? source.length : nextLineBreak + 1;
  const beforeRangeOnLine = source.slice(lineStart, start);
  const afterRangeOnLine = source.slice(end, lineEnd);

  if (beforeRangeOnLine.trim() === "" && afterRangeOnLine.trim() === "") {
    return {
      start: lineStart,
      end: lineEndWithBreak,
      text: source.slice(lineStart, lineEndWithBreak),
    };
  }

  return {
    start,
    end,
    text: source.slice(start, end),
  };
}

function sourceIndexFromLocation(source: string, line: number, column: number) {
  if (!Number.isFinite(line) || !Number.isFinite(column) || line < 1 || column < 1) return null;

  let currentLine = 1;
  let lineStart = 0;
  while (currentLine < line) {
    const nextLine = source.indexOf("\n", lineStart);
    if (nextLine === -1) return null;
    lineStart = nextLine + 1;
    currentLine += 1;
  }

  return Math.min(source.length, lineStart + column - 1);
}
