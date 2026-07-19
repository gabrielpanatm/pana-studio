import type { MoodBoardImageAdjustments } from "$lib/mood-board/model";

export const defaultImageAdjustments: MoodBoardImageAdjustments = {
  opacity: 1,
  brightness: 100,
  contrast: 100,
  saturation: 100,
  grayscale: 0,
  blur: 0,
};

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

export function normalizeImageAdjustments(value: Partial<MoodBoardImageAdjustments> | null | undefined): MoodBoardImageAdjustments {
  const source = value ?? {};
  return {
    opacity: clamp(Number.isFinite(source.opacity) ? source.opacity ?? 1 : 1, 0, 1),
    brightness: clamp(Number.isFinite(source.brightness) ? source.brightness ?? 100 : 100, 0, 200),
    contrast: clamp(Number.isFinite(source.contrast) ? source.contrast ?? 100 : 100, 0, 200),
    saturation: clamp(Number.isFinite(source.saturation) ? source.saturation ?? 100 : 100, 0, 250),
    grayscale: clamp(Number.isFinite(source.grayscale) ? source.grayscale ?? 0 : 0, 0, 100),
    blur: clamp(Number.isFinite(source.blur) ? source.blur ?? 0 : 0, 0, 24),
  };
}

export function imageFilterValue(adjustments: Partial<MoodBoardImageAdjustments> | null | undefined) {
  const value = normalizeImageAdjustments(adjustments);
  const filters = [
    `brightness(${value.brightness}%)`,
    `contrast(${value.contrast}%)`,
    `saturate(${value.saturation}%)`,
  ];
  if (value.grayscale > 0) filters.push(`grayscale(${value.grayscale}%)`);
  if (value.blur > 0) filters.push(`blur(${value.blur}px)`);
  return filters.join(" ");
}

export function imageOpacityValue(adjustments: Partial<MoodBoardImageAdjustments> | null | undefined) {
  return normalizeImageAdjustments(adjustments).opacity;
}

export function isDefaultImageAdjustments(adjustments: Partial<MoodBoardImageAdjustments> | null | undefined) {
  const value = normalizeImageAdjustments(adjustments);
  return value.opacity === defaultImageAdjustments.opacity
    && value.brightness === defaultImageAdjustments.brightness
    && value.contrast === defaultImageAdjustments.contrast
    && value.saturation === defaultImageAdjustments.saturation
    && value.grayscale === defaultImageAdjustments.grayscale
    && value.blur === defaultImageAdjustments.blur;
}
