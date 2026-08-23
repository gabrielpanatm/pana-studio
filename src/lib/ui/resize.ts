import { defaultTerminalPaneHeight } from "$lib/terminal/runtime";
import {
  startPointerSession,
  type PointerCaptureTarget,
  type PointerSessionEnvironment,
} from "$lib/ui/pointer-session";

export type ResizeKind = "left" | "right" | "terminal";

type ResizeState = {
  leftPaneWidth: number;
  rightPaneWidth: number;
  terminalPaneHeight: number;
};

type BeginResizeDragOptions = {
  kind: ResizeKind;
  event: PointerEvent;
  state: ResizeState;
  applyLiveState?: (nextState: ResizeState) => void;
  onUpdate: (nextState: ResizeState) => void;
  onStop: () => void;
  captureTarget?: PointerCaptureTarget | null;
  environment?: PointerSessionEnvironment;
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
  let latestX = startX;
  let latestY = startY;

  applyResizeBodyClasses(options.kind);

  const nextResizeState = (clientX: number, clientY: number): ResizeState => {
    if (options.kind === "left") {
      return {
        ...startState,
        leftPaneWidth: clampResizeValue("left", startState.leftPaneWidth + (clientX - startX)),
      };
    }

    if (options.kind === "right") {
      return {
        ...startState,
        rightPaneWidth: clampResizeValue("right", startState.rightPaneWidth - (clientX - startX)),
      };
    }

    return {
      ...startState,
      terminalPaneHeight: clampResizeValue("terminal", startState.terminalPaneHeight - (clientY - startY)),
    };
  };

  const publishLiveResize = () => {
    const nextState = nextResizeState(latestX, latestY);
    if (options.applyLiveState) options.applyLiveState(nextState);
    else options.onUpdate(nextState);
  };

  const session = startPointerSession({
    pointerId: options.event.pointerId,
    captureTarget: options.captureTarget
      ?? (typeof HTMLElement !== "undefined" && options.event.currentTarget instanceof HTMLElement
        ? options.event.currentTarget
        : null),
    safetyTimeoutMs: 8_000,
    environment: options.environment,
    onMove: (event, currentSession) => {
      event.preventDefault();
      latestX = event.clientX;
      latestY = event.clientY;
      currentSession.requestFrame(publishLiveResize);
    },
    onCommit: (event, currentSession) => {
      if (event) {
        latestX = event.clientX;
        latestY = event.clientY;
      }
      currentSession.flushFrame();
      options.onUpdate(nextResizeState(latestX, latestY));
      options.onStop();
    },
    onCancel: () => {
      options.onUpdate(startState);
      options.onStop();
    },
  });

  return () => session.destroy();
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
