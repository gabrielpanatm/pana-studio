import type { MoodBoardImageFraming } from "$lib/mood-board/model";

export const defaultImageFraming: MoodBoardImageFraming = {
  positionX: 50,
  positionY: 50,
  scale: 100,
};

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function finiteNumber(value: unknown, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

export function normalizeImageFraming(value: Partial<MoodBoardImageFraming> | null | undefined): MoodBoardImageFraming {
  const source = value ?? {};
  return {
    positionX: clamp(finiteNumber(source.positionX, defaultImageFraming.positionX), 0, 100),
    positionY: clamp(finiteNumber(source.positionY, defaultImageFraming.positionY), 0, 100),
    scale: clamp(finiteNumber(source.scale, defaultImageFraming.scale), 100, 300),
  };
}

export function imageObjectPositionValue(framing: Partial<MoodBoardImageFraming> | null | undefined) {
  const value = normalizeImageFraming(framing);
  return `${value.positionX}% ${value.positionY}%`;
}

export function imageTransformValue(framing: Partial<MoodBoardImageFraming> | null | undefined) {
  const value = normalizeImageFraming(framing);
  return `scale(${value.scale / 100})`;
}

export function imageTransformOriginValue(framing: Partial<MoodBoardImageFraming> | null | undefined) {
  const value = normalizeImageFraming(framing);
  return `${value.positionX}% ${value.positionY}%`;
}

export function imageDrawRect(
  naturalWidth: number,
  naturalHeight: number,
  fit: "cover" | "contain",
  width: number,
  height: number,
  framing: Partial<MoodBoardImageFraming> | null | undefined,
) {
  const sourceRatio = naturalWidth / Math.max(1, naturalHeight);
  const targetRatio = width / Math.max(1, height);
  const useWidth = fit === "cover" ? sourceRatio < targetRatio : sourceRatio > targetRatio;
  const baseWidth = useWidth ? width : height * sourceRatio;
  const baseHeight = useWidth ? width / sourceRatio : height;
  const value = normalizeImageFraming(framing);
  const drawWidth = baseWidth * (value.scale / 100);
  const drawHeight = baseHeight * (value.scale / 100);

  return {
    x: (width - drawWidth) * (value.positionX / 100),
    y: (height - drawHeight) * (value.positionY / 100),
    width: drawWidth,
    height: drawHeight,
  };
}

export function isDefaultImageFraming(framing: Partial<MoodBoardImageFraming> | null | undefined) {
  const value = normalizeImageFraming(framing);
  return value.positionX === defaultImageFraming.positionX
    && value.positionY === defaultImageFraming.positionY
    && value.scale === defaultImageFraming.scale;
}
