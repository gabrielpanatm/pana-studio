export type PointerSessionCancelReason =
  | "pointercancel"
  | "blur"
  | "escape"
  | "hidden"
  | "timeout"
  | "identity-change"
  | "replaced"
  | "programmatic"
  | "destroy";

export type PointerCaptureTarget = Pick<
  HTMLElement,
  "setPointerCapture" | "hasPointerCapture" | "releasePointerCapture"
>;

type ListenerTarget = {
  addEventListener: (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ) => void;
  removeEventListener: (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ) => void;
};

export type PointerSessionEnvironment = {
  windowTarget: ListenerTarget;
  documentTarget: ListenerTarget & { readonly visibilityState: DocumentVisibilityState };
  requestAnimationFrame: (callback: FrameRequestCallback) => number;
  cancelAnimationFrame: (handle: number) => void;
  setTimeout: (callback: () => void, timeoutMs: number) => number;
  clearTimeout: (handle: number) => void;
};

export type PointerSessionOptions = {
  pointerId: number;
  captureTarget?: PointerCaptureTarget | null;
  safetyTimeoutMs?: number;
  environment?: PointerSessionEnvironment;
  onMove?: (event: PointerEvent, session: PointerSession) => void;
  onCommit?: (event: PointerEvent | null, session: PointerSession) => void;
  onCancel?: (reason: PointerSessionCancelReason, session: PointerSession) => void;
};

function browserEnvironment(): PointerSessionEnvironment {
  return {
    windowTarget: window,
    documentTarget: document,
    requestAnimationFrame: (callback) => window.requestAnimationFrame(callback),
    cancelAnimationFrame: (handle) => window.cancelAnimationFrame(handle),
    setTimeout: (callback, timeoutMs) => window.setTimeout(callback, timeoutMs),
    clearTimeout: (handle) => window.clearTimeout(handle),
  };
}

export class PointerSession {
  readonly pointerId: number;

  private readonly options: PointerSessionOptions;
  private readonly environment: PointerSessionEnvironment;
  private activeValue = true;
  private frameHandle: number | null = null;
  private frameCallback: (() => void) | null = null;
  private safetyTimer: number | null = null;

  constructor(options: PointerSessionOptions) {
    this.options = options;
    this.pointerId = options.pointerId;
    this.environment = options.environment ?? browserEnvironment();
    this.capturePointer();
    this.listen();
    const timeoutMs = options.safetyTimeoutMs ?? 8_000;
    if (timeoutMs > 0) {
      this.safetyTimer = this.environment.setTimeout(() => {
        this.cancel("timeout");
      }, timeoutMs);
    }
  }

  get active() {
    return this.activeValue;
  }

  get hasPendingFrame() {
    return this.frameHandle !== null;
  }

  requestFrame(callback: () => void): boolean {
    if (!this.activeValue || this.frameHandle !== null) return false;
    this.frameCallback = callback;
    this.frameHandle = this.environment.requestAnimationFrame(() => {
      this.frameHandle = null;
      const pending = this.frameCallback;
      this.frameCallback = null;
      if (this.activeValue) pending?.();
    });
    return true;
  }

  flushFrame(): boolean {
    if (this.frameHandle === null) return false;
    this.environment.cancelAnimationFrame(this.frameHandle);
    this.frameHandle = null;
    const pending = this.frameCallback;
    this.frameCallback = null;
    pending?.();
    return Boolean(pending);
  }

  cancelFrame(): boolean {
    if (this.frameHandle === null) return false;
    this.environment.cancelAnimationFrame(this.frameHandle);
    this.frameHandle = null;
    this.frameCallback = null;
    return true;
  }

  commit(event: PointerEvent | null = null): boolean {
    if (!this.activeValue) return false;
    this.activeValue = false;
    try {
      this.options.onCommit?.(event, this);
    } finally {
      this.teardown();
    }
    return true;
  }

  cancel(reason: PointerSessionCancelReason = "programmatic"): boolean {
    if (!this.activeValue) return false;
    this.activeValue = false;
    this.teardown();
    this.options.onCancel?.(reason, this);
    return true;
  }

  destroy(): boolean {
    return this.cancel("destroy");
  }

  private readonly handlePointerMove: EventListener = (event) => {
    const pointerEvent = event as PointerEvent;
    if (!this.activeValue || pointerEvent.pointerId !== this.pointerId) return;
    this.options.onMove?.(pointerEvent, this);
  };

  private readonly handlePointerUp: EventListener = (event) => {
    const pointerEvent = event as PointerEvent;
    if (pointerEvent.pointerId !== this.pointerId) return;
    this.commit(pointerEvent);
  };

  private readonly handlePointerCancel: EventListener = (event) => {
    const pointerEvent = event as PointerEvent;
    if (pointerEvent.pointerId !== this.pointerId) return;
    this.cancel("pointercancel");
  };

  private readonly handleBlur: EventListener = () => {
    this.cancel("blur");
  };

  private readonly handleKeydown: EventListener = (event) => {
    const keyboardEvent = event as KeyboardEvent;
    if (keyboardEvent.key !== "Escape") return;
    keyboardEvent.preventDefault();
    this.cancel("escape");
  };

  private readonly handleVisibilityChange: EventListener = () => {
    if (this.environment.documentTarget.visibilityState === "hidden") {
      this.cancel("hidden");
    }
  };

  private capturePointer() {
    try {
      this.options.captureTarget?.setPointerCapture(this.pointerId);
    } catch {
      // Window listeners remain the fallback when capture is unavailable.
    }
  }

  private releasePointer() {
    const target = this.options.captureTarget;
    if (!target) return;
    try {
      if (target.hasPointerCapture(this.pointerId)) {
        target.releasePointerCapture(this.pointerId);
      }
    } catch {
      // The browser may already have released capture after pointerup.
    }
  }

  private listen() {
    const windowTarget = this.environment.windowTarget;
    windowTarget.addEventListener("pointermove", this.handlePointerMove);
    windowTarget.addEventListener("pointerup", this.handlePointerUp);
    windowTarget.addEventListener("pointercancel", this.handlePointerCancel);
    windowTarget.addEventListener("blur", this.handleBlur);
    windowTarget.addEventListener("keydown", this.handleKeydown);
    this.environment.documentTarget.addEventListener(
      "visibilitychange",
      this.handleVisibilityChange,
    );
  }

  private teardown() {
    this.cancelFrame();
    if (this.safetyTimer !== null) {
      this.environment.clearTimeout(this.safetyTimer);
      this.safetyTimer = null;
    }
    this.releasePointer();
    const windowTarget = this.environment.windowTarget;
    windowTarget.removeEventListener("pointermove", this.handlePointerMove);
    windowTarget.removeEventListener("pointerup", this.handlePointerUp);
    windowTarget.removeEventListener("pointercancel", this.handlePointerCancel);
    windowTarget.removeEventListener("blur", this.handleBlur);
    windowTarget.removeEventListener("keydown", this.handleKeydown);
    this.environment.documentTarget.removeEventListener(
      "visibilitychange",
      this.handleVisibilityChange,
    );
  }
}

export function startPointerSession(options: PointerSessionOptions) {
  return new PointerSession(options);
}
