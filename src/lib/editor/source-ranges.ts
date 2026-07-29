import type { SourceRange } from "$lib/types";

export type CodeSelectionRange = {
  from: number;
  to: number;
};

export type CodeSelectionRanges = CodeSelectionRange | CodeSelectionRange[];

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
