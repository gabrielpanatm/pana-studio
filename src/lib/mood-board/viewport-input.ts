import type { MoodBoard } from "$lib/mood-board/model";

export type NativeCanvasZoomPayload = {
  x: number;
  y: number;
  scale: number;
  phase: number;
};

const nativePinchPhaseBegin = 0;
const nativePinchPhaseUpdate = 1;
const nativePinchPhaseEnd = 2;
const nativePinchPhaseCancel = 3;
const domDeltaPixel = 0;

export type MoodBoardWheelIntent =
  | { kind: "pan"; deltaX: number; deltaY: number }
  | { kind: "zoom"; zoomFactor: number };

export function isNativeCanvasZoomBegin(phase: number) {
  return phase === nativePinchPhaseBegin;
}

export function isNativeCanvasZoomUpdate(phase: number) {
  return phase === nativePinchPhaseUpdate;
}

export function isNativeCanvasZoomEnd(phase: number) {
  return phase === nativePinchPhaseEnd || phase === nativePinchPhaseCancel;
}

export function moodBoardWheelIntent(event: WheelEvent): MoodBoardWheelIntent {
  const normalizedDeltaX = event.deltaMode === domDeltaPixel ? event.deltaX : event.deltaX * 40;
  const normalizedDeltaY = event.deltaMode === domDeltaPixel ? event.deltaY : event.deltaY * 40;
  const looksLikeMouseWheel = Math.abs(normalizedDeltaX) < 1 && Math.abs(normalizedDeltaY) >= 50;
  const shouldZoom = event.ctrlKey || event.metaKey || looksLikeMouseWheel;

  if (!shouldZoom) return { kind: "pan", deltaX: normalizedDeltaX, deltaY: normalizedDeltaY };
  return { kind: "zoom", zoomFactor: Math.exp(-normalizedDeltaY * 0.0016) };
}

export function moodBoardPanViewportByWheel(baseBoard: MoodBoard, deltaX: number, deltaY: number): MoodBoard {
  return {
    ...baseBoard,
    viewport: {
      ...baseBoard.viewport,
      x: baseBoard.viewport.x - deltaX,
      y: baseBoard.viewport.y - deltaY,
    },
  };
}
