import {
  cloneMoodBoard,
  mapMoodBoardItems,
  type MoodBoard,
  type MoodBoardItem,
  type MoodBoardResizeHandle,
} from "$lib/mood-board/model";
import {
  minSizeForMoodBoardItem,
  moodBoardItemIntersects,
  resizeMoodBoardVectorItem,
  resizeWithAspectRatio,
  type ResizeAspectAxis,
} from "$lib/mood-board/resize";
import { snapMoodBoardMove, snapMoodBoardResize, type MoodBoardSnapGuide } from "$lib/mood-board/snap";
import {
  guidesInRootCoordinates,
  rootAttachFrameForItems,
  snapContextItems,
} from "$lib/mood-board/tree";
import {
  attachRootItemToFrameIfNeeded,
  attachRootItemsToFrameIfNeeded,
  detachChildFromFrameIfNeeded,
} from "$lib/mood-board/frame-actions";

export type MoodBoardDragState = {
  kind: "item" | "pan" | "resize" | "marquee";
  itemId?: string;
  itemIds?: string[];
  itemOrigins?: Record<string, { x: number; y: number }>;
  initialSelection?: string[];
  additiveSelection?: boolean;
  startX: number;
  startY: number;
  originX: number;
  originY: number;
  originWidth?: number;
  originHeight?: number;
  resizeHandle?: MoodBoardResizeHandle;
  resizeAspectAxis?: ResizeAspectAxis;
  before: MoodBoard;
};

export type MoodBoardMarqueeBox = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type MoodBoardPointerDragUpdate = {
  board: MoodBoard;
  snapGuides: MoodBoardSnapGuide[];
  attachFrameId: string | null;
  resizeAspectAxis?: ResizeAspectAxis;
};

type MoodBoardPoint = {
  x: number;
  y: number;
};

type StageRect = Pick<DOMRect, "left" | "top">;

export function isMoodBoardSecondaryPointer(event: PointerEvent) {
  return event.button === 2 || (event.buttons & 2) === 2;
}

export function shouldIgnoreMoodBoardItemDrag(event: PointerEvent) {
  if (!(event.target instanceof HTMLElement)) return false;
  return Boolean(event.target.closest("input, textarea, button, [data-resize-handle]"))
    || Boolean(event.detail >= 2 && event.target.closest(".design-text-preview"));
}

export function isMoodBoardCanvasPanTarget(event: PointerEvent) {
  return event.target === event.currentTarget
    || (event.target instanceof HTMLElement && event.target.classList.contains("mood-content"));
}

export function createMoodBoardPanDragState(board: MoodBoard, event: PointerEvent): MoodBoardDragState {
  return {
    kind: "pan",
    startX: event.clientX,
    startY: event.clientY,
    originX: board.viewport.x,
    originY: board.viewport.y,
    before: cloneMoodBoard(board),
  };
}

export function createMoodBoardItemDragState(options: {
  board: MoodBoard;
  event: PointerEvent;
  item: MoodBoardItem;
  itemId: string;
  itemIds: string[];
  allItems: MoodBoardItem[];
}): MoodBoardDragState {
  const itemOrigins = Object.fromEntries(
    options.allItems
      .filter((entry) => options.itemIds.includes(entry.id))
      .map((entry) => [entry.id, { x: entry.x, y: entry.y }]),
  );

  return {
    kind: "item",
    itemId: options.itemId,
    itemIds: options.itemIds,
    itemOrigins,
    startX: options.event.clientX,
    startY: options.event.clientY,
    originX: options.item.x,
    originY: options.item.y,
    before: cloneMoodBoard(options.board),
  };
}

export function createMoodBoardResizeDragState(
  board: MoodBoard,
  event: PointerEvent,
  item: MoodBoardItem,
  resizeHandle: MoodBoardResizeHandle,
): MoodBoardDragState {
  return {
    kind: "resize",
    itemId: item.id,
    startX: event.clientX,
    startY: event.clientY,
    originX: item.x,
    originY: item.y,
    originWidth: item.width,
    originHeight: item.height,
    resizeHandle,
    before: cloneMoodBoard(board),
  };
}

export function createMoodBoardMarqueeDragState(
  board: MoodBoard,
  event: PointerEvent,
  initialSelection: string[],
  additiveSelection: boolean,
): MoodBoardDragState {
  return {
    kind: "marquee",
    startX: event.clientX,
    startY: event.clientY,
    originX: 0,
    originY: 0,
    initialSelection,
    additiveSelection,
    before: cloneMoodBoard(board),
  };
}

export function moodBoardMarqueeDragUpdate(
  state: MoodBoardDragState,
  stageRect: StageRect,
  start: MoodBoardPoint,
  end: MoodBoardPoint,
  visibleItems: MoodBoardItem[],
  clientX: number,
  clientY: number,
) {
  if (state.kind !== "marquee") return null;

  const left = Math.min(start.x, end.x);
  const top = Math.min(start.y, end.y);
  const right = Math.max(start.x, end.x);
  const bottom = Math.max(start.y, end.y);
  const hitIds = visibleItems
    .filter((item) => moodBoardItemIntersects(left, top, right, bottom, item))
    .map((item) => item.id);

  return {
    marqueeBox: {
      x: Math.min(state.startX, clientX) - stageRect.left,
      y: Math.min(state.startY, clientY) - stageRect.top,
      width: Math.abs(clientX - state.startX),
      height: Math.abs(clientY - state.startY),
    },
    selectedItemIds: state.additiveSelection
      ? [...(state.initialSelection ?? []), ...hitIds]
      : hitIds,
  };
}

export function moodBoardPointerDragUpdate(
  state: MoodBoardDragState,
  event: PointerEvent,
): MoodBoardPointerDragUpdate | null {
  const baseBoard = state.before;
  const zoom = baseBoard.viewport.zoom || 1;
  const dx = event.clientX - state.startX;
  const dy = event.clientY - state.startY;

  if (state.kind === "pan") {
    return {
      board: {
        ...baseBoard,
        viewport: { ...baseBoard.viewport, x: state.originX + dx, y: state.originY + dy },
      },
      snapGuides: [],
      attachFrameId: null,
    };
  }

  if (state.kind === "resize") {
    return resizeMoodBoardItemDuringDrag(state, event.shiftKey, dx, dy, zoom);
  }

  if (state.kind === "item") {
    return moveMoodBoardItemsDuringDrag(state, dx / zoom, dy / zoom, zoom);
  }

  return null;
}

export function finalizeMoodBoardItemDragAttachment(current: MoodBoard, state: MoodBoardDragState) {
  if (state.kind !== "item") return current;

  const itemIds = state.itemIds ?? (state.itemId ? [state.itemId] : []);
  if (itemIds.length === 1 && state.itemId) {
    return attachRootItemToFrameIfNeeded(detachChildFromFrameIfNeeded(current, state.itemId), state.itemId);
  }
  if (itemIds.length > 1) return attachRootItemsToFrameIfNeeded(current, itemIds);
  return current;
}

function resizeMoodBoardItemDuringDrag(
  state: MoodBoardDragState,
  lockAspectRatio: boolean,
  dx: number,
  dy: number,
  zoom: number,
): MoodBoardPointerDragUpdate {
  const baseBoard = state.before;
  let resizeAspectAxis = state.resizeAspectAxis;
  let snapGuides: MoodBoardSnapGuide[] = [];

  const board = {
    ...baseBoard,
    items: mapMoodBoardItems(baseBoard.items, (item) => {
      if (item.id !== state.itemId) return item;

      const minSize = minSizeForMoodBoardItem(item);
      const handle = state.resizeHandle ?? "se";
      const originWidth = state.originWidth ?? item.width;
      const originHeight = state.originHeight ?? item.height;
      const moveX = dx / zoom;
      const moveY = dy / zoom;
      let rect = lockAspectRatio
        ? resizeWithAspectRatio({
          handle,
          originX: state.originX,
          originY: state.originY,
          originWidth,
          originHeight,
          moveX,
          moveY,
          minSize,
          lockedAxis: resizeAspectAxis,
        })
        : null;

      if (rect && !resizeAspectAxis && Math.max(Math.abs(moveX), Math.abs(moveY)) > 2) {
        resizeAspectAxis = rect.axis;
      }

      let x = rect?.rect.x ?? state.originX;
      let y = rect?.rect.y ?? state.originY;
      let width = rect?.rect.width ?? originWidth;
      let height = rect?.rect.height ?? originHeight;

      if (!rect) {
        if (handle.includes("e")) {
          width = originWidth + moveX;
        } else {
          width = originWidth - moveX;
          x = state.originX + moveX;
        }

        if (handle.includes("s")) {
          height = originHeight + moveY;
        } else {
          height = originHeight - moveY;
          y = state.originY + moveY;
        }

        if (width < minSize.width) {
          if (handle.includes("w")) x = state.originX + originWidth - minSize.width;
          width = minSize.width;
        }

        if (height < minSize.height) {
          if (handle.includes("n")) y = state.originY + originHeight - minSize.height;
          height = minSize.height;
        }
      }

      const snapped = snapMoodBoardResize(
        snapContextItems(baseBoard, new Set([item.id])),
        item.id,
        { x, y, width, height },
        handle,
        zoom,
      );
      snapGuides = guidesInRootCoordinates(baseBoard, item.id, snapped.guides);
      const snappedSnapshot = snapped.snapshot;

      return {
        ...resizeMoodBoardVectorItem(
          item,
          Math.max(minSize.width, snappedSnapshot.width),
          Math.max(minSize.height, snappedSnapshot.height),
        ),
        x: snappedSnapshot.x,
        y: snappedSnapshot.y,
      };
    }),
  };

  return {
    board,
    snapGuides,
    attachFrameId: null,
    resizeAspectAxis,
  };
}

function moveMoodBoardItemsDuringDrag(
  state: MoodBoardDragState,
  rawDx: number,
  rawDy: number,
  zoom: number,
): MoodBoardPointerDragUpdate {
  const baseBoard = state.before;
  const itemOrigins = state.itemOrigins ?? {};
  const draggedIds = new Set(state.itemIds ?? (state.itemId ? [state.itemId] : []));
  const snapped = snapMoodBoardMove(snapContextItems(baseBoard, draggedIds), draggedIds, itemOrigins, rawDx, rawDy, zoom);
  const board = {
    ...baseBoard,
    items: mapMoodBoardItems(baseBoard.items, (item) => {
      if (!draggedIds.has(item.id)) return item;
      const origin = itemOrigins[item.id] ?? { x: item.x, y: item.y };
      return { ...item, x: origin.x + snapped.dx, y: origin.y + snapped.dy };
    }),
  };

  return {
    board,
    snapGuides: state.itemId ? guidesInRootCoordinates(baseBoard, state.itemId, snapped.guides) : snapped.guides,
    attachFrameId: rootAttachFrameForItems(board, [...draggedIds])?.id ?? null,
  };
}
