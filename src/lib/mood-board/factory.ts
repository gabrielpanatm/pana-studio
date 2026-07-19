import { defaultImageAdjustments } from "$lib/mood-board/image-adjustments";
import { defaultImageFraming } from "$lib/mood-board/image-framing";
import {
  createMoodBoardItemId,
  type MoodBoardItem,
} from "$lib/mood-board/model";

export type MoodBoardPoint = {
  x: number;
  y: number;
};

export type FramePreset = "desktop" | "tablet" | "mobile" | "hero" | "section";

export const framePresets: Record<FramePreset, { title: string; width: number; height: number }> = {
  desktop: { title: "Desktop 1440", width: 1440, height: 900 },
  tablet: { title: "Tablet", width: 768, height: 1024 },
  mobile: { title: "Mobile", width: 390, height: 844 },
  hero: { title: "Hero section", width: 1440, height: 720 },
  section: { title: "Section", width: 1440, height: 600 },
};

export function createMoodBoardNote(point: MoodBoardPoint): MoodBoardItem {
  return {
    id: createMoodBoardItemId(),
    type: "note",
    x: point.x - 120,
    y: point.y - 70,
    width: 240,
    height: 140,
    text: "Notă nouă",
  };
}

export function createMoodBoardText(point: MoodBoardPoint): MoodBoardItem {
  return {
    id: createMoodBoardItemId(),
    type: "text",
    x: point.x - 150,
    y: point.y - 50,
    width: 300,
    height: 100,
    text: "Titlu nou",
    color: "#1a2825",
    fontSize: 34,
    fontWeight: 800,
    textAlign: "left",
  };
}

export function createMoodBoardColor(point: MoodBoardPoint): MoodBoardItem {
  return {
    id: createMoodBoardItemId(),
    type: "color",
    x: point.x - 70,
    y: point.y - 80,
    width: 140,
    height: 160,
    color: "#1d7f6a",
    label: "Accent",
  };
}

export function createMoodBoardReference(point: MoodBoardPoint): MoodBoardItem {
  return {
    id: createMoodBoardItemId(),
    type: "reference",
    x: point.x - 140,
    y: point.y - 85,
    width: 280,
    height: 170,
    title: "Referință",
    url: "",
    note: "",
  };
}

export function createMoodBoardFrame(point: MoodBoardPoint, preset: FramePreset = "desktop"): MoodBoardItem {
  const frame = framePresets[preset];
  return {
    id: createMoodBoardItemId(),
    type: "frame",
    x: point.x - frame.width / 2,
    y: point.y - frame.height / 2,
    width: frame.width,
    height: frame.height,
    title: frame.title,
    tone: "#1d7f6a",
    preset,
    background: "#ffffff",
    children: [],
  };
}

export function createMoodBoardShape(point: MoodBoardPoint): MoodBoardItem {
  return {
    id: createMoodBoardItemId(),
    type: "shape",
    x: point.x - 80,
    y: point.y - 60,
    width: 160,
    height: 120,
    shape: "rectangle",
    fill: "#ffffff",
    stroke: "#1d7f6a",
    strokeWidth: 2,
  };
}

export function createMoodBoardVectorPath(point: MoodBoardPoint): MoodBoardItem {
  return {
    id: createMoodBoardItemId(),
    type: "vectorPath",
    x: point.x - 120,
    y: point.y - 80,
    width: 240,
    height: 160,
    nodes: [
      { x: 24, y: 96, out: { x: 34, y: -76 } },
      { x: 118, y: 34, in: { x: -44, y: -8 }, out: { x: 52, y: 8 } },
      { x: 216, y: 92, in: { x: -34, y: -76 }, out: { x: 30, y: 70 } },
      { x: 116, y: 142, in: { x: 58, y: 12 }, out: { x: -58, y: -12 } },
    ],
    closed: true,
    fill: "#d8eee8",
    stroke: "#1d7f6a",
    strokeWidth: 3,
    viewBoxWidth: 240,
    viewBoxHeight: 160,
  };
}

export function imageTitleFromPath(path: string) {
  return path.split("/").filter(Boolean).at(-1) ?? "Imagine";
}

export function isSvgPath(path: string) {
  return path.trim().toLowerCase().split(/[?#]/, 1)[0]?.endsWith(".svg") ?? false;
}

export function createMoodBoardImageItem(path: string, point: MoodBoardPoint): MoodBoardItem | null {
  const normalizedPath = path.trim().replaceAll("\\", "/");
  if (!normalizedPath) return null;
  return {
    id: createMoodBoardItemId(),
    type: "image",
    x: point.x - 140,
    y: point.y - 110,
    width: 280,
    height: 220,
    title: imageTitleFromPath(normalizedPath),
    path: normalizedPath,
    fit: "cover",
    radius: 8,
    shadow: true,
    adjustments: { ...defaultImageAdjustments },
    framing: { ...defaultImageFraming },
  };
}

export function createMoodBoardPaletteItems(
  sourceItem: Extract<MoodBoardItem, { type: "image" }>,
  colors: string[],
) {
  const startX = sourceItem.x;
  const startY = sourceItem.y + sourceItem.height + 18;
  return colors.map((color, index): MoodBoardItem => ({
    id: createMoodBoardItemId(),
    type: "color",
    x: startX + index * 132,
    y: startY,
    width: 120,
    height: 132,
    color,
    label: `Paletă ${index + 1}`,
  }));
}
