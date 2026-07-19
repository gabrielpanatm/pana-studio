import type { MotionTimelineClip } from "$lib/js/motion-timeline";
import {
  MOTION_TIMELINE_MIN_DURATION_MS,
  MOTION_TIMELINE_ROW_HEIGHT,
  formatMotionTimelinePosition,
  quantizeMotionMs,
} from "$lib/js/motion-timeline";
import { listenForExternalReconcileInteractionBarrier } from "$lib/session/external-reconcile-barrier";

export type MotionTimelineDragMode = "move" | "resize";

export type MotionTimelineTiming = {
  startMs: number;
  durationMs: number;
};

export type MotionTimelineTimingPatch = {
  position?: string;
  duration?: number;
};

export type MotionTimelineDragCommit = {
  stepId: string;
  stepIndex: number;
  patch: MotionTimelineTimingPatch;
  timing: MotionTimelineTiming;
};

export type MotionTimelineDragController = {
  cancel: () => void;
};

type MotionTimelineDragOptions = {
  event: PointerEvent;
  clip: MotionTimelineClip;
  captureTarget: HTMLElement;
  mode: MotionTimelineDragMode;
  maxMs: number;
  trackWidthPx: number;
  onPreview: (timing: MotionTimelineTiming | null) => void;
  onSelect: (stepId: string) => void;
  onCommit: (commit: MotionTimelineDragCommit) => void;
  onFinish?: () => void;
};

export type MotionTimelineDragSession = {
  mode: MotionTimelineDragMode;
  originX: number;
  originStartMs: number;
  originDurationMs: number;
  msPerPx: number;
  maxMs: number;
};

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

export function motionTimelineClipStyle(timing: MotionTimelineTiming, maxMs: number, rowIndex: number): string {
  const safeMax = Math.max(1, maxMs);
  const left = clamp((timing.startMs / safeMax) * 100, 0, 100);
  const right = clamp(((timing.startMs + timing.durationMs) / safeMax) * 100, left, 100);
  const width = Math.max(1.5, right - left);
  const top = 5 + Math.max(0, rowIndex) * MOTION_TIMELINE_ROW_HEIGHT;
  return `left:${left}%;width:${width}%;top:${top}px;`;
}

export function calculateMotionTimelineDragTiming(session: MotionTimelineDragSession, clientX: number): MotionTimelineTiming {
  const deltaMs = (clientX - session.originX) * session.msPerPx;
  if (session.mode === "move") {
    const maxStart = Math.max(0, session.maxMs - MOTION_TIMELINE_MIN_DURATION_MS);
    return {
      startMs: clamp(quantizeMotionMs(session.originStartMs + deltaMs), 0, maxStart),
      durationMs: session.originDurationMs,
    };
  }

  return {
    startMs: session.originStartMs,
    durationMs: clamp(
      quantizeMotionMs(session.originDurationMs + deltaMs),
      MOTION_TIMELINE_MIN_DURATION_MS,
      Math.max(MOTION_TIMELINE_MIN_DURATION_MS, session.maxMs - session.originStartMs),
    ),
  };
}

function timingPatchFromDrag(
  session: MotionTimelineDragSession,
  timing: MotionTimelineTiming,
): MotionTimelineTimingPatch | null {
  const patch: MotionTimelineTimingPatch = {};
  const nextPosition = formatMotionTimelinePosition(timing.startMs);
  const originPosition = formatMotionTimelinePosition(session.originStartMs);
  if (nextPosition !== originPosition) patch.position = nextPosition;
  if (timing.durationMs !== session.originDurationMs) patch.duration = timing.durationMs;
  return Object.keys(patch).length > 0 ? patch : null;
}

export function beginMotionTimelineClipDrag(options: MotionTimelineDragOptions): MotionTimelineDragController {
  const { event, clip, captureTarget, mode, maxMs } = options;
  const pointerId = event.pointerId;
  const session: MotionTimelineDragSession = {
    mode,
    originX: event.clientX,
    originStartMs: clip.startMs,
    originDurationMs: clip.durationMs,
    msPerPx: Math.max(1, maxMs) / Math.max(1, options.trackWidthPx),
    maxMs,
  };

  let latestClientX = event.clientX;
  let latestTiming: MotionTimelineTiming = {
    startMs: clip.startMs,
    durationMs: clip.durationMs,
  };
  let animationFrame: number | null = null;
  let moved = false;
  let finished = false;
  let safetyTimer: number | null = null;
  let stopExternalReconcileBarrier = () => {};

  const releaseCaptureAfterDispatch = () => {
    const release = () => {
      try {
        if (captureTarget.isConnected !== false && captureTarget.hasPointerCapture?.(pointerId)) {
          captureTarget.releasePointerCapture(pointerId);
        }
      } catch {
        // Pointer capture cleanup is best effort.
      }
    };
    if (typeof window.queueMicrotask === "function") window.queueMicrotask(release);
    else Promise.resolve().then(release);
  };

  const reportCallbackError = (error: unknown) => {
    console.error("[Pana Motion] Timeline drag callback failed", error);
  };

  const cleanup = (releaseCapture: boolean) => {
    if (animationFrame !== null) {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }
    if (safetyTimer !== null) {
      window.clearTimeout(safetyTimer);
      safetyTimer = null;
    }
    window.removeEventListener("pointermove", handlePointerMove, true);
    window.removeEventListener("pointerup", handlePointerUp, true);
    window.removeEventListener("pointercancel", handlePointerCancel, true);
    document.removeEventListener("visibilitychange", handleVisibilityChange);
    window.removeEventListener("blur", handleWindowBlur);
    window.removeEventListener("keydown", handleKeydown);
    window.removeEventListener("dragstart", handleNativeDragStart, true);
    window.removeEventListener("selectstart", handleNativeSelectStart, true);
    captureTarget.removeEventListener("lostpointercapture", handleLostPointerCapture);
    stopExternalReconcileBarrier();
    document.body?.classList.remove("motion-timeline-dragging");
    if (releaseCapture) releaseCaptureAfterDispatch();
  };

  const finish = (commit: boolean, releaseCapture = true) => {
    if (finished) return;
    finished = true;
    if (animationFrame !== null) {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = null;
      latestTiming = calculateMotionTimelineDragTiming(session, latestClientX);
    }
    cleanup(releaseCapture);

    try {
      if (!commit || !moved) {
        options.onPreview(null);
        if (commit) options.onSelect(clip.id);
        return;
      }

      const patch = timingPatchFromDrag(session, latestTiming);
      if (!patch) {
        options.onPreview(null);
        options.onSelect(clip.id);
        return;
      }

      options.onCommit({
        stepId: clip.id,
        stepIndex: clip.stepIndex,
        patch,
        timing: latestTiming,
      });
    } catch (error) {
      reportCallbackError(error);
    } finally {
      try {
        options.onFinish?.();
      } catch (error) {
        reportCallbackError(error);
      }
    }
  };

  function flushPreview() {
    animationFrame = null;
    latestTiming = calculateMotionTimelineDragTiming(session, latestClientX);
    try {
      options.onPreview(latestTiming);
    } catch (error) {
      reportCallbackError(error);
      finish(false);
    }
  }

  function schedulePreview() {
    if (animationFrame !== null) return;
    animationFrame = window.requestAnimationFrame(flushPreview);
  }

  function handlePointerMove(moveEvent: PointerEvent) {
    if (finished || moveEvent.pointerId !== pointerId) return;
    if (
      typeof moveEvent.buttons === "number"
      && moveEvent.buttons === 0
      && (moveEvent.pointerType === "mouse" || moveEvent.pointerType === "pen")
    ) {
      finish(true);
      return;
    }
    moveEvent.preventDefault();
    moveEvent.stopPropagation();
    latestClientX = moveEvent.clientX;
    moved = moved || Math.abs(latestClientX - session.originX) > 2;
    schedulePreview();
  }

  function handlePointerUp(upEvent: PointerEvent) {
    if (finished || upEvent.pointerId !== pointerId) return;
    upEvent.preventDefault();
    upEvent.stopPropagation();
    // Pointer Events releases capture implicitly after pointerup dispatch.
    // Explicit release here is re-entrant in WebKitGTK and is unnecessary.
    finish(true, false);
  }

  function handlePointerCancel(cancelEvent: PointerEvent) {
    if (finished || cancelEvent.pointerId !== pointerId) return;
    finish(false, false);
  }

  function handleLostPointerCapture(captureEvent: PointerEvent) {
    if (finished || captureEvent.pointerId !== pointerId) return;
    finish(true, false);
  }

  function handleVisibilityChange() {
    if (document.visibilityState === "hidden") finish(false);
  }

  function handleWindowBlur() {
    finish(false);
  }

  function handleKeydown(keyEvent: KeyboardEvent) {
    if (keyEvent.key === "Escape") finish(false);
  }

  function handleNativeDragStart(dragEvent: DragEvent) {
    if (finished) return;
    dragEvent.preventDefault();
    dragEvent.stopPropagation();
    finish(false);
  }

  function handleNativeSelectStart(selectEvent: Event) {
    if (finished) return;
    selectEvent.preventDefault();
    selectEvent.stopPropagation();
  }

  event.preventDefault();
  event.stopPropagation();
  window.getSelection?.()?.removeAllRanges();
  document.body?.classList.add("motion-timeline-dragging");

  safetyTimer = window.setTimeout(() => finish(false), 8000);
  // Capture-phase window listeners cannot be bypassed by a child that stops
  // propagation and remain valid if the captured element is detached.
  window.addEventListener("pointermove", handlePointerMove, { capture: true, passive: false });
  window.addEventListener("pointerup", handlePointerUp, { capture: true });
  window.addEventListener("pointercancel", handlePointerCancel, { capture: true });
  document.addEventListener("visibilitychange", handleVisibilityChange);
  window.addEventListener("blur", handleWindowBlur, { once: true });
  window.addEventListener("keydown", handleKeydown);
  window.addEventListener("dragstart", handleNativeDragStart, { capture: true });
  window.addEventListener("selectstart", handleNativeSelectStart, { capture: true });
  stopExternalReconcileBarrier = listenForExternalReconcileInteractionBarrier(() => finish(false));
  captureTarget.addEventListener("lostpointercapture", handleLostPointerCapture);
  try {
    captureTarget.setPointerCapture(pointerId);
  } catch {
    // Window capture listeners still keep drag controlled if pointer capture is unavailable.
  }

  return {
    cancel: () => finish(false),
  };
}
