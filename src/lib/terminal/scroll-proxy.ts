export type TerminalScrollProxyGeometry = {
  contentHeightPx: number;
  maxLine: number;
  scrollTopPx: number;
};

type TerminalScrollProxyInput = {
  viewportHeightPx: number;
  rows: number;
  baseY: number;
  viewportY: number;
};

function finiteNonNegative(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

export function deriveTerminalScrollProxyGeometry(
  input: TerminalScrollProxyInput,
): TerminalScrollProxyGeometry {
  const viewportHeightPx = finiteNonNegative(input.viewportHeightPx);
  const rows = Math.max(1, Math.floor(finiteNonNegative(input.rows)));
  const maxLine = Math.floor(finiteNonNegative(input.baseY));
  const viewportY = Math.min(
    maxLine,
    Math.floor(finiteNonNegative(input.viewportY)),
  );
  const scrollRangePx = maxLine > 0
    ? viewportHeightPx * maxLine / rows
    : 0;

  return {
    contentHeightPx: viewportHeightPx + scrollRangePx,
    maxLine,
    scrollTopPx: maxLine > 0
      ? scrollRangePx * viewportY / maxLine
      : 0,
  };
}

export function terminalLineFromProxyScroll(
  scrollTopPx: number,
  maxScrollTopPx: number,
  maxLine: number,
): number {
  const safeMaxLine = Math.floor(finiteNonNegative(maxLine));
  const safeMaxScrollTop = finiteNonNegative(maxScrollTopPx);
  if (safeMaxLine === 0 || safeMaxScrollTop === 0) return 0;

  const safeScrollTop = Math.min(
    safeMaxScrollTop,
    finiteNonNegative(scrollTopPx),
  );
  return Math.round(safeScrollTop / safeMaxScrollTop * safeMaxLine);
}
