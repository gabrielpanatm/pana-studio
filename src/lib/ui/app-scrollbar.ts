export const APP_SCROLLBAR_MINIMUM_THUMB_SIZE = 28;

export type AppScrollbarGeometry = Readonly<{
  overflow: boolean;
  viewportSize: number;
  contentSize: number;
  trackSize: number;
  maxScrollOffset: number;
  thumbSize: number;
  thumbTravel: number;
  thumbOffset: number;
}>;

function finiteNonNegative(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

export function appScrollbarGeometry(
  viewportSize: number,
  contentSize: number,
  scrollOffset: number,
  trackSize = viewportSize,
  minimumThumbSize = APP_SCROLLBAR_MINIMUM_THUMB_SIZE,
): AppScrollbarGeometry {
  const viewport = finiteNonNegative(viewportSize);
  const content = finiteNonNegative(contentSize);
  const track = finiteNonNegative(trackSize);
  const minimumThumb = finiteNonNegative(minimumThumbSize);
  const maxScrollOffset = Math.max(0, content - viewport);
  const overflow = maxScrollOffset > 0 && track > 0;
  const thumbSize = overflow
    ? Math.min(
        track,
        Math.max(minimumThumb, track * viewport / Math.max(content, 1)),
      )
    : track;
  const thumbTravel = Math.max(0, track - thumbSize);
  const boundedScrollOffset = Math.min(
    maxScrollOffset,
    finiteNonNegative(scrollOffset),
  );
  const thumbOffset = maxScrollOffset > 0 && thumbTravel > 0
    ? boundedScrollOffset / maxScrollOffset * thumbTravel
    : 0;

  return {
    overflow,
    viewportSize: viewport,
    contentSize: content,
    trackSize: track,
    maxScrollOffset,
    thumbSize,
    thumbTravel,
    thumbOffset,
  };
}

export function appScrollbarOffsetFromThumbDelta(
  geometry: AppScrollbarGeometry,
  initialScrollOffset: number,
  thumbDelta: number,
): number {
  if (!geometry.overflow || geometry.thumbTravel <= 0) return 0;
  const next = finiteNonNegative(initialScrollOffset)
    + thumbDelta * geometry.maxScrollOffset / geometry.thumbTravel;
  return Math.min(geometry.maxScrollOffset, Math.max(0, next));
}

export function appScrollbarOffsetFromTrackPoint(
  geometry: AppScrollbarGeometry,
  trackPoint: number,
): number {
  if (!geometry.overflow || geometry.thumbTravel <= 0) return 0;
  const centeredThumb = finiteNonNegative(trackPoint) - geometry.thumbSize / 2;
  const boundedThumb = Math.min(
    geometry.thumbTravel,
    Math.max(0, centeredThumb),
  );
  return boundedThumb / geometry.thumbTravel * geometry.maxScrollOffset;
}
