import type { SourceRange } from "$lib/types";

export type CodeSelectionRange = {
  from: number;
  to: number;
};

export type CodeSelectionRanges = CodeSelectionRange | CodeSelectionRange[];

export function codeSelectionRangeForSourceRange(source: string, range: SourceRange): CodeSelectionRange {
  const hasByteRange = Number.isSafeInteger(range.start)
    && Number.isSafeInteger(range.end)
    && range.start >= 0
    && range.end >= range.start;
  const from = hasByteRange
    ? sourceOffsetForUtf8ByteOffset(source, range.start)
    : sourceOffsetForLineColumn(source, range.line, range.column);
  const to = hasByteRange
    ? sourceOffsetForUtf8ByteOffset(source, range.end)
    : sourceOffsetForLineColumn(source, range.endLine, range.endColumn);
  return { from, to: Math.max(from, to) };
}

export function sourceOffsetForUtf8ByteOffset(source: string, byteOffset: number) {
  const target = Math.max(0, byteOffset);
  let consumedBytes = 0;
  let codeUnitOffset = 0;
  for (const character of source) {
    const codePoint = character.codePointAt(0) ?? 0;
    const byteLength = codePoint <= 0x7f
      ? 1
      : codePoint <= 0x7ff
        ? 2
        : codePoint <= 0xffff
          ? 3
          : 4;
    if (consumedBytes + byteLength > target) break;
    consumedBytes += byteLength;
    codeUnitOffset += character.length;
  }
  return codeUnitOffset;
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
