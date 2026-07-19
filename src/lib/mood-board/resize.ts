import type {
  MoodBoardItem,
  MoodBoardResizeHandle,
} from "$lib/mood-board/model";

export type MoodBoardSize = {
  width: number;
  height: number;
};

export type MoodBoardRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type ResizeAspectAxis = "x" | "y";

export function minSizeForMoodBoardItem(item: MoodBoardItem): MoodBoardSize {
  if (item.type === "color") return { width: 110, height: 130 };
  if (item.type === "image") return { width: 80, height: 80 };
  if (item.type === "reference") return { width: 220, height: 150 };
  if (item.type === "frame") return { width: 260, height: 180 };
  if (item.type === "group") return { width: 40, height: 40 };
  if (item.type === "shape") return { width: 60, height: 60 };
  if (item.type === "vectorPath") return { width: 80, height: 80 };
  if (item.type === "vectorGroup") return { width: 80, height: 80 };
  if (item.type === "text") return { width: 80, height: 44 };
  return { width: 160, height: 110 };
}

export function resizeWithAspectRatio(options: {
  handle: MoodBoardResizeHandle;
  originX: number;
  originY: number;
  originWidth: number;
  originHeight: number;
  moveX: number;
  moveY: number;
  minSize: MoodBoardSize;
  lockedAxis?: ResizeAspectAxis;
}): { rect: MoodBoardRect; axis: ResizeAspectAxis } {
  const ratio = options.originWidth / Math.max(1, options.originHeight);
  const widthDirection = options.handle.includes("e") ? 1 : -1;
  const heightDirection = options.handle.includes("s") ? 1 : -1;
  const widthDelta = options.moveX * widthDirection;
  const heightDeltaAsWidth = options.moveY * heightDirection * ratio;
  const axis = options.lockedAxis ?? (
    Math.abs(widthDelta) >= Math.abs(heightDeltaAsWidth) ? "x" : "y"
  );
  const projectedWidthDelta = axis === "x" ? widthDelta : heightDeltaAsWidth;
  let width = options.originWidth + projectedWidthDelta;
  let height = width / ratio;

  if (width < options.minSize.width) {
    width = options.minSize.width;
    height = width / ratio;
  }
  if (height < options.minSize.height) {
    height = options.minSize.height;
    width = height * ratio;
  }

  return {
    axis,
    rect: {
      x: options.handle.includes("w") ? options.originX + options.originWidth - width : options.originX,
      y: options.handle.includes("n") ? options.originY + options.originHeight - height : options.originY,
      width,
      height,
    },
  };
}

export function resizeMoodBoardVectorItem(item: MoodBoardItem, width: number, height: number): MoodBoardItem {
  if (item.type === "group") {
    const scaleX = width / Math.max(1, item.width);
    const scaleY = height / Math.max(1, item.height);
    const scaleChild = (child: MoodBoardItem): MoodBoardItem => {
      const nextChild = {
        ...child,
        x: Math.round(child.x * scaleX * 10) / 10,
        y: Math.round(child.y * scaleY * 10) / 10,
        width: Math.max(1, Math.round(child.width * scaleX * 10) / 10),
        height: Math.max(1, Math.round(child.height * scaleY * 10) / 10),
      };
      if (nextChild.type !== "frame" && nextChild.type !== "group") return nextChild;
      return {
        ...nextChild,
        children: nextChild.children.map(scaleChild),
      };
    };
    return {
      ...item,
      width,
      height,
      children: item.children.map(scaleChild),
    };
  }
  if (item.type !== "vectorPath") return { ...item, width, height };
  const scaleX = width / Math.max(1, item.viewBoxWidth);
  const scaleY = height / Math.max(1, item.viewBoxHeight);
  return {
    ...item,
    width,
    height,
    viewBoxWidth: width,
    viewBoxHeight: height,
    nodes: item.nodes.map((node) => ({
      x: Math.round(node.x * scaleX * 10) / 10,
      y: Math.round(node.y * scaleY * 10) / 10,
      in: node.in ? {
        x: Math.round(node.in.x * scaleX * 10) / 10,
        y: Math.round(node.in.y * scaleY * 10) / 10,
      } : null,
      out: node.out ? {
        x: Math.round(node.out.x * scaleX * 10) / 10,
        y: Math.round(node.out.y * scaleY * 10) / 10,
      } : null,
      handleMode: node.handleMode,
    })),
  };
}

export function moodBoardItemIntersects(
  left: number,
  top: number,
  right: number,
  bottom: number,
  item: MoodBoardItem,
) {
  return item.x < right
    && item.x + item.width > left
    && item.y < bottom
    && item.y + item.height > top;
}
