import { defaultTerminalPaneHeight } from "$lib/terminal/runtime";

export type ResizeKind = "left" | "right" | "terminal";

type ResizeState = {
  leftPaneWidth: number;
  rightPaneWidth: number;
  terminalPaneHeight: number;
};

type BeginResizeDragOptions = {
  kind: ResizeKind;
  event: MouseEvent;
  state: ResizeState;
  applyLiveState?: (nextState: ResizeState) => void;
  onUpdate: (nextState: ResizeState) => void;
  onStop: () => void;
};

export function clampResizeValue(kind: ResizeKind, value: number) {
  if (kind === "left") {
    return clamp(value, 220, 460);
  }

  if (kind === "right") {
    return clamp(value, 280, 520);
  }

  return clamp(value, 160, 480);
}

export function defaultResizeValue(kind: ResizeKind) {
  if (kind === "left") {
    return 260;
  }

  if (kind === "right") {
    return 320;
  }

  return defaultTerminalPaneHeight;
}

export function applyResizeBodyClasses(kind: ResizeKind) {
  document.body.classList.add("is-resizing");
  document.body.classList.toggle("is-col-resizing", kind === "left" || kind === "right");
  document.body.classList.toggle("is-row-resizing", kind === "terminal");
}

export function clearResizeBodyClasses() {
  document.body.classList.remove("is-resizing", "is-col-resizing", "is-row-resizing");
}

export function beginResizeDrag(options: BeginResizeDragOptions) {
  options.event.preventDefault();

  const startX = options.event.clientX;
  const startY = options.event.clientY;
  const startState = { ...options.state };
  let animationFrame: number | null = null;
  let latestMoveEvent: MouseEvent | null = null;
  let stopped = false;
  let safetyTimer: number | null = null;

  applyResizeBodyClasses(options.kind);

  const nextResizeState = (moveEvent: MouseEvent): ResizeState => {
    if (options.kind === "left") {
      return {
        ...startState,
        leftPaneWidth: clampResizeValue("left", startState.leftPaneWidth + (moveEvent.clientX - startX)),
      };
    }

    if (options.kind === "right") {
      return {
        ...startState,
        rightPaneWidth: clampResizeValue("right", startState.rightPaneWidth - (moveEvent.clientX - startX)),
      };
    }

    return {
      ...startState,
      terminalPaneHeight: clampResizeValue("terminal", startState.terminalPaneHeight - (moveEvent.clientY - startY)),
    };
  };

  const flushResize = () => {
    animationFrame = null;
    if (!latestMoveEvent) return;
    const nextState = nextResizeState(latestMoveEvent);
    if (options.applyLiveState) options.applyLiveState(nextState);
    else options.onUpdate(nextState);
  };

  const handlePointerMove = (moveEvent: MouseEvent) => {
    moveEvent.preventDefault();
    latestMoveEvent = moveEvent;
    if (animationFrame !== null) return;
    animationFrame = window.requestAnimationFrame(flushResize);
  };

  const stopDrag = (commit: boolean) => {
    if (stopped) return;
    stopped = true;
    if (animationFrame !== null) {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }
    if (commit && latestMoveEvent) {
      options.onUpdate(nextResizeState(latestMoveEvent));
    }
    latestMoveEvent = null;
    options.onStop();
  };

  const handlePointerUp = () => stopDrag(true);
  const cancelDrag = () => stopDrag(false);
  const handleVisibilityChange = () => {
    if (document.visibilityState === "hidden") cancelDrag();
  };
  const handleKeydown = (keyEvent: KeyboardEvent) => {
    if (keyEvent.key === "Escape") cancelDrag();
  };

  safetyTimer = window.setTimeout(cancelDrag, 8000);
  window.addEventListener("mousemove", handlePointerMove);
  window.addEventListener("mouseup", handlePointerUp, { once: true });
  window.addEventListener("blur", cancelDrag, { once: true });
  window.addEventListener("keydown", handleKeydown);
  document.addEventListener("visibilitychange", handleVisibilityChange);

  return () => {
    if (animationFrame !== null) {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }
    if (safetyTimer !== null) {
      window.clearTimeout(safetyTimer);
      safetyTimer = null;
    }
    window.removeEventListener("mousemove", handlePointerMove);
    window.removeEventListener("mouseup", handlePointerUp);
    window.removeEventListener("blur", cancelDrag);
    window.removeEventListener("keydown", handleKeydown);
    document.removeEventListener("visibilitychange", handleVisibilityChange);
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
