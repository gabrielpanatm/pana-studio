import type {
  MoodBoardImageAdjustments,
  MoodBoardImageFraming,
} from "$lib/mood-board/model";

function numberOrFallback(value: string, fallback: number) {
  const numberValue = Number(value);
  return Number.isFinite(numberValue) ? numberValue : fallback;
}

export function moodBoardImageRadiusValue(value: string) {
  return Math.min(48, Math.max(0, numberOrFallback(value, 0)));
}

export function moodBoardImageAdjustmentValue(
  field: keyof MoodBoardImageAdjustments,
  value: string,
  current: MoodBoardImageAdjustments,
) {
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue)) return current[field];
  if (field === "opacity") return Math.min(1, Math.max(0, numberValue));
  if (field === "brightness" || field === "contrast") return Math.min(200, Math.max(0, numberValue));
  if (field === "saturation") return Math.min(250, Math.max(0, numberValue));
  if (field === "grayscale") return Math.min(100, Math.max(0, numberValue));
  return Math.min(24, Math.max(0, numberValue));
}

export function moodBoardImageFramingValue(
  field: keyof MoodBoardImageFraming,
  value: string,
  current: MoodBoardImageFraming,
) {
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue)) return current[field];
  if (field === "scale") return Math.min(300, Math.max(100, numberValue));
  return Math.min(100, Math.max(0, numberValue));
}

export function moodBoardTextFontSizeValue(value: string) {
  return Math.min(120, Math.max(10, numberOrFallback(value, 10)));
}

export function moodBoardShapeStrokeWidthValue(value: string) {
  return Math.min(24, Math.max(0, numberOrFallback(value, 0)));
}

export function moodBoardVectorStrokeWidthValue(value: string) {
  return Math.min(32, Math.max(0, numberOrFallback(value, 0)));
}

export function moodBoardSvgStrokeWidthValue(value: string) {
  return Math.min(64, Math.max(0, numberOrFallback(value, 0)));
}

export function moodBoardSvgOpacityValue(value: string) {
  return Math.min(1, Math.max(0, numberOrFallback(value, 0)));
}

export function moodBoardSvgTextFontSizeValue(value: string) {
  return Math.min(512, Math.max(1, numberOrFallback(value, 16)));
}

export function moodBoardColorInputValue(value: string | undefined, fallback: string) {
  const color = value?.trim().toLowerCase() ?? "";
  if (/^#[0-9a-f]{6}$/i.test(color)) return color;
  if (/^#[0-9a-f]{3}$/i.test(color)) {
    return `#${color[1]}${color[1]}${color[2]}${color[2]}${color[3]}${color[3]}`;
  }
  if (color === "black") return "#000000";
  if (color === "white") return "#ffffff";
  return fallback;
}
