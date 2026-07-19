import type { MoodBoardItem } from "$lib/mood-board/model";

export type MoodBoardSnapGuide = {
  orientation: "vertical" | "horizontal";
  position: number;
  start: number;
  end: number;
};

export type MoodBoardResizeSnapshot = {
  x: number;
  y: number;
  width: number;
  height: number;
};

type Bounds = {
  left: number;
  top: number;
  right: number;
  bottom: number;
  centerX: number;
  centerY: number;
};

type SnapCandidate = {
  delta: number;
  distance: number;
  target: Bounds;
  targetPosition: number;
};

const SNAP_THRESHOLD_PX = 8;
const GUIDE_PADDING = 28;
const FRAME_INNER_PADDING = 24;

function boundsFor(item: MoodBoardItem, origin?: { x: number; y: number }): Bounds {
  const left = origin?.x ?? item.x;
  const top = origin?.y ?? item.y;
  const right = left + item.width;
  const bottom = top + item.height;
  return {
    left,
    top,
    right,
    bottom,
    centerX: left + item.width / 2,
    centerY: top + item.height / 2,
  };
}

function insetBounds(bounds: Bounds, padding: number): Bounds {
  const left = bounds.left + padding;
  const top = bounds.top + padding;
  const right = bounds.right - padding;
  const bottom = bounds.bottom - padding;
  return {
    left,
    top,
    right,
    bottom,
    centerX: left + (right - left) / 2,
    centerY: top + (bottom - top) / 2,
  };
}

function snapTargetsFor(item: MoodBoardItem): Bounds[] {
  const outer = boundsFor(item);
  if (item.type !== "frame" || item.width <= FRAME_INNER_PADDING * 2 || item.height <= FRAME_INNER_PADDING * 2) {
    return [outer];
  }

  return [outer, insetBounds(outer, FRAME_INNER_PADDING)];
}

function groupBounds(bounds: Bounds[]): Bounds | null {
  if (bounds.length === 0) return null;
  const left = Math.min(...bounds.map((entry) => entry.left));
  const top = Math.min(...bounds.map((entry) => entry.top));
  const right = Math.max(...bounds.map((entry) => entry.right));
  const bottom = Math.max(...bounds.map((entry) => entry.bottom));
  return {
    left,
    top,
    right,
    bottom,
    centerX: left + (right - left) / 2,
    centerY: top + (bottom - top) / 2,
  };
}

function bestCandidate(movingPositions: number[], targets: Bounds[], targetPositions: (target: Bounds) => number[], threshold: number) {
  let best: SnapCandidate | null = null;

  for (const movingPosition of movingPositions) {
    for (const target of targets) {
      for (const targetPosition of targetPositions(target)) {
        const delta = targetPosition - movingPosition;
        const distance = Math.abs(delta);
        if (distance > threshold) continue;
        if (!best || distance < best.distance) {
          best = { delta, distance, target, targetPosition };
        }
      }
    }
  }

  return best;
}

function translated(bounds: Bounds, dx: number, dy: number): Bounds {
  return {
    left: bounds.left + dx,
    top: bounds.top + dy,
    right: bounds.right + dx,
    bottom: bounds.bottom + dy,
    centerX: bounds.centerX + dx,
    centerY: bounds.centerY + dy,
  };
}

export function snapMoodBoardMove(
  items: MoodBoardItem[],
  movingIds: Set<string>,
  itemOrigins: Record<string, { x: number; y: number }>,
  rawDx: number,
  rawDy: number,
  zoom: number,
) {
  const movingItems = items.filter((item) => movingIds.has(item.id));
  const targetBounds = items
    .filter((item) => !movingIds.has(item.id))
    .flatMap((item) => snapTargetsFor(item));
  const movingBounds = groupBounds(movingItems.map((item) => boundsFor(item, itemOrigins[item.id])));
  const threshold = SNAP_THRESHOLD_PX / Math.max(0.2, zoom || 1);

  if (!movingBounds || targetBounds.length === 0) {
    return { dx: rawDx, dy: rawDy, guides: [] as MoodBoardSnapGuide[] };
  }

  const proposed = translated(movingBounds, rawDx, rawDy);
  const xCandidate = bestCandidate(
    [proposed.left, proposed.centerX, proposed.right],
    targetBounds,
    (target) => [target.left, target.centerX, target.right],
    threshold,
  );
  const yCandidate = bestCandidate(
    [proposed.top, proposed.centerY, proposed.bottom],
    targetBounds,
    (target) => [target.top, target.centerY, target.bottom],
    threshold,
  );

  const dx = rawDx + (xCandidate?.delta ?? 0);
  const dy = rawDy + (yCandidate?.delta ?? 0);
  const snapped = translated(movingBounds, dx, dy);
  const guides: MoodBoardSnapGuide[] = [];

  if (xCandidate) {
    guides.push({
      orientation: "vertical",
      position: xCandidate.targetPosition,
      start: Math.min(snapped.top, xCandidate.target.top) - GUIDE_PADDING,
      end: Math.max(snapped.bottom, xCandidate.target.bottom) + GUIDE_PADDING,
    });
  }

  if (yCandidate) {
    guides.push({
      orientation: "horizontal",
      position: yCandidate.targetPosition,
      start: Math.min(snapped.left, yCandidate.target.left) - GUIDE_PADDING,
      end: Math.max(snapped.right, yCandidate.target.right) + GUIDE_PADDING,
    });
  }

  return { dx, dy, guides };
}

export function snapMoodBoardResize(
  items: MoodBoardItem[],
  itemId: string,
  snapshot: MoodBoardResizeSnapshot,
  handle: "nw" | "ne" | "sw" | "se",
  zoom: number,
) {
  const movingBounds = {
    left: snapshot.x,
    top: snapshot.y,
    right: snapshot.x + snapshot.width,
    bottom: snapshot.y + snapshot.height,
    centerX: snapshot.x + snapshot.width / 2,
    centerY: snapshot.y + snapshot.height / 2,
  };
  const targetBounds = items
    .filter((item) => item.id !== itemId)
    .flatMap((item) => snapTargetsFor(item));
  const threshold = SNAP_THRESHOLD_PX / Math.max(0.2, zoom || 1);

  if (targetBounds.length === 0) {
    return { snapshot, guides: [] as MoodBoardSnapGuide[] };
  }

  const xCandidate = bestCandidate(
    [handle.includes("e") ? movingBounds.right : movingBounds.left],
    targetBounds,
    (target) => [target.left, target.centerX, target.right],
    threshold,
  );
  const yCandidate = bestCandidate(
    [handle.includes("s") ? movingBounds.bottom : movingBounds.top],
    targetBounds,
    (target) => [target.top, target.centerY, target.bottom],
    threshold,
  );

  let next = { ...snapshot };
  const guides: MoodBoardSnapGuide[] = [];

  if (xCandidate) {
    if (handle.includes("e")) {
      next.width += xCandidate.delta;
    } else {
      next.x += xCandidate.delta;
      next.width -= xCandidate.delta;
    }
    const snappedBounds = {
      ...movingBounds,
      left: next.x,
      right: next.x + next.width,
      top: next.y,
      bottom: next.y + next.height,
    };
    guides.push({
      orientation: "vertical",
      position: xCandidate.targetPosition,
      start: Math.min(snappedBounds.top, xCandidate.target.top) - GUIDE_PADDING,
      end: Math.max(snappedBounds.bottom, xCandidate.target.bottom) + GUIDE_PADDING,
    });
  }

  if (yCandidate) {
    if (handle.includes("s")) {
      next.height += yCandidate.delta;
    } else {
      next.y += yCandidate.delta;
      next.height -= yCandidate.delta;
    }
    const snappedBounds = {
      ...movingBounds,
      left: next.x,
      right: next.x + next.width,
      top: next.y,
      bottom: next.y + next.height,
    };
    guides.push({
      orientation: "horizontal",
      position: yCandidate.targetPosition,
      start: Math.min(snappedBounds.left, yCandidate.target.left) - GUIDE_PADDING,
      end: Math.max(snappedBounds.right, yCandidate.target.right) + GUIDE_PADDING,
    });
  }

  return { snapshot: next, guides };
}
