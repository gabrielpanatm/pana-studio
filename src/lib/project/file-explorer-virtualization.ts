export type FileExplorerVirtualWindow = Readonly<{
  start: number;
  end: number;
  topSpacerPx: number;
  bottomSpacerPx: number;
}>;

export function projectFileExplorerVirtualWindow(
  rowCount: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight = 25,
  overscan = 10,
): FileExplorerVirtualWindow {
  const count = Math.max(0, Math.floor(rowCount));
  const height = Math.max(1, rowHeight);
  const padding = Math.max(0, Math.floor(overscan));
  const viewport = Math.max(0, viewportHeight);
  const offset = Math.max(0, scrollTop);
  const start = Math.max(0, Math.floor(offset / height) - padding);
  const end = Math.min(
    count,
    Math.ceil((offset + viewport) / height) + padding,
  );
  return {
    start,
    end,
    topSpacerPx: start * height,
    bottomSpacerPx: Math.max(0, (count - end) * height),
  };
}

export function projectFileExplorerScrollTopForIndex(
  index: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight = 25,
): number {
  const height = Math.max(1, rowHeight);
  const rowTop = Math.max(0, Math.floor(index)) * height;
  const rowBottom = rowTop + height;
  const viewportTop = Math.max(0, scrollTop);
  const viewport = Math.max(0, viewportHeight);
  if (viewport === 0 || rowTop < viewportTop) return rowTop;
  if (rowBottom > viewportTop + viewport) {
    return Math.max(0, rowBottom - viewport);
  }
  return viewportTop;
}
